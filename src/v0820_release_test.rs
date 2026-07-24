//! v0.82.0 release-delta tests: generated model data invariants, image additions,
//! Qwen Token Plan providers, shared text/uuid utilities, and OpenCode Go Responses.

#[cfg(test)]
mod tests {
    use crate::events::Event;
    use crate::provider::responses::stream_responses;
    use crate::registry::{get_model, list_models, list_providers};
    use crate::types::{ContentBlock, Context, Message, Role, StreamOptions};
    use serde_json::Value;
    use std::collections::HashSet;
    use tokio_stream::StreamExt;
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn generated_model_data_is_structurally_valid_and_v0820_counts_match() {
        let all = crate::models_generated::builtin_models();
        let pairs = all
            .iter()
            .map(|m| (m.provider.as_str(), m.id.as_str()))
            .collect::<HashSet<_>>();
        assert_eq!(pairs.len(), 1116);
        let provider_count = all
            .iter()
            .map(|m| m.provider.as_str())
            .collect::<HashSet<_>>()
            .len();
        assert_eq!(provider_count, 37);
        for model in &all {
            assert!(!model.id.is_empty(), "empty id: {model:?}");
            assert!(!model.provider.is_empty(), "empty provider: {model:?}");
            assert!(
                !model.api.is_empty(),
                "empty api: {}/{}",
                model.provider,
                model.id
            );
            assert!(
                !model.name.is_empty(),
                "empty name: {}/{}",
                model.provider,
                model.id
            );
            assert!(
                model.context_window > 0,
                "bad context window: {}/{}",
                model.provider,
                model.id
            );
            assert!(
                model.max_tokens > 0,
                "bad max tokens: {}/{}",
                model.provider,
                model.id
            );
            assert!(
                !model.input.is_empty(),
                "bad input: {}/{}",
                model.provider,
                model.id
            );
            assert!(model.input.iter().all(|i| i == "text" || i == "image"));
            assert!(model.cost.input.is_finite());
            assert!(model.cost.output.is_finite());
            assert!(model.cost.cache_read.is_finite());
            assert!(model.cost.cache_write.is_finite());
        }
    }

    #[test]
    fn image_model_catalog_includes_v0820_openrouter_additions() {
        let ids = crate::images::list_image_models(Some("openrouter"))
            .into_iter()
            .map(|m| m.id)
            .collect::<HashSet<_>>();
        assert_eq!(ids.len(), 40);
        for id in [
            "krea/krea-2-large",
            "krea/krea-2-medium",
            "krea/krea-2-medium-turbo",
            "openrouter/auto-beta",
            "microsoft/mai-image-2.5-pro",
        ] {
            assert!(ids.contains(id), "missing image model {id}");
        }
    }

    #[test]
    fn qwen_token_plan_providers_and_env_keys_are_registered() {
        assert!(list_providers().contains(&"qwen-token-plan".to_string()));
        assert!(list_providers().contains(&"qwen-token-plan-cn".to_string()));
        assert!(!list_models(Some("qwen-token-plan")).is_empty());
        assert!(!list_models(Some("qwen-token-plan-cn")).is_empty());
        assert_eq!(
            crate::env::api_key_env_vars("qwen-token-plan"),
            Some(&["QWEN_TOKEN_PLAN_API_KEY"][..])
        );
        assert_eq!(
            crate::env::api_key_env_vars("qwen-token-plan-cn"),
            Some(&["QWEN_TOKEN_PLAN_CN_API_KEY"][..])
        );
    }

    #[test]
    fn shared_retry_overflow_text_and_uuid_utilities_match_release_invariants() {
        let mut err_msg = Message {
            role: Role::Assistant,
            content: Vec::new(),
            timestamp: 0,
            api: None,
            provider: None,
            model: None,
            response_id: None,
            response_model: None,
            diagnostics: Vec::new(),
            usage: None,
            stop_reason: Some(crate::types::StopReason::Error),
            error_message: Some("stream ended before a terminal response event".into()),
            tool_call_id: None,
            tool_name: None,
            is_error: true,
            details: None,
            added_tool_names: Vec::new(),
        };
        assert!(crate::retry::is_retryable_assistant_error(&err_msg));
        err_msg.error_message = Some("Range of input length should be [1, 100000]".into());
        let model = get_model("qwen-token-plan", "qwen-plus-latest").unwrap_or_else(|| {
            list_models(Some("qwen-token-plan"))
                .into_iter()
                .next()
                .unwrap()
        });
        assert!(crate::context::is_context_overflow(&err_msg, &model));
        let content = vec![
            ContentBlock::Text {
                text: "a".into(),
                text_signature: None,
            },
            ContentBlock::Image {
                data: "x".into(),
                mime_type: "image/png".into(),
            },
            ContentBlock::Text {
                text: "b".into(),
                text_signature: None,
            },
        ];
        assert_eq!(crate::utils::content_text(&content, "\n"), "a\nb");
        assert_eq!(crate::utils::content_text(&content, " "), "a b");
        let a = crate::utils::uuidv7();
        let b = crate::utils::uuidv7();
        assert_eq!(a.len(), 36);
        assert_eq!(&a[14..15], "7", "version nibble");
        assert!(
            matches!(&a[19..20], "8" | "9" | "a" | "b"),
            "variant nibble"
        );
        assert!(
            a < b,
            "uuidv7 should be time/sequence ordered for sequential calls: {a} !< {b}"
        );
    }

    fn ctx() -> Context {
        Context {
            system_prompt: None,
            tools: Vec::new(),
            messages: vec![Message {
                role: Role::User,
                content: vec![ContentBlock::Text {
                    text: "hi".into(),
                    text_signature: None,
                }],
                timestamp: 0,
                api: None,
                provider: None,
                model: None,
                response_id: None,
                response_model: None,
                diagnostics: Vec::new(),
                usage: None,
                stop_reason: None,
                error_message: None,
                tool_call_id: None,
                tool_name: None,
                is_error: false,
                details: None,
                added_tool_names: Vec::new(),
            }],
        }
    }

    #[test]
    fn json_schema_constrained_sampling_resolves_strict_mode() {
        let tool = crate::types::Tool {
            name: "emit".into(),
            description: "emit json".into(),
            parameters: serde_json::json!({"type":"object"}),
            constrained_sampling: Some(serde_json::json!({"type":"json_schema","strict":"prefer"})),
        };
        assert_eq!(
            crate::utils::resolve_json_schema_strict_sampling(&tool, true).unwrap(),
            Some(true)
        );
        assert_eq!(
            crate::utils::resolve_json_schema_strict_sampling(&tool, false).unwrap(),
            None
        );
        let required = crate::types::Tool {
            constrained_sampling: Some(
                serde_json::json!({"type":"json_schema","strict":"require"}),
            ),
            ..tool
        };
        assert!(
            crate::utils::resolve_json_schema_strict_sampling(&required, false)
                .unwrap_err()
                .contains("strict tools are unsupported")
        );
    }

    #[test]
    fn grammar_constrained_sampling_builds_custom_responses_tool() {
        let tool = crate::types::Tool {
            name: "emit".into(),
            description: "emit grammar".into(),
            parameters: serde_json::json!({"type":"object","properties":{"input":{"type":"string"}},"required":["input"]}),
            constrained_sampling: Some(
                serde_json::json!({"type":"grammar","variants":{"openai_lark":"start: /[a-z]+/"}}),
            ),
        };
        let grammar = crate::utils::resolve_grammar_constrained_sampling(&tool, true)
            .unwrap()
            .unwrap();
        assert_eq!(grammar.input_property, "input");
        let mut buffer = crate::utils::GrammarToolInputJsonBuffer::default();
        assert_eq!(
            crate::utils::append_grammar_tool_input_json_delta(&mut buffer, "input", "ab", false)
                .unwrap(),
            Some("{\"input\":\"ab".into())
        );
        assert_eq!(
            crate::utils::append_grammar_tool_input_json_delta(&mut buffer, "input", "abc", true)
                .unwrap(),
            Some("c\"}".into())
        );
        let mut model = get_model("openai", "gpt-5-mini").unwrap();
        model.compat.supports_openai_grammar_tools = Some(true);
        let payload = crate::provider::responses::build_responses_payload(
            &model,
            &Context {
                system_prompt: None,
                messages: Vec::new(),
                tools: vec![tool],
            },
            &StreamOptions::default(),
        );
        assert_eq!(payload["tools"][0]["type"], "custom");
        assert_eq!(payload["tools"][0]["format"]["type"], "grammar");
        assert_eq!(payload["tools"][0]["format"]["syntax"], "lark");
    }

    #[test]
    fn pipe_delimited_tool_call_ids_preserve_item_uniqueness() {
        let a =
            crate::provider::openai::normalize_tool_call_id("call_same|item_a", "qwen-token-plan");
        let b =
            crate::provider::openai::normalize_tool_call_id("call_same|item_b", "qwen-token-plan");
        assert_eq!(a, "call_same_item_a");
        assert_eq!(b, "call_same_item_b");
        assert_ne!(a, b);
        let long = crate::provider::openai::normalize_tool_call_id(
            &format!("{}|{}", "c".repeat(80), "i".repeat(80)),
            "qwen-token-plan",
        );
        assert!(long.chars().count() <= 40);
        assert!(long.contains('_'));
    }

    #[test]
    fn builtin_runtime_wires_oauth_providers_for_openrouter_and_kimi() {
        let runtime = crate::models_runtime::ModelsRuntime::new();
        runtime.populate_builtin_fallbacks();
        assert!(runtime.provider_has_oauth("openrouter"));
        assert!(runtime.provider_has_oauth("kimi-coding"));
    }

    #[tokio::test]
    async fn openrouter_and_kimi_oauth_helpers_cover_release_behaviour() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"key":"or-key"})),
            )
            .mount(&server)
            .await;
        let openrouter =
            crate::oauth::exchange_openrouter_code_at(&server.uri(), "code", "verifier")
                .await
                .unwrap();
        assert_eq!(openrouter.access, "or-key");
        assert_eq!(openrouter.expires, i64::MAX);

        let server = MockServer::start().await;
        Mock::given(method("POST")).and(wiremock::matchers::path("/api/oauth/device_authorization"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "device_code":"dev", "user_code":"USER", "verification_uri":"https://kimi.com/device",
                "verification_uri_complete":"https://kimi.com/device?user_code=USER", "interval":1, "expires_in":60
            }))).mount(&server).await;
        Mock::given(method("POST")).and(wiremock::matchers::path("/api/oauth/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"access_token":"kimi-access","refresh_token":"kimi-refresh","expires_in":60})))
            .mount(&server).await;
        let device = crate::oauth::request_kimi_device_authorization_at(&server.uri())
            .await
            .unwrap();
        assert_eq!(
            device.verification_uri_complete,
            "https://kimi.com/device?user_code=USER"
        );
        let refreshed = crate::oauth::refresh_kimi_code_token_at(&server.uri(), "old-refresh")
            .await
            .unwrap();
        assert_eq!(refreshed.access, "kimi-access");
    }

    #[tokio::test]
    async fn opencode_go_grok_45_uses_openai_responses_request_path() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).insert_header("content-type", "text/event-stream").set_body_string(
                "data: {\"type\":\"response.completed\",\"response\":{\"id\":\"r\",\"status\":\"completed\",\"usage\":{\"input_tokens\":1,\"output_tokens\":1,\"total_tokens\":2}}}\n\ndata: [DONE]\n\n",
            ))
            .mount(&server)
            .await;
        let mut model = get_model("opencode-go", "grok-4.5").expect("opencode-go/grok-4.5");
        assert_eq!(model.api, crate::types::api::OPENAI_RESPONSES);
        model.base_url = server.uri();
        model.api_key = Some("opencode-key".into());
        let c = ctx();
        let opts = StreamOptions::default();
        let mut stream = stream_responses(&model, &c, &opts);
        while let Some(event) = stream.next().await {
            if let Event::Error { error, .. } = event {
                panic!("unexpected error: {error}");
            }
        }
        let reqs = server.received_requests().await.unwrap();
        assert_eq!(reqs[0].url.path(), "/responses");
        assert_eq!(
            reqs[0]
                .headers
                .get("authorization")
                .unwrap()
                .to_str()
                .unwrap(),
            "Bearer opencode-key"
        );
        let body: Value = serde_json::from_slice(&reqs[0].body).unwrap();
        assert_eq!(body["model"], "grok-4.5");
    }
}
