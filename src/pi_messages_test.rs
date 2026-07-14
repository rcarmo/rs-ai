//! Test-for-test port of upstream `test/pi-messages.test.ts` (`@earendil-works/pi-ai` v0.80.7).

#[cfg(test)]
mod tests {
    use crate::events::Event;
    use crate::provider::pi_messages::stream_pi_messages;
    use crate::types::*;
    use serde_json::json;
    use std::collections::HashMap;
    use tokio_stream::StreamExt;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn model(base_url: &str) -> Model {
        Model {
            id: "auto".into(),
            name: "Radius Auto".into(),
            api: "pi-messages".into(),
            provider: "radius".into(),
            base_url: base_url.into(),
            reasoning: false,
            thinking_level_map: None,
            input: vec!["text".into()],
            cost: ModelCost::default(),
            context_window: 128000,
            max_tokens: 16384,
            headers: None,
            api_key: None,
            compat: Default::default(),
        }
    }

    fn context() -> Context {
        Context {
            system_prompt: None,
            tools: Vec::new(),
            messages: vec![user_message("Hello")],
        }
    }

    fn sse(events: Vec<serde_json::Value>) -> String {
        events
            .into_iter()
            .map(|e| format!("data: {}\n\n", e))
            .collect::<String>()
    }

    fn usage() -> serde_json::Value {
        json!({"input":10,"output":5,"cacheRead":0,"cacheWrite":0,"totalTokens":15,"cost":{"input":0.1,"output":0.2,"cacheRead":0.0,"cacheWrite":0.0,"total":0.3}})
    }

    async fn collect(
        mut stream: std::pin::Pin<Box<dyn futures::Stream<Item = Event> + Send + '_>>,
    ) -> Vec<Event> {
        let mut out = Vec::new();
        while let Some(ev) = stream.next().await {
            out.push(ev);
        }
        out
    }

    #[tokio::test]
    async fn streams_text_tool_calls_payload_and_terminal_message() {
        let server = MockServer::start().await;
        Mock::given(method("POST")).and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(200).insert_header("content-type", "text/event-stream").set_body_string(sse(vec![
                json!({"type":"start"}),
                json!({"type":"text_start","contentIndex":0}),
                json!({"type":"text_delta","contentIndex":0,"delta":"Hel"}),
                json!({"type":"text_delta","contentIndex":0,"delta":"lo"}),
                json!({"type":"text_end","contentIndex":0,"content":"Hello"}),
                json!({"type":"toolcall_start","contentIndex":1,"id":"call_1","toolName":"read"}),
                json!({"type":"toolcall_delta","contentIndex":1,"delta":"{\"path\":"}),
                json!({"type":"toolcall_delta","contentIndex":1,"delta":"\"a.txt\"}"}),
                json!({"type":"toolcall_end","contentIndex":1,"toolCall":{"type":"toolCall","id":"call_1","name":"read","arguments":{"path":"a.txt"}}}),
                json!({"type":"done","reason":"toolUse","usage":usage(),"responseId":"resp_1"}),
            ]))).mount(&server).await;

        let mut headers = HashMap::new();
        headers.insert("x-custom".into(), "1".into());
        let opts = StreamOptions {
            api_key: Some("test-key".into()),
            session_id: Some("session-1".into()),
            tool_choice: Some(json!("auto")),
            max_tokens: Some(100),
            headers: Some(headers),
            ..Default::default()
        };
        let events = collect(stream_pi_messages(
            &model(&format!("{}/v1", server.uri())),
            &context(),
            &opts,
        ))
        .await;
        assert!(events.iter().any(|e| matches!(e, Event::TextDelta { .. })));
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e, Event::ToolCallEnd { .. }))
                .count(),
            1
        );
        let Event::Done { reason, message } = events.last().unwrap() else {
            panic!("expected done");
        };
        assert_eq!(*reason, StopReason::ToolUse);
        assert_eq!(message.response_id.as_deref(), Some("resp_1"));
        assert_eq!(message.model.as_deref(), Some("auto"));
        assert_eq!(message.provider.as_deref(), Some("radius"));
        assert!(matches!(&message.content[0], ContentBlock::Text { text, .. } if text == "Hello"));
        assert!(
            matches!(&message.content[1], ContentBlock::ToolCall { id, name, arguments, .. } if id == "call_1" && name == "read" && arguments.get("path").and_then(|v| v.as_str()) == Some("a.txt"))
        );
    }

    #[tokio::test]
    async fn debug_on_response_and_server_error_are_reported() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .insert_header("x-pi-gateway-upstream-provider", "anthropic")
                    .set_body_string(sse(vec![
                        json!({"type":"done","reason":"stop","usage":usage()}),
                    ])),
            )
            .mount(&server)
            .await;
        let mut metadata = HashMap::new();
        metadata.insert("debug".into(), json!(true));
        let seen = std::sync::Arc::new(std::sync::Mutex::new(None::<String>));
        let seen_cb = seen.clone();
        let opts = StreamOptions {
            api_key: Some("test-key".into()),
            metadata: Some(metadata),
            on_response: Some(std::sync::Arc::new(move |_status, headers, _model| {
                *seen_cb.lock().unwrap() = headers.get("x-pi-gateway-upstream-provider").cloned();
            })),
            ..Default::default()
        };
        let events = collect(stream_pi_messages(
            &model(&format!("{}/v1", server.uri())),
            &context(),
            &opts,
        ))
        .await;
        assert!(matches!(
            events.last(),
            Some(Event::Done {
                reason: StopReason::Stop,
                ..
            })
        ));
        assert_eq!(seen.lock().unwrap().as_deref(), Some("anthropic"));
    }

    #[tokio::test]
    async fn server_sent_error_no_key_and_missing_terminal_are_errors() {
        let server = MockServer::start().await;
        Mock::given(method("POST")).and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(200).insert_header("content-type", "text/event-stream").set_body_string(sse(vec![json!({"type":"start"}), json!({"type":"error","reason":"error","usage":usage(),"errorMessage":"Upstream failed"})])))
            .mount(&server).await;
        let opts = StreamOptions {
            api_key: Some("test-key".into()),
            ..Default::default()
        };
        let events = collect(stream_pi_messages(
            &model(&format!("{}/v1", server.uri())),
            &context(),
            &opts,
        ))
        .await;
        assert!(
            matches!(events.last(), Some(Event::Error { reason: StopReason::Error, message: Some(m), .. }) if m.error_message.as_deref() == Some("Upstream failed"))
        );

        let no_key = collect(stream_pi_messages(
            &model("http://127.0.0.1:1/v1"),
            &context(),
            &StreamOptions::default(),
        ))
        .await;
        assert!(
            matches!(no_key.last(), Some(Event::Error { error, .. }) if error.to_string().contains("No API key provided"))
        );

        let server2 = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_string(sse(vec![
                        json!({"type":"start"}),
                        json!({"type":"text_start","contentIndex":0}),
                        json!({"type":"text_delta","contentIndex":0,"delta":"partial"}),
                    ])),
            )
            .mount(&server2)
            .await;
        let missing = collect(stream_pi_messages(
            &model(&format!("{}/v1", server2.uri())),
            &context(),
            &opts,
        ))
        .await;
        assert!(
            matches!(missing.last(), Some(Event::Error { error, .. }) if error.to_string().contains("stream ended without a terminal event"))
        );
    }
}
