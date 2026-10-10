#[cfg(test)]
mod tests {
    use crate::durable::*;
    use serde_json::json;

    fn batch(seq: u64) -> CommitBatch {
        let seq = CommitSeq::new(seq).unwrap();
        let conversation = ConversationId::new(1).unwrap();
        CommitBatch {
            seq,
            next_id: 5,
            next_seq: seq.get() + 1,
            entries: vec![EntryRecord {
                id: EntryId::new(2).unwrap(),
                conversation_id: conversation,
                kind: "user".into(),
                value: json!({"text":"hello"}),
                by_task_id: None,
                created_seq: seq,
            }],
            tasks: vec![TaskRecord {
                id: TaskId::new(3).unwrap(),
                conversation_id: conversation,
                kind: "generation".into(),
                version: 1,
                owner_task_id: None,
                state: TaskState::Pending,
                input: json!({}),
                checkpoint: json!({}),
                outcome: None,
                abort_requested: false,
                started_at: None,
                ended_at: None,
                updated_seq: seq,
            }],
            submissions: vec![SubmissionRecord {
                id: SubmissionId::new(4).unwrap(),
                conversation_id: conversation,
                request_id: Some("request-1".into()),
                status: "pending".into(),
                entry_id: EntryId::new(2).unwrap(),
                answer_id: None,
                reason: None,
                updated_seq: seq,
            }],
            documents: vec![DocumentRecord {
                conversation_id: conversation,
                kind: "pi.live".into(),
                version: 1,
                value: json!({"task":3}),
                updated_seq: seq,
            }],
        }
    }

    async fn conformance(storage: Box<dyn DurableStorage>) {
        let claim = storage.claim_writer().unwrap();
        assert!(storage.claim_writer().is_err());
        let initial = storage.load(&claim).await.unwrap();
        assert_eq!(initial.next_seq, 1);
        storage.commit(&claim, batch(1)).await.unwrap();
        let state = storage.load(&claim).await.unwrap();
        assert_eq!(state.entries.len(), 1);
        assert_eq!(state.tasks.len(), 1);
        assert_eq!(state.submissions.len(), 1);
        assert_eq!(state.documents.len(), 1);
        assert_eq!(
            state
                .request_ids
                .get(&(ConversationId::new(1).unwrap(), "request-1".into())),
            Some(&SubmissionId::new(4).unwrap())
        );
        let mut detached = state.entries[&EntryId::new(2).unwrap()].clone();
        detached.value["text"] = json!("changed");
        assert_eq!(
            storage.load(&claim).await.unwrap().entries[&EntryId::new(2).unwrap()].value["text"],
            "hello"
        );
        assert!(matches!(
            storage.commit(&claim, batch(1)).await,
            Err(DurableError::Rejected(_))
        ));
    }

    #[tokio::test]
    async fn memory_storage_passes_atomic_detached_conformance() {
        conformance(Box::new(MemoryStorage::new())).await;
    }

    #[tokio::test]
    async fn journal_storage_passes_atomic_detached_conformance() {
        let dir =
            std::env::temp_dir().join(format!("rs-ai-journal-conformance-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        conformance(Box::new(
            JournalStorage::open(dir.join("state.durable")).unwrap(),
        ))
        .await;
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn ordered_snapshot_scans_continue_with_cursor_order_and_detach_values() {
        let mut snapshot = StorageSnapshot::empty();
        snapshot.apply(&batch(1)).unwrap();
        let conversation = ConversationId::new(1).unwrap();
        let entry = snapshot.entries.values().next().unwrap().clone();
        let task = snapshot.tasks.values().next().unwrap().clone();
        let submission = snapshot.submissions.values().next().unwrap().clone();
        for id in [5, 9, 20] {
            snapshot.entries.insert(
                EntryId::new(id).unwrap(),
                EntryRecord {
                    id: EntryId::new(id).unwrap(),
                    ..entry.clone()
                },
            );
            snapshot.tasks.insert(
                TaskId::new(id).unwrap(),
                TaskRecord {
                    id: TaskId::new(id).unwrap(),
                    ..task.clone()
                },
            );
            snapshot.submissions.insert(
                SubmissionId::new(id).unwrap(),
                SubmissionRecord {
                    id: SubmissionId::new(id).unwrap(),
                    ..submission.clone()
                },
            );
        }
        snapshot.entries.insert(
            EntryId::new(30).unwrap(),
            EntryRecord {
                id: EntryId::new(30).unwrap(),
                conversation_id: ConversationId::new(99).unwrap(),
                ..entry
            },
        );
        for order in [ScanOrder::Ascending, ScanOrder::Descending] {
            let mut options = ScanOptions {
                order: Some(order),
                limit: 2,
                ..Default::default()
            };
            let mut ids = Vec::new();
            loop {
                let mut page = snapshot.scan_entries(conversation, &options).unwrap();
                assert!(page.items.len() <= 2);
                ids.extend(page.items.iter().map(|entry| entry.id.get()));
                if let Some(first) = page.items.first_mut() {
                    first.value["text"] = json!("detached");
                }
                let Some(cursor) = page.cursor else {
                    break;
                };
                assert_eq!(cursor.order, Some(order));
                options.cursor = Some(cursor);
                options.order = None;
            }
            assert_eq!(
                ids,
                if order == ScanOrder::Ascending {
                    vec![2, 5, 9, 20]
                } else {
                    vec![20, 9, 5, 2]
                }
            );
        }
        assert_eq!(
            snapshot.entries[&EntryId::new(2).unwrap()].value["text"],
            "hello"
        );
        let defaults = ScanOptions::default();
        assert_eq!(
            snapshot
                .scan_entries(conversation, &defaults)
                .unwrap()
                .items[0]
                .id
                .get(),
            20
        );
        assert_eq!(
            snapshot.scan_tasks(conversation, &defaults).unwrap().items[0]
                .id
                .get(),
            3
        );
        assert_eq!(
            snapshot
                .scan_submissions(conversation, &defaults)
                .unwrap()
                .items[0]
                .id
                .get(),
            4
        );
        for order in [ScanOrder::Ascending, ScanOrder::Descending] {
            let options = ScanOptions {
                order: Some(order),
                limit: 1,
                ..Default::default()
            };
            let tasks = snapshot.scan_tasks(conversation, &options).unwrap();
            let submissions = snapshot.scan_submissions(conversation, &options).unwrap();
            assert_eq!(
                tasks.items[0].id.get(),
                if order == ScanOrder::Ascending { 3 } else { 20 }
            );
            assert_eq!(
                submissions.items[0].id.get(),
                if order == ScanOrder::Ascending { 4 } else { 20 }
            );
            let continuation = ScanOptions {
                cursor: tasks.cursor,
                limit: 10,
                ..Default::default()
            };
            assert_eq!(
                snapshot
                    .scan_tasks(conversation, &continuation)
                    .unwrap()
                    .items
                    .len(),
                3
            );
            let continuation = ScanOptions {
                cursor: submissions.cursor,
                limit: 10,
                ..Default::default()
            };
            assert_eq!(
                snapshot
                    .scan_submissions(conversation, &continuation)
                    .unwrap()
                    .items
                    .len(),
                3
            );
        }
        let legacy = ScanOptions {
            cursor: Some(ScanCursor {
                after: 9,
                order: None,
            }),
            ..Default::default()
        };
        assert_eq!(
            snapshot
                .scan_entries(conversation, &legacy)
                .unwrap()
                .items
                .iter()
                .map(|entry| entry.id.get())
                .collect::<Vec<_>>(),
            [5, 2]
        );
        let conflict = ScanOptions {
            order: Some(ScanOrder::Ascending),
            ..legacy
        };
        assert!(snapshot.scan_entries(conversation, &conflict).is_err());
        let invalid = ScanOptions {
            cursor: Some(ScanCursor {
                after: MAX_ID + 1,
                order: None,
            }),
            ..Default::default()
        };
        assert!(snapshot.scan_tasks(conversation, &invalid).is_err());
        let zero_limit = ScanOptions {
            limit: 0,
            ..Default::default()
        };
        assert!(
            snapshot
                .scan_submissions(conversation, &zero_limit)
                .is_err()
        );
        assert!(
            serde_json::from_value::<ScanCursor>(json!({"after":5,"order":"sideways"})).is_err()
        );
    }

    #[test]
    fn rejected_commit_leaves_snapshot_completely_unchanged() {
        let mut snapshot = StorageSnapshot::empty();
        snapshot.apply(&batch(1)).unwrap();
        let before = snapshot.clone();
        let mut rejected = batch(2);
        rejected.entries[0].id = EntryId::new(5).unwrap();
        rejected.next_id = 6;
        rejected.documents[0].kind = "invalid-last-record".into();
        assert!(snapshot.apply(&rejected).is_err());
        assert_eq!(snapshot, before);
    }

    #[test]
    fn ids_references_high_water_and_sizes_fail_closed() {
        assert!(ConversationId::new(0).is_err());
        assert!(ConversationId::new((i64::MAX as u64) + 1).is_err());
        assert!(serde_json::from_str::<EntryId>(&format!("{}", (i64::MAX as u64) + 1)).is_err());
        let mut collision = batch(1);
        collision.tasks[0].id = TaskId::new(2).unwrap();
        assert!(matches!(
            crate::durable::storage::validate_batch(&StorageSnapshot::empty(), &collision),
            Err(DurableError::Rejected(_))
        ));
        let mut bad_high_water = batch(1);
        bad_high_water.next_id = 4;
        assert!(matches!(
            crate::durable::storage::validate_batch(&StorageSnapshot::empty(), &bad_high_water),
            Err(DurableError::Rejected(_))
        ));
        let mut bad_reference = batch(1);
        bad_reference.submissions[0].entry_id = EntryId::new(99).unwrap();
        assert!(matches!(
            crate::durable::storage::validate_batch(&StorageSnapshot::empty(), &bad_reference),
            Err(DurableError::Rejected(_))
        ));
        let mut oversized = batch(1);
        oversized.documents[0].value = json!("x".repeat(MAX_DOCUMENT_BYTES + 1));
        assert!(matches!(
            crate::durable::storage::validate_batch(&StorageSnapshot::empty(), &oversized),
            Err(DurableError::TooLarge { .. })
        ));
    }

    #[test]
    fn task_owner_cycles_and_terminal_mutation_fail_closed() {
        let mut cycle = batch(1);
        cycle.tasks = vec![
            TaskRecord {
                id: TaskId::new(3).unwrap(),
                owner_task_id: Some(TaskId::new(4).unwrap()),
                ..cycle.tasks[0].clone()
            },
            TaskRecord {
                id: TaskId::new(4).unwrap(),
                owner_task_id: Some(TaskId::new(3).unwrap()),
                ..cycle.tasks[0].clone()
            },
        ];
        cycle.submissions.clear();
        cycle.next_id = 5;
        assert!(matches!(
            crate::durable::storage::validate_batch(&StorageSnapshot::empty(), &cycle),
            Err(DurableError::Rejected(_))
        ));

        let mut initial = batch(1);
        initial.tasks[0].state = TaskState::Succeeded;
        initial.tasks[0].outcome = Some(json!({"ok": true}));
        let mut state = StorageSnapshot::empty();
        state.apply(&initial).unwrap();
        let seq = CommitSeq::new(2).unwrap();
        let terminal_mutation = CommitBatch {
            seq,
            next_id: 5,
            next_seq: 3,
            entries: vec![],
            tasks: vec![TaskRecord {
                updated_seq: seq,
                checkpoint: json!({"changed": true}),
                ..initial.tasks[0].clone()
            }],
            submissions: vec![],
            documents: vec![],
        };
        assert!(matches!(
            crate::durable::storage::validate_batch(&state, &terminal_mutation),
            Err(DurableError::Rejected(_))
        ));
    }

    #[test]
    fn submission_state_and_kind_version_schema_fail_closed() {
        let mut unknown = batch(1);
        unknown.submissions[0].status = "mystery".into();
        assert!(matches!(
            crate::durable::storage::validate_batch(&StorageSnapshot::empty(), &unknown),
            Err(DurableError::Rejected(_))
        ));

        let mut done_without_answer = batch(1);
        done_without_answer.submissions[0].status = "done".into();
        assert!(matches!(
            crate::durable::storage::validate_batch(
                &StorageSnapshot::empty(),
                &done_without_answer
            ),
            Err(DurableError::Rejected(_))
        ));

        let mut bad_kind = batch(1);
        bad_kind.tasks[0].kind = "other".into();
        assert!(matches!(
            crate::durable::storage::validate_batch(&StorageSnapshot::empty(), &bad_kind),
            Err(DurableError::Rejected(_))
        ));

        let mut bad_version = batch(1);
        bad_version.documents[0].version = 0;
        assert!(matches!(
            crate::durable::storage::validate_batch(&StorageSnapshot::empty(), &bad_version),
            Err(DurableError::Rejected(_))
        ));
    }
}
