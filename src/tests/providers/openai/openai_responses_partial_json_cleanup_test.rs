//! Test-for-test port of upstream `test/openai-responses-partial-json-cleanup.test.ts`
//! (`@earendil-works/pi-ai` v0.80.2).
//!
//! A streamed `function_call` accumulates argument deltas and, at
//! `output_item.done`, persists a tool-call block with fully-parsed arguments
//! and no streaming scratch buffer. (rs-ai's `ContentBlock::ToolCall` has no
//! `partialJson`/`partialArgs` field at all, so the "scratch removed" invariant
//! is structural; the substance is the fully-parsed arguments, asserted on both
//! the persisted block and the `toolcall_end` event.)

#[cfg(test)]
mod tests {
    use crate::events::Event;
    use crate::provider::responses::stream_responses;
    use crate::types::{ContentBlock, Context, Message, Model, ModelCost, Role, StreamOptions};
    use tokio_stream::StreamExt;
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn model(base_url: &str) -> Model {
        Model {
            id: "gpt-5-mini".into(),
            name: "GPT-5 Mini".into(),
            api: "openai-responses".into(),
            provider: "openai".into(),
            base_url: base_url.into(),
            reasoning: true,
            thinking_level_map: None,
            input: vec!["text".into()],
            cost: ModelCost::default(),
            context_window: 400000,
            max_tokens: 128000,
            sampling_params: None,
            headers: None,
            api_key: Some("test".into()),
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
                provider_thinking_level: None,
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
    async fn removes_partial_json_from_persisted_tool_call_blocks_at_output_item_done() {
        let body = concat!(
            "data: {\"type\":\"response.output_item.added\",\"item\":{\"type\":\"function_call\",\"id\":\"fc_test\",\"call_id\":\"call_test\",\"name\":\"edit\",\"arguments\":\"\"}}\n\n",
            "data: {\"type\":\"response.function_call_arguments.delta\",\"delta\":\"{\\\"path\\\":\\\"README.md\\\"\"}\n\n",
            "data: {\"type\":\"response.function_call_arguments.delta\",\"delta\":\",\\\"content\\\":\\\"updated\\\"}\"}\n\n",
            "data: {\"type\":\"response.function_call_arguments.done\",\"arguments\":\"{\\\"path\\\":\\\"README.md\\\",\\\"content\\\":\\\"updated\\\"}\"}\n\n",
            "data: {\"type\":\"response.output_item.done\",\"item\":{\"type\":\"function_call\",\"id\":\"fc_test\",\"call_id\":\"call_test\",\"name\":\"edit\",\"arguments\":\"{\\\"path\\\":\\\"README.md\\\",\\\"content\\\":\\\"updated\\\"}\"}}\n\n",
            "data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_test\",\"status\":\"completed\"}}\n\n",
            "data: [DONE]\n\n",
        ).to_string();
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_string(body),
            )
            .mount(&server)
            .await;
        let m = model(&server.uri());
        let c = ctx();
        let opts = StreamOptions::default();
        let mut stream = stream_responses(&m, &c, &opts);
        let mut end_args = None;
        let mut message = None;
        while let Some(evt) = stream.next().await {
            match evt {
                Event::ToolCallEnd { arguments, .. } => end_args = Some(arguments),
                Event::Done { message: msg, .. } => message = Some(msg),
                _ => {}
            }
        }
        let m = message.expect("Done");
        assert_eq!(m.content.len(), 1);
        match &m.content[0] {
            ContentBlock::ToolCall {
                name, arguments, ..
            } => {
                assert_eq!(name, "edit");
                assert_eq!(
                    arguments.get("path").and_then(|v| v.as_str()),
                    Some("README.md")
                );
                assert_eq!(
                    arguments.get("content").and_then(|v| v.as_str()),
                    Some("updated")
                );
            }
            other => panic!("expected toolCall, got {other:?}"),
        }
        // The toolcall_end event carries the same fully-parsed arguments.
        let ea = end_args.expect("toolcall_end");
        assert_eq!(ea.get("path").and_then(|v| v.as_str()), Some("README.md"));
        assert_eq!(ea.get("content").and_then(|v| v.as_str()), Some("updated"));
    }
}
