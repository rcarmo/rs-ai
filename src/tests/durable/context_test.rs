#[cfg(test)]
mod tests {
    use crate::durable::context::order_tool_results;
    use crate::types::{ContentBlock, Message, Role};
    use std::collections::HashMap;

    fn assistant(ids: &[&str]) -> Message {
        let mut message = crate::user_message("");
        message.role = Role::Assistant;
        message.timestamp = 123;
        message.content = ids
            .iter()
            .map(|id| ContentBlock::ToolCall {
                id: (*id).into(),
                name: format!("tool-{id}"),
                arguments: HashMap::new(),
                thought_signature: None,
                namespace: None,
            })
            .collect();
        message
    }
    fn tool(id: &str, text: &str) -> Message {
        let mut message = crate::user_message(text);
        message.role = Role::ToolResult;
        message.tool_call_id = Some(id.into());
        message.tool_name = Some(format!("tool-{id}"));
        message.duration_ms = Some(42);
        message
    }

    #[test]
    fn results_follow_call_order_before_interleaved_users_and_drop_orphans() {
        let messages = order_tool_results(vec![
            tool("orphan", "drop"),
            assistant(&["a", "b"]),
            crate::user_message("later"),
            tool("b", "second"),
            tool("a", "first"),
            tool("a", "duplicate"),
            assistant(&[]),
        ]);
        assert_eq!(
            messages
                .iter()
                .map(|message| message.role.clone())
                .collect::<Vec<_>>(),
            [
                Role::Assistant,
                Role::ToolResult,
                Role::ToolResult,
                Role::User,
                Role::Assistant
            ]
        );
        assert_eq!(messages[1].tool_call_id.as_deref(), Some("a"));
        assert_eq!(messages[2].tool_call_id.as_deref(), Some("b"));
        assert_eq!(messages[1].duration_ms, Some(42));
        assert!(
            matches!(messages[1].content.first(), Some(ContentBlock::Text { text, .. }) if text == "first")
        );
    }

    #[test]
    fn missing_results_are_synthesized_and_never_cross_next_assistant_boundary() {
        let messages = order_tool_results(vec![
            assistant(&["a"]),
            assistant(&[]),
            tool("a", "too-late"),
        ]);
        assert_eq!(messages.len(), 3);
        assert!(messages[1].is_error);
        assert_eq!(messages[1].timestamp, 123);
        assert_eq!(messages[1].duration_ms, None);
        assert_eq!(
            messages[1].details.as_ref().unwrap()["reason"],
            "missing_result"
        );
        assert_eq!(messages[1].tool_name.as_deref(), Some("tool-a"));
    }
}
