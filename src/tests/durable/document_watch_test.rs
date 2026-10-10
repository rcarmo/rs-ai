#[cfg(test)]
mod tests {
    use crate::durable::storage::{StorageFuture, WriterClaim};
    use crate::durable::*;
    use serde_json::{Value, json};
    use std::sync::{
        Arc,
        atomic::{AtomicU8, Ordering},
    };
    use std::time::Duration;
    use tokio::sync::Notify;

    fn address() -> DocumentAddress {
        DocumentAddress {
            conversation_id: ConversationId::new(1).unwrap(),
            kind: "custom.watch".into(),
            key: None,
        }
    }
    fn draft(value: Value) -> DocumentDraft {
        DocumentDraft {
            address: address(),
            version: 1,
            history: DocumentHistory::Latest,
            fork: DocumentFork::Current,
            value,
        }
    }
    async fn put(session: &DurableSession, value: Value) -> GenericDocumentRecord {
        session
            .transact_entries(move |tx| tx.put_document(draft(value)))
            .await
            .unwrap()
    }
    async fn next(watch: &mut DocumentWatch) -> Option<DocumentEvent> {
        tokio::time::timeout(Duration::from_secs(2), watch.next())
            .await
            .expect("watch delivery stalled")
    }
    fn replacement(
        event: Option<DocumentEvent>,
    ) -> (Option<GenericDocumentRecord>, CommitSeq, bool) {
        let Some(DocumentEvent::Replacement {
            record,
            seq,
            reconciled,
        }) = event
        else {
            panic!("expected replacement: {event:?}");
        };
        (record, seq, reconciled)
    }

    #[tokio::test]
    async fn acquisition_and_exact_frames_are_detached_and_only_adopted() {
        let session = DurableSession::open(Box::new(MemoryStorage::new()))
            .await
            .unwrap();
        assert!(matches!(
            session.watch_document(address()).await,
            Err(DurableError::Rejected(_))
        ));
        let initial = put(&session, json!({"n":0})).await;
        let mut watch = session.watch_document(address()).await.unwrap();
        assert_eq!(watch.id(), initial.id);
        let mut value = watch.value().unwrap();
        value.value = json!({"changed":true});
        assert_eq!(watch.value().unwrap(), initial);
        assert!(
            tokio::time::timeout(Duration::from_millis(10), watch.next())
                .await
                .is_err(),
            "acquisition is not a callback frame"
        );
        let first = put(&session, json!({"n":1})).await;
        let second = put(&session, json!({"n":2})).await;
        assert_eq!(
            watch.value().unwrap(),
            initial,
            "undelivered commits do not advance value"
        );
        let (mut delivered, seq, reconciled) = replacement(next(&mut watch).await);
        assert_eq!(delivered.as_ref(), Some(&first));
        assert_eq!(seq, first.updated_seq);
        assert!(!reconciled);
        delivered.as_mut().unwrap().value = json!({"mutation":true});
        assert_eq!(watch.value().unwrap(), first);
        assert_eq!(replacement(next(&mut watch).await).0, Some(second));
        session
            .append_entry(
                ConversationId::new(1).unwrap(),
                EntryDraft::new("unrelated"),
            )
            .await
            .unwrap();
        assert!(
            session
                .transact_entries(|tx| tx.put_document(draft(json!(null))))
                .await
                .is_err()
        );
        let failed: Result<(), DurableError> = session
            .transact_entries(|tx| {
                tx.put_document(draft(json!({"rollback":true})))?;
                Err(DurableError::Rejected("rollback".into()))
            })
            .await;
        assert!(failed.is_err());
        assert!(
            tokio::time::timeout(Duration::from_millis(10), watch.next())
                .await
                .is_err()
        );
        session.close().await.unwrap();
        assert_eq!(
            next(&mut watch).await,
            Some(DocumentEvent::End(DocumentWatchEnd::Closed))
        );
        assert_eq!(next(&mut watch).await, None);
        assert!(matches!(
            session.watch_document(address()).await,
            Err(DurableError::Closed)
        ));
    }

    #[tokio::test]
    async fn one_frame_per_commit_uses_final_staged_value() {
        let session = DurableSession::open(Box::new(MemoryStorage::new()))
            .await
            .unwrap();
        put(&session, json!({"n":0})).await;
        let mut watch = session.watch_document(address()).await.unwrap();
        let final_record = session
            .transact_entries(|tx| {
                tx.put_document(draft(json!({"n":1})))?;
                tx.put_document(draft(json!({"n":2})))
            })
            .await
            .unwrap();
        assert_eq!(replacement(next(&mut watch).await).0, Some(final_record));
        assert!(
            tokio::time::timeout(Duration::from_millis(10), watch.next())
                .await
                .is_err()
        );
        watch.stop();
        session.close().await.unwrap();
    }

    #[tokio::test]
    async fn retirement_null_frame_ends_original_incarnation_not_recreated_address() {
        let session = DurableSession::open(Box::new(MemoryStorage::new()))
            .await
            .unwrap();
        let original = put(&session, json!({"nullable":null})).await;
        let mut watch = session.watch_document(address()).await.unwrap();
        let updated = put(&session, json!({"nullable":null,"n":1})).await;
        session
            .transact_entries(|tx| tx.retire_document(&address()))
            .await
            .unwrap();
        let retired = session.snapshot().await.unwrap().last_seq.unwrap();
        let recreated = put(&session, json!({"n":2})).await;
        assert_ne!(original.id, recreated.id);
        assert_eq!(replacement(next(&mut watch).await).0, Some(updated));
        assert_eq!(replacement(next(&mut watch).await), (None, retired, false));
        assert_eq!(watch.value(), None);
        assert_eq!(
            next(&mut watch).await,
            Some(DocumentEvent::End(DocumentWatchEnd::Retired))
        );
        watch.stop();
        assert_eq!(next(&mut watch).await, None);
        let mut newer = session.watch_document(address()).await.unwrap();
        assert_eq!(newer.value(), Some(recreated));
        session.close().await.unwrap();
        assert_eq!(
            next(&mut newer).await,
            Some(DocumentEvent::End(DocumentWatchEnd::Closed))
        );
    }

    #[tokio::test]
    async fn hundred_pending_frames_overflow_to_exact_replacement_and_retirement() {
        let session = DurableSession::open(Box::new(MemoryStorage::new()))
            .await
            .unwrap();
        put(&session, json!({"n":0})).await;
        let mut watch = session.watch_document(address()).await.unwrap();
        for n in 1..=205 {
            put(&session, json!({"n":n})).await;
        }
        let (record, seq, reconciled) = replacement(next(&mut watch).await);
        assert_eq!(record.unwrap().value, json!({"n":201}));
        assert_eq!(seq.get(), 202);
        assert!(reconciled);
        for n in 202..=205 {
            let (record, _, reconciled) = replacement(next(&mut watch).await);
            assert_eq!(record.unwrap().value, json!({"n":n}));
            assert!(!reconciled);
        }
        for n in 1..=100 {
            put(&session, json!({"n":n})).await;
        }
        session
            .transact_entries(|tx| tx.retire_document(&address()))
            .await
            .unwrap();
        let (record, _, reconciled) = replacement(next(&mut watch).await);
        assert!(record.is_none());
        assert!(reconciled);
        assert_eq!(
            next(&mut watch).await,
            Some(DocumentEvent::End(DocumentWatchEnd::Retired))
        );
        session.close().await.unwrap();
    }

    #[tokio::test]
    async fn stop_close_and_drop_discard_pending_frames_and_reclaim_slots() {
        let session = DurableSession::open(Box::new(MemoryStorage::new()))
            .await
            .unwrap();
        put(&session, json!({})).await;
        let mut watches = Vec::new();
        for _ in 0..64 {
            watches.push(session.watch_document(address()).await.unwrap());
        }
        assert!(matches!(
            session.watch_document(address()).await,
            Err(DurableError::Rejected(_))
        ));
        put(&session, json!({"queued":true})).await;
        watches[0].stop();
        watches[0].stop();
        assert_eq!(
            next(&mut watches[0]).await,
            Some(DocumentEvent::End(DocumentWatchEnd::Stopped))
        );
        assert_eq!(next(&mut watches[0]).await, None);
        drop(session.watch_document(address()).await.unwrap());
        drop(watches);
        let mut stopped = session.watch_document(address()).await.unwrap();
        session
            .transact_entries(|tx| tx.retire_document(&address()))
            .await
            .unwrap();
        stopped.stop();
        assert_eq!(
            next(&mut stopped).await,
            Some(DocumentEvent::End(DocumentWatchEnd::Stopped)),
            "retirement does not win until delivered"
        );
        put(&session, json!({"recreated":true})).await;
        let mut closed = session.watch_document(address()).await.unwrap();
        session
            .transact_entries(|tx| tx.retire_document(&address()))
            .await
            .unwrap();
        session.close().await.unwrap();
        assert_eq!(
            next(&mut closed).await,
            Some(DocumentEvent::End(DocumentWatchEnd::Closed))
        );
    }

    #[tokio::test]
    async fn byte_budget_reconciles_and_bounds_escaped_serialization() {
        let session = DurableSession::open(Box::new(MemoryStorage::new()))
            .await
            .unwrap();
        let mut record = put(&session, json!({"text":"quotes\"\n\\ unicode 𐐀"})).await;
        for value in [
            json!({"text":"quotes\"\n\\ unicode 𐐀"}),
            json!({"array":[null,true,false,-1,1e300,{"control":"\u{0001}"}],"empty":{}}),
            json!({"large":"x".repeat(128*1024)}),
        ] {
            record.value = value;
            assert!(
                crate::durable::document_watch::record_size(&record)
                    >= serde_json::to_vec(&record).unwrap().len()
            );
        }
        let queue = crate::durable::document_watch::DocumentQueue::new(record.clone());
        let mut watch = DocumentWatch {
            queue: queue.clone(),
        };
        for seq in 2..=4 {
            record.updated_seq = CommitSeq::new(seq).unwrap();
            queue.publish(Arc::new(record.clone()), MAX_COMMIT_BYTES / 2);
        }
        assert_eq!(
            replacement(next(&mut watch).await),
            (Some(record), CommitSeq::new(4).unwrap(), true)
        );
        assert!(
            tokio::time::timeout(Duration::from_millis(10), watch.next())
                .await
                .is_err()
        );
        watch.stop();
        session.close().await.unwrap();
    }

    #[tokio::test]
    async fn large_document_budget_reconciles_independently_for_each_reader() {
        let session = DurableSession::open(Box::new(MemoryStorage::new()))
            .await
            .unwrap();
        put(&session, json!({"n":0})).await;
        let mut slow = session.watch_document(address()).await.unwrap();
        let mut fast = session.watch_document(address()).await.unwrap();
        let mut final_record = None;
        for n in 1..=3 {
            let record = put(&session, json!({"n":n,"text":"x".repeat(2*1024*1024)})).await;
            let (delivered, _, reconciled) = replacement(next(&mut fast).await);
            assert_eq!(delivered, Some(record.clone()));
            assert!(!reconciled);
            final_record = Some(record);
        }
        let (mut delivered, seq, reconciled) = replacement(next(&mut slow).await);
        assert_eq!(delivered, final_record);
        assert_eq!(seq, final_record.as_ref().unwrap().updated_seq);
        assert!(reconciled);
        delivered.as_mut().unwrap().value = json!({"changed":true});
        assert_eq!(fast.value(), final_record);
        assert_eq!(
            session
                .document(address(), DocumentPoint::Current)
                .await
                .unwrap(),
            final_record
        );
        fast.stop();
        slow.stop();
        session.close().await.unwrap();
    }

    #[tokio::test]
    async fn session_worker_unwind_terminates_document_reader() {
        let session = DurableSession::open_with_clock(
            Box::new(MemoryStorage::new()),
            Arc::new(|| panic!("clock failure")),
        )
        .await
        .unwrap();
        put(&session, json!({})).await;
        let mut watch = session.watch_document(address()).await.unwrap();
        let state = session.snapshot().await.unwrap();
        let seq = CommitSeq::new(state.next_seq).unwrap();
        let task = TaskRecord {
            id: TaskId::new(state.next_id).unwrap(),
            conversation_id: ConversationId::new(1).unwrap(),
            kind: "generation".into(),
            version: 1,
            owner_task_id: None,
            state: TaskState::Running,
            input: json!({}),
            checkpoint: json!({}),
            outcome: None,
            abort_requested: false,
            started_at: None,
            ended_at: None,
            updated_seq: seq,
        };
        assert!(
            session
                .commit(CommitBatch {
                    seq,
                    next_id: state.next_id + 1,
                    next_seq: state.next_seq + 1,
                    tasks: vec![task],
                    entries: vec![],
                    conversations: vec![],
                    generic_documents: vec![],
                    submissions: vec![],
                    documents: vec![]
                })
                .await
                .is_err()
        );
        assert_eq!(
            next(&mut watch).await,
            Some(DocumentEvent::End(DocumentWatchEnd::Poisoned))
        );
        assert_eq!(next(&mut watch).await, None);
        assert!(session.close().await.is_err());
    }

    struct ControlledStorage {
        memory: MemoryStorage,
        mode: Arc<AtomicU8>,
        admitted: Arc<Notify>,
        release: Arc<Notify>,
    }
    impl DurableStorage for ControlledStorage {
        fn claim_writer(&self) -> Result<WriterClaim, DurableError> {
            self.memory.claim_writer()
        }
        fn load<'a>(&'a self, claim: &'a WriterClaim) -> StorageFuture<'a, StorageSnapshot> {
            Box::pin(async move {
                if self.mode.load(Ordering::SeqCst) == 4 {
                    return Err(DurableError::Io("post-commit reload failed".into()));
                }
                self.memory.load(claim).await
            })
        }
        fn commit<'a>(
            &'a self,
            claim: &'a WriterClaim,
            batch: CommitBatch,
        ) -> StorageFuture<'a, ()> {
            Box::pin(async move {
                let mode = self.mode.load(Ordering::SeqCst);
                if mode == 1 {
                    self.admitted.notify_one();
                    self.release.notified().await;
                }
                if mode == 2 {
                    return Err(DurableError::Uncertain("injected".into()));
                }
                if mode == 3 {
                    panic!("storage worker panic");
                }
                self.memory.commit(claim, batch).await?;
                if mode == 5 {
                    self.mode.store(4, Ordering::SeqCst);
                }
                Ok(())
            })
        }
        fn close<'a>(&'a self, claim: &'a WriterClaim) -> StorageFuture<'a, ()> {
            self.memory.close(claim)
        }
    }
    async fn controlled() -> (Arc<DurableSession>, Arc<AtomicU8>, Arc<Notify>, Arc<Notify>) {
        let mode = Arc::new(AtomicU8::new(0));
        let admitted = Arc::new(Notify::new());
        let release = Arc::new(Notify::new());
        let session = Arc::new(
            DurableSession::open(Box::new(ControlledStorage {
                memory: MemoryStorage::new(),
                mode: mode.clone(),
                admitted: admitted.clone(),
                release: release.clone(),
            }))
            .await
            .unwrap(),
        );
        put(&session, json!({"n":0})).await;
        (session, mode, admitted, release)
    }
    #[tokio::test]
    async fn atomic_attach_waits_for_admitted_commit_and_cancelled_calls_do_not_leak_slots() {
        let (session, mode, admitted, release) = controlled().await;
        mode.store(1, Ordering::SeqCst);
        let writer = {
            let session = session.clone();
            tokio::spawn(async move { put(&session, json!({"n":1})).await })
        };
        admitted.notified().await;
        let acquisition = {
            let session = session.clone();
            tokio::spawn(async move { session.watch_document(address()).await })
        };
        // Poll a queued acquisition to submission then cancel its receiver.
        let mut cancelled = Box::pin(session.watch_document(address()));
        assert!(
            tokio::time::timeout(Duration::from_millis(10), &mut cancelled)
                .await
                .is_err()
        );
        drop(cancelled);
        writer.abort();
        assert!(writer.await.unwrap_err().is_cancelled());
        release.notify_one();
        mode.store(0, Ordering::SeqCst);
        let mut watch = acquisition.await.unwrap().unwrap();
        assert_eq!(watch.value().unwrap().value, json!({"n":1}));
        let second = put(&session, json!({"n":2})).await;
        assert_eq!(replacement(next(&mut watch).await).0, Some(second));
        let mut others = Vec::new();
        for _ in 0..63 {
            others.push(session.watch_document(address()).await.unwrap());
        }
        assert!(session.watch_document(address()).await.is_err());
        session.close().await.unwrap();
    }
    #[tokio::test]
    async fn uncertain_panic_and_failed_adoption_end_both_watch_kinds_without_false_frames() {
        for fault in [2, 3, 5] {
            let (session, mode, _, _) = controlled().await;
            let mut watch = session.watch_document(address()).await.unwrap();
            let mut committed = session.watch().await.unwrap();
            assert!(matches!(
                committed.next().await,
                Some(DurableEvent::Snapshot(_))
            ));
            mode.store(fault, Ordering::SeqCst);
            assert!(
                session
                    .transact_entries(|tx| tx.put_document(draft(json!({"unadopted":true}))))
                    .await
                    .is_err()
            );
            assert_eq!(
                next(&mut watch).await,
                Some(DocumentEvent::End(DocumentWatchEnd::Poisoned))
            );
            assert_eq!(next(&mut watch).await, None);
            assert_eq!(
                tokio::time::timeout(Duration::from_secs(2), committed.next())
                    .await
                    .unwrap(),
                Some(DurableEvent::End(WatchEnd::Poisoned))
            );
            assert!(matches!(
                session.watch_document(address()).await,
                Err(DurableError::Poisoned)
            ));
            let closed = tokio::time::timeout(Duration::from_secs(2), session.close())
                .await
                .unwrap();
            if fault == 3 {
                assert!(closed.is_err());
            } else {
                closed.unwrap();
            }
        }
    }
}
