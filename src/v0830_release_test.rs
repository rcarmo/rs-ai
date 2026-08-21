//! v0.83.0 release-delta tests: generated model data invariants, image additions,
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
    fn generated_model_data_is_structurally_valid_and_current_counts_match() {
        let all = crate::models_generated::builtin_models();
        let pairs = all
            .iter()
            .map(|m| (m.provider.as_str(), m.id.as_str()))
            .collect::<HashSet<_>>();
        assert_eq!(pairs.len(), 1267);
        let provider_count = all
            .iter()
            .map(|m| m.provider.as_str())
            .collect::<HashSet<_>>()
            .len();
        assert_eq!(provider_count, 39);
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
    fn opus_5_bedrock_and_anthropic_metadata_is_present() {
        let anthropic = get_model("anthropic", "claude-opus-5").expect("anthropic opus 5");
        assert_eq!(
            anthropic.thinking_level_map.as_ref().unwrap().get("xhigh"),
            Some(&Some("xhigh".into()))
        );
        assert_eq!(
            anthropic.thinking_level_map.as_ref().unwrap().get("max"),
            Some(&Some("max".into()))
        );
        let bedrock = get_model("amazon-bedrock", "us.anthropic.claude-opus-5")
            .expect("bedrock opus 5 profile");
        assert!(bedrock.id.contains("opus-5"));
        assert_eq!(
            bedrock.thinking_level_map.as_ref().unwrap().get("xhigh"),
            Some(&Some("xhigh".into()))
        );
    }

    #[test]
    fn image_model_catalog_includes_v0830_openrouter_additions() {
        let ids = crate::images::list_image_models(Some("openrouter"))
            .into_iter()
            .map(|m| m.id)
            .collect::<HashSet<_>>();
        assert_eq!(ids.len(), 45);
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
    fn v0830_auth_error_cause_etag_and_anthropic_auth_token_behaviour() {
        let err = crate::auth::ModelsError::with_cause(
            crate::auth::ModelsErrorCode::Auth,
            "Credential store read failed for anthropic",
            "disk offline",
        );
        assert_eq!(
            err.to_string(),
            "Credential store read failed for anthropic: disk offline"
        );
        let entry = crate::models_runtime::ModelsStoreEntry {
            models: Vec::new(),
            last_modified: Some(123),
            checked_at: Some(456),
            etag: Some("\"abc\"".into()),
        };
        assert_eq!(entry.etag.as_deref(), Some("\"abc\""));
        assert_eq!(entry.last_modified, Some(123));
        assert_eq!(
            crate::env::api_key_env_vars("anthropic").unwrap()[0],
            "ANTHROPIC_AUTH_TOKEN"
        );
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
            deferred: None,
            error_message: Some("stream ended before a terminal response event".into()),
            raw_stop_reason: None,
            end_turn: None,
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
                deferred: None,
                error_message: None,
                raw_stop_reason: None,
                end_turn: None,
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

    #[test]
    fn openai_completions_grammar_tool_request_shape_is_custom() {
        let tool = crate::types::Tool {
            name: "emit".into(),
            description: "emit grammar".into(),
            parameters: serde_json::json!({"type":"object","properties":{"input":{"type":"string"}},"required":["input"]}),
            constrained_sampling: Some(
                serde_json::json!({"type":"grammar","variants":{"openai_regex":"[a-z]+"}}),
            ),
        };
        let mut model = get_model("openai", "gpt-5-mini").unwrap();
        model.compat.supports_openai_grammar_tools = Some(true);
        let payload = crate::provider::openai::build_payload(
            &model,
            &Context {
                system_prompt: None,
                messages: Vec::new(),
                tools: vec![tool],
            },
            &StreamOptions::default(),
            &crate::compat::detect_compat(&model),
        );
        assert_eq!(payload["tools"][0]["type"], "custom");
        assert_eq!(payload["tools"][0]["format"]["syntax"], "regex");
    }

    #[test]
    fn codex_payload_uses_responses_grammar_custom_tool_shape() {
        let tool = crate::types::Tool {
            name: "emit".into(),
            description: "emit grammar".into(),
            parameters: serde_json::json!({"type":"object","properties":{"input":{"type":"string"}},"required":["input"]}),
            constrained_sampling: Some(
                serde_json::json!({"type":"grammar","variants":{"openai_regex":"[a-z]+"}}),
            ),
        };
        let mut model = get_model("openai-codex", "gpt-5.4-codex")
            .unwrap_or_else(|| get_model("openai-codex", "gpt-5.4").unwrap());
        model.compat.supports_openai_grammar_tools = Some(true);
        let payload = crate::provider::codex::build_codex_payload(
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
    async fn azure_responses_pending_terminal_status_is_error_with_raw_reason() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).insert_header("content-type", "text/event-stream").set_body_string("data: {\"type\":\"response.completed\",\"response\":{\"status\":\"queued\",\"usage\":{\"input_tokens\":1,\"output_tokens\":0,\"total_tokens\":1}}}\n\ndata: [DONE]\n\n"))
            .mount(&server).await;
        let mut model = get_model("azure-openai-responses", "gpt-5-mini")
            .unwrap_or_else(|| get_model("openai", "gpt-5-mini").unwrap());
        model.api = "azure-openai-responses".into();
        model.provider = "azure-openai-responses".into();
        model.base_url = server.uri();
        model.api_key = Some("k".into());
        let c = ctx();
        let opts = StreamOptions::default();
        let mut stream = stream_responses(&model, &c, &opts);
        let mut err = None;
        let mut done = false;
        while let Some(event) = stream.next().await {
            match event {
                Event::Error { message, .. } => err = message,
                Event::Done { .. } => done = true,
                _ => {}
            }
        }
        assert!(!done, "queued Azure terminal status must not emit Done");
        let msg = err.expect("queued terminal status must emit Error with message");
        assert_eq!(msg.stop_reason, Some(crate::types::StopReason::Error));
        assert_eq!(msg.raw_stop_reason.as_deref(), Some("queued"));
        assert_eq!(
            msg.error_message.as_deref(),
            Some("Response did not complete: queued")
        );
    }

    #[tokio::test]
    async fn azure_responses_stream_reconstructs_grammar_custom_tool_input() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).insert_header("content-type", "text/event-stream").set_body_string(concat!(
                "data: {\"type\":\"response.output_item.added\",\"item\":{\"type\":\"custom_tool_call\",\"call_id\":\"call_1\",\"id\":\"ctc_1\",\"name\":\"emit\",\"input\":\"\"}}\n\n",
                "data: {\"type\":\"response.custom_tool_call_input.delta\",\"delta\":\"ab\"}\n\n",
                "data: {\"type\":\"response.custom_tool_call_input.done\",\"input\":\"abc\"}\n\n",
                "data: {\"type\":\"response.output_item.done\",\"item\":{\"type\":\"custom_tool_call\",\"call_id\":\"call_1\",\"id\":\"ctc_1\",\"name\":\"emit\",\"input\":\"abc\"}}\n\n",
                "data: {\"type\":\"response.completed\",\"response\":{\"status\":\"completed\",\"usage\":{\"input_tokens\":1,\"output_tokens\":1,\"total_tokens\":2}}}\n\n",
                "data: [DONE]\n\n"
            )))
            .mount(&server).await;
        let mut model = get_model("azure-openai-responses", "gpt-5-mini")
            .unwrap_or_else(|| get_model("openai", "gpt-5-mini").unwrap());
        model.api = "azure-openai-responses".into();
        model.provider = "azure-openai-responses".into();
        model.base_url = server.uri();
        model.api_key = Some("k".into());
        model.compat.supports_openai_grammar_tools = Some(true);
        let tool = crate::types::Tool {
            name: "emit".into(),
            description: "emit".into(),
            parameters: serde_json::json!({"type":"object","properties":{"payload":{"type":"string"}},"required":["payload"]}),
            constrained_sampling: Some(
                serde_json::json!({"type":"grammar","variants":{"openai_regex":"[a-z]+"}}),
            ),
        };
        let c = Context {
            system_prompt: None,
            messages: Vec::new(),
            tools: vec![tool],
        };
        let opts = StreamOptions::default();
        let mut stream = stream_responses(&model, &c, &opts);
        let mut delta = String::new();
        let mut done_args = None;
        while let Some(event) = stream.next().await {
            match event {
                Event::ToolCallDelta { delta: d } => delta.push_str(&d),
                Event::ToolCallEnd { arguments, .. } => done_args = Some(arguments),
                Event::Error { error, .. } => panic!("unexpected error: {error}"),
                _ => {}
            }
        }
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&delta).unwrap(),
            serde_json::json!({"payload":"abc"})
        );
        assert_eq!(done_args.unwrap(), serde_json::json!({"payload":"abc"}));
        assert_eq!(
            server.received_requests().await.unwrap()[0].url.path(),
            "/responses"
        );
    }

    #[tokio::test]
    async fn radius_runtime_refresh_reuses_cached_catalog_on_etag_304() {
        use crate::models_runtime::ModelsStore;
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(304))
            .mount(&server)
            .await;
        let store = std::sync::Arc::new(crate::models_runtime::InMemoryModelsStore::new());
        let mut cached = get_model("radius", "auto").unwrap_or_else(|| crate::types::Model {
            id: "cached".into(),
            name: "cached".into(),
            api: "pi-messages".into(),
            provider: "radius".into(),
            base_url: "http://cached/v1".into(),
            reasoning: false,
            thinking_level_map: None,
            input: vec!["text".into()],
            cost: crate::types::ModelCost::default(),
            context_window: 10,
            max_tokens: 5,
            sampling_params: None,
            headers: None,
            api_key: None,
            compat: Default::default(),
        });
        cached.id = "cached".into();
        store
            .write(
                "radius",
                crate::models_runtime::ModelsStoreEntry {
                    models: vec![cached],
                    last_modified: Some(1),
                    checked_at: Some(1),
                    etag: Some("\"abc\"".into()),
                },
            )
            .await
            .unwrap();
        let runtime = crate::models_runtime::ModelsRuntime::with_models_store(store);
        runtime.set_provider(crate::models_runtime::RuntimeProvider::radius(
            "radius",
            "Radius",
            server.uri(),
            Vec::new(),
        ));
        let result = runtime
            .refresh(crate::models_runtime::RefreshOptions {
                allow_network: true,
                force: false,
                cancel: None,
                providers: None,
            })
            .await;
        assert!(result.errors.is_empty());
        assert!(runtime.get_model("radius", "cached").is_some());
        let req = server.received_requests().await.unwrap().pop().unwrap();
        assert_eq!(
            req.headers.get("if-none-match").unwrap().to_str().unwrap(),
            "\"abc\""
        );
    }

    #[test]
    fn bedrock_raw_stop_reason_helper_errors_unknown_and_preserves_raw() {
        let mut msg = Message {
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
            stop_reason: Some(crate::types::StopReason::Pending),
            deferred: None,
            error_message: None,
            raw_stop_reason: None,
            end_turn: None,
            tool_call_id: None,
            tool_name: None,
            is_error: false,
            details: None,
            added_tool_names: Vec::new(),
        };
        crate::provider::bedrock::apply_bedrock_raw_stop_reason(&mut msg, "guardrail_intervened");
        assert_eq!(msg.raw_stop_reason.as_deref(), Some("guardrail_intervened"));
        assert_eq!(msg.stop_reason, Some(crate::types::StopReason::Error));
        assert_eq!(
            msg.error_message.as_deref(),
            Some("Provider stopped with: guardrail_intervened")
        );
    }

    #[tokio::test]
    async fn anthropic_auth_token_env_sends_bearer_not_x_api_key() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).insert_header("content-type", "text/event-stream").set_body_string("event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"m\",\"role\":\"assistant\",\"content\":[],\"model\":\"claude\",\"stop_reason\":null,\"usage\":{\"input_tokens\":1,\"output_tokens\":0}}}\n\nevent: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":0}}\n\nevent: message_stop\ndata: {\"type\":\"message_stop\"}\n\n"))
            .mount(&server).await;
        let mut model = get_model("anthropic", "claude-opus-5").unwrap();
        model.base_url = server.uri();
        model.api_key = None;
        unsafe {
            std::env::set_var("ANTHROPIC_AUTH_TOKEN", "auth-token");
        }
        let c = ctx();
        let opts = StreamOptions::default();
        let mut stream = crate::provider::anthropic::stream_anthropic(&model, &c, &opts);
        while let Some(event) = stream.next().await {
            if let Event::Error { error, .. } = event {
                panic!("unexpected error: {error}");
            }
        }
        unsafe {
            std::env::remove_var("ANTHROPIC_AUTH_TOKEN");
        }
        let req = server.received_requests().await.unwrap().pop().unwrap();
        assert_eq!(
            req.headers.get("authorization").unwrap().to_str().unwrap(),
            "Bearer auth-token"
        );
        assert!(req.headers.get("x-api-key").is_none());
    }

    #[tokio::test]
    async fn responses_pending_status_is_error_and_preserves_raw_stop_reason() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).insert_header("content-type", "text/event-stream").set_body_string("data: {\"type\":\"response.completed\",\"response\":{\"status\":\"queued\",\"usage\":{\"input_tokens\":1,\"output_tokens\":0,\"total_tokens\":1}}}\n\ndata: [DONE]\n\n"))
            .mount(&server).await;
        let mut model = get_model("openai", "gpt-5-mini").unwrap();
        model.base_url = server.uri();
        model.api_key = Some("k".into());
        let c = ctx();
        let opts = StreamOptions::default();
        let mut stream = stream_responses(&model, &c, &opts);
        let mut message = None;
        while let Some(event) = stream.next().await {
            if let Event::Error { message: m, .. } = event {
                message = m;
            }
        }
        let message = message.unwrap();
        assert_eq!(message.stop_reason, Some(crate::types::StopReason::Error));
        assert_eq!(message.raw_stop_reason.as_deref(), Some("queued"));
    }

    fn provider_model(api: &str, provider: &str, base_url: &str) -> crate::types::Model {
        crate::types::Model {
            id: "test-model".into(),
            name: "Test".into(),
            api: api.into(),
            provider: provider.into(),
            base_url: base_url.into(),
            reasoning: false,
            thinking_level_map: None,
            input: vec!["text".into()],
            cost: crate::types::ModelCost::default(),
            context_window: 128000,
            max_tokens: 4096,
            sampling_params: None,
            headers: None,
            api_key: Some("k".into()),
            compat: Default::default(),
        }
    }

    #[tokio::test]
    async fn malformed_openai_delta_preserves_function_when_custom_is_empty() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).insert_header("content-type", "text/event-stream").set_body_string("data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_1\",\"type\":\"function\",\"function\":{\"name\":\"read\",\"arguments\":\"{\\\"path\\\":\\\"README.md\\\"}\"},\"custom\":{}}]},\"finish_reason\":\"tool_calls\"}]}\n\ndata: [DONE]\n\n"))
            .mount(&server).await;
        let model = provider_model("openai-completions", "openai", &server.uri());
        let c = ctx();
        let opts = StreamOptions::default();
        let mut stream = crate::provider::openai::stream_openai(&model, &c, &opts);
        let mut done = None;
        while let Some(event) = stream.next().await {
            if let Event::Done { message, .. } = event {
                done = Some(message);
            }
        }
        let msg = done.unwrap();
        match &msg.content[0] {
            ContentBlock::ToolCall {
                name, arguments, ..
            } => {
                assert_eq!(name, "read");
                assert_eq!(
                    arguments.get("path").and_then(|v| v.as_str()),
                    Some("README.md")
                );
            }
            other => panic!("expected tool call, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn anthropic_raw_stop_and_missing_stop_are_executable() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).insert_header("content-type", "text/event-stream").set_body_string("event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"m\",\"role\":\"assistant\",\"content\":[],\"model\":\"claude\",\"stop_reason\":null,\"usage\":{\"input_tokens\":1,\"output_tokens\":0}}}\n\nevent: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":0}}\n\nevent: message_stop\ndata: {\"type\":\"message_stop\"}\n\n"))
            .mount(&server).await;
        let model = provider_model("anthropic-messages", "anthropic", &server.uri());
        let c = ctx();
        let opts = StreamOptions::default();
        let mut stream = crate::provider::anthropic::stream_anthropic(&model, &c, &opts);
        let mut done = None;
        while let Some(event) = stream.next().await {
            if let Event::Done { message, .. } = event {
                done = Some(message);
            }
        }
        assert_eq!(done.unwrap().raw_stop_reason.as_deref(), Some("end_turn"));

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).insert_header("content-type", "text/event-stream").set_body_string("event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"m\",\"role\":\"assistant\",\"content\":[],\"model\":\"claude\",\"stop_reason\":null,\"usage\":{\"input_tokens\":1,\"output_tokens\":0}}}\n\nevent: message_stop\ndata: {\"type\":\"message_stop\"}\n\n"))
            .mount(&server).await;
        let model = provider_model("anthropic-messages", "anthropic", &server.uri());
        let mut stream = crate::provider::anthropic::stream_anthropic(&model, &c, &opts);
        let mut err = None;
        while let Some(event) = stream.next().await {
            if let Event::Error { message, .. } = event {
                err = message;
            }
        }
        assert_eq!(
            err.unwrap().stop_reason,
            Some(crate::types::StopReason::Error)
        );
    }

    #[tokio::test]
    async fn google_and_mistral_raw_stop_reasons_are_executable() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).insert_header("content-type", "text/event-stream").set_body_string("data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"x\"}]},\"finishReason\":\"SAFETY\"}],\"usageMetadata\":{\"promptTokenCount\":1,\"candidatesTokenCount\":1,\"totalTokenCount\":2}}\n\n"))
            .mount(&server).await;
        let model = provider_model("google-generative-ai", "google", &server.uri());
        let c = ctx();
        let opts = StreamOptions::default();
        let mut stream = crate::provider::google::stream_google(&model, &c, &opts);
        let mut err = None;
        while let Some(event) = stream.next().await {
            if let Event::Error { message, .. } = event {
                err = message;
            }
        }
        assert_eq!(err.unwrap().raw_stop_reason.as_deref(), Some("SAFETY"));

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).insert_header("content-type", "text/event-stream").set_body_string("data: {\"choices\":[{\"delta\":{\"content\":\"x\"},\"finish_reason\":\"length\"}],\"usage\":{\"prompt_tokens\":1,\"completion_tokens\":1,\"total_tokens\":2}}\n\ndata: [DONE]\n\n"))
            .mount(&server).await;
        let model = provider_model("mistral-conversations", "mistral", &server.uri());
        let mut stream = crate::provider::mistral::stream_mistral(&model, &c, &opts);
        let mut done = None;
        while let Some(event) = stream.next().await {
            if let Event::Done { message, .. } = event {
                done = Some(message);
            }
        }
        assert_eq!(done.unwrap().raw_stop_reason.as_deref(), Some("length"));
    }

    #[tokio::test]
    async fn codex_pending_status_is_error_and_raw() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).insert_header("content-type", "text/event-stream").set_body_string("data: {\"type\":\"response.completed\",\"response\":{\"status\":\"queued\",\"usage\":{\"input_tokens\":1,\"output_tokens\":0,\"total_tokens\":1}}}\n\ndata: [DONE]\n\n"))
            .mount(&server).await;
        let mut model = provider_model("openai-codex-responses", "openai-codex", &server.uri());
        model.api_key = Some("a.b.c".into());
        let c = ctx();
        let opts = StreamOptions {
            transport: Some(crate::types::Transport::Sse),
            ..Default::default()
        };
        let mut stream = crate::provider::codex::stream_codex(&model, &c, &opts);
        let mut err = None;
        let mut done = None;
        while let Some(event) = stream.next().await {
            match event {
                Event::Error { message, .. } => err = message,
                Event::Done { message, .. } => done = Some(message),
                _ => {}
            }
        }
        if let Some(msg) = err.or(done) {
            assert_eq!(msg.stop_reason, Some(crate::types::StopReason::Error));
            assert_eq!(msg.raw_stop_reason.as_deref(), Some("queued"));
        }
    }

    #[tokio::test]
    async fn openai_completions_preserves_raw_stop_reason() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).insert_header("content-type", "text/event-stream").set_body_string("data: {\"choices\":[{\"delta\":{\"content\":\"x\"},\"finish_reason\":null}]}\n\ndata: {\"choices\":[{\"delta\":{},\"finish_reason\":\"content_filter\"}]}\n\ndata: [DONE]\n\n"))
            .mount(&server).await;
        let mut model = get_model("openai", "gpt-5-mini").unwrap();
        model.base_url = server.uri();
        model.api_key = Some("k".into());
        let c = ctx();
        let opts = StreamOptions::default();
        let mut stream = crate::provider::openai::stream_openai(&model, &c, &opts);
        let mut message = None;
        while let Some(event) = stream.next().await {
            if let Event::Error { message: m, .. } = event {
                message = m;
            }
        }
        assert_eq!(
            message.unwrap().raw_stop_reason.as_deref(),
            Some("content_filter")
        );
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
