// Copyright (C) 2025 Takayuki Sato. All Rights Reserved.
// This program is free software under MIT License.
// See the file LICENSE in this distribution for more details.

use crate::{Err, ReasonAndSource, SendSyncNonNull};

#[cfg(any(feature = "errs-notify", feature = "errs-notify-tokio"))]
use crate::notify;

use std::{any, error, fmt, marker, panic, ptr};

#[cfg(any(feature = "errs-notify", feature = "errs-notify-tokio"))]
use std::sync::atomic;

unsafe impl<T: Send + Sync> Send for SendSyncNonNull<T> {}
unsafe impl<T: Send + Sync> Sync for SendSyncNonNull<T> {}

impl<T: Send + Sync> SendSyncNonNull<T> {
    fn new(non_null_ptr: ptr::NonNull<T>) -> Self {
        Self {
            non_null_ptr,
            _phantom: marker::PhantomData,
        }
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

        let boxed = Box::new(ReasonAndSource::<R>::new(reason));
        let ptr = ptr::NonNull::from(Box::leak(boxed)).cast::<ReasonAndSource>();

        #[cfg(any(feature = "errs-notify", feature = "errs-notify-tokio"))]
        {
            let err_notified = Self {
                file: loc.file(),
                line: loc.line(),
                reason_and_source: SendSyncNonNull::new(ptr),
            };
            if let Err(e) = notify::notify_err(err_notified) {
                eprintln!("ERROR(errs): {e:?}");
            }

            Self {
                file: loc.file(),
                line: loc.line(),
                reason_and_source: SendSyncNonNull::new(ptr),
            }
        }
        #[cfg(not(any(feature = "errs-notify", feature = "errs-notify-tokio")))]
        {
            Self {
                file: loc.file(),
                line: loc.line(),
                reason_and_source: SendSyncNonNull::new(ptr),
            }
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

        let boxed = Box::new(ReasonAndSource::<R, E>::with_source(reason, source));
        let ptr = ptr::NonNull::from(Box::leak(boxed)).cast::<ReasonAndSource>();

        #[cfg(any(feature = "errs-notify", feature = "errs-notify-tokio"))]
        {
            let err_notified = Self {
                file: loc.file(),
                line: loc.line(),
                reason_and_source: SendSyncNonNull::new(ptr),
            };
            if let Err(e) = notify::notify_err(err_notified) {
                eprintln!("ERROR(errs): {e:?}");
            }

            Self {
                file: loc.file(),
                line: loc.line(),
                reason_and_source: SendSyncNonNull::new(ptr),
            }
        }
        #[cfg(not(any(feature = "errs-notify", feature = "errs-notify-tokio")))]
        {
            Self {
                file: loc.file(),
                line: loc.line(),
                reason_and_source: SendSyncNonNull::new(ptr),
            }
        }
    }

    /// Gets the name of the source file where the error occurred.
    #[inline]
    pub fn file(&self) -> &'static str {
        self.file
    }

    /// Gets the line number in the source file where the error occurred.
    #[inline]
    pub fn line(&self) -> u32 {
        self.line
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
        let ptr = self.reason_and_source.non_null_ptr.as_ptr();
        let is_fn = unsafe { (*ptr).is_fn };
        if is_fn(type_id) {
            let typed_ptr = ptr as *const ReasonAndSource<R>;
            Ok(unsafe { &((*typed_ptr).reason_and_source.0) })
        } else {
            Err(self)
        }
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
        let ptr = self.reason_and_source.non_null_ptr.as_ptr();
        let is_fn = unsafe { (*ptr).is_fn };
        if is_fn(type_id) {
            let typed_ptr = ptr as *const ReasonAndSource<R>;
            func(unsafe { &((*typed_ptr).reason_and_source.0) });
        }

        self
    }
}

impl Drop for Err {
    fn drop(&mut self) {
        let drop_fn = unsafe { (*self.reason_and_source.non_null_ptr.as_ptr()).drop_fn };
        drop_fn(self.reason_and_source.non_null_ptr);
    }
}

impl fmt::Debug for Err {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let debug_fn = unsafe { (*self.reason_and_source.non_null_ptr.as_ptr()).debug_fn };

        write!(f, "{} {{ ", any::type_name::<Err>())?;
        debug_fn(self.reason_and_source.non_null_ptr, f)?;
        write!(f, ", file = {}, line = {}", self.file, self.line)?;
        write!(f, " }}")
    }
}

impl fmt::Display for Err {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let display_fn = unsafe { (*self.reason_and_source.non_null_ptr.as_ptr()).display_fn };
        display_fn(self.reason_and_source.non_null_ptr, f)
    }
}

impl error::Error for Err {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        let source_fn = unsafe { (*self.reason_and_source.non_null_ptr.as_ptr()).source_fn };
        source_fn(self.reason_and_source.non_null_ptr)
    }
}

impl<R, E> ReasonAndSource<R, E>
where
    R: fmt::Debug + Send + Sync + 'static,
    E: error::Error + Send + Sync + 'static,
{
    fn new(reason: R) -> Self {
        Self {
            is_fn: is_reason::<R>,
            drop_fn: drop_reason_and_source::<R, E>,
            debug_fn: debug_reason_and_source::<R, E>,
            display_fn: display_reason_and_source::<R, E>,
            source_fn: get_source::<R, E>,
            #[cfg(any(feature = "errs-notify", feature = "errs-notify-tokio"))]
            is_referenced_by_another: atomic::AtomicBool::new(true),
            reason_and_source: (reason, None),
        }
    }

    fn with_source(reason: R, source: E) -> Self {
        Self {
            is_fn: is_reason::<R>,
            drop_fn: drop_reason_and_source::<R, E>,
            debug_fn: debug_reason_and_source::<R, E>,
            display_fn: display_reason_and_source::<R, E>,
            source_fn: get_source::<R, E>,
            #[cfg(any(feature = "errs-notify", feature = "errs-notify-tokio"))]
            is_referenced_by_another: atomic::AtomicBool::new(true),
            reason_and_source: (reason, Some(*Box::new(source))),
        }
    }
}

fn is_reason<R>(type_id: any::TypeId) -> bool
where
    R: fmt::Debug + Send + Sync + 'static,
{
    any::TypeId::of::<R>() == type_id
}

fn drop_reason_and_source<R, E>(ptr: ptr::NonNull<ReasonAndSource>)
where
    R: fmt::Debug + Send + Sync + 'static,
    E: error::Error + Send + Sync + 'static,
{
    let typed_ptr = ptr.cast::<ReasonAndSource<R, E>>().as_ptr();
    #[cfg(any(feature = "errs-notify", feature = "errs-notify-tokio"))]
    {
        let is_ref = unsafe { &(*typed_ptr).is_referenced_by_another };
        if !is_ref.fetch_and(false, atomic::Ordering::AcqRel) {
            unsafe { drop(Box::from_raw(typed_ptr)) };
        }
    }
    #[cfg(not(any(feature = "errs-notify", feature = "errs-notify-tokio")))]
    {
        unsafe { drop(Box::from_raw(typed_ptr)) };
    }
}

fn debug_reason_and_source<R, E>(
    ptr: ptr::NonNull<ReasonAndSource>,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result
where
    R: fmt::Debug + Send + Sync + 'static,
    E: error::Error + Send + Sync + 'static,
{
    let typed_ptr = ptr.cast::<ReasonAndSource<R, E>>().as_ptr();
    let reason_and_source = unsafe { &(*typed_ptr).reason_and_source };
    write!(
        f,
        "reason = {} {:?}",
        any::type_name::<R>(),
        reason_and_source.0
    )?;

    match &reason_and_source.1 {
        Some(src) => write!(f, ", source = {:?}", src),
        None => Ok(()),
    }
}

fn display_reason_and_source<R, E>(
    ptr: ptr::NonNull<ReasonAndSource>,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result
where
    R: fmt::Debug + Send + Sync + 'static,
    E: error::Error + Send + Sync + 'static,
{
    let typed_ptr = ptr.cast::<ReasonAndSource<R, E>>().as_ptr();
    write!(f, "{:?}", unsafe { &(*typed_ptr).reason_and_source.0 })
}

fn get_source<R, E>(
    ptr: ptr::NonNull<ReasonAndSource>,
) -> Option<&'static (dyn error::Error + 'static)>
where
    R: fmt::Debug + Send + Sync + 'static,
    E: error::Error + Send + Sync + 'static,
{
    let typed_ptr = ptr.cast::<ReasonAndSource<R, E>>().as_ptr();
    match unsafe { &(*typed_ptr).reason_and_source.1 } {
        Some(src) => Some(src),
        None => None,
    }
}

#[cfg(test)]
mod tests_of_err {
    use super::*;
    use std::sync::{LazyLock, Mutex};

    struct Logger {
        log_vec: Vec<String>,
    }

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
    }

    const BASE_LINE: u32 = line!();

    mod test_of_drop {
        use super::*;

        static LOGGER: LazyLock<Mutex<Logger>> = LazyLock::new(|| Mutex::new(Logger::new()));

        #[allow(dead_code)]
        #[derive(Debug)]
        enum Enum0 {
            InvalidValue { name: String, value: String },
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
                format!("errs::Err {{ reason = errs::err::tests_of_err::test_of_drop::Enum0 InvalidValue {{ name: \"foo\", value: \"abc\" }}, file = src/err.rs, line = {} }}", BASE_LINE + 19),
            );
            #[cfg(windows)]
            assert_eq!(
                format!("{err:?}"),
                format!("errs::Err {{ reason = errs::err::tests_of_err::test_of_drop::Enum0 InvalidValue {{ name: \"foo\", value: \"abc\" }}, file = src\\err.rs, line = {} }}", BASE_LINE + 19),
            );

            LOGGER.lock().unwrap().log("consumed Enum0");
        }

        #[test]
        fn test() {
            consume_err();

            std::thread::sleep(std::time::Duration::from_secs(1));

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
        use std::error::Error;

        #[derive(Debug)]
        enum Enum0 {
            InvalidValue { name: String, value: String },
        }

        #[test]
        fn new_err() {
            let err = Err::new(Enum0::InvalidValue {
                name: "foo".to_string(),
                value: "abc".to_string(),
            });

            #[cfg(unix)]
            assert_eq!(err.file(), "src/err.rs");
            #[cfg(windows)]
            assert_eq!(err.file(), "src\\err.rs");
            assert_eq!(err.line(), BASE_LINE + 75);
            assert_eq!(
                format!("{err}"),
                "InvalidValue { name: \"foo\", value: \"abc\" }",
            );
            #[cfg(unix)]
            assert_eq!(
                format!("{err:?}"),
                format!("errs::Err {{ reason = errs::err::tests_of_err::test_of_new::Enum0 InvalidValue {{ name: \"foo\", value: \"abc\" }}, file = src/err.rs, line = {} }}", BASE_LINE + 75),
            );
            #[cfg(windows)]
            assert_eq!(
                format!("{err:?}"),
                format!("errs::Err {{ reason = errs::err::tests_of_err::test_of_new::Enum0 InvalidValue {{ name: \"foo\", value: \"abc\" }}, file = src\\err.rs, line = {} }}", BASE_LINE + 75),
            );

            match err.reason::<Enum0>().unwrap() {
                Enum0::InvalidValue { name, value } => {
                    assert_eq!(name, "foo");
                    assert_eq!(value, "abc");
                }
            }
            assert!(err.source().is_none());
        }
    }

    mod test_of_with_source {
        use super::*;
        use std::error::Error;

        #[derive(Debug)]
        enum Enum0 {
            InvalidValue { name: String, value: String },
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
            assert_eq!(err.file, "src/err.rs");
            #[cfg(windows)]
            assert_eq!(err.file, "src\\err.rs");

            assert_eq!(err.line, BASE_LINE + 122);
            assert_eq!(
                format!("{err}"),
                "InvalidValue { name: \"foo\", value: \"abc\" }",
            );

            #[cfg(unix)]
            assert_eq!(
                format!("{err:?}"),
                format!("errs::Err {{ reason = errs::err::tests_of_err::test_of_with_source::Enum0 InvalidValue {{ name: \"foo\", value: \"abc\" }}, source = Custom {{ kind: NotFound, error: \"oh no!\" }}, file = src/err.rs, line = {} }}", BASE_LINE + 122),
            );
            #[cfg(windows)]
            assert_eq!(
                format!("{err:?}"),
                format!("errs::Err {{ reason = errs::err::tests_of_err::test_of_with_source::Enum0 InvalidValue {{ name: \"foo\", value: \"abc\" }}, source = Custom {{ kind: NotFound, error: \"oh no!\" }}, file = src\\err.rs, line = {} }}", BASE_LINE + 122),
            );

            match err.reason::<Enum0>().unwrap() {
                Enum0::InvalidValue { name, value } => {
                    assert_eq!(name, "foo");
                    assert_eq!(value, "abc");
                }
            }

            assert!(err.source().is_some());
            match err.source() {
                Some(e) => match e.downcast_ref::<std::io::Error>() {
                    Some(io_err) => {
                        assert_eq!((*io_err).kind(), std::io::ErrorKind::NotFound);
                    }
                    _ => unreachable!(),
                },
                None => unreachable!(),
            }
        }

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
        fn source_is_a_user_defined_error() {
            let source = MyError::new("hello");
            let err = Err::with_source(
                Enum0::InvalidValue {
                    name: "foo".to_string(),
                    value: "abc".to_string(),
                },
                source,
            );

            #[cfg(unix)]
            assert_eq!(err.file, "src/err.rs");
            #[cfg(windows)]
            assert_eq!(err.file, "src\\err.rs");
            assert_eq!(err.line, BASE_LINE + 192);
            assert_eq!(
                format!("{err}"),
                "InvalidValue { name: \"foo\", value: \"abc\" }",
            );
            #[cfg(unix)]
            assert_eq!(
                format!("{err:?}"),
                format!("errs::Err {{ reason = errs::err::tests_of_err::test_of_with_source::Enum0 InvalidValue {{ name: \"foo\", value: \"abc\" }}, source = MyError {{ message: \"hello\" }}, file = src/err.rs, line = {} }}", BASE_LINE + 192),
            );
            #[cfg(windows)]
            assert_eq!(
                format!("{err:?}"),
                format!("errs::Err {{ reason = errs::err::tests_of_err::test_of_with_source::Enum0 InvalidValue {{ name: \"foo\", value: \"abc\" }}, source = MyError {{ message: \"hello\" }}, file = src\\err.rs, line = {} }}", BASE_LINE + 192),
            );

            assert!(err.source().is_some());
            match err.source() {
                Some(e) => match e.downcast_ref::<MyError>() {
                    Some(my_err) => {
                        assert_eq!((*my_err).message, "hello".to_string());
                    }
                    _ => unreachable!(),
                },
                None => unreachable!(),
            }
        }
    }

    mod test_of_reason {
        use super::*;
        use std::error::Error;

        #[test]
        fn reason_is_a_boolean() {
            let err = Err::new(true);
            assert_eq!(format!("{err}"), "true");
            #[cfg(unix)]
            assert_eq!(
                format!("{err:?}"),
                format!(
                    "errs::Err {{ reason = bool true, file = src/err.rs, line = {} }}",
                    BASE_LINE + 239,
                ),
            );
            #[cfg(windows)]
            assert_eq!(
                format!("{err:?}"),
                format!(
                    "errs::Err {{ reason = bool true, file = src\\err.rs, line = {} }}",
                    BASE_LINE + 239,
                ),
            );

            match err.reason() {
                Ok(true) => {}
                Ok(false) => unreachable!(),
                Err(_) => unreachable!(),
            }

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
                    "errs::Err {{ reason = i64 123, file = src/err.rs, line = {} }}",
                    BASE_LINE + 269,
                ),
            );
            #[cfg(windows)]
            assert_eq!(
                format!("{err:?}"),
                format!(
                    "errs::Err {{ reason = i64 123, file = src\\err.rs, line = {} }}",
                    BASE_LINE + 269,
                ),
            );

            match err.reason::<i64>() {
                Ok(n) => assert_eq!(*n, 123i64),
                Err(_) => unreachable!(),
            }

            assert!(err.source().is_none());
        }

        #[test]
        fn reason_is_a_string() {
            let err = Err::new("abc".to_string());
            assert_eq!(format!("{err}"), "\"abc\"");
            #[cfg(unix)]
            assert_eq!(
                format!("{err:?}"),
                format!(
                    "errs::Err {{ reason = alloc::string::String \"abc\", file = src/err.rs, line = {} }}",
                    BASE_LINE + 298,
                ),
            );
            #[cfg(windows)]
            assert_eq!(
                format!("{err:?}"),
                format!(
                    "errs::Err {{ reason = alloc::string::String \"abc\", file = src\\err.rs, line = {} }}",
                    BASE_LINE + 298,
                ),
            );

            match err.reason::<String>() {
                Ok(s) => assert_eq!(s, "abc"),
                Err(_) => unreachable!(),
            }

            assert!(err.source().is_none());
        }

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
                format!("errs::Err {{ reason = errs::err::tests_of_err::test_of_reason::StructA StructA {{ name: \"abc\", value: 123 }}, file = src/err.rs, line = {} }}", BASE_LINE + 333),
            );
            #[cfg(windows)]
            assert_eq!(
                format!("{err:?}"),
                format!("errs::Err {{ reason = errs::err::tests_of_err::test_of_reason::StructA StructA {{ name: \"abc\", value: 123 }}, file = src\\err.rs, line = {} }}", BASE_LINE + 333),
            );

            match err.reason::<StructA>() {
                Ok(st) => {
                    assert_eq!(st.name, "abc".to_string());
                    assert_eq!(st.value, 123);
                }
                Err(_) => unreachable!(),
            }

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
                    "errs::Err {{ reason = () (), file = src/err.rs, line = {} }}",
                    BASE_LINE + 362,
                ),
            );
            #[cfg(windows)]
            assert_eq!(
                format!("{err:?}"),
                format!(
                    "errs::Err {{ reason = () (), file = src\\err.rs, line = {} }}",
                    BASE_LINE + 362,
                ),
            );

            match err.reason::<()>() {
                Ok(()) => {}
                Err(_) => unreachable!(),
            }

            assert!(err.source().is_none());
        }
    }

    mod test_of_match_reason {
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
