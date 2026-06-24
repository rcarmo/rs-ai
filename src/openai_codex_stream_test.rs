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
        let body: Value = serde_json::from_slice(&req.body).unwrap();
        (text, reason, headers, body)
    }

    #[tokio::test]
    async fn streams_sse_responses_into_assistant_message() {
        let (text, reason, _h, _b) = run(COMPLETED_SSE, StreamOptions::default()).await;
        assert_eq!(text, "Hello");
        assert!(matches!(reason, StopReason::Stop));
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
}
