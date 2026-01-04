#[cfg(feature = "notify-tokio")]
#[cfg(test)]
mod tests_of_notification {
    use std::sync::{LazyLock, Mutex};

    static LOGGER: LazyLock<Mutex<(Vec<String>, Vec<String>)>> =
        LazyLock::new(|| Mutex::new((Vec::new(), Vec::new())));

    #[cfg(feature = "notify")]
    errs::add_sync_err_handler!(|err, _dttm| {
        LOGGER.lock().unwrap().0.push(format!("[sync] {err:?}"));
    });
    #[cfg(feature = "notify")]
    errs::add_async_err_handler!(|err, _dttm| {
        LOGGER.lock().unwrap().0.push(format!("[async] {err:?}"));
    });
    errs::add_tokio_async_err_handler!(async |err, _| {
        LOGGER.lock().unwrap().1.push(format!("[tokio] {err:?}"));
    });

    #[derive(Debug)]
    enum Reasons {
        FailToDoSomething,
    }

    const BASE_LINE: u32 = line!();

    #[tokio::test]
    async fn test() {
        let _err = errs::Err::new(Reasons::FailToDoSomething);

        #[cfg(feature = "notify")]
        {
            let logs = &LOGGER.lock().unwrap().0;
            assert_eq!(logs.len(), 1);
            #[cfg(unix)]
            {
                assert_eq!(logs[0], format!("[sync] errs::Err {{ reason = global_handler_registeration_on_tokio_rt_test::tests_of_notification::Reasons FailToDoSomething, file = tests/global_handler_registeration_on_tokio_rt_test.rs, line = {} }}", BASE_LINE + 4));
            }
            #[cfg(windows)]
            {
                assert_eq!(logs[0], format!("[sync] errs::Err {{ reason = global_handler_registeration_on_tokio_rt_test::tests_of_notification::Reasons FailToDoSomething, file = tests\\global_handler_registeration_on_tokio_rt_test.rs, line = {} }}", BASE_LINE + 4));
            }
        }
        #[cfg(not(feature = "notify"))]
        {
            let logs = &LOGGER.lock().unwrap().0;
            assert_eq!(logs.len(), 0);
        }

        {
            let logs = &LOGGER.lock().unwrap().1;
            assert_eq!(logs.len(), 0);
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        #[cfg(feature = "notify")]
        {
            let logs = &LOGGER.lock().unwrap().0;
            assert_eq!(logs.len(), 2);
            #[cfg(unix)]
            {
                assert_eq!(logs[0], format!("[sync] errs::Err {{ reason = global_handler_registeration_on_tokio_rt_test::tests_of_notification::Reasons FailToDoSomething, file = tests/global_handler_registeration_on_tokio_rt_test.rs, line = {} }}", BASE_LINE + 4));
                assert_eq!(logs[1], format!("[async] errs::Err {{ reason = global_handler_registeration_on_tokio_rt_test::tests_of_notification::Reasons FailToDoSomething, file = tests/global_handler_registeration_on_tokio_rt_test.rs, line = {} }}", BASE_LINE + 4));
            }
            #[cfg(windows)]
            {
                assert_eq!(logs[0], format!("[sync] errs::Err {{ reason = global_handler_registeration_on_tokio_rt_test::tests_of_notification::Reasons FailToDoSomething, file = tests\\global_handler_registeration_on_tokio_rt_test.rs, line = {} }}", BASE_LINE + 4));
                assert_eq!(logs[1], format!("[async] errs::Err {{ reason = global_handler_registeration_on_tokio_rt_test::tests_of_notification::Reasons FailToDoSomething, file = tests\\global_handler_registeration_on_tokio_rt_test.rs, line = {} }}", BASE_LINE + 4));
            }
        }
        #[cfg(not(feature = "notify"))]
        {
            let logs = &LOGGER.lock().unwrap().0;
            assert_eq!(logs.len(), 0);
        }

        {
            let logs = &LOGGER.lock().unwrap().1;
            assert_eq!(logs.len(), 1);
            #[cfg(unix)]
            {
                assert_eq!(logs[0], format!("[tokio] errs::Err {{ reason = global_handler_registeration_on_tokio_rt_test::tests_of_notification::Reasons FailToDoSomething, file = tests/global_handler_registeration_on_tokio_rt_test.rs, line = {} }}", BASE_LINE + 4));
            }
            #[cfg(windows)]
            {
                assert_eq!(logs[0], format!("[tokio] errs::Err {{ reason = global_handler_registeration_on_tokio_rt_test::tests_of_notification::Reasons FailToDoSomething, file = tests\\global_handler_registeration_on_tokio_rt_test.rs, line = {} }}", BASE_LINE + 4));
            }
        }
    }
}
