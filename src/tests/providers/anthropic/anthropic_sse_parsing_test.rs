//! Test-for-test port of upstream `test/anthropic-sse-parsing.test.ts`
//! (`@earendil-works/pi-ai` v0.80.2).
//!
//! Raw Anthropic SSE parsing: malformed event/tool JSON is repaired (invalid
//! escapes + raw control chars); a `refusal` stop surfaces its explanation as
//! the error message; and unknown events after `message_stop` are ignored.

#[cfg(test)]
mod tests {
    use crate::events::Event;
    use crate::provider::anthropic::stream_anthropic;
    use crate::registry::get_model;
    use crate::types::{ContentBlock, Context, Message, Model, StreamOptions, Tool};
    use serde_json::json;
    use tokio_stream::StreamExt;
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn anthropic(id: &str, base_url: &str) -> Model {
        let mut m = get_model("anthropic", id).unwrap_or_else(|| panic!("catalog anthropic/{id}"));
        m.base_url = base_url.into();
        m.api_key = Some("test".into());
        m
    }

    fn user_ctx(text: &str, tools: Vec<Tool>) -> Context {
        Context {
            system_prompt: None,
            tools,
            messages: vec![Message {
                role: Role::User,
                content: vec![ContentBlock::Text {
                    text: text.into(),
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
    use crate::types::Role;

    async fn run(model: Model, ctx: Context, body: String) -> crate::types::Message {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_string(body),
            )
            .mount(&server)
            .await;
        let model = anthropic(&model.id, &server.uri());
        let opts = StreamOptions::default();
        let mut stream = stream_anthropic(&model, &ctx, &opts);
        let mut out = None;
        while let Some(evt) = stream.next().await {
            match evt {
                Event::Done {
                    reason,
                    mut message,
                } => {
                    message.stop_reason = Some(reason);
                    out = Some(message);
                }
                Event::Error {
                    reason,
                    message: Some(mut m),
                    ..
                } => {
                    m.stop_reason = Some(reason);
                    out = Some(m);
                }
                _ => {}
            }
        }
        out.expect("a terminal event")
    }

    #[tokio::test]
    async fn repairs_malformed_sse_json_and_malformed_streamed_tool_json() {
        // partial_json string value is `{"path":"A\H","text":"col1<TAB>col2"}` — an
        // invalid `\H` escape and a raw tab, both of which must be repaired.
        let malformed_delta = "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"{\\\"path\\\":\\\"A\\H\\\",\\\"text\\\":\\\"col1\tcol2\\\"}\"}}\n\n";
        let body = format!(
            concat!(
                "event: message_start\ndata: {{\"type\":\"message_start\",\"message\":{{\"id\":\"msg_test\",\"usage\":{{\"input_tokens\":12,\"output_tokens\":0}}}}}}\n\n",
                "event: content_block_start\ndata: {{\"type\":\"content_block_start\",\"index\":0,\"content_block\":{{\"type\":\"tool_use\",\"id\":\"toolu_test\",\"name\":\"edit\",\"input\":{{}}}}}}\n\n",
                "{delta}",
                "event: content_block_stop\ndata: {{\"type\":\"content_block_stop\",\"index\":0}}\n\n",
                "event: message_delta\ndata: {{\"type\":\"message_delta\",\"delta\":{{\"stop_reason\":\"tool_use\"}},\"usage\":{{\"input_tokens\":12,\"output_tokens\":5}}}}\n\n",
                "event: message_stop\ndata: {{\"type\":\"message_stop\"}}\n\n",
            ),
            delta = malformed_delta,
        );
        let tools = vec![Tool {
            name: "edit".into(),
            description: "Edit a file.".into(),
            parameters: json!({"type": "object", "properties": {"path": {"type": "string"}, "text": {"type": "string"}}, "required": ["path", "text"]}),
            constrained_sampling: None,
        }];
        let m = run(
            anthropic("claude-haiku-4-5", "http://x"),
            user_ctx("Use the edit tool.", tools),
            body,
        )
        .await;
        assert!(matches!(
            m.stop_reason,
            Some(crate::types::StopReason::ToolUse)
        ));
        assert!(m.error_message.is_none());
        let tc = m
            .content
            .iter()
            .find_map(|b| match b {
                ContentBlock::ToolCall { arguments, .. } => Some(arguments),
                _ => None,
            })
            .expect("toolCall");
        assert_eq!(tc.get("path").and_then(|v| v.as_str()), Some("A\\H"));
        assert_eq!(tc.get("text").and_then(|v| v.as_str()), Some("col1\tcol2"));
    }

    #[tokio::test]
    async fn preserves_refusal_stop_details_from_message_delta() {
        let explanation = "This request triggered restrictions on violative cyber content and was blocked under Anthropic's Usage Policy.";
        let body = format!(
            concat!(
                "event: message_start\ndata: {{\"type\":\"message_start\",\"message\":{{\"id\":\"msg_1\",\"usage\":{{\"input_tokens\":412,\"output_tokens\":0}}}}}}\n\n",
                "event: message_delta\ndata: {{\"type\":\"message_delta\",\"delta\":{{\"stop_reason\":\"refusal\",\"stop_details\":{{\"type\":\"refusal\",\"category\":\"cyber\",\"explanation\":\"{e}\"}}}},\"usage\":{{\"input_tokens\":412,\"output_tokens\":0}}}}\n\n",
                "event: message_stop\ndata: {{\"type\":\"message_stop\"}}\n\n",
            ),
            e = explanation,
        );
        let m = run(
            anthropic("claude-fable-5", "http://x"),
            user_ctx("blocked request", Vec::new()),
            body,
        )
        .await;
        assert!(matches!(
            m.stop_reason,
            Some(crate::types::StopReason::Error)
        ));
        assert_eq!(m.error_message.as_deref(), Some(explanation));
    }

    #[tokio::test]
    async fn ignores_unknown_sse_events_after_message_stop() {
        let body = concat!(
            "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_test\",\"usage\":{\"input_tokens\":12,\"output_tokens\":0}}}\n\n",
            "event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n",
            "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hello\"}}\n\n",
            "event: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":0}\n\n",
            "event: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"input_tokens\":12,\"output_tokens\":5}}\n\n",
            "event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n",
            "event: done\ndata: [DONE]\n\n",
            "event: proxy.stats\ndata: not json\n\n",
        ).to_string();
        let m = run(
            anthropic("claude-haiku-4-5", "http://x"),
            user_ctx("Say hello.", Vec::new()),
            body,
        )
        .await;
        assert!(matches!(
            m.stop_reason,
            Some(crate::types::StopReason::Stop)
        ));
        assert!(m.error_message.is_none());
        assert_eq!(m.content.len(), 1);
        assert!(matches!(&m.content[0], ContentBlock::Text { text, .. } if text == "Hello"));
    }

    #[tokio::test]
    async fn preserves_initial_text_and_thinking_from_content_block_start() {
        let body = concat!(
            "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_test\",\"usage\":{\"input_tokens\":12,\"output_tokens\":0}}}\n\n",
            "event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"thinking\",\"thinking\":\"seed-think\",\"signature\":\"sig0\"}}\n\n",
            "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"thinking_delta\",\"thinking\":\"+delta\"}}\n\n",
            "event: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":0}\n\n",
            "event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":1,\"content_block\":{\"type\":\"text\",\"text\":\"seed-text\"}}\n\n",
            "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":1,\"delta\":{\"type\":\"text_delta\",\"text\":\"+delta\"}}\n\n",
            "event: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":1}\n\n",
            "event: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"input_tokens\":12,\"output_tokens\":5}}\n\n",
            "event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n",
        ).to_string();
        let m = run(
            anthropic("claude-haiku-4-5", "http://x"),
            user_ctx("Say hello.", Vec::new()),
            body,
        )
        .await;
        assert_eq!(m.content.len(), 2);
        assert!(
            matches!(&m.content[0], ContentBlock::Thinking { thinking, thinking_signature, .. } if thinking == "seed-think+delta" && thinking_signature.as_deref() == Some("sig0"))
        );
        assert!(
            matches!(&m.content[1], ContentBlock::Text { text, .. } if text == "seed-text+delta")
        );
    }

    #[tokio::test]
    async fn captures_reasoning_tokens_from_message_delta() {
        let body = concat!(
            "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_test\",\"usage\":{\"input_tokens\":12,\"output_tokens\":0}}}\n\n",
            "event: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"input_tokens\":12,\"output_tokens\":40,\"output_tokens_details\":{\"thinking_tokens\":25}}}\n\n",
            "event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n",
        ).to_string();
        let m = run(
            anthropic("claude-haiku-4-5", "http://x"),
            user_ctx("Think.", Vec::new()),
            body,
        )
        .await;
        assert_eq!(m.usage.as_ref().and_then(|u| u.reasoning), Some(25));
    }
}
