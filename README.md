# [errs][repo-url] [![crates.io][cratesio-img]][cratesio-url] [![doc.rs][docrs-img]][docrs-url] [![CI Status][ci-img]][ci-url] [![MIT License][mit-img]][mit-url]

## Introduction

This crate is an error handling library for Rust that focuses on handling the "reason" of an error.

Since arbitrary types can be used as error reasons, the same API can be used for everything from lightweight string-based usage to large-scale type-safe designs using enums and structs.

```rust
let err = errs::Err::new("invalid state");
```

```rust
enum FileErrors {
    FailToOpenFile,
    FailToReadLine,
}

let err = errs::Err::new(FileErrors::FailToOpenFile);
```

## Core Concepts

### Arbitrary types can be used as error reasons

Error reasons can be represented by arbitrary types such as strings, enums, and structs.

- Easy to start with simple string reasons for small utilities
- Type-safe handling with enums or structs for large applications
- No need to create a global error enum

This allows the library to be used consistently from small CLI utilities to applications with layered architectures.

### Error reasons can be defined locally

Enums or structs representing error reasons can be defined near the location where the error may occur.

```rust
pub mod auth_service {
    pub enum AuthServiceErrors {
        InvalidToken,
        ExpiredSession,
    }
}
```

This provides several advantages:

- Module paths naturally express error categories and origins
- Easier to maintain module locality
- Helps avoid dependency concentration

In addition, since reason types can be defined easily at the point where errors occur, developers are more likely to implement fine-grained error handling.

### Type-safe reason identification

Error reasons can be identified in a type-safe manner.

```rust
match err.reason::<FileErrors>() {
    FileErrors::FailToOpenFile { file_path } => ...,
    _ => ...,
}
```

```rust
let val: bool = err.match_reason::<AuthServiceErrors, bool>(|r| match r { ... })
    .or_match_reason::<FileErrors>(|r| match r { ... }) 
    .or_result(|e| ... )?;
```

This enables reason-based error handling without relying on string comparisons or error codes.

### Source error chaining

The original source error can be retained.

```rust
let err = Err::with_source(reason, source);
```

This makes it possible to separate:

- High-level failure reasons
- Low-level underlying causes

### Stores error creation location

File and line information at the point of error creation are retained.

This integrates well with logging, monitoring, and telemetry systems.

### Error notification mechanism

The `notify` and `notify-tokio` features provide notification mechanisms for error generation.

Supported features include:

- Synchronous and asynchronous notifications
- Tokio runtime integration
- Inventory-based static registration of error handlers

This makes the library suitable for observability integrations such as logging, monitoring, telemetry, remote reporting, and Rust standard backtraces.

The global or local handlers to be notified whenever an Err is instantiated are registered with functions or macros.

```rust
add_sync_err_handler!(|err, tm| println!("{}: {}", tm, err));

errs::add_sync_err_handler(|err, tm| println!("{}: {}", tm, err));
errs::fix_err_handlers();
```

**NOTE**: If `fix_err_handlers` is not executed explicitly, the handler registration will be fixed internally when the first Err instance is created.

## Installation

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

## Detailed Usage

### Err instantiation and identification of a reason

The `Err` struct can be instantiated with `new<R>(reason: R)` function or
`with_source<R, E>(reason: R, source: E)` function.

Then, the reason can be identified with `reason<R>(&self)` method and a `match` statement,
or `match_reason<R, T>(&self, func: impl FnOnce(&R) -> Result<T, Err>)` method.

The following code is an example which uses `new<R>(reason: R)` function for instantiation,
and `reason<R>(&self)` method and a `match` statement for identifying a reason:

```rust
use errs::Err;

#[derive(Debug)]
enum Reasons {
    IllegalState { state: String },
    // ...
}

let err = Err::new(Reasons::IllegalState { state: "bad state".to_string() });

match err.reason::<Reasons>() {
    Ok(r) => match r {
        Reasons::IllegalState { state } => println!("state = {state}"),
        _ => { /* ... */ }
    }
    Err(e) => match err.reason::<String>() {
        Ok(s) => println!("string reason = {s}"),
        Err(e) => { /* ... */ }
    }
}
```

The following code is an example which uses `match_reason`, `or_match_reason`, and `or_result`
methods for identifying a reason in a chaining manner:

```rust
use errs::Err;

#[derive(Debug)]
enum Reasons {
    IllegalState { state: u8 },
    // ...
}

let err = Err::new(Reasons::IllegalState { state: 0u8 });

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


## Supporting Rust versions

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
   Considered (min … max):   Rust 1.56.1 … Rust 1.95.0
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
