//! Test-for-test port (deterministic substance) of upstream
//! `test/github-copilot-oauth.test.ts` (`@earendil-works/pi-ai` v0.84.2).
//!
//! The full interactive browser login UI remains host-owned, but the portable
//! OAuth/device-code primitives, model-picker parsing, Individual-account policy
//! fallback, and bounded Copilot policy-enable batching are deterministic here.

#[cfg(test)]
mod tests {
    use crate::oauth::{
        COPILOT_API_VERSION, COPILOT_POLICY_CONCURRENCY, copilot_policy_model_ids,
        enable_github_copilot_models_at, fetch_available_github_copilot_model_ids_at,
        is_selectable_copilot_model, selectable_copilot_model_ids,
        selectable_copilot_model_ids_with_policy_fallback,
    };
    use serde_json::json;
    use std::time::Duration;
    use wiremock::matchers::{header, method, path, path_regex};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn filters_models_to_the_authenticated_account_picker_catalog() {
        let data = vec![
            json!({"id": "gpt-4.1", "model_picker_enabled": true, "capabilities": {"supports": {"tool_calls": true}}}),
            json!({"id": "claude-opus-4.7", "model_picker_enabled": true, "policy": {"state": "disabled"}, "capabilities": {"supports": {"tool_calls": true}}}),
            json!({"id": "gpt-5.4-nano", "model_picker_enabled": false, "capabilities": {"supports": {"tool_calls": true}}}),
        ];
        assert_eq!(
            selectable_copilot_model_ids(&data),
            vec!["gpt-4.1".to_string()]
        );
    }

    #[test]
    fn is_selectable_requires_picker_enabled_not_disabled_and_tool_calls() {
        // picker enabled + tool calls + no disabled policy -> selectable.
        assert!(is_selectable_copilot_model(
            &json!({"model_picker_enabled": true, "capabilities": {"supports": {"tool_calls": true}}})
        ));
        // policy disabled -> not selectable.
        assert!(!is_selectable_copilot_model(
            &json!({"model_picker_enabled": true, "policy": {"state": "disabled"}, "capabilities": {"supports": {"tool_calls": true}}})
        ));
        // picker disabled -> not selectable.
        assert!(!is_selectable_copilot_model(
            &json!({"model_picker_enabled": false, "capabilities": {"supports": {"tool_calls": true}}})
        ));
        // no tool-call support -> not selectable.
        assert!(!is_selectable_copilot_model(
            &json!({"model_picker_enabled": true, "capabilities": {"supports": {"tool_calls": false}}})
        ));
    }

    #[test]
    fn falls_back_to_policy_enabled_ids_only_for_individual_endpoint() {
        let data = vec![
            json!({"id": "claude-opus-4.7", "model_picker_enabled": false, "policy": {"state": "enabled"}, "capabilities": {"supports": {"tool_calls": true}}}),
            json!({"id": "gpt-no-tools", "model_picker_enabled": false, "policy": {"state": "enabled"}, "capabilities": {"supports": {"tool_calls": false}}}),
            json!({"id": "gpt-disabled", "model_picker_enabled": false, "policy": {"state": "disabled"}, "capabilities": {"supports": {"tool_calls": true}}}),
        ];
        assert_eq!(
            selectable_copilot_model_ids_with_policy_fallback(&data, false),
            Vec::<String>::new()
        );
        assert_eq!(
            selectable_copilot_model_ids_with_policy_fallback(&data, true),
            vec!["claude-opus-4.7".to_string()]
        );
    }

    #[tokio::test]
    async fn fetch_available_model_ids_uses_copilot_headers() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/models"))
            .and(header("authorization", "Bearer token"))
            .and(header("x-github-api-version", COPILOT_API_VERSION))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": [
                    {"id": "gpt-4.1", "model_picker_enabled": true, "policy": {"state": "enabled"}}
                ]
            })))
            .mount(&server)
            .await;
        let ids = fetch_available_github_copilot_model_ids_at(&server.uri(), "token")
            .await
            .unwrap();
        assert_eq!(ids, vec!["gpt-4.1".to_string()]);
    }

    #[test]
    fn policy_updates_only_known_tool_capable_unconfigured_models() {
        let data = vec![
            json!({"id":"gpt-4.1","policy":{"state":"enabled"},"capabilities":{"supports":{"tool_calls":true}}}),
            json!({"id":"claude-sonnet-4.6","policy":{"state":"unconfigured"},"capabilities":{"supports":{"tool_calls":true}}}),
            json!({"id":"remote-only-model","policy":{"state":"unconfigured"},"capabilities":{"supports":{"tool_calls":true}}}),
            json!({"id":"gpt-5.4","policy":{"state":"unconfigured"},"capabilities":{"supports":{"tool_calls":false}}}),
        ];
        assert_eq!(
            copilot_policy_model_ids(&data),
            vec!["claude-sonnet-4.6".to_string()]
        );
    }

    #[tokio::test]
    async fn retries_throttled_policy_update_once_and_continues_transport_failures() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/models/gpt-4.1/policy"))
            .respond_with(ResponseTemplate::new(429).insert_header("retry-after", "0"))
            .up_to_n_times(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/models/gpt-4.1/policy"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/models/claude-sonnet-4.6/policy"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;
        enable_github_copilot_models_at(
            &server.uri(),
            "token",
            &["gpt-4.1".into(), "claude-sonnet-4.6".into()],
        )
        .await
        .unwrap();
        let requests = server.received_requests().await.unwrap();
        let paths = requests
            .iter()
            .map(|r| r.url.path().to_string())
            .collect::<Vec<_>>();
        assert_eq!(
            paths,
            vec![
                "/models/gpt-4.1/policy",
                "/models/claude-sonnet-4.6/policy",
                "/models/gpt-4.1/policy"
            ]
        );
    }

    #[tokio::test]
    async fn limits_concurrent_policy_updates_to_four_during_login() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path_regex(r"^/models/[^/]+/policy$"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "application/json")
                    .set_body_json(json!({"ok": true}))
                    .set_delay(Duration::from_millis(120)),
            )
            .mount(&server)
            .await;

        let ids = (0..10).map(|i| format!("model-{i}")).collect::<Vec<_>>();
        let handle = tokio::spawn({
            let base = server.uri();
            let ids = ids.clone();
            async move { enable_github_copilot_models_at(&base, "token", &ids).await }
        });

        let mut first_wave = 0;
        for _ in 0..20 {
            first_wave = server.received_requests().await.unwrap().len();
            if first_wave > 0 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert_eq!(first_wave, COPILOT_POLICY_CONCURRENCY);

        tokio::time::sleep(Duration::from_millis(140)).await;
        let second_wave = server.received_requests().await.unwrap().len();
        assert!(
            (COPILOT_POLICY_CONCURRENCY + 1..=COPILOT_POLICY_CONCURRENCY * 2)
                .contains(&second_wave),
            "second wave should be bounded: {second_wave}"
        );

        handle.await.unwrap().unwrap();
        let requests = server.received_requests().await.unwrap();
        assert_eq!(requests.len(), ids.len());
        for request in requests {
            assert_eq!(request.method.as_str(), "POST");
            assert_eq!(
                request.headers["authorization"].to_str().unwrap(),
                "Bearer token"
            );
            assert_eq!(
                request.headers["openai-intent"].to_str().unwrap(),
                "chat-policy"
            );
            assert_eq!(
                request.headers["x-interaction-type"].to_str().unwrap(),
                "chat-policy"
            );
            assert_eq!(
                request.body_json::<serde_json::Value>().unwrap(),
                json!({"state":"enabled"})
            );
        }
    }
}
