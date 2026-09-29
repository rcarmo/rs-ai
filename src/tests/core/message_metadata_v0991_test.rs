//! v0.99.1 assistant thinking-level and nested tool-call message metadata.

#[cfg(test)]
mod tests {
    use crate::provider::anthropic::build_anthropic_payload;
    use crate::provider::openai::build_payload;
    use crate::types::{
        ContentBlock, Context, Message, ModelThinkingLevel, NestedToolCallRecord,
        NestedToolCallStatus, NestedToolCalls, Role, StreamOptions,
    };

    fn tool_result() -> Message {
        Message {
            role: Role::ToolResult,
            content: vec![ContentBlock::Text {
                text: "done".into(),
                text_signature: None,
            }],
            timestamp: 1,
            api: None,
            provider: None,
            model: None,
            response_id: None,
            response_model: None,
            provider_thinking_level: None,
            thinking_level: None,
            diagnostics: Vec::new(),
            usage: None,
            stop_reason: None,
            deferred: None,
            error_message: None,
            raw_stop_reason: None,
            end_turn: None,
            tool_call_id: Some("outer".into()),
            tool_name: Some("run".into()),
            is_error: false,
            details: None,
            nested_calls: Some(NestedToolCalls {
                calls: vec![NestedToolCallRecord {
                    id: "inner".into(),
                    name: "read".into(),
                    arguments: Some(
                        serde_json::from_value(serde_json::json!({"path":"a.rs"})).unwrap(),
                    ),
                    arguments_bytes: None,
                    status: NestedToolCallStatus::Ok,
                    duration_ms: Some(12),
                    error: None,
                }],
                complete: true,
            }),
            added_tool_names: Vec::new(),
            sections: None,
            tools_added: Vec::new(),
            tools_removed: Vec::new(),
        }
    }

    #[test]
    fn thinking_level_and_nested_calls_use_camel_case_and_round_trip() {
        let mut assistant = crate::types::user_message("hi");
        assistant.role = Role::Assistant;
        assistant.thinking_level = Some(ModelThinkingLevel::High);
        assistant.provider_thinking_level = Some("max".into());
        let encoded = serde_json::to_value(&assistant).unwrap();
        assert_eq!(encoded["thinkingLevel"], "high");
        assert_eq!(encoded["providerThinkingLevel"], "max");
        assert!(encoded.get("thinking_level").is_none());

        let message = tool_result();
        let encoded = serde_json::to_value(&message).unwrap();
        assert_eq!(encoded["nestedCalls"]["calls"][0]["durationMs"], 12);
        assert_eq!(encoded["nestedCalls"]["complete"], true);
        assert_eq!(
            serde_json::from_value::<Message>(encoded)
                .unwrap()
                .nested_calls,
            message.nested_calls
        );
    }

    #[test]
    fn nested_calls_are_not_sent_to_openai_or_anthropic() {
        let context = Context {
            system_prompt: None,
            messages: vec![tool_result()],
            tools: Vec::new(),
        };
        let openai = crate::registry::get_model("openai", "gpt-4o-mini").unwrap();
        let payload = build_payload(
            &openai,
            &context,
            &StreamOptions::default(),
            &crate::compat::detect_compat(&openai),
        );
        let text = serde_json::to_string(&payload).unwrap();
        assert!(!text.contains("nestedCalls"));
        assert!(!text.contains("durationMs"));

        let anthropic = crate::registry::get_model("anthropic", "claude-sonnet-4-6").unwrap();
        let payload = build_anthropic_payload(&anthropic, &context, &StreamOptions::default());
        let text = serde_json::to_string(&payload).unwrap();
        assert!(!text.contains("nestedCalls"));
        assert!(!text.contains("durationMs"));
    }
}
