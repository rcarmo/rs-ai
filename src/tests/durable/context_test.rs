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
    fn context_excludes_aborted_error_deferred_assistants_and_their_orphan_results() {
        use crate::durable::*;
        let mut snapshot = StorageSnapshot::empty();
        let conversation = ConversationId::new(1).unwrap();
        for (index, stop) in [
            crate::types::StopReason::Aborted,
            crate::types::StopReason::Error,
            crate::types::StopReason::Deferred,
            crate::types::StopReason::ToolUse,
        ]
        .into_iter()
        .enumerate()
        {
            let id = EntryId::new(index as u64 * 2 + 1).unwrap();
            let mut message = assistant(&[&format!("call-{index}")]);
            message.stop_reason = Some(stop);
            snapshot.entries.insert(
                id,
                EntryRecord {
                    id,
                    conversation_id: conversation,
                    kind: "assistant".into(),
                    value: serde_json::json!({"message":message}),
                    by_task_id: None,
                    created_seq: CommitSeq::new(1).unwrap(),
                },
            );
            let id = EntryId::new(index as u64 * 2 + 2).unwrap();
            snapshot.entries.insert(id, EntryRecord { id, conversation_id: conversation, kind: "tool_result".into(), value: serde_json::json!({"tool_call_id":format!("call-{index}"),"tool_name":"tool","result":"ok"}), by_task_id: None, created_seq: CommitSeq::new(1).unwrap() });
        }
        let messages = crate::durable::context::messages(&snapshot, conversation, None).unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(
            messages[0].stop_reason,
            Some(crate::types::StopReason::ToolUse)
        );
        assert_eq!(messages[1].tool_call_id.as_deref(), Some("call-3"));
        assert!(
            crate::durable::context::messages(
                &snapshot,
                ConversationId::new(2).unwrap(),
                Some(EntryId::new(1).unwrap())
            )
            .is_err()
        );
    }

    #[test]
    fn malformed_native_context_fails_without_mutating_records() {
        use crate::durable::*;
        let conversation = ConversationId::new(1).unwrap();
        for (kind, value) in [
            ("user", serde_json::json!({})),
            ("assistant", serde_json::json!({"message":{"role":"user"}})),
            ("tool_result", serde_json::json!({"result":true})),
        ] {
            let mut snapshot = StorageSnapshot::empty();
            let id = EntryId::new(1).unwrap();
            snapshot.entries.insert(
                id,
                EntryRecord {
                    id,
                    conversation_id: conversation,
                    kind: kind.into(),
                    value,
                    by_task_id: None,
                    created_seq: CommitSeq::new(1).unwrap(),
                },
            );
            let before = snapshot.clone();
            assert!(matches!(
                crate::durable::context::messages(&snapshot, conversation, None),
                Err(DurableError::Corrupt(_))
            ));
            assert_eq!(snapshot, before);
        }
    }

    #[test]
    fn older_head_markers_keep_edits_but_only_newest_marker_contributes() {
        use crate::durable::*;
        use serde_json::json;
        let conversation = ConversationId::new(1).unwrap();
        let mut snapshot = StorageSnapshot::empty();
        for (id, kind, value) in [
            (1, "user", json!({"text":"target"})),
            (2, "user", json!({"text":"kept"})),
            (
                3,
                "context",
                json!({"head":2,"messages":[crate::user_message("old-marker")],"edits":[{"type":"replace","target":1,"messages":[crate::user_message("replacement")]}]}),
            ),
            (
                4,
                "context",
                json!({"head":1,"messages":[crate::user_message("new-marker")]}),
            ),
        ] {
            let id = EntryId::new(id).unwrap();
            snapshot.entries.insert(
                id,
                EntryRecord {
                    id,
                    conversation_id: conversation,
                    kind: kind.into(),
                    value,
                    by_task_id: None,
                    created_seq: CommitSeq::new(1).unwrap(),
                },
            );
        }
        let text = |messages: Vec<Message>| {
            messages
                .into_iter()
                .map(|message| match message.content.into_iter().next() {
                    Some(ContentBlock::Text { text, .. }) => text,
                    _ => String::new(),
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(
            text(crate::durable::context::messages(&snapshot, conversation, None).unwrap()),
            ["replacement", "kept", "new-marker"]
        );
        assert_eq!(
            text(
                crate::durable::context::messages(
                    &snapshot,
                    conversation,
                    Some(EntryId::new(3).unwrap())
                )
                .unwrap()
            ),
            ["kept", "old-marker"]
        );
        snapshot
            .entries
            .get_mut(&EntryId::new(4).unwrap())
            .unwrap()
            .value["edits"] = json!([{"type":"omit","target":1}]);
        assert_eq!(
            text(crate::durable::context::messages(&snapshot, conversation, None).unwrap()),
            ["kept", "new-marker"]
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
