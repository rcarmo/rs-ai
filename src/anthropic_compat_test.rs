//! Test-for-test ports of upstream anthropic compat files (`@earendil-works/pi-ai`
//! v0.80.2): `anthropic-eager-tool-input-compat.test.ts`,
//! `anthropic-adaptive-thinking-models.test.ts`, and
//! `anthropic-empty-thinking-signature-compat.test.ts`.

#[cfg(test)]
mod tests {
    use crate::events::Event;
    use crate::provider::anthropic::{build_anthropic_payload, stream_anthropic};
    use crate::registry::list_models;
    use crate::types::{
        ContentBlock, Context, Message, Model, ModelCompat, ModelCost, Role, StopReason,
        StreamOptions, Tool,
    };
    use serde_json::{Value, json};
    use tokio_stream::StreamExt;
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    // --- eager tool input streaming ---

    fn eager_model(base_url: &str, compat: ModelCompat) -> Model {
        Model {
            id: "claude-opus-4-8".into(),
            name: "Claude Opus 4.8".into(),
            api: "anthropic-messages".into(),
            provider: "test-anthropic".into(),
            base_url: base_url.into(),
            reasoning: true,
            thinking_level_map: None,
            input: vec!["text".into()],
            cost: ModelCost::default(),
            context_window: 200000,
            max_tokens: 32000,
            headers: None,
            api_key: Some("test-key".into()),
            compat,
        }
    }

    fn lookup_tool() -> Tool {
        Tool {
            name: "lookup".into(),
            description: "Look up a value".into(),
            parameters: json!({"type": "object", "properties": {"value": {"type": "string"}}}),
            constrained_sampling: None,
        }
    }

    fn user_ctx(tools: Vec<Tool>) -> Context {
        Context {
            system_prompt: None,
            tools,
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
                raw_stop_reason: None,
                tool_call_id: None,
                tool_name: None,
                is_error: false,
                details: None,
                added_tool_names: Vec::new(),
            }],
        }
    }

    /// Drive a request against an empty SSE response and capture (body, anthropic-beta header).
    async fn capture(compat: ModelCompat, ctx: Context) -> (Value, Option<String>) {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_string(""),
            )
            .mount(&server)
            .await;
        let model = eager_model(&server.uri(), compat);
        let opts = StreamOptions {
            cache_retention: Some(crate::types::CacheRetention::None),
            ..Default::default()
        };
        let mut stream = stream_anthropic(&model, &ctx, &opts);
        while let Some(evt) = stream.next().await {
            if matches!(evt, Event::Done { .. } | Event::Error { .. }) {
                break;
            }
        }
        let reqs = server.received_requests().await.unwrap();
        let req = reqs.last().expect("a request");
        let body: Value = serde_json::from_slice(&req.body).unwrap();
        let beta = req
            .headers
            .get("anthropic-beta")
            .map(|v| v.to_str().unwrap().to_string());
        (body, beta)
    }

    fn force_adaptive() -> ModelCompat {
        ModelCompat {
            force_adaptive_thinking: Some(true),
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn sends_per_tool_eager_input_streaming_by_default() {
        let (body, beta) = capture(force_adaptive(), user_ctx(vec![lookup_tool()])).await;
        assert_eq!(body["tools"][0]["eager_input_streaming"], json!(true));
        assert!(
            beta.is_none(),
            "no anthropic-beta header by default, got {beta:?}"
        );
    }

    #[tokio::test]
    async fn uses_legacy_fine_grained_beta_when_eager_disabled() {
        let compat = ModelCompat {
            force_adaptive_thinking: Some(true),
            supports_eager_tool_input_streaming: Some(false),
            ..Default::default()
        };
        let (body, beta) = capture(compat, user_ctx(vec![lookup_tool()])).await;
        assert!(body["tools"][0].get("eager_input_streaming").is_none());
        assert_eq!(
            beta.as_deref(),
            Some("fine-grained-tool-streaming-2025-05-14")
        );
    }

    #[tokio::test]
    async fn no_legacy_beta_when_there_are_no_tools() {
        let compat = ModelCompat {
            force_adaptive_thinking: Some(true),
            supports_eager_tool_input_streaming: Some(false),
            ..Default::default()
        };
        let (body, beta) = capture(compat, user_ctx(Vec::new())).await;
        assert!(body.get("tools").is_none());
        assert!(beta.is_none());
    }

    // --- adaptive thinking model metadata ---

    #[test]
    fn marks_builtin_anthropic_models_that_use_adaptive_thinking() {
        let mut flagged: Vec<String> = list_models(None)
            .into_iter()
            .filter(|m| {
                m.api == "anthropic-messages" && m.compat.force_adaptive_thinking == Some(true)
            })
            .map(|m| format!("{}/{}", m.provider, m.id))
            .collect();
        flagged.sort();
        for expected in [
            "anthropic/claude-fable-5",
            "anthropic/claude-opus-4-8",
            "cloudflare-ai-gateway/claude-fable-5",
            "kimi-coding/k3",
            "kimi-coding/kimi-for-coding",
            "kimi-coding/kimi-for-coding-highspeed",
            "opencode/claude-opus-4-8",
            "vercel-ai-gateway/anthropic/claude-opus-4.8",
        ] {
            assert!(
                flagged.iter().any(|f| f == expected),
                "missing adaptive-thinking model {expected}; got {flagged:?}"
            );
        }
        let re = regex_lite_matches;
        for id in &flagged {
            assert!(
                re(id),
                "flagged model {id} does not match the expected opus/sonnet/fable pattern"
            );
        }
    }

    /// Mirrors the upstream adaptive-thinking model families
    /// (opus 4.6/4.7/4.8, sonnet 4.6, sonnet 5, fable 5, Kimi Coding).
    fn regex_lite_matches(id: &str) -> bool {
        let opus = id.contains("opus")
            && (id.contains("4-6")
                || id.contains("4.6")
                || id.contains("4-7")
                || id.contains("4.7")
                || id.contains("4-8")
                || id.contains("4.8")
                || id.contains("opus-5")
                || id.contains("opus.5"));
        let sonnet = id.contains("sonnet")
            && (id.contains("4-6")
                || id.contains("4.6")
                || id.contains("sonnet-5")
                || id.contains("sonnet.5"));
        let fable = id.contains("fable-5") || id.contains("fable.5");
        let kimi_coding = id.starts_with("kimi-coding/");
        opus || sonnet || fable || kimi_coding
    }

    // --- empty thinking signature ---

    fn mimo_model(allow_empty_signature: Option<bool>) -> Model {
        Model {
            id: "mimo-v2.5-pro".into(),
            name: "MiMo-V2.5-Pro".into(),
            api: "anthropic-messages".into(),
            provider: "xiaomi-token-plan-ams".into(),
            base_url: "http://127.0.0.1:9/anthropic".into(),
            reasoning: true,
            thinking_level_map: None,
            input: vec!["text".into()],
            cost: ModelCost::default(),
            context_window: 1048576,
            max_tokens: 1024,
            headers: None,
            api_key: None,
            compat: ModelCompat {
                allow_empty_signature,
                ..Default::default()
            },
        }
    }

    fn thinking_ctx(signature: &str) -> Context {
        thinking_ctx_with_text(signature, "internal reasoning")
    }

    fn thinking_ctx_with_text(signature: &str, thinking: &str) -> Context {
        thinking_ctx_with_text_for(
            signature,
            thinking,
            "xiaomi-token-plan-ams",
            "mimo-v2.5-pro",
        )
    }

    fn thinking_ctx_with_text_for(
        signature: &str,
        thinking: &str,
        provider: &str,
        model_id: &str,
    ) -> Context {
        let assistant = Message {
            role: Role::Assistant,
            content: vec![ContentBlock::Thinking {
                thinking: thinking.into(),
                thinking_signature: Some(signature.into()),
                redacted: false,
            }],
            timestamp: 0,
            api: Some("anthropic-messages".into()),
            provider: Some(provider.into()),
            model: Some(model_id.into()),
            response_id: None,
            response_model: None,
            diagnostics: Vec::new(),
            usage: None,
            stop_reason: Some(StopReason::Stop),
            error_message: None,
            raw_stop_reason: None,
            tool_call_id: None,
            tool_name: None,
            is_error: false,
            details: None,
            added_tool_names: Vec::new(),
        };
        Context {
            system_prompt: None,
            tools: Vec::new(),
            messages: vec![
                Message {
                    role: Role::User,
                    content: vec![ContentBlock::Text {
                        text: "first".into(),
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
                    raw_stop_reason: None,
                    tool_call_id: None,
                    tool_name: None,
                    is_error: false,
                    details: None,
                    added_tool_names: Vec::new(),
                },
                assistant,
                Message {
                    role: Role::User,
                    content: vec![ContentBlock::Text {
                        text: "second".into(),
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
                    raw_stop_reason: None,
                    tool_call_id: None,
                    tool_name: None,
                    is_error: false,
                    details: None,
                    added_tool_names: Vec::new(),
                },
            ],
        }
    }

    fn assistant_content(p: &Value) -> &Value {
        p["messages"]
            .as_array()
            .unwrap()
            .iter()
            .find(|m| m.get("role").and_then(|r| r.as_str()) == Some("assistant"))
            .map(|m| &m["content"])
            .unwrap()
    }

    #[test]
    fn converts_empty_signature_thinking_to_text_by_default() {
        let p = build_anthropic_payload(
            &mimo_model(None),
            &thinking_ctx(""),
            &StreamOptions::default(),
        );
        assert_eq!(
            *assistant_content(&p),
            json!([{"type": "text", "text": "internal reasoning"}])
        );
    }

    #[test]
    fn preserves_empty_signature_thinking_when_allow_empty_signature_enabled() {
        let p = build_anthropic_payload(
            &mimo_model(Some(true)),
            &thinking_ctx(" "),
            &StreamOptions::default(),
        );
        assert_eq!(
            *assistant_content(&p),
            json!([{"type": "thinking", "thinking": "internal reasoning", "signature": ""}])
        );
    }

    #[test]
    fn kimi_coding_models_allow_empty_signatures() {
        for id in ["k3", "kimi-for-coding"] {
            let model = crate::registry::get_model("kimi-coding", id).expect("kimi-coding model");
            assert_eq!(model.compat.allow_empty_signature, Some(true));
            let p = build_anthropic_payload(
                &model,
                &thinking_ctx_with_text_for(" ", "internal reasoning", "kimi-coding", id),
                &StreamOptions::default(),
            );
            assert_eq!(
                *assistant_content(&p),
                json!([{"type": "thinking", "thinking": "internal reasoning", "signature": ""}]),
                "model {id}"
            );
        }
    }

    #[test]
    fn preserves_empty_thinking_text_when_the_signature_is_present() {
        let p = build_anthropic_payload(
            &mimo_model(None),
            &thinking_ctx_with_text("signed-thinking", ""),
            &StreamOptions::default(),
        );
        assert_eq!(
            *assistant_content(&p),
            json!([{"type": "thinking", "thinking": "", "signature": "signed-thinking"}])
        );
    }
}
