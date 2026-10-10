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
    fn retained_range_decodes_only_appends_and_rebuilds_on_edits_or_heads() {
        use crate::durable::context::{MessageRange, messages};
        use crate::durable::*;
        use serde_json::json;
        let conversation = ConversationId::new(1).unwrap();
        let mut snapshot = StorageSnapshot::empty();
        let insert = |snapshot: &mut StorageSnapshot, id, value| {
            let id = EntryId::new(id).unwrap();
            snapshot.entries.insert(
                id,
                EntryRecord {
                    id,
                    conversation_id: conversation,
                    kind: "custom".into(),
                    value,
                    by_task_id: None,
                    created_seq: CommitSeq::new(1).unwrap(),
                },
            );
        };
        insert(
            &mut snapshot,
            1,
            json!({"model":[crate::user_message("input"), assistant(&["a","b"])]}),
        );
        let mut range = MessageRange::build(&snapshot, conversation)
            .unwrap()
            .unwrap();
        assert_eq!(range.decoded_entries, 1);
        assert!(range.messages[2].is_error);
        assert!(range.messages[3].is_error);
        assert!(!range.extend(&snapshot, conversation).unwrap());
        for (id, value) in [
            (
                2,
                json!({"model":[tool("b","second"),crate::user_message("interleaved")]}),
            ),
            (
                3,
                json!({"model":[tool("a","first"),tool("a","duplicate"),tool("orphan","drop")]}),
            ),
            (4, json!({"model":[assistant(&["c"])]})),
            (5, json!({"model":[tool("c","third")]})),
        ] {
            insert(&mut snapshot, id, value);
            range.extend(&snapshot, conversation).unwrap();
            assert_eq!(range.decoded_entries, id as usize);
            assert_eq!(
                serde_json::to_value(&range.messages).unwrap(),
                serde_json::to_value(messages(&snapshot, conversation, None).unwrap()).unwrap()
            );
        }
        let historical = messages(&snapshot, conversation, Some(EntryId::new(1).unwrap())).unwrap();
        assert!(historical[2].is_error);
        assert!(historical[3].is_error);
        insert(
            &mut snapshot,
            6,
            json!({"edits":[{"type":"omit","target":1}]}),
        );
        range.extend(&snapshot, conversation).unwrap();
        assert_eq!(range.decoded_entries, 6); // full rebuild
        assert_eq!(
            serde_json::to_value(&range.messages).unwrap(),
            serde_json::to_value(messages(&snapshot, conversation, None).unwrap()).unwrap()
        );
        insert(
            &mut snapshot,
            7,
            json!({"head":7,"model":[crate::user_message("reset")]}),
        );
        range.extend(&snapshot, conversation).unwrap();
        assert_eq!(range.messages.len(), 1);
        assert_eq!(
            serde_json::to_value(&range.messages).unwrap(),
            serde_json::to_value(messages(&snapshot, conversation, None).unwrap()).unwrap()
        );
    }

    #[test]
    fn incremental_ranges_match_full_derivation_over_mixed_transcripts() {
        use crate::durable::context::{MessageRange, messages};
        use crate::durable::*;
        use serde_json::json;
        let conversation = ConversationId::new(1).unwrap();
        let mut snapshot = StorageSnapshot::empty();
        let mut range: Option<MessageRange> = None;
        let mut seed = 13u64;
        let mut call = String::from("initial");
        for id in 1..=150u64 {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let target = if id > 1 { seed % (id - 1) + 1 } else { 1 };
            let value = match seed % 9 {
                0 => json!({"model":[crate::user_message(&format!("user-{id}"))]}),
                1 => {
                    call = format!("call-{id}");
                    json!({"model":[assistant(&[&call]) ]})
                }
                2 => {
                    json!({"model":[tool(&call,"first"),tool(&call,"duplicate"),tool("orphan","drop")]})
                }
                3 => {
                    let mut system = crate::user_message("baseline");
                    system.role = Role::System;
                    json!({"model":[system]})
                }
                4 if id > 1 => {
                    json!({"edits":[{"type":"replace","target":target,"messages":[crate::user_message("replacement"),tool("orphan","drop")]}]})
                }
                5 if id > 1 => json!({"edits":[{"type":"omit","target":target}]}),
                6 => json!({"head":id,"model":[crate::user_message("self-head")]}),
                7 if id > 1 => json!({"head":target,"model":[crate::user_message("prior-head")]}),
                _ => {
                    let mut excluded = assistant(&["excluded"]);
                    excluded.stop_reason = Some(crate::types::StopReason::Aborted);
                    json!({"model":[excluded]})
                }
            };
            let entry_id = EntryId::new(id).unwrap();
            snapshot.entries.insert(
                entry_id,
                EntryRecord {
                    id: entry_id,
                    conversation_id: conversation,
                    kind: "mixed".into(),
                    value,
                    by_task_id: None,
                    created_seq: CommitSeq::new(id).unwrap(),
                },
            );
            if let Some(range) = &mut range {
                range.extend(&snapshot, conversation).unwrap();
            } else {
                range = MessageRange::build(&snapshot, conversation).unwrap();
            }
            let expected = messages(&snapshot, conversation, None).unwrap();
            assert_eq!(
                serde_json::to_value(&range.as_ref().unwrap().messages).unwrap(),
                serde_json::to_value(expected).unwrap(),
                "step {id}"
            );
            // Historical cuts must not contaminate the retained current range.
            let before = serde_json::to_value(&range.as_ref().unwrap().messages).unwrap();
            messages(&snapshot, conversation, Some(EntryId::new(target).unwrap())).unwrap();
            assert_eq!(
                serde_json::to_value(&range.as_ref().unwrap().messages).unwrap(),
                before
            );
        }
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

    #[tokio::test]
    async fn context_view_aligns_detached_entries_and_unrepaired_contributions() {
        use crate::durable::*;
        use serde_json::json;
        let session = DurableSession::open(Box::new(MemoryStorage::new()))
            .await
            .unwrap();
        let conversation = ConversationId::new(1).unwrap();
        let empty = session.context_view(conversation, None).await.unwrap();
        assert!(empty.head.is_none());
        assert!(empty.entries.is_empty());
        assert!(empty.contributions.is_empty());
        assert!(empty.messages.is_empty());
        for (id, kind, value) in [
            (1, "user", json!({"text":"input"})),
            (2, "assistant", json!({"message":assistant(&["a"])})),
            (3, "model_error", json!({"message":"diagnostic"})),
            (
                4,
                "context",
                json!({"head":1,"messages":[crate::user_message("handoff")],"edits":[{"type":"omit","target":1}]}),
            ),
        ] {
            let seq = CommitSeq::new(id).unwrap();
            session
                .commit(CommitBatch {
                    generic_documents: vec![],
                    conversations: vec![],
                    seq,
                    next_id: id + 1,
                    next_seq: id + 1,
                    entries: vec![EntryRecord {
                        id: EntryId::new(id).unwrap(),
                        conversation_id: conversation,
                        kind: kind.into(),
                        value,
                        by_task_id: None,
                        created_seq: seq,
                    }],
                    tasks: vec![],
                    submissions: vec![],
                    documents: vec![],
                })
                .await
                .unwrap();
        }
        let before = session.snapshot().await.unwrap();
        let mut view = session.context_view(conversation, None).await.unwrap();
        assert_eq!(
            view.entries
                .iter()
                .map(|entry| entry.id.get())
                .collect::<Vec<_>>(),
            [4, 1, 2, 3]
        );
        assert_eq!(
            view.contributions.iter().map(Vec::len).collect::<Vec<_>>(),
            [1, 0, 1, 0]
        );
        assert_eq!(view.head.as_ref().unwrap().head.get(), 1);
        assert_eq!(view.head.as_ref().unwrap().entry.id.get(), 4);
        assert_eq!(view.messages.len(), 3);
        assert!(view.messages[2].is_error);
        assert_eq!(view.contributions[2].len(), 1); // synthesized result is not a log contribution
        assert_eq!(
            serde_json::to_value(&view.messages).unwrap(),
            serde_json::to_value(session.message_context(conversation, None).await.unwrap())
                .unwrap()
        );
        let historical = session
            .context_view(conversation, Some(EntryId::new(2).unwrap()))
            .await
            .unwrap();
        assert!(historical.head.is_none());
        assert_eq!(historical.entries.len(), 2);
        assert_eq!(
            historical
                .contributions
                .iter()
                .map(Vec::len)
                .collect::<Vec<_>>(),
            [1, 1]
        );
        assert!(
            session
                .context_view(
                    ConversationId::new(2).unwrap(),
                    Some(EntryId::new(2).unwrap())
                )
                .await
                .is_err()
        );
        view.entries[0].value = json!(null);
        view.head.as_mut().unwrap().entry.value = json!(null);
        view.contributions.clear();
        view.messages.clear();
        assert_eq!(session.snapshot().await.unwrap(), before);
        assert_eq!(
            session
                .context_view(conversation, None)
                .await
                .unwrap()
                .messages
                .len(),
            3
        );
        session.close().await.unwrap();
        assert!(matches!(
            session.context_view(conversation, None).await,
            Err(DurableError::Closed)
        ));
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
            ["new-marker", "replacement", "kept"]
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
            ["old-marker", "kept"]
        );
        snapshot
            .entries
            .get_mut(&EntryId::new(4).unwrap())
            .unwrap()
            .value["edits"] = json!([{"type":"omit","target":1}]);
        assert_eq!(
            text(crate::durable::context::messages(&snapshot, conversation, None).unwrap()),
            ["new-marker", "kept"]
        );
    }

    #[test]
    fn orphan_results_do_not_hide_leading_system_and_edits_still_filter_assistants() {
        use crate::durable::*;
        let conversation = ConversationId::new(1).unwrap();
        let mut system = crate::user_message("baseline");
        system.role = Role::System;
        let mut excluded = assistant(&[]);
        excluded.stop_reason = Some(crate::types::StopReason::Error);
        let mut snapshot = StorageSnapshot::empty();
        for (id, value) in [
            (
                1,
                serde_json::json!({"messages":[crate::user_message("input"), tool("orphan", "ignored"), system]}),
            ),
            (
                2,
                serde_json::json!({"edits":[{"type":"replace","target":1,"messages":[crate::user_message("replacement"), tool("orphan", "ignored"), system, excluded]}]}),
            ),
        ] {
            let id = EntryId::new(id).unwrap();
            snapshot.entries.insert(
                id,
                EntryRecord {
                    id,
                    conversation_id: conversation,
                    kind: "context".into(),
                    value,
                    by_task_id: None,
                    created_seq: CommitSeq::new(1).unwrap(),
                },
            );
        }
        for at in [None, Some(EntryId::new(1).unwrap())] {
            let messages = crate::durable::context::messages(&snapshot, conversation, at).unwrap();
            assert_eq!(messages.len(), 2);
            assert_eq!(messages[0].role, Role::System);
            assert_eq!(messages[1].role, Role::User);
        }
    }

    #[test]
    fn edits_before_retained_range_do_not_override_new_head_contribution() {
        use crate::durable::*;
        let conversation = ConversationId::new(1).unwrap();
        let mut snapshot = StorageSnapshot::empty();
        for (id, value) in [
            (
                1,
                serde_json::json!({"messages":[crate::user_message("old marker")],"head":"self"}),
            ),
            (2, serde_json::json!({"edits":[{"type":"omit","target":1}]})),
            (
                3,
                serde_json::json!({"head":"self","messages":[crate::user_message("new marker")]}),
            ),
        ] {
            let id = EntryId::new(id).unwrap();
            snapshot.entries.insert(
                id,
                EntryRecord {
                    id,
                    conversation_id: conversation,
                    kind: "context".into(),
                    value,
                    by_task_id: None,
                    created_seq: CommitSeq::new(1).unwrap(),
                },
            );
        }
        let messages = crate::durable::context::messages(&snapshot, conversation, None).unwrap();
        assert_eq!(messages.len(), 1);
        assert!(
            matches!(&messages[0].content[0], ContentBlock::Text { text, .. } if text == "new marker")
        );
    }

    #[test]
    fn head_contribution_precedes_retained_assistant_and_honours_omit_edit() {
        use crate::durable::*;
        let conversation = ConversationId::new(1).unwrap();
        let mut snapshot = StorageSnapshot::empty();
        for (id, value) in [
            (1, serde_json::json!({"messages":[assistant(&[])]})),
            (
                2,
                serde_json::json!({"head":1,"messages":[crate::user_message("handoff")]}),
            ),
            (3, serde_json::json!({"edits":[{"type":"omit","target":2}]})),
        ] {
            let id = EntryId::new(id).unwrap();
            snapshot.entries.insert(
                id,
                EntryRecord {
                    id,
                    conversation_id: conversation,
                    kind: "context".into(),
                    value,
                    by_task_id: None,
                    created_seq: CommitSeq::new(1).unwrap(),
                },
            );
        }
        let messages = crate::durable::context::messages(
            &snapshot,
            conversation,
            Some(EntryId::new(2).unwrap()),
        )
        .unwrap();
        assert_eq!(
            messages
                .iter()
                .map(|message| message.role.clone())
                .collect::<Vec<_>>(),
            [Role::User, Role::Assistant]
        );
        let messages = crate::durable::context::messages(&snapshot, conversation, None).unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].role, Role::Assistant);
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
        assert!(
            matches!(&messages[1].content[0], ContentBlock::Text { text, .. } if text == "Tool result unavailable: history ends before this call completed.")
        );
        assert_eq!(
            messages[1].details.as_ref().unwrap()["reason"],
            "missing_result"
        );
        assert_eq!(messages[1].tool_name.as_deref(), Some("tool-a"));
    }
}
