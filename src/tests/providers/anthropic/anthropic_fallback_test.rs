#[cfg(test)]
mod tests {
    use crate::events::Event;
    use crate::provider::anthropic::{build_anthropic_payload, stream_anthropic};
    use crate::types::{Context, Model, ModelCompat, ModelCost, StreamOptions};
    use futures::StreamExt;
    use serde_json::json;
    use std::collections::HashMap;
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn fallback_model(base_url: &str) -> Model {
        Model {
            id: "claude-sonnet-5".into(),
            name: "Claude Sonnet 5".into(),
            api: crate::types::api::ANTHROPIC_MESSAGES.into(),
            provider: "anthropic".into(),
            base_url: base_url.into(),
            reasoning: false,
            thinking_level_map: None,
            input: vec!["text".into()],
            cost: ModelCost {
                input: 3.0,
                output: 15.0,
                cache_read: 0.3,
                cache_write: 3.75,
                tiers: vec![],
            },
            context_window: 200000,
            max_tokens: 4096,
            sampling_params: None,
            headers: None,
            api_key: Some("sk-ant-test".into()),
            compat: ModelCompat {
                allowed_fallback_models: Some(json!([
                    {
                        "provider": "anthropic",
                        "model": "claude-opus-4-8",
                        "cost": {"input": 5.0, "output": 25.0, "cacheRead": 0.5, "cacheWrite": 6.25}
                    }
                ])),
                ..Default::default()
            },
        }
    }

    #[test]
    fn anthropic_payload_includes_server_side_fallbacks() {
        let model = fallback_model("https://example.invalid/v1");
        let payload = build_anthropic_payload(
            &model,
            &Context {
                system_prompt: None,
                messages: vec![crate::types::user_message("Hi")],
                tools: Vec::new(),
            },
            &StreamOptions::default(),
        );
        assert_eq!(payload["fallbacks"], json!([{"model":"claude-opus-4-8"}]));
    }

    #[tokio::test]
    async fn anthropic_stream_sends_fallback_beta_and_prices_response_model_usage() {
        let server = MockServer::start().await;
        let body = concat!(
            "event: message_start\n",
            "data: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_1\",\"model\":\"claude-opus-4-8\",\"usage\":{\"input_tokens\":1000,\"output_tokens\":0},\"content\":[]}}\n\n",
            "event: content_block_start\n",
            "data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"done\"}}\n\n",
            "event: content_block_stop\n",
            "data: {\"type\":\"content_block_stop\",\"index\":0}\n\n",
            "event: message_delta\n",
            "data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":2000}}\n\n",
            "event: message_stop\n",
            "data: {\"type\":\"message_stop\"}\n\n"
        );
        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_string(body),
            )
            .mount(&server)
            .await;

        let model = fallback_model(&server.uri());
        let ctx = Context {
            system_prompt: None,
            messages: vec![crate::types::user_message("Hi")],
            tools: Vec::new(),
        };
        let opts = StreamOptions {
            headers: Some(HashMap::from([("x-custom".into(), "present".into())])),
            ..Default::default()
        };
        let mut stream = stream_anthropic(&model, &ctx, &opts);
        let mut done = None;
        while let Some(event) = stream.next().await {
            match event {
                Event::Done { message, .. } => done = Some(message),
                Event::Error { error, .. } => panic!("unexpected error: {error}"),
                _ => {}
            }
        }
        let message = done.expect("done");
        assert_eq!(message.response_model.as_deref(), Some("claude-opus-4-8"));
        let usage = message.usage.expect("usage");
        assert_eq!(usage.input, 1000);
        assert_eq!(usage.output, 2000);
        assert!(
            (usage.cost.total - 0.055).abs() < 0.000001,
            "fallback cost: {:?}",
            usage.cost
        );

        let requests = server.received_requests().await.unwrap();
        let headers = &requests[0].headers;
        let beta = headers["anthropic-beta"].to_str().unwrap();
        assert!(beta.contains("server-side-fallback-2026-07-01"), "{beta}");
        assert_eq!(headers["x-custom"].to_str().unwrap(), "present");
        let payload: serde_json::Value = serde_json::from_slice(&requests[0].body).unwrap();
        assert_eq!(payload["fallbacks"], json!([{"model":"claude-opus-4-8"}]));
    }
}
