//! Test-for-test port of upstream `test/fireworks-models.test.ts`
//! (`@earendil-works/pi-ai` v0.80.2) — catalog/compat/env + the Anthropic
//! session-affinity / tool-compat integration cases.

#[cfg(test)]
mod tests {
    use crate::env::get_env_api_key;
    use crate::events::Event;
    use crate::provider::anthropic::stream_anthropic;
    use crate::registry::{get_model, list_models};
    use crate::types::{CacheRetention, ContentBlock, Context, Message, Role, StreamOptions, Tool};
    use serde_json::{Value, json};
    use tokio_stream::StreamExt;
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    const KIMI: &str = "accounts/fireworks/models/kimi-k2p6";

    // --- catalog / compat / env ---

    #[test]
    fn registers_default_kimi_k2_6_via_anthropic_messages() {
        let m = get_model("fireworks", KIMI).expect("fireworks kimi");
        assert_eq!(m.api, "anthropic-messages");
        assert_eq!(m.provider, "fireworks");
        assert_eq!(m.base_url, "https://api.fireworks.ai/inference");
        assert!(m.reasoning);
        assert_eq!(m.input, vec!["text".to_string(), "image".to_string()]);
        assert_eq!(m.context_window, 262000);
        assert_eq!(m.max_tokens, 262000);
        assert_eq!(
            (
                m.cost.input,
                m.cost.output,
                m.cost.cache_read,
                m.cost.cache_write
            ),
            (0.95, 4.0, 0.16, 0.0)
        );
    }

    #[test]
    fn registers_fire_pass_turbo_router_model() {
        let m = list_models(Some("fireworks"))
            .into_iter()
            .find(|c| c.id.starts_with("accounts/fireworks/routers/") && c.id.ends_with("-turbo"))
            .expect("a turbo router model");
        assert_eq!(m.api, "anthropic-messages");
        assert_eq!(m.base_url, "https://api.fireworks.ai/inference");
        assert_eq!(m.input, vec!["text".to_string(), "image".to_string()]);
    }

    #[test]
    fn resolves_fireworks_api_key_from_env() {
        unsafe {
            std::env::set_var("FIREWORKS_API_KEY", "test-fireworks-key");
        }
        let got = get_env_api_key("fireworks");
        unsafe {
            std::env::remove_var("FIREWORKS_API_KEY");
        }
        assert_eq!(got.as_deref(), Some("test-fireworks-key"));
    }

    #[test]
    fn sets_fireworks_specific_compat() {
        let m = get_model("fireworks", KIMI).unwrap();
        assert_eq!(m.compat.send_session_affinity_headers, Some(true));
        assert_eq!(m.compat.supports_eager_tool_input_streaming, Some(false));
        assert_eq!(m.compat.supports_cache_control_on_tools, Some(false));
        assert_eq!(m.compat.supports_long_cache_retention, Some(false));
    }

    // --- integration: header + tool payload ---

    fn lookup_tool() -> Tool {
        Tool {
            name: "lookup".into(),
            description: "Look up a value".into(),
            parameters: json!({"type": "object", "properties": {"value": {"type": "string"}}}),
        }
    }

    fn user_ctx() -> Context {
        Context {
            system_prompt: None,
            tools: vec![lookup_tool()],
            messages: vec![Message {
                role: Role::User,
                content: vec![ContentBlock::Text {
                    text: "Use the tool".into(),
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
            }],
        }
    }

    async fn capture(
        model_id_provider: (&str, &str),
        opts: StreamOptions,
    ) -> (Value, Option<String>) {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_string(""),
            )
            .mount(&server)
            .await;
        let mut model = get_model(model_id_provider.1, model_id_provider.0).unwrap();
        model.base_url = server.uri();
        model.api_key = Some("test".into());
        let ctx = user_ctx();
        let mut stream = stream_anthropic(&model, &ctx, &opts);
        while let Some(evt) = stream.next().await {
            if matches!(evt, Event::Done { .. } | Event::Error { .. }) {
                break;
            }
        }
        let reqs = server.received_requests().await.unwrap();
        let req = reqs.last().expect("a request");
        let body: Value = serde_json::from_slice(&req.body).unwrap();
        let affinity = req
            .headers
            .get("x-session-affinity")
            .map(|v| v.to_str().unwrap().to_string());
        (body, affinity)
    }

    fn fireworks() -> (&'static str, &'static str) {
        (KIMI, "fireworks")
    }
    fn native() -> (&'static str, &'static str) {
        ("claude-haiku-4-5", "anthropic")
    }

    fn opts_session(sid: &str, retention: Option<CacheRetention>) -> StreamOptions {
        StreamOptions {
            session_id: Some(sid.into()),
            cache_retention: retention,
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn sends_x_session_affinity_for_fireworks_models() {
        let (_b, affinity) = capture(fireworks(), opts_session("fireworks-session-1", None)).await;
        assert_eq!(affinity.as_deref(), Some("fireworks-session-1"));
    }

    #[tokio::test]
    async fn omits_x_session_affinity_for_native_anthropic_models() {
        let (_b, affinity) = capture(native(), opts_session("anthropic-session-1", None)).await;
        assert!(affinity.is_none());
    }

    #[tokio::test]
    async fn omits_x_session_affinity_when_cache_retention_none() {
        let (_b, affinity) = capture(
            fireworks(),
            opts_session("fireworks-session-2", Some(CacheRetention::None)),
        )
        .await;
        assert!(affinity.is_none());
    }

    #[tokio::test]
    async fn omits_cache_control_on_tools_for_fireworks_models() {
        let (body, _a) = capture(fireworks(), StreamOptions::default()).await;
        let tools = body["tools"].as_array().unwrap();
        assert!(tools.last().unwrap().get("cache_control").is_none());
    }

    #[tokio::test]
    async fn omits_eager_input_streaming_on_tools_for_fireworks_models() {
        let (body, _a) = capture(fireworks(), StreamOptions::default()).await;
        for t in body["tools"].as_array().unwrap() {
            assert!(t.get("eager_input_streaming").is_none());
        }
    }

    #[tokio::test]
    async fn sends_cache_control_on_tools_for_native_anthropic_models() {
        let (body, _a) = capture(native(), StreamOptions::default()).await;
        let last = body["tools"].as_array().unwrap().last().unwrap();
        assert_eq!(last["cache_control"]["type"], json!("ephemeral"));
    }

    #[tokio::test]
    async fn sends_eager_input_streaming_on_tools_for_native_anthropic_models() {
        let (body, _a) = capture(native(), StreamOptions::default()).await;
        assert_eq!(body["tools"][0]["eager_input_streaming"], json!(true));
    }

    #[test]
    fn aligns_glm_5_2_fast_with_glm_5_2_openai_compatible_config() {
        // v0.80.5: the GLM 5.2 Fast router mirrors GLM 5.2's OpenAI-compatible config.
        let base = get_model("fireworks", "accounts/fireworks/models/glm-5p2").expect("glm-5p2");
        let fast = get_model("fireworks", "accounts/fireworks/routers/glm-5p2-fast")
            .expect("glm-5p2-fast");
        assert_eq!(fast.api, base.api);
        assert_eq!(fast.base_url, base.base_url);
        assert_eq!(fast.compat, base.compat);
        assert_eq!(fast.thinking_level_map, base.thinking_level_map);
    }
}
