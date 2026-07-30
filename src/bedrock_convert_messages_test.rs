//! Test-for-test port of upstream `test/bedrock-convert-messages.test.ts`
//! (`@earendil-works/pi-ai` v0.80.2) — the cases representable in Rust.
//!
//! The "unknown content block" cases (`{type:"unknown"}` injected via `as any`)
//! and the lone-surrogate cases are N/A: rs-ai's `ContentBlock` enum is
//! exhaustive (no unknown variant) and Rust `String`s cannot hold lone
//! surrogates. The blank/placeholder/filter cases are ported via the extracted
//! `build_bedrock_messages`.

#[cfg(test)]
mod tests {
    use crate::provider::bedrock::build_bedrock_messages;
    use crate::types::{ContentBlock, Message, Model, ModelCost, Role, StreamOptions};
    use aws_sdk_bedrockruntime::types::ContentBlock as BedrockContent;

    fn model() -> Model {
        Model {
            id: "us.anthropic.claude-sonnet-4-5-20250929-v1:0".into(),
            name: "Sonnet 4.5".into(),
            api: "bedrock-converse-stream".into(),
            provider: "amazon-bedrock".into(),
            base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
            reasoning: true,
            thinking_level_map: None,
            input: vec!["text".into()],
            cost: ModelCost::default(),
            context_window: 200000,
            max_tokens: 8192,
            headers: None,
            api_key: None,
            compat: Default::default(),
        }
    }

    fn user(content: Vec<ContentBlock>) -> Message {
        Message {
            role: Role::User,
            content,
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
            raw_stop_reason: None,
            tool_call_id: None,
            tool_name: None,
            is_error: false,
            details: None,
            added_tool_names: Vec::new(),
        }
    }

    fn text(t: &str) -> ContentBlock {
        ContentBlock::Text {
            text: t.into(),
            text_signature: None,
        }
    }

    fn build(messages: Vec<Message>) -> Vec<aws_sdk_bedrockruntime::types::Message> {
        // cacheRetention none so no trailing cache point perturbs the content assertions.
        let opts = StreamOptions {
            cache_retention: Some(crate::types::CacheRetention::None),
            ..Default::default()
        };
        build_bedrock_messages(&messages, &model(), &opts).unwrap()
    }

    fn texts(m: &aws_sdk_bedrockruntime::types::Message) -> Vec<String> {
        m.content()
            .iter()
            .filter_map(|b| match b {
                BedrockContent::Text(t) => Some(t.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn replaces_blank_user_string_content_with_a_placeholder() {
        let msgs = build(vec![user(vec![text("   ")])]);
        assert_eq!(msgs.len(), 1);
        assert_eq!(texts(&msgs[0]), vec!["<empty>".to_string()]);
    }

    #[test]
    fn filters_blank_user_text_blocks_when_other_content_remains() {
        let msgs = build(vec![user(vec![text(""), text("hello")])]);
        assert_eq!(msgs.len(), 1);
        assert_eq!(texts(&msgs[0]), vec!["hello".to_string()]);
    }

    #[test]
    fn replaces_user_message_with_no_renderable_content_with_a_placeholder() {
        // A user message whose only block produces nothing (blank text) becomes "<empty>".
        let msgs = build(vec![user(vec![text("")])]);
        assert_eq!(msgs.len(), 1);
        assert_eq!(texts(&msgs[0]), vec!["<empty>".to_string()]);
    }

    #[test]
    fn skips_assistant_messages_with_no_renderable_content() {
        let assistant = Message {
            role: Role::Assistant,
            content: vec![text("")],
            timestamp: 0,
            api: Some("bedrock-converse-stream".into()),
            provider: Some("amazon-bedrock".into()),
            model: Some("us.anthropic.claude-sonnet-4-5-20250929-v1:0".into()),
            response_id: None,
            response_model: None,
            diagnostics: Vec::new(),
            usage: None,
            stop_reason: Some(crate::types::StopReason::Stop),
            error_message: None,
            raw_stop_reason: None,
            tool_call_id: None,
            tool_name: None,
            is_error: false,
            details: None,
            added_tool_names: Vec::new(),
        };
        let msgs = build(vec![assistant]);
        assert_eq!(msgs.len(), 0, "assistant with only blank text is dropped");
    }

    #[test]
    fn replaces_blank_tool_result_content_with_a_placeholder() {
        let tool_result = Message {
            role: Role::ToolResult,
            content: vec![text("")],
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
            raw_stop_reason: None,
            tool_call_id: Some("tool-1".into()),
            tool_name: Some("tool".into()),
            is_error: false,
            details: None,
            added_tool_names: Vec::new(),
        };
        let msgs = build(vec![tool_result]);
        assert_eq!(msgs.len(), 1);
        // The single tool-result block carries a "<empty>" text placeholder.
        let tr = msgs[0]
            .content()
            .iter()
            .find_map(|b| match b {
                BedrockContent::ToolResult(tr) => Some(tr),
                _ => None,
            })
            .expect("a toolResult block");
        let placeholder = tr.content().iter().any(|c| {
            matches!(c,
            aws_sdk_bedrockruntime::types::ToolResultContentBlock::Text(t) if t == "<empty>")
        });
        assert!(
            placeholder,
            "blank tool result must carry the <empty> placeholder"
        );
    }
}
