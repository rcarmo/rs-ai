//! Tests for `estimate.rs` (port of upstream `utils/estimate.ts` behavior).

#[cfg(test)]
mod tests {
    use crate::estimate::*;
    use crate::types::*;
    use std::collections::HashMap;

    fn usage(input: u32, output: u32, total: u32) -> Usage {
        Usage {
            input,
            output,
            total_tokens: total,
            ..Default::default()
        }
    }

    fn msg(
        role: Role,
        content: Vec<ContentBlock>,
        u: Option<Usage>,
        stop: Option<StopReason>,
    ) -> Message {
        msg_ts(role, content, u, stop, 0)
    }

    fn msg_ts(
        role: Role,
        content: Vec<ContentBlock>,
        u: Option<Usage>,
        stop: Option<StopReason>,
        timestamp: i64,
    ) -> Message {
        Message {
            role,
            content,
            timestamp,
            api: None,
            provider: None,
            model: None,
            response_id: None,
            response_model: None,
            diagnostics: Vec::new(),
            usage: u,
            stop_reason: stop,
            error_message: None,
            raw_stop_reason: None,
            tool_call_id: None,
            tool_name: None,
            is_error: false,
            details: None,
            added_tool_names: Vec::new(),
        }
    }

    fn text(s: &str) -> ContentBlock {
        ContentBlock::Text {
            text: s.into(),
            text_signature: None,
        }
    }

    #[test]
    fn text_tokens_ceil_div_4() {
        assert_eq!(estimate_text_tokens("12345678"), 2);
        assert_eq!(estimate_text_tokens("123456789"), 3); // ceil(9/4)
        assert_eq!(estimate_text_tokens(""), 0);
    }

    #[test]
    fn content_tokens_weight_images_at_4800() {
        // "abcd"(4) + image(4800) = 4804 -> ceil(4804/4) = 1201
        let content = vec![
            text("abcd"),
            ContentBlock::Image {
                data: "x".into(),
                mime_type: "image/png".into(),
            },
        ];
        assert_eq!(estimate_text_and_image_content_tokens(&content), 1201);
    }

    #[test]
    fn calculate_context_tokens_prefers_total_else_sums() {
        assert_eq!(calculate_context_tokens(&usage(10, 20, 99)), 99);
        let mut u = usage(10, 20, 0);
        u.cache_read = 5;
        u.cache_write = 3;
        assert_eq!(calculate_context_tokens(&u), 38); // 10+20+5+3
    }

    #[test]
    fn assistant_message_counts_text_thinking_and_toolcall_json() {
        let mut args = HashMap::new();
        args.insert("a".to_string(), serde_json::json!(1));
        let m = msg(
            Role::Assistant,
            vec![
                text("hello"), // 5
                ContentBlock::Thinking {
                    thinking: "think".into(),
                    thinking_signature: None,
                    redacted: false,
                }, // 5
                ContentBlock::ToolCall {
                    id: "1".into(),
                    name: "fn".into(),
                    arguments: args,
                    thought_signature: None,
                }, // 2 + len({"a":1})=7
            ],
            None,
            None,
        );
        // chars = 5 + 5 + (2 + 7) = 19 -> ceil(19/4) = 5
        assert_eq!(estimate_message_tokens(&m), 5);
    }

    #[test]
    fn context_anchors_on_last_assistant_usage_and_adds_trailing() {
        let messages = vec![
            msg(
                Role::User,
                vec![text("ignored because anchored")],
                None,
                None,
            ),
            msg(
                Role::Assistant,
                vec![text("done")],
                Some(usage(0, 0, 100)),
                Some(StopReason::Stop),
            ),
            msg(Role::User, vec![text("abcd")], None, None), // trailing: ceil(4/4)=1
        ];
        let ctx = Context {
            system_prompt: Some("sys".into()),
            tools: Vec::new(),
            messages,
        };
        let est = estimate_context_tokens(&ctx);
        assert_eq!(est.usage_tokens, 100);
        assert_eq!(est.trailing_tokens, 1);
        assert_eq!(est.tokens, 101); // no prefix added when anchored
        assert_eq!(est.last_usage_index, Some(1));
    }

    #[test]
    fn context_without_usage_adds_system_prefix() {
        // aborted assistant usage is skipped as an anchor
        let messages = vec![
            msg(
                Role::Assistant,
                vec![text("x")],
                Some(usage(0, 0, 500)),
                Some(StopReason::Aborted),
            ),
            msg(Role::User, vec![text("abcd")], None, None), // 1 token
        ];
        let ctx = Context {
            system_prompt: Some("12345678".into()),
            tools: Vec::new(),
            messages,
        }; // sys = 2 tokens
        let est = estimate_context_tokens(&ctx);
        assert_eq!(est.last_usage_index, None);
        // message tokens: assistant "x"=ceil(1/4)=1, user "abcd"=1 => 2; +sys 2 = 4
        assert_eq!(est.tokens, 4);
        assert_eq!(est.usage_tokens, 0);
    }

    #[test]
    fn ignores_stale_assistant_usage_after_a_newer_message_is_inserted_before_it() {
        // Test-for-test port of upstream v0.80.6 `context-estimate.test.ts`.
        let context = Context {
            system_prompt: Some("system".into()),
            tools: Vec::new(),
            messages: vec![
                msg_ts(Role::User, vec![text("summary")], None, None, 200),
                msg_ts(
                    Role::Assistant,
                    vec![text("kept")],
                    Some(usage(9500, 0, 9500)),
                    Some(StopReason::Stop),
                    100,
                ),
                msg_ts(Role::User, vec![text(&"x".repeat(4000))], None, None, 300),
            ],
        };

        let est = estimate_context_tokens(&context);
        assert_eq!(est.tokens, 1005);
        assert_eq!(est.usage_tokens, 0);
        assert_eq!(est.trailing_tokens, 1005);
        assert_eq!(est.last_usage_index, None);

        let mut model = crate::types::Model {
            id: "test-model".into(),
            name: "Test Model".into(),
            api: "openai-responses".into(),
            provider: "openai".into(),
            base_url: "https://api.openai.com/v1".into(),
            reasoning: false,
            thinking_level_map: None,
            input: vec!["text".into()],
            cost: ModelCost::default(),
            context_window: 10_000,
            max_tokens: 8_000,
            headers: None,
            api_key: None,
            compat: Default::default(),
        };
        // Keep the fixture exact: buildBaseOptions(model, context).maxTokens == 4_899.
        assert_eq!(
            crate::simple_options::clamp_max_tokens_to_context(&model, &context, model.max_tokens),
            4899
        );
        model.context_window = 0;
        assert_eq!(
            crate::simple_options::clamp_max_tokens_to_context(&model, &context, model.max_tokens),
            8000
        );
    }

    #[test]
    fn uses_assistant_usage_again_after_a_response_to_the_inserted_context() {
        // Test-for-test port of upstream v0.80.6 `context-estimate.test.ts`.
        let context = Context {
            system_prompt: None,
            tools: Vec::new(),
            messages: vec![
                msg_ts(Role::User, vec![text("summary")], None, None, 200),
                msg_ts(
                    Role::Assistant,
                    vec![text("kept")],
                    Some(usage(9500, 0, 9500)),
                    Some(StopReason::Stop),
                    100,
                ),
                msg_ts(Role::User, vec![text("new prompt")], None, None, 300),
                msg_ts(
                    Role::Assistant,
                    vec![text("kept")],
                    Some(usage(2000, 0, 2000)),
                    Some(StopReason::Stop),
                    400,
                ),
                msg_ts(Role::User, vec![text("tail")], None, None, 500),
            ],
        };

        let est = estimate_context_tokens(&context);
        assert_eq!(est.tokens, 2001);
        assert_eq!(est.usage_tokens, 2000);
        assert_eq!(est.trailing_tokens, 1);
        assert_eq!(est.last_usage_index, Some(3));
    }
}
