#[cfg(test)]
mod tests {
    use crate::durable::storage::{StorageFuture, WriterClaim};
    use crate::durable::*;
    use serde_json::json;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    use std::time::Duration;
    use tokio::sync::Notify;
    fn root() -> ConversationId {
        ConversationId::new(1).unwrap()
    }
    fn missing() -> ConversationId {
        ConversationId::new(999).unwrap()
    }
    fn task(state: TaskState, abort_requested: bool) -> TaskRecord {
        let terminal = state.terminal();
        TaskRecord {
            id: TaskId::new(1).unwrap(),
            conversation_id: root(),
            kind: "generation".into(),
            version: 1,
            owner_task_id: None,
            state,
            input: json!({}),
            checkpoint: json!({}),
            outcome: terminal.then(|| json!({})),
            abort_requested,
            started_at: None,
            ended_at: None,
            updated_seq: CommitSeq::new(1).unwrap(),
        }
    }
    fn write(state: &StorageSnapshot) -> CommitBatch {
        CommitBatch {
            seq: CommitSeq::new(state.next_seq).unwrap(),
            next_id: state.next_id,
            next_seq: state.next_seq + 1,
            conversations: vec![],
            generic_documents: vec![],
            entries: vec![],
            tasks: vec![],
            submissions: vec![],
            documents: vec![],
        }
    }
    async fn with_owner(state: TaskState, abort: bool) -> DurableSession {
        let session = DurableSession::open(Box::new(MemoryStorage::new()))
            .await
            .unwrap();
        let mut batch = write(&session.snapshot().await.unwrap());
        batch.next_id = 2;
        batch.tasks.push(task(state, abort));
        session.commit(batch).await.unwrap();
        session
    }
    #[tokio::test]
    async fn missing_conversation_appends_reject_without_minting_or_publication() {
        let session = DurableSession::open(Box::new(MemoryStorage::new()))
            .await
            .unwrap();
        let before = session.snapshot().await.unwrap();
        let mut watch = session.watch().await.unwrap();
        assert!(matches!(
            watch.next().await,
            Some(DurableEvent::Snapshot(_))
        ));
        assert_eq!(
            session
                .append_entry(missing(), EntryDraft::new("note"))
                .await
                .unwrap_err(),
            DurableError::Rejected("conversation missing".into())
        );
        assert!(
            session
                .transact_entries(|tx| {
                    tx.append_entry(root(), EntryDraft::new("rollback"))?;
                    assert!(
                        tx.append_entry(missing(), EntryDraft::new("missing"))
                            .is_err()
                    );
                    Ok(())
                })
                .await
                .is_err()
        );
        assert_eq!(session.snapshot().await.unwrap(), before);
        assert!(
            tokio::time::timeout(Duration::from_millis(10), watch.next())
                .await
                .is_err()
        );
        watch.stop();
        session.close().await.unwrap();
    }
    #[tokio::test]
    async fn staged_conversations_admit_entries_and_documents_on_same_mutation_line() {
        let session = DurableSession::open(Box::new(MemoryStorage::new()))
            .await
            .unwrap();
        let (conversation, entry) = session
            .transact_entries(|tx| {
                let conversation = tx.create_conversation(ConversationOwnership::Ownerless)?;
                let definition = DocumentDefinition::new(
                    "admission",
                    1,
                    DocumentHistory::Latest,
                    DocumentFork::Initial,
                    || Ok(json!({"created":true})),
                )?;
                tx.edit_document(&definition, conversation.id, None, &(), |_| Ok(()))?;
                let entry = tx.append_entry(conversation.id, EntryDraft::new("same-batch"))?;
                Ok((conversation, entry))
            })
            .await
            .unwrap();
        assert_eq!(entry.conversation_id, conversation.id);
        assert_eq!(
            session
                .entries(conversation.id, Default::default())
                .await
                .unwrap()
                .items,
            vec![entry]
        );
        assert_eq!(
            session
                .documents(DocumentQuery::current(conversation.id))
                .await
                .unwrap()
                .items
                .len(),
            1
        );
        session
            .append_entry(conversation.id, EntryDraft::new("next-batch"))
            .await
            .unwrap();
        session.close().await.unwrap();
    }
    #[tokio::test]
    async fn missing_document_owner_rejects_before_initializer_editor_and_related_staging() {
        let session = DurableSession::open(Box::new(MemoryStorage::new()))
            .await
            .unwrap();
        let before = session.snapshot().await.unwrap();
        let calls = Arc::new(AtomicUsize::new(0));
        let count = calls.clone();
        let definition = DocumentDefinition::new(
            "admission",
            1,
            DocumentHistory::Latest,
            DocumentFork::Initial,
            move || {
                count.fetch_add(1, Ordering::SeqCst);
                Ok(json!({}))
            },
        )
        .unwrap();
        assert!(
            session
                .transact_entries(move |tx| {
                    tx.append_entry(root(), EntryDraft::new("rollback"))?;
                    assert!(
                        tx.edit_document(
                            &definition,
                            missing(),
                            None,
                            &(),
                            |_| -> Result<(), DurableError> {
                                panic!("must not edit missing owner")
                            }
                        )
                        .is_err()
                    );
                    Ok(())
                })
                .await
                .is_err()
        );
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert_eq!(session.snapshot().await.unwrap(), before);
        session.close().await.unwrap();
    }
    #[tokio::test]
    async fn live_owned_conversations_accept_pending_and_running_tasks() {
        for state in [TaskState::Pending, TaskState::Running] {
            let session = with_owner(state, false).await;
            let conversation = session
                .create_conversation(ConversationOwnership::Task {
                    task_id: TaskId::new(1).unwrap(),
                })
                .await
                .unwrap();
            assert_eq!(conversation.owner.unwrap().task_id, TaskId::new(1).unwrap());
            session
                .append_entry(conversation.id, EntryDraft::new("owned"))
                .await
                .unwrap();
            session.close().await.unwrap();
        }
    }
    #[tokio::test]
    async fn completing_terminal_and_abort_marked_owners_reject_atomic_creation_and_forks() {
        for (state, abort) in [
            (TaskState::Completing, false),
            (TaskState::Succeeded, false),
            (TaskState::Failed, false),
            (TaskState::Aborted, false),
            (TaskState::Pending, true),
            (TaskState::Running, true),
        ] {
            let session = with_owner(state, abort).await;
            let cutoff = session
                .append_entry(root(), EntryDraft::new("cut"))
                .await
                .unwrap();
            let before = session.snapshot().await.unwrap();
            let owner = ConversationOwnership::Task {
                task_id: TaskId::new(1).unwrap(),
            };
            assert!(session.create_conversation(owner).await.is_err());
            assert!(
                session
                    .fork_conversation(root(), cutoff.id, owner)
                    .await
                    .is_err()
            );
            assert_eq!(session.snapshot().await.unwrap(), before);
            session.close().await.unwrap();
        }
    }
    #[tokio::test]
    async fn raw_conversation_owner_checks_use_final_same_batch_candidate() {
        let session = with_owner(TaskState::Running, false).await;
        let before = session.snapshot().await.unwrap();
        for mode in 0..3 {
            let mut batch = write(&before);
            let mut owner = before.tasks[&TaskId::new(1).unwrap()].clone();
            owner.updated_seq = batch.seq;
            match mode {
                0 => owner.state = TaskState::Completing,
                1 => owner.abort_requested = true,
                _ => {
                    owner.state = TaskState::Succeeded;
                    owner.outcome = Some(json!({}));
                }
            }
            batch.tasks.push(owner);
            batch.next_id += 1;
            batch.conversations.push(ConversationRecord {
                id: ConversationId::new(before.next_id).unwrap(),
                parent: None,
                owner: Some(ConversationOwner {
                    conversation_id: root(),
                    task_id: TaskId::new(1).unwrap(),
                }),
                created_seq: Some(batch.seq),
            });
            assert!(session.commit(batch).await.is_err());
            assert_eq!(session.snapshot().await.unwrap(), before);
        }
        let mut batch = write(&before);
        batch.next_id += 1;
        batch.conversations.push(ConversationRecord {
            id: ConversationId::new(before.next_id).unwrap(),
            parent: None,
            owner: Some(ConversationOwner {
                conversation_id: root(),
                task_id: TaskId::new(1).unwrap(),
            }),
            created_seq: Some(batch.seq),
        });
        session.commit(batch).await.unwrap();
        session.close().await.unwrap();
    }
    #[tokio::test]
    async fn raw_legacy_batches_and_journal_replay_keep_inferred_scopes_without_reallocation() {
        let dir = std::env::temp_dir().join(format!("rs-ai-admission-{}", crate::utils::uuidv7()));
        std::fs::create_dir(&dir).unwrap();
        let path = dir.join("journal");
        let session = DurableSession::open(Box::new(JournalStorage::open(&path).unwrap()))
            .await
            .unwrap();
        let mut batch = write(&session.snapshot().await.unwrap());
        batch.next_id = 2;
        batch.entries.push(EntryRecord {
            id: EntryId::new(1).unwrap(),
            conversation_id: missing(),
            kind: "user".into(),
            value: json!({"text":"legacy"}),
            by_task_id: None,
            created_seq: batch.seq,
        });
        session.commit(batch).await.unwrap();
        let before = session.snapshot().await.unwrap();
        session.close().await.unwrap();
        let session = DurableSession::open(Box::new(JournalStorage::open(&path).unwrap()))
            .await
            .unwrap();
        assert_eq!(session.snapshot().await.unwrap(), before);
        assert!(session.conversation(missing()).await.unwrap().is_some());
        let entry = session
            .append_entry(missing(), EntryDraft::new("after-replay"))
            .await
            .unwrap();
        assert_eq!(entry.id.get(), 2);
        session.close().await.unwrap();
        std::fs::remove_dir_all(dir).unwrap();
    }
    struct BarrierStorage {
        memory: MemoryStorage,
        admitted: Arc<Notify>,
        release: Arc<Notify>,
    }
    impl DurableStorage for BarrierStorage {
        fn claim_writer(&self) -> Result<WriterClaim, DurableError> {
            self.memory.claim_writer()
        }
        fn load<'a>(&'a self, claim: &'a WriterClaim) -> StorageFuture<'a, StorageSnapshot> {
            self.memory.load(claim)
        }
        fn close<'a>(&'a self, claim: &'a WriterClaim) -> StorageFuture<'a, ()> {
            self.memory.close(claim)
        }
        fn commit<'a>(
            &'a self,
            claim: &'a WriterClaim,
            batch: CommitBatch,
        ) -> StorageFuture<'a, ()> {
            Box::pin(async move {
                self.admitted.notify_one();
                self.release.notified().await;
                self.memory.commit(claim, batch).await
            })
        }
    }
    #[tokio::test]
    async fn queued_append_checks_adopted_membership_not_callers_earlier_snapshot() {
        let admitted = Arc::new(Notify::new());
        let release = Arc::new(Notify::new());
        let session = Arc::new(
            DurableSession::open(Box::new(BarrierStorage {
                memory: MemoryStorage::new(),
                admitted: admitted.clone(),
                release: release.clone(),
            }))
            .await
            .unwrap(),
        );
        let creator = {
            let session = session.clone();
            tokio::spawn(async move {
                session
                    .create_conversation(ConversationOwnership::Ownerless)
                    .await
            })
        };
        admitted.notified().await;
        let mut append = Box::pin(session.append_entry(
            ConversationId::new(2).unwrap(),
            EntryDraft::new("queued after creation"),
        ));
        assert!(
            tokio::time::timeout(Duration::from_millis(10), &mut append)
                .await
                .is_err()
        );
        release.notify_one();
        let created = creator.await.unwrap().unwrap();
        assert_eq!(created.id.get(), 2);
        admitted.notified().await;
        release.notify_one();
        let appended = append.await.unwrap();
        assert_eq!(appended.conversation_id, created.id);
        assert_eq!(session.snapshot().await.unwrap().entries.len(), 1);
        session.close().await.unwrap();
    }
    #[tokio::test]
    async fn queued_owned_creation_rechecks_abort_mark_on_adoption() {
        let admitted = Arc::new(Notify::new());
        let release = Arc::new(Notify::new());
        let session = Arc::new(
            DurableSession::open(Box::new(BarrierStorage {
                memory: MemoryStorage::new(),
                admitted: admitted.clone(),
                release: release.clone(),
            }))
            .await
            .unwrap(),
        );
        let mut initial = write(&StorageSnapshot::empty());
        initial.next_id = 2;
        initial.tasks.push(task(TaskState::Running, false));
        let bootstrap = {
            let session = session.clone();
            tokio::spawn(async move { session.commit(initial).await })
        };
        admitted.notified().await;
        release.notify_one();
        bootstrap.await.unwrap().unwrap();
        let before = session.snapshot().await.unwrap();
        let mut marked = write(&before);
        let mut owner = before.tasks[&TaskId::new(1).unwrap()].clone();
        owner.abort_requested = true;
        owner.updated_seq = marked.seq;
        marked.tasks.push(owner);
        let mark = {
            let session = session.clone();
            tokio::spawn(async move { session.commit(marked).await })
        };
        admitted.notified().await;
        let mut creation = Box::pin(session.create_conversation(ConversationOwnership::Task {
            task_id: TaskId::new(1).unwrap(),
        }));
        assert!(
            tokio::time::timeout(Duration::from_millis(10), &mut creation)
                .await
                .is_err()
        );
        release.notify_one();
        mark.await.unwrap().unwrap();
        assert!(creation.await.is_err());
        let after = session.snapshot().await.unwrap();
        assert_eq!(after.next_id, before.next_id);
        assert_eq!(after.conversations, before.conversations);
        session.close().await.unwrap();
    }
}
