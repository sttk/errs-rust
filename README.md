# [errs for Rust][repo-url] [![crates.io][cratesio-img]][cratesio-url] [![doc.rs][docrs-img]][docrs-url] [![CI Status][ci-img]][ci-url] [![MIT License][mit-img]][mit-url]

This crate is for error handling in Rust programs, providing an Err struct which represents an error with a reason.

The type of this reason is any, but typically an enum variant is used. The name of this enum variant indicates the reason for the error, and its fields store contextual information about the situation in which the error occurred. Since the path of an enum variant, including its package, is unique within a program, the enum variant representing the reason is useful for identifying the specific error, locating where it occurred, or generating appropriate error messages, etc.

Optionally, by using notify feature and registering error handlers in advance, it is possible to receive notifications either synchronously or asynchronously at the time the error struct is created.

## Installation

In `Cargo.toml`, write this crate as a dependency:

```toml
[dependencies]
errs = "0.3.0"
```

If you want to use error notification, specifies `errs-notify` or `full` in the dependency features:

```toml
[dependencies]
errs = { version = "0.3.0", features = ["errs-notify"] }
```

## Usage

### Err instantiation and identification of a reason

The `Err` struct can be instantiated with `new<R>(reason: R)` function or
`with_source<R, E>(reason: R, source: E)` function.

Then, the reason can be identified with `reason<R>(&self)` method and a `match` statement,
or `match_reason<R>(&self, func fn(&R))` method.

The following code is an example which uses `new<R>(reason: R)` function for instantiation,
and `reason<R>(&self)` method and a `match` statement for identifying a reason:

```
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
    Err(err) => match err.reason::<String>() {
        Ok(s) => println!("string reason = {s}"),
        Err(err) => { /* ... */ }
    }
}
```

### Notification of Err instantiations

This crate optionally provides a feature to notify pre-registered error handlers when an `Err`
is instantiated.
Multiple error handlers can be registered, and you can choose to receive notifications either
synchronously or asynchronously.
To register error handlers that receive notifications synchronously, use the
`add_sync_err_handler` function.
For asynchronous notifications, use the `add_async_err_handler!` macro.

Error notifications will not occur until the `fix_err_handlers` function is called.
This function locks the current set of error handlers, preventing further additions and
enabling notification processing.

```
errs::add_async_err_handler!(async |err, tm| {
    println!("{}:{}:{} - {}", tm, err.file, err.line, err);
});

errs::add_sync_err_handler(|err, tm| {
    // ...
});

errs::fix_err_handlers();
```

## Supporting Rust versions

This crate supports Rust 1.80.1 or later.

```sh
% ./build.sh msrv
  [Meta]   cargo-msrv 0.18.4

Compatibility Check #1: Rust 1.73.0
  [FAIL]   Is incompatible

Compatibility Check #2: Rust 1.81.0
  [OK]     Is compatible

Compatibility Check #3: Rust 1.77.2
  [FAIL]   Is incompatible

Compatibility Check #4: Rust 1.79.0
  [FAIL]   Is incompatible

Compatibility Check #5: Rust 1.80.1
  [OK]     Is compatible

Result:
   Considered (min … max):   Rust 1.56.1 … Rust 1.89.0
   Search method:            bisect
   MSRV:                     1.80.1
   Target:                   x86_64-apple-darwin
```

## License

Copyright (C) 2025 Takayuki Sato

This program is free software under MIT License.<br>
See the file LICENSE in this distribution for more details.


[repo-url]: https://github.com/sttk/errs-rust
[cratesio-img]: https://img.shields.io/badge/crates.io-ver.0.3.0-fc8d62?logo=rust
[cratesio-url]: https://crates.io/crates/errs
[docrs-img]: https://img.shields.io/badge/doc.rs-errs-66c2a5?logo=docs.rs
[docrs-url]: https://docs.rs/errs
[ci-img]: https://github.com/sttk/errs-rust/actions/workflows/rust.yml/badge.svg?branch=main
[ci-url]: https://github.com/sttk/errs-rust/actions?query=branch%3Amain
[mit-img]: https://img.shields.io/badge/license-MIT-green.svg
[mit-url]: https://opensource.org/licenses/MIT
