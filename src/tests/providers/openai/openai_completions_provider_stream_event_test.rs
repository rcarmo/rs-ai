//! v0.99.1 parsed provider stream event observer coverage.

#[cfg(test)]
mod tests {
    use crate::events::Event;
    use crate::provider::openai::stream_openai;
    use crate::types::{Context, StreamOptions, user_message};
    use std::sync::{Arc, Mutex};
    use tokio_stream::StreamExt;
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn exposes_openrouter_chunks_before_normalization() {
        let server = MockServer::start().await;
        let first = serde_json::json!({
            "id":"chatcmpl-1",
            "model":"anthropic/claude-sonnet-4.6",
            "choices":[{"index":0,"delta":{"content":"hello"}}]
        });
        let final_chunk = serde_json::json!({
            "id":"chatcmpl-1",
            "model":"anthropic/claude-sonnet-4.6",
            "choices":[{"index":0,"delta":{},"finish_reason":"stop"}],
            "usage":{"prompt_tokens":10,"completion_tokens":2,"total_tokens":12,"cost":0.0012,"is_byok":false},
            "openrouter_metadata":{"strategy":"direct","region":"iad"}
        });
        let body = format!("data: {first}\n\ndata: {final_chunk}\n\ndata: [DONE]\n\n");
        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_string(body),
            )
            .mount(&server)
            .await;
        let mut model = crate::registry::get_model("openrouter", "openrouter/auto").unwrap();
        model.base_url = server.uri();
        model.api_key = Some("test".into());
        let events = Arc::new(Mutex::new(Vec::new()));
        let captured = events.clone();
        let context = Context {
            system_prompt: None,
            messages: vec![user_message("hi")],
            tools: vec![],
        };
        let options = StreamOptions {
            on_provider_stream_event: Some(Arc::new(move |value, _| {
                captured.lock().unwrap().push(value);
                Ok(())
            })),
            ..Default::default()
        };
        let mut stream = stream_openai(&model, &context, &options);
        let mut done = None;
        while let Some(event) = stream.next().await {
            if let Event::Done { message, .. } = event {
                done = Some(message);
            }
        }
        let done = done.expect("done event");
        assert!(done.content.iter().any(|block| matches!(
            block,
            crate::types::ContentBlock::Text { text, .. } if text == "hello"
        )));
        assert_eq!(&*events.lock().unwrap(), &[first, final_chunk]);
    }

    #[tokio::test]
    async fn callback_failures_terminate_the_stream() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_string("data: {\"choices\":[{\"delta\":{\"content\":\"x\"}}]}\n\n"),
            )
            .mount(&server)
            .await;
        let mut model = crate::registry::get_model("openrouter", "openrouter/auto").unwrap();
        model.base_url = server.uri();
        model.api_key = Some("test".into());
        let context = Context {
            system_prompt: None,
            messages: vec![user_message("hi")],
            tools: vec![],
        };
        let options = StreamOptions {
            on_provider_stream_event: Some(Arc::new(|_, _| {
                Err(Box::<dyn std::error::Error + Send + Sync>::from(
                    "observer failed",
                ))
            })),
            ..Default::default()
        };
        let mut stream = stream_openai(&model, &context, &options);
        let mut error = None;
        while let Some(event) = stream.next().await {
            if let Event::Error { error: value, .. } = event {
                error = Some(value.to_string());
                break;
            }
        }
        assert_eq!(error.as_deref(), Some("observer failed"));
    }
}
