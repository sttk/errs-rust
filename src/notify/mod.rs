// Copyright (C) 2025 Takayuki Sato. All Rights Reserved.
// This program is free software under MIT License.
// See the file LICENSE in this distribution for more details.

mod errors;

#[cfg(feature = "errs-notify")]
mod std_handler;

#[cfg(feature = "errs-notify")]
pub use std_handler::{AsyncHandlerRegistration, SyncHandlerRegistration};

#[cfg(feature = "errs-notify-tokio")]
mod tokio_handler;

#[cfg(feature = "errs-notify-tokio")]
pub use tokio_handler::TokioAsyncHandlerRegistration;

use crate::Err;
use chrono::{DateTime, Utc};

use std::sync;

#[cfg(feature = "errs-notify-tokio")]
use std::future::Future;

/// Represents the specific kind of error that can occur within the error handling
/// notification system.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum ErrHandlingErrorKind {
    StdMutexIsPoisoned,
    InvalidInternalState,
    InvalidCallTiming,
}

/// Represents an error that occurred during the error handling notification process.
///
/// This struct wraps an [`ErrHandlingErrorKind`] to provide more context about the
/// nature of the error in the notification system.
#[derive(Debug)]
pub struct ErrHandlingError {
    kind: ErrHandlingErrorKind,
}

/// Registers an asynchronous error handler.
///
/// This handler will be executed in a separate thread when an `Err` instance is created.
/// Handlers can only be registered before [`fix_err_handlers`] is called, or before the
/// first `Err` instance is created.
///
/// # Parameters
/// - `handler`: A closure that takes a reference to an `Err` instance and a `DateTime<Utc>`
///   timestamp, and performs error handling logic. It must be `Send + Sync + 'static`.
///
/// # Returns
/// - `Ok(())` if the handler was successfully registered.
/// - `Err(ErrHandlingError)` if an error occurred during registration (e.g., trying to register
///   after the handlers have been fixed).
#[cfg(feature = "errs-notify")]
#[cfg_attr(docsrs, doc(cfg(feature = "errs-notify")))]
pub fn add_async_err_handler<F>(handler: F) -> Result<(), ErrHandlingError>
where
    F: Fn(&Err, DateTime<Utc>) + Send + Sync + 'static,
{
    std_handler::add_async_handler(&std_handler::HANDLERS, handler)
}

/// Registers a synchronous error handler.
///
/// This handler will be executed in the current thread when an `Err` instance is created.
/// Handlers can only be registered before [`fix_err_handlers`] is called, or before the
/// first `Err` instance is created.
///
/// # Parameters
/// - `handler`: A closure that takes a reference to an `Err` instance and a `DateTime<Utc>`
///   timestamp, and performs error handling logic. It must be `Send + Sync + 'static`.
///
/// # Returns
/// - `Ok(())` if the handler was successfully registered.
/// - `Err(ErrHandlingError)` if an error occurred during registration (e.g., trying to register
///   after the handlers have been fixed).
#[cfg(feature = "errs-notify")]
#[cfg_attr(docsrs, doc(cfg(feature = "errs-notify")))]
pub fn add_sync_err_handler<F>(handler: F) -> Result<(), ErrHandlingError>
where
    F: Fn(&Err, DateTime<Utc>) + Send + Sync + 'static,
{
    std_handler::add_sync_handler(&std_handler::HANDLERS, handler)
}

/// Registers a Tokio-based asynchronous error handler.
///
/// This handler will be executed as an asynchronous task on a Tokio runtime when an `Err`
/// instance is created. If the notification occurs outside a Tokio runtime, a new runtime
/// will be spawned in a separate thread to run the handler.
///
/// Handlers can only be registered before [`fix_err_handlers`] is called, or before the
/// first `Err` instance is created.
///
/// # Parameters
/// - `handler`: An `async` closure that takes an `Arc<Err>` and a `DateTime<Utc>`
///   timestamp, and returns a `Future`. The `Arc<Err>` is used to allow the `Err`
///   instance to be shared across multiple asynchronous handlers. The closure must
///   be `Send + Sync + 'static`.
///
/// # Returns
/// - `Ok(())` if the handler was successfully registered.
/// - `Err(ErrHandlingError)` if an error occurred during registration.
#[cfg(feature = "errs-notify-tokio")]
#[cfg_attr(docsrs, doc(cfg(feature = "errs-notify-tokio")))]
pub fn add_tokio_async_err_handler<F, Fut>(handler: F) -> Result<(), ErrHandlingError>
where
    F: Fn(sync::Arc<Err>, DateTime<Utc>) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    tokio_handler::add_tokio_async_handler(&tokio_handler::HANDLERS, handler)
}

/// Fixes the set of registered error handlers, preventing any further additions.
///
/// Once this function is called, attempts to register new handlers using
/// [`add_sync_err_handler`], [`add_async_err_handler`], or [`add_tokio_async_err_handler`]
/// will fail.
/// If `Err` instances are created before this function is explicitly called, the handlers
/// will be implicitly fixed upon the first `Err` notification.
///
/// # Returns
/// - `Ok(())` if the handlers were successfully fixed or were already fixed.
/// - `Err(ErrHandlingError)` if an error occurred during the fixing process.
pub fn fix_err_handlers() -> Result<(), ErrHandlingError> {
    #[cfg(feature = "errs-notify")]
    let result_std = std_handler::fix_handlers(&std_handler::HANDLERS);

    #[cfg(feature = "errs-notify-tokio")]
    let result_tokio = tokio_handler::fix_handlers(&tokio_handler::HANDLERS);

    #[cfg(feature = "errs-notify")]
    result_std?;
    #[cfg(feature = "errs-notify-tokio")]
    result_tokio?;

    Ok(())
}

pub(crate) fn notify_err(err: Err) -> Result<(), ErrHandlingError> {
    let tm = Utc::now();
    let err = sync::Arc::new(err);

    #[cfg(feature = "errs-notify")]
    let result_std = std_handler::handle_err(&std_handler::HANDLERS, sync::Arc::clone(&err), tm);

    #[cfg(feature = "errs-notify-tokio")]
    let result_tokio =
        tokio_handler::handle_err(&tokio_handler::HANDLERS, sync::Arc::clone(&err), tm);

    #[cfg(feature = "errs-notify")]
    result_std?;
    #[cfg(feature = "errs-notify-tokio")]
    result_tokio?;

    Ok(())
}
