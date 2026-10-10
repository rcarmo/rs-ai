#[cfg(test)]
mod tests {
    use crate::durable::storage::{StorageFuture, WriterClaim};
    use crate::durable::*;
    use serde_json::json;
    use std::sync::{Arc, Mutex};
    use tokio::sync::Notify;

    fn batch(seq: u64) -> CommitBatch {
        let s = CommitSeq::new(seq).unwrap();
        CommitBatch {
            generic_documents: vec![],
            conversations: vec![],
            seq: s,
            next_id: 2,
            next_seq: seq + 1,
            entries: vec![EntryRecord {
                id: EntryId::new(1).unwrap(),
                conversation_id: ConversationId::new(1).unwrap(),
                kind: "user".into(),
                value: json!({"seq":seq}),
                by_task_id: None,
                created_seq: s,
            }],
            tasks: vec![],
            submissions: vec![],
            documents: vec![],
        }
    }

    fn task_batch(seq: u64, state: TaskState, started_at: Option<i64>) -> CommitBatch {
        let commit_seq = CommitSeq::new(seq).unwrap();
        let terminal = state.terminal();
        CommitBatch {
            generic_documents: vec![],
            conversations: vec![],
            seq: commit_seq,
            next_id: 2,
            next_seq: seq + 1,
            entries: vec![],
            tasks: vec![TaskRecord {
                id: TaskId::new(1).unwrap(),
                conversation_id: ConversationId::new(1).unwrap(),
                kind: "generation".into(),
                version: 1,
                owner_task_id: None,
                state,
                input: json!({}),
                checkpoint: json!({}),
                outcome: terminal.then(|| json!({"ok":true})),
                abort_requested: false,
                started_at,
                ended_at: None,
                updated_seq: commit_seq,
            }],
            submissions: vec![],
            documents: vec![],
        }
    }

    #[tokio::test]
    async fn injected_clock_stamps_only_lifecycle_transitions_and_preserves_recovery_start() {
        let clock = Arc::new(std::sync::atomic::AtomicI64::new(100));
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let now = clock.clone();
        let count = calls.clone();
        let session = DurableSession::open_with_clock(
            Box::new(MemoryStorage::new()),
            Arc::new(move || {
                count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                now.load(std::sync::atomic::Ordering::SeqCst)
            }),
        )
        .await
        .unwrap();
        session
            .commit(task_batch(1, TaskState::Pending, None))
            .await
            .unwrap();
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 0);
        session
            .commit(task_batch(2, TaskState::Running, None))
            .await
            .unwrap();
        clock.store(200, std::sync::atomic::Ordering::SeqCst);
        session
            .commit(task_batch(3, TaskState::Pending, None))
            .await
            .unwrap();
        session
            .commit(task_batch(4, TaskState::Running, None))
            .await
            .unwrap();
        let running = session.snapshot().await.unwrap();
        assert_eq!(
            running.tasks[&TaskId::new(1).unwrap()].started_at,
            Some(100)
        );
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
        session
            .commit(task_batch(5, TaskState::Succeeded, None))
            .await
            .unwrap();
        let task = session.snapshot().await.unwrap().tasks[&TaskId::new(1).unwrap()].clone();
        assert_eq!(task.started_at, Some(100));
        assert_eq!(task.ended_at, Some(200));
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 2);
        session.close().await.unwrap();
    }

    #[tokio::test]
    async fn invalid_clock_timestamp_rejects_without_mutating_state() {
        let session =
            DurableSession::open_with_clock(Box::new(MemoryStorage::new()), Arc::new(|| -1))
                .await
                .unwrap();
        let before = session.snapshot().await.unwrap();
        assert!(matches!(
            session
                .commit(task_batch(1, TaskState::Running, None))
                .await,
            Err(DurableError::Rejected(_))
        ));
        assert_eq!(session.snapshot().await.unwrap(), before);
        session.close().await.unwrap();
    }

    #[tokio::test]
    async fn native_context_cache_reuses_detached_reads_and_extends_after_commit() {
        let session = DurableSession::open(Box::new(MemoryStorage::new()))
            .await
            .unwrap();
        let conversation = ConversationId::new(1).unwrap();
        let mut first = batch(1);
        first.entries[0].value = json!({"text":"first"});
        session.commit(first).await.unwrap();
        let mut messages = session.message_context(conversation, None).await.unwrap();
        messages[0].content.clear();
        assert!(
            !session.message_context(conversation, None).await.unwrap()[0]
                .content
                .is_empty()
        );
        assert_eq!(session.context_cache_stats().await, (1, 1));
        assert!(session.commit(batch(1)).await.is_err());
        assert_eq!(
            session.context_cache_stats().await,
            (1, 1),
            "rejected mutation does not invalidate derived context"
        );
        let mut second = batch(2);
        second.entries[0].id = EntryId::new(2).unwrap();
        second.entries[0].value = json!({"text":"second"});
        second.next_id = 3;
        session.commit(second).await.unwrap();
        assert_eq!(session.context_cache_stats().await, (1, 1));
        assert_eq!(
            session
                .message_context(conversation, None)
                .await
                .unwrap()
                .len(),
            2
        );
        assert_eq!(session.context_cache_stats().await, (1, 2));
        assert_eq!(
            session
                .message_context(conversation, Some(EntryId::new(1).unwrap()))
                .await
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            session.context_cache_stats().await,
            (1, 3),
            "historical cut does not replace current cache"
        );
        session.close().await.unwrap();
        assert!(matches!(
            session.message_context(conversation, None).await,
            Err(DurableError::Closed)
        ));
    }

    #[tokio::test]
    async fn retained_context_survives_unrelated_commits_but_task_input_changes_invalidate() {
        let session = DurableSession::open(Box::new(MemoryStorage::new()))
            .await
            .unwrap();
        let conversation = ConversationId::new(1).unwrap();
        session
            .commit(task_batch(1, TaskState::Pending, None))
            .await
            .unwrap();
        let mut draft = EntryDraft::new("note");
        draft.model = Some(vec![crate::user_message("kept")]);
        session.append_entry(conversation, draft).await.unwrap();
        session.message_context(conversation, None).await.unwrap();
        assert_eq!(session.context_cache_stats().await, (1, 1));
        session
            .append_entry(ConversationId::new(2).unwrap(), EntryDraft::new("foreign"))
            .await
            .unwrap();
        session.message_context(conversation, None).await.unwrap();
        assert_eq!(session.context_cache_stats().await, (1, 1));
        let state = session.snapshot().await.unwrap();
        let mut task = state.tasks[&TaskId::new(1).unwrap()].clone();
        task.checkpoint = json!({"changed":true});
        task.updated_seq = CommitSeq::new(state.next_seq).unwrap();
        session
            .commit(CommitBatch {
                generic_documents: vec![],
                conversations: vec![],
                seq: task.updated_seq,
                next_id: state.next_id,
                next_seq: state.next_seq + 1,
                tasks: vec![task],
                entries: vec![],
                submissions: vec![],
                documents: vec![],
            })
            .await
            .unwrap();
        session.message_context(conversation, None).await.unwrap();
        assert_eq!(session.context_cache_stats().await, (1, 1));
        let state = session.snapshot().await.unwrap();
        let mut task = state.tasks[&TaskId::new(1).unwrap()].clone();
        task.input = json!({"changed":"model identity"});
        task.updated_seq = CommitSeq::new(state.next_seq).unwrap();
        session
            .commit(CommitBatch {
                generic_documents: vec![],
                conversations: vec![],
                seq: task.updated_seq,
                next_id: state.next_id,
                next_seq: state.next_seq + 1,
                tasks: vec![task],
                entries: vec![],
                submissions: vec![],
                documents: vec![],
            })
            .await
            .unwrap();
        assert_eq!(session.context_cache_stats().await, (0, 1));
        session.message_context(conversation, None).await.unwrap();
        assert_eq!(session.context_cache_stats().await, (1, 2));
        session.close().await.unwrap();
    }

    #[tokio::test]
    async fn retention_budget_overflow_returns_context_without_retaining_range() {
        let session = DurableSession::open(Box::new(MemoryStorage::new()))
            .await
            .unwrap();
        let conversation = ConversationId::new(1).unwrap();
        let mut draft = EntryDraft::new("large-context");
        draft.model = Some(vec![crate::user_message("small"); 4096]);
        session.append_entry(conversation, draft).await.unwrap();
        assert_eq!(
            session
                .message_context(conversation, None)
                .await
                .unwrap()
                .len(),
            4096
        );
        assert_eq!(session.context_cache_stats().await, (0, 1));
        assert_eq!(
            session
                .message_context(conversation, None)
                .await
                .unwrap()
                .len(),
            4096
        );
        assert_eq!(session.context_cache_stats().await, (0, 2));
        session.close().await.unwrap();
    }

    #[tokio::test]
    async fn busy_ranges_survive_zero_and_short_retention_until_terminal_settlement() {
        for retention in [
            std::time::Duration::ZERO,
            std::time::Duration::from_millis(20),
        ] {
            let session = DurableSession::open_with_settings(
                Box::new(MemoryStorage::new()),
                Arc::new(|| 100),
                SessionSettings {
                    context_retention: retention,
                },
            )
            .await
            .unwrap();
            let conversation = ConversationId::new(1).unwrap();
            session
                .commit(task_batch(1, TaskState::Pending, None))
                .await
                .unwrap();
            let mut draft = EntryDraft::new("note");
            draft.model = Some(vec![crate::user_message("busy")]);
            session.append_entry(conversation, draft).await.unwrap();
            session.message_context(conversation, None).await.unwrap();
            assert_eq!(session.context_cache_stats().await, (1, 1));
            tokio::time::sleep(std::time::Duration::from_millis(30)).await;
            session.message_context(conversation, None).await.unwrap();
            assert_eq!(session.context_cache_stats().await, (1, 1));
            let mut running = task_batch(3, TaskState::Running, None);
            running.next_id = 3;
            session.commit(running).await.unwrap();
            let mut terminal = task_batch(4, TaskState::Succeeded, None);
            terminal.next_id = 3;
            session.commit(terminal).await.unwrap();
            if retention.is_zero() {
                assert_eq!(session.context_cache_stats().await, (0, 1));
            } else {
                assert_eq!(session.context_cache_stats().await, (1, 1));
                tokio::time::sleep(std::time::Duration::from_millis(30)).await;
                assert_eq!(session.context_cache_stats().await, (0, 1));
            }
            session.close().await.unwrap();
        }
    }

    #[tokio::test]
    async fn idle_reads_do_not_slide_retention_deadline() {
        let session = DurableSession::open_with_settings(
            Box::new(MemoryStorage::new()),
            Arc::new(|| 100),
            SessionSettings {
                context_retention: std::time::Duration::from_millis(200),
            },
        )
        .await
        .unwrap();
        let conversation = ConversationId::new(1).unwrap();
        let mut draft = EntryDraft::new("note");
        draft.model = Some(vec![crate::user_message("idle")]);
        session.append_entry(conversation, draft).await.unwrap();
        session.message_context(conversation, None).await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(120)).await;
        session.message_context(conversation, None).await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(110)).await;
        assert_eq!(session.context_cache_stats().await.0, 0);
        session.close().await.unwrap();
    }

    #[tokio::test]
    async fn context_retention_expiry_and_zero_mode_drop_cached_messages() {
        for retention in [
            std::time::Duration::ZERO,
            std::time::Duration::from_millis(20),
        ] {
            let session = DurableSession::open_with_settings(
                Box::new(MemoryStorage::new()),
                Arc::new(crate::utils::now_millis),
                SessionSettings {
                    context_retention: retention,
                },
            )
            .await
            .unwrap();
            let conversation = ConversationId::new(1).unwrap();
            let mut first = batch(1);
            first.entries[0].value = json!({"text":"first"});
            session.commit(first).await.unwrap();
            session.message_context(conversation, None).await.unwrap();
            assert_eq!(
                session.context_cache_stats().await.0,
                usize::from(!retention.is_zero())
            );
            tokio::time::sleep(std::time::Duration::from_millis(30)).await;
            assert_eq!(session.context_cache_stats().await.0, 0);
            session.message_context(conversation, None).await.unwrap();
            assert_eq!(session.context_cache_stats().await.1, 2);
            session.close().await.unwrap();
        }
    }

    async fn repeated_context_reads(retention: std::time::Duration) {
        let session = DurableSession::open_with_settings(
            Box::new(MemoryStorage::new()),
            Arc::new(crate::utils::now_millis),
            SessionSettings {
                context_retention: retention,
            },
        )
        .await
        .unwrap();
        let conversation = ConversationId::new(1).unwrap();
        let seq = CommitSeq::new(1).unwrap();
        let entries = (1..=128)
            .map(|id| EntryRecord {
                id: EntryId::new(id).unwrap(),
                conversation_id: conversation,
                kind: "user".into(),
                value: json!({"text":"x".repeat(256)}),
                by_task_id: None,
                created_seq: seq,
            })
            .collect();
        session
            .commit(CommitBatch {
                generic_documents: vec![],
                conversations: vec![],
                seq,
                next_id: 129,
                next_seq: 2,
                entries,
                tasks: vec![],
                submissions: vec![],
                documents: vec![],
            })
            .await
            .unwrap();
        for _ in 0..64 {
            let messages = session.message_context(conversation, None).await.unwrap();
            assert_eq!(messages.len(), 128);
            assert!(
                matches!(messages[127].content.first(), Some(crate::types::ContentBlock::Text { text, .. }) if text.len() == 256)
            );
        }
        assert_eq!(
            session.context_cache_stats().await.1,
            if retention.is_zero() { 64 } else { 1 }
        );
        session.close().await.unwrap();
    }

    #[tokio::test]
    async fn growing_native_context_reads_incremental_workload() {
        let session = DurableSession::open(Box::new(MemoryStorage::new()))
            .await
            .unwrap();
        let conversation = ConversationId::new(1).unwrap();
        for id in 1..=128 {
            let mut draft = EntryDraft::new("note");
            draft.model = Some(vec![crate::user_message(&"x".repeat(256))]);
            session.append_entry(conversation, draft).await.unwrap();
            let messages = session.message_context(conversation, None).await.unwrap();
            assert_eq!(messages.len(), id);
        }
        assert_eq!(session.context_cache_stats().await, (1, 128));
        session.close().await.unwrap();
    }

    #[tokio::test]
    async fn repeated_native_context_reads_uncached() {
        repeated_context_reads(std::time::Duration::ZERO).await;
    }

    #[tokio::test]
    async fn repeated_native_context_reads_cached() {
        repeated_context_reads(std::time::Duration::from_secs(600)).await;
    }

    struct BarrierStorage {
        inner: MemoryStorage,
        inner_claim: WriterClaim,
        admitted: Arc<Notify>,
        release: Arc<Notify>,
        commits: Arc<Mutex<usize>>,
        uncertain: bool,
    }
    impl BarrierStorage {
        fn new(uncertain: bool) -> (Self, Arc<Notify>, Arc<Notify>, Arc<Mutex<usize>>) {
            let inner = MemoryStorage::new();
            let inner_claim = inner.claim_writer().unwrap();
            let admitted = Arc::new(Notify::new());
            let release = Arc::new(Notify::new());
            let commits = Arc::new(Mutex::new(0));
            (
                Self {
                    inner,
                    inner_claim,
                    admitted: admitted.clone(),
                    release: release.clone(),
                    commits: commits.clone(),
                    uncertain,
                },
                admitted,
                release,
                commits,
            )
        }
    }
    impl DurableStorage for BarrierStorage {
        fn claim_writer(&self) -> Result<WriterClaim, DurableError> {
            Ok(WriterClaim::fresh())
        }
        fn load<'a>(&'a self, _claim: &'a WriterClaim) -> StorageFuture<'a, StorageSnapshot> {
            self.inner.load(&self.inner_claim)
        }
        fn commit<'a>(&'a self, _claim: &'a WriterClaim, b: CommitBatch) -> StorageFuture<'a, ()> {
            Box::pin(async move {
                *self.commits.lock().unwrap() += 1;
                self.admitted.notify_waiters();
                self.release.notified().await;
                if self.uncertain {
                    return Err(DurableError::Uncertain("injected".into()));
                }
                self.inner.commit(&self.inner_claim, b).await
            })
        }
        fn close<'a>(&'a self, _claim: &'a WriterClaim) -> StorageFuture<'a, ()> {
            self.inner.close(&self.inner_claim)
        }
    }

    #[tokio::test]
    async fn entry_transaction_cancellation_before_and_after_storage_admission() {
        let (storage, admitted, release, commits) = BarrierStorage::new(false);
        let session = Arc::new(DurableSession::open(Box::new(storage)).await.unwrap());
        let writer = session.clone();
        let first = tokio::spawn(async move {
            writer
                .transact_entries(|tx| {
                    tx.append_entry(ConversationId::new(1)?, EntryDraft::new("first"))?;
                    tx.append_entry(ConversationId::new(1)?, EntryDraft::new("second"))?;
                    Ok(())
                })
                .await
        });
        admitted.notified().await;
        let called = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let callback_called = called.clone();
        let mut queued = Box::pin(session.transact_entries(move |tx| {
            callback_called.store(true, std::sync::atomic::Ordering::SeqCst);
            tx.append_entry(ConversationId::new(1)?, EntryDraft::new("cancelled"))?;
            Ok(())
        }));
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(10), &mut queued)
                .await
                .is_err()
        );
        drop(queued);
        first.abort();
        let _ = first.await;
        release.notify_waiters();
        let state = session.snapshot().await.unwrap();
        assert_eq!(state.entries.len(), 2);
        assert_eq!(state.next_id, 3);
        assert_eq!(state.next_seq, 2);
        assert!(!called.load(std::sync::atomic::Ordering::SeqCst));
        assert_eq!(*commits.lock().unwrap(), 1);
        session.close().await.unwrap();
    }

    #[tokio::test]
    async fn entry_transaction_uncertainty_poisons_and_close_waits_for_batch_settlement() {
        for uncertain in [false, true] {
            let (storage, admitted, release, _) = BarrierStorage::new(uncertain);
            let session = Arc::new(DurableSession::open(Box::new(storage)).await.unwrap());
            let mut watch = session.watch().await.unwrap();
            watch.next().await.unwrap();
            let writer = session.clone();
            let transaction = tokio::spawn(async move {
                writer
                    .transact_entries(|tx| {
                        tx.append_entry(ConversationId::new(1)?, EntryDraft::new("one"))?;
                        tx.append_entry(ConversationId::new(1)?, EntryDraft::new("two"))?;
                        Ok(42)
                    })
                    .await
            });
            admitted.notified().await;
            assert!(
                tokio::time::timeout(std::time::Duration::from_millis(10), watch.next())
                    .await
                    .is_err()
            );
            if uncertain {
                release.notify_waiters();
                assert!(matches!(
                    transaction.await.unwrap(),
                    Err(DurableError::Uncertain(_))
                ));
                assert_eq!(
                    watch.next().await,
                    Some(DurableEvent::End(WatchEnd::Poisoned))
                );
                assert!(matches!(
                    session.transact_entries(|_| Ok(())).await,
                    Err(DurableError::Poisoned)
                ));
                session.close().await.unwrap();
            } else {
                let closer = session.clone();
                let close = tokio::spawn(async move { closer.close().await });
                tokio::task::yield_now().await;
                assert!(!close.is_finished());
                release.notify_waiters();
                assert_eq!(transaction.await.unwrap().unwrap(), 42);
                assert!(
                    matches!(watch.next().await,Some(DurableEvent::Commit(batch)) if batch.entries.len()==2)
                );
                close.await.unwrap().unwrap();
            }
        }
    }

    #[tokio::test]
    async fn generic_append_dropped_before_dequeue_does_not_consume_identity() {
        let (storage, admitted, release, commits) = BarrierStorage::new(false);
        let session = Arc::new(DurableSession::open(Box::new(storage)).await.unwrap());
        let writer = session.clone();
        let first = tokio::spawn(async move {
            writer
                .append_entry(ConversationId::new(1).unwrap(), EntryDraft::new("first"))
                .await
        });
        admitted.notified().await;
        let mut queued = Box::pin(session.append_entry(
            ConversationId::new(1).unwrap(),
            EntryDraft::new("cancelled"),
        ));
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(10), &mut queued)
                .await
                .is_err()
        );
        drop(queued);
        release.notify_waiters();
        assert_eq!(first.await.unwrap().unwrap().id.get(), 1);
        let snapshot = session.snapshot().await.unwrap();
        assert_eq!(snapshot.entries.len(), 1);
        assert_eq!(snapshot.next_id, 2);
        assert_eq!(snapshot.next_seq, 2);
        assert_eq!(*commits.lock().unwrap(), 1);
        session.close().await.unwrap();
    }

    #[tokio::test]
    async fn generic_append_survives_admitted_caller_drop_and_poison_rejects_later_appends() {
        for uncertain in [false, true] {
            let (storage, admitted, release, commits) = BarrierStorage::new(uncertain);
            let session = Arc::new(DurableSession::open(Box::new(storage)).await.unwrap());
            let mut watch = session.watch().await.unwrap();
            watch.next().await.unwrap();
            let writer = session.clone();
            let append = tokio::spawn(async move {
                writer
                    .append_entry(ConversationId::new(1).unwrap(), EntryDraft::new("note"))
                    .await
            });
            admitted.notified().await;
            append.abort();
            let _ = append.await;
            assert!(
                tokio::time::timeout(std::time::Duration::from_millis(10), watch.next())
                    .await
                    .is_err()
            );
            release.notify_waiters();
            if uncertain {
                assert_eq!(
                    watch.next().await,
                    Some(DurableEvent::End(WatchEnd::Poisoned))
                );
                assert!(matches!(
                    session
                        .append_entry(ConversationId::new(1).unwrap(), EntryDraft::new("later"))
                        .await,
                    Err(DurableError::Poisoned)
                ));
            } else {
                assert!(
                    matches!(watch.next().await, Some(DurableEvent::Commit(batch)) if batch.entries[0].kind == "note")
                );
                assert_eq!(session.snapshot().await.unwrap().entries.len(), 1);
            }
            assert_eq!(*commits.lock().unwrap(), 1);
            session.close().await.unwrap();
        }
    }

    #[tokio::test]
    async fn caller_drop_after_admission_does_not_abandon_settlement_or_ordering() {
        let (storage, admitted, release, commits) = BarrierStorage::new(false);
        let session = Arc::new(DurableSession::open(Box::new(storage)).await.unwrap());
        let worker = session.clone();
        let task = tokio::spawn(async move { worker.commit(batch(1)).await });
        admitted.notified().await;
        task.abort();
        let _ = task.await;
        let close_session = session.clone();
        let close = tokio::spawn(async move { close_session.close().await });
        tokio::task::yield_now().await;
        assert!(!close.is_finished());
        release.notify_waiters();
        assert!(close.await.unwrap().is_ok());
        assert_eq!(*commits.lock().unwrap(), 1);
    }

    #[tokio::test]
    async fn close_seals_admission_waits_and_repeated_callers_share_outcome() {
        let (storage, admitted, release, _) = BarrierStorage::new(false);
        let session = Arc::new(DurableSession::open(Box::new(storage)).await.unwrap());
        let s = session.clone();
        let commit = tokio::spawn(async move { s.commit(batch(1)).await });
        admitted.notified().await;
        let s1 = session.clone();
        let close1 = tokio::spawn(async move { s1.close().await });
        let s2 = session.clone();
        let close2 = tokio::spawn(async move { s2.close().await });
        tokio::task::yield_now().await;
        assert!(matches!(
            session.commit(batch(2)).await,
            Err(DurableError::Closed)
        ));
        assert!(!close1.is_finished());
        release.notify_waiters();
        assert!(commit.await.unwrap().is_ok());
        assert!(close1.await.unwrap().is_ok());
        assert!(close2.await.unwrap().is_ok());
    }

    #[tokio::test]
    async fn dropped_close_caller_does_not_cancel_owned_close() {
        let (storage, admitted, release, _) = BarrierStorage::new(false);
        let session = Arc::new(DurableSession::open(Box::new(storage)).await.unwrap());
        let s = session.clone();
        let commit = tokio::spawn(async move { s.commit(batch(1)).await });
        admitted.notified().await;
        let s = session.clone();
        let dropped = tokio::spawn(async move { s.close().await });
        tokio::task::yield_now().await;
        dropped.abort();
        let _ = dropped.await;
        release.notify_waiters();
        assert!(commit.await.unwrap().is_ok());
        assert!(session.close().await.is_ok());
    }

    #[tokio::test]
    async fn uncertain_commit_poisons_reads_and_later_dispatch() {
        let (storage, admitted, release, _) = BarrierStorage::new(true);
        let session = Arc::new(DurableSession::open(Box::new(storage)).await.unwrap());
        let s = session.clone();
        let mut watch = session.watch().await.unwrap();
        assert!(matches!(
            watch.next().await,
            Some(DurableEvent::Snapshot(_))
        ));
        let task = tokio::spawn(async move { s.commit(batch(1)).await });
        admitted.notified().await;
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(10), watch.next())
                .await
                .is_err()
        );
        release.notify_waiters();
        assert!(matches!(
            task.await.unwrap(),
            Err(DurableError::Uncertain(_))
        ));
        assert_eq!(
            watch.next().await,
            Some(DurableEvent::End(WatchEnd::Poisoned))
        );
        assert!(matches!(session.watch().await, Err(DurableError::Poisoned)));
        assert!(matches!(
            session.snapshot().await,
            Err(DurableError::Poisoned)
        ));
        assert!(matches!(
            session.commit(batch(1)).await,
            Err(DurableError::Poisoned)
        ));
        let conversation = ConversationId::new(1).unwrap();
        assert!(matches!(
            session.entries(conversation, EntryQuery::default()).await,
            Err(DurableError::Poisoned)
        ));
        assert!(matches!(
            session.tasks(conversation, TaskQuery::default()).await,
            Err(DurableError::Poisoned)
        ));
        assert!(matches!(
            session
                .submissions(conversation, SubmissionQuery::default())
                .await,
            Err(DurableError::Poisoned)
        ));
        assert!(matches!(
            session.context_view(conversation, None).await,
            Err(DurableError::Poisoned)
        ));
        assert!(matches!(
            session.entry(conversation, EntryId::new(1).unwrap()).await,
            Err(DurableError::Poisoned)
        ));
        assert!(matches!(
            session.tasks_in(None, TaskQuery::default()).await,
            Err(DurableError::Poisoned)
        ));
        assert!(matches!(
            session.conversation(conversation).await,
            Err(DurableError::Poisoned)
        ));
        assert!(matches!(
            session.conversations(ConversationQuery::default()).await,
            Err(DurableError::Poisoned)
        ));
        assert!(matches!(
            session
                .document(
                    DocumentAddress {
                        conversation_id: conversation,
                        kind: "test".into(),
                        key: None
                    },
                    DocumentPoint::Current
                )
                .await,
            Err(DurableError::Poisoned)
        ));
        assert!(matches!(
            session
                .create_conversation(ConversationOwnership::Ownerless)
                .await,
            Err(DurableError::Poisoned)
        ));
        assert!(matches!(
            session
                .submissions_in(None, SubmissionQuery::default())
                .await,
            Err(DurableError::Poisoned)
        ));
        assert!(session.close().await.is_ok());
    }

    #[tokio::test]
    async fn paged_read_waits_for_admitted_commit_and_observes_settled_state() {
        let (storage, admitted, release, _) = BarrierStorage::new(false);
        let session = Arc::new(DurableSession::open(Box::new(storage)).await.unwrap());
        let writer = session.clone();
        let commit = tokio::spawn(async move {
            let mut batch = batch(1);
            batch.entries[0].value["text"] = json!("settled input");
            writer.commit(batch).await
        });
        admitted.notified().await;
        let reader = session.clone();
        let read = tokio::spawn(async move {
            reader
                .entries(ConversationId::new(1).unwrap(), EntryQuery::default())
                .await
        });
        let viewer = session.clone();
        let view_read = tokio::spawn(async move {
            viewer
                .context_view(ConversationId::new(1).unwrap(), None)
                .await
        });
        let lookup = session.clone();
        let entry_read = tokio::spawn(async move {
            lookup
                .entry(ConversationId::new(1).unwrap(), EntryId::new(1).unwrap())
                .await
        });
        tokio::task::yield_now().await;
        assert!(!read.is_finished());
        assert!(!view_read.is_finished());
        assert!(!entry_read.is_finished());
        release.notify_waiters();
        commit.await.unwrap().unwrap();
        let view = view_read.await.unwrap().unwrap();
        assert_eq!(view.entries.len(), 1);
        assert_eq!(view.contributions.len(), 1);
        assert_eq!(view.contributions[0].len(), 1);
        assert!(
            matches!(&view.messages[0].content[0], crate::types::ContentBlock::Text { text, .. } if text == "settled input")
        );
        let page = read.await.unwrap().unwrap();
        assert_eq!(entry_read.await.unwrap().unwrap().unwrap(), page.items[0]);
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].value["seq"], 1);
        session.close().await.unwrap();
    }

    #[tokio::test]
    async fn actual_journal_admission_drop_close_and_successor_fence() {
        let path =
            std::env::temp_dir().join(format!("rs-ai-session-journal-{}", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let storage = JournalStorage::open(&path).unwrap();
        let (reached, release) = storage.inject_commit_barrier();
        let session = Arc::new(DurableSession::open(Box::new(storage)).await.unwrap());

        let first_session = session.clone();
        let first = tokio::spawn(async move { first_session.commit(batch(1)).await });
        reached.await.unwrap();
        first.abort();
        let _ = first.await;

        let queued_session = session.clone();
        let queued = tokio::spawn(async move { queued_session.commit(batch(2)).await });
        let close_session = session.clone();
        let close = tokio::spawn(async move { close_session.close().await });
        tokio::task::yield_now().await;
        assert!(matches!(
            session.commit(batch(2)).await,
            Err(DurableError::Closed)
        ));
        release.send(()).unwrap();
        assert!(matches!(queued.await.unwrap(), Err(DurableError::Closed)));
        assert!(close.await.unwrap().is_ok());

        let reopened = JournalStorage::open(&path).unwrap();
        let claim = reopened.claim_writer().unwrap();
        assert_eq!(reopened.load(&claim).await.unwrap().entries.len(), 1);
        reopened.close(&claim).await.unwrap();
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn dropped_before_dequeue_is_tombstoned() {
        let (storage, admitted, release, commits) = BarrierStorage::new(false);
        let session = Arc::new(DurableSession::open(Box::new(storage)).await.unwrap());
        let first = session.clone();
        let task = tokio::spawn(async move { first.commit(batch(1)).await });
        admitted.notified().await;
        let second = session.clone();
        let queued = tokio::spawn(async move { second.commit(batch(2)).await });
        queued.abort();
        let _ = queued.await;
        release.notify_waiters();
        assert!(task.await.unwrap().is_ok());
        tokio::task::yield_now().await;
        assert_eq!(*commits.lock().unwrap(), 1);
        session.close().await.unwrap();
    }
}
