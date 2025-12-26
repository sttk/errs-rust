// Copyright (C) 2025 Takayuki Sato. All Rights Reserved.
// This program is free software under MIT License.
// See the file LICENSE in this distribution for more details.

//! This crate is for error handling in Rust programs, providing an `Err` struct which represents
//! an error with a reason.
//!
//! The type of this reason is any, but typically an enum variant is used.
//! The name of this enum variant indicates the reason for the error, and its fields store
//! contextual information about the situation in which the error occurred.
//! Since the path of an enum variant, including its package, is unique within a program, the enum
//! variant representing the reason is useful for identifying the specific error, locating where it
//! occurred, or generating appropriate error messages, etc.
//!
//! Optionally, by using `errs-notify` feature and registering error handlers in advance, it is
//! possible to receive notifications either synchronously or asynchronously at the time the error
//! struct is created.
//!
//! There is also an `errs-notify-tokio` feature, which is for applications that use the Tokio
//! runtime. If this feature is used, error notifications are received by asynchronous handlers
//! running on the Tokio runtime.
//!
//! ## Install
//!
//! In `Cargo.toml`, write this crate as a dependency:
//!
//! ```toml
//! [dependencies]
//! errs = "0.6.0"
//! ```
//!
//! If you want to use error notification, specify `errs-notify` or `errs-notify-tokio` in the
//! dependency features. The `errs-notify` feature is for general use, while the
//! `errs-notify-tokio` feature is for use with the Tokio runtime.
//!
//! ```toml
//! [dependencies]
//! errs = { version = "0.6.0", features = ["errs-notify"] }
//! ```
//!
//! If you are using Tokio, you should specify `errs-notify-tokio`:
//!
//! ```toml
//! [dependencies]
//! errs = { version = "0.6.0", features = ["errs-notify-tokio"] }
//! ```
//!
//! ## Usage
//!
//! ### Err instantiation and identification of a reason
//!
//! The `Err` struct can be instantiated with `new<R>(reason: R)` function or
//! `with_source<R, E>(reason: R, source: E)` function.
//!
//! Then, the reason can be identified with `reason<R>(&self)` method and a `match` statement,
//! or `match_reason<R>(&self, func fn(&R))` method.
//!
//! The following code is an example which uses `new<R>(reason: R)` function for instantiation,
//! and `reason<R>(&self)` method and a `match` statement for identifying a reason:
//!
//! ```
//! use errs::Err;
//!
//! #[derive(Debug)]
//! enum Reasons {
//!     IllegalState { state: String },
//!     // ...
//! }
//!
//! let err = Err::new(Reasons::IllegalState { state: "bad state".to_string() });
//!
//! match err.reason::<Reasons>() {
//!     Ok(r) => match r {
//!         Reasons::IllegalState { state } => println!("state = {state}"),
//!         _ => { /* ... */ }
//!     }
//!     Err(err) => match err.reason::<String>() {
//!         Ok(s) => println!("string reason = {s}"),
//!         Err(err) => { /* ... */ }
//!     }
//! }
//! ```
//!
//! ### Macro-based Registration of Err Handlers
//!
//! In addition to function-based handler registration, this crate provides macros for
//! registering error handlers from a static context (e.g., outside a function body).
//! These macros utilize the `inventory` crate to collect handlers at compile time,
//! making them available for the error notification system.
//!
//! Registered handlers are activated when the `fix_err_handlers` function is called
//! or implicitly upon the first `Err` instance creation.
//!
//! #### `add_sync_err_handler!`
//! Statically registers a synchronous error handler.
//!
//! ```rust
//! #[cfg(feature = "errs-notify")]
//! use errs::{add_sync_err_handler, Err};
//! #[cfg(feature = "errs-notify")]
//! use chrono::{DateTime, Utc};
//!
//! #[cfg(feature = "errs-notify")]
//! fn my_static_sync_handler(err: &Err, tm: DateTime<Utc>) {
//!     println!("[Static Sync] Error at {}: {}", tm, err);
//! }
//!
//! #[cfg(feature = "errs-notify")]
//! add_sync_err_handler!(my_static_sync_handler);
//! ```
//!
//! #### `add_async_err_handler!`
//! Statically registers a general-purpose asynchronous error handler.
//!
//! ```rust
//! #[cfg(feature = "errs-notify")]
//! use errs::{add_async_err_handler, Err};
//! #[cfg(feature = "errs-notify")]
//! use chrono::{DateTime, Utc};
//!
//! #[cfg(feature = "errs-notify")]
//! fn my_static_async_handler(err: &Err, tm: DateTime<Utc>) {
//!     println!("[Static Async] Error at {}: {}", tm, err);
//! }
//!
//! #[cfg(feature = "errs-notify")]
//! add_async_err_handler!(my_static_async_handler);
//! ```
//!
//! #### `add_tokio_async_err_handler!`
//! Statically registers a Tokio-based asynchronous error handler.
//!
//! ```rust
//! #[cfg(feature = "errs-notify-tokio")]
//! use errs::{add_tokio_async_err_handler, Err};
//! #[cfg(feature = "errs-notify-tokio")]
//! use chrono::{DateTime, Utc};
//! use std::sync::Arc;
//!
//! #[cfg(feature = "errs-notify-tokio")]
//! add_tokio_async_err_handler!(async |err: Arc<Err>, tm: DateTime<Utc>| {
//!     tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
//!     println!("[Static Tokio Async] Error at {}: {}", tm, err);
//! });
//!
//! // You can also register a function pointer:
//! // #[cfg(feature = "errs-notify-tokio")]
//! // fn my_static_tokio_handler(err: Arc<Err>, tm: DateTime<Utc>) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> {
//! //     Box::pin(async move {
//! //         println!("[Static Tokio Async Fn] Error at {}: {}", tm, err);
//! //     })
//! // }
//! // #[cfg(feature = "errs-notify-tokio")]
//! // add_tokio_async_err_handler!(my_static_tokio_handler);
//! ```
//!
//! After registering handlers using either functions or macros, ensure you call
//! `fix_err_handlers()` or allow the first error instantiation to implicitly
//! fix the handlers.
//!
//! ### Notification of Err instantiations
//!
//! This crate optionally provides a feature to notify pre-registered error handlers when an `Err`
//! is instantiated.
//! Multiple error handlers can be registered, and you can choose to receive notifications either
//! synchronously or asynchronously.
//! To register error handlers that receive notifications synchronously, use the
//! `add_sync_err_handler` function.
//!
//! For asynchronous notifications, there are two approaches: one for general use and another
//! specifically for applications using the Tokio runtime.
//!
//! For general-purpose asynchronous notifications, use the `add_async_err_handler` function.
//! This function is available when the `errs-notify` feature is enabled.
//!
//! For applications using the Tokio runtime, the `add_tokio_async_err_handler` function should
//! be used. This function is available when the `errs-notify-tokio` feature is enabled and
//! ensures that the asynchronous error handling is integrated with the Tokio runtime.
//!
//! Error notifications will not occur until the `fix_err_handlers` function is called.
//! This function locks the current set of error handlers, preventing further additions and
//! enabling notification processing.
//!
//! ```rust
//! #[cfg(feature = "errs-notify")]
//! errs::add_sync_err_handler(|err, tm| {
//!     println!("{}:{}:{} - {}", tm, err.file(), err.line(), err);
//! });
//!
//! #[cfg(feature = "errs-notify")]
//! errs::add_async_err_handler(|err, tm| {
//!     println!("{}:{}:{} - {}", tm, err.file(), err.line(), err);
//! });
//!
//! #[cfg(feature = "errs-notify-tokio")]
//! errs::add_tokio_async_err_handler(async |err, tm| {
//!     println!("{}:{}:{} - {}", tm, err.file(), err.line(), err);
//! });
//! // When rust version is less than 1.85.0.
//! //errs::add_tokio_async_err_handler(|err, tm| Box::pin(async move {
//! //    println!("{}:{}:{} - {}", tm, err.file(), err.line(), err);
//! //}));
//!
//! #[cfg(any(feature = "errs-notify", feature = "errs-notify-tokio"))]
//! errs::fix_err_handlers();
//! ```

#![cfg_attr(docsrs, feature(doc_cfg))]

mod err;

#[cfg(any(feature = "errs-notify", feature = "errs-notify-tokio"))]
#[cfg_attr(
    docsrs,
    doc(cfg(any(feature = "errs-notify", feature = "errs-notify-tokio")))
)]
mod notify;

#[cfg(feature = "errs-notify")]
#[cfg_attr(docsrs, doc(cfg(feature = "errs-notify")))]
pub use notify::{
    add_async_err_handler, add_sync_err_handler, AsyncHandlerRegistration, SyncHandlerRegistration,
};

#[cfg(feature = "errs-notify-tokio")]
#[cfg_attr(docsrs, doc(cfg(feature = "errs-notify-tokio")))]
pub use notify::{add_tokio_async_err_handler, TokioAsyncHandlerRegistration};

#[cfg(any(feature = "errs-notify", feature = "errs-notify-tokio"))]
#[cfg_attr(
    docsrs,
    doc(cfg(any(feature = "errs-notify", feature = "errs-notify-tokio")))
)]
pub use notify::{fix_err_handlers, ErrHandlingError, ErrHandlingErrorKind};

use std::{any, cell, error, fmt, marker, ptr, result};

#[cfg(any(feature = "errs-notify", feature = "errs-notify-tokio"))]
#[cfg_attr(
    docsrs,
    doc(cfg(any(feature = "errs-notify", feature = "errs-notify-tokio")))
)]
use std::sync::atomic;

/// Struct that represents an error with a reason.
///
/// This struct encapsulates the reason for the error, which can be any data type.
/// Typically, the reason is an enum variant, which makes it easy to uniquely identify
/// the error kind and location in the source code.
/// In addition, since an enum variant can store additional information as their fields,
/// it is possible to provide more detailed information about the error.
///
/// The reason for the error can be distinguished with match statements, and type
/// casting, so it is easy to handle the error in a type-safe manner.
///
/// This struct also contains an optional cause error, which is the error caused the
/// current error. This is useful for chaining errors.
///
/// This struct is implements the `std::errors::Error` trait, so it can be used as an
/// common error type in Rust programs.
pub struct Err {
    file: &'static str,
    line: u32,
    reason_and_source: SendSyncNonNull<ReasonAndSource>,
}

#[derive(Debug)]
struct DummyReason {}

#[derive(Debug)]
struct DummyError {}
impl fmt::Display for DummyError {
    fn fmt(&self, _f: &mut fmt::Formatter<'_>) -> result::Result<(), fmt::Error> {
        Ok(())
    }
}
impl error::Error for DummyError {}

#[repr(C)]
struct ReasonAndSource<R = DummyReason, E = DummyError>
where
    R: fmt::Debug + Send + Sync + 'static,
    E: error::Error + Send + Sync + 'static,
{
    is_fn: fn(any::TypeId) -> bool,
    drop_fn: fn(ptr::NonNull<ReasonAndSource>),
    debug_fn: fn(ptr::NonNull<ReasonAndSource>, f: &mut fmt::Formatter<'_>) -> fmt::Result,
    display_fn: fn(ptr::NonNull<ReasonAndSource>, f: &mut fmt::Formatter<'_>) -> fmt::Result,
    source_fn: fn(ptr::NonNull<ReasonAndSource>) -> Option<&'static (dyn error::Error + 'static)>,
    #[cfg(any(feature = "errs-notify", feature = "errs-notify-tokio"))]
    is_referenced_by_another: atomic::AtomicBool,
    reason_and_source: (R, Option<E>),
}

// When a struct contains a raw pointer as a field, the compiler cannot guarantee the safety of
// the data the pointer points to. Therefore, the Send and Sync traits are not implemented
// automatically, which means the struct cannot be safely moved or shared across threads.
//
// However, if it is verified that the data pointed to is Send and Sync, they can use an
// unsafe block to manually implement these traits.
//
// This SendSyncNonNull struct solves this issue by using a generic parameter T with a
// Send + Sync trait bound. This ensures at compile time that the internal pointer will always
// point to data that is thread-safe. As a result, it is safe to implement Send and Sync using
// unsafe on this struct itself. By including SendSyncNonNull as a field in another struct,
// that outer struct can also be made thread-safe.
struct SendSyncNonNull<T: Send + Sync> {
    non_null_ptr: ptr::NonNull<T>,

    // NonNull<T> is covariant over T, meaning it can be unsound if T with a shorter lifetime is
    // cast to a longer one. To solve this, a PhantomData<Cell<T>> field is added to make the type
    // invariant over T. This prevents the problematic casting. PhantomData is a zero-sized and
    // zero-cost type that is only used by the compiler.
    //
    // While this specific issue won't occur in the current implementation — because
    // SendSyncNonNull is only used inside Err with a concrete type ReasonAndSource<R> that has
    // a 'static lifetime constraint—the SendSyncNonNull type itself still has the potential for
    // this kind of unsoundness.
    //
    // Therefore, for good measure, this PhantomData<Cell<T>> field is added.
    _phantom: marker::PhantomData<cell::Cell<T>>,
}

/// A specialized [`Result`](https://doc.rust-lang.org/std/result/enum.Result.html) type
/// for `errs` crate, where the error type is [`Err`].
///
/// This type is broadly used across the `errs` crate for any operation that may produce an error.
/// Its primary purpose is to avoid repeatedly writing out [`Err`] directly, making code
/// more concise.
///
/// # Examples
///
/// A convenience function that bubbles an `errs::Result` to its caller:
///
/// ```
/// use errs::{Err, Result};
///
/// #[derive(Debug)]
/// enum Reasons {
///     NotFound,
///     PermissionDenied,
/// }
///
/// fn might_fail(s: &str) -> Result<String> {
///     if s == "fail" {
///         Err(Err::new(Reasons::NotFound))
///     } else {
///         Ok(s.to_string())
///     }
/// }
///
/// fn call_might_fail() -> Result<String> {
///     let s = might_fail("test")?;
///     Ok(s)
/// }
/// ```
pub type Result<T> = result::Result<T, Err>;
