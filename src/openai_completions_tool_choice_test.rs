//! Test-for-test port of upstream `test/openai-completions-tool-choice.test.ts`
//! (`@earendil-works/pi-ai` v0.80.2).
//!
//! Payload-shape cases are asserted against the directly-callable
//! `build_payload` (rs-ai's equivalent of upstream's `onPayload`-captured
//! request body); stream-consumption cases drive `stream_openai` over a wiremock
//! SSE endpoint with the upstream chunk fixtures; metadata cases read the
//! catalog model's compat. Identical model ids / inputs / expected values.

#[cfg(test)]
mod tests {
    use crate::compat::detect_compat;
    use crate::provider::openai::{build_payload, stream_openai};
    use crate::registry::get_model;
    use crate::types::{Context, ContentBlock, Message, Model, Role, StreamOptions, ThinkingLevel, Tool};
    use crate::events::Event;
    use serde_json::{json, Value};
    use tokio_stream::StreamExt;
    use wiremock::{Mock, MockServer, ResponseTemplate};
    use wiremock::matchers::method;

    fn cat(provider: &str, id: &str) -> Model {
        get_model(provider, id).unwrap_or_else(|| panic!("missing catalog model {provider}/{id}"))
    }

    fn user_ctx() -> Context {
        Context {
            system_prompt: None, tools: Vec::new(),
            messages: vec![msg(Role::User, "Hi")],
        }
    }

    fn sys_ctx() -> Context {
        Context {
            system_prompt: Some("Follow instructions.".into()), tools: Vec::new(),
            messages: vec![msg(Role::User, "Hi")],
        }
    }

    fn msg(role: Role, text: &str) -> Message {
        Message {
            role,
            content: vec![ContentBlock::Text { text: text.into(), text_signature: None }],
            timestamp: 0, api: None, provider: None, model: None, response_id: None,
            response_model: None, diagnostics: Vec::new(), usage: None,
            stop_reason: None, error_message: None,
            tool_call_id: None, tool_name: None, is_error: false, details: None,
        }
    }

    fn ping_tool() -> Tool {
        Tool { name: "ping".into(), description: "Ping tool".into(),
            parameters: json!({"type": "object", "properties": {"ok": {"type": "boolean"}}}) }
    }

    fn payload(model: &Model, ctx: &Context, opts: &StreamOptions) -> Value {
        build_payload(model, ctx, opts, &detect_compat(model))
    }

    fn reasoning(level: ThinkingLevel) -> StreamOptions {
        StreamOptions { reasoning: Some(level), ..Default::default() }
    }

    // --- payload shape ---

    #[test]
    fn forwards_tool_choice_from_simple_options_to_payload() {
        let mut ctx = user_ctx();
        ctx.tools = vec![ping_tool()];
        let opts = StreamOptions { tool_choice: Some(json!("required")), ..Default::default() };
        let p = payload(&cat("openai", "gpt-4o-mini"), &ctx, &opts);
        assert_eq!(p["tool_choice"], json!("required"));
        assert!(p["tools"].as_array().is_some_and(|a| !a.is_empty()));
    }

    #[test]
    fn omits_strict_when_compat_disables_strict_mode() {
        let mut model = cat("openai", "gpt-4o-mini");
        model.compat.supports_strict_mode = Some(false);
        let mut ctx = user_ctx();
        ctx.tools = vec![ping_tool()];
        let p = payload(&model, &ctx, &StreamOptions::default());
        let func = &p["tools"][0]["function"];
        assert!(func.get("strict").is_none(), "strict must be omitted: {func}");
    }

    #[test]
    fn maps_groq_qwen3_reasoning_levels_to_default_reasoning_effort() {
        let p = payload(&cat("groq", "qwen/qwen3-32b"), &user_ctx(), &reasoning(ThinkingLevel::Medium));
        assert_eq!(p["reasoning_effort"], json!("default"));
    }

    #[test]
    fn keeps_normal_reasoning_effort_for_groq_models_without_compat_mapping() {
        let p = payload(&cat("groq", "openai/gpt-oss-20b"), &user_ctx(), &reasoning(ThinkingLevel::Medium));
        assert_eq!(p["reasoning_effort"], json!("medium"));
    }

    #[test]
    fn enables_tool_stream_for_supported_zai_models_with_tools() {
        let mut ctx = user_ctx();
        ctx.tools = vec![ping_tool()];
        let p = payload(&cat("zai", "glm-5.1"), &ctx, &StreamOptions::default());
        assert_eq!(p["tool_stream"], json!(true));
    }

    #[test]
    fn omits_tool_stream_for_unsupported_zai_models() {
        let mut ctx = user_ctx();
        ctx.tools = vec![ping_tool()];
        let p = payload(&cat("zai", "glm-4.5-air"), &ctx, &StreamOptions::default());
        assert!(p.get("tool_stream").is_none());
    }

    #[test]
    fn omits_tool_stream_when_no_tools_are_provided() {
        let p = payload(&cat("zai", "glm-5.1"), &user_ctx(), &StreamOptions::default());
        assert!(p.get("tool_stream").is_none());
    }

    #[test]
    fn maps_zai_glm_5_2_thinking_levels_to_reasoning_effort() {
        for (level, effort) in [
            (ThinkingLevel::Low, "high"),
            (ThinkingLevel::Medium, "high"),
            (ThinkingLevel::High, "high"),
            (ThinkingLevel::XHigh, "max"),
        ] {
            let p = payload(&cat("zai", "glm-5.2"), &user_ctx(), &reasoning(level.clone()));
            assert_eq!(p["thinking"], json!({"type": "enabled"}), "level {level:?}");
            assert_eq!(p["reasoning_effort"], json!(effort), "level {level:?}");
        }
    }

    #[test]
    fn omits_zai_glm_5_2_reasoning_effort_when_thinking_is_off() {
        let p = payload(&cat("zai", "glm-5.2"), &user_ctx(), &StreamOptions::default());
        assert_eq!(p["thinking"], json!({"type": "disabled"}));
        assert!(p.get("reasoning_effort").is_none());
    }

    #[test]
    fn sends_thinking_disabled_for_opencode_go_kimi_when_off() {
        let p = payload(&cat("opencode-go", "kimi-k2.6"), &user_ctx(), &StreamOptions::default());
        assert_eq!(p["thinking"], json!({"type": "disabled"}));
        assert!(p.get("reasoning_effort").is_none());
    }

    #[test]
    fn sends_thinking_enabled_for_opencode_go_kimi_when_enabled() {
        let p = payload(&cat("opencode-go", "kimi-k2.6"), &user_ctx(), &reasoning(ThinkingLevel::High));
        assert_eq!(p["thinking"], json!({"type": "enabled"}));
        assert!(p.get("reasoning_effort").is_none());
    }

    #[test]
    fn omits_reasoning_effort_for_opencode_grok_build() {
        let p = payload(&cat("opencode", "grok-build-0.1"), &user_ctx(), &reasoning(ThinkingLevel::High));
        assert!(p.get("reasoning_effort").is_none());
    }

    #[test]
    fn sends_max_tokens_for_opencode_completions_models() {
        for model in [cat("opencode-go", "kimi-k2.6"), cat("opencode", "grok-build-0.1")] {
            assert_eq!(model.compat.max_tokens_field.as_deref(), Some("max_tokens"));
            let opts = StreamOptions { max_tokens: Some(123), ..Default::default() };
            let p = payload(&model, &user_ctx(), &opts);
            assert_eq!(p["max_tokens"], json!(123));
            assert!(p.get("max_completion_tokens").is_none());
        }
    }

    #[test]
    fn uses_openrouter_reasoning_object_instead_of_reasoning_effort() {
        let p = payload(&cat("openrouter", "deepseek/deepseek-r1"), &user_ctx(), &reasoning(ThinkingLevel::High));
        assert_eq!(p["reasoning"], json!({"effort": "high"}));
        assert!(p.get("reasoning_effort").is_none());
    }

    #[test]
    fn keeps_developer_messages_for_openai_reasoning_model_instructions() {
        let p = payload(&cat("openai", "gpt-5.5"), &sys_ctx(), &StreamOptions::default());
        assert_eq!(p["messages"][0]["role"], json!("developer"));
    }

    // --- metadata ---

    #[test]
    fn stores_zai_tool_stream_support_in_model_compat_metadata() {
        assert_eq!(cat("zai", "glm-5.1").compat.zai_tool_stream, Some(true));
        assert_eq!(cat("zai", "glm-4.7").compat.zai_tool_stream, Some(true));
        assert_eq!(cat("zai", "glm-5-turbo").compat.zai_tool_stream, Some(true));
        assert_eq!(cat("zai", "glm-4.5-air").compat.zai_tool_stream, None);
    }

    #[test]
    fn stores_zai_glm_5_2_effort_metadata() {
        for provider in ["zai", "zai-coding-cn"] {
            let m = cat(provider, "glm-5.2");
            assert_eq!(m.compat.supports_reasoning_effort, Some(true));
        }
    }

    // --- stream consumption (upstream mockState.chunks fixtures) ---

    /// Returns (terminal stop reason, error string if any, terminal message),
    /// mapping rs-ai's event-level Error to upstream's response.stopReason/errorMessage.
    async fn run_openai_chunks(model: Model, body: String) -> (crate::types::StopReason, Option<String>, crate::types::Message) {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_string(body))
            .mount(&server)
            .await;
        let mut model = model;
        model.base_url = server.uri();
        model.api_key = Some("test".into());
        let ctx = user_ctx();
        let opts = StreamOptions::default();
        let mut stream = stream_openai(&model, &ctx, &opts);
        let mut out = None;
        while let Some(evt) = stream.next().await {
            match evt {
                Event::Done { reason, message } => out = Some((reason, message.error_message.clone(), message)),
                Event::Error { reason, error, message } => {
                    out = Some((reason, Some(error.to_string()), message.unwrap_or_else(|| msg(Role::Assistant, ""))));
                }
                _ => {}
            }
        }
        out.expect("a terminal event")
    }

    #[tokio::test]
    async fn maps_non_standard_finish_reason_to_stop_reason_error() {
        let body = concat!(
            "data: {\"choices\":[{\"delta\":{\"content\":\"partial\"},\"finish_reason\":null}]}\n\n",
            "data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"network_error\"}],\"usage\":{\"prompt_tokens\":1,\"completion_tokens\":1,\"prompt_tokens_details\":{\"cached_tokens\":0},\"completion_tokens_details\":{\"reasoning_tokens\":0}}}\n\n",
            "data: [DONE]\n\n",
        ).to_string();
        let (reason, err, _m) = run_openai_chunks(cat("openai", "gpt-4o-mini"), body).await;
        assert!(matches!(reason, crate::types::StopReason::Error));
        assert_eq!(err.as_deref(), Some("Provider finish_reason: network_error"));
    }

    #[tokio::test]
    async fn ignores_null_stream_chunks() {
        let body = concat!(
            "data: null\n\n",
            "data: {\"id\":\"chatcmpl-test\",\"choices\":[{\"delta\":{\"content\":\"OK\"},\"finish_reason\":null}]}\n\n",
            "data: {\"id\":\"chatcmpl-test\",\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":3,\"completion_tokens\":1,\"prompt_tokens_details\":{\"cached_tokens\":0},\"completion_tokens_details\":{\"reasoning_tokens\":0}}}\n\n",
            "data: [DONE]\n\n",
        ).to_string();
        let (reason, err, m) = run_openai_chunks(cat("openai", "gpt-4o-mini"), body).await;
        assert!(matches!(reason, crate::types::StopReason::Stop));
        assert!(err.is_none());
        assert_eq!(m.response_id.as_deref(), Some("chatcmpl-test"));
        assert_eq!(m.usage.as_ref().map(|u| u.total_tokens), Some(4));
        assert_eq!(m.content.len(), 1);
        assert!(matches!(&m.content[0], ContentBlock::Text { text, .. } if text == "OK"));
    }

    #[tokio::test]
    async fn errors_when_stream_ends_after_only_null_finish_reason_chunks() {
        let body = concat!(
            "data: {\"id\":\"x\",\"choices\":[{\"delta\":{\"content\":\"partial\"},\"finish_reason\":null}]}\n\n",
            "data: {\"id\":\"x\",\"choices\":[{\"delta\":{\"content\":\"partial\"},\"finish_reason\":null}]}\n\n",
            "data: [DONE]\n\n",
        ).to_string();
        let (reason, err, _m) = run_openai_chunks(cat("openai", "gpt-4o-mini"), body).await;
        assert!(matches!(reason, crate::types::StopReason::Error));
        assert_eq!(err.as_deref(), Some("Stream ended without finish_reason"));
    }

    #[tokio::test]
    async fn does_not_double_count_reasoning_tokens_in_completion_usage() {
        let body = concat!(
            "data: {\"id\":\"x\",\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":10,\"completion_tokens\":33,\"prompt_tokens_details\":{\"cached_tokens\":0},\"completion_tokens_details\":{\"reasoning_tokens\":21}}}\n\n",
            "data: [DONE]\n\n",
        ).to_string();
        let (_r, _e, m) = run_openai_chunks(cat("openai", "gpt-4o-mini"), body).await;
        let u = m.usage.unwrap();
        assert_eq!(u.input, 10);
        assert_eq!(u.output, 33);
        assert_eq!(u.total_tokens, 43);
    }

    #[tokio::test]
    async fn preserves_prompt_tokens_details_cache_read_write_fields() {
        let body = concat!(
            "data: {\"id\":\"x\",\"choices\":[{\"delta\":{\"content\":\"OK\"},\"finish_reason\":null}]}\n\n",
            "data: {\"id\":\"x\",\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":100,\"completion_tokens\":5,\"prompt_tokens_details\":{\"cached_tokens\":50,\"cache_write_tokens\":30},\"completion_tokens_details\":{\"reasoning_tokens\":0}}}\n\n",
            "data: [DONE]\n\n",
        ).to_string();
        let (_r, _e, m) = run_openai_chunks(cat("openai", "gpt-4o-mini"), body).await;
        let u = m.usage.unwrap();
        assert_eq!(u.input, 20);
        assert_eq!(u.cache_read, 50);
        assert_eq!(u.cache_write, 30);
        assert_eq!(u.total_tokens, 105);
    }

    // --- chat-template thinking kwargs ---

    fn local_vllm(id: &str, compat: crate::types::ModelCompat, thinking_level_map: Option<std::collections::HashMap<String, Option<String>>>) -> Model {
        Model {
            id: id.into(), name: id.into(), api: "openai-completions".into(),
            provider: "local-vllm".into(), base_url: "http://localhost:8000/v1".into(),
            reasoning: true, thinking_level_map,
            input: vec!["text".into()], cost: crate::types::ModelCost::default(),
            context_window: 128000, max_tokens: 8192, headers: None, api_key: None, compat,
        }
    }

    #[test]
    fn uses_configurable_chat_template_boolean_thinking_kwargs() {
        let mut compat = crate::types::ModelCompat::default();
        compat.thinking_format = Some("chat-template".into());
        compat.supports_reasoning_effort = Some(false);
        compat.chat_template_kwargs = Some(json!({"thinking": {"$var": "thinking.enabled"}}));
        let model = local_vllm("deepseek-ai/DeepSeek-V3.1", compat, None);
        for (level, expected) in [(Some(ThinkingLevel::High), true), (None, false)] {
            let opts = StreamOptions { reasoning: level, ..Default::default() };
            let p = payload(&model, &user_ctx(), &opts);
            assert_eq!(p["chat_template_kwargs"], json!({"thinking": expected}));
            assert!(p.get("thinking").is_none());
            assert!(p.get("reasoning_effort").is_none());
        }
    }

    #[test]
    fn uses_qwen_chat_template_thinking_kwargs() {
        let mut compat = crate::types::ModelCompat::default();
        compat.thinking_format = Some("qwen-chat-template".into());
        compat.supports_reasoning_effort = Some(false);
        let model = local_vllm("Qwen/Qwen3-Coder", compat, None);
        for (level, expected) in [(Some(ThinkingLevel::High), true), (None, false)] {
            let opts = StreamOptions { reasoning: level, ..Default::default() };
            let p = payload(&model, &user_ctx(), &opts);
            assert_eq!(p["chat_template_kwargs"], json!({"enable_thinking": expected, "preserve_thinking": true}));
            assert!(p.get("reasoning_effort").is_none());
        }
    }

    #[test]
    fn uses_configurable_chat_template_effort_kwargs_with_static_kwargs() {
        let mut compat = crate::types::ModelCompat::default();
        compat.thinking_format = Some("chat-template".into());
        compat.supports_reasoning_effort = Some(false);
        compat.chat_template_kwargs = Some(json!({
            "preserve_thinking": true,
            "reasoning_effort": {"$var": "thinking.effort", "omitWhenOff": true},
        }));
        let mut map = std::collections::HashMap::new();
        map.insert("xhigh".to_string(), Some("max".to_string()));
        let model = local_vllm("unsloth/gpt-oss-120b-GGUF", compat, Some(map));
        let opts = StreamOptions { reasoning: Some(ThinkingLevel::XHigh), ..Default::default() };
        let p = payload(&model, &user_ctx(), &opts);
        assert_eq!(p["chat_template_kwargs"], json!({"preserve_thinking": true, "reasoning_effort": "max"}));
        assert!(p.get("reasoning_effort").is_none());
    }

    // --- moonshot kimi metadata/payload ---

    #[test]
    fn omits_disabled_thinking_for_moonshot_kimi_k2_7_code_models() {
        for model in [cat("moonshotai", "kimi-k2.7-code"), cat("moonshotai-cn", "kimi-k2.7-code")] {
            let p = payload(&model, &user_ctx(), &StreamOptions::default());
            assert!(p.get("thinking").is_none(), "k2.7-code must omit disabled thinking");
            assert!(p.get("reasoning_effort").is_none());
        }
    }

    #[test]
    fn keeps_disabled_thinking_for_moonshot_kimi_k2_6_when_off() {
        let p = payload(&cat("moonshotai-cn", "kimi-k2.6"), &user_ctx(), &StreamOptions::default());
        assert_eq!(p["thinking"], json!({"type": "disabled"}));
        assert!(p.get("reasoning_effort").is_none());
    }

    #[test]
    fn stores_xiaomi_mimo_reasoning_replay_compat_metadata() {
        for provider in ["xiaomi", "xiaomi-token-plan-cn", "xiaomi-token-plan-ams", "xiaomi-token-plan-sgp"] {
            let m = cat(provider, "mimo-v2.5-pro");
            assert_eq!(m.compat.requires_reasoning_content_on_assistant_messages, Some(true));
            assert_eq!(m.compat.thinking_format.as_deref(), Some("deepseek"));
        }
    }

    #[test]
    fn uses_ant_ling_compatibility_metadata() {
        let m = cat("ant-ling", "Ring-2.6-1T");
        assert_eq!(m.compat.supports_developer_role, Some(false));
        assert_eq!(m.compat.supports_reasoning_effort, Some(false));
        assert_eq!(m.compat.max_tokens_field.as_deref(), Some("max_tokens"));
        assert_eq!(m.compat.thinking_format.as_deref(), Some("ant-ling"));
        // System prompt stays role "system" (developer role unsupported).
        let p = payload(&m, &sys_ctx(), &reasoning(ThinkingLevel::High));
        assert_eq!(p["messages"][0]["role"], json!("system"));
    }

    // --- tool-call delta coalescing by stable index ---

    #[tokio::test]
    async fn coalesces_tool_call_deltas_by_stable_index_when_provider_mutates_ids() {
        let body = concat!(
            "data: {\"id\":\"k\",\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"functions.read:0\",\"type\":\"function\",\"function\":{\"name\":\"read\",\"arguments\":\"\"}}]},\"finish_reason\":null}]}\n\n",
            "data: {\"id\":\"k\",\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"chatcmpl-tool-a\",\"type\":\"function\",\"function\":{\"name\":null,\"arguments\":\"{\\\"path\\\":\\\"README\"}}]},\"finish_reason\":null}]}\n\n",
            "data: {\"id\":\"k\",\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"chatcmpl-tool-b\",\"type\":\"function\",\"function\":{\"name\":null,\"arguments\":\".md\\\"}\"}}]},\"finish_reason\":\"tool_calls\"}],\"usage\":{\"prompt_tokens\":10,\"completion_tokens\":5,\"prompt_tokens_details\":{\"cached_tokens\":0},\"completion_tokens_details\":{\"reasoning_tokens\":0}}}\n\n",
            "data: [DONE]\n\n",
        ).to_string();
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).insert_header("content-type", "text/event-stream").set_body_string(body))
            .mount(&server).await;
        let mut model = cat("openai", "gpt-4o-mini");
        model.base_url = server.uri();
        model.api_key = Some("test".into());
        let mut ctx = user_ctx();
        ctx.tools = vec![Tool { name: "read".into(), description: "Read a file".into(), parameters: json!({"type":"object","properties":{"path":{"type":"string"}}}) }];
        let opts = StreamOptions::default();
        let mut stream = stream_openai(&model, &ctx, &opts);
        let mut indexes: Vec<usize> = Vec::new();
        let mut message = None;
        while let Some(evt) = stream.next().await {
            match evt {
                Event::ToolCallStart { .. } => indexes.push(0),
                Event::ToolCallDelta { .. } => indexes.push(0),
                Event::ToolCallEnd { .. } => indexes.push(0),
                Event::Done { message: m, .. } => message = Some(m),
                _ => {}
            }
        }
        let m = message.expect("Done");
        assert!(matches!(m.stop_reason, Some(crate::types::StopReason::ToolUse)));
        assert_eq!(indexes, vec![0, 0, 0, 0, 0], "all tool events on stable content index 0");
        assert_eq!(m.content.len(), 1);
        match &m.content[0] {
            ContentBlock::ToolCall { id, name, arguments, .. } => {
                assert_eq!(id, "functions.read:0", "first id is kept across mutations");
                assert_eq!(name, "read");
                assert_eq!(arguments.get("path").and_then(|v| v.as_str()), Some("README.md"));
            }
            other => panic!("expected toolCall, got {other:?}"),
        }
    }
}
