#[cfg(test)]
mod tests_of_notification {
    use std::{
        sync::{LazyLock, Mutex},
        time,
    };

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
    #[cfg(feature = "notify-tokio")]
    errs::add_tokio_async_err_handler!(async |err, _dttm| {
        LOGGER.lock().unwrap().1.push(format!("[tokio] {err:?}"));
    });

    #[derive(Debug)]
    enum Reasons {
        FailToDoSomething,
    }

    #[cfg(any(feature = "notify", feature = "notify-tokio"))]
    const BASE_LINE: u32 = line!();

    #[test]
    fn test() {
        let _err = errs::Err::new(Reasons::FailToDoSomething);

        #[cfg(feature = "notify")]
        {
            let logs = &LOGGER.lock().unwrap().0;
            assert_eq!(logs.len(), 1);
            #[cfg(unix)]
            {
                assert_eq!(logs[0], format!("[sync] errs::Err {{ reason = global_handler_registeration_on_std_rt_test::tests_of_notification::Reasons FailToDoSomething, file = tests/global_handler_registeration_on_std_rt_test.rs, line = {} }}", BASE_LINE + 4));
            }
            #[cfg(windows)]
            {
                assert_eq!(logs[0], format!("[sync] errs::Err {{ reason = global_handler_registeration_on_std_rt_test::tests_of_notification::Reasons FailToDoSomething, file = tests\\global_handler_registeration_on_std_rt_test.rs, line = {} }}", BASE_LINE + 4));
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

        std::thread::sleep(time::Duration::from_millis(100));

        #[cfg(feature = "notify")]
        {
            let logs = &LOGGER.lock().unwrap().0;
            assert_eq!(logs.len(), 2);
            #[cfg(unix)]
            {
                assert_eq!(logs[0], format!("[sync] errs::Err {{ reason = global_handler_registeration_on_std_rt_test::tests_of_notification::Reasons FailToDoSomething, file = tests/global_handler_registeration_on_std_rt_test.rs, line = {} }}", BASE_LINE + 4));
                assert_eq!(logs[1], format!("[async] errs::Err {{ reason = global_handler_registeration_on_std_rt_test::tests_of_notification::Reasons FailToDoSomething, file = tests/global_handler_registeration_on_std_rt_test.rs, line = {} }}", BASE_LINE + 4));
            }
            #[cfg(windows)]
            {
                assert_eq!(logs[0], format!("[sync] errs::Err {{ reason = global_handler_registeration_on_std_rt_test::tests_of_notification::Reasons FailToDoSomething, file = tests\\global_handler_registeration_on_std_rt_test.rs, line = {} }}", BASE_LINE + 4));
                assert_eq!(logs[1], format!("[async] errs::Err {{ reason = global_handler_registeration_on_std_rt_test::tests_of_notification::Reasons FailToDoSomething, file = tests\\global_handler_registeration_on_std_rt_test.rs, line = {} }}", BASE_LINE + 4));
            }
        }
        #[cfg(not(feature = "notify"))]
        {
            let logs = &LOGGER.lock().unwrap().0;
            assert_eq!(logs.len(), 0);
        }

        #[cfg(feature = "notify-tokio")]
        {
            let logs = &LOGGER.lock().unwrap().1;
            assert_eq!(logs.len(), 1);
            #[cfg(unix)]
            {
                assert_eq!(logs[0], format!("[tokio] errs::Err {{ reason = global_handler_registeration_on_std_rt_test::tests_of_notification::Reasons FailToDoSomething, file = tests/global_handler_registeration_on_std_rt_test.rs, line = {} }}", BASE_LINE + 4));
            }
            #[cfg(windows)]
            {
                assert_eq!(logs[0], format!("[tokio] errs::Err {{ reason = global_handler_registeration_on_std_rt_test::tests_of_notification::Reasons FailToDoSomething, file = tests\\global_handler_registeration_on_std_rt_test.rs, line = {} }}", BASE_LINE + 4));
            }
        }
        #[cfg(not(feature = "notify-tokio"))]
        {
            let logs = &LOGGER.lock().unwrap().1;
            assert_eq!(logs.len(), 0);
        }
    }
}
