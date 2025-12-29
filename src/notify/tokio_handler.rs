// Copyright (C) 2025 Takayuki Sato. All Rights Reserved.
// This program is free software under MIT License.
// See the file LICENSE in this distribution for more details.

use super::{ErrHandlingError, ErrHandlingErrorKind};
use crate::Err;

use chrono::{DateTime, Utc};
use setup_read_cleanup::{graceful::GracefulPhasedCellSync, PhasedErrorKind};

use std::{future::Future, pin::Pin, sync::Arc};

type TokioAsyncFn =
    Box<dyn Fn(Arc<Err>, DateTime<Utc>) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>;

type TokioAsyncRawFn = fn(Arc<Err>, DateTime<Utc>) -> Pin<Box<dyn Future<Output = ()> + Send>>;

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
    if let Err(e) = handlers.transition_to_read(register_handlers_by_inventory) {
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
    let result = match handlers.transition_to_read(register_handlers_by_inventory) {
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

#[doc(hidden)]
pub struct TokioAsyncHandlerRegistration {
    handler: TokioAsyncRawFn,
}
impl TokioAsyncHandlerRegistration {
    pub const fn new(handler: TokioAsyncRawFn) -> Self {
        Self { handler }
    }
}
inventory::collect!(TokioAsyncHandlerRegistration);

/// Statically registers a Tokio-based asynchronous error handler.
///
/// This macro provides a way to register an asynchronous error handler from a static context,
/// designed for integration with the Tokio runtime. It uses the `inventory` crate to collect
/// handlers at compile time. These handlers are spawned as Tokio tasks when an `Err`
/// instance is created.
///
/// This is the macro-based alternative to the [`add_tokio_async_err_handler`](crate::add_tokio_async_err_handler()) function.
///
/// The macro supports two forms:
/// 1. An `async` block: `add_tokio_async_err_handler!(async |err, tm| { ... });`
/// 2. A function pointer: `add_tokio_async_err_handler!(my_handler_fn);`
///
/// # Note
/// The handler function must have a signature compatible with
/// `fn(Arc<Err>, DateTime<Utc>) -> impl Future<Output = ()> + Send`.
///
/// # Examples
///
/// ### Using an `async` block
/// ```rust
/// use errs::{add_tokio_async_err_handler, Err};
/// use chrono::{DateTime, Utc};
/// use std::sync::Arc;
///
/// // Register the handler statically using an async block.
/// add_tokio_async_err_handler!(async |err: Arc<Err>, tm: DateTime<Utc>| {
///     // This will run as a Tokio task.
///     println!("[Tokio Handler] Error occurred at {}: {}", tm, err);
/// });
///
/// // In your application's initialization:
/// // errs::fix_err_handlers();
/// ```
///
/// ### Using a function pointer
/// ```rust
/// use errs::{add_tokio_async_err_handler, Err};
/// use chrono::{DateTime, Utc};
/// use std::sync::Arc;
/// use std::future::Future;
/// use std::pin::Pin;
///
/// fn my_tokio_handler(err: Arc<Err>, tm: DateTime<Utc>) -> Pin<Box<dyn Future<Output = ()> + Send>> {
///     Box::pin(async move {
///         println!("[Tokio Handler Fn] Error occurred at {}: {}", tm, err);
///     })
/// }
///
/// // Register the handler statically using a function pointer.
/// add_tokio_async_err_handler!(my_tokio_handler);
///
/// // In your application's initialization:
/// // errs::fix_err_handlers();
/// ```
#[macro_export]
macro_rules! add_tokio_async_err_handler {
    (async | $err:tt , $tm:tt | $body:block ) => {
        inventory::submit! {
            $crate::TokioAsyncHandlerRegistration::new(|$err: std::sync::Arc<$crate::Err>, $tm: chrono::DateTime<chrono::Utc>| {
                std::boxed::Box::pin(async move { $body })
            })
        }
    };

    (async | $err:tt : $errty:ty, $tm:tt : $tmty:ty | $body:block ) => {
        inventory::submit! {
            $crate::TokioAsyncHandlerRegistration::new(|$err: $errty, $tm: $tmty| {
                std::boxed::Box::pin(async move { $body })
            })
        }
    };

    ($handler:expr) => {
        inventory::submit! {
            $crate::TokioAsyncHandlerRegistration::new($handler)
        }
    };
}

fn register_handlers_by_inventory(v: &mut Vec<TokioAsyncFn>) -> Result<(), ErrHandlingError> {
    let vec: Vec<TokioAsyncFn> = inventory::iter::<TokioAsyncHandlerRegistration>
        .into_iter()
        .map(|reg| Box::new(reg.handler) as TokioAsyncFn)
        .collect();
    v.splice(0..0, vec);

    Ok(())
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

        static LOGGER: LazyLock<Mutex<Vec<String>>> = LazyLock::new(|| Mutex::new(Vec::new()));

        const LINE: u32 = line!();

        #[tokio::test]
        async fn add_and_fix_and_notify() {
            assert!(add_tokio_async_handler(&HANDLERS, async |err, _tm| {
                tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
                LOGGER.lock().unwrap().push(format!("1: err={err:?}"));
            })
            .is_ok());
            // When rust version is less than 1.85.
            //assert!(
            //    add_tokio_async_handler(&HANDLERS, |err: Arc<Err>, _tm| Box::pin(async move {
            //        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
            //        LOGGER.lock().unwrap().push(format!("1: err={err:?}"));
            //    }))
            //    .is_ok()
            //);

            assert!(add_tokio_async_handler(&HANDLERS, async |err, _tm| {
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                LOGGER.lock().unwrap().push(format!("2: err={err:?}"));
            })
            .is_ok());
            // When rust version is less than 1.85.
            //assert!(
            //    add_tokio_async_handler(&HANDLERS, |err: Arc<Err>, _tm| Box::pin(async move {
            //        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            //        LOGGER.lock().unwrap().push(format!("2: err={err:?}"));
            //    }))
            //    .is_ok()
            //);

            assert!(fix_handlers(&HANDLERS).is_ok());

            assert!(add_tokio_async_handler(&HANDLERS, async |err, _tm| {
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                LOGGER.lock().unwrap().push(format!("1: err={err:?}"));
            })
            .is_err());
            // When rust version is less than 1.85.
            //assert!(
            //    add_tokio_async_handler(&HANDLERS, |err: Arc<Err>, _tm| Box::pin(async move {
            //        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            //        LOGGER.lock().unwrap().push(format!("1: err={err:?}"));
            //    }))
            //    .is_err()
            //);

            let err = Err::new(Errors::FailToDoSomething);
            assert!(handle_err(&HANDLERS, err.into(), Utc::now()).is_ok());

            {
                let vec = LOGGER.lock().unwrap();
                assert_eq!(vec.len(), 0);
            }

            tokio::time::sleep(std::time::Duration::from_millis(200)).await;

            #[cfg(unix)]
            {
                let vec = LOGGER.lock().unwrap();
                assert_eq!(vec.len(), 2);
                assert_eq!(vec[0], format!("2: err=errs::Err {{ reason = errs::notify::tokio_handler::tests_of_notify::Errors FailToDoSomething, file = src/notify/tokio_handler.rs, line = {} }}", LINE + 48));
                assert_eq!(vec[1], format!("1: err=errs::Err {{ reason = errs::notify::tokio_handler::tests_of_notify::Errors FailToDoSomething, file = src/notify/tokio_handler.rs, line = {} }}", LINE + 48));
            }
            #[cfg(windows)]
            {
                let vec = LOGGER.lock().unwrap();
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

        static LOGGER: LazyLock<Mutex<Vec<String>>> = LazyLock::new(|| Mutex::new(Vec::new()));

        const LINE: u32 = line!();

        #[test]
        fn add_and_fix_and_notify() {
            assert!(add_tokio_async_handler(&HANDLERS, async |err, _tm| {
                tokio::time::sleep(tokio::time::Duration::from_millis(90)).await;
                LOGGER.lock().unwrap().push(format!("1: err={err:?}"));
            })
            .is_ok());
            // When rust version is less than 1.85.
            //assert!(
            //    add_tokio_async_handler(&HANDLERS, |err: Arc<Err>, _tm| Box::pin(async move {
            //        tokio::time::sleep(tokio::time::Duration::from_millis(90)).await;
            //        LOGGER.lock().unwrap().push(format!("1: err={err:?}"));
            //    }))
            //    .is_ok()
            //);

            assert!(add_tokio_async_handler(&HANDLERS, async |err, _tm| {
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                LOGGER.lock().unwrap().push(format!("2: err={err:?}"));
            })
            .is_ok());
            // When rust version is less than 1.85.
            //assert!(
            //    add_tokio_async_handler(&HANDLERS, |err: Arc<Err>, _tm| Box::pin(async move {
            //        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            //        LOGGER.lock().unwrap().push(format!("2: err={err:?}"));
            //    }))
            //    .is_ok()
            //);

            assert!(fix_handlers(&HANDLERS).is_ok());

            assert!(add_tokio_async_handler(&HANDLERS, async |err, _tm| {
                std::thread::sleep(std::time::Duration::from_millis(10));
                LOGGER.lock().unwrap().push(format!("1: err={err:?}"));
            })
            .is_err());
            // When rust version is less than 1.85.
            //assert!(
            //    add_tokio_async_handler(&HANDLERS, |err: Arc<Err>, _tm| Box::pin(async move {
            //        std::thread::sleep(std::::Duration::from_millis(10));
            //        LOGGER.lock().unwrap().push(format!("1: err={err:?}"));
            //    }))
            //    .is_err()
            //);

            let err = Err::new(Errors::FailToDoSomething);
            assert!(handle_err(&HANDLERS, err.into(), Utc::now()).is_ok());

            {
                let vec = LOGGER.lock().unwrap();
                assert_eq!(vec.len(), 0);
            }

            std::thread::sleep(std::time::Duration::from_millis(200));

            #[cfg(unix)]
            {
                let vec = LOGGER.lock().unwrap();
                assert_eq!(vec.len(), 2);
                assert_eq!(vec[0], format!("2: err=errs::Err {{ reason = errs::notify::tokio_handler::tests_of_notify::Errors FailToDoSomething, file = src/notify/tokio_handler.rs, line = {} }}", LINE + 48));
                assert_eq!(vec[1], format!("1: err=errs::Err {{ reason = errs::notify::tokio_handler::tests_of_notify::Errors FailToDoSomething, file = src/notify/tokio_handler.rs, line = {} }}", LINE + 48));
            }
            #[cfg(windows)]
            {
                let vec = LOGGER.lock().unwrap();
                assert_eq!(vec.len(), 2);
                assert_eq!(vec[0], format!("2: err=errs::Err {{ reason = errs::notify::tokio_handler::tests_of_notify::Errors FailToDoSomething, file = src\\notify\\tokio_handler.rs, line = {} }}", LINE + 48));
                assert_eq!(vec[1], format!("1: err=errs::Err {{ reason = errs::notify::tokio_handler::tests_of_notify::Errors FailToDoSomething, file = src\\notify\\tokio_handler.rs, line = {} }}", LINE + 48));
            }
        }
    }
}
