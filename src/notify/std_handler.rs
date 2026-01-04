// Copyright (C) 2025-2026 Takayuki Sato. All Rights Reserved.
// This program is free software under MIT License.
// See the file LICENSE in this distribution for more details.

use super::{ErrHandlingError, ErrHandlingErrorKind};
use crate::Err;

use chrono::{DateTime, Utc};
use setup_read_cleanup::{graceful::GracefulPhasedCellSync, PhasedErrorKind};

use std::{sync::Arc, thread};

type SyncBoxedFn = Box<dyn Fn(&Err, DateTime<Utc>) + Send + Sync + 'static>;
type AsyncArcFn = Arc<dyn Fn(&Err, DateTime<Utc>) + Send + Sync + 'static>;

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
    handlers: &'static GracefulPhasedCellSync<(Vec<SyncBoxedFn>, Vec<AsyncArcFn>)>,
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
        Ok(vv) => {
            let err_clone = Arc::clone(&err);
            #[cfg(not(feature = "notify-tokio"))]
            {
                thread::spawn(move || {
                    for handle in vv.1.iter() {
                        let e = Arc::clone(&err_clone);
                        let h = Arc::clone(handle);
                        thread::spawn(move || h(&e, tm));
                    }
                });
            }
            #[cfg(feature = "notify-tokio")]
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

#[doc(hidden)]
pub struct SyncHandlerRegistration {
    handler: fn(&Err, DateTime<Utc>),
}
impl SyncHandlerRegistration {
    pub const fn new(handler: fn(&Err, DateTime<Utc>)) -> Self {
        Self { handler }
    }
}
inventory::collect!(SyncHandlerRegistration);

#[doc(hidden)]
pub struct AsyncHandlerRegistration {
    handler: fn(&Err, DateTime<Utc>),
}
impl AsyncHandlerRegistration {
    pub const fn new(handler: fn(&Err, DateTime<Utc>)) -> Self {
        Self { handler }
    }
}
inventory::collect!(AsyncHandlerRegistration);

/// Statically registers a synchronous error handler.
///
/// This macro provides a way to register an error handler from a static context, such as
/// outside a function body. It uses the `inventory` crate to collect handlers at compile
/// time, which are then added when `fix_err_handlers` is called or the first error
/// notification occurs.
///
/// This is the macro-based alternative to the [`add_sync_err_handler`](crate::add_sync_err_handler()) function.
///
/// # Note
/// The handler function must be a function pointer `fn(&Err, DateTime<Utc>)`.
/// Closures are not supported in this macro.
///
/// # Example
/// ```rust
/// use errs::{add_sync_err_handler, Err};
/// use chrono::{DateTime, Utc};
///
/// fn my_sync_handler(err: &Err, tm: DateTime<Utc>) {
///     // In a real scenario, you might log the error to a file or service.
///     println!("[Sync Handler] Error occurred at {}: {}", tm, err);
/// }
///
/// // Register the handler statically.
/// add_sync_err_handler!(my_sync_handler);
///
/// // In your application's initialization:
/// // errs::fix_err_handlers();
/// ```
#[macro_export]
macro_rules! add_sync_err_handler {
    ($handler:expr) => {
        inventory::submit! {
          $crate::SyncHandlerRegistration::new($handler)
        }
    };
}

/// Statically registers an asynchronous error handler.
///
/// This macro provides a way to register an asynchronous error handler from a static context.
/// It leverages the `inventory` crate to collect handlers at compile time. These handlers
/// are executed in a separate thread when an `Err` instance is created.
///
/// This is the macro-based alternative to the [`add_async_err_handler`](crate::add_async_err_handler()) function.
///
/// # Note
/// The handler function must be a function pointer `fn(&Err, DateTime<Utc>)`.
/// Closures are not supported in this macro.
///
/// # Example
/// ```rust
/// use errs::{add_async_err_handler, Err};
/// use chrono::{DateTime, Utc};
///
/// fn my_async_handler(err: &Err, tm: DateTime<Utc>) {
///     // This will run in a separate thread.
///     println!("[Async Handler] Error occurred at {}: {}", tm, err);
/// }
///
/// // Register the handler statically.
/// add_async_err_handler!(my_async_handler);
///
/// // In your application's initialization:
/// // errs::fix_err_handlers();
/// ```
#[macro_export]
macro_rules! add_async_err_handler {
    ($handler:expr) => {
        inventory::submit! {
          $crate::AsyncHandlerRegistration::new($handler)
        }
    };
}

fn register_handlers_by_inventory(
    vv: &mut (Vec<SyncBoxedFn>, Vec<AsyncArcFn>),
) -> Result<(), ErrHandlingError> {
    let vec: Vec<SyncBoxedFn> = inventory::iter::<SyncHandlerRegistration>
        .into_iter()
        .map(|reg| Box::new(reg.handler) as SyncBoxedFn)
        .collect();
    vv.0.splice(0..0, vec);

    let vec: Vec<AsyncArcFn> = inventory::iter::<AsyncHandlerRegistration>
        .into_iter()
        .map(|reg| Arc::new(reg.handler) as AsyncArcFn)
        .collect();
    vv.1.splice(0..0, vec);

    Ok(())
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

        static LOGGER: LazyLock<Mutex<Vec<String>>> = LazyLock::new(|| Mutex::new(Vec::new()));

        const LINE: u32 = line!();

        #[test]
        fn add_and_fix_and_notify() {
            assert!(add_sync_handler(&HANDLERS, |err, _tm| {
                LOGGER.lock().unwrap().push(format!("1: err={err:?}"));
            })
            .is_ok());
            assert!(add_sync_handler(&HANDLERS, |err, _tm| {
                LOGGER.lock().unwrap().push(format!("2: err={err:?}"));
            })
            .is_ok());

            assert!(fix_handlers(&HANDLERS).is_ok());

            assert!(add_sync_handler(&HANDLERS, |err, _tm| {
                LOGGER.lock().unwrap().push(format!("3: err={err:?}"));
            })
            .is_err());

            let err = Err::new(Errors::FailToDoSomething);
            assert!(handle_err(&HANDLERS, err.into(), Utc::now()).is_ok());

            #[cfg(unix)]
            {
                let vec = LOGGER.lock().unwrap();
                assert_eq!(vec.len(), 2);
                assert_eq!(vec[0], format!("1: err=errs::Err {{ reason = errs::notify::std_handler::tests_of_notify::Errors FailToDoSomething, file = src/notify/std_handler.rs, line = {} }}", LINE + 20));
                assert_eq!(vec[1], format!("2: err=errs::Err {{ reason = errs::notify::std_handler::tests_of_notify::Errors FailToDoSomething, file = src/notify/std_handler.rs, line = {} }}", LINE + 20));
            }
            #[cfg(windows)]
            {
                let vec = LOGGER.lock().unwrap();
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

        static LOGGER: LazyLock<Mutex<Vec<String>>> = LazyLock::new(|| Mutex::new(Vec::new()));

        const LINE: u32 = line!();

        #[test]
        fn add_and_fix_and_notify() {
            assert!(add_async_handler(&HANDLERS, |err, _tm| {
                thread::sleep(std::time::Duration::from_millis(50));
                LOGGER.lock().unwrap().push(format!("1: err={err:?}"));
            })
            .is_ok());
            assert!(add_async_handler(&HANDLERS, |err, _tm| {
                thread::sleep(std::time::Duration::from_millis(10));
                LOGGER.lock().unwrap().push(format!("2: err={err:?}"));
            })
            .is_ok());

            assert!(fix_handlers(&HANDLERS).is_ok());

            assert!(add_async_handler(&HANDLERS, |err, _tm| {
                thread::sleep(std::time::Duration::from_millis(10));
                LOGGER.lock().unwrap().push(format!("3: err={err:?}"));
            })
            .is_err());

            let err = Err::new(Errors::FailToDoSomething);
            assert!(handle_err(&HANDLERS, err.into(), Utc::now()).is_ok());

            {
                let vec = LOGGER.lock().unwrap();
                assert_eq!(vec.len(), 0);
            }

            thread::sleep(std::time::Duration::from_millis(200));

            #[cfg(unix)]
            {
                let vec = LOGGER.lock().unwrap();
                assert_eq!(vec.len(), 2);
                assert_eq!(vec[0], format!("2: err=errs::Err {{ reason = errs::notify::std_handler::tests_of_notify::Errors FailToDoSomething, file = src/notify/std_handler.rs, line = {} }}", LINE + 23));
                assert_eq!(vec[1], format!("1: err=errs::Err {{ reason = errs::notify::std_handler::tests_of_notify::Errors FailToDoSomething, file = src/notify/std_handler.rs, line = {} }}", LINE + 23));
            }
            #[cfg(windows)]
            {
                let vec = LOGGER.lock().unwrap();
                assert_eq!(vec.len(), 2);
                assert_eq!(vec[0], format!("2: err=errs::Err {{ reason = errs::notify::std_handler::tests_of_notify::Errors FailToDoSomething, file = src\\notify\\std_handler.rs, line = {} }}", LINE + 23));
                assert_eq!(vec[1], format!("1: err=errs::Err {{ reason = errs::notify::std_handler::tests_of_notify::Errors FailToDoSomething, file = src\\notify\\std_handler.rs, line = {} }}", LINE + 23));
            }
        }
    }

    #[cfg(feature = "notify-tokio")]
    mod tests_of_async_err_handling_on_tokio {
        use super::*;
        use std::sync::{LazyLock, Mutex};
        use tokio::time::Duration;

        static HANDLERS: GracefulPhasedCellSync<(Vec<SyncBoxedFn>, Vec<AsyncArcFn>)> =
            GracefulPhasedCellSync::new((Vec::new(), Vec::new()));

        static LOGGER: LazyLock<Mutex<Vec<String>>> = LazyLock::new(|| Mutex::new(Vec::new()));

        const LINE: u32 = line!();

        #[tokio::test]
        async fn add_and_fix_and_notify() {
            assert!(add_async_handler(&HANDLERS, |err, _tm| {
                thread::sleep(std::time::Duration::from_millis(50)); // intentionally block
                LOGGER.lock().unwrap().push(format!("tokio-1: err={err:?}"));
            })
            .is_ok());
            assert!(add_async_handler(&HANDLERS, |err, _tm| {
                thread::sleep(std::time::Duration::from_millis(10)); // intentionally block
                LOGGER.lock().unwrap().push(format!("tokio-2: err={err:?}"));
            })
            .is_ok());

            assert!(fix_handlers(&HANDLERS).is_ok());

            assert!(add_async_handler(&HANDLERS, |err, _tm| {
                thread::sleep(std::time::Duration::from_millis(10));
                LOGGER.lock().unwrap().push(format!("3: err={err:?}"));
            })
            .is_err());

            let err = Err::new(Errors::FailToDoSomething);
            assert!(handle_err(&HANDLERS, Arc::new(err), Utc::now()).is_ok());

            {
                let vec = LOGGER.lock().unwrap();
                assert_eq!(vec.len(), 0);
            }

            tokio::time::sleep(Duration::from_millis(200)).await;

            #[cfg(unix)]
            {
                let vec = LOGGER.lock().unwrap();
                assert_eq!(vec.len(), 2);
                assert_eq!(vec[0], format!("tokio-2: err=errs::Err {{ reason = errs::notify::std_handler::tests_of_notify::Errors FailToDoSomething, file = src/notify/std_handler.rs, line = {} }}", LINE + 23));
                assert_eq!(vec[1], format!("tokio-1: err=errs::Err {{ reason = errs::notify::std_handler::tests_of_notify::Errors FailToDoSomething, file = src/notify/std_handler.rs, line = {} }}", LINE + 23));
            }
            #[cfg(windows)]
            {
                let vec = LOGGER.lock().unwrap();
                assert_eq!(vec.len(), 2);
                assert_eq!(vec[0], format!("tokio-2: err=errs::Err {{ reason = errs::notify::std_handler::tests_of_notify::Errors FailToDoSomething, file = src\\notify\\std_handler.rs, line = {} }}", LINE + 23));
                assert_eq!(vec[1], format!("tokio-1: err=errs::Err {{ reason = errs::notify::std_handler::tests_of_notify::Errors FailToDoSomething, file = src\\notify\\std_handler.rs, line = {} }}", LINE + 23));
            }
        }
    }

    mod tests_of_no_handlers {
        use super::*;
        use std::sync::{LazyLock, Mutex};

        static HANDLERS: GracefulPhasedCellSync<(Vec<SyncBoxedFn>, Vec<AsyncArcFn>)> =
            GracefulPhasedCellSync::new((Vec::new(), Vec::new()));

        static LOGGER: LazyLock<Mutex<Vec<String>>> = LazyLock::new(|| Mutex::new(Vec::new()));

        #[test]
        fn no_handlers_registered_should_not_panic() {
            assert!(fix_handlers(&HANDLERS).is_ok());

            let err = Err::new(Errors::FailToDoSomething);
            let result = handle_err(&HANDLERS, Arc::new(err), Utc::now());

            assert!(result.is_ok());
            assert!(LOGGER.lock().unwrap().is_empty());
        }
    }
}
