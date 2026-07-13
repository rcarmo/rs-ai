//! SIMULATED-FIXTURE-PORTED bucket.
//!
//! These cases re-audit the live-E2E / credential-gated upstream test files and
//! port the portable substance — *how the rs-ai port handles a provider
//! RESPONSE* — by driving each provider's real streaming path against a
//! faithful **simulated wire fixture** (wiremock) instead of a live endpoint.
//! No real credentials are used; the fixtures are hand-authored to match the
//! documented provider wire format. Only genuine real-model nondeterminism
//! (actual token counts, model phrasing, latency/abort timing) remains N/A.
//!
//! Upstream files whose response-handling substance is covered here:
//!   - `responseid.test.ts`        → responseId surfaced from a completed stream
//!   - `tokens.test.ts`            → usage surfaced on the terminal message
//!   - `total-tokens.test.ts`      → native (OpenAI) vs computed (Anthropic) total
//!   - `context-overflow.test.ts`  → overflow error → stopReason error + isContextOverflow
//!   - `unicode-surrogate.test.ts` → emoji in tool results round-trips into the request body
//!   - `google-thinking-disable.test.ts` → response with no thinking parts yields no Thinking events

#[cfg(test)]
mod tests {
    use crate::context::is_context_overflow;
    use crate::events::Event;
    use crate::provider::anthropic::stream_anthropic;
    use crate::provider::google::stream_google;
    use crate::provider::openai::stream_openai;
    use crate::provider::responses::stream_responses;
    use crate::types::{
        ContentBlock, Context, Message, Model, ModelCost, Role, StopReason, StreamOptions,
    };
    use serde_json::Value;
    use tokio_stream::StreamExt;
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn model(api: &str, provider: &str, base_url: &str) -> Model {
        Model {
            id: "test-model".into(),
            name: "Test".into(),
            api: api.into(),
            provider: provider.into(),
            base_url: base_url.into(),
            reasoning: false,
            thinking_level_map: None,
            input: vec!["text".into()],
            cost: ModelCost::default(),
            context_window: 1000,
            max_tokens: 4096,
            headers: None,
            api_key: Some("test".into()),
            compat: Default::default(),
        }
    }

    fn user_ctx(text: &str) -> Context {
        Context {
            system_prompt: None,
            tools: Vec::new(),
            messages: vec![msg(Role::User, text)],
        }
    }

    fn msg(role: Role, text: &str) -> Message {
        Message {
            role,
            content: vec![ContentBlock::Text {
                text: text.into(),
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
        }
    }

    /// Drive the right provider stream against a single SSE fixture; return the
    /// terminal Message (Done, or Error-with-message) plus the captured request body.
    async fn drive(m: Model, c: Context, sse: &str) -> (Message, Value) {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_string(sse.to_string()),
            )
            .mount(&server)
            .await;
        let mut m = m;
        m.base_url = server.uri();
        let opts = StreamOptions::default();
        let api = m.api.clone();
        let mut stream = match api.as_str() {
            "anthropic-messages" | "anthropic" => stream_anthropic(&m, &c, &opts),
            "google-generative-ai" | "google" => stream_google(&m, &c, &opts),

            "openai-responses" => stream_responses(&m, &c, &opts),
            _ => stream_openai(&m, &c, &opts),
        };
        let mut out: Option<Message> = None;
        while let Some(evt) = stream.next().await {
            match evt {
                Event::Done {
                    reason,
                    mut message,
                } => {
                    message.stop_reason = Some(reason);
                    out = Some(message);
                }
                Event::Error {
                    reason,
                    message: Some(mut mm),
                    ..
                } => {
                    mm.stop_reason = Some(reason);
                    out = Some(mm);
                }
                Event::Error { .. } => {}
                _ => {}
            }
        }
        let reqs = server.received_requests().await.unwrap();
        let body: Value = serde_json::from_slice(&reqs.last().unwrap().body).unwrap_or(Value::Null);
        (out.expect("a terminal message"), body)
    }

    async fn drive_events(m: Model, c: Context, sse: &str) -> Vec<Event> {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_string(sse.to_string()),
            )
            .mount(&server)
            .await;
        let mut m = m;
        m.base_url = server.uri();
        let opts = StreamOptions::default();
        let api = m.api.clone();
        let mut stream = match api.as_str() {
            "anthropic-messages" | "anthropic" => stream_anthropic(&m, &c, &opts),
            "google-generative-ai" | "google" => stream_google(&m, &c, &opts),

            "openai-responses" => stream_responses(&m, &c, &opts),
            _ => stream_openai(&m, &c, &opts),
        };
        let mut events = Vec::new();
        while let Some(evt) = stream.next().await {
            events.push(evt);
        }
        events
    }

    /// Drive the right provider stream with caller-supplied options; capture request body.
    async fn drive_opts(m: Model, c: Context, opts: StreamOptions, sse: &str) -> Value {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_string(sse.to_string()),
            )
            .mount(&server)
            .await;
        let mut m = m;
        m.base_url = server.uri();
        let api = m.api.clone();
        let mut stream = match api.as_str() {
            "anthropic-messages" | "anthropic" => stream_anthropic(&m, &c, &opts),
            "google-generative-ai" | "google" => stream_google(&m, &c, &opts),
            "openai-responses" => stream_responses(&m, &c, &opts),
            _ => stream_openai(&m, &c, &opts),
        };
        while stream.next().await.is_some() {}
        let reqs = server.received_requests().await.unwrap();
        serde_json::from_slice(&reqs.last().unwrap().body).unwrap_or(Value::Null)
    }

    const ANTHROPIC_OK: &str = concat!(
        "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"m\",\"role\":\"assistant\",\"content\":[],\"usage\":{\"input_tokens\":1,\"output_tokens\":0}}}\n\n",
        "event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n",
        "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"ok\"}}\n\n",
        "event: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":0}\n\n",
        "event: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"input_tokens\":1,\"output_tokens\":1}}\n\n",
        "event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n",
    );

    fn sys_ctx() -> Context {
        Context {
            system_prompt: Some("You are a helpful assistant.".into()),
            tools: Vec::new(),
            messages: vec![msg(Role::User, "Hello")],
        }
    }

    // ---------- responseid.test.ts ----------

    const OPENAI_COMPLETED: &str = concat!(
        "data: {\"id\":\"chatcmpl-abc123\",\"model\":\"gpt-4o-mini\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"response id test\"}}]}\n\n",
        "data: {\"id\":\"chatcmpl-abc123\",\"model\":\"gpt-4o-mini\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":12,\"completion_tokens\":4,\"total_tokens\":16,\"prompt_tokens_details\":{\"cached_tokens\":0}}}\n\n",
        "data: [DONE]\n\n",
    );

    #[tokio::test]
    async fn responseid_openai_completions_surfaces_response_id() {
        for _ in 0..3 {
            let (m, _b) = drive(
                model("openai-completions", "openai", "x"),
                user_ctx("hi"),
                OPENAI_COMPLETED,
            )
            .await;
            assert_ne!(m.stop_reason, Some(StopReason::Error));
            assert_eq!(m.response_id.as_deref(), Some("chatcmpl-abc123"));
        }
    }

    #[tokio::test]
    async fn responseid_google_surfaces_response_id() {
        let sse = "data: {\"responseId\":\"gen-resp-77\",\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"response id test\"}]},\"finishReason\":\"STOP\"}],\"usageMetadata\":{\"promptTokenCount\":5,\"candidatesTokenCount\":4,\"totalTokenCount\":9}}\n\n";
        for _ in 0..3 {
            let (m, _b) = drive(
                model("google-generative-ai", "google", "x"),
                user_ctx("hi"),
                sse,
            )
            .await;
            assert_ne!(m.stop_reason, Some(StopReason::Error));
            assert_eq!(m.response_id.as_deref(), Some("gen-resp-77"));
        }
    }

    // ---------- tokens.test.ts ----------

    #[tokio::test]
    async fn tokens_openai_surfaces_usage_on_terminal_message() {
        for _ in 0..3 {
            let (m, _b) = drive(
                model("openai-completions", "openai", "x"),
                user_ctx("hi"),
                OPENAI_COMPLETED,
            )
            .await;
            let u = m.usage.expect("usage present");
            assert_eq!(u.input, 12);
            assert_eq!(u.output, 4);
        }
    }

    // ---------- total-tokens.test.ts ----------

    #[tokio::test]
    async fn total_tokens_openai_is_computed_not_native() {
        // Upstream openai-completions computes total = input+output+cacheRead+cacheWrite
        // and ignores the provider's native `total_tokens` field. rs-ai mirrors this:
        // native 37 is ignored; computed = 12+4 = 16.
        let sse = concat!(
            "data: {\"id\":\"c1\",\"model\":\"gpt-4o-mini\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"hi\"}}]}\n\n",
            "data: {\"id\":\"c1\",\"model\":\"gpt-4o-mini\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":12,\"completion_tokens\":4,\"total_tokens\":37,\"prompt_tokens_details\":{\"cached_tokens\":0}}}\n\n",
            "data: [DONE]\n\n",
        );
        for _ in 0..3 {
            let (m, _b) = drive(
                model("openai-completions", "openai", "x"),
                user_ctx("hi"),
                sse,
            )
            .await;
            assert_eq!(m.usage.unwrap().total_tokens, 16);
        }
    }

    #[tokio::test]
    async fn total_tokens_anthropic_is_computed_sum() {
        // Anthropic has no native total; computed = input+output+cacheRead+cacheWrite.
        let sse = concat!(
            "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_1\",\"role\":\"assistant\",\"content\":[],\"usage\":{\"input_tokens\":100,\"output_tokens\":0,\"cache_read_input_tokens\":20,\"cache_creation_input_tokens\":5}}}\n\n",
            "event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n",
            "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"hi\"}}\n\n",
            "event: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":0}\n\n",
            "event: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"input_tokens\":100,\"output_tokens\":50,\"cache_read_input_tokens\":20,\"cache_creation_input_tokens\":5}}\n\n",
            "event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n",
        );
        for _ in 0..3 {
            let (m, _b) = drive(
                model("anthropic-messages", "anthropic", "x"),
                user_ctx("hi"),
                sse,
            )
            .await;
            let u = m.usage.expect("usage");
            assert_eq!(u.input, 100);
            assert_eq!(u.output, 50);
            assert_eq!(u.cache_read, 20);
            assert_eq!(u.cache_write, 5);
            assert_eq!(u.total_tokens, 175);
        }
    }

    #[tokio::test]
    async fn total_tokens_openai_responses_uses_native_total() {
        // openai-responses DOES use the provider's native total_tokens (distinct
        // from completions). Native 99 must pass through unchanged.
        let sse = concat!(
            "data: {\"type\":\"response.created\",\"response\":{\"id\":\"resp-1\"}}\n\n",
            "data: {\"type\":\"response.output_text.delta\",\"delta\":\"hi\"}\n\n",
            "data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp-1\",\"status\":\"completed\",\"usage\":{\"input_tokens\":8,\"output_tokens\":2,\"total_tokens\":99,\"input_tokens_details\":{\"cached_tokens\":0}}}}\n\n",
        );
        for _ in 0..3 {
            let (m, _b) = drive(
                model("openai-responses", "openai", "x"),
                user_ctx("hi"),
                sse,
            )
            .await;
            assert_eq!(m.usage.unwrap().total_tokens, 99);
        }
    }

    // ---------- context-overflow.test.ts ----------

    #[tokio::test]
    async fn context_overflow_anthropic_error_is_detected() {
        // Anthropic surfaces overflow as an SSE error event with "prompt is too long".
        let sse = "event: error\ndata: {\"type\":\"error\",\"error\":{\"type\":\"invalid_request_error\",\"message\":\"prompt is too long: 213462 tokens > 200000 maximum\"}}\n\n";
        for _ in 0..3 {
            let (m, _b) = drive(
                model("anthropic-messages", "anthropic", "x"),
                user_ctx("hi"),
                sse,
            )
            .await;
            assert_eq!(m.stop_reason, Some(StopReason::Error));
            let model_def = model("anthropic-messages", "anthropic", "x");
            assert!(
                is_context_overflow(&m, &model_def),
                "overflow error must be detected"
            );
        }
    }

    #[tokio::test]
    async fn context_overflow_rate_limit_is_not_overflow() {
        // A throttling/rate-limit error must NOT be classified as overflow.
        let sse = "event: error\ndata: {\"type\":\"error\",\"error\":{\"type\":\"rate_limit_error\",\"message\":\"rate limit exceeded, too many requests\"}}\n\n";
        for _ in 0..3 {
            let (m, _b) = drive(
                model("anthropic-messages", "anthropic", "x"),
                user_ctx("hi"),
                sse,
            )
            .await;
            assert_eq!(m.stop_reason, Some(StopReason::Error));
            let model_def = model("anthropic-messages", "anthropic", "x");
            assert!(
                !is_context_overflow(&m, &model_def),
                "rate-limit must not be overflow"
            );
        }
    }

    // ---------- unicode-surrogate.test.ts ----------

    fn tool_result_ctx(emoji: &str) -> Context {
        let mut tr = msg(Role::ToolResult, &format!("Result: {emoji} done"));
        tr.tool_call_id = Some("test_1".into());
        tr.tool_name = Some("emoji_tool".into());
        let mut assistant = msg(Role::Assistant, "");
        assistant.content = vec![ContentBlock::ToolCall {
            id: "test_1".into(),
            name: "emoji_tool".into(),
            arguments: std::collections::HashMap::new(),
            thought_signature: None,
        }];
        Context {
            system_prompt: None,
            tools: Vec::new(),
            messages: vec![msg(Role::User, "use the tool"), assistant, tr],
        }
    }

    #[tokio::test]
    async fn unicode_surrogate_emoji_roundtrips_into_openai_request() {
        // Astral-plane emoji (🎉 U+1F389) must serialize intact (no lone surrogates).
        let emoji = "🎉🚀😀";
        for _ in 0..3 {
            let (_m, body) = drive(
                model("openai-completions", "openai", "x"),
                tool_result_ctx(emoji),
                OPENAI_COMPLETED,
            )
            .await;
            let serialized = serde_json::to_string(&body).unwrap();
            assert!(
                serialized.contains(emoji),
                "request body must contain intact emoji: {serialized}"
            );
        }
    }

    #[tokio::test]
    async fn unicode_surrogate_emoji_roundtrips_into_anthropic_request() {
        let emoji = "🎉🚀😀";
        let sse = concat!(
            "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"m\",\"role\":\"assistant\",\"content\":[],\"usage\":{\"input_tokens\":1,\"output_tokens\":0}}}\n\n",
            "event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n",
            "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"ok\"}}\n\n",
            "event: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":0}\n\n",
            "event: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"input_tokens\":1,\"output_tokens\":1}}\n\n",
            "event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n",
        );
        for _ in 0..3 {
            let (_m, body) = drive(
                model("anthropic-messages", "anthropic", "x"),
                tool_result_ctx(emoji),
                sse,
            )
            .await;
            let serialized = serde_json::to_string(&body).unwrap();
            assert!(
                serialized.contains(emoji),
                "anthropic request body must contain intact emoji"
            );
        }
    }

    // ---------- cache-retention.test.ts ----------

    use crate::types::{CacheRetention, ModelCompat};

    fn anthropic_cache_model() -> Model {
        let mut m = model("anthropic-messages", "anthropic", "x");
        // Direct api.anthropic.com base so default (short) retention path is exercised.
        m.base_url = "https://api.anthropic.com".into();
        m
    }

    #[tokio::test]
    async fn cache_retention_default_short_has_cache_control_without_ttl() {
        for _ in 0..3 {
            let body = drive_opts(
                anthropic_cache_model(),
                sys_ctx(),
                StreamOptions::default(),
                ANTHROPIC_OK,
            )
            .await;
            let cc = &body["system"][0]["cache_control"];
            assert_eq!(
                *cc,
                serde_json::json!({"type": "ephemeral"}),
                "default = ephemeral, no ttl"
            );
        }
    }

    #[tokio::test]
    async fn cache_retention_long_adds_1h_ttl() {
        let opts = StreamOptions {
            cache_retention: Some(CacheRetention::Long),
            ..Default::default()
        };
        for _ in 0..3 {
            let body = drive_opts(
                anthropic_cache_model(),
                sys_ctx(),
                opts.clone(),
                ANTHROPIC_OK,
            )
            .await;
            let cc = &body["system"][0]["cache_control"];
            assert_eq!(*cc, serde_json::json!({"type": "ephemeral", "ttl": "1h"}));
        }
    }

    #[tokio::test]
    async fn cache_retention_long_omitted_when_compat_unsupported() {
        let mut m = anthropic_cache_model();
        m.compat = ModelCompat {
            supports_long_cache_retention: Some(false),
            ..Default::default()
        };
        let opts = StreamOptions {
            cache_retention: Some(CacheRetention::Long),
            ..Default::default()
        };
        for _ in 0..3 {
            let body = drive_opts(m.clone(), sys_ctx(), opts.clone(), ANTHROPIC_OK).await;
            let cc = &body["system"][0]["cache_control"];
            assert_eq!(
                *cc,
                serde_json::json!({"type": "ephemeral"}),
                "unsupported long retention omits ttl"
            );
        }
    }

    // ---------- google-thinking-disable.test.ts ----------

    #[tokio::test]
    async fn google_thinking_disable_response_yields_no_thinking_events() {
        // A response with only plain text parts (no `thought:true` parts) must
        // surface text and produce zero Thinking events.
        let sse = "data: {\"responseId\":\"r\",\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"pong pong pong\"}]},\"finishReason\":\"STOP\"}],\"usageMetadata\":{\"promptTokenCount\":5,\"candidatesTokenCount\":3,\"totalTokenCount\":8}}\n\n";
        for _ in 0..3 {
            let events = drive_events(
                model("google-generative-ai", "google", "x"),
                user_ctx("pong x40"),
                sse,
            )
            .await;
            let thinking = events
                .iter()
                .filter(|e| matches!(e, Event::ThinkingDelta { .. }))
                .count();
            let text: String = events
                .iter()
                .filter_map(|e| match e {
                    Event::TextDelta { delta } => Some(delta.clone()),
                    _ => None,
                })
                .collect();
            assert_eq!(
                thinking, 0,
                "thinking-disabled response must have no thinking events"
            );
            assert!(text.contains("pong"));
        }
    }

    // ---------- error-body.test.ts / provider-error-body-*.test.ts ----------
    //
    // Upstream's three new 0.80.3 error-body files mock the JS SDKs so a 403
    // gateway response with a body the SDK folds into an opaque "403 status code
    // (no body)" message still surfaces status + body. rs-ai's reqwest path reads
    // `resp.text()` directly (it never had the JS-SDK body-hiding bug), so here we
    // port the deterministic *behavioral contract* against a real 403-with-body
    // wire response: providers must surface both the status and the body reason,
    // with the responses/azure branded prefix. Stronger than the JS mocks (real
    // transport), each asserted 3x for determinism.

    /// Drive a provider against a single non-2xx response carrying `body`; return
    /// the terminal error message string.
    async fn drive_http_error(m: Model, status: u16, body: &str) -> String {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(status)
                    .insert_header("content-type", "application/json")
                    .set_body_string(body.to_string()),
            )
            .mount(&server)
            .await;
        let mut m = m;
        m.base_url = server.uri();
        let opts = StreamOptions::default();
        let api = m.api.clone();
        let c = user_ctx("hi");
        let mut stream = match api.as_str() {
            "google-generative-ai" | "google" => stream_google(&m, &c, &opts),
            "openai-responses" => stream_responses(&m, &c, &opts),
            _ => stream_openai(&m, &c, &opts),
        };
        let mut err: Option<String> = None;
        while let Some(evt) = stream.next().await {
            if let Event::Error { error, .. } = evt {
                err = Some(error.to_string());
            }
        }
        err.expect("a terminal error")
    }

    #[tokio::test]
    async fn error_body_openai_completions_surfaces_status_and_body() {
        let body = r#"{"error":"blocked by gateway WAF"}"#;
        for _ in 0..3 {
            let msg =
                drive_http_error(model("openai-completions", "openrouter", "x"), 403, body).await;
            assert!(msg.contains("403"), "status surfaced: {msg}");
            assert!(
                msg.contains("blocked by gateway WAF"),
                "body reason surfaced: {msg}"
            );
            assert_eq!(msg, format!("403: {body}"));
        }
    }

    #[tokio::test]
    async fn error_body_openai_responses_keeps_prefix_and_surfaces_body() {
        let body = r#"{"error":"blocked by gateway WAF"}"#;
        for _ in 0..3 {
            let msg = drive_http_error(model("openai-responses", "openai", "x"), 403, body).await;
            assert!(
                msg.contains("OpenAI API error (403)"),
                "branded prefix + status: {msg}"
            );
            assert!(
                msg.contains("blocked by gateway WAF"),
                "body reason surfaced: {msg}"
            );
            assert_eq!(msg, format!("OpenAI API error (403): {body}"));
        }
    }

    #[tokio::test]
    async fn error_body_google_surfaces_status_and_body() {
        let body = r#"{"error":{"code":403,"message":"Permission denied"}}"#;
        for _ in 0..3 {
            let msg =
                drive_http_error(model("google-generative-ai", "google", "x"), 403, body).await;
            assert!(
                msg.contains("403") && msg.contains("Permission denied"),
                "status+body: {msg}"
            );
            assert_eq!(msg, format!("403: {body}"));
        }
    }
}
