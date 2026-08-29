//! Adaptation of @go-ai `TestStreamViaWebSocketProtocolFlow`
//! (`inference/provider/openaicodex/codex_ws_test.go`) into idiomatic Rust.
//!
//! Exercises the full Codex WebSocket happy path end-to-end: the client sends a
//! `response.create` frame (carrying `model`), the server streams
//! created/output_item.added/output_text.delta/output_item.done/completed, and
//! the client must surface Start + TextDelta("ok") + Done(Stop) without error.
//! This is the coverage that would have caught the missing-handshake-header bug.

#[cfg(test)]
mod tests {
    use crate::events::Event;
    use crate::provider::codex::{WS_FALLBACK_TEST_LOCK, clear_ws_fallback, stream_codex};
    use crate::types::{
        ContentBlock, Context, Message, Model, ModelCost, Role, StopReason, StreamOptions,
        Transport,
    };
    use futures::{SinkExt, StreamExt};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;
    use tokio::net::TcpListener;
    use tokio_tungstenite::tungstenite::Message as WsMessage;

    fn codex_model(base_url: &str) -> Model {
        Model {
            id: "codex-mini".into(),
            name: "Codex mini".into(),
            api: "openai-codex-responses".into(),
            provider: "openai-codex".into(),
            base_url: base_url.into(),
            reasoning: false,
            thinking_level_map: None,
            input: vec!["text".into()],
            cost: ModelCost::default(),
            context_window: 256000,
            max_tokens: 16384,
            sampling_params: None,
            headers: None,
            api_key: Some("test-key".into()),
            compat: Default::default(),
        }
    }

    fn ctx() -> Context {
        Context {
            system_prompt: None,
            tools: Vec::new(),
            messages: vec![Message {
                role: Role::User,
                content: vec![ContentBlock::Text {
                    text: "hello".into(),
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

    #[tokio::test]
    async fn stream_via_websocket_protocol_flow() {
        let _guard = WS_FALLBACK_TEST_LOCK.lock().await;
        clear_ws_fallback(None);
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let received: Arc<Mutex<Option<serde_json::Value>>> = Arc::new(Mutex::new(None));
        let sink = received.clone();

        let server = tokio::spawn(async move {
            if let Ok((stream, _)) = listener.accept().await
                && let Ok(mut ws) = tokio_tungstenite::accept_async(stream).await
            {
                // Capture the outbound response.create frame.
                if let Some(Ok(WsMessage::Text(t))) = ws.next().await
                    && let Ok(v) = serde_json::from_str::<serde_json::Value>(&t)
                {
                    *sink.lock().unwrap() = Some(v);
                }
                for frame in [
                    r#"{"type":"response.created","response":{"id":"resp_1"}}"#,
                    r#"{"type":"response.output_item.added","item":{"type":"message","id":"item_1"}}"#,
                    r#"{"type":"response.output_text.delta","delta":"ok"}"#,
                    r#"{"type":"response.output_item.done","item":{"type":"message","id":"item_1"}}"#,
                    r#"{"type":"response.completed","response":{"id":"resp_1","status":"completed","usage":{"input_tokens":1,"output_tokens":1,"total_tokens":2}}}"#,
                ] {
                    let _ = ws.send(WsMessage::Text(frame.into())).await;
                }
                let _ = ws.close(None).await;
            }
        });

        let model = codex_model(&format!("http://{addr}"));
        let opts = StreamOptions {
            transport: Some(Transport::Auto),
            ..Default::default()
        };
        let c = ctx();

        let mut saw_start = false;
        let mut saw_text = false;
        let mut done_reason: Option<StopReason> = None;
        let mut saw_error = false;
        let run = async {
            let mut stream = stream_codex(&model, &c, &opts);
            while let Some(evt) = stream.next().await {
                match evt {
                    Event::Start { .. } => saw_start = true,
                    Event::TextDelta { delta } => saw_text |= delta == "ok",
                    Event::Done { reason, .. } => done_reason = Some(reason),
                    Event::Error { .. } => saw_error = true,
                    _ => {}
                }
            }
        };
        tokio::time::timeout(Duration::from_secs(5), run)
            .await
            .expect("codex ws stream timed out");
        let _ = server.await;

        // Outbound payload carries the model id.
        let payload = received
            .lock()
            .unwrap()
            .clone()
            .expect("server received a client frame");
        assert_eq!(payload["model"], serde_json::json!("codex-mini"));
        assert_eq!(payload["type"], serde_json::json!("response.create"));

        assert!(!saw_error, "happy-path WS flow must not surface an error");
        assert!(saw_start, "expected a Start event");
        assert!(saw_text, "expected a TextDelta(\"ok\") event");
        assert!(
            matches!(done_reason, Some(StopReason::Stop)),
            "expected Done(Stop), got {done_reason:?}"
        );
    }
}
