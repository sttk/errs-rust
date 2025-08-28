// Copyright (C) 2025 Takayuki Sato. All Rights Reserved.
// This program is free software under MIT License.
// See the file LICENSE in this distribution for more details.

use crate::{Err, ReasonContainer, SendSyncNonNull};

#[cfg(feature = "notify")]
mod notify;
#[cfg(feature = "notify")]
pub use notify::{add_raw_async_err_handler, add_sync_err_handler, fix_err_handlers};
#[cfg(feature = "notify")]
use notify::{can_notify, notify_err_async, notify_err_sync, will_notify_async};

use std::any;
use std::error;
use std::fmt;
use std::panic;
use std::ptr;
use std::sync::{self, atomic};

unsafe impl<T: Send + Sync> Send for SendSyncNonNull<T> {}

unsafe impl<T: Send + Sync> Sync for SendSyncNonNull<T> {}

impl<T: Send + Sync> SendSyncNonNull<T> {
    fn as_ptr(&self) -> *mut T {
        self.0.as_ptr()
    }
}

impl Err {
    /// Creates a new `Err` instance with the given reason.
    ///
    /// The reason can be of any type, but typically it is an enum variant that uniquely
    /// identifies the error's nature.
    ///
    /// # Parameters
    /// - `reason`: The reason for the error.
    ///
    /// # Returns
    /// A new `Err` instance containing the given reason.
    ///
    /// ```rust
    /// use errs::Err;
    ///
    /// #[derive(Debug)]
    /// enum Reasons {
    ///     IllegalState { state: String },
    /// }
    ///
    /// let err = Err::new(Reasons::IllegalState { state: "bad state".to_string() });
    /// ```
    #[track_caller]
    pub fn new<R>(reason: R) -> Self
    where
        R: fmt::Debug + Send + Sync + 'static,
    {
        let loc = panic::Location::caller();

        #[cfg(feature = "notify")]
        if can_notify() {
            let tm = chrono::Utc::now();

            if will_notify_async() {
                let boxed = Box::new(ReasonContainer::<R>::new(reason, true));
                let ptr = ptr::NonNull::from(Box::leak(boxed)).cast::<ReasonContainer>();

                let err = Self {
                    file: loc.file(),
                    line: loc.line(),
                    reason_container: SendSyncNonNull(ptr),
                    source: None,
                };
                notify_err_sync(&err, &tm);

                let err_notified = Self {
                    file: err.file,
                    line: err.line,
                    reason_container: SendSyncNonNull(ptr),
                    source: None,
                };
                notify_err_async(err_notified, tm);

                return err;
            } else {
                let boxed = Box::new(ReasonContainer::<R>::new(reason, false));
                let ptr = ptr::NonNull::from(Box::leak(boxed)).cast::<ReasonContainer>();

                let err = Self {
                    file: loc.file(),
                    line: loc.line(),
                    reason_container: SendSyncNonNull(ptr),
                    source: None,
                };
                notify_err_sync(&err, &tm);

                return err;
            }
        }

        let boxed = Box::new(ReasonContainer::<R>::new(reason, false));
        let ptr = ptr::NonNull::from(Box::leak(boxed)).cast::<ReasonContainer>();
        Self {
            file: loc.file(),
            line: loc.line(),
            reason_container: SendSyncNonNull(ptr),
            source: None,
        }
    }

    /// Creates a new `Err` instance with the give reason and underlying source error.
    ///
    /// This constructor is useful when the error is caused by another error.
    ///
    /// # Parameters
    /// - `reason`: The reason for the error.
    /// - `source`: The underlying source error that caused the error.
    ///
    /// # Returns
    /// A new `Err` instance containing the given reason and source error.
    ///
    ///
    /// ```rust
    /// use errs::Err;
    /// use std::io;
    ///
    /// #[derive(Debug)]
    /// enum Reasons {
    ///     FailToDoSomething,
    /// }
    ///
    /// let io_error = io::Error::other("oh no!");
    ///
    /// let err = Err::with_source(Reasons::FailToDoSomething, io_error);
    /// ```
    #[track_caller]
    pub fn with_source<R, E>(reason: R, source: E) -> Self
    where
        R: fmt::Debug + Send + Sync + 'static,
        E: error::Error + Send + Sync + 'static,
    {
        let loc = panic::Location::caller();

        #[cfg(feature = "notify")]
        if can_notify() {
            let tm = chrono::Utc::now();

            if will_notify_async() {
                let boxed = Box::new(ReasonContainer::<R>::new(reason, true));
                let ptr = ptr::NonNull::from(Box::leak(boxed)).cast::<ReasonContainer>();
                let src_arc = sync::Arc::<E>::new(source);

                let err = Self {
                    file: loc.file(),
                    line: loc.line(),
                    reason_container: SendSyncNonNull(ptr),
                    source: Some(Box::new(src_arc.clone())),
                };
                notify_err_sync(&err, &tm);

                let err_notified = Self {
                    file: err.file,
                    line: err.line,
                    reason_container: SendSyncNonNull(ptr),
                    source: Some(Box::new(src_arc)),
                };
                notify_err_async(err_notified, tm);

                return err;
            } else {
                let boxed = Box::new(ReasonContainer::<R>::new(reason, false));
                let ptr = ptr::NonNull::from(Box::leak(boxed)).cast::<ReasonContainer>();

                let err = Self {
                    file: loc.file(),
                    line: loc.line(),
                    reason_container: SendSyncNonNull(ptr),
                    source: Some(Box::new(source)),
                };
                notify_err_sync(&err, &tm);

                return err;
            }
        }

        let boxed = Box::new(ReasonContainer::<R>::new(reason, false));
        let ptr = ptr::NonNull::from(Box::leak(boxed)).cast::<ReasonContainer>();

        Self {
            file: loc.file(),
            line: loc.line(),
            reason_container: SendSyncNonNull(ptr),
            source: Some(Box::new(source)),
        }
    }

    /// Attempts to retrieve the error's reason as a specific type.
    ///
    /// This method checks whether the stored reason matches the specified type
    /// and returns a reference to the reason if the type matches.
    ///
    /// # Parameters
    /// - `R`: The expected type of the reason.
    ///
    /// # Returns
    /// - `Ok(&R)`: A reference to the reason if it is of the specified type.
    /// - `Err(&self)`: A reference to this `Err` itself if the reason is not of the specified type.
    ///
    ///
    /// ```rust
    /// use errs::Err;
    ///
    /// #[derive(Debug)]
    /// enum Reasons {
    ///     IllegalState { state: String },
    /// }
    ///
    /// let err = Err::new(Reasons::IllegalState { state: "bad state".to_string() });
    /// match err.reason::<Reasons>() {
    ///   Ok(r) => match r {
    ///     Reasons::IllegalState { state } => println!("state = {state}"),
    ///     _ => { /* ... */ }
    ///   }
    ///   Err(err) => match err.reason::<String>() {
    ///      Ok(s) => println!("string reason = {s}"),
    ///      Err(_err) => { /* ... */ }
    ///   }
    /// }
    /// ```
    pub fn reason<R>(&self) -> Result<&R, &Self>
    where
        R: fmt::Debug + Send + Sync + 'static,
    {
        let type_id = any::TypeId::of::<R>();
        let ptr = self.reason_container.as_ptr();
        let is_fn = unsafe { (*ptr).is_fn };
        if is_fn(type_id) {
            let typed_ptr = ptr as *const ReasonContainer<R>;
            return Ok(unsafe { &((*typed_ptr).reason) });
        }

        Err(self)
    }

    /// Executes a function if the error's reason matches a specific type.
    ///
    /// This method allows you to perform actions based on the type of the error's reason.
    /// If the reason matches the expected type, the provided function is called with
    /// a reference to the reason.
    ///
    /// # Parameters
    /// - `R`: The expected type of the reason.
    /// - `func`: The function to execute if the reason matches the type.
    ///
    /// # Returns
    /// A reference to the current `Err` instance.
    ///
    ///
    /// ```rust
    /// use errs::Err;
    ///
    /// #[derive(Debug)]
    /// enum Reasons {
    ///     IllegalState { state: String },
    /// }
    ///
    /// let err = Err::new(Reasons::IllegalState { state: "bad state".to_string() });
    /// err.match_reason::<Reasons>(|r| match r {
    ///     Reasons::IllegalState { state } => println!("state = {state}"),
    ///     _ => { /* ... */ }
    /// })
    /// .match_reason::<String>(|s| {
    ///     println!("string reason = {s}");
    /// });
    /// ```
    pub fn match_reason<R>(&self, func: fn(&R)) -> &Self
    where
        R: fmt::Debug + Send + Sync + 'static,
    {
        let type_id = any::TypeId::of::<R>();
        let ptr = self.reason_container.as_ptr();
        let is_fn = unsafe { (*ptr).is_fn };
        if is_fn(type_id) {
            let typed_ptr = ptr as *const ReasonContainer<R>;
            func(unsafe { &((*typed_ptr).reason) });
        }

        self
    }
}

impl Drop for Err {
    fn drop(&mut self) {
        let ptr = self.reason_container.as_ptr();
        let drop_fn = unsafe { (*ptr).drop_fn };
        drop_fn(ptr);
    }
}

impl fmt::Debug for Err {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let ptr = self.reason_container.as_ptr();
        let debug_fn = unsafe { (*ptr).debug_fn };

        write!(f, "{} {{ reason = ", any::type_name::<Err>())?;
        debug_fn(ptr, f)?;

        write!(f, ", file = {}, line = {}", self.file, self.line)?;

        if let Some(src) = &self.source {
            write!(f, ", source = {:?}", src)?;
        }

        write!(f, " }}")
    }
}

impl fmt::Display for Err {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let ptr = self.reason_container.as_ptr();
        let display_fn = unsafe { (*ptr).display_fn };
        display_fn(ptr, f)
    }
}

impl error::Error for Err {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        self.source
            .as_deref()
            .map(|e| e as &(dyn error::Error + 'static))
    }
}

impl<R> ReasonContainer<R>
where
    R: fmt::Debug + Send + Sync + 'static,
{
    fn new(reason: R, use_async_handler: bool) -> Self {
        let is_referenced_by_another = if use_async_handler {
            Some(atomic::AtomicBool::new(use_async_handler))
        } else {
            None
        };

        Self {
            is_fn: is_reason::<R>,
            drop_fn: drop_reason::<R>,
            debug_fn: debug_reason::<R>,
            display_fn: display_reason::<R>,
            reason,
            is_referenced_by_another,
        }
    }
}

fn is_reason<R>(type_id: any::TypeId) -> bool
where
    R: fmt::Debug + Send + Sync + 'static,
{
    any::TypeId::of::<R>() == type_id
}

fn drop_reason<R>(ptr: *const ReasonContainer)
where
    R: fmt::Debug + Send + Sync + 'static,
{
    let typed_ptr = ptr as *mut ReasonContainer<R>;
    unsafe {
        match &(*typed_ptr).is_referenced_by_another {
            Some(atomic_bool) => {
                if let Err(_) = atomic_bool.compare_exchange(
                    true,
                    false,
                    atomic::Ordering::Relaxed,
                    atomic::Ordering::Relaxed,
                ) {
                    drop(Box::from_raw(typed_ptr));
                }
            }
            None => {
                drop(Box::from_raw(typed_ptr));
            }
        }
    }
}

fn debug_reason<R>(ptr: *const ReasonContainer, f: &mut fmt::Formatter<'_>) -> fmt::Result
where
    R: fmt::Debug + Send + Sync + 'static,
{
    let typed_ptr = ptr as *mut ReasonContainer<R>;
    write!(f, "{} {:?}", any::type_name::<R>(), unsafe {
        &(*typed_ptr).reason
    })
}

fn display_reason<R>(ptr: *const ReasonContainer, f: &mut fmt::Formatter<'_>) -> fmt::Result
where
    R: fmt::Debug + Send + Sync + 'static,
{
    let typed_ptr = ptr as *mut ReasonContainer<R>;
    write!(f, "{:?}", unsafe { &(*typed_ptr).reason })
}

#[cfg(test)]
mod tests_of_err {
    use super::*;
    use std::error::Error;
    use std::sync::{LazyLock, Mutex};

    struct Logger {
        log_vec: Vec<String>,
    }

    const BASE_LINE: u32 = line!();

    #[allow(dead_code)]
    impl Logger {
        fn new() -> Self {
            Self {
                log_vec: Vec::<String>::new(),
            }
        }
        fn log(&mut self, s: &str) {
            self.log_vec.push(s.to_string());
        }
        fn assert_logs(&self, logs: &[&str]) {
            if self.log_vec.len() != logs.len() {
                assert_eq!(self.log_vec, logs);
                return;
            }
            for i in 0..self.log_vec.len() {
                assert_eq!(self.log_vec[i], logs[i]);
            }
        }
        fn clear(&mut self) {
            self.log_vec.clear();
        }
    }

    mod test_of_drop {
        use super::*;
        use std::thread;

        static LOGGER: LazyLock<Mutex<Logger>> = LazyLock::new(|| Mutex::new(Logger::new()));

        #[allow(dead_code)]
        #[derive(Debug)]
        enum Enum0 {
            InvalidValue { name: String, value: String },
            FailToGetValue { name: String },
        }
        impl Drop for Enum0 {
            fn drop(&mut self) {
                LOGGER.lock().unwrap().log("drop Enum0");
            }
        }

        fn create_err() -> Result<(), Err> {
            let err = Err::new(Enum0::InvalidValue {
                name: "foo".to_string(),
                value: "abc".to_string(),
            });
            LOGGER.lock().unwrap().log("created Enum0");
            Err(err)
        }

        fn consume_err() {
            let err = create_err().unwrap_err();
            assert_eq!(
                format!("{err}"),
                "InvalidValue { name: \"foo\", value: \"abc\" }",
            );
            #[cfg(unix)]
            assert_eq!(
                format!("{err:?}"),
                format!("errs::Err {{ reason = errs::err::tests_of_err::test_of_drop::Enum0 InvalidValue {{ name: \"foo\", value: \"abc\" }}, file = src/err/mod.rs, line = {} }}", BASE_LINE + 45),
            );
            #[cfg(windows)]
            assert_eq!(
                format!("{err:?}"),
                format!("errs::Err {{ reason = errs::err::tests_of_err::test_of_drop::Enum0 InvalidValue {{ name: \"foo\", value: \"abc\" }}, file = src\\err\\mod.rs, line = {} }}", BASE_LINE + 45),
            );

            LOGGER.lock().unwrap().log("consumed Enum0");
        }

        #[test]
        fn test() {
            consume_err();
            thread::sleep(std::time::Duration::from_millis(200));
            LOGGER.lock().unwrap().log("end");

            LOGGER.lock().unwrap().assert_logs(&[
                "created Enum0",
                "consumed Enum0",
                "drop Enum0",
                "end",
            ]);
        }
    }

    mod test_of_new {
        use super::*;

        #[allow(dead_code)]
        #[derive(Debug)]
        enum Enum0 {
            InvalidValue { name: String, value: String },
            FailToGetValue { name: String },
        }

        #[test]
        fn reason_is_enum() {
            let err = Err::new(Enum0::InvalidValue {
                name: "foo".to_string(),
                value: "abc".to_string(),
            });

            #[cfg(unix)]
            assert_eq!(err.file, "src/err/mod.rs");
            #[cfg(windows)]
            assert_eq!(err.file, "src\\err\\mod.rs");
            assert_eq!(err.line, BASE_LINE + 100);
            assert_eq!(
                format!("{err}"),
                "InvalidValue { name: \"foo\", value: \"abc\" }",
            );
            #[cfg(unix)]
            assert_eq!(
                format!("{err:?}"),
                format!("errs::Err {{ reason = errs::err::tests_of_err::test_of_new::Enum0 InvalidValue {{ name: \"foo\", value: \"abc\" }}, file = src/err/mod.rs, line = {} }}", BASE_LINE + 100),
            );
            #[cfg(windows)]
            assert_eq!(
                format!("{err:?}"),
                format!("errs::Err {{ reason = errs::err::tests_of_err::test_of_new::Enum0 InvalidValue {{ name: \"foo\", value: \"abc\" }}, file = src\\err\\mod.rs, line = {} }}", BASE_LINE + 100),
            );
            assert!(err.source().is_none());
        }

        #[test]
        fn source_is_a_standard_error() {
            let source = std::io::Error::new(std::io::ErrorKind::NotFound, "oh no!");
            let err = Err::with_source(
                Enum0::InvalidValue {
                    name: "foo".to_string(),
                    value: "abc".to_string(),
                },
                source,
            );

            #[cfg(unix)]
            assert_eq!(err.file, "src/err/mod.rs");
            #[cfg(windows)]
            assert_eq!(err.file, "src\\err\\mod.rs");

            assert_eq!(err.line, BASE_LINE + 130);
            assert_eq!(
                format!("{err}"),
                "InvalidValue { name: \"foo\", value: \"abc\" }",
            );

            #[cfg(unix)]
            assert_eq!(
                format!("{err:?}"),
                format!("errs::Err {{ reason = errs::err::tests_of_err::test_of_new::Enum0 InvalidValue {{ name: \"foo\", value: \"abc\" }}, file = src/err/mod.rs, line = {}, source = Custom {{ kind: NotFound, error: \"oh no!\" }} }}", BASE_LINE + 130),
            );
            #[cfg(windows)]
            assert_eq!(
                format!("{err:?}"),
                format!("errs::Err {{ reason = errs::err::tests_of_err::test_of_new::Enum0 InvalidValue {{ name: \"foo\", value: \"abc\" }}, file = src\\err\\mod.rs, line = {}, source = Custom {{ kind: NotFound, error: \"oh no!\" }} }}", BASE_LINE + 130),
            );
            assert!(err.source().is_some());
        }

        #[allow(dead_code)]
        #[derive(Debug)]
        struct MyError {
            message: String,
        }
        impl MyError {
            fn new(msg: &str) -> Self {
                Self {
                    message: msg.to_string(),
                }
            }
        }
        impl fmt::Display for MyError {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
                write!(f, "MyError{{message:\"{}\"}}", self.message)
            }
        }
        impl error::Error for MyError {}

        #[test]
        fn source_is_a_custom_error() {
            let source = MyError::new("hello");
            let err = Err::with_source(
                Enum0::InvalidValue {
                    name: "foo".to_string(),
                    value: "abc".to_string(),
                },
                source,
            );

            #[cfg(unix)]
            assert_eq!(err.file, "src/err/mod.rs");
            #[cfg(windows)]
            assert_eq!(err.file, "src\\err\\mod.rs");
            assert_eq!(err.line, BASE_LINE + 184);
            assert_eq!(
                format!("{err}"),
                "InvalidValue { name: \"foo\", value: \"abc\" }",
            );
            #[cfg(unix)]
            assert_eq!(
                format!("{err:?}"),
                format!("errs::Err {{ reason = errs::err::tests_of_err::test_of_new::Enum0 InvalidValue {{ name: \"foo\", value: \"abc\" }}, file = src/err/mod.rs, line = {}, source = MyError {{ message: \"hello\" }} }}", BASE_LINE + 184),
            );
            #[cfg(windows)]
            assert_eq!(
                format!("{err:?}"),
                format!("errs::Err {{ reason = errs::err::tests_of_err::test_of_new::Enum0 InvalidValue {{ name: \"foo\", value: \"abc\" }}, file = src\\err\\mod.rs, line = {}, source = MyError {{ message: \"hello\" }} }}", BASE_LINE + 184),
            );
            assert!(err.source().is_some());
        }

        #[test]
        fn source_is_also_a_errs_err() {
            let source = Err::new(Enum0::InvalidValue {
                name: "foo".to_string(),
                value: "abc".to_string(),
            });

            let err = Err::with_source(
                Enum0::FailToGetValue {
                    name: "foo".to_string(),
                },
                source,
            );

            #[cfg(unix)]
            assert_eq!(err.file, "src/err/mod.rs");
            #[cfg(windows)]
            assert_eq!(err.file, "src\\err\\mod.rs");
            assert_eq!(err.line, BASE_LINE + 221);
            assert_eq!(format!("{err}"), "FailToGetValue { name: \"foo\" }",);
            #[cfg(unix)]
            assert_eq!(
                format!("{err:?}"),
                format!("errs::Err {{ reason = errs::err::tests_of_err::test_of_new::Enum0 FailToGetValue {{ name: \"foo\" }}, file = src/err/mod.rs, line = {}, source = errs::Err {{ reason = errs::err::tests_of_err::test_of_new::Enum0 InvalidValue {{ name: \"foo\", value: \"abc\" }}, file = src/err/mod.rs, line = {} }} }}", BASE_LINE + 221, BASE_LINE + 216),
            );
            #[cfg(windows)]
            assert_eq!(
                format!("{err:?}"),
                format!("errs::Err {{ reason = errs::err::tests_of_err::test_of_new::Enum0 FailToGetValue {{ name: \"foo\" }}, file = src\\err\\mod.rs, line = {}, source = errs::Err {{ reason = errs::err::tests_of_err::test_of_new::Enum0 InvalidValue {{ name: \"foo\", value: \"abc\" }}, file = src\\err\\mod.rs, line = {} }} }}", BASE_LINE + 221, BASE_LINE + 216),
            );
            assert!(err.source().is_some());
        }

        #[test]
        fn reason_is_a_boolean() {
            let err = Err::new(true);
            assert_eq!(format!("{err}"), "true");
            #[cfg(unix)]
            assert_eq!(
                format!("{err:?}"),
                format!(
                    "errs::Err {{ reason = bool true, file = src/err/mod.rs, line = {} }}",
                    BASE_LINE + 249,
                ),
            );
            #[cfg(windows)]
            assert_eq!(
                format!("{err:?}"),
                format!(
                    "errs::Err {{ reason = bool true, file = src\\err\\mod.rs, line = {} }}",
                    BASE_LINE + 249,
                ),
            );
            assert!(err.source().is_none());
        }

        #[test]
        fn reason_is_a_number() {
            let err = Err::new(123i64);
            assert_eq!(format!("{err}"), "123");
            #[cfg(unix)]
            assert_eq!(
                format!("{err:?}"),
                format!(
                    "errs::Err {{ reason = i64 123, file = src/err/mod.rs, line = {} }}",
                    BASE_LINE + 272,
                ),
            );
            #[cfg(windows)]
            assert_eq!(
                format!("{err:?}"),
                format!(
                    "errs::Err {{ reason = i64 123, file = src\\err\\mod.rs, line = {} }}",
                    BASE_LINE + 272,
                ),
            );
            assert!(err.source().is_none());
        }

        #[test]
        fn reason_is_a_string() {
            let err = Err::new("abc".to_string());
            assert_eq!(format!("{err}"), "\"abc\"");
            #[cfg(unix)]
            assert_eq!(
                format!("{err:?}"),
                format!("errs::Err {{ reason = alloc::string::String \"abc\", file = src/err/mod.rs, line = {} }}", BASE_LINE + 295),
            );
            #[cfg(windows)]
            assert_eq!(
                format!("{err:?}"),
                format!("errs::Err {{ reason = alloc::string::String \"abc\", file = src\\err\\mod.rs, line = {} }}", BASE_LINE + 295),
            );
            assert!(err.source().is_none());
        }

        #[allow(dead_code)]
        #[derive(Debug)]
        struct StructA {
            name: String,
            value: i64,
        }

        #[test]
        fn reason_is_a_struct() {
            let err = Err::new(StructA {
                name: "abc".to_string(),
                value: 123,
            });
            assert_eq!(format!("{err}"), "StructA { name: \"abc\", value: 123 }");
            #[cfg(unix)]
            assert_eq!(
                format!("{err:?}"),
                format!("errs::Err {{ reason = errs::err::tests_of_err::test_of_new::StructA StructA {{ name: \"abc\", value: 123 }}, file = src/err/mod.rs, line = {} }}", BASE_LINE + 319),
            );
            #[cfg(windows)]
            assert_eq!(
                format!("{err:?}"),
                format!("errs::Err {{ reason = errs::err::tests_of_err::test_of_new::StructA StructA {{ name: \"abc\", value: 123 }}, file = src\\err\\mod.rs, line = {} }}", BASE_LINE + 319),
            );
            assert!(err.source().is_none());
        }

        #[test]
        fn reaason_is_an_unit() {
            let err = Err::new(());
            assert_eq!(format!("{err}"), "()");
            #[cfg(unix)]
            assert_eq!(
                format!("{err:?}"),
                format!(
                    "errs::Err {{ reason = () (), file = src/err/mod.rs, line = {} }}",
                    BASE_LINE + 339,
                ),
            );
            #[cfg(windows)]
            assert_eq!(
                format!("{err:?}"),
                format!(
                    "errs::Err {{ reason = () (), file = src\\err\\mod.rs, line = {} }}",
                    BASE_LINE + 339,
                ),
            );
            assert!(err.source().is_none());
        }
    }

    mod match_statement_for_reason {
        use super::*;

        #[allow(dead_code)]
        #[derive(Debug)]
        enum Enum0 {
            InvalidValue { name: String, value: String },
            FailToGetValue { name: String },
        }

        #[test]
        fn reason_is_enum() {
            let err = Err::new(Enum0::InvalidValue {
                name: "foo".to_string(),
                value: "abc".to_string(),
            });

            match err.reason::<String>() {
                Ok(_) => panic!(),
                Err(err) => match err.reason::<Enum0>() {
                    Ok(r) => match r {
                        Enum0::InvalidValue { name, value } => {
                            assert_eq!(name, "foo");
                            assert_eq!(value, "abc");
                        }
                        _ => panic!(),
                    },
                    Err(_) => panic!(),
                },
            }

            err.match_reason::<String>(|_s| {
                panic!();
            })
            .match_reason::<Enum0>(|r| match r {
                Enum0::InvalidValue { name, value } => {
                    assert_eq!(name, "foo");
                    assert_eq!(value, "abc");
                }
                _ => panic!(),
            });
        }
    }
}
