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
//! ## Install
//!
//! In `Cargo.toml`, write this crate as a dependency:
//!
//! ```toml
//! [dependencies]
//! errs = "0.3.2"
//! ```
//!
//! If you want to use error notification, specifies `errs-notify` or `full` in the dependency
//! features:
//!
//! ```toml
//! [dependencies]
//! errs = { version = "0.3.2", features = ["errs-notify"] }
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
//! ### Notification of Err instantiations
//!
//! This crate optionally provides a feature to notify pre-registered error handlers when an `Err`
//! is instantiated.
//! Multiple error handlers can be registered, and you can choose to receive notifications either
//! synchronously or asynchronously.
//! To register error handlers that receive notifications synchronously, use the
//! `add_sync_err_handler` function.
//! For asynchronous notifications, use the `add_async_err_handler!` macro.
//!
//! Error notifications will not occur until the `fix_err_handlers` function is called.
//! This function locks the current set of error handlers, preventing further additions and
//! enabling notification processing.
//!
//! ```rust
//! errs::add_async_err_handler!(async |err, tm| {
//!     println!("{}:{}:{} - {}", tm, err.file(), err.line(), err);
//! });
//!
//! errs::add_sync_err_handler(|err, tm| {
//!     println!("{}:{}:{} - {}", tm, err.file(), err.line(), err);
//! });
//!
//! errs::fix_err_handlers();
//! ```

#![cfg_attr(docsrs, feature(doc_cfg))]

mod err;

#[cfg(feature = "errs-notify")]
#[cfg_attr(docsrs, doc(cfg(feature = "errs-notify")))]
mod notify;

#[cfg(feature = "errs-notify")]
#[cfg_attr(docsrs, doc(cfg(feature = "errs-notify")))]
pub use notify::{add_raw_async_err_handler, add_sync_err_handler, fix_err_handlers};

use std::any;
use std::cell::Cell;
use std::error;
use std::fmt;
use std::marker::PhantomData;
use std::ptr;
use std::sync::atomic;

/// Is the struct that represents an error with a reason.
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
    reason_container: SendSyncNonNull<ReasonContainer>,
    source: Option<Box<dyn error::Error + Send + Sync>>,
}

#[derive(Debug)]
struct DummyReason {}

#[repr(C)]
struct ReasonContainer<R = DummyReason>
where
    R: fmt::Debug + Send + Sync + 'static,
{
    is_fn: fn(any::TypeId) -> bool,
    drop_fn: fn(ptr::NonNull<ReasonContainer>),
    debug_fn: fn(ptr::NonNull<ReasonContainer>, f: &mut fmt::Formatter<'_>) -> fmt::Result,
    display_fn: fn(ptr::NonNull<ReasonContainer>, f: &mut fmt::Formatter<'_>) -> fmt::Result,
    reason: R,
    is_referenced_by_another: Option<atomic::AtomicBool>,
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
    // SendSyncNonNull is only used inside Err with a concrete type ReasonContainer<R> that has
    // a 'static lifetime constraint—the SendSyncNonNull type itself still has the potential for
    // this kind of unsoundness.
    //
    // Therefore, for good measure, this PhantomData<Cell<T>> field is added.
    _phantom: PhantomData<Cell<T>>,
}
