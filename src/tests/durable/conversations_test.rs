#[cfg(test)]
mod tests {
    use crate::durable::*;
    use serde_json::json;
    use std::sync::Arc;

    fn task(id: u64, conversation: u64, seq: u64) -> TaskRecord {
        TaskRecord {
            id: TaskId::new(id).unwrap(),
            conversation_id: ConversationId::new(conversation).unwrap(),
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
            updated_seq: CommitSeq::new(seq).unwrap(),
        }
    }
    #[tokio::test]
    async fn create_query_owner_filters_and_transaction_reads_are_detached_and_ordered() {
        let session = DurableSession::open(Box::new(MemoryStorage::new()))
            .await
            .unwrap();
        let root = ConversationId::new(1).unwrap();
        assert_eq!(
            session
                .conversation(root)
                .await
                .unwrap()
                .unwrap()
                .created_seq,
            None
        );
        let ownerless = session
            .create_conversation(ConversationOwnership::Ownerless)
            .await
            .unwrap();
        assert_eq!(ownerless.id.get(), 2);
        let seq = CommitSeq::new(2).unwrap();
        session
            .commit(CommitBatch {
                conversations: vec![],
                seq,
                next_id: 4,
                next_seq: 3,
                tasks: vec![task(3, 1, 2)],
                entries: vec![],
                submissions: vec![],
                documents: vec![],
            })
            .await
            .unwrap();
        let owned = session
            .create_conversation(ConversationOwnership::Task {
                task_id: TaskId::new(3).unwrap(),
            })
            .await
            .unwrap();
        assert_eq!(owned.owner.as_ref().unwrap().conversation_id, root);
        let mut page = session
            .conversations(ConversationQuery {
                scan: ScanOptions {
                    limit: 1,
                    order: Some(ScanOrder::Descending),
                    ..Default::default()
                },
                ..Default::default()
            })
            .await
            .unwrap();
        assert_eq!(page.items[0].id, owned.id);
        page.items[0].owner = None;
        let rest = session
            .conversations(ConversationQuery {
                scan: ScanOptions {
                    cursor: page.cursor,
                    ..Default::default()
                },
                ..Default::default()
            })
            .await
            .unwrap();
        assert_eq!(
            rest.items
                .iter()
                .map(|record| record.id.get())
                .collect::<Vec<_>>(),
            [2, 1]
        );
        let filtered = session
            .conversations(ConversationQuery {
                owner_task_id: Some(TaskId::new(3).unwrap()),
                owner_conversation_id: Some(root),
                ..Default::default()
            })
            .await
            .unwrap();
        assert_eq!(filtered.items.as_slice(), std::slice::from_ref(&owned));
        assert!(
            session
                .conversations(ConversationQuery {
                    owner_conversation_id: Some(ownerless.id),
                    ..Default::default()
                })
                .await
                .unwrap()
                .items
                .is_empty()
        );
        let owned_id = owned.id;
        session
            .transact_entries(move |tx| {
                assert_eq!(
                    tx.conversation(owned_id)?
                        .unwrap()
                        .owner
                        .unwrap()
                        .task_id
                        .get(),
                    3
                );
                assert_eq!(
                    tx.conversations(&ConversationQuery::default())?.items.len(),
                    3
                );
                let created = tx.create_conversation(ConversationOwnership::Ownerless)?;
                assert!(tx.conversation(created.id).is_err());
                tx.append_entry(created.id, EntryDraft::new("note"))?;
                Ok(())
            })
            .await
            .unwrap();
        assert_eq!(
            session.conversation(owned.id).await.unwrap().unwrap(),
            owned
        );
        assert!(
            session
                .conversation(ConversationId::new(999).unwrap())
                .await
                .unwrap()
                .is_none()
        );
        session.close().await.unwrap();
        assert!(matches!(
            session.conversations(ConversationQuery::default()).await,
            Err(DurableError::Closed)
        ));
    }
    #[tokio::test]
    async fn inferred_legacy_scopes_reopen_without_rewriting_records_or_encoding() {
        let root = std::env::temp_dir().join(format!(
            "rs-ai-conversation-legacy-{}",
            crate::utils::uuidv7()
        ));
        std::fs::create_dir(&root).unwrap();
        let path = root.join("journal");
        let session = DurableSession::open(Box::new(JournalStorage::open(&path).unwrap()))
            .await
            .unwrap();
        let seq = CommitSeq::new(1).unwrap();
        let batch = CommitBatch {
            conversations: vec![],
            seq,
            next_id: 2,
            next_seq: 2,
            entries: vec![EntryRecord {
                id: EntryId::new(1).unwrap(),
                conversation_id: ConversationId::new(2).unwrap(),
                kind: "user".into(),
                value: json!({"text":"legacy"}),
                by_task_id: None,
                created_seq: seq,
            }],
            tasks: vec![],
            submissions: vec![],
            documents: vec![],
        };
        let encoded = serde_json::to_value(&batch).unwrap();
        assert!(encoded.get("conversations").is_none());
        assert_eq!(
            serde_json::from_value::<CommitBatch>(encoded).unwrap(),
            batch
        );
        session.commit(batch).await.unwrap();
        session.close().await.unwrap();
        let reopened = DurableSession::open(Box::new(JournalStorage::open(&path).unwrap()))
            .await
            .unwrap();
        let inferred = reopened
            .conversation(ConversationId::new(2).unwrap())
            .await
            .unwrap()
            .unwrap();
        assert!(inferred.owner.is_none());
        assert!(inferred.created_seq.is_none());
        let created = reopened
            .create_conversation(ConversationOwnership::Ownerless)
            .await
            .unwrap();
        assert_eq!(created.id.get(), 3);
        assert_eq!(
            reopened
                .entry(ConversationId::new(2).unwrap(), EntryId::new(1).unwrap())
                .await
                .unwrap()
                .unwrap()
                .value["text"],
            "legacy"
        );
        reopened.close().await.unwrap();
        let reopened = DurableSession::open(Box::new(JournalStorage::open(&path).unwrap()))
            .await
            .unwrap();
        assert_eq!(
            reopened.conversation(created.id).await.unwrap().unwrap(),
            created
        );
        reopened.close().await.unwrap();
        std::fs::remove_dir_all(root).unwrap();
    }
    #[tokio::test]
    async fn invalid_owners_duplicate_records_and_cycles_reject_without_publication() {
        let session = DurableSession::open(Box::new(MemoryStorage::new()))
            .await
            .unwrap();
        let before = session.snapshot().await.unwrap();
        let mut watch = session.watch().await.unwrap();
        watch.next().await.unwrap();
        assert!(
            session
                .create_conversation(ConversationOwnership::Task {
                    task_id: TaskId::new(999).unwrap()
                })
                .await
                .is_err()
        );
        assert_eq!(session.snapshot().await.unwrap(), before);
        let seq = CommitSeq::new(1).unwrap();
        let record = ConversationRecord {
            id: ConversationId::new(2).unwrap(),
            parent: None,
            owner: None,
            created_seq: Some(seq),
        };
        for records in [
            vec![record.clone(), record.clone()],
            vec![ConversationRecord {
                id: ConversationId::new(1).unwrap(),
                ..record.clone()
            }],
            vec![ConversationRecord {
                owner: Some(ConversationOwner {
                    conversation_id: ConversationId::new(1).unwrap(),
                    task_id: TaskId::new(999).unwrap(),
                }),
                ..record.clone()
            }],
        ] {
            assert!(
                session
                    .commit(CommitBatch {
                        conversations: records,
                        seq,
                        next_id: 3,
                        next_seq: 2,
                        entries: vec![],
                        tasks: vec![],
                        submissions: vec![],
                        documents: vec![]
                    })
                    .await
                    .is_err()
            );
            assert_eq!(session.snapshot().await.unwrap(), before);
        }
        let cyclic = CommitBatch {
            conversations: vec![
                ConversationRecord {
                    id: ConversationId::new(2).unwrap(),
                    parent: None,
                    owner: Some(ConversationOwner {
                        conversation_id: ConversationId::new(3).unwrap(),
                        task_id: TaskId::new(5).unwrap(),
                    }),
                    created_seq: Some(seq),
                },
                ConversationRecord {
                    id: ConversationId::new(3).unwrap(),
                    parent: None,
                    owner: Some(ConversationOwner {
                        conversation_id: ConversationId::new(2).unwrap(),
                        task_id: TaskId::new(4).unwrap(),
                    }),
                    created_seq: Some(seq),
                },
            ],
            seq,
            next_id: 6,
            next_seq: 2,
            entries: vec![],
            tasks: vec![task(4, 2, 1), task(5, 3, 1)],
            submissions: vec![],
            documents: vec![],
        };
        assert!(session.commit(cyclic).await.is_err());
        assert_eq!(session.snapshot().await.unwrap(), before);
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(10), watch.next())
                .await
                .is_err()
        );
        session
            .create_conversation(ConversationOwnership::Ownerless)
            .await
            .unwrap();
        assert!(
            matches!(watch.next().await,Some(DurableEvent::Commit(batch)) if batch.conversations.len()==1)
        );
        session.close().await.unwrap();
    }
    #[tokio::test]
    async fn concurrent_creation_assigns_unique_membership_ids_and_seq() {
        let session = Arc::new(
            DurableSession::open(Box::new(MemoryStorage::new()))
                .await
                .unwrap(),
        );
        let mut jobs = vec![];
        for _ in 0..16 {
            let session = session.clone();
            jobs.push(tokio::spawn(async move {
                session
                    .create_conversation(ConversationOwnership::Ownerless)
                    .await
                    .unwrap()
            }));
        }
        let mut records = vec![];
        for job in jobs {
            records.push(job.await.unwrap());
        }
        records.sort_by_key(|record| record.id);
        assert_eq!(
            records
                .iter()
                .map(|record| record.id.get())
                .collect::<Vec<_>>(),
            (2..=17).collect::<Vec<_>>()
        );
        assert_eq!(session.snapshot().await.unwrap().next_seq, 17);
        session.close().await.unwrap();
    }
}
