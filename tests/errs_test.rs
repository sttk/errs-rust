#[cfg(test)]
mod integration_tests_of_err {
    use errs;
    use std::error::Error;

    #[derive(Debug)]
    enum IoErrs {
        FileNotFound { path: String },
        NoPermission { path: String, r#mod: (u8, u8, u8) },
        DueToSomeError { path: String },
    }

    fn find_file() -> Result<(), errs::Err> {
        let err = errs::Err::new(IoErrs::FileNotFound {
            path: "/aaa/bbb/ccc".to_string(),
        });
        Err(err)
    }

    fn read_file() -> Result<(), errs::Err> {
        let err = errs::Err::new(IoErrs::NoPermission {
            path: "/aaa/bbb/ccc".to_string(),
            r#mod: (4, 4, 4),
        });
        Err(err)
    }

    fn write_file() -> Result<(), errs::Err> {
        let path = "/aaa/bbb/ccc".to_string();
        let source = std::io::Error::new(std::io::ErrorKind::AlreadyExists, path.clone());
        let err = errs::Err::with_source(IoErrs::DueToSomeError { path }, source);
        Err(err)
    }

    #[test]
    fn should_create_err_and_identify_reason() {
        match find_file() {
            Ok(_) => panic!(),
            Err(err) => match err.reason::<IoErrs>() {
                Ok(r) => match r {
                    IoErrs::FileNotFound { path } => {
                        assert_eq!(path, "/aaa/bbb/ccc");
                        assert!(err.source().is_none());
                    }
                    IoErrs::NoPermission { path: _, r#mod: _ } => panic!(),
                    IoErrs::DueToSomeError { path: _ } => panic!(),
                },
                _ => panic!(),
            },
        }

        match read_file() {
            Ok(_) => panic!(),
            Err(err) => match err.reason::<IoErrs>() {
                Ok(r) => match r {
                    IoErrs::FileNotFound { path: _ } => panic!(),
                    IoErrs::NoPermission { path, r#mod } => {
                        assert_eq!(path, "/aaa/bbb/ccc");
                        assert_eq!(*r#mod, (4, 4, 4));
                        assert!(err.source().is_none());
                    }
                    IoErrs::DueToSomeError { path: _ } => panic!(),
                },
                _ => panic!(),
            },
        }

        match write_file() {
            Ok(_) => panic!(),
            Err(err) => match err.reason::<IoErrs>() {
                Ok(r) => match r {
                    IoErrs::FileNotFound { path: _ } => panic!(),
                    IoErrs::NoPermission { path: _, r#mod: _ } => panic!(),
                    IoErrs::DueToSomeError { path } => {
                        assert_eq!(path, "/aaa/bbb/ccc");
                        let source = err.source().unwrap();
                        let io_err = source.downcast_ref::<std::io::Error>().unwrap();
                        assert_eq!(io_err.kind(), std::io::ErrorKind::AlreadyExists);
                        assert_eq!(io_err.to_string(), "/aaa/bbb/ccc");
                    }
                },
                _ => panic!(),
            },
        }
    }

    #[test]
    fn should_match_reason() {
        match find_file() {
            Ok(_) => panic!(),
            Err(err) => err.match_reason::<IoErrs>(|r| match r {
                IoErrs::FileNotFound { path } => {
                    assert_eq!(path, "/aaa/bbb/ccc");
                }
                IoErrs::NoPermission { path: _, r#mod: _ } => panic!(),
                IoErrs::DueToSomeError { path: _ } => panic!(),
            }),
        };

        match read_file() {
            Ok(_) => panic!(),
            Err(err) => err.match_reason::<IoErrs>(|r| match r {
                IoErrs::FileNotFound { path: _ } => panic!(),
                IoErrs::NoPermission { path, r#mod } => {
                    assert_eq!(path, "/aaa/bbb/ccc");
                    assert_eq!(*r#mod, (4, 4, 4));
                }
                IoErrs::DueToSomeError { path: _ } => panic!(),
            }),
        };

        match write_file() {
            Ok(_) => panic!(),
            Err(err) => err.match_reason::<IoErrs>(|r| match r {
                IoErrs::FileNotFound { path: _ } => panic!(),
                IoErrs::NoPermission { path: _, r#mod: _ } => panic!(),
                IoErrs::DueToSomeError { path } => {
                    assert_eq!(path, "/aaa/bbb/ccc");
                }
            }),
        };
    }

    #[test]
    fn should_output_err_in_debug_format() {
        let err = find_file().unwrap_err();
        //println!("{err:?}");
        #[cfg(unix)]
        assert_eq!(
            format!("{err:?}"),
            "errs::Err { reason = errs_test::integration_tests_of_err::IoErrs FileNotFound { path: \"/aaa/bbb/ccc\" }, file = tests/errs_test.rs, line = 14 }"
        );
        #[cfg(windows)]
        assert_eq!(
            format!("{err:?}"),
            "errs::Err { reason = errs_test::integration_tests_of_err::IoErrs FileNotFound { path: \"/aaa/bbb/ccc\" }, file = tests\\errs_test.rs, line = 14 }"
        );

        let err = read_file().unwrap_err();
        //println!("{err:?}");
        #[cfg(unix)]
        assert_eq!(
            format!("{err:?}"),
            "errs::Err { reason = errs_test::integration_tests_of_err::IoErrs NoPermission { path: \"/aaa/bbb/ccc\", mod: (4, 4, 4) }, file = tests/errs_test.rs, line = 21 }"
        );
        #[cfg(windows)]
        assert_eq!(
            format!("{err:?}"),
            "errs::Err { reason = errs_test::integration_tests_of_err::IoErrs NoPermission { path: \"/aaa/bbb/ccc\", mod: (4, 4, 4) }, file = tests\\errs_test.rs, line = 21 }"
        );

        let err = write_file().unwrap_err();
        //println!("{err:?}");
        #[cfg(unix)]
        assert_eq!(
            format!("{err:?}"),
            "errs::Err { reason = errs_test::integration_tests_of_err::IoErrs DueToSomeError { path: \"/aaa/bbb/ccc\" }, file = tests/errs_test.rs, line = 31, source = Custom { kind: AlreadyExists, error: \"/aaa/bbb/ccc\" } }"
        );
        #[cfg(windows)]
        assert_eq!(
            format!("{err:?}"),
            "errs::Err { reason = errs_test::integration_tests_of_err::IoErrs DueToSomeError { path: \"/aaa/bbb/ccc\" }, file = tests\\errs_test.rs, line = 31, source = Custom { kind: AlreadyExists, error: \"/aaa/bbb/ccc\" } }"
        );
    }

    #[test]
    fn should_output_err_in_display_format() {
        let err = find_file().unwrap_err();
        //println!("{err}");
        assert_eq!(format!("{err}"), "FileNotFound { path: \"/aaa/bbb/ccc\" }");

        let err = read_file().unwrap_err();
        //println!("{err}");
        assert_eq!(
            format!("{err}"),
            "NoPermission { path: \"/aaa/bbb/ccc\", mod: (4, 4, 4) }"
        );

        let err = write_file().unwrap_err();
        //println!("{err}");
        assert_eq!(
            format!("{err}"),
            "DueToSomeError { path: \"/aaa/bbb/ccc\" }"
        );
    }

    #[test]
    fn should_get_source_of_err() {
        let err = find_file().unwrap_err();
        assert!(err.source().is_none());

        let err = read_file().unwrap_err();
        assert!(err.source().is_none());

        let err = write_file().unwrap_err();
        if let Some(source) = err.source() {
            let io_err = source.downcast_ref::<std::io::Error>().unwrap();
            assert_eq!(io_err.kind(), std::io::ErrorKind::AlreadyExists);
            assert_eq!(io_err.to_string(), "/aaa/bbb/ccc");
        } else {
            panic!();
        }
    }
}

#[test]
fn compile_error_check() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/compile_errors/*_errs.rs");
}
