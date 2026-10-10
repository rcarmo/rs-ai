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
        let mut lookup = session.entry(conversation, note.id).await.unwrap().unwrap();
        assert_eq!(lookup, note);
        lookup.value = json!(null);
        assert!(
            session
                .entry(ConversationId::new(2).unwrap(), note.id)
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            session
                .entry(conversation, EntryId::new(99999).unwrap())
                .await
                .unwrap()
                .is_none()
        );
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
            session.entry(conversation, note.id).await,
            Err(DurableError::Closed)
        ));
        assert!(matches!(
            session
                .append_entry(conversation, EntryDraft::new("note"))
                .await,
            Err(DurableError::Closed)
        ));
    }

    #[tokio::test]
    async fn ordered_batch_heads_and_edits_can_target_earlier_same_batch_entries() {
        for journal in [false, true] {
            let root =
                std::env::temp_dir().join(format!("rs-ai-entry-batch-{}", crate::utils::uuidv7()));
            std::fs::create_dir(&root).unwrap();
            let storage: Box<dyn DurableStorage> = if journal {
                Box::new(JournalStorage::open(root.join("journal")).unwrap())
            } else {
                Box::new(MemoryStorage::new())
            };
            let session = DurableSession::open(storage).await.unwrap();
            let conversation = ConversationId::new(1).unwrap();
            let seq = CommitSeq::new(1).unwrap();
            let batch = CommitBatch {
                seq,
                next_id: 4,
                next_seq: 2,
                entries: vec![
                    EntryRecord {
                        id: EntryId::new(1).unwrap(),
                        conversation_id: conversation,
                        kind: "note".into(),
                        value: json!({"model":[crate::user_message("original")]}),
                        by_task_id: None,
                        created_seq: seq,
                    },
                    EntryRecord {
                        id: EntryId::new(2).unwrap(),
                        conversation_id: conversation,
                        kind: "summary".into(),
                        value: json!({"head":1,"model":[crate::user_message("summary")],"edits":[{"type":"replace","target":1,"messages":[crate::user_message("replacement")]}]}),
                        by_task_id: None,
                        created_seq: seq,
                    },
                    EntryRecord {
                        id: EntryId::new(3).unwrap(),
                        conversation_id: conversation,
                        kind: "context".into(),
                        value: json!({"edits":[{"type":"omit","target":2}]}),
                        by_task_id: None,
                        created_seq: seq,
                    },
                ],
                tasks: vec![],
                submissions: vec![],
                documents: vec![],
            };
            let before = session.snapshot().await.unwrap();
            let mut reordered = batch.clone();
            reordered.entries.swap(0, 1);
            assert!(session.commit(reordered).await.is_err());
            assert_eq!(session.snapshot().await.unwrap(), before);
            let mut foreign = batch.clone();
            foreign.entries[0].conversation_id = ConversationId::new(2).unwrap();
            assert!(session.commit(foreign).await.is_err());
            assert_eq!(session.snapshot().await.unwrap(), before);
            session.commit(batch).await.unwrap();
            let view = session.context_view(conversation, None).await.unwrap();
            assert_eq!(text(&view.messages), ["replacement"]);
            assert_eq!(
                view.entries
                    .iter()
                    .map(|entry| entry.id.get())
                    .collect::<Vec<_>>(),
                [2, 1, 3]
            );
            session.close().await.unwrap();
            if journal {
                let reopened = DurableSession::open(Box::new(
                    JournalStorage::open(root.join("journal")).unwrap(),
                ))
                .await
                .unwrap();
                assert_eq!(
                    text(
                        &reopened
                            .context_view(conversation, None)
                            .await
                            .unwrap()
                            .messages
                    ),
                    ["replacement"]
                );
                reopened.close().await.unwrap();
            }
            std::fs::remove_dir_all(root).unwrap();
        }
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

    #[tokio::test]
    async fn typed_entry_tokens_match_kind_and_decode_data_without_registration() {
        #[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize)]
        struct Note {
            text: String,
            count: u64,
        }
        let definition = EntryDefinition::<Note>::new("custom.note").unwrap();
        assert_eq!(definition.clone().kind(), "custom.note"); // Note need not be Clone
        let same_kind = EntryDefinition::<Note>::new("custom.note").unwrap();
        assert_eq!(definition.kind(), "custom.note");
        assert!(!definition.matches(None));
        assert!(EntryDefinition::<Note>::new("").is_err());
        assert!(EntryDefinition::<Note>::new("user").is_err());
        let session = DurableSession::open(Box::new(MemoryStorage::new()))
            .await
            .unwrap();
        let conversation = ConversationId::new(1).unwrap();
        let draft = definition
            .draft(Note {
                text: "typed".into(),
                count: 3,
            })
            .unwrap();
        let record = session.append_entry(conversation, draft).await.unwrap();
        assert!(same_kind.matches(Some(&record)));
        assert_eq!(
            same_kind.decode(Some(&record)).unwrap(),
            Some(Note {
                text: "typed".into(),
                count: 3
            })
        );
        assert!(same_kind.decode(None).unwrap().is_none());
        let different = EntryDefinition::<Note>::new("other").unwrap();
        assert!(different.decode(Some(&record)).unwrap().is_none());
        let malformed = session
            .append_entry(conversation, EntryDraft::new("custom.note"))
            .await
            .unwrap();
        assert!(same_kind.matches(Some(&malformed))); // kind guard does not validate payload
        assert!(same_kind.decode(Some(&malformed)).is_err());
        let mut wrong_shape = EntryDraft::new("custom.note");
        wrong_shape.data = Some(json!({"text":42,"count":3}));
        let wrong_shape = session
            .append_entry(conversation, wrong_shape)
            .await
            .unwrap();
        assert!(same_kind.decode(Some(&wrong_shape)).is_err());
        let null_kind = EntryDefinition::<Option<Note>>::new("nullable").unwrap();
        let null = session
            .append_entry(conversation, null_kind.draft(None).unwrap())
            .await
            .unwrap();
        assert_eq!(null_kind.decode(Some(&null)).unwrap(), Some(None));
        let state = session.snapshot().await.unwrap();
        assert!(state.tasks.is_empty());
        assert!(state.submissions.is_empty());
        assert!(state.documents.is_empty());
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
        assert_eq!(reopened.entry(entry.id).await.unwrap().unwrap(), entry);
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
