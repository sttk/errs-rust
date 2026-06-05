# [errs][repo-url] [![crates.io][cratesio-img]][cratesio-url] [![doc.rs][docrs-img]][docrs-url] [![CI Status][ci-img]][ci-url] [![MIT License][mit-img]][mit-url]

A library for handling errors with reasons for Rust

## Overview

`errs` is an error handling library for Rust designed to focus on the "Reason" behind an error.

### Expressing "Why It Failed" via the Type System

Rather than treating errors as simple message strings or type-erased objects, it embraces a design that expresses "why it failed" through types, allowing for safe and clear propagation and determination.

For error reasons, you can use anything from lightweight types like `String` to type-safe definitions using `enum` or `struct`, all handled flexibly with the same API.
By using an `enum` in particular, you can not only express failure factors within the type system but also hold contextual information in its fields, propagating the situation and relevant data at the time of the error as-is.
Furthermore, since reasons can be determined in a type-safe manner using `reason::<T>()` or `match_reason`, you can avoid fragile error handling that relies on string comparisons.

### Decentralized Error Definition and  Traceability

`errs` encourages defining error reasons close to where they occur.
This eliminates the need to share a massive, monolithic error type across the entire application, enabling a highly maintainable design while keeping dependencies between modules clean.
Type information is utilized to identify the reason, and the type identifiers required for this determination are resolved statically at compile time.
This provides type-safe error handling with minimal runtime overhead.

The core `Err` type of the library implements `std::error::Error`, allowing it to integrate naturally with standard Rust error handling, including the `?` operator.
It can also retain lower-layer errors as causes (source errors), enabling you to manage the "Reason" of the upper layer and the "Cause" of the lower layer separately.
Additionally, it automatically records the file name and line number when an error is generated, making log output and failure analysis effortless.

### Powerful Error-Instantiation Notification & Monitoring Ecosystem

Furthermore, `errs` features a mechanism to notify error generation events.
By enabling the `notify` or `notify-tokio` feature, an automatic notification can be sent to registered handlers the exact moment an `Err` is created.
It supports synchronous handlers, generic asynchronous handlers, and asynchronous handlers tailored for Tokio.
It accommodates both dynamic registration within functions and static global registration via macros.
This makes it easy to implement logging, monitoring, metrics collection, and integration with telemetry systems.

While `anyhow`-like libraries place importance on "propagating errors flexibly," and `thiserror`-like libraries focus on "making error type definitions easy", `errs` emphasizes "expressing failure reasons through types and observing their occurrence".
This library is ideal when you want to clearly manage the semantics of errors occurring within an application while integrating seamlessly with monitoring and operations infrastructure.

## Install

In `Cargo.toml`, write this crate as a dependency:

```toml
[dependencies]
errs = "0.9.0"
```

If you want to use error notification, specify the `notify` or `notify-tokio` in the dependency features.
The `notify` feature is for general use, while the `notify-tokio` feature is for use with the Tokio runtime.

```toml
[dependencies]
errs = { version = "0.9.0", features = ["notify"] }
```

If you are using Tokio, you should specify `notify-tokio`:

```toml
[dependencies]
errs = { version = "0.9.0", features = ["notify-tokio"] }
```

## Usage

### Locally Defined Reasons and Instantiate an Err with Them

An `Err` struct can be instantiated with any arbitrary error reason.
Typically, a variant of an enum defined to indicate the cause or context of the error is used as the reason.
This variant does not need to belong to a single, centrally managed enum; rather, it is preferable to define it close to where the error using it as a reason actually occurs.

```rust
use errs::Err;

#[derive(Debug)]
enum Reasons {
    IllegalState { state: String },
    // ...
}

let err = Err::new(Reasons::IllegalState { state: "bad state".to_string() });
```

An `Err` can also be instantiated using `Err::with_source`, which accepts the underlying cause error along with the reason.

```rust
use std::io::{Error, ErrorKind};
use errs::Err;

let io_error = Error::new(ErrorKind::Interrupted, "oh no!");

let err = Err::with_source(Reasons::IllegalState { state: "bad state".to_string() }, io_error);
```

### Type-Safe Reason Identification

By using the `reason::<R>(&self)` method, you can extract the error reason as the specified type `R`.
Since the return value is `Result<&R, &Err>`, you can use a `match` statement to safely branch and identify the reason in a type-safe manner.

```rust
match err.reason::<Reasons>() {
    Ok(r) => match r {
        Reasons::IllegalState { state } => println!("state = {state}"),
        _ => { /* ... */ }
    }
    Err(err) => match err.reason::<String>() {
        Ok(s) => println!("string reason = {s}"),
        Err(err) => { /* ... */ }
    }
}
```

Methods like `match_reason` allow you to write error reason identification and fallback processing elegantly using method chaining, without having to nest multiple `match` statements.

```rust
let val: u8 = err.match_reason::<Reasons, u8>(|r| match r {
    Reasons::IllegalState { _state } => Ok(1u8),
    _ => Ok(2u8),
}).or_match_reason::<String>(|_string| {
    Ok(3u8)
}).or_result(|_err| {
    Ok(4u8)
})?;
```

### Function-based Error Handler Registration

> To enable this feature, you must specify the feature `notify` or `notify-tokio` in `Cargo.toml`.

This crate optionally provides a feature to notify pre-registered error handlers when an `Err`
is instantiated.
Multiple error handlers can be registered, and you can choose to receive notifications either
synchronously or asynchronously.

To register handlers inside a function (like `main`), you can use the following functions:
- `add_sync_err_handler`: For synchronous handlers.
- `add_async_err_handler`: For general-purpose asynchronous handlers.
- `add_tokio_async_err_handler`: For Tokio-based asynchronous handlers.

Error notifications will not occur until the `fix_err_handlers` function is called.
This function locks the current set of error handlers, preventing further additions and
enabling notification processing.

```rust
// In your main function or initialization code:

#[cfg(feature = "notify")]
errs::add_sync_err_handler(|err, tm| {
    println!("[Sync] {}:{}:{} - {}", tm, err.file(), err.line(), err);
});

#[cfg(feature = "notify")]
errs::add_async_err_handler(|err, tm| {
    println!("[Async] {}:{}:{} - {}", tm, err.file(), err.line(), err);
});

#[cfg(feature = "notify-tokio")]
errs::add_tokio_async_err_handler(async |err, tm| {
    println!("[Tokio Async] {}:{}:{} - {}", tm, err.file(), err.line(), err);
});

// Fix the handlers to start receiving notifications.
#[cfg(any(feature = "notify", feature = "notify-tokio"))]
errs::fix_err_handlers();
```

### Macro-based Error Handler Registration

> To enable this feature, you must specify the feature `notify` or `notify-tokio` in `Cargo.toml`.

Alternatively, you can register handlers from a static context (outside a function body)
using macros. These are useful for setting up global handlers that are compiled into your
program.

- `add_sync_err_handler!`: Statically registers a synchronous handler.
- `add_async_err_handler!`: Statically registers a general-purpose asynchronous handler.
- `add_tokio_async_err_handler!`: Statically registers a Tokio-based asynchronous handler.

These macros require function pointers, not closures.

```rust
#[cfg(feature = "notify")]
use errs::{add_async_err_handler, add_sync_err_handler};
#[cfg(feature = "notify-tokio")]
use errs::{add_tokio_async_err_handler};
use errs::Err;
use chrono::{DateTime, Utc};
use std::sync::Arc;

// Define a static synchronous handler
fn my_sync_handler(err: &Err, tm: DateTime<Utc>) {
    println!("[Static Sync] Error at {}: {}", tm, err);
}
#[cfg(feature = "notify")]
add_sync_err_handler!(my_sync_handler);

// Define a static asynchronous handler
fn my_async_handler(err: &Err, tm: DateTime<Utc>) {
    println!("[Static Async] Error at {}: {}", tm, err);
}
#[cfg(feature = "notify")]
add_async_err_handler!(my_async_handler);

// Define a static Tokio-based asynchronous handler
#[cfg(feature = "notify-tokio")]
add_tokio_async_err_handler!(async |err: Arc<Err>, tm: DateTime<Utc>| {
    println!("[Static Tokio Async] Error at {}: {}", tm, err);
});

// Later, in your main function, you still need to fix the handlers.
// errs::fix_err_handlers();
```

## Supported Rust versions

This crate supports Rust 1.80.1 or later.

```bash
% ./build.sh msrv
  [Meta]   cargo-msrv 0.18.4

Compatibility Check #1: Rust 1.76.0
  [FAIL]   Is incompatible

Compatibility Check #2: Rust 1.86.0
  [OK]     Is compatible

Compatibility Check #3: Rust 1.81.0
  [OK]     Is compatible

Compatibility Check #4: Rust 1.78.0
  [FAIL]   Is incompatible

Compatibility Check #5: Rust 1.79.0
  [FAIL]   Is incompatible

Compatibility Check #6: Rust 1.80.1
  [OK]     Is compatible

Result:
   Considered (min … max):   Rust 1.56.1 … Rust 1.96.0
   Search method:            bisect
   MSRV:                     1.80.1
   Target:                   x86_64-apple-darwin
```

## License

Copyright (C) 2025-2026 Takayuki Sato

This program is free software under MIT License.<br>
See the file LICENSE in this distribution for more details.


[repo-url]: https://github.com/sttk/errs-rust
[cratesio-img]: https://img.shields.io/badge/crates.io-ver.0.9.0-fc8d62?logo=rust
[cratesio-url]: https://crates.io/crates/errs
[docrs-img]: https://img.shields.io/badge/doc.rs-errs-66c2a5?logo=docs.rs
[docrs-url]: https://docs.rs/errs
[ci-img]: https://github.com/sttk/errs-rust/actions/workflows/rust.yml/badge.svg?branch=main
[ci-url]: https://github.com/sttk/errs-rust/actions?query=branch%3Amain
[mit-img]: https://img.shields.io/badge/license-MIT-green.svg
[mit-url]: https://opensource.org/licenses/MIT
