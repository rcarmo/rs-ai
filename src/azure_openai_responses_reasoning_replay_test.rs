//! Test-for-test port of upstream `azure-openai-responses-reasoning-replay.test.ts`.
//! Ensures terminal `response.completed.output` encrypted_content backfills only
//! when the prior `response.output_item.done` reasoning item omitted it.

#[cfg(test)]
mod tests {
    use crate::events::Event;
    use crate::provider::responses::{build_responses_payload, stream_responses};
    use crate::types::{
        ContentBlock, Context, Message, Model, ModelCost, Role, StopReason, StreamOptions, Usage,
    };
    use serde_json::{Value, json};
    use tokio_stream::StreamExt;
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn model(base_url: &str) -> Model {
        Model {
            id: "gpt-5-mini".into(),
            name: "GPT-5 Mini".into(),
            api: "azure-openai-responses".into(),
            provider: "azure-openai-responses".into(),
            base_url: base_url.into(),
            reasoning: true,
            thinking_level_map: None,
            input: vec!["text".into()],
            cost: ModelCost::default(),
            context_window: 400000,
            max_tokens: 128000,
            headers: None,
            api_key: Some("test".into()),
            compat: Default::default(),
        }
    }

    fn user(text: &str, timestamp: i64) -> Message {
        Message {
            role: Role::User,
            content: vec![ContentBlock::Text {
                text: text.into(),
                text_signature: None,
            }],
            timestamp,
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

    fn stream_body(done_item: Value, completed_item: Value) -> String {
        format!(
            "data: {}\n\ndata: {}\n\ndata: {}\n\ndata: [DONE]\n\n",
            json!({
                "type": "response.output_item.added",
                "output_index": 0,
                "sequence_number": 0,
                "item": {"type": "reasoning", "id": done_item["id"], "summary": []}
            }),
            json!({
                "type": "response.output_item.done",
                "output_index": 0,
                "sequence_number": 1,
                "item": done_item
            }),
            json!({
                "type": "response.completed",
                "sequence_number": 2,
                "response": {
                    "id": "resp_test",
                    "status": "completed",
                    "output": [completed_item]
                }
            }),
        )
    }

    async fn collect_assistant(done_item: Value, completed_item: Value) -> (Model, Message) {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_string(stream_body(done_item, completed_item)),
            )
            .mount(&server)
            .await;
        let m = model(&server.uri());
        let ctx = Context {
            system_prompt: None,
            tools: Vec::new(),
            messages: vec![user("first", 1)],
        };
        let opts = StreamOptions::default();
        let mut stream = stream_responses(&m, &ctx, &opts);
        let mut message = None;
        while let Some(evt) = stream.next().await {
            match evt {
                Event::Done { message: msg, .. } => message = Some(msg),
                Event::Error { error, .. } => panic!("unexpected error: {error}"),
                _ => {}
            }
        }
        drop(stream);
        (m, message.expect("Done"))
    }

    fn replayed_reasoning(model: &Model, assistant: Message) -> Value {
        let ctx = Context {
            system_prompt: None,
            tools: Vec::new(),
            messages: vec![user("first", 1), assistant, user("follow-up", 2)],
        };
        let payload = build_responses_payload(model, &ctx, &StreamOptions::default());
        payload["input"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["type"] == "reasoning")
            .expect("replayed reasoning")
            .clone()
    }

    #[tokio::test]
    async fn preserves_existing_encrypted_content_from_output_item_done() {
        let done_item = json!({
            "type": "reasoning",
            "id": "rs_done",
            "summary": [],
            "encrypted_content": "from-output-item-done"
        });
        let completed_item = json!({
            "type": "reasoning",
            "id": "rs_done",
            "summary": [],
            "encrypted_content": "from-response-completed"
        });
        let (model, assistant) = collect_assistant(done_item, completed_item).await;
        assert_eq!(assistant.stop_reason, Some(StopReason::Stop));
        assert!(matches!(assistant.usage, Some(Usage { .. }) | None));
        let replay = replayed_reasoning(&model, assistant);
        assert_eq!(replay["type"], "reasoning");
        assert_eq!(replay["id"], "rs_done");
        assert_eq!(replay["encrypted_content"], "from-output-item-done");
    }

    #[tokio::test]
    async fn fills_encrypted_content_when_output_item_done_omitted_it() {
        let done_item = json!({
            "type": "reasoning",
            "id": "rs_missing",
            "summary": []
        });
        let completed_item = json!({
            "type": "reasoning",
            "id": "rs_missing",
            "summary": [],
            "encrypted_content": "from-response-completed"
        });
        let (model, assistant) = collect_assistant(done_item, completed_item).await;
        let replay = replayed_reasoning(&model, assistant);
        assert_eq!(replay["type"], "reasoning");
        assert_eq!(replay["id"], "rs_missing");
        assert_eq!(replay["encrypted_content"], "from-response-completed");
    }
}
