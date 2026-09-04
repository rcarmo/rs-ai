//! Test-for-test port of upstream `test/anthropic-cache-write-1h-cost.test.ts`
//! (`@earendil-works/pi-ai` v0.80.2).
//!
//! The 1h portion of a cache write is priced at 2x base input; the remainder at
//! the 5m cacheWrite rate. With no `cache_creation` breakdown the whole write
//! falls back to the 5m rate (cacheWrite1h = 0).

#[cfg(test)]
mod tests {
    use crate::events::Event;
    use crate::provider::anthropic::stream_anthropic;
    use crate::registry::get_model;
    use crate::types::{ContentBlock, Context, Message, Model, Role, StreamOptions};
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

    async fn run(model: Model, body: String) -> crate::types::Usage {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_string(body),
            )
            .mount(&server)
            .await;
        let mut model = model;
        model.base_url = server.uri();
        model.api_key = Some("test".into());
        let c = ctx();
        let opts = StreamOptions::default();
        let mut stream = stream_anthropic(&model, &c, &opts);
        let mut usage = None;
        while let Some(evt) = stream.next().await {
            if let Event::Done { message, .. } = evt {
                usage = message.usage;
            }
        }
        usage.expect("usage")
    }

    fn body(start_usage_extra: &str) -> String {
        format!(
            concat!(
                "event: message_start\ndata: {{\"type\":\"message_start\",\"message\":{{\"id\":\"msg_test\",\"usage\":{{\"input_tokens\":100,\"output_tokens\":0,\"cache_read_input_tokens\":0,\"cache_creation_input_tokens\":1000000{extra}}}}}}}\n\n",
                "event: content_block_start\ndata: {{\"type\":\"content_block_start\",\"index\":0,\"content_block\":{{\"type\":\"text\",\"text\":\"\"}}}}\n\n",
                "event: content_block_delta\ndata: {{\"type\":\"content_block_delta\",\"index\":0,\"delta\":{{\"type\":\"text_delta\",\"text\":\"Hi\"}}}}\n\n",
                "event: content_block_stop\ndata: {{\"type\":\"content_block_stop\",\"index\":0}}\n\n",
                "event: message_delta\ndata: {{\"type\":\"message_delta\",\"delta\":{{\"stop_reason\":\"end_turn\"}},\"usage\":{{\"input_tokens\":100,\"output_tokens\":5,\"cache_read_input_tokens\":0,\"cache_creation_input_tokens\":1000000}}}}\n\n",
                "event: message_stop\ndata: {{\"type\":\"message_stop\"}}\n\n",
            ),
            extra = start_usage_extra,
        )
    }

    #[tokio::test]
    async fn prices_the_1h_portion_at_2x_input_and_the_rest_at_the_5m_rate() {
        let extra = ",\"cache_creation\":{\"ephemeral_5m_input_tokens\":600000,\"ephemeral_1h_input_tokens\":400000}";
        let u = run(
            get_model("anthropic", "claude-opus-4-8").unwrap(),
            body(extra),
        )
        .await;
        assert_eq!(u.cache_write, 1_000_000);
        assert_eq!(u.cache_write_1h, Some(400_000));
        // 600k * 6.25/Mtok + 400k * 10/Mtok = 3.75 + 4.0 = 7.75
        assert!(
            (u.cost.cache_write - 7.75).abs() < 1e-9,
            "cacheWrite cost = {}",
            u.cost.cache_write
        );
    }

    #[tokio::test]
    async fn falls_back_to_the_5m_rate_when_no_breakdown_is_reported() {
        let u = run(get_model("anthropic", "claude-opus-4-8").unwrap(), body("")).await;
        assert_eq!(u.cache_write, 1_000_000);
        assert_eq!(u.cache_write_1h.unwrap_or(0), 0);
        // 1M * 6.25/Mtok = 6.25
        assert!(
            (u.cost.cache_write - 6.25).abs() < 1e-9,
            "cacheWrite cost = {}",
            u.cost.cache_write
        );
    }
}
