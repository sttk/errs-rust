// Copyright (C) 2024 Takayuki Sato. All Rights Reserved.
// This program is free software under MIT License.
// See the file LICENSE in this distribution for more details.

use crate::Err;

use chrono::{DateTime, Utc};
use futures::future::join_all;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex, OnceLock};

type BoxedFn = Box<dyn Fn(&Err, &DateTime<Utc>) + Send + Sync + 'static>;
type BoxedFut = Pin<Box<dyn Future<Output = ()> + Send + Sync + 'static>>;
type BoxedFutFn = Box<dyn Fn(Arc<Err>, DateTime<Utc>) -> BoxedFut + Send + Sync + 'static>;

static SYNC_VEC_MUTEX: Mutex<Vec<BoxedFn>> = Mutex::new(Vec::new());
static SYNC_VEC_FIXED: OnceLock<Vec<BoxedFn>> = OnceLock::new();
static ASYNC_VEC_MUTEX: Mutex<Vec<BoxedFutFn>> = Mutex::new(Vec::new());
static ASYNC_VEC_FIXED: OnceLock<Vec<BoxedFutFn>> = OnceLock::new();

/// Adds a synchronous error handler.
///
/// This function allows you to register a closure that will be executed synchronously
/// when an error is notified. Handlers can only be added before they are fixed
/// by calling `fix_err_handlers`.
///
/// # Arguments
///
/// * `handler` - A closure that takes a reference to `errs::Err` and `DateTime<Utc>`.
pub fn add_sync_err_handler<F>(handler: F)
where
    F: Fn(&Err, &DateTime<Utc>) + Send + Sync + 'static,
{
    if SYNC_VEC_FIXED.get().is_none() {
        if let Ok(mut vec) = SYNC_VEC_MUTEX.lock() {
            vec.push(Box::new(handler));
        } else {
            eprintln!(
                "ERROR: Failed to add synchronous errs::Err handler due to a Mutex lock failure."
            );
        }
    } else {
        eprintln!("WARNING: Attempted to add synchronous errs::Err handler after fixed. The operation was ignored.");
    }
}

/// Adds an asynchronous error handler.
///
/// This is a low-level, public function that directly adds a boxed closure to the
/// handler list. It is typically not called directly by users but is instead
/// used internally by the `add_async_err_handler!` macro to provide a
/// more convenient syntax.
///
/// Handlers can only be added before they are finalized by calling `fix_err_handlers`.
///
/// # Arguments
///
/// * `handler` - A closure that takes an `Arc<Err>` and `DateTime<Utc>`, and returns a boxed future.
pub fn add_raw_async_err_handler<F>(handler: F)
where
    F: Fn(Arc<Err>, DateTime<Utc>) -> BoxedFut + Send + Sync + 'static,
{
    if ASYNC_VEC_FIXED.get().is_none() {
        if let Ok(mut vec) = ASYNC_VEC_MUTEX.lock() {
            vec.push(Box::new(handler));
        } else {
            eprintln!(
                "ERROR: Failed to add asynchronous errs::Err handler due to a Mutex lock failure."
            );
        }
    } else {
        eprintln!("WARNING: Attempted to add asynchronous errs::Err handler after fixed. The operation was ignored.");
    }
}

/// A macro for adding asynchronous error handlers.
///
/// This macro provides a simplified syntax for registering an asynchronous error handler.
/// It wraps a closure, converting it into a `Pin<Box<dyn Future...>>` and
/// passing it to the internal `add_raw_async_err_handler` function.
///
/// **Note**: Handlers can only be added before they are fixed by calling `fix_err_handlers`.
///
/// # Syntax
///
/// `add_async_err_handler!(async |err, tm| { ... })`  // without environment capture
/// `add_async_err_handler!(move |err, tm| { async move { ... } })`  // with environment capture
///
/// # Arguments
///
/// * `err` - The captured `Arc<Err>` instance.
/// * `tm` - The captured `DateTime<Utc>` instance.
#[cfg(feature = "errs-notify")]
#[cfg_attr(docsrs, doc(cfg(feature = "errs-notify")))]
#[macro_export]
macro_rules! add_async_err_handler {
    ( async | $err:ident , $tm:ident | $body:block ) => {
        $crate::add_raw_async_err_handler(
            |$err: std::sync::Arc<$crate::Err>, $tm: chrono::DateTime<chrono::Utc>| {
                Box::pin(async move { $body })
            },
        )
    };
    ( move | $err:ident , $tm:ident | $body:block) => {
        $crate::add_raw_async_err_handler(
            move |$err: std::sync::Arc<$crate::Err>, $tm: chrono::DateTime<chrono::Utc>| {
                Box::pin({ $body })
            },
        )
    };
}

pub fn fix_err_handlers() {
    if SYNC_VEC_FIXED.get().is_none() {
        if let Ok(mut vec) = SYNC_VEC_MUTEX.lock() {
            let owned_vec = std::mem::take(&mut *vec);
            let _ = SYNC_VEC_FIXED.set(owned_vec);
        } else {
            eprintln!(
                "ERROR: Failed to fix synchronous errs::Err handler due to a Mutex lock failure."
            );
        }
    }

    if ASYNC_VEC_FIXED.get().is_none() {
        if let Ok(mut vec) = ASYNC_VEC_MUTEX.lock() {
            let owned_vec = std::mem::take(&mut *vec);
            let _ = ASYNC_VEC_FIXED.set(owned_vec);
        } else {
            eprintln!(
                "ERROR: Failed to fix asynchronous errs::Err handler due to a Mutex lock failure."
            );
        }
    }
}

pub(crate) fn can_notify() -> bool {
    SYNC_VEC_FIXED.get().is_some()
}

pub(crate) fn will_notify_async() -> bool {
    if let Some(vec) = ASYNC_VEC_FIXED.get() {
        return !vec.is_empty();
    }
    false
}

pub(crate) fn notify_err_sync(err: &Err, tm: &chrono::DateTime<chrono::Utc>) {
    if let Some(vec) = SYNC_VEC_FIXED.get() {
        for handle in vec.iter() {
            handle(err, tm)
        }
    }
}

pub(crate) fn notify_err_async(err: Err, tm: chrono::DateTime<chrono::Utc>) {
    let task = move || async move {
        if let Some(vec) = ASYNC_VEC_FIXED.get() {
            let err_arc = Arc::<Err>::new(err);
            let futures: Vec<BoxedFut> = vec.iter().map(|f| f(Arc::clone(&err_arc), tm)).collect();
            join_all(futures).await;
        }
    };

    if let Ok(handle) = tokio::runtime::Handle::try_current() {
        handle.spawn(task());
    } else {
        std::thread::spawn(move || match tokio::runtime::Runtime::new() {
            Ok(rt) => {
                rt.block_on(task());
            }
            Err(e) => {
                eprintln!("Failed to create a tokiio runtime due to: {:?}", e);
            }
        });
    }
}

#[cfg(test)]
mod tests_of_notify {
    use super::*;
    use std::sync::{LazyLock, Mutex};

    static LOGGER: LazyLock<Mutex<Vec<String>>> = LazyLock::new(|| Mutex::new(Vec::new()));

    #[derive(Debug)]
    enum Errors {
        FailToDoSomething,
    }

    #[test]
    fn test() {
        add_sync_err_handler(|err, _tm| {
            if err.file.ends_with("notify.rs") {
                LOGGER.lock().unwrap().push(format!("1: err={err:?}"));
            }
        });

        add_raw_async_err_handler(|err, _tm| {
            Box::pin(async move {
                if err.file.ends_with("notify.rs") {
                    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                    LOGGER.lock().unwrap().push(format!("2: err={err:?}"));
                }
            })
        });

        add_async_err_handler!(async |err, _tm| {
            if err.file.ends_with("notify.rs") {
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                LOGGER.lock().unwrap().push(format!("3: err={err:?}"));
            }
        });

        let n1 = Arc::new(1);
        let n2 = n1.clone();

        add_raw_async_err_handler(move |err, _tm| {
            Box::pin({
                let n_cloned = n1.clone();
                async move {
                    if err.file.ends_with("notify.rs") {
                        tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
                        LOGGER
                            .lock()
                            .unwrap()
                            .push(format!("4: err={err:?}, captured n={n_cloned}"));
                    }
                }
            })
        });

        add_async_err_handler!(move |err, _tm| {
            let n_cloned = n2.clone();
            async move {
                if err.file.ends_with("notify.rs") {
                    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                    LOGGER
                        .lock()
                        .unwrap()
                        .push(format!("5: err={err:?}, captured n={n_cloned}"));
                }
            }
        });

        fix_err_handlers();

        add_sync_err_handler(move |_err, _tm| {
            LOGGER
                .lock()
                .unwrap()
                .push(format!("handler isn't added handler after fixed"));
        });

        add_raw_async_err_handler(|_err, _tm| {
            Box::pin({
                async move {
                    LOGGER
                        .lock()
                        .unwrap()
                        .push(format!("handler isn't added handler after fixed"));
                }
            })
        });

        add_async_err_handler!(async |_err, _tm| {
            LOGGER
                .lock()
                .unwrap()
                .push(format!("handler isn't added handler after fixed"));
        });

        let _ = Err::new(Errors::FailToDoSomething);
        //assert_eq!(LOGGER.lock().unwrap().len(), 1);

        std::thread::sleep(std::time::Duration::from_secs(6));

        for log in LOGGER.lock().unwrap().iter() {
            println!("{}", log);
        }
        assert_eq!(LOGGER.lock().unwrap().len(), 5);

        let vec = LOGGER.lock().unwrap();
        #[cfg(unix)]
        {
            assert_eq!(vec[0], "1: err=errs::Err { reason = errs::notify::tests_of_notify::Errors FailToDoSomething, file = src/notify.rs, line = 274 }");
            assert_eq!(vec[1], "3: err=errs::Err { reason = errs::notify::tests_of_notify::Errors FailToDoSomething, file = src/notify.rs, line = 274 }");
            assert_eq!(vec[2], "5: err=errs::Err { reason = errs::notify::tests_of_notify::Errors FailToDoSomething, file = src/notify.rs, line = 274 }, captured n=1");
            assert_eq!(vec[3], "4: err=errs::Err { reason = errs::notify::tests_of_notify::Errors FailToDoSomething, file = src/notify.rs, line = 274 }, captured n=1");
            assert_eq!(vec[4], "2: err=errs::Err { reason = errs::notify::tests_of_notify::Errors FailToDoSomething, file = src/notify.rs, line = 274 }");
        }
        #[cfg(windows)]
        {
            assert_eq!(vec[0], "1: err=errs::Err { reason = errs::notify::tests_of_notify::Errors FailToDoSomething, file = src\\notify.rs, line = 274 }");
            assert_eq!(vec[1], "3: err=errs::Err { reason = errs::notify::tests_of_notify::Errors FailToDoSomething, file = src\\notify.rs, line = 274 }");
            assert_eq!(vec[2], "5: err=errs::Err { reason = errs::notify::tests_of_notify::Errors FailToDoSomething, file = src\\notify.rs, line = 274 }, captured n=1");
            assert_eq!(vec[3], "4: err=errs::Err { reason = errs::notify::tests_of_notify::Errors FailToDoSomething, file = src\\notify.rs, line = 274 }, captured n=1");
            assert_eq!(vec[4], "2: err=errs::Err { reason = errs::notify::tests_of_notify::Errors FailToDoSomething, file = src\\notify.rs, line = 274 }");
        }
    }
}
