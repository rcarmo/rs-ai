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
    fn root() -> ConversationId {
        ConversationId::new(1).unwrap()
    }
    fn address() -> DocumentAddress {
        DocumentAddress {
            conversation_id: root(),
            kind: "custom.lifecycle".into(),
            key: None,
        }
    }
    fn draft(value: i64) -> DocumentDraft {
        DocumentDraft {
            address: address(),
            version: 1,
            history: DocumentHistory::Rewindable,
            fork: DocumentFork::AsOf,
            value: json!({"n":value}),
        }
    }
    async fn setup() -> DurableSession {
        DurableSession::open(Box::new(MemoryStorage::new()))
            .await
            .unwrap()
    }
    async fn put(session: &DurableSession, n: i64) -> GenericDocumentRecord {
        session
            .transact_entries(move |tx| tx.put_document(draft(n)))
            .await
            .unwrap()
    }
    async fn next(watch: &mut DocumentWatch) -> Option<DocumentEvent> {
        tokio::time::timeout(Duration::from_secs(2), watch.next())
            .await
            .unwrap()
    }
    #[tokio::test]
    async fn retirement_recreation_is_atomic_preserves_old_final_content_and_ends_old_watch() {
        let session = setup().await;
        let original = put(&session, 1).await;
        let mut watch = session.watch_document(address()).await.unwrap();
        let mut commits = session.watch().await.unwrap();
        assert!(matches!(
            commits.next().await,
            Some(DurableEvent::Snapshot(_))
        ));
        let replacement = session
            .transact_entries(|tx| {
                tx.put_document(draft(2))?;
                assert!(tx.retire_document(&address())?);
                assert!(tx.document(&address(), DocumentPoint::Current)?.is_none());
                assert!(!tx.retire_document(&address())?);
                let replacement = tx.put_document(draft(3))?;
                assert_eq!(
                    tx.document(&address(), DocumentPoint::Current)?.unwrap(),
                    replacement
                );
                tx.put_document(draft(4))
            })
            .await
            .unwrap();
        assert_ne!(original.id, replacement.id);
        let state = session.snapshot().await.unwrap();
        let retired = &state.generic_documents[&original.id];
        assert_eq!(retired.value, json!({"n":2}));
        assert_eq!(retired.retired_seq, Some(replacement.created_seq));
        assert_eq!(state.generic_documents.len(), 2);
        assert_eq!(
            session
                .document(address(), DocumentPoint::Current)
                .await
                .unwrap(),
            Some(replacement.clone())
        );
        assert_eq!(
            session
                .document(address(), DocumentPoint::At(original.created_seq))
                .await
                .unwrap()
                .unwrap()
                .value,
            original.value
        );
        assert_eq!(
            session
                .document(address(), DocumentPoint::At(replacement.created_seq))
                .await
                .unwrap(),
            Some(replacement.clone())
        );
        assert_eq!(
            next(&mut watch).await,
            Some(DocumentEvent::Replacement {
                record: None,
                seq: replacement.created_seq,
                reconciled: false
            })
        );
        assert_eq!(
            next(&mut watch).await,
            Some(DocumentEvent::End(DocumentWatchEnd::Retired))
        );
        assert_eq!(next(&mut watch).await, None);
        let Some(DurableEvent::Commit(batch)) = commits.next().await else {
            panic!("missing lifecycle commit")
        };
        assert_eq!(batch.generic_documents.len(), 2);
        assert_eq!(batch.generic_documents[0].id, original.id);
        assert_eq!(batch.generic_documents[1].id, replacement.id);
        let fresh = session.watch_document(address()).await.unwrap();
        assert_eq!(fresh.id(), replacement.id);
        drop(fresh);
        session.close().await.unwrap();
    }
    #[tokio::test]
    async fn new_empty_lifetimes_are_persisted_but_never_visible_and_multiple_cycles_use_latest() {
        let session = setup().await;
        let records = session
            .transact_entries(|tx| {
                let one = tx.put_document(draft(1))?;
                assert!(tx.retire_document(&address())?);
                assert!(tx.document(&address(), DocumentPoint::Current)?.is_none());
                let two = tx.put_document(draft(2))?;
                assert_eq!(
                    tx.document(&address(), DocumentPoint::Current)?.unwrap().id,
                    two.id
                );
                tx.put_document(draft(3))?;
                assert!(tx.retire_document(&address())?);
                let three = tx.put_document(draft(4))?;
                Ok((one, two, three))
            })
            .await
            .unwrap();
        let state = session.snapshot().await.unwrap();
        assert_eq!(state.next_id, 4);
        assert_eq!(state.generic_documents.len(), 3);
        for record in [&records.0, &records.1] {
            let retired = &state.generic_documents[&record.id];
            assert_eq!(retired.created_seq, retired.retired_seq.unwrap());
        }
        assert_eq!(state.generic_documents[&records.1.id].value, json!({"n":3}));
        let mut query = DocumentQuery::current(root());
        query.point = DocumentPoint::At(records.2.created_seq);
        assert_eq!(
            session.documents(query).await.unwrap().items,
            vec![records.2.clone()]
        );
        session
            .transact_entries(|tx| tx.retire_document(&address()))
            .await
            .unwrap();
        assert!(
            session
                .document(address(), DocumentPoint::Current)
                .await
                .unwrap()
                .is_none()
        );
        session.close().await.unwrap();
    }
    #[tokio::test]
    async fn creating_then_retiring_only_document_records_empty_lifetime_and_absent_retirement_is_noop()
     {
        let session = setup().await;
        let first = session
            .transact_entries(|tx| {
                assert!(!tx.retire_document(&address())?);
                let first = tx.put_document(draft(1))?;
                assert!(tx.retire_document(&address())?);
                Ok(first)
            })
            .await
            .unwrap();
        let state = session.snapshot().await.unwrap();
        assert_eq!(
            state.generic_documents[&first.id].retired_seq,
            Some(first.created_seq)
        );
        assert_eq!(state.document_revisions[&first.id].len(), 1);
        for point in [DocumentPoint::Current, DocumentPoint::At(first.created_seq)] {
            assert!(session.document(address(), point).await.unwrap().is_none());
        }
        assert!(
            !session
                .transact_entries(|tx| tx.retire_document(&address()))
                .await
                .unwrap()
        );
        assert_eq!(session.snapshot().await.unwrap(), state);
        let replacement = put(&session, 2).await;
        assert_ne!(first.id, replacement.id);
        session.close().await.unwrap();
    }
    #[tokio::test]
    async fn typed_family_reinitialises_with_new_seed_and_can_change_policies_only_after_retirement()
     {
        let session = setup().await;
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let count = calls.clone();
        let token = DocumentDefinition::family(
            "custom.lifecycle",
            1,
            DocumentHistory::Rewindable,
            DocumentFork::AsOf,
            move |seed: &i64| {
                count.fetch_add(1, Ordering::SeqCst);
                Ok(json!({"seed":seed}))
            },
        )
        .unwrap();
        let tx_token = token.clone();
        session
            .transact_entries(move |tx| {
                tx.edit_document(&tx_token, root(), Some("key"), &1, |_| Ok(()))
            })
            .await
            .unwrap();
        let tx_token = token.clone();
        session
            .transact_entries(move |tx| {
                tx.edit_document(&tx_token, root(), Some("key"), &99, |value| {
                    assert_eq!(value["seed"], 1);
                    Ok(())
                })?;
                tx.retire_document(&tx_token.address(root(), Some("key"))?)?;
                tx.edit_document(&tx_token, root(), Some("key"), &2, |value| {
                    assert_eq!(value["seed"], 2);
                    Ok(())
                })?;
                tx.edit_document(&tx_token, root(), Some("key"), &3, |value| {
                    assert_eq!(value["seed"], 2);
                    Ok(())
                })
            })
            .await
            .unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        let changed = DocumentDefinition::<Value, i64>::family(
            "custom.lifecycle",
            2,
            DocumentHistory::Latest,
            DocumentFork::Initial,
            |seed| Ok(json!({"seed":seed})),
        )
        .unwrap();
        let tx_token = changed.clone();
        assert!(
            session
                .transact_entries(move |tx| tx.edit_document(
                    &tx_token,
                    root(),
                    Some("key"),
                    &4,
                    |_| Ok(())
                ))
                .await
                .is_err()
        );
        let tx_token = changed.clone();
        session
            .transact_entries(move |tx| {
                tx.retire_document(&tx_token.address(root(), Some("key"))?)?;
                tx.edit_document(&tx_token, root(), Some("key"), &4, |_| Ok(()))
            })
            .await
            .unwrap();
        assert_eq!(
            session
                .typed_document(&changed, root(), Some("key"), DocumentPoint::Current)
                .await
                .unwrap(),
            Some(json!({"seed":4}))
        );
        session.close().await.unwrap();
    }
    #[tokio::test]
    async fn lifecycle_rollback_and_caught_validation_errors_never_partially_adopt() {
        let session = setup().await;
        put(&session, 1).await;
        let before = session.snapshot().await.unwrap();
        let mut watch = session.watch_document(address()).await.unwrap();
        for mode in 0..3 {
            let result: Result<(), DurableError> = session
                .transact_entries(move |tx| {
                    tx.retire_document(&address())?;
                    tx.put_document(draft(2))?;
                    tx.append_entry(root(), EntryDraft::new("rollback"))?;
                    match mode {
                        0 => Err(DurableError::Rejected("callback failed".into())),
                        1 => panic!("callback panic"),
                        _ => {
                            let mut invalid = draft(3);
                            invalid.value = json!(null);
                            assert!(tx.put_document(invalid).is_err());
                            Ok(())
                        }
                    }
                })
                .await;
            assert!(result.is_err());
            assert_eq!(session.snapshot().await.unwrap(), before);
        }
        assert!(
            tokio::time::timeout(Duration::from_millis(10), watch.next())
                .await
                .is_err()
        );
        watch.stop();
        session.close().await.unwrap();
    }
    #[tokio::test]
    async fn lifecycle_record_and_byte_limits_reject_whole_transaction() {
        let session = setup().await;
        let before = session.snapshot().await.unwrap();
        for byte_limit in [false, true] {
            let result: Result<(), DurableError> = session
                .transact_entries(move |tx| {
                    let count = if byte_limit { 9 } else { 4097 };
                    for n in 0..count {
                        let mut value = draft(n);
                        if byte_limit {
                            value.value = json!({"text":"x".repeat(MAX_DOCUMENT_BYTES-32)});
                        }
                        tx.put_document(value)?;
                        tx.retire_document(&address())?;
                    }
                    Ok(())
                })
                .await;
            assert!(result.is_err());
            assert_eq!(session.snapshot().await.unwrap(), before);
        }
        session.close().await.unwrap();
    }
    #[tokio::test]
    async fn admitted_retirement_recreation_survives_caller_cancellation() {
        let mode = Arc::new(AtomicU8::new(0));
        let admitted = Arc::new(tokio::sync::Notify::new());
        let release = Arc::new(tokio::sync::Notify::new());
        let session = Arc::new(
            DurableSession::open(Box::new(Controlled {
                memory: MemoryStorage::new(),
                mode: mode.clone(),
                gate: Some((admitted.clone(), release.clone())),
            }))
            .await
            .unwrap(),
        );
        let old = put(&session, 1).await;
        let mut watch = session.watch_document(address()).await.unwrap();
        mode.store(3, Ordering::SeqCst);
        let writer = {
            let session = session.clone();
            tokio::spawn(async move {
                session
                    .transact_entries(|tx| {
                        tx.retire_document(&address())?;
                        tx.put_document(draft(2))
                    })
                    .await
            })
        };
        admitted.notified().await;
        assert_eq!(watch.value().unwrap(), old);
        assert!(
            tokio::time::timeout(Duration::from_millis(10), watch.next())
                .await
                .is_err()
        );
        writer.abort();
        assert!(writer.await.unwrap_err().is_cancelled());
        release.notify_one();
        assert!(matches!(
            next(&mut watch).await,
            Some(DocumentEvent::Replacement { record: None, .. })
        ));
        assert_eq!(
            next(&mut watch).await,
            Some(DocumentEvent::End(DocumentWatchEnd::Retired))
        );
        let new = session
            .document(address(), DocumentPoint::Current)
            .await
            .unwrap()
            .unwrap();
        assert_ne!(new.id, old.id);
        assert_eq!(new.value, json!({"n":2}));
        session.close().await.unwrap();
    }

    fn raw_batch(
        state: &StorageSnapshot,
        records: Vec<GenericDocumentRecord>,
        next_id: u64,
    ) -> CommitBatch {
        CommitBatch {
            seq: CommitSeq::new(state.next_seq).unwrap(),
            next_id,
            next_seq: state.next_seq + 1,
            generic_documents: records,
            conversations: vec![],
            entries: vec![],
            tasks: vec![],
            submissions: vec![],
            documents: vec![],
        }
    }
    fn fresh(state: &StorageSnapshot, original: &GenericDocumentRecord) -> GenericDocumentRecord {
        let mut new = original.clone();
        new.id = DocumentId::new(state.next_id).unwrap();
        new.created_seq = CommitSeq::new(state.next_seq).unwrap();
        new.updated_seq = new.created_seq;
        new
    }
    #[tokio::test]
    async fn raw_batches_reject_multiple_live_owners_duplicate_ids_and_reordered_retirement() {
        let session = setup().await;
        let old = put(&session, 1).await;
        let state = session.snapshot().await.unwrap();
        let mut retired = old.clone();
        retired.updated_seq = CommitSeq::new(state.next_seq).unwrap();
        retired.retired_seq = Some(retired.updated_seq);
        let new = fresh(&state, &old);
        let mut another = new.clone();
        another.id = DocumentId::new(new.id.get() + 1).unwrap();
        for records in [
            vec![new.clone()],
            vec![new.clone(), retired.clone()],
            vec![retired.clone(), new.clone(), another],
            vec![retired.clone(), retired.clone(), new.clone()],
            vec![old.clone(), new.clone()],
        ] {
            let next_id = records
                .iter()
                .map(|record| record.id.get() + 1)
                .max()
                .unwrap()
                .max(state.next_id);
            assert!(
                session
                    .commit(raw_batch(&state, records, next_id))
                    .await
                    .is_err()
            );
            assert_eq!(session.snapshot().await.unwrap(), state);
        }
        session
            .commit(raw_batch(
                &state,
                vec![retired, new.clone()],
                state.next_id + 1,
            ))
            .await
            .unwrap();
        assert_eq!(
            session
                .document(address(), DocumentPoint::Current)
                .await
                .unwrap(),
            Some(new)
        );
        session.close().await.unwrap();
    }
    #[tokio::test]
    async fn journal_replay_and_as_of_forks_select_replacement_at_same_commit_cutoff() {
        let dir =
            std::env::temp_dir().join(format!("rs-ai-incarnation-{}", crate::utils::uuidv7()));
        std::fs::create_dir(&dir).unwrap();
        let path = dir.join("journal");
        let session = DurableSession::open(Box::new(JournalStorage::open(&path).unwrap()))
            .await
            .unwrap();
        let old = put(&session, 1).await;
        let old_cut = session
            .append_entry(root(), EntryDraft::new("old-cut"))
            .await
            .unwrap();
        let cutoff = session
            .transact_entries(|tx| {
                tx.retire_document(&address())?;
                let empty = tx.put_document(draft(2))?;
                tx.retire_document(&address())?;
                let new = tx.put_document(draft(3))?;
                let cutoff = tx.append_entry(root(), EntryDraft::new("replacement-cut"))?;
                Ok((empty, new, cutoff))
            })
            .await
            .unwrap();
        let child = session
            .fork_conversation(root(), cutoff.2.id, ConversationOwnership::Ownerless)
            .await
            .unwrap();
        assert_eq!(
            session
                .document(
                    DocumentAddress {
                        conversation_id: child.id,
                        ..address()
                    },
                    DocumentPoint::Current
                )
                .await
                .unwrap()
                .unwrap()
                .value,
            json!({"n":3})
        );
        let old_child = session
            .fork_conversation(root(), old_cut.id, ConversationOwnership::Ownerless)
            .await
            .unwrap();
        assert_eq!(
            session
                .document(
                    DocumentAddress {
                        conversation_id: old_child.id,
                        ..address()
                    },
                    DocumentPoint::Current
                )
                .await
                .unwrap()
                .unwrap()
                .value,
            old.value
        );
        let before = session.snapshot().await.unwrap();
        session.close().await.unwrap();
        let session = DurableSession::open(Box::new(JournalStorage::open(&path).unwrap()))
            .await
            .unwrap();
        assert_eq!(session.snapshot().await.unwrap(), before);
        assert_eq!(
            session
                .document(address(), DocumentPoint::At(cutoff.2.created_seq))
                .await
                .unwrap(),
            Some(cutoff.1)
        );
        assert_eq!(
            before.generic_documents[&cutoff.0.id].created_seq,
            before.generic_documents[&cutoff.0.id].retired_seq.unwrap()
        );
        session.close().await.unwrap();
        std::fs::remove_dir_all(dir).unwrap();
    }
    struct Controlled {
        memory: MemoryStorage,
        mode: Arc<AtomicU8>,
        gate: Option<(Arc<tokio::sync::Notify>, Arc<tokio::sync::Notify>)>,
    }
    impl DurableStorage for Controlled {
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
                match self.mode.load(Ordering::SeqCst) {
                    1 => return Err(DurableError::Rejected("rejected lifecycle".into())),
                    2 => return Err(DurableError::Uncertain("uncertain lifecycle".into())),
                    3 => {
                        let (admitted, release) = self.gate.as_ref().unwrap();
                        admitted.notify_one();
                        release.notified().await;
                    }
                    _ => {}
                };
                self.memory.commit(claim, batch).await
            })
        }
    }
    #[tokio::test]
    async fn storage_rejection_and_uncertainty_never_publish_partial_retirement_or_creation() {
        let mode = Arc::new(AtomicU8::new(0));
        let session = DurableSession::open(Box::new(Controlled {
            memory: MemoryStorage::new(),
            mode: mode.clone(),
            gate: None,
        }))
        .await
        .unwrap();
        put(&session, 1).await;
        let before = session.snapshot().await.unwrap();
        let mut watch = session.watch_document(address()).await.unwrap();
        mode.store(1, Ordering::SeqCst);
        assert!(
            session
                .transact_entries(|tx| {
                    tx.retire_document(&address())?;
                    tx.put_document(draft(2))
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
        mode.store(2, Ordering::SeqCst);
        assert!(
            session
                .transact_entries(|tx| {
                    tx.retire_document(&address())?;
                    tx.put_document(draft(2))
                })
                .await
                .is_err()
        );
        assert_eq!(
            next(&mut watch).await,
            Some(DocumentEvent::End(DocumentWatchEnd::Poisoned))
        );
        session.close().await.unwrap();
    }
}
