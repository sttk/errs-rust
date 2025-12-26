#[cfg(test)]
mod tests_of_notification {
    use std::sync::{LazyLock, Mutex};

    static LOGGER: LazyLock<Mutex<(Vec<String>, Vec<String>)>> =
        LazyLock::new(|| Mutex::new((Vec::new(), Vec::new())));

    #[cfg(feature = "errs-notify")]
    errs::add_sync_err_handler!(|err, _tm| {
        LOGGER
            .lock()
            .unwrap()
            .0
            .push(format!("[global sync] {err:?}"));
    });

    #[derive(Debug)]
    enum Reasons {
        FailToDoSomething,
    }

    #[cfg(feature = "errs-notify")]
    const BASE_LINE: u32 = line!();

    #[test]
    fn test() {
        #[cfg(feature = "errs-notify")]
        let _ = errs::add_sync_err_handler(|err, _| {
            LOGGER
                .lock()
                .unwrap()
                .0
                .push(format!("[local sync] {err:?}"));
        });

        let _err = errs::Err::new(Reasons::FailToDoSomething);

        #[cfg(feature = "errs-notify")]
        {
            let logs = &LOGGER.lock().unwrap().0;
            assert_eq!(logs.len(), 2);
            #[cfg(unix)]
            {
                assert_eq!(logs[0], format!("[global sync] errs::Err {{ reason = sync_handler_registration_order_test::tests_of_notification::Reasons FailToDoSomething, file = tests/sync_handler_registration_order_test.rs, line = {} }}", BASE_LINE + 13));
                assert_eq!(logs[1], format!("[local sync] errs::Err {{ reason = sync_handler_registration_order_test::tests_of_notification::Reasons FailToDoSomething, file = tests/sync_handler_registration_order_test.rs, line = {} }}", BASE_LINE + 13));
            }
            #[cfg(windows)]
            {
                assert_eq!(logs[0], format!("[global sync] errs::Err {{ reason = sync_handler_registration_order_test::tests_of_notification::Reasons FailToDoSomething, file = tests\\sync_handler_registration_order_test.rs, line = {} }}", BASE_LINE + 13));
                assert_eq!(logs[1], format!("[local sync] errs::Err {{ reason = sync_handler_registration_order_test::tests_of_notification::Reasons FailToDoSomething, file = tests\\sync_handler_registration_order_test.rs, line = {} }}", BASE_LINE + 13));
            }
        }
        #[cfg(not(feature = "errs-notify"))]
        {
            let logs = &LOGGER.lock().unwrap().0;
            assert_eq!(logs.len(), 0);
        }
    }
}
