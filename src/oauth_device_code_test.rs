//! Test-for-test port of upstream `test/oauth-device-code.test.ts`
//! (`@earendil-works/pi-ai` v0.80.2).
//!
//! Generic device-code polling loop: polls immediately, then every
//! `interval_seconds` until complete, and aborts an in-flight wait with
//! "Login cancelled". Uses tokio's paused clock as the deterministic analogue of
//! upstream's `vi.useFakeTimers` / `advanceTimersByTimeAsync`.

#[cfg(test)]
mod tests {
    use crate::oauth::{poll_oauth_device_code_flow, DevicePollOutcome};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    #[tokio::test(start_paused = true)]
    async fn polls_immediately_and_returns_the_completed_value() {
        let polls = Arc::new(AtomicUsize::new(0));
        let p = polls.clone();
        let poll = move || {
            let p = p.clone();
            async move {
                let n = p.fetch_add(1, Ordering::SeqCst) + 1;
                if n == 1 { DevicePollOutcome::Pending } else { DevicePollOutcome::Complete("token") }
            }
        };
        let handle = tokio::spawn(poll_oauth_device_code_flow(2, 30, false, poll, std::future::pending::<()>()));

        // Immediate poll runs at t=0.
        tokio::task::yield_now().await;
        assert_eq!(polls.load(Ordering::SeqCst), 1, "polls immediately");

        // Nothing before the interval elapses.
        tokio::time::advance(Duration::from_millis(1999)).await;
        tokio::task::yield_now().await;
        assert_eq!(polls.load(Ordering::SeqCst), 1, "no poll before the 2s interval");

        // At the interval boundary the second poll runs and completes.
        tokio::time::advance(Duration::from_millis(1)).await;
        let result = handle.await.unwrap();
        assert_eq!(result.unwrap(), "token");
        assert_eq!(polls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test(start_paused = true)]
    async fn cancels_an_in_flight_wait() {
        let (tx, rx) = tokio::sync::oneshot::channel::<()>();
        let cancel = async move { let _ = rx.await; };
        let poll = || async { DevicePollOutcome::<&str>::Pending };
        let handle = tokio::spawn(poll_oauth_device_code_flow(5, 30, false, poll, cancel));

        // Let the first (pending) poll run, then abort the in-flight interval wait.
        tokio::task::yield_now().await;
        let _ = tx.send(());
        let err = handle.await.unwrap().unwrap_err();
        assert_eq!(err, "Login cancelled");
    }

    #[tokio::test(start_paused = true)]
    async fn propagates_a_failed_poll_message() {
        let poll = || async { DevicePollOutcome::<&str>::Failed("access_denied".to_string()) };
        let err = poll_oauth_device_code_flow(5, 30, false, poll, std::future::pending::<()>())
            .await
            .unwrap_err();
        assert_eq!(err, "access_denied");
    }

    #[tokio::test(start_paused = true)]
    async fn times_out_with_the_default_message() {
        let poll = || async { DevicePollOutcome::<&str>::Pending };
        let handle = tokio::spawn(poll_oauth_device_code_flow(2, 6, false, poll, std::future::pending::<()>()));
        tokio::task::yield_now().await;
        tokio::time::advance(Duration::from_secs(7)).await;
        let err = handle.await.unwrap().unwrap_err();
        assert_eq!(err, "Device flow timed out");
    }

    #[tokio::test(start_paused = true)]
    async fn slow_down_increases_interval_and_uses_slow_down_timeout_message() {
        // First poll asks to slow down (interval 2s -> 7s), then stays pending
        // until the deadline; the slow-down-specific timeout message is used.
        let polls = Arc::new(AtomicUsize::new(0));
        let p = polls.clone();
        let poll = move || {
            let p = p.clone();
            async move {
                let n = p.fetch_add(1, Ordering::SeqCst) + 1;
                if n == 1 { DevicePollOutcome::<&str>::SlowDown(None) } else { DevicePollOutcome::<&str>::Pending }
            }
        };
        let handle = tokio::spawn(poll_oauth_device_code_flow(2, 20, false, poll, std::future::pending::<()>()));
        tokio::task::yield_now().await;
        assert_eq!(polls.load(Ordering::SeqCst), 1, "polls immediately");

        // Interval should now be 7s, not the original 2s.
        tokio::time::advance(Duration::from_millis(6999)).await;
        tokio::task::yield_now().await;
        assert_eq!(polls.load(Ordering::SeqCst), 1, "no second poll before the 7s slow-down interval");
        tokio::time::advance(Duration::from_millis(1)).await;
        tokio::task::yield_now().await;
        assert_eq!(polls.load(Ordering::SeqCst), 2, "second poll after the increased interval");

        // Run out the clock and confirm the slow-down timeout message.
        tokio::time::advance(Duration::from_secs(20)).await;
        let err = handle.await.unwrap().unwrap_err();
        assert_eq!(
            err,
            "Device flow timed out after one or more slow_down responses. This is often caused by clock drift in WSL or VM environments. Please sync or restart the VM clock and try again."
        );
    }

    // ---- v0.80.5 additions ----

    #[tokio::test(start_paused = true)]
    async fn can_wait_before_the_first_poll() {
        // With wait_before_first_poll, the first poll is deferred by one interval.
        let polls = Arc::new(AtomicUsize::new(0));
        let p = polls.clone();
        let poll = move || {
            let p = p.clone();
            async move {
                p.fetch_add(1, Ordering::SeqCst);
                DevicePollOutcome::Complete("token")
            }
        };
        let handle = tokio::spawn(poll_oauth_device_code_flow(2, 30, true, poll, std::future::pending::<()>()));

        tokio::task::yield_now().await;
        tokio::time::advance(Duration::from_millis(1999)).await;
        tokio::task::yield_now().await;
        assert_eq!(polls.load(Ordering::SeqCst), 0, "no poll before the first-interval wait elapses");

        tokio::time::advance(Duration::from_millis(1)).await;
        let result = handle.await.unwrap();
        assert_eq!(result.unwrap(), "token");
        assert_eq!(polls.load(Ordering::SeqCst), 1, "first poll runs at t=2s");
    }

    #[tokio::test(start_paused = true)]
    async fn increases_the_interval_by_5_seconds_after_slow_down_without_a_server_interval() {
        let polls = Arc::new(AtomicUsize::new(0));
        let p = polls.clone();
        let poll = move || {
            let p = p.clone();
            async move {
                let n = p.fetch_add(1, Ordering::SeqCst) + 1;
                if n == 1 { DevicePollOutcome::<&str>::SlowDown(None) } else { DevicePollOutcome::Complete("token") }
            }
        };
        let handle = tokio::spawn(poll_oauth_device_code_flow(2, 900, false, poll, std::future::pending::<()>()));
        tokio::task::yield_now().await;
        assert_eq!(polls.load(Ordering::SeqCst), 1, "polls immediately");

        // Interval increases 2s -> 7s: no poll before 7s.
        tokio::time::advance(Duration::from_millis(6999)).await;
        tokio::task::yield_now().await;
        assert_eq!(polls.load(Ordering::SeqCst), 1);
        tokio::time::advance(Duration::from_millis(1)).await;
        let result = handle.await.unwrap();
        assert_eq!(result.unwrap(), "token");
        assert_eq!(polls.load(Ordering::SeqCst), 2, "second poll after the +5s interval");
    }

    #[tokio::test(start_paused = true)]
    async fn honors_a_server_provided_slow_down_interval() {
        let polls = Arc::new(AtomicUsize::new(0));
        let p = polls.clone();
        let poll = move || {
            let p = p.clone();
            async move {
                let n = p.fetch_add(1, Ordering::SeqCst) + 1;
                if n == 1 { DevicePollOutcome::<&str>::SlowDown(Some(30)) } else { DevicePollOutcome::Complete("token") }
            }
        };
        let handle = tokio::spawn(poll_oauth_device_code_flow(2, 900, false, poll, std::future::pending::<()>()));
        tokio::task::yield_now().await;
        assert_eq!(polls.load(Ordering::SeqCst), 1, "polls immediately");

        // Server interval overrides to 30s: no poll before 30s.
        tokio::time::advance(Duration::from_millis(29999)).await;
        tokio::task::yield_now().await;
        assert_eq!(polls.load(Ordering::SeqCst), 1);
        tokio::time::advance(Duration::from_millis(1)).await;
        let result = handle.await.unwrap();
        assert_eq!(result.unwrap(), "token");
        assert_eq!(polls.load(Ordering::SeqCst), 2, "second poll after the server-provided 30s interval");
    }
}
