#[cfg(test)]
mod tests {
    use crate::durable::*;
    use crate::types::{ContentBlock, Role};
    use serde_json::json;
    use std::sync::{Arc, Mutex};

    fn text(messages: &[crate::types::Message]) -> Vec<String> {
        messages
            .iter()
            .map(|message| match message.content.first() {
                Some(ContentBlock::Text { text, .. }) => text.clone(),
                _ => String::new(),
            })
            .collect()
    }

    #[tokio::test]
    async fn generic_entries_preserve_data_contribute_messages_and_resolve_self_heads() {
        let session = DurableSession::open(Box::new(MemoryStorage::new()))
            .await
            .unwrap();
        let conversation = ConversationId::new(1).unwrap();
        let mut watch = session.watch().await.unwrap();
        assert!(matches!(
            watch.next().await,
            Some(DurableEvent::Snapshot(_))
        ));
        let mut draft = EntryDraft::new("custom.note");
        draft.data = Some(json!({"nested":["detached"]}));
        let mut note = session.append_entry(conversation, draft).await.unwrap();
        assert_eq!(note.kind, "custom.note");
        assert!(note.by_task_id.is_none());
        assert!(note.value.get("model").is_none());
        assert!(
            matches!(watch.next().await, Some(DurableEvent::Commit(batch)) if batch.entries[0] == note)
        );
        let mut draft = EntryDraft::new("pi.user");
        draft.model = Some(vec![crate::user_message("old")]);
        let input = session.append_entry(conversation, draft).await.unwrap();
        let mut draft = EntryDraft::new("summary");
        draft.head = Some(ContextHead::SelfEntry(SelfHead::SelfEntry));
        draft.model = Some(vec![crate::user_message("handoff")]);
        draft.data = Some(json!(null));
        let reset = session.append_entry(conversation, draft).await.unwrap();
        assert_eq!(reset.value["head"], reset.id.get());
        assert!(reset.value.as_object().unwrap().contains_key("data"));
        assert_eq!(reset.value["data"], json!(null));
        let mut draft = EntryDraft::new("custom.tail");
        draft.model = Some(vec![crate::user_message("tail")]);
        let tail = session.append_entry(conversation, draft).await.unwrap();
        let mut draft = EntryDraft::new("edit");
        draft.edits = vec![ContextEdit::Replace {
            target: tail.id,
            messages: vec![crate::user_message("replacement")],
        }];
        session.append_entry(conversation, draft).await.unwrap();
        let view = session.context_view(conversation, None).await.unwrap();
        assert_eq!(view.head.as_ref().unwrap().head, reset.id);
        assert_eq!(view.head.as_ref().unwrap().entry.kind, "summary");
        assert_eq!(text(&view.messages), ["handoff", "replacement"]);
        assert_eq!(view.entries.len(), 3);
        assert_eq!(
            view.contributions.iter().map(Vec::len).collect::<Vec<_>>(),
            [1, 1, 0]
        );
        let historic = session
            .context_view(conversation, Some(input.id))
            .await
            .unwrap();
        assert_eq!(historic.entries.len(), 2);
        assert!(historic.contributions[0].is_empty());
        assert_eq!(text(&historic.messages), ["old"]);
        note.value["data"]["nested"][0] = json!("mutated");
        let records = session
            .entries(
                conversation,
                EntryQuery {
                    kind: Some("custom.note".into()),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert_eq!(records.items[0].value["data"]["nested"][0], "detached");
        let state = session.snapshot().await.unwrap();
        assert!(state.tasks.is_empty());
        assert!(state.submissions.is_empty());
        session.close().await.unwrap();
        assert!(matches!(
            session
                .append_entry(conversation, EntryDraft::new("note"))
                .await,
            Err(DurableError::Closed)
        ));
    }

    #[tokio::test]
    async fn concurrent_appends_assign_unique_ids_on_session_line() {
        let session = Arc::new(
            DurableSession::open(Box::new(MemoryStorage::new()))
                .await
                .unwrap(),
        );
        let mut jobs = Vec::new();
        for index in 0..32 {
            let session = session.clone();
            jobs.push(tokio::spawn(async move {
                let mut draft = EntryDraft::new("note");
                draft.data = Some(json!(index));
                session
                    .append_entry(ConversationId::new(1).unwrap(), draft)
                    .await
                    .unwrap()
            }));
        }
        let mut entries = Vec::new();
        for job in jobs {
            entries.push(job.await.unwrap());
        }
        entries.sort_by_key(|entry| entry.id);
        for (index, entry) in entries.iter().enumerate() {
            assert_eq!(entry.id.get(), index as u64 + 1);
            assert_eq!(entry.created_seq.get(), entry.id.get());
        }
        let snapshot = session.snapshot().await.unwrap();
        assert_eq!(snapshot.next_id, 33);
        assert_eq!(snapshot.next_seq, 33);
        session.close().await.unwrap();
    }

    #[tokio::test]
    async fn generic_validation_rejects_before_identity_allocation_or_publication() {
        let session = DurableSession::open(Box::new(MemoryStorage::new()))
            .await
            .unwrap();
        let conversation = ConversationId::new(1).unwrap();
        let foreign = session
            .append_entry(ConversationId::new(2).unwrap(), EntryDraft::new("note"))
            .await
            .unwrap();
        let mut watch = session.watch().await.unwrap();
        watch.next().await.unwrap();
        let before = session.snapshot().await.unwrap();
        let mut invalid = vec![
            EntryDraft::new(""),
            EntryDraft::new("x".repeat(65)),
            EntryDraft::new("user"),
            EntryDraft::new("context"),
        ];
        let mut draft = EntryDraft::new("future");
        draft.head = Some(ContextHead::Entry(
            EntryId::new(before.next_id + 10).unwrap(),
        ));
        invalid.push(draft);
        let mut draft = EntryDraft::new("foreign");
        draft.edits = vec![ContextEdit::Omit { target: foreign.id }];
        invalid.push(draft);
        let mut draft = EntryDraft::new("self-edit");
        draft.edits = vec![ContextEdit::Omit {
            target: EntryId::new(before.next_id).unwrap(),
        }];
        invalid.push(draft);
        let mut draft = EntryDraft::new("too-many");
        draft.model = Some(vec![crate::user_message(""); 4097]);
        invalid.push(draft);
        let mut draft = EntryDraft::new("deep");
        let mut data = json!(true);
        for _ in 0..129 {
            data = json!([data]);
        }
        draft.data = Some(data);
        invalid.push(draft);
        for draft in invalid {
            assert!(session.append_entry(conversation, draft).await.is_err());
            assert_eq!(session.snapshot().await.unwrap(), before);
        }
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(10), watch.next())
                .await
                .is_err()
        );
        session.close().await.unwrap();
    }

    #[test]
    fn draft_json_distinguishes_absent_data_from_explicit_null() {
        let absent: EntryDraft = serde_json::from_value(json!({"kind":"note"})).unwrap();
        let null: EntryDraft = serde_json::from_value(json!({"kind":"note","data":null})).unwrap();
        assert!(absent.data.is_none());
        assert_eq!(null.data, Some(json!(null)));
        assert_eq!(
            serde_json::to_value(absent).unwrap(),
            json!({"kind":"note"})
        );
        assert_eq!(
            serde_json::to_value(null).unwrap(),
            json!({"kind":"note","data":null})
        );
        assert!(serde_json::from_value::<EntryDraft>(json!({"kind":"note","id":1})).is_err());
    }

    #[test]
    fn raw_generic_payload_and_reference_validation_remains_fail_closed() {
        let snapshot = StorageSnapshot::empty();
        for value in [
            json!({"unknown":true}),
            json!({"head":"self"}),
            json!({"model":[{"role":"invalid"}]}),
            json!({"edits":[{"type":"replace","target":1}]}),
        ] {
            let batch = CommitBatch {
                seq: CommitSeq::new(1).unwrap(),
                next_id: 2,
                next_seq: 2,
                entries: vec![EntryRecord {
                    id: EntryId::new(1).unwrap(),
                    conversation_id: ConversationId::new(1).unwrap(),
                    kind: "note".into(),
                    value,
                    by_task_id: None,
                    created_seq: CommitSeq::new(1).unwrap(),
                }],
                tasks: vec![],
                submissions: vec![],
                documents: vec![],
            };
            assert!(crate::durable::storage::validate_batch(&snapshot, &batch).is_err());
        }
    }

    #[tokio::test]
    async fn journal_reopen_retains_generic_context_without_dispatch() {
        struct Capture(Arc<Mutex<Vec<ModelIntent>>>);
        impl DurableModelRunner for Capture {
            fn run<'a>(&'a self, intent: ModelIntent) -> crate::durable::model::ModelFuture<'a> {
                self.0.lock().unwrap().push(intent);
                Box::pin(async {
                    ModelRun::one(ModelTerminal::Answer {
                        content: vec![],
                        text: "done".into(),
                        usage: DurableUsage::default(),
                        response_id: None,
                        stop_reason: "stop".into(),
                    })
                })
            }
        }
        let root =
            std::env::temp_dir().join(format!("rs-ai-generic-entries-{}", crate::utils::uuidv7()));
        std::fs::create_dir(&root).unwrap();
        let path = root.join("journal");
        let seen = Arc::new(Mutex::new(Vec::new()));
        crate::registry::register_builtin_models();
        let model =
            PinnedModel::from_model(&crate::registry::get_model("openai", "gpt-4o-mini").unwrap())
                .unwrap();
        let harness = DurableHarness::open(
            Box::new(JournalStorage::open(&path).unwrap()),
            Arc::new(Capture(seen.clone())),
            model.clone(),
            PinnedOptions::default(),
        )
        .await
        .unwrap();
        let mut draft = EntryDraft::new("pi.system");
        let mut system = crate::user_message("baseline");
        system.role = Role::System;
        draft.model = Some(vec![system]);
        let entry = harness.append_entry(draft).await.unwrap();
        harness.close().await.unwrap();
        let reopened = DurableHarness::open(
            Box::new(JournalStorage::open(&path).unwrap()),
            Arc::new(Capture(seen.clone())),
            model,
            PinnedOptions::default(),
        )
        .await
        .unwrap();
        assert!(seen.lock().unwrap().is_empty());
        assert_eq!(
            reopened
                .context_view(ContextOptions::default())
                .await
                .unwrap()
                .entries[0],
            entry
        );
        let handle = reopened
            .submit(SubmitRequest {
                request_id: "after-entry".into(),
                content: "question".into(),
            })
            .await
            .unwrap();
        reopened.wait(handle).await.unwrap();
        {
            let intents = seen.lock().unwrap();
            let messages = intents[0].native_messages.as_ref().unwrap();
            assert_eq!(messages[0].role, Role::System);
            assert_eq!(text(messages), ["baseline", "question"]);
        }
        reopened.close().await.unwrap();
        std::fs::remove_dir_all(root).unwrap();
    }
}
