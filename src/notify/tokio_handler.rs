// Copyright (C) 2025 Takayuki Sato. All Rights Reserved.
// This program is free software under MIT License.
// See the file LICENSE in this distribution for more details.

use super::{ErrHandlingError, ErrHandlingErrorKind};
use crate::Err;

use chrono::{DateTime, Utc};
use setup_read_cleanup::{graceful::GracefulPhasedCellSync, PhasedErrorKind};

use std::{future::Future, pin::Pin, sync::Arc};

pub type TokioAsyncFn =
    Box<dyn Fn(Arc<Err>, DateTime<Utc>) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>;

#[allow(clippy::type_complexity)]
const NOOP: fn(&mut Vec<TokioAsyncFn>) -> Result<(), ErrHandlingError> = |_| Ok(());

pub(crate) static HANDLERS: GracefulPhasedCellSync<Vec<TokioAsyncFn>> =
    GracefulPhasedCellSync::new(Vec::new());

pub(crate) fn add_tokio_async_handler<F, Fut>(
    handlers: &GracefulPhasedCellSync<Vec<TokioAsyncFn>>,
    handler: F,
) -> Result<(), ErrHandlingError>
where
    F: Fn(Arc<Err>, DateTime<Utc>) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    match handlers.lock() {
        Ok(mut v) => {
            v.push(Box::new(move |err, tm| Box::pin(handler(err, tm))));
            Ok(())
        }
        Err(e) => match e.kind() {
            PhasedErrorKind::InternalDataUnavailable => Err(ErrHandlingError::new(
                ErrHandlingErrorKind::InvalidInternalState,
            )),
            PhasedErrorKind::InternalDataMutexIsPoisoned => Err(ErrHandlingError::new(
                ErrHandlingErrorKind::StdMutexIsPoisoned,
            )),
            _ => Err(ErrHandlingError::new(
                ErrHandlingErrorKind::InvalidCallTiming,
            )),
        },
    }
}

pub(crate) fn fix_handlers(
    handlers: &GracefulPhasedCellSync<Vec<TokioAsyncFn>>,
) -> Result<(), ErrHandlingError> {
    if let Err(e) = handlers.transition_to_read(NOOP) {
        match e.kind() {
            PhasedErrorKind::PhaseIsAlreadyRead => Ok(()),
            PhasedErrorKind::InternalDataUnavailable => Err(ErrHandlingError::new(
                ErrHandlingErrorKind::InvalidInternalState,
            )),
            PhasedErrorKind::InternalDataMutexIsPoisoned => Err(ErrHandlingError::new(
                ErrHandlingErrorKind::StdMutexIsPoisoned,
            )),
            // PhasedErrorKind::FailToRunClosureDuringTransitionToRead => {}, // impossible case
            _ => Err(ErrHandlingError::new(
                ErrHandlingErrorKind::InvalidCallTiming,
            )),
        }
    } else {
        Ok(())
    }
}

pub(crate) fn handle_err(
    handlers: &'static GracefulPhasedCellSync<Vec<TokioAsyncFn>>,
    err: Arc<Err>,
    tm: DateTime<Utc>,
) -> Result<(), ErrHandlingError> {
    let result = match handlers.transition_to_read(NOOP) {
        Ok(_) => handlers.read(),
        Err(e) => match e.kind() {
            PhasedErrorKind::PhaseIsAlreadyRead => handlers.read_relaxed(),
            PhasedErrorKind::DuringTransitionToRead => handlers.read(),
            PhasedErrorKind::InternalDataUnavailable => {
                return Err(ErrHandlingError::new(
                    ErrHandlingErrorKind::InvalidInternalState,
                ));
            }
            PhasedErrorKind::InternalDataMutexIsPoisoned => {
                return Err(ErrHandlingError::new(
                    ErrHandlingErrorKind::StdMutexIsPoisoned,
                ));
            }
            // PhasedErrorKind::FailToRunClosureDuringTransitionToRead => {}, // impossible case
            _ => {
                return Err(ErrHandlingError::new(
                    ErrHandlingErrorKind::InvalidCallTiming,
                ));
            }
        },
    };

    match result {
        Ok(v) => {
            if let Ok(rt_handle) = tokio::runtime::Handle::try_current() {
                for handle in v.iter() {
                    let e = Arc::clone(&err);
                    rt_handle.spawn(handle(e, tm));
                }
            } else {
                std::thread::spawn(move || {
                    let rt = match tokio::runtime::Runtime::new() {
                        Ok(rt) => rt,
                        Err(e) => {
                            eprintln!("ERROR(errs): Fail to create Tokio runtime: {e:?}");
                            return;
                        }
                    };

                    rt.block_on(async {
                        let mut rt_handles = Vec::new();
                        for handle in v.iter() {
                            let e = Arc::clone(&err);
                            rt_handles.push(tokio::spawn(handle(e, tm)));
                        }

                        for rt_handle in rt_handles {
                            if let Err(e) = rt_handle.await {
                                eprintln!("ERROR(errs): Fail to run tokio handler: {e:?}");
                            }
                        }
                    });
                });
            }
            Ok(())
        }
        Err(e) => match e.kind() {
            PhasedErrorKind::InternalDataUnavailable => Err(ErrHandlingError::new(
                ErrHandlingErrorKind::InvalidInternalState,
            )),
            PhasedErrorKind::GracefulWaitMutexIsPoisoned => Err(ErrHandlingError::new(
                ErrHandlingErrorKind::StdMutexIsPoisoned,
            )),
            _ => Err(ErrHandlingError::new(
                ErrHandlingErrorKind::InvalidCallTiming,
            )),
        },
    }
}

#[cfg(test)]
mod tests_of_notify {
    use super::*;

    #[derive(Debug)]
    enum Errors {
        FailToDoSomething,
    }

    mod tests_of_tokio_async_err_handling_on_tokio_runtime {
        use super::*;
        use std::sync::{LazyLock, Mutex};

        static HANDLERS: GracefulPhasedCellSync<Vec<TokioAsyncFn>> =
            GracefulPhasedCellSync::new(Vec::new());

        static LOGGERS: LazyLock<Mutex<Vec<String>>> = LazyLock::new(|| Mutex::new(Vec::new()));

        const LINE: u32 = line!();

        #[tokio::test]
        async fn add_and_fix_and_notify() {
            assert!(add_tokio_async_handler(&HANDLERS, async |err, _tm| {
                tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
                LOGGERS.lock().unwrap().push(format!("1: err={err:?}"));
            })
            .is_ok());
            // When rust version is less than 1.85.
            //assert!(
            //    add_tokio_async_handler(&HANDLERS, |err: Arc<Err>, _tm| Box::pin(async move {
            //        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
            //        LOGGERS.lock().unwrap().push(format!("1: err={err:?}"));
            //    }))
            //    .is_ok()
            //);

            assert!(add_tokio_async_handler(&HANDLERS, async |err, _tm| {
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                LOGGERS.lock().unwrap().push(format!("2: err={err:?}"));
            })
            .is_ok());
            // When rust version is less than 1.85.
            //assert!(
            //    add_tokio_async_handler(&HANDLERS, |err: Arc<Err>, _tm| Box::pin(async move {
            //        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            //        LOGGERS.lock().unwrap().push(format!("2: err={err:?}"));
            //    }))
            //    .is_ok()
            //);

            assert!(fix_handlers(&HANDLERS).is_ok());

            assert!(add_tokio_async_handler(&HANDLERS, async |err, _tm| {
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                LOGGERS.lock().unwrap().push(format!("1: err={err:?}"));
            })
            .is_err());
            // When rust version is less than 1.85.
            //assert!(
            //    add_tokio_async_handler(&HANDLERS, |err: Arc<Err>, _tm| Box::pin(async move {
            //        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            //        LOGGERS.lock().unwrap().push(format!("1: err={err:?}"));
            //    }))
            //    .is_err()
            //);

            let err = Err::new(Errors::FailToDoSomething);
            assert!(handle_err(&HANDLERS, err.into(), Utc::now()).is_ok());

            {
                let vec = LOGGERS.lock().unwrap();
                assert_eq!(vec.len(), 0);
            }

            tokio::time::sleep(std::time::Duration::from_millis(200)).await;

            #[cfg(unix)]
            {
                let vec = LOGGERS.lock().unwrap();
                assert_eq!(vec.len(), 2);
                assert_eq!(vec[0], format!("2: err=errs::Err {{ reason = errs::notify::tokio_handler::tests_of_notify::Errors FailToDoSomething, file = src/notify/tokio_handler.rs, line = {} }}", LINE + 48));
                assert_eq!(vec[1], format!("1: err=errs::Err {{ reason = errs::notify::tokio_handler::tests_of_notify::Errors FailToDoSomething, file = src/notify/tokio_handler.rs, line = {} }}", LINE + 48));
            }
            #[cfg(windows)]
            {
                let vec = LOGGERS.lock().unwrap();
                assert_eq!(vec.len(), 2);
                assert_eq!(vec[0], format!("2: err=errs::Err {{ reason = errs::notify::tokio_handler::tests_of_notify::Errors FailToDoSomething, file = src\\notify\\tokio_handler.rs, line = {} }}", LINE + 48));
                assert_eq!(vec[1], format!("1: err=errs::Err {{ reason = errs::notify::tokio_handler::tests_of_notify::Errors FailToDoSomething, file = src\\notify\\tokio_handler.rs, line = {} }}", LINE + 48));
            }
        }
    }

    mod tests_of_tokio_async_err_handling_on_thread {
        use super::*;
        use std::sync::{LazyLock, Mutex};

        static HANDLERS: GracefulPhasedCellSync<Vec<TokioAsyncFn>> =
            GracefulPhasedCellSync::new(Vec::new());

        static LOGGERS: LazyLock<Mutex<Vec<String>>> = LazyLock::new(|| Mutex::new(Vec::new()));

        const LINE: u32 = line!();

        #[test]
        fn add_and_fix_and_notify() {
            assert!(add_tokio_async_handler(&HANDLERS, async |err, _tm| {
                tokio::time::sleep(tokio::time::Duration::from_millis(90)).await;
                LOGGERS.lock().unwrap().push(format!("1: err={err:?}"));
            })
            .is_ok());
            // When rust version is less than 1.85.
            //assert!(
            //    add_tokio_async_handler(&HANDLERS, |err: Arc<Err>, _tm| Box::pin(async move {
            //        tokio::time::sleep(tokio::time::Duration::from_millis(90)).await;
            //        LOGGERS.lock().unwrap().push(format!("1: err={err:?}"));
            //    }))
            //    .is_ok()
            //);

            assert!(add_tokio_async_handler(&HANDLERS, async |err, _tm| {
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                LOGGERS.lock().unwrap().push(format!("2: err={err:?}"));
            })
            .is_ok());
            // When rust version is less than 1.85.
            //assert!(
            //    add_tokio_async_handler(&HANDLERS, |err: Arc<Err>, _tm| Box::pin(async move {
            //        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            //        LOGGERS.lock().unwrap().push(format!("2: err={err:?}"));
            //    }))
            //    .is_ok()
            //);

            assert!(fix_handlers(&HANDLERS).is_ok());

            assert!(add_tokio_async_handler(&HANDLERS, async |err, _tm| {
                std::thread::sleep(std::time::Duration::from_millis(10));
                LOGGERS.lock().unwrap().push(format!("1: err={err:?}"));
            })
            .is_err());
            // When rust version is less than 1.85.
            //assert!(
            //    add_tokio_async_handler(&HANDLERS, |err: Arc<Err>, _tm| Box::pin(async move {
            //        std::thread::sleep(std::::Duration::from_millis(10));
            //        LOGGERS.lock().unwrap().push(format!("1: err={err:?}"));
            //    }))
            //    .is_err()
            //);

            let err = Err::new(Errors::FailToDoSomething);
            assert!(handle_err(&HANDLERS, err.into(), Utc::now()).is_ok());

            {
                let vec = LOGGERS.lock().unwrap();
                assert_eq!(vec.len(), 0);
            }

            std::thread::sleep(std::time::Duration::from_millis(200));

            #[cfg(unix)]
            {
                let vec = LOGGERS.lock().unwrap();
                assert_eq!(vec.len(), 2);
                assert_eq!(vec[0], format!("2: err=errs::Err {{ reason = errs::notify::tokio_handler::tests_of_notify::Errors FailToDoSomething, file = src/notify/tokio_handler.rs, line = {} }}", LINE + 48));
                assert_eq!(vec[1], format!("1: err=errs::Err {{ reason = errs::notify::tokio_handler::tests_of_notify::Errors FailToDoSomething, file = src/notify/tokio_handler.rs, line = {} }}", LINE + 48));
            }
            #[cfg(windows)]
            {
                let vec = LOGGERS.lock().unwrap();
                assert_eq!(vec.len(), 2);
                assert_eq!(vec[0], format!("2: err=errs::Err {{ reason = errs::notify::tokio_handler::tests_of_notify::Errors FailToDoSomething, file = src\\notify\\tokio_handler.rs, line = {} }}", LINE + 48));
                assert_eq!(vec[1], format!("1: err=errs::Err {{ reason = errs::notify::tokio_handler::tests_of_notify::Errors FailToDoSomething, file = src\\notify\\tokio_handler.rs, line = {} }}", LINE + 48));
            }
        }
    }
}
