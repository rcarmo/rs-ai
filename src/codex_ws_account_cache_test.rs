//! v0.84.0 Codex account-scoped WebSocket fallback cache regression.
//!
//! The sticky WS->SSE fallback cache is keyed by ChatGPT account id plus session
//! id, not session id alone. A failed WebSocket for one account must not force a
//! second account with the same session id onto SSE, while the original account
//! reuses SSE.

#[cfg(test)]
mod tests {
    use crate::events::Event;
    use crate::provider::codex::{WS_FALLBACK_TEST_LOCK, clear_ws_fallback, stream_codex};
    use crate::types::{
        ContentBlock, Context, Message, Model, ModelCost, Role, StreamOptions, Transport,
    };
    use base64::Engine;
    use futures::{SinkExt, StreamExt};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;
    use tokio_tungstenite::tungstenite::Message as WsMessage;

    fn token(account_id: &str) -> String {
        let payload = serde_json::json!({
            "https://api.openai.com/auth": {"chatgpt_account_id": account_id}
        });
        let encoded =
            base64::engine::general_purpose::STANDARD.encode(serde_json::to_vec(&payload).unwrap());
        format!("h.{encoded}.s")
    }

    fn codex_model(base_url: &str, api_key: &str) -> Model {
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
            api_key: Some(api_key.into()),
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
                tool_call_id: None,
                tool_name: None,
                is_error: false,
                details: None,
                added_tool_names: Vec::new(),
            }],
        }
    }

    async fn http_sse_response(mut stream: tokio::net::TcpStream, body: &str) {
        let mut buf = [0u8; 4096];
        let _ = stream.read(&mut buf).await;
        let response = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        let _ = stream.write_all(response.as_bytes()).await;
        let _ = stream.shutdown().await;
    }

    async fn run_text(model: &Model, opts: &StreamOptions) -> (String, bool) {
        let c = ctx();
        let mut stream = stream_codex(model, &c, opts);
        let mut text = String::new();
        let mut saw_error = false;
        while let Some(evt) = stream.next().await {
            match evt {
                Event::TextDelta { delta } => text.push_str(&delta),
                Event::Error { .. } => saw_error = true,
                _ => {}
            }
        }
        (text, saw_error)
    }

    #[tokio::test]
    async fn websocket_fallback_is_scoped_by_account_and_session() {
        let _guard = WS_FALLBACK_TEST_LOCK.lock().await;
        clear_ws_fallback(None);
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let observed = Arc::new(Mutex::new(Vec::<String>::new()));
        let observed_server = observed.clone();

        let server = tokio::spawn(async move {
            // 1. Account A: WebSocket connects but closes before a terminal event,
            // forcing the provider to record fallback and retry via SSE.
            let (stream, _) = listener.accept().await.unwrap();
            observed_server.lock().unwrap().push("a-ws-fail".into());
            let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();
            let _ = ws.next().await;
            let _ = ws.close(None).await;

            // 2. Account A SSE fallback.
            let (stream, _) = listener.accept().await.unwrap();
            observed_server.lock().unwrap().push("a-sse".into());
            http_sse_response(
                stream,
                "data: {\"type\":\"response.completed\",\"response\":{\"id\":\"r-a\",\"status\":\"completed\",\"usage\":{\"input_tokens\":1,\"output_tokens\":1,\"total_tokens\":2}}}\n\n",
            )
            .await;

            // 3. Account B, same session id: must still attempt WebSocket.
            let (stream, _) = listener.accept().await.unwrap();
            observed_server.lock().unwrap().push("b-ws".into());
            let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();
            let _ = ws.next().await;
            for frame in [
                r#"{"type":"response.created","response":{"id":"r-b"}}"#,
                r#"{"type":"response.output_text.delta","delta":"bee"}"#,
                r#"{"type":"response.completed","response":{"id":"r-b","status":"completed","usage":{"input_tokens":1,"output_tokens":1,"total_tokens":2}}}"#,
            ] {
                let _ = ws.send(WsMessage::Text(frame.into())).await;
            }
            let _ = ws.close(None).await;

            // 4. Account A, same session id: reuses its own sticky SSE fallback.
            let (stream, _) = listener.accept().await.unwrap();
            observed_server.lock().unwrap().push("a-sse-reuse".into());
            http_sse_response(
                stream,
                "data: {\"type\":\"response.completed\",\"response\":{\"id\":\"r-a2\",\"status\":\"completed\",\"usage\":{\"input_tokens\":1,\"output_tokens\":1,\"total_tokens\":2}}}\n\n",
            )
            .await;
        });

        let base = format!("http://{addr}");
        let opts = StreamOptions {
            session_id: Some("shared-session".into()),
            transport: Some(Transport::Auto),
            ..Default::default()
        };
        let a = codex_model(&base, &token("account-a"));
        let b = codex_model(&base, &token("account-b"));

        let (_a_text, a_error) = tokio::time::timeout(Duration::from_secs(5), run_text(&a, &opts))
            .await
            .expect("account A fallback stream timed out");
        assert!(!a_error, "account A fallback should complete through SSE");

        let (b_text, b_error) = tokio::time::timeout(Duration::from_secs(5), run_text(&b, &opts))
            .await
            .expect("account B stream timed out");
        assert!(!b_error, "account B should not inherit account A fallback");
        assert_eq!(b_text, "bee");

        let (_a2_text, a2_error) =
            tokio::time::timeout(Duration::from_secs(5), run_text(&a, &opts))
                .await
                .expect("account A fallback reuse timed out");
        assert!(
            !a2_error,
            "account A sticky SSE fallback should be reusable"
        );

        server.await.unwrap();
        assert_eq!(
            &*observed.lock().unwrap(),
            &["a-ws-fail", "a-sse", "b-ws", "a-sse-reuse"]
        );
        clear_ws_fallback(None);
    }
}
