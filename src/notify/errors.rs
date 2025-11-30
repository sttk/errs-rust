// Copyright (C) 2025 Takayuki Sato. All Rights Reserved.
// This program is free software under MIT License.
// See the file LICENSE in this distribution for more details.

use super::{ErrHandlingError, ErrHandlingErrorKind};

use std::{error, fmt};

impl ErrHandlingError {
    pub(crate) fn new(kind: ErrHandlingErrorKind) -> Self {
        Self { kind }
    }

    /// Returns the kind of this error.
    ///
    /// This method allows you to identify the specific type of error that occurred
    /// during error handling operations.
    pub fn kind(&self) -> ErrHandlingErrorKind {
        self.kind
    }
}

impl fmt::Display for ErrHandlingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.kind)
    }
}

impl error::Error for ErrHandlingError {}

#[cfg(test)]
mod tests_of_err_handling_error {
    use super::*;

    #[test]
    fn test_new() {
        let e = ErrHandlingError::new(ErrHandlingErrorKind::InvalidInternalState);
        assert_eq!(e.kind(), ErrHandlingErrorKind::InvalidInternalState);
    }

    #[test]
    fn test_debug() {
        let e = ErrHandlingError::new(ErrHandlingErrorKind::InvalidCallTiming);
        assert_eq!(e.kind(), ErrHandlingErrorKind::InvalidCallTiming);
        assert_eq!(
            format!("{e:?}"),
            "ErrHandlingError { kind: InvalidCallTiming }"
        );
    }

    #[test]
    fn test_display() {
        let e = ErrHandlingError::new(ErrHandlingErrorKind::InvalidCallTiming);
        assert_eq!(e.kind(), ErrHandlingErrorKind::InvalidCallTiming);
        assert_eq!(format!("{e}"), "InvalidCallTiming");
    }
}
