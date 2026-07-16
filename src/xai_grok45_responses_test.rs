//! Verifies upstream `grok-4.5` xAI routing uses the OpenAI Responses request path.

#[cfg(test)]
mod tests {
    use crate::events::Event;
    use crate::provider::responses::stream_responses;
    use crate::registry::get_model;
    use crate::types::{ContentBlock, Context, Message, Role, StreamOptions};
    use serde_json::Value;
    use tokio_stream::StreamExt;
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

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
                error_message: None,
                tool_call_id: None,
                tool_name: None,
                is_error: false,
                details: None,
                added_tool_names: Vec::new(),
            }],
        }
    }

    #[tokio::test]
    async fn xai_grok_45_sends_actual_responses_request() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_string("data: {\"type\":\"response.completed\",\"response\":{\"id\":\"r\",\"status\":\"completed\",\"usage\":{\"input_tokens\":1,\"output_tokens\":1,\"total_tokens\":2}}}\n\ndata: [DONE]\n\n"),
            )
            .mount(&server)
            .await;
        let mut model = get_model("xai", "grok-4.5").expect("xai/grok-4.5");
        assert_eq!(model.api, crate::types::api::OPENAI_RESPONSES);
        model.base_url = server.uri();
        model.api_key = Some("xai-token".into());
        let c = ctx();
        let opts = StreamOptions::default();
        let mut stream = stream_responses(&model, &c, &opts);
        while let Some(event) = stream.next().await {
            if let Event::Error { error, .. } = event {
                panic!("unexpected error: {error}");
            }
        }
        let reqs = server.received_requests().await.unwrap();
        assert_eq!(reqs.len(), 1);
        assert_eq!(reqs[0].url.path(), "/responses");
        assert_eq!(
            reqs[0]
                .headers
                .get("authorization")
                .unwrap()
                .to_str()
                .unwrap(),
            "Bearer xai-token"
        );
        let body: Value = serde_json::from_slice(&reqs[0].body).unwrap();
        assert_eq!(body["model"], "grok-4.5");
        assert!(body.get("input").and_then(|v| v.as_array()).is_some());
    }
}
