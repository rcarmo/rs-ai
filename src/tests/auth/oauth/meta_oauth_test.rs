#[cfg(test)]
mod tests {
    use crate::auth::OAuthAuth;
    use crate::auth_providers::MetaOAuth;
    use crate::oauth::{
        META_CLIENT_ID, MetaDeviceAuthorization, mint_meta_api_key_at, poll_meta_identity_token_at,
        request_meta_device_authorization_at,
    };
    use serde_json::json;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

    struct SequenceResponder {
        calls: Arc<AtomicUsize>,
        responses: Vec<(u16, serde_json::Value)>,
    }

    impl Respond for SequenceResponder {
        fn respond(&self, _request: &Request) -> ResponseTemplate {
            let index = self.calls.fetch_add(1, Ordering::SeqCst);
            let (status, body) = self
                .responses
                .get(index)
                .or_else(|| self.responses.last())
                .cloned()
                .unwrap();
            ResponseTemplate::new(status).set_body_json(body)
        }
    }

    #[tokio::test(start_paused = true)]
    async fn meta_device_flow_polls_and_mints_api_key() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/device"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "device_code":"device-code-123", "user_code":"ABCD-1234",
                "verification_uri":"https://auth.meta.com/oauth/device/",
                "verification_uri_complete":"https://auth.meta.com/oauth/device/?code=ABCD-1234",
                "interval":1, "expires_in":600
            })))
            .mount(&server)
            .await;
        let calls = Arc::new(AtomicUsize::new(0));
        Mock::given(method("POST"))
            .and(path("/token"))
            .respond_with(SequenceResponder {
                calls: calls.clone(),
                responses: vec![
                    (400, json!({"error":"authorization_pending"})),
                    (200, json!({"access_token":"identity-token"})),
                ],
            })
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/mint"))
            .and(header("authorization", "Bearer identity-token"))
            .and(header("x-api-version", "1.0.0"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(json!({"api_key":"LLM|minted-key"})),
            )
            .mount(&server)
            .await;

        let device = request_meta_device_authorization_at(&format!("{}/device", server.uri()))
            .await
            .unwrap();
        assert_eq!(device.user_code, "ABCD-1234");
        let token_url = format!("{}/token", server.uri());
        let poll = tokio::spawn(async move {
            poll_meta_identity_token_at(&token_url, &device, std::future::pending::<()>()).await
        });
        tokio::time::advance(std::time::Duration::from_secs(2)).await;
        let identity = poll.await.unwrap().unwrap();
        assert_eq!(identity, "identity-token");
        let before = crate::utils::now_millis();
        let credential = mint_meta_api_key_at(&format!("{}/mint", server.uri()), &identity)
            .await
            .unwrap();
        assert_eq!(credential.access, "LLM|minted-key");
        assert_eq!(credential.refresh.as_deref(), Some("identity-token"));
        assert!(credential.expires >= before + 24 * 60 * 60 * 1000);
        assert_eq!(calls.load(Ordering::SeqCst), 2);

        let requests = server.received_requests().await.unwrap();
        let body = requests
            .iter()
            .find(|request| request.url.path() == "/device")
            .unwrap();
        assert!(
            String::from_utf8_lossy(&body.body).contains(&format!("client_id={META_CLIENT_ID}"))
        );
    }

    #[tokio::test]
    async fn meta_refresh_remints_from_stored_identity_token() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/mint"))
            .and(header("authorization", "Bearer identity-token"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(json!({"api_key":"LLM|fresh-key"})),
            )
            .mount(&server)
            .await;
        let oauth = MetaOAuth {
            mint_url: Some(format!("{}/mint", server.uri())),
        };
        let refreshed = oauth
            .refresh(&crate::auth::OAuthCredential {
                access: "LLM|old-key".into(),
                refresh: Some("identity-token".into()),
                expires: 1,
                account_id: None,
            })
            .await
            .unwrap();
        assert_eq!(refreshed.access, "LLM|fresh-key");
        assert_eq!(refreshed.refresh.as_deref(), Some("identity-token"));
        assert_eq!(
            oauth.to_auth(&refreshed).await.unwrap().api_key.as_deref(),
            Some("LLM|fresh-key")
        );
    }

    #[tokio::test]
    async fn meta_mint_errors_include_setup_or_relogin_guidance() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/setup"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                json!({"require_payment":true,"action_url":"https://dev.meta.ai/billing"}),
            ))
            .mount(&server)
            .await;
        let err = mint_meta_api_key_at(&format!("{}/setup", server.uri()), "identity")
            .await
            .unwrap_err();
        assert!(err.contains("Complete setup at https://dev.meta.ai/billing"));

        Mock::given(method("POST"))
            .and(path("/expired"))
            .respond_with(ResponseTemplate::new(401).set_body_json(json!({"detail":"expired"})))
            .mount(&server)
            .await;
        let err = mint_meta_api_key_at(&format!("{}/expired", server.uri()), "identity")
            .await
            .unwrap_err();
        assert!(err.contains("Run `/login meta`"));
    }

    #[test]
    fn meta_device_authorization_type_is_public_and_exact() {
        let value = MetaDeviceAuthorization {
            device_code: "d".into(),
            user_code: "u".into(),
            verification_uri: "https://example.com/".into(),
            interval_seconds: 5,
            expires_in_seconds: 600,
        };
        assert_eq!(value.interval_seconds, 5);
    }
}
