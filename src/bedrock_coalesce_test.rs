//! Adaptation of @go-ai `TestConvertMessagesCoalescesConsecutiveToolResults`
//! (`inference/provider/bedrock/bedrock_test.go`) into idiomatic Rust.
//!
//! Two consecutive tool-result messages must coalesce into a single Bedrock
//! `user` message, and (for a cache-supporting Claude model with default short
//! retention) that message carries a trailing cache point: 2 tool results + 1
//! cache point = 3 content blocks.

#[cfg(test)]
mod tests {
    use crate::provider::bedrock::build_bedrock_messages;
    use crate::types::{ContentBlock, Message, Model, ModelCost, Role, StreamOptions};
    use aws_sdk_bedrockruntime::types::{ContentBlock as BedrockContent, ConversationRole};

    fn bedrock_model(id: &str) -> Model {
        Model {
            id: id.into(),
            name: "Claude".into(),
            api: "bedrock-converse-stream".into(),
            provider: "amazon-bedrock".into(),
            base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
            reasoning: false,
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

    fn user(text: &str) -> Message {
        Message {
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
            diagnostics: Vec::new(),
            usage: None,
            stop_reason: None,
            error_message: None,
            tool_call_id: None,
            tool_name: None,
            is_error: false,
            details: None,
        }
    }

    fn tool_result(id: &str, name: &str, text: &str, is_error: bool) -> Message {
        Message {
            role: Role::ToolResult,
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
            diagnostics: Vec::new(),
            usage: None,
            stop_reason: None,
            error_message: None,
            tool_call_id: Some(id.into()),
            tool_name: Some(name.into()),
            is_error,
            details: None,
        }
    }

    #[test]
    fn coalesces_consecutive_tool_results() {
        let model = bedrock_model("anthropic.claude-3-7-sonnet");
        let messages = vec![
            user("start"),
            tool_result("tc1", "a", "one", false),
            tool_result("tc2", "b", "two", true),
        ];
        let msgs = build_bedrock_messages(&messages, &model, &StreamOptions::default()).unwrap();

        assert_eq!(
            msgs.len(),
            2,
            "expected user + coalesced tool-result message"
        );
        assert_eq!(
            *msgs[1].role(),
            ConversationRole::User,
            "tool results become a user message"
        );

        let content = msgs[1].content();
        assert_eq!(content.len(), 3, "expected 2 tool results + 1 cache point");
        let tool_results = content
            .iter()
            .filter(|b| matches!(b, BedrockContent::ToolResult(_)))
            .count();
        let cache_points = content
            .iter()
            .filter(|b| matches!(b, BedrockContent::CachePoint(_)))
            .count();
        assert_eq!(tool_results, 2);
        assert_eq!(cache_points, 1);
    }

    #[test]
    fn omits_cache_point_for_non_caching_model() {
        // A model that does not support Bedrock prompt caching coalesces the two
        // results but adds no cache point (2 blocks).
        let model = bedrock_model("meta.llama3-70b");
        let messages = vec![
            user("start"),
            tool_result("tc1", "a", "one", false),
            tool_result("tc2", "b", "two", false),
        ];
        let msgs = build_bedrock_messages(&messages, &model, &StreamOptions::default()).unwrap();
        assert_eq!(msgs.len(), 2);
        assert_eq!(
            msgs[1].content().len(),
            2,
            "no cache point for a non-caching model"
        );
    }
}
