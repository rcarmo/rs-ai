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
}
