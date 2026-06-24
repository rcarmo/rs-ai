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
        let handle = tokio::spawn(poll_oauth_device_code_flow(2, 30, poll, std::future::pending::<()>()));

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
        let handle = tokio::spawn(poll_oauth_device_code_flow(5, 30, poll, cancel));

        // Let the first (pending) poll run, then abort the in-flight interval wait.
        tokio::task::yield_now().await;
        let _ = tx.send(());
        let err = handle.await.unwrap().unwrap_err();
        assert_eq!(err, "Login cancelled");
    }
}
