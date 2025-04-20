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
//! Optionally, by using `notify` feature and registering error handlers in advance, it is possible
//! to receive notifications either synchronously or asynchronously at the time the error struct is
//! created.
//!
//! ## Install
//!
//! In `Cargo.toml`, write this crate as a dependency:
//!
//! ```toml
//! [dependencies]
//! errs = "0.1.0"
//! ```
//!
//! If you want to use error notification, specifies `notify` or `full` in the dependency features:
//!
//! ```toml
//! [dependencies]
//! errs = { version = "0.1.0", features = ["notify"] }
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
//! For asynchronous notifications, use the `add_async_err_handler` function.
//!
//! Error notifications will not occur until the `fix_err_handlers` function is called.
//! This function locks the current set of error handlers, preventing further additions and
//! enabling notification processing.
//!
//! ```
//! errs::add_async_err_handler(|info, tm| {
//!     println!("{}:{}:{} - {}", tm, info.file, info.line, info.reason_type);
//! });
//!
//! errs::add_sync_err_handler(|info, tm| {
//!     // ...
//! });
//!
//! errs::fix_err_handlers();
//! ```

#![cfg_attr(docsrs, feature(doc_cfg))]

mod err;

#[cfg(feature = "notify")]
#[cfg_attr(docsrs, doc(cfg(feature = "notify")))]
pub use err::{ErrInfo, add_async_err_handler, add_sync_err_handler, fix_err_handlers};

use std::any;
use std::error;
use std::fmt;
use std::ptr;

/// Is the struct that represents an error with a reason.
///
/// This struct encapsulates the reason for the error, which can be any data type.
/// Typically, the reason is an enum variant, which makes it easy to uniquely identify
/// the error kind and location in the source code.
/// In addition, since an enum variant can store additional informations as their fields,
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
    /// The name of the source file where the error occurred.
    pub file: &'static str,

    /// The line number in the source file where the error occurred.
    pub line: u32,

    reason_container: ptr::NonNull<ReasonContainer>,
    source: Option<Box<dyn error::Error>>,
}

#[derive(Debug)]
struct DummyReason {}

#[repr(C)]
struct ReasonContainer<R = DummyReason>
where
    R: fmt::Debug + Send + Sync + 'static,
{
    is_fn: fn(any::TypeId) -> bool,
    drop_fn: fn(*const ReasonContainer),
    debug_fn: fn(*const ReasonContainer, f: &mut fmt::Formatter<'_>) -> fmt::Result,
    display_fn: fn(*const ReasonContainer, f: &mut fmt::Formatter<'_>) -> fmt::Result,
    reason: R,
}
