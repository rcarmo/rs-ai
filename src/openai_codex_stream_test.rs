//! Test-for-test port (deterministic SSE/header/payload subset) of upstream
//! `test/openai-codex-stream.test.ts` (`@earendil-works/pi-ai` v0.80.2).
//!
//! The WebSocket-transport cases (cached context, connect-timeout / idle /
//! connection-limit fallbacks, websocket-cached deltas, debug stats) live in the
//! documented WS-pooling gap; the SSE streaming, session headers, and
//! prompt-cache payload cases are ported here via `stream_codex` (transport=sse)
//! + wiremock.

#[cfg(test)]
mod tests {
    use crate::provider::codex::stream_codex;
    use crate::types::{Context, ContentBlock, Message, Model, ModelCost, Role, StopReason, StreamOptions, Transport};
    use crate::events::Event;
    use serde_json::Value;
    use std::collections::HashMap;
    use tokio_stream::StreamExt;
    use wiremock::{Mock, MockServer, ResponseTemplate};
    use wiremock::matchers::method;

    fn codex_model(base_url: &str) -> Model {
        Model {
            id: "gpt-5.5".into(), name: "GPT-5.5".into(), api: "openai-codex-responses".into(),
            provider: "openai-codex".into(), base_url: base_url.into(), reasoning: true,
            thinking_level_map: None, input: vec!["text".into()], cost: ModelCost::default(),
            context_window: 256000, max_tokens: 16384, headers: None, api_key: Some("a.b.c".into()), compat: Default::default(),
        }
    }

    fn ctx() -> Context {
        Context {
            system_prompt: None, tools: Vec::new(),
            messages: vec![Message {
                role: Role::User, content: vec![ContentBlock::Text { text: "hi".into(), text_signature: None }],
                timestamp: 0, api: None, provider: None, model: None, response_id: None,
                response_model: None, diagnostics: Vec::new(), usage: None,
                stop_reason: None, error_message: None,
                tool_call_id: None, tool_name: None, is_error: false, details: None,
            }],
        }
    }

    const COMPLETED_SSE: &str = concat!(
        "data: {\"type\":\"response.output_item.added\",\"item\":{\"type\":\"message\",\"id\":\"msg_1\",\"role\":\"assistant\",\"status\":\"in_progress\",\"content\":[]}}\n\n",
        "data: {\"type\":\"response.output_text.delta\",\"delta\":\"Hello\"}\n\n",
        "data: {\"type\":\"response.completed\",\"response\":{\"status\":\"completed\",\"usage\":{\"input_tokens\":5,\"output_tokens\":3,\"total_tokens\":8,\"input_tokens_details\":{\"cached_tokens\":0}}}}\n\n",
        "data: [DONE]\n\n",
    );

    /// Drive a codex SSE request; return (events-text, terminal reason, request headers, request body).
    async fn run(body_sse: &str, opts: StreamOptions) -> (String, StopReason, HashMap<String, String>, Value) {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).insert_header("content-type", "text/event-stream").set_body_string(body_sse))
            .mount(&server).await;
        let model = codex_model(&server.uri());
        let c = ctx();
        let mut o = opts; o.transport = Some(Transport::Sse);
        let mut stream = stream_codex(&model, &c, &o);
        let mut text = String::new();
        let mut reason = StopReason::Stop;
        let mut saw_start = false;
        while let Some(evt) = stream.next().await {
            match evt {
                Event::Start { .. } => saw_start = true,
                Event::TextDelta { delta } => text.push_str(&delta),
                Event::Done { reason: r, .. } => reason = r,
                Event::Error { reason: r, .. } => reason = r,
                _ => {}
            }
        }
        assert!(saw_start, "a Start event is expected");
        let reqs = server.received_requests().await.unwrap();
        let req = reqs.last().unwrap();
        let headers = req.headers.iter().map(|(k, v)| (k.as_str().to_string(), v.to_str().unwrap_or("").to_string())).collect();
        let body: Value = decode_request_body(req.headers.get("content-encoding").and_then(|v| v.to_str().ok()), &req.body);
        (text, reason, headers, body)
    }

    /// Decode a captured Codex SSE request body, decompressing zstd frames.
    fn decode_request_body(content_encoding: Option<&str>, body: &[u8]) -> Value {
        let raw = if content_encoding == Some("zstd") {
            zstd::stream::decode_all(body).expect("body is a valid zstd frame")
        } else {
            body.to_vec()
        };
        serde_json::from_slice(&raw).unwrap()
    }

    #[tokio::test]
    async fn streams_sse_responses_into_assistant_message() {
        let (text, reason, _h, _b) = run(COMPLETED_SSE, StreamOptions::default()).await;
        assert_eq!(text, "Hello");
        assert!(matches!(reason, StopReason::Stop));
    }

    #[tokio::test]
    async fn sse_header_timeout_surfaces_codex_message() {
        // v0.80.3: when response headers do not arrive within the configured HTTP
        // timeout, surface `Codex SSE response headers timed out after {ms}ms`.
        use std::time::Duration;
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_delay(Duration::from_secs(30))
                .set_body_string(COMPLETED_SSE))
            .mount(&server).await;
        let model = codex_model(&server.uri());
        let c = ctx();
        let o = StreamOptions { transport: Some(Transport::Sse), timeout_ms: Some(10), ..Default::default() };
        let mut stream = stream_codex(&model, &c, &o);
        let mut err: Option<String> = None;
        while let Some(evt) = stream.next().await {
            if let Event::Error { error, .. } = evt { err = Some(error.to_string()); }
        }
        assert_eq!(err.as_deref(), Some("Codex SSE response headers timed out after 10ms"));
    }

    #[tokio::test]
    async fn maps_response_incomplete_to_length() {
        let sse = concat!(
            "data: {\"type\":\"response.output_text.delta\",\"delta\":\"Hi\"}\n\n",
            "data: {\"type\":\"response.incomplete\",\"response\":{\"status\":\"incomplete\",\"usage\":{\"input_tokens\":1,\"output_tokens\":1,\"total_tokens\":2,\"input_tokens_details\":{\"cached_tokens\":0}}}}\n\n",
            "data: [DONE]\n\n",
        );
        let (_t, reason, _h, _b) = run(sse, StreamOptions::default()).await;
        assert!(matches!(reason, StopReason::Length));
    }

    #[tokio::test]
    async fn sets_session_headers_and_prompt_cache_key_when_session_provided() {
        let opts = StreamOptions { session_id: Some("test-session-123".into()), ..Default::default() };
        let (_t, _r, h, b) = run(COMPLETED_SSE, opts).await;
        assert_eq!(h.get("session-id").map(String::as_str), Some("test-session-123"));
        assert!(!h.contains_key("session_id"), "underscore session_id must not be set");
        assert_eq!(h.get("x-client-request-id").map(String::as_str), Some("test-session-123"));
        assert_eq!(b["prompt_cache_key"], serde_json::json!("test-session-123"));
    }

    #[tokio::test]
    async fn clamps_prompt_cache_key_to_64_chars() {
        let opts = StreamOptions { session_id: Some("x".repeat(67)), ..Default::default() };
        let (_t, _r, _h, b) = run(COMPLETED_SSE, opts).await;
        assert_eq!(b["prompt_cache_key"], serde_json::json!("x".repeat(64)));
    }

    #[tokio::test]
    async fn does_not_set_session_headers_when_no_session() {
        let (_t, _r, h, b) = run(COMPLETED_SSE, StreamOptions::default()).await;
        assert!(!h.contains_key("session-id"));
        assert!(!h.contains_key("x-client-request-id"));
        assert!(b.get("prompt_cache_key").is_none());
    }

    #[tokio::test]
    async fn compresses_sse_request_body_with_zstd() {
        // v0.80.5: the Codex SSE responses request body is zstd-compressed
        // (Content-Encoding: zstd), matching the official Codex client. Assert the
        // captured request is really zstd-framed and decodes back to the payload.
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).insert_header("content-type", "text/event-stream").set_body_string(COMPLETED_SSE))
            .mount(&server).await;
        let model = codex_model(&server.uri());
        let c = ctx();
        let o = StreamOptions { transport: Some(Transport::Sse), ..Default::default() };
        let mut stream = stream_codex(&model, &c, &o);
        while stream.next().await.is_some() {}

        let reqs = server.received_requests().await.unwrap();
        let req = reqs.last().unwrap();
        assert_eq!(
            req.headers.get("content-encoding").and_then(|v| v.to_str().ok()),
            Some("zstd"),
            "content-encoding must be zstd"
        );
        // zstd magic number: 0x28 0xB5 0x2F 0xFD (little-endian 0xFD2FB528).
        assert_eq!(&req.body[0..4], &[0x28, 0xB5, 0x2F, 0xFD], "body must be a zstd frame");
        // The frame decompresses back to the JSON payload with the expected model.
        let decoded = zstd::stream::decode_all(&req.body[..]).expect("valid zstd frame");
        let body: Value = serde_json::from_slice(&decoded).unwrap();
        assert_eq!(body["model"], serde_json::json!("gpt-5.5"));
    }

    #[test]
    fn recycles_cached_websocket_at_backend_connection_age_limit() {
        // v0.80.5: a cached session WebSocket is recycled once its age reaches the
        // backend connection age limit (SESSION_WEBSOCKET_MAX_AGE_MS = 55 min); a
        // younger socket stays reusable.
        use crate::provider::codex::{codex_websocket_session_expired, SESSION_WEBSOCKET_MAX_AGE_MS};
        let now = 3_600_000_u64; // 60 min
        // Older than 55 min -> expired (open a fresh connection).
        let old_created = now - SESSION_WEBSOCKET_MAX_AGE_MS; // exactly 55 min old
        assert!(codex_websocket_session_expired(old_created, now));
        assert!(codex_websocket_session_expired(now - (56 * 60 * 1000), now));
        // Younger than 55 min -> reusable.
        assert!(!codex_websocket_session_expired(now - (54 * 60 * 1000), now));
        assert!(!codex_websocket_session_expired(now, now));
    }
}
