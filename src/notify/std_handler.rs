// Copyright (C) 2025 Takayuki Sato. All Rights Reserved.
// This program is free software under MIT License.
// See the file LICENSE in this distribution for more details.

use super::{ErrHandlingError, ErrHandlingErrorKind};
use crate::Err;

use chrono::{DateTime, Utc};
use setup_read_cleanup::{graceful::GracefulPhasedCellSync, PhasedErrorKind};

use std::{sync::Arc, thread};

type SyncBoxedFn = Box<dyn Fn(&Err, DateTime<Utc>) + Send + Sync + 'static>;
type AsyncArcFn = Arc<dyn Fn(&Err, DateTime<Utc>) + Send + Sync + 'static>;

#[allow(clippy::type_complexity)]
const NOOP: fn(&mut (Vec<SyncBoxedFn>, Vec<AsyncArcFn>)) -> Result<(), ErrHandlingError> =
    |_| Ok(());

pub(crate) static HANDLERS: GracefulPhasedCellSync<(Vec<SyncBoxedFn>, Vec<AsyncArcFn>)> =
    GracefulPhasedCellSync::new((Vec::new(), Vec::new()));

pub(crate) fn add_sync_handler<F>(
    handlers: &GracefulPhasedCellSync<(Vec<SyncBoxedFn>, Vec<AsyncArcFn>)>,
    handler: F,
) -> Result<(), ErrHandlingError>
where
    F: Fn(&Err, DateTime<Utc>) + Send + Sync + 'static,
{
    match handlers.lock() {
        Ok(mut vv) => {
            vv.0.push(Box::new(handler));
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

pub(crate) fn add_async_handler<F>(
    handlers: &GracefulPhasedCellSync<(Vec<SyncBoxedFn>, Vec<AsyncArcFn>)>,
    handler: F,
) -> Result<(), ErrHandlingError>
where
    F: Fn(&Err, DateTime<Utc>) + Send + Sync + 'static,
{
    match handlers.lock() {
        Ok(mut vv) => {
            vv.1.push(Arc::new(handler));
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
    handlers: &GracefulPhasedCellSync<(Vec<SyncBoxedFn>, Vec<AsyncArcFn>)>,
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
    handlers: &'static GracefulPhasedCellSync<(Vec<SyncBoxedFn>, Vec<AsyncArcFn>)>,
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
        Ok(vv) => {
            let err_clone = Arc::clone(&err);
            #[cfg(not(feature = "errs-notify-tokio"))]
            {
                thread::spawn(move || {
                    for handle in vv.1.iter() {
                        let e = Arc::clone(&err_clone);
                        let h = Arc::clone(handle);
                        thread::spawn(move || h(&e, tm));
                    }
                });
            }
            #[cfg(feature = "errs-notify-tokio")]
            {
                if let Ok(rt_handle) = tokio::runtime::Handle::try_current() {
                    thread::spawn(move || {
                        for handle in vv.1.iter() {
                            let e = Arc::clone(&err_clone);
                            let h = Arc::clone(handle);
                            rt_handle.spawn_blocking(move || h(&e, tm));
                        }
                    });
                } else {
                    thread::spawn(move || {
                        for handle in vv.1.iter() {
                            let e = Arc::clone(&err_clone);
                            let h = Arc::clone(handle);
                            thread::spawn(move || h(&e, tm));
                        }
                    });
                }
            }

            for handle in vv.0.iter() {
                handle(&err, tm);
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

    mod tests_of_sync_err_handling {
        use super::*;
        use std::sync::{LazyLock, Mutex};

        static HANDLERS: GracefulPhasedCellSync<(Vec<SyncBoxedFn>, Vec<AsyncArcFn>)> =
            GracefulPhasedCellSync::new((Vec::new(), Vec::new()));

        static LOGGERS: LazyLock<Mutex<Vec<String>>> = LazyLock::new(|| Mutex::new(Vec::new()));

        const LINE: u32 = line!();

        #[test]
        fn add_and_fix_and_notify() {
            assert!(add_sync_handler(&HANDLERS, |err, _tm| {
                LOGGERS.lock().unwrap().push(format!("1: err={err:?}"));
            })
            .is_ok());
            assert!(add_sync_handler(&HANDLERS, |err, _tm| {
                LOGGERS.lock().unwrap().push(format!("2: err={err:?}"));
            })
            .is_ok());

            assert!(fix_handlers(&HANDLERS).is_ok());

            assert!(add_sync_handler(&HANDLERS, |err, _tm| {
                LOGGERS.lock().unwrap().push(format!("3: err={err:?}"));
            })
            .is_err());

            let err = Err::new(Errors::FailToDoSomething);
            assert!(handle_err(&HANDLERS, err.into(), Utc::now()).is_ok());

            #[cfg(unix)]
            {
                let vec = LOGGERS.lock().unwrap();
                assert_eq!(vec.len(), 2);
                assert_eq!(vec[0], format!("1: err=errs::Err {{ reason = errs::notify::std_handler::tests_of_notify::Errors FailToDoSomething, file = src/notify/std_handler.rs, line = {} }}", LINE + 20));
                assert_eq!(vec[1], format!("2: err=errs::Err {{ reason = errs::notify::std_handler::tests_of_notify::Errors FailToDoSomething, file = src/notify/std_handler.rs, line = {} }}", LINE + 20));
            }
            #[cfg(windows)]
            {
                let vec = LOGGERS.lock().unwrap();
                assert_eq!(vec.len(), 2);
                assert_eq!(vec[0], format!("1: err=errs::Err {{ reason = errs::notify::std_handler::tests_of_notify::Errors FailToDoSomething, file = src\\notify\\std_handler.rs, line = {} }}", LINE + 20));
                assert_eq!(vec[1], format!("2: err=errs::Err {{ reason = errs::notify::std_handler::tests_of_notify::Errors FailToDoSomething, file = src\\notify\\std_handler.rs, line = {} }}", LINE + 20));
            }
        }
    }

    mod tests_of_async_err_handling {
        use super::*;
        use std::sync::{LazyLock, Mutex};

        static HANDLERS: GracefulPhasedCellSync<(Vec<SyncBoxedFn>, Vec<AsyncArcFn>)> =
            GracefulPhasedCellSync::new((Vec::new(), Vec::new()));

        static LOGGERS: LazyLock<Mutex<Vec<String>>> = LazyLock::new(|| Mutex::new(Vec::new()));

        const LINE: u32 = line!();

        #[test]
        fn add_and_fix_and_notify() {
            assert!(add_async_handler(&HANDLERS, |err, _tm| {
                thread::sleep(std::time::Duration::from_millis(50));
                LOGGERS.lock().unwrap().push(format!("1: err={err:?}"));
            })
            .is_ok());
            assert!(add_async_handler(&HANDLERS, |err, _tm| {
                thread::sleep(std::time::Duration::from_millis(10));
                LOGGERS.lock().unwrap().push(format!("2: err={err:?}"));
            })
            .is_ok());

            assert!(fix_handlers(&HANDLERS).is_ok());

            assert!(add_async_handler(&HANDLERS, |err, _tm| {
                thread::sleep(std::time::Duration::from_millis(10));
                LOGGERS.lock().unwrap().push(format!("3: err={err:?}"));
            })
            .is_err());

            let err = Err::new(Errors::FailToDoSomething);
            assert!(handle_err(&HANDLERS, err.into(), Utc::now()).is_ok());

            {
                let vec = LOGGERS.lock().unwrap();
                assert_eq!(vec.len(), 0);
            }

            thread::sleep(std::time::Duration::from_millis(200));

            #[cfg(unix)]
            {
                let vec = LOGGERS.lock().unwrap();
                assert_eq!(vec.len(), 2);
                assert_eq!(vec[0], format!("2: err=errs::Err {{ reason = errs::notify::std_handler::tests_of_notify::Errors FailToDoSomething, file = src/notify/std_handler.rs, line = {} }}", LINE + 23));
                assert_eq!(vec[1], format!("1: err=errs::Err {{ reason = errs::notify::std_handler::tests_of_notify::Errors FailToDoSomething, file = src/notify/std_handler.rs, line = {} }}", LINE + 23));
            }
            #[cfg(windows)]
            {
                let vec = LOGGERS.lock().unwrap();
                assert_eq!(vec.len(), 2);
                assert_eq!(vec[0], format!("2: err=errs::Err {{ reason = errs::notify::std_handler::tests_of_notify::Errors FailToDoSomething, file = src\\notify\\std_handler.rs, line = {} }}", LINE + 23));
                assert_eq!(vec[1], format!("1: err=errs::Err {{ reason = errs::notify::std_handler::tests_of_notify::Errors FailToDoSomething, file = src\\notify\\std_handler.rs, line = {} }}", LINE + 23));
            }
        }
    }

    #[cfg(feature = "errs-notify-tokio")]
    mod tests_of_async_err_handling_on_tokio {
        use super::*;
        use std::sync::{LazyLock, Mutex};
        use tokio::time::Duration;

        static HANDLERS: GracefulPhasedCellSync<(Vec<SyncBoxedFn>, Vec<AsyncArcFn>)> =
            GracefulPhasedCellSync::new((Vec::new(), Vec::new()));

        static LOGGERS: LazyLock<Mutex<Vec<String>>> = LazyLock::new(|| Mutex::new(Vec::new()));

        const LINE: u32 = line!();

        #[tokio::test]
        async fn add_and_fix_and_notify() {
            assert!(add_async_handler(&HANDLERS, |err, _tm| {
                thread::sleep(std::time::Duration::from_millis(50)); // intentionally block
                LOGGERS
                    .lock()
                    .unwrap()
                    .push(format!("tokio-1: err={err:?}"));
            })
            .is_ok());
            assert!(add_async_handler(&HANDLERS, |err, _tm| {
                thread::sleep(std::time::Duration::from_millis(10)); // intentionally block
                LOGGERS
                    .lock()
                    .unwrap()
                    .push(format!("tokio-2: err={err:?}"));
            })
            .is_ok());

            assert!(fix_handlers(&HANDLERS).is_ok());

            assert!(add_async_handler(&HANDLERS, |err, _tm| {
                thread::sleep(std::time::Duration::from_millis(10));
                LOGGERS.lock().unwrap().push(format!("3: err={err:?}"));
            })
            .is_err());

            let err = Err::new(Errors::FailToDoSomething);
            assert!(handle_err(&HANDLERS, Arc::new(err), Utc::now()).is_ok());

            {
                let vec = LOGGERS.lock().unwrap();
                assert_eq!(vec.len(), 0);
            }

            tokio::time::sleep(Duration::from_millis(200)).await;

            #[cfg(unix)]
            {
                let vec = LOGGERS.lock().unwrap();
                assert_eq!(vec.len(), 2);
                assert_eq!(vec[0], format!("tokio-2: err=errs::Err {{ reason = errs::notify::std_handler::tests_of_notify::Errors FailToDoSomething, file = src/notify/std_handler.rs, line = {} }}", LINE + 29));
                assert_eq!(vec[1], format!("tokio-1: err=errs::Err {{ reason = errs::notify::std_handler::tests_of_notify::Errors FailToDoSomething, file = src/notify/std_handler.rs, line = {} }}", LINE + 29));
            }
            #[cfg(windows)]
            {
                let vec = LOGGERS.lock().unwrap();
                assert_eq!(vec.len(), 2);
                assert_eq!(vec[0], format!("tokio-2: err=errs::Err {{ reason = errs::notify::std_handler::tests_of_notify::Errors FailToDoSomething, file = src\\notify\\std_handler.rs, line = {} }}", LINE + 29));
                assert_eq!(vec[1], format!("tokio-1: err=errs::Err {{ reason = errs::notify::std_handler::tests_of_notify::Errors FailToDoSomething, file = src\\notify\\std_handler.rs, line = {} }}", LINE + 29));
            }
        }
    }

    mod tests_of_no_handlers {
        use super::*;
        use std::sync::{LazyLock, Mutex};

        static HANDLERS: GracefulPhasedCellSync<(Vec<SyncBoxedFn>, Vec<AsyncArcFn>)> =
            GracefulPhasedCellSync::new((Vec::new(), Vec::new()));

        static LOGGERS: LazyLock<Mutex<Vec<String>>> = LazyLock::new(|| Mutex::new(Vec::new()));

        #[test]
        fn no_handlers_registered_should_not_panic() {
            assert!(fix_handlers(&HANDLERS).is_ok());

            let err = Err::new(Errors::FailToDoSomething);
            let result = handle_err(&HANDLERS, Arc::new(err), Utc::now());

            assert!(result.is_ok());
            assert!(LOGGERS.lock().unwrap().is_empty());
        }
    }
}
