// Copyright (C) 2025 Takayuki Sato. All Rights Reserved.
// This program is free software under MIT License.
// See the file LICENSE in this distribution for more details.

use super::{ErrHandlingError, ErrHandlingErrorKind};
use crate::Err;

use chrono::{DateTime, Utc};
use setup_read_cleanup::{PhasedCellSync, PhasedErrorKind};

use std::{sync, thread};

type BoxedFn = Box<dyn Fn(&Err, DateTime<Utc>) + Send + Sync + 'static>;

#[allow(clippy::type_complexity)]
const NOOP: fn(&mut (Vec<BoxedFn>, Vec<BoxedFn>)) -> Result<(), ErrHandlingError> = |_| Ok(());

pub(crate) static HANDLERS: PhasedCellSync<(Vec<BoxedFn>, Vec<BoxedFn>)> =
    PhasedCellSync::new((Vec::new(), Vec::new()));

#[inline]
pub(crate) fn add_sync_handler<F>(
    handlers: &PhasedCellSync<(Vec<BoxedFn>, Vec<BoxedFn>)>,
    handler: F,
) -> Result<(), ErrHandlingError>
where
    F: Fn(&Err, DateTime<Utc>) + Send + Sync + 'static,
{
    match handlers.lock() {
        Ok(mut vv) => {
            vv.0.push(Box::new(handler));
            Ok(())
        }
        Err(e) => match e.kind() {
            PhasedErrorKind::InternalDataUnavailable => Err(ErrHandlingError::new(
                ErrHandlingErrorKind::InvalidInternalState,
            )),
            PhasedErrorKind::StdMutexIsPoisoned => Err(ErrHandlingError::new(
                ErrHandlingErrorKind::StdMutexIsPoisoned,
            )),
            _ => Err(ErrHandlingError::new(
                ErrHandlingErrorKind::InvalidCallTiming,
            )),
        },
    }
}

pub(crate) fn add_async_handler<F>(
    handlers: &PhasedCellSync<(Vec<BoxedFn>, Vec<BoxedFn>)>,
    handler: F,
) -> Result<(), ErrHandlingError>
where
    F: Fn(&Err, DateTime<Utc>) + Send + Sync + 'static,
{
    match handlers.lock() {
        Ok(mut vv) => {
            vv.1.push(Box::new(handler));
            Ok(())
        }
        Err(e) => match e.kind() {
            PhasedErrorKind::InternalDataUnavailable => Err(ErrHandlingError::new(
                ErrHandlingErrorKind::InvalidInternalState,
            )),
            PhasedErrorKind::StdMutexIsPoisoned => Err(ErrHandlingError::new(
                ErrHandlingErrorKind::StdMutexIsPoisoned,
            )),
            _ => Err(ErrHandlingError::new(
                ErrHandlingErrorKind::InvalidCallTiming,
            )),
        },
    }
}

#[inline]
pub(crate) fn fix_handlers(
    handlers: &PhasedCellSync<(Vec<BoxedFn>, Vec<BoxedFn>)>,
) -> Result<(), ErrHandlingError> {
    if let Err(e) = handlers.transition_to_read(NOOP) {
        match e.kind() {
            PhasedErrorKind::PhaseIsAlreadyRead => Ok(()),
            PhasedErrorKind::InternalDataUnavailable => Err(ErrHandlingError::new(
                ErrHandlingErrorKind::InvalidInternalState,
            )),
            PhasedErrorKind::StdMutexIsPoisoned => Err(ErrHandlingError::new(
                ErrHandlingErrorKind::StdMutexIsPoisoned,
            )),
            // PhasedErrorKind::FailToRunClosureDuringTransitionToRead => {}, // impossible case
            _ => Err(ErrHandlingError::new(
                ErrHandlingErrorKind::InvalidCallTiming,
            )),
        }
    } else {
        Ok(())
    }
}

pub(crate) fn handle_err(
    handlers: &'static PhasedCellSync<(Vec<BoxedFn>, Vec<BoxedFn>)>,
    err: Err,
    tm: DateTime<Utc>,
) -> Result<(), ErrHandlingError> {
    let result = match handlers.transition_to_read(NOOP) {
        Ok(_) => handlers.read_relaxed(),
        Err(e) => match e.kind() {
            PhasedErrorKind::PhaseIsAlreadyRead => handlers.read_relaxed(),
            PhasedErrorKind::DuringTransitionToRead => handlers.read_ready(),
            PhasedErrorKind::InternalDataUnavailable => {
                return Err(ErrHandlingError::new(
                    ErrHandlingErrorKind::InvalidInternalState,
                ));
            }
            PhasedErrorKind::StdMutexIsPoisoned => {
                return Err(ErrHandlingError::new(
                    ErrHandlingErrorKind::StdMutexIsPoisoned,
                ));
            }
            // PhasedErrorKind::FailToRunClosureDuringTransitionToRead => {}, // impossible case
            _ => {
                return Err(ErrHandlingError::new(
                    ErrHandlingErrorKind::InvalidCallTiming,
                ));
            }
        },
    };

    match result {
        Ok(vv) => {
            let err = sync::Arc::new(err);
            for handle in vv.1.iter() {
                let err1 = sync::Arc::clone(&err);
                thread::spawn(move || handle(&err1, tm));
            }

            for handle in vv.0.iter() {
                handle(&err, tm);
            }

            Ok(())
        }
        Err(e) => match e.kind() {
            PhasedErrorKind::InternalDataUnavailable => Err(ErrHandlingError::new(
                ErrHandlingErrorKind::InvalidInternalState,
            )),
            PhasedErrorKind::StdMutexIsPoisoned => Err(ErrHandlingError::new(
                ErrHandlingErrorKind::StdMutexIsPoisoned,
            )),
            _ => Err(ErrHandlingError::new(
                ErrHandlingErrorKind::InvalidCallTiming,
            )),
        },
    }
}

#[cfg(test)]
mod tests_of_notify {
    use super::*;

    #[derive(Debug)]
    enum Errors {
        FailToDoSomething,
    }

    mod tests_of_sync_err_handling {
        use super::*;
        use std::sync::{LazyLock, Mutex};

        static HANDLERS: PhasedCellSync<(Vec<BoxedFn>, Vec<BoxedFn>)> =
            PhasedCellSync::new((Vec::new(), Vec::new()));

        static LOGGERS: LazyLock<Mutex<Vec<String>>> = LazyLock::new(|| Mutex::new(Vec::new()));

        const LINE: u32 = line!();

        #[test]
        fn add_and_fix_and_notify() {
            assert!(add_sync_handler(&HANDLERS, |err, _tm| {
                LOGGERS.lock().unwrap().push(format!("1: err={err:?}"));
            })
            .is_ok());
            assert!(add_sync_handler(&HANDLERS, |err, _tm| {
                LOGGERS.lock().unwrap().push(format!("2: err={err:?}"));
            })
            .is_ok());

            assert!(fix_handlers(&HANDLERS).is_ok());

            assert!(add_sync_handler(&HANDLERS, |err, _tm| {
                LOGGERS.lock().unwrap().push(format!("3: err={err:?}"));
            })
            .is_err());

            let err = Err::new(Errors::FailToDoSomething);
            assert!(handle_err(&HANDLERS, err, Utc::now()).is_ok());

            #[cfg(unix)]
            {
                let vec = LOGGERS.lock().unwrap();
                assert_eq!(vec.len(), 2);
                assert_eq!(vec[0], format!("1: err=errs::Err {{ reason = errs::notify::blocking::tests_of_notify::Errors FailToDoSomething, file = src/notify/blocking.rs, line = {} }}", LINE + 20));
                assert_eq!(vec[1], format!("2: err=errs::Err {{ reason = errs::notify::blocking::tests_of_notify::Errors FailToDoSomething, file = src/notify/blocking.rs, line = {} }}", LINE + 20));
            }
            #[cfg(windows)]
            {
                let vec = LOGGERS.lock().unwrap();
                assert_eq!(vec.len(), 2);
                assert_eq!(vec[0], format!("1: err=errs::Err {{ reason = errs::notify::blocking::tests_of_notify::Errors FailToDoSomething, file = src\\notify\\blocking.rs, line = {} }}", LINE + 20));
                assert_eq!(vec[1], format!("2: err=errs::Err {{ reason = errs::notify::blocking::tests_of_notify::Errors FailToDoSomething, file = src\\notify\\blocking.rs, line = {} }}", LINE + 20));
            }
        }
    }

    mod tests_of_async_err_handling {
        use super::*;
        use std::sync::{LazyLock, Mutex};

        static HANDLERS: PhasedCellSync<(Vec<BoxedFn>, Vec<BoxedFn>)> =
            PhasedCellSync::new((Vec::new(), Vec::new()));

        static LOGGERS: LazyLock<Mutex<Vec<String>>> = LazyLock::new(|| Mutex::new(Vec::new()));

        const LINE: u32 = line!();

        #[test]
        fn add_and_fix_and_notify() {
            assert!(add_async_handler(&HANDLERS, |err, _tm| {
                thread::sleep(std::time::Duration::from_millis(20));
                LOGGERS.lock().unwrap().push(format!("1: err={err:?}"));
            })
            .is_ok());
            assert!(add_async_handler(&HANDLERS, |err, _tm| {
                thread::sleep(std::time::Duration::from_millis(10));
                LOGGERS.lock().unwrap().push(format!("2: err={err:?}"));
            })
            .is_ok());

            assert!(fix_handlers(&HANDLERS).is_ok());

            assert!(add_async_handler(&HANDLERS, |err, _tm| {
                thread::sleep(std::time::Duration::from_millis(10));
                LOGGERS.lock().unwrap().push(format!("3: err={err:?}"));
            })
            .is_err());

            let err = Err::new(Errors::FailToDoSomething);
            assert!(handle_err(&HANDLERS, err, Utc::now()).is_ok());

            {
                let vec = LOGGERS.lock().unwrap();
                assert_eq!(vec.len(), 0);
            }

            thread::sleep(std::time::Duration::from_millis(100));

            #[cfg(unix)]
            {
                let vec = LOGGERS.lock().unwrap();
                assert_eq!(vec.len(), 2);
                assert_eq!(vec[0], format!("2: err=errs::Err {{ reason = errs::notify::blocking::tests_of_notify::Errors FailToDoSomething, file = src/notify/blocking.rs, line = {} }}", LINE + 23));
                assert_eq!(vec[1], format!("1: err=errs::Err {{ reason = errs::notify::blocking::tests_of_notify::Errors FailToDoSomething, file = src/notify/blocking.rs, line = {} }}", LINE + 23));
            }
            #[cfg(windows)]
            {
                let vec = LOGGERS.lock().unwrap();
                assert_eq!(vec.len(), 2);
                assert_eq!(vec[0], format!("2: err=errs::Err {{ reason = errs::notify::blocking::tests_of_notify::Errors FailToDoSomething, file = src\\notify\\blocking.rs, line = {} }}", LINE + 23));
                assert_eq!(vec[1], format!("1: err=errs::Err {{ reason = errs::notify::blocking::tests_of_notify::Errors FailToDoSomething, file = src\\notify\\blocking.rs, line = {} }}", LINE + 23));
            }
        }
    }
}
