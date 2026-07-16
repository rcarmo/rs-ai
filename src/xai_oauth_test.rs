//! Deterministic xAI device OAuth parity for upstream `auth/oauth/xai.ts`.

#[cfg(test)]
mod tests {
    use crate::auth::OAuthAuth;
    use crate::auth_providers::XaiOAuth;
    use crate::oauth::*;
    use serde_json::json;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

    struct Seq {
        calls: Arc<AtomicUsize>,
        responses: Vec<(u16, serde_json::Value)>,
    }
    impl Respond for Seq {
        fn respond(&self, _request: &Request) -> ResponseTemplate {
            let idx = self.calls.fetch_add(1, Ordering::SeqCst);
            let (status, body) = self
                .responses
                .get(idx)
                .or_else(|| self.responses.last())
                .cloned()
                .unwrap();
            ResponseTemplate::new(status).set_body_json(body)
        }
    }

    async fn mount_device(
        server: &MockServer,
        interval: serde_json::Value,
        expires: u64,
        uri: &str,
    ) {
        Mock::given(method("POST")).and(path("/device"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "device_code":"dev", "user_code":"USER", "verification_uri":uri, "interval": interval, "expires_in": expires
            }))).mount(server).await;
    }

    async fn wait_for(calls: &AtomicUsize, n: usize) {
        for _ in 0..100 {
            if calls.load(Ordering::SeqCst) >= n {
                return;
            }
            tokio::task::yield_now().await;
            tokio::time::advance(Duration::from_millis(1)).await;
        }
    }

    #[tokio::test(start_paused = true)]
    async fn device_login_polls_pending_slowdown_then_success_after_initial_wait() {
        let server = MockServer::start().await;
        mount_device(&server, json!(2), 120, "https://x.ai/activate").await;
        let calls = Arc::new(AtomicUsize::new(0));
        Mock::given(method("POST")).and(path("/token"))
            .respond_with(Seq { calls: calls.clone(), responses: vec![
                (400, json!({"error":"authorization_pending"})),
                (400, json!({"error":"slow_down","interval":7})),
                (200, json!({"access_token":"access","refresh_token":"refresh","expires_in":3600})),
            ]}).mount(&server).await;
        let handle = tokio::spawn({
            let base = server.uri();
            async move {
                login_xai_device_code_at(
                    &format!("{base}/device"),
                    &format!("{base}/token"),
                    std::future::pending::<()>(),
                )
                .await
            }
        });
        tokio::task::yield_now().await;
        assert_eq!(
            calls.load(Ordering::SeqCst),
            0,
            "xAI waits before first poll"
        );
        tokio::time::advance(Duration::from_secs(2)).await;
        wait_for(&calls, 1).await;
        tokio::time::advance(Duration::from_secs(3)).await;
        wait_for(&calls, 2).await;
        tokio::time::advance(Duration::from_secs(7)).await;
        wait_for(&calls, 3).await;
        let cred = handle.await.unwrap().unwrap();
        assert_eq!(cred.access, "access");
        assert_eq!(cred.refresh.as_deref(), Some("refresh"));
        assert_eq!(calls.load(Ordering::SeqCst), 3, "stops after success");
        let reqs = server.received_requests().await.unwrap();
        let device_body = String::from_utf8_lossy(
            &reqs
                .iter()
                .find(|r| r.url.path() == "/device")
                .unwrap()
                .body,
        );
        assert!(device_body.contains("client_id="));
        assert!(device_body.contains("referrer=pi"));
    }

    #[tokio::test(start_paused = true)]
    async fn terminal_errors_timeout_and_cancel_are_propagated() {
        for (err, expected) in [
            ("access_denied", "xAI device authorization was denied"),
            (
                "authorization_denied",
                "xAI device authorization was denied",
            ),
            ("expired_token", "xAI device code expired"),
        ] {
            let server = MockServer::start().await;
            mount_device(&server, json!(1), 60, "https://x.ai/activate").await;
            let calls = Arc::new(AtomicUsize::new(0));
            Mock::given(method("POST"))
                .and(path("/token"))
                .respond_with(Seq {
                    calls: calls.clone(),
                    responses: vec![(400, json!({"error":err}))],
                })
                .mount(&server)
                .await;
            let e = login_xai_device_code_at(
                &format!("{}/device", server.uri()),
                &format!("{}/token", server.uri()),
                std::future::pending::<()>(),
            )
            .await
            .unwrap_err();
            assert_eq!(e, expected);
            assert_eq!(calls.load(Ordering::SeqCst), 1);
        }

        let server = MockServer::start().await;
        mount_device(&server, json!(1), 3, "https://x.ai/activate").await;
        let calls = Arc::new(AtomicUsize::new(0));
        Mock::given(method("POST"))
            .and(path("/token"))
            .respond_with(Seq {
                calls: calls.clone(),
                responses: vec![(400, json!({"error":"authorization_pending"}))],
            })
            .mount(&server)
            .await;
        let handle = tokio::spawn({
            let base = server.uri();
            async move {
                login_xai_device_code_at(
                    &format!("{base}/device"),
                    &format!("{base}/token"),
                    std::future::pending::<()>(),
                )
                .await
            }
        });
        tokio::time::advance(Duration::from_secs(5)).await;
        assert_eq!(handle.await.unwrap().unwrap_err(), "Device flow timed out");
        assert!(calls.load(Ordering::SeqCst) <= 4);

        let server = MockServer::start().await;
        mount_device(&server, json!(5), 60, "https://x.ai/activate").await;
        let calls = Arc::new(AtomicUsize::new(0));
        Mock::given(method("POST"))
            .and(path("/token"))
            .respond_with(Seq {
                calls: calls.clone(),
                responses: vec![(400, json!({"error":"authorization_pending"}))],
            })
            .mount(&server)
            .await;
        let (tx, rx) = tokio::sync::oneshot::channel::<()>();
        let handle = tokio::spawn({
            let base = server.uri();
            async move {
                login_xai_device_code_at(
                    &format!("{base}/device"),
                    &format!("{base}/token"),
                    async {
                        let _ = rx.await;
                    },
                )
                .await
            }
        });
        tx.send(()).unwrap();
        assert_eq!(handle.await.unwrap().unwrap_err(), "Login cancelled");
        assert!(calls.load(Ordering::SeqCst) <= 1);
    }

    #[tokio::test]
    async fn refresh_preserves_unrotated_refresh_token_and_adapter_derives_api_key() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/token"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(json!({"access_token":"new-access","expires_in":60})),
            )
            .mount(&server)
            .await;
        let cred = refresh_xai_token_at(&format!("{}/token", server.uri()), "old-refresh")
            .await
            .unwrap();
        assert_eq!(cred.access, "new-access");
        assert_eq!(cred.refresh.as_deref(), Some("old-refresh"));
        let adapter = XaiOAuth {
            token_url: Some(format!("{}/token", server.uri())),
        };
        let refreshed = adapter
            .refresh(&crate::auth::OAuthCredential {
                access: "old".into(),
                refresh: Some("old-refresh".into()),
                expires: 0,
                account_id: None,
            })
            .await
            .unwrap();
        assert_eq!(refreshed.access, "new-access");
        let auth = adapter.to_auth(&refreshed).await.unwrap();
        assert_eq!(auth.api_key.as_deref(), Some("new-access"));
    }

    #[tokio::test]
    async fn rejects_untrusted_verification_uri_and_invalid_fields() {
        let server = MockServer::start().await;
        mount_device(&server, json!(0), 60, "http://evil.test/activate").await;
        let err = request_xai_device_code_at(&format!("{}/device", server.uri()))
            .await
            .unwrap_err();
        assert_eq!(err, "Untrusted verification URI in xAI OAuth response");

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/device"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"device_code":"dev"})))
            .mount(&server)
            .await;
        let err = request_xai_device_code_at(&format!("{}/device", server.uri()))
            .await
            .unwrap_err();
        assert!(err.starts_with("Invalid xAI OAuth response field:"));
    }
}
