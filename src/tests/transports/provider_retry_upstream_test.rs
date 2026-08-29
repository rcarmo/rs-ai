//! Test-for-test adaptation of upstream `test/provider-retry.test.ts`.

#[cfg(test)]
mod tests {
    use crate::retry::{RetryConfig, do_with_retry_cancel};
    use std::time::Duration;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn retry_cfg(max_retries: u32, max_retry_delay_ms: u64) -> RetryConfig {
        RetryConfig {
            max_retries,
            initial_delay: Duration::from_millis(1),
            max_delay: Duration::from_millis(max_retry_delay_ms.max(1)),
            backoff_multiplier: 1.0,
            jitter_fraction: 0.0,
            max_retry_delay_ms,
        }
    }

    #[tokio::test]
    async fn retries_retryable_provider_errors_after_retry_after_ms() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/retry"))
            .respond_with(ResponseTemplate::new(429).insert_header("retry-after-ms", "1"))
            .up_to_n_times(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/retry"))
            .respond_with(ResponseTemplate::new(200).set_body_string("ok"))
            .mount(&server)
            .await;
        let client = reqwest::Client::new();
        let resp = do_with_retry_cancel(
            &client,
            client.get(format!("{}/retry", server.uri())),
            &retry_cfg(1, 1000),
            None,
        )
        .await
        .unwrap();
        assert_eq!(resp.status(), 200);
        assert_eq!(server.received_requests().await.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn does_not_retry_provider_marked_non_retryable() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/no"))
            .respond_with(ResponseTemplate::new(429).insert_header("x-should-retry", "false"))
            .mount(&server)
            .await;
        let client = reqwest::Client::new();
        let resp = do_with_retry_cancel(
            &client,
            client.get(format!("{}/no", server.uri())),
            &retry_cfg(2, 1000),
            None,
        )
        .await
        .unwrap();
        assert_eq!(resp.status(), 429);
        assert_eq!(server.received_requests().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn rejects_provider_requested_retry_delay_above_limit() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/delay"))
            .respond_with(ResponseTemplate::new(429).insert_header("retry-after", "277403"))
            .mount(&server)
            .await;
        let client = reqwest::Client::new();
        let err = do_with_retry_cancel(
            &client,
            client.get(format!("{}/delay", server.uri())),
            &retry_cfg(1, 1000),
            None,
        )
        .await
        .unwrap_err();
        assert!(
            err.to_string()
                .contains("Server requested 277403s retry delay (max: 1s)")
        );
        assert_eq!(server.received_requests().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn allows_disabling_provider_requested_retry_delay_cap() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/delay"))
            .respond_with(ResponseTemplate::new(429).insert_header("retry-after", "0.001"))
            .up_to_n_times(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/delay"))
            .respond_with(ResponseTemplate::new(200).set_body_string("ok"))
            .mount(&server)
            .await;
        let client = reqwest::Client::new();
        let resp = do_with_retry_cancel(
            &client,
            client.get(format!("{}/delay", server.uri())),
            &retry_cfg(1, 0),
            None,
        )
        .await
        .unwrap();
        assert_eq!(resp.status(), 200);
        assert_eq!(server.received_requests().await.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn aborts_provider_requested_retry_delay() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/abort"))
            .respond_with(ResponseTemplate::new(429).insert_header("retry-after", "277403"))
            .mount(&server)
            .await;
        let client = reqwest::Client::new();
        let (tx, rx) = tokio::sync::watch::channel(false);
        let url = format!("{}/abort", server.uri());
        let cfg = retry_cfg(2, 0);
        let handle = tokio::spawn(async move {
            do_with_retry_cancel(&client, client.get(url), &cfg, Some(rx)).await
        });
        tokio::time::timeout(Duration::from_millis(250), async {
            while server.received_requests().await.unwrap().is_empty() {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .expect("first request should be issued");
        tx.send(true).unwrap();
        let err = handle.await.unwrap().unwrap_err();
        assert_eq!(err.to_string(), "Request aborted");
        assert_eq!(server.received_requests().await.unwrap().len(), 1);
    }
}
