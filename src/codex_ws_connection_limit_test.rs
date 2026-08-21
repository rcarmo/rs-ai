//! Closes the WS connection-limit retry gap (identified vs upstream/@go-ai
//! `isWebSocketConnectionLimitReachedError` + `retriedWebSocketConnectionLimit`).
//!
//! Upstream retries the Codex WebSocket connection ONCE when the server rejects
//! it with `websocket_connection_limit_reached` before the message stream starts,
//! only falling back to SSE if the retry also fails. This test stands up a real
//! WS server that rejects the first connection with the connection-limit error
//! and serves a valid stream on the second, and asserts the retry succeeds (text
//! is produced without falling back).

#[cfg(test)]
mod tests {
    use crate::events::Event;
    use crate::provider::codex::{
        WS_CONNECTION_LIMIT_CODE, WS_FALLBACK_TEST_LOCK, clear_ws_fallback,
        is_ws_connection_limit_error, stream_codex,
    };
    use crate::types::{
        ContentBlock, Context, Message, Model, ModelCost, Role, StreamOptions, Transport,
    };
    use futures::{SinkExt, StreamExt};
    use std::time::Duration;
    use tokio::net::TcpListener;
    use tokio_tungstenite::tungstenite::Message as WsMessage;

    #[test]
    fn detects_connection_limit_marker() {
        assert!(is_ws_connection_limit_error(&format!(
            "{WS_CONNECTION_LIMIT_CODE}: too many"
        )));
        assert!(!is_ws_connection_limit_error(
            "WebSocket stream closed before response.completed"
        ));
        assert!(!is_ws_connection_limit_error("some other error"));
    }

    fn codex_model(base_url: &str) -> Model {
        Model {
            id: "gpt-5.4-mini".into(),
            name: "GPT-5.4 mini".into(),
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

    #[tokio::test]
    async fn retries_websocket_once_on_connection_limit_then_succeeds() {
        let _guard = WS_FALLBACK_TEST_LOCK.lock().await;
        clear_ws_fallback(None);
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let server = tokio::spawn(async move {
            // Attempt 1: reject with a connection-limit error event.
            if let Ok((stream, _)) = listener.accept().await
                && let Ok(mut ws) = tokio_tungstenite::accept_async(stream).await
            {
                let _ = ws.next().await; // consume the response.create frame
                let _ = ws.send(WsMessage::Text(
                    r#"{"type":"error","code":"websocket_connection_limit_reached","message":"too many connections"}"#.into(),
                )).await;
                let _ = ws.close(None).await;
            }
            // Attempt 2 (the retry): serve a valid stream.
            if let Ok((stream, _)) = listener.accept().await
                && let Ok(mut ws) = tokio_tungstenite::accept_async(stream).await
            {
                let _ = ws.next().await; // consume the response.create frame
                for frame in [
                    r#"{"type":"response.created","response":{"id":"r","model":"gpt-5.4-mini"}}"#,
                    r#"{"type":"response.output_text.delta","delta":"hello"}"#,
                    r#"{"type":"response.completed","response":{"id":"r","model":"gpt-5.4-mini","usage":{"input_tokens":1,"output_tokens":1,"total_tokens":2}}}"#,
                ] {
                    let _ = ws.send(WsMessage::Text(frame.into())).await;
                }
                let _ = ws.close(None).await;
            }
        });

        let model = codex_model(&format!("http://{addr}"));
        // transport=Auto (default) so WebSocket is attempted; no session id so the
        // sticky SSE-fallback set stays clean.
        let opts = StreamOptions {
            transport: Some(Transport::Auto),
            ..Default::default()
        };
        let c = ctx();

        let mut text = String::new();
        let mut saw_error = false;
        let run = async {
            let mut stream = stream_codex(&model, &c, &opts);
            while let Some(evt) = stream.next().await {
                match evt {
                    Event::TextDelta { delta } => text.push_str(&delta),
                    Event::Error { .. } => saw_error = true,
                    _ => {}
                }
            }
        };
        // Bound the test: a working retry produces the stream quickly; a regression
        // (surfacing the error or SSE-falling-back to the non-HTTP listener) must not hang.
        tokio::time::timeout(Duration::from_secs(5), run)
            .await
            .expect("codex stream timed out");
        let _ = server.await;

        assert!(
            !saw_error,
            "connection-limit retry must not surface an error"
        );
        assert_eq!(
            text, "hello",
            "the retried WebSocket stream must produce the text"
        );
    }
}
