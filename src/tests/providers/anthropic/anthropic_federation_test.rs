//! Deterministic Rust adaptation of upstream v0.99.2 Anthropic federation tests.

#[cfg(test)]
mod tests {
    use crate::env::{AnthropicFederationConfig, anthropic_federation_config_from};
    use crate::events::Event;
    use crate::provider::anthropic::{
        reset_anthropic_federation_cache, stream_anthropic_with_federation,
    };
    use crate::types::{ContentBlock, Context, Model, ModelCost, StreamOptions};
    use futures::StreamExt;
    use serde_json::{Value, json};
    use std::collections::HashMap;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

    static FEDERATION_TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

    fn model(base_url: &str) -> Model {
        Model {
            id: "claude-test".into(),
            name: "Claude Test".into(),
            api: "anthropic-messages".into(),
            provider: "anthropic".into(),
            base_url: base_url.into(),
            reasoning: false,
            thinking_level_map: None,
            input: vec!["text".into()],
            input_limits: None,
            prompt_cache: None,
            enabled: None,
            lab: None,
            providers: None,
            cost: ModelCost::default(),
            context_window: 200_000,
            max_tokens: 8192,
            sampling_params: None,
            headers: None,
            api_key: None,
            compat: Default::default(),
        }
    }

    fn context() -> Context {
        Context {
            system_prompt: None,
            messages: vec![crate::user_message("hello")],
            tools: Vec::new(),
        }
    }

    fn identity_token_file(name: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "rs-ai-{name}-{}-{}.jwt",
            std::process::id(),
            crate::utils::now_millis()
        ));
        std::fs::write(&path, "identity-jwt\n").unwrap();
        path
    }

    fn config(file: &str, rule: &str) -> AnthropicFederationConfig {
        AnthropicFederationConfig {
            rule_id: rule.into(),
            organization_id: "org-test".into(),
            service_account_id: Some("svc-test".into()),
            identity_token_file: file.into(),
            workspace_id: Some("ws-test".into()),
        }
    }

    fn sse() -> String {
        [
            "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_1\",\"model\":\"claude-test\",\"usage\":{\"input_tokens\":1,\"output_tokens\":0}}}\n\n",
            "event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n",
            "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"OK\"}}\n\n",
            "event: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":0}\n\n",
            "event: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":1}}\n\n",
            "event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n",
        ]
        .concat()
    }

    async fn stream_result(
        model: &Model,
        options: &StreamOptions,
        federation: Option<AnthropicFederationConfig>,
    ) -> Result<crate::types::Message, String> {
        let context = context();
        let mut stream = stream_anthropic_with_federation(model, &context, options, federation);
        let mut final_message = None;
        while let Some(event) = stream.next().await {
            match event {
                Event::Done { message, .. } => final_message = Some(message),
                Event::Error { error, .. } => return Err(error.to_string()),
                _ => {}
            }
        }
        final_message.ok_or_else(|| "stream ended without a terminal message".into())
    }

    async fn finish(
        model: &Model,
        options: &StreamOptions,
        federation: Option<AnthropicFederationConfig>,
    ) -> crate::types::Message {
        stream_result(model, options, federation).await.unwrap()
    }

    #[test]
    fn federation_config_requires_all_required_values_and_anthropic_provider() {
        let values = HashMap::from([
            ("ANTHROPIC_FEDERATION_RULE_ID", "rule".to_string()),
            ("ANTHROPIC_ORGANIZATION_ID", "org".to_string()),
            ("ANTHROPIC_IDENTITY_TOKEN_FILE", "/token".to_string()),
            ("ANTHROPIC_SERVICE_ACCOUNT_ID", "svc".to_string()),
            ("ANTHROPIC_WORKSPACE_ID", "ws".to_string()),
        ]);
        let got = anthropic_federation_config_from("anthropic", |name| values.get(name).cloned())
            .unwrap();
        assert_eq!(got.rule_id, "rule");
        assert_eq!(got.organization_id, "org");
        assert_eq!(got.identity_token_file, "/token");
        assert_eq!(got.service_account_id.as_deref(), Some("svc"));
        assert_eq!(got.workspace_id.as_deref(), Some("ws"));
        assert!(
            anthropic_federation_config_from("kimi-coding", |name| values.get(name).cloned())
                .is_none()
        );
        for missing in [
            "ANTHROPIC_FEDERATION_RULE_ID",
            "ANTHROPIC_ORGANIZATION_ID",
            "ANTHROPIC_IDENTITY_TOKEN_FILE",
        ] {
            assert!(
                anthropic_federation_config_from("anthropic", |name| {
                    (name != missing)
                        .then(|| values.get(name).cloned())
                        .flatten()
                })
                .is_none()
            );
        }
    }

    #[tokio::test]
    async fn exchanges_file_identity_once_and_reuses_bearer_token() {
        let _test_guard = FEDERATION_TEST_LOCK.lock().await;
        reset_anthropic_federation_cache().await;
        let server = MockServer::start().await;
        let token_file = identity_token_file("cache");
        Mock::given(method("POST"))
            .and(path("/v1/oauth/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "access_token":"federated-token",
                "expires_in":3600
            })))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/messages"))
            .and(header("authorization", "Bearer federated-token"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_string(sse()),
            )
            .expect(3)
            .mount(&server)
            .await;
        let model = model(&server.uri());
        let cfg = config(token_file.to_str().unwrap(), "rule-cache");
        for _ in 0..3 {
            let message = finish(&model, &StreamOptions::default(), Some(cfg.clone())).await;
            assert!(
                message
                    .content
                    .iter()
                    .any(|block| matches!(block, ContentBlock::Text { text, .. } if text == "OK"))
            );
        }
        server.verify().await;
        let requests = server.received_requests().await.unwrap();
        let token_requests = requests
            .iter()
            .filter(|request| request.url.path() == "/v1/oauth/token")
            .collect::<Vec<_>>();
        assert_eq!(token_requests.len(), 1);
        assert_eq!(token_requests[0].url.path(), "/v1/oauth/token");
        assert!(token_requests[0].url.query().is_none());
        assert_eq!(
            token_requests[0]
                .headers
                .get("host")
                .and_then(|value| value.to_str().ok()),
            server.uri().strip_prefix("http://")
        );
        assert_eq!(
            serde_json::from_slice::<Value>(&token_requests[0].body).unwrap(),
            json!({
                "grant_type":"urn:ietf:params:oauth:grant-type:jwt-bearer",
                "assertion":"identity-jwt",
                "federation_rule_id":"rule-cache",
                "organization_id":"org-test",
                "service_account_id":"svc-test",
                "workspace_id":"ws-test"
            })
        );
        assert_eq!(
            token_requests[0]
                .headers
                .get("anthropic-beta")
                .and_then(|value| value.to_str().ok()),
            Some("oauth-2025-04-20,oidc-federation-2026-04-01")
        );
        assert!(token_requests[0].headers.get("authorization").is_none());
        assert!(token_requests[0].headers.get("x-api-key").is_none());
        let message_requests = requests
            .iter()
            .filter(|request| request.url.path() == "/messages")
            .collect::<Vec<_>>();
        assert!(message_requests.iter().all(|request| {
            request
                .headers
                .get("authorization")
                .is_some_and(|value| value.to_str().ok() == Some("Bearer federated-token"))
                && request.headers.get("x-api-key").is_none()
        }));
        std::fs::remove_file(token_file).unwrap();
    }

    struct CountTokenRequests(Arc<AtomicUsize>);
    impl Respond for CountTokenRequests {
        fn respond(&self, request: &Request) -> ResponseTemplate {
            if request.url.path() == "/v1/oauth/token" {
                self.0.fetch_add(1, Ordering::SeqCst);
            }
            ResponseTemplate::new(500)
        }
    }

    #[tokio::test]
    async fn explicit_api_key_and_authorization_header_bypass_federation() {
        let _test_guard = FEDERATION_TEST_LOCK.lock().await;
        reset_anthropic_federation_cache().await;
        let server = MockServer::start().await;
        let token_file = identity_token_file("precedence");
        let exchanges = Arc::new(AtomicUsize::new(0));
        Mock::given(method("POST"))
            .and(path("/v1/oauth/token"))
            .respond_with(CountTokenRequests(exchanges.clone()))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/messages"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_string(sse()),
            )
            .expect(3)
            .mount(&server)
            .await;
        let model = model(&server.uri());
        let mut model_header = model.clone();
        model_header.headers = Some(HashMap::from([("x-api-key".into(), "model-key".into())]));
        let cfg = config(token_file.to_str().unwrap(), "rule-precedence");
        finish(
            &model,
            &StreamOptions {
                api_key: Some("direct-key".into()),
                ..Default::default()
            },
            Some(cfg.clone()),
        )
        .await;
        finish(
            &model,
            &StreamOptions {
                headers: Some(HashMap::from([(
                    "Authorization".into(),
                    "Bearer header-token".into(),
                )])),
                ..Default::default()
            },
            Some(cfg.clone()),
        )
        .await;
        finish(&model_header, &StreamOptions::default(), Some(cfg)).await;
        assert_eq!(exchanges.load(Ordering::SeqCst), 0);
        let requests = server.received_requests().await.unwrap();
        let message_requests = requests
            .iter()
            .filter(|request| request.url.path() == "/messages")
            .collect::<Vec<_>>();
        assert_eq!(message_requests.len(), 3);
        assert!(message_requests.iter().any(|request| {
            request
                .headers
                .get("authorization")
                .is_some_and(|value| value.to_str().ok() == Some("Bearer header-token"))
        }));
        for key in ["direct-key", "model-key"] {
            assert!(message_requests.iter().any(|request| {
                request
                    .headers
                    .get("x-api-key")
                    .is_some_and(|value| value.to_str().ok() == Some(key))
            }));
        }
        std::fs::remove_file(token_file).unwrap();
    }

    #[tokio::test]
    async fn concurrent_callers_coalesce_one_token_exchange() {
        let _test_guard = FEDERATION_TEST_LOCK.lock().await;
        reset_anthropic_federation_cache().await;
        let server = MockServer::start().await;
        let token_file = identity_token_file("concurrent");
        Mock::given(method("POST"))
            .and(path("/v1/oauth/token"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_delay(std::time::Duration::from_millis(50))
                    .set_body_json(json!({
                        "access_token":"coalesced-token",
                        "token_type":"Bearer",
                        "expires_in":3600
                    })),
            )
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/messages"))
            .and(header("authorization", "Bearer coalesced-token"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_string(sse()),
            )
            .expect(8)
            .mount(&server)
            .await;
        let model = Arc::new(model(&server.uri()));
        let cfg = config(token_file.to_str().unwrap(), "rule-concurrent");
        let mut handles = Vec::new();
        for _ in 0..8 {
            let model = model.clone();
            let cfg = cfg.clone();
            handles.push(tokio::spawn(async move {
                finish(&model, &StreamOptions::default(), Some(cfg)).await
            }));
        }
        for handle in handles {
            assert_eq!(
                handle.await.unwrap().stop_reason,
                Some(crate::types::StopReason::Stop)
            );
        }
        server.verify().await;
        std::fs::remove_file(token_file).unwrap();
    }

    #[tokio::test]
    async fn expiring_token_refreshes_and_cache_reset_isolates_keys() {
        let _test_guard = FEDERATION_TEST_LOCK.lock().await;
        reset_anthropic_federation_cache().await;
        let server = MockServer::start().await;
        let file_a = identity_token_file("expiry-a");
        let file_b = identity_token_file("expiry-b");
        let exchanges = Arc::new(AtomicUsize::new(0));
        let exchanges_cb = exchanges.clone();
        Mock::given(method("POST"))
            .and(path("/v1/oauth/token"))
            .respond_with(move |_request: &Request| {
                let n = exchanges_cb.fetch_add(1, Ordering::SeqCst) + 1;
                ResponseTemplate::new(200).set_body_json(json!({
                    "access_token":format!("token-{n}"),
                    "token_type":"Bearer",
                    "expires_in":if n <= 2 { 1 } else { 3600 }
                }))
            })
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/messages"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_string(sse()),
            )
            .mount(&server)
            .await;
        let model = model(&server.uri());
        let a = config(file_a.to_str().unwrap(), "rule-expiry-a");
        let b = config(file_b.to_str().unwrap(), "rule-expiry-b");
        finish(&model, &StreamOptions::default(), Some(a.clone())).await;
        finish(&model, &StreamOptions::default(), Some(a.clone())).await;
        finish(&model, &StreamOptions::default(), Some(b)).await;
        assert_eq!(
            exchanges.load(Ordering::SeqCst),
            3,
            "expiry and key isolation must exchange"
        );
        reset_anthropic_federation_cache().await;
        finish(&model, &StreamOptions::default(), Some(a)).await;
        assert_eq!(
            exchanges.load(Ordering::SeqCst),
            4,
            "cache reset must force exchange"
        );
        std::fs::remove_file(file_a).unwrap();
        std::fs::remove_file(file_b).unwrap();
    }

    #[tokio::test]
    async fn trims_identity_assertion_and_keeps_it_out_of_url_and_headers() {
        let _test_guard = FEDERATION_TEST_LOCK.lock().await;
        reset_anthropic_federation_cache().await;
        let server = MockServer::start().await;
        let token_file = identity_token_file("trim");
        Mock::given(method("POST"))
            .and(path("/v1/oauth/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "access_token":"trimmed-token", "token_type":"Bearer", "expires_in":3600
            })))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/messages"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_string(sse()),
            )
            .mount(&server)
            .await;
        finish(
            &model(&server.uri()),
            &StreamOptions::default(),
            Some(config(token_file.to_str().unwrap(), "rule-trim")),
        )
        .await;
        let request = server
            .received_requests()
            .await
            .unwrap()
            .into_iter()
            .find(|request| request.url.path() == "/v1/oauth/token")
            .unwrap();
        assert_eq!(
            serde_json::from_slice::<Value>(&request.body).unwrap()["assertion"],
            "identity-jwt"
        );
        assert!(!request.url.as_str().contains("identity-jwt"));
        assert!(request.headers.values().all(|value| {
            value
                .to_str()
                .ok()
                .is_none_or(|text| !text.contains("identity-jwt"))
        }));
        std::fs::remove_file(token_file).unwrap();
    }

    #[tokio::test]
    async fn rejects_missing_access_token_and_bad_token_type() {
        let _test_guard = FEDERATION_TEST_LOCK.lock().await;
        for (name, response, expected) in [
            (
                "missing",
                json!({"token_type":"Bearer","expires_in":3600}),
                "missing access_token",
            ),
            (
                "type",
                json!({"access_token":"secret-access","token_type":"Basic","expires_in":3600}),
                "unsupported token_type",
            ),
        ] {
            reset_anthropic_federation_cache().await;
            let server = MockServer::start().await;
            let token_file = identity_token_file(name);
            Mock::given(method("POST"))
                .and(path("/v1/oauth/token"))
                .respond_with(ResponseTemplate::new(200).set_body_json(response))
                .mount(&server)
                .await;
            let error = stream_result(
                &model(&server.uri()),
                &StreamOptions::default(),
                Some(config(
                    token_file.to_str().unwrap(),
                    &format!("rule-{name}"),
                )),
            )
            .await
            .unwrap_err();
            assert!(error.contains(expected), "{error}");
            assert!(!error.contains("secret-access"));
            assert!(!error.contains("identity-jwt"));
            std::fs::remove_file(token_file).unwrap();
        }
    }

    #[tokio::test]
    async fn endpoint_and_transport_failures_redact_assertion_echo() {
        let _test_guard = FEDERATION_TEST_LOCK.lock().await;
        reset_anthropic_federation_cache().await;
        let server = MockServer::start().await;
        let token_file = identity_token_file("redact");
        Mock::given(method("POST"))
            .and(path("/v1/oauth/token"))
            .respond_with(ResponseTemplate::new(401).set_body_json(json!({
                "error":"invalid_grant",
                "error_description":"assertion identity-jwt rejected",
                "assertion":"identity-jwt",
                "access_token":"leaked-token"
            })))
            .mount(&server)
            .await;
        let error = stream_result(
            &model(&server.uri()),
            &StreamOptions::default(),
            Some(config(token_file.to_str().unwrap(), "rule-redact")),
        )
        .await
        .unwrap_err();
        assert!(error.contains("invalid_grant"));
        assert!(error.contains("<redacted>"));
        for secret in ["identity-jwt", "leaked-token"] {
            assert!(!error.contains(secret), "error leaked {secret}: {error}");
        }
        let transport_error = stream_result(
            &model("http://127.0.0.1:1"),
            &StreamOptions::default(),
            Some(config(token_file.to_str().unwrap(), "rule-transport")),
        )
        .await
        .unwrap_err();
        assert!(transport_error.contains("token exchange failed"));
        assert!(!transport_error.contains("identity-jwt"));
        std::fs::remove_file(token_file).unwrap();
    }

    #[tokio::test]
    async fn cancellation_interrupts_token_exchange() {
        let _test_guard = FEDERATION_TEST_LOCK.lock().await;
        reset_anthropic_federation_cache().await;
        let server = MockServer::start().await;
        let token_file = identity_token_file("cancel");
        Mock::given(method("POST"))
            .and(path("/v1/oauth/token"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_delay(std::time::Duration::from_secs(30))
                    .set_body_json(json!({
                        "access_token":"late-token","token_type":"Bearer","expires_in":3600
                    })),
            )
            .mount(&server)
            .await;
        let (tx, rx) = tokio::sync::watch::channel(false);
        let model = model(&server.uri());
        let cfg = config(token_file.to_str().unwrap(), "rule-cancel");
        let handle = tokio::spawn(async move {
            stream_result(
                &model,
                &StreamOptions {
                    cancel: Some(rx),
                    ..Default::default()
                },
                Some(cfg),
            )
            .await
        });
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            while server.received_requests().await.unwrap().is_empty() {
                tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            }
        })
        .await
        .expect("token request started");
        tx.send(true).unwrap();
        let error = handle.await.unwrap().unwrap_err();
        assert_eq!(error, "Anthropic federation request aborted");
        std::fs::remove_file(token_file).unwrap();
    }
}
