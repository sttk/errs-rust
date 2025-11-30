// Copyright (C) 2025 Takayuki Sato. All Rights Reserved.
// This program is free software under MIT License.
// See the file LICENSE in this distribution for more details.

mod blocking;
mod errors;

use crate::Err;

use blocking::{add_async_handler, add_sync_handler, fix_handlers, handle_err, HANDLERS};

use chrono::{DateTime, Utc};

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
pub fn add_async_err_handler<F>(handler: F) -> Result<(), ErrHandlingError>
where
    F: Fn(&Err, DateTime<Utc>) + Send + Sync + 'static,
{
    add_async_handler(&HANDLERS, handler)
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
pub fn add_sync_err_handler<F>(handler: F) -> Result<(), ErrHandlingError>
where
    F: Fn(&Err, DateTime<Utc>) + Send + Sync + 'static,
{
    add_sync_handler(&HANDLERS, handler)
}

/// Fixes the set of registered error handlers, preventing any further additions.
///
/// Once this function is called, attempts to register new handlers using
/// [`add_async_err_handler`] or [`add_sync_err_handler`] will fail.
/// If `Err` instances are created before this function is explicitly called, the handlers
/// will be implicitly fixed upon the first `Err` notification.
///
/// # Returns
/// - `Ok(())` if the handlers were successfully fixed or were already fixed.
/// - `Err(ErrHandlingError)` if an error occurred during the fixing process.
pub fn fix_err_handlers() -> Result<(), ErrHandlingError> {
    fix_handlers(&HANDLERS)
}

pub(crate) fn notify_err(err: Err) -> Result<(), ErrHandlingError> {
    let tm = Utc::now();
    handle_err(&HANDLERS, err, tm)
}
