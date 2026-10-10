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
            duration_ms: None,
            api: None,
            provider: None,
            model: None,
            response_id: None,
            response_model: None,
            provider_thinking_level: None,
            thinking_level: None,
            diagnostics: Vec::new(),
            usage: u,
            stop_reason: stop,
            deferred: None,
            error_message: None,
            raw_stop_reason: None,
            end_turn: None,
            tool_call_id: None,
            tool_name: None,
            is_error: false,
            details: None,
            nested_calls: None,
            added_tool_names: Vec::new(),
            sections: None,
            tools_added: Vec::new(),
            tools_removed: Vec::new(),
        }
    }

    fn text(s: &str) -> ContentBlock {
        ContentBlock::Text {
            text: s.into(),
            text_signature: None,
        }
    }

    #[test]
    fn text_tokens_reserve_three_and_a_half_utf16_units_per_token() {
        assert_eq!(estimate_text_tokens("1234567"), 2);
        assert_eq!(estimate_text_tokens("12345678"), 3);
        assert_eq!(estimate_text_tokens("123456789"), 3);
        assert_eq!(estimate_text_tokens("éééé"), 2);
        assert_eq!(estimate_text_tokens("😀😀"), 2);
        assert_eq!(estimate_text_tokens(""), 0);
    }

    #[test]
    fn v110_large_new_input_limits_output_with_three_and_a_half_ratio() {
        let context = Context {
            system_prompt: None,
            tools: Vec::new(),
            messages: vec![
                msg_ts(
                    Role::Assistant,
                    vec![text("kept")],
                    Some(usage(2000, 0, 2000)),
                    Some(StopReason::Stop),
                    100,
                ),
                msg_ts(Role::User, vec![text(&"x".repeat(3500))], None, None, 200),
            ],
        };
        assert_eq!(
            estimate_context_tokens(&context),
            ContextEstimate {
                tokens: 3000,
                usage_tokens: 2000,
                trailing_tokens: 1000,
                last_usage_index: Some(0),
            }
        );
        let mut model =
            crate::registry::get_model(crate::types::provider_id::OPENAI, "gpt-4o-mini")
                .expect("built-in fixture model");
        model.context_window = 10_000;
        model.max_tokens = 8_000;
        assert_eq!(
            crate::simple_options::clamp_max_tokens_to_context(&model, &context, 8000),
            2904
        );
    }

    #[test]
    fn system_messages_include_added_and_removed_tool_definitions() {
        let mut message = msg(Role::System, vec![text("prompt")], None, None);
        message.tools_added.push(Tool {
            name: "echo".into(),
            description: "Echo input".into(),
            parameters: serde_json::json!({"type":"object"}),
            constrained_sampling: None,
        });
        message
            .tools_removed
            .push(ToolReference { name: "old".into() });
        let expected = estimate_text_tokens("prompt")
            + estimate_text_tokens(&serde_json::to_string(&message.tools_added).unwrap())
            + estimate_text_tokens(&serde_json::to_string(&message.tools_removed).unwrap());
        assert_eq!(estimate_message_tokens(&message), expected);
    }

    #[test]
    fn content_tokens_weight_images_at_4800() {
        // "abcd"(4) + image(4800) = 4804 -> ceil(4804/3.5) = 1373
        let content = vec![
            text("abcd"),
            ContentBlock::Image {
                data: "x".into(),
                mime_type: "image/png".into(),
            },
        ];
        assert_eq!(estimate_text_and_image_content_tokens(&content), 1373);
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
                    redacted: None,
                }, // 5
                ContentBlock::ToolCall {
                    id: "1".into(),
                    name: "fn".into(),
                    arguments: args,
                    thought_signature: None,
                    namespace: None,
                }, // 2 + len({"a":1})=7
            ],
            None,
            None,
        );
        // chars = 5 + 5 + (2 + 7) = 19 -> ceil(19/3.5) = 6
        assert_eq!(estimate_message_tokens(&m), 6);
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
            msg(Role::User, vec![text("abcd")], None, None), // trailing: ceil(4/3.5)=2
        ];
        let ctx = Context {
            system_prompt: Some("sys".into()),
            tools: Vec::new(),
            messages,
        };
        let est = estimate_context_tokens(&ctx);
        assert_eq!(est.usage_tokens, 100);
        assert_eq!(est.trailing_tokens, 2);
        assert_eq!(est.tokens, 102); // no prefix added when anchored
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
            msg(Role::User, vec![text("abcd")], None, None), // 2 tokens
        ];
        let ctx = Context {
            system_prompt: Some("12345678".into()),
            tools: Vec::new(),
            messages,
        }; // sys = 3 tokens
        let est = estimate_context_tokens(&ctx);
        assert_eq!(est.last_usage_index, None);
        // message tokens: assistant "x"=1, user "abcd"=2; +system 3 = 6
        assert_eq!(est.tokens, 6);
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
        assert_eq!(est.tokens, 1149);
        assert_eq!(est.usage_tokens, 0);
        assert_eq!(est.trailing_tokens, 1149);
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
            input_limits: None,
            prompt_cache: None,
            enabled: None,
            lab: None,
            providers: None,
            cost: ModelCost::default(),
            context_window: 10_000,
            max_tokens: 8_000,
            sampling_params: None,
            sampling_params_by_thinking_level: None,
            headers: None,
            api_key: None,
            compat: Default::default(),
        };
        // v1.1.0 upstream fixture: buildBaseOptions(model, context).maxTokens == 4_755.
        assert_eq!(
            crate::simple_options::clamp_max_tokens_to_context(&model, &context, model.max_tokens),
            4755
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
        assert_eq!(est.tokens, 2002);
        assert_eq!(est.usage_tokens, 2000);
        assert_eq!(est.trailing_tokens, 2);
        assert_eq!(est.last_usage_index, Some(3));
    }
}
