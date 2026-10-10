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

    fn root() -> ConversationId {
        ConversationId::new(1).unwrap()
    }
    fn token(version: u32) -> DocumentDefinition<Value> {
        DocumentDefinition::new(
            "custom.migrate",
            version,
            DocumentHistory::Rewindable,
            DocumentFork::AsOf,
            || Ok(json!({"count":1})),
        )
        .unwrap()
    }
    fn current() -> DocumentDefinition<Value> {
        token(3).with_migration(|mut value, version| {
            assert_eq!(version, 1);
            value["migrated"] = json!(true);
            Ok(value)
        })
    }
    async fn initialise(session: &DurableSession) -> GenericDocumentRecord {
        session
            .transact_entries(|tx| tx.edit_document(&token(1), root(), None, &(), |_| Ok(())))
            .await
            .unwrap();
        session
            .document(
                token(1).address(root(), None).unwrap(),
                DocumentPoint::Current,
            )
            .await
            .unwrap()
            .unwrap()
    }
    async fn frame(watch: &mut DocumentWatch) -> GenericDocumentRecord {
        let event = tokio::time::timeout(Duration::from_secs(2), watch.next())
            .await
            .unwrap();
        let Some(DocumentEvent::Replacement {
            record: Some(record),
            seq,
            reconciled: false,
        }) = event
        else {
            panic!("expected exact migrated replacement: {event:?}")
        };
        assert_eq!(seq, record.updated_seq);
        record
    }
    #[tokio::test]
    async fn migration_only_commits_keep_identity_and_historical_versions_then_coalesce_edits() {
        let session = DurableSession::open(Box::new(MemoryStorage::new()))
            .await
            .unwrap();
        let first = initialise(&session).await;
        let mut watch = session.watch_document(first.address.clone()).await.unwrap();
        session
            .transact_entries(|tx| tx.edit_document(&current(), root(), None, &(), |_| Ok(())))
            .await
            .unwrap();
        let migrated = frame(&mut watch).await;
        assert_eq!(migrated.id, first.id);
        assert_eq!(migrated.created_seq, first.created_seq);
        assert_eq!(migrated.version, 3);
        assert_eq!(migrated.value, json!({"count":1,"migrated":true}));
        let state = session.snapshot().await.unwrap();
        assert_eq!(state.next_id, 2);
        let past = session
            .document(first.address.clone(), DocumentPoint::At(first.created_seq))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(past, first);
        assert_eq!(
            session
                .typed_document(
                    &token(1),
                    root(),
                    None,
                    DocumentPoint::At(first.created_seq)
                )
                .await
                .unwrap(),
            Some(first.value.clone())
        );
        assert!(
            session
                .typed_document(&token(1), root(), None, DocumentPoint::Current)
                .await
                .is_err()
        );
        let mut query = DocumentQuery::current(root());
        query.point = DocumentPoint::At(first.created_seq);
        assert_eq!(
            session.documents(query).await.unwrap().items,
            vec![first.clone()]
        );
        session
            .transact_entries(|tx| tx.edit_document(&current(), root(), None, &(), |_| Ok(())))
            .await
            .unwrap();
        assert_eq!(
            session.snapshot().await.unwrap(),
            state,
            "already-upgraded no-op does not commit"
        );
        assert!(
            tokio::time::timeout(Duration::from_millis(10), watch.next())
                .await
                .is_err()
        );
        session
            .transact_entries(|tx| {
                tx.edit_document(&current(), root(), None, &(), |value| {
                    value["count"] = json!(7);
                    Ok(())
                })?;
                tx.edit_document(&current(), root(), None, &(), |value| {
                    value["count"] = json!(8);
                    Ok(())
                })
            })
            .await
            .unwrap();
        let updated = frame(&mut watch).await;
        assert_eq!(updated.version, 3);
        assert_eq!(updated.value["count"], 8);
        assert_eq!(
            session.snapshot().await.unwrap().document_revisions[&first.id].len(),
            3
        );
        watch.stop();
        session.close().await.unwrap();
    }
    #[tokio::test]
    async fn unchanged_migration_value_still_persists_version_and_publishes_once() {
        let session = DurableSession::open(Box::new(MemoryStorage::new()))
            .await
            .unwrap();
        let first = initialise(&session).await;
        let mut watch = session.watch_document(first.address.clone()).await.unwrap();
        let new = token(2).with_migration(|value, _| Ok(value));
        session
            .transact_entries(move |tx| tx.edit_document(&new, root(), None, &(), |_| Ok(())))
            .await
            .unwrap();
        let migrated = frame(&mut watch).await;
        assert_eq!(migrated.value, first.value);
        assert_eq!(migrated.version, 2);
        assert_eq!(migrated.id, first.id);
        assert_eq!(
            session
                .document(first.address, DocumentPoint::At(first.created_seq))
                .await
                .unwrap()
                .unwrap()
                .version,
            1
        );
        watch.stop();
        session.close().await.unwrap();
    }
    #[tokio::test]
    async fn failures_and_caught_errors_rollback_migration_edits_and_related_writes() {
        let session = DurableSession::open(Box::new(MemoryStorage::new()))
            .await
            .unwrap();
        let first = initialise(&session).await;
        let before = session.snapshot().await.unwrap();
        let mut watch = session.watch_document(first.address.clone()).await.unwrap();
        for mode in 0..6 {
            let definition = token(2).with_migration(move |value, _| match mode {
                0 => Err(DurableError::Rejected("migration error".into())),
                1 => panic!("migration panic"),
                2 => Ok(json!(null)),
                3 => Ok(json!({"text":"x".repeat(MAX_DOCUMENT_BYTES)})),
                _ => Ok(value),
            });
            let result: Result<(), DurableError> = session
                .transact_entries(move |tx| {
                    tx.append_entry(root(), EntryDraft::new("rollback-related"))?;
                    let result = tx.edit_document(&definition, root(), None, &(), |value| {
                        if mode == 4 {
                            panic!("edit panic")
                        };
                        if mode == 5 {
                            return Err(DurableError::Rejected("edit error".into()));
                        };
                        value["count"] = json!(9);
                        Ok(())
                    });
                    assert!(result.is_err());
                    Ok(())
                })
                .await;
            assert!(result.is_err());
            assert_eq!(session.snapshot().await.unwrap(), before);
        }
        let result: Result<(), DurableError> = session
            .transact_entries(|tx| {
                tx.edit_document(&current(), root(), None, &(), |value| {
                    value["count"] = json!(99);
                    Ok(())
                })?;
                Err(DurableError::Rejected("outer rollback".into()))
            })
            .await;
        assert!(result.is_err());
        assert_eq!(session.snapshot().await.unwrap(), before);
        assert!(
            tokio::time::timeout(Duration::from_millis(10), watch.next())
                .await
                .is_err()
        );
        session
            .transact_entries(|tx| tx.edit_document(&current(), root(), None, &(), |_| Ok(())))
            .await
            .unwrap();
        assert_eq!(frame(&mut watch).await.version, 3);
        watch.stop();
        session.close().await.unwrap();
    }
    #[tokio::test]
    async fn migrations_survive_legacy_journal_replay_forks_and_retirement() {
        let dir = std::env::temp_dir().join(format!("rs-ai-migration-{}", crate::utils::uuidv7()));
        std::fs::create_dir(&dir).unwrap();
        let path = dir.join("journal");
        let session = DurableSession::open(Box::new(JournalStorage::open(&path).unwrap()))
            .await
            .unwrap();
        let first = initialise(&session).await;
        let cutoff = session
            .append_entry(root(), EntryDraft::new("old-version-cutoff"))
            .await
            .unwrap();
        session.close().await.unwrap();
        // These original frames have the old whole-record schema and version 1.
        let session = DurableSession::open(Box::new(JournalStorage::open(&path).unwrap()))
            .await
            .unwrap();
        session
            .transact_entries(|tx| {
                tx.edit_document(&current(), root(), None, &(), |value| {
                    value["count"] = json!(4);
                    Ok(())
                })
            })
            .await
            .unwrap();
        let child = session
            .fork_conversation(root(), cutoff.id, ConversationOwnership::Ownerless)
            .await
            .unwrap();
        let copy = session
            .document(
                token(1).address(child.id, None).unwrap(),
                DocumentPoint::Current,
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(copy.version, 1);
        assert_eq!(copy.value, first.value);
        let child_id = child.id;
        session
            .transact_entries(move |tx| {
                tx.edit_document(&current(), child_id, None, &(), |value| {
                    value["count"] = json!(5);
                    Ok(())
                })
            })
            .await
            .unwrap();
        session
            .transact_entries(|tx| {
                tx.edit_document(
                    &token(4).with_migration(|value, _| Ok(value)),
                    root(),
                    None,
                    &(),
                    |_| Ok(()),
                )?;
                tx.retire_document(&token(4).address(root(), None)?)
            })
            .await
            .unwrap();
        let state = session.snapshot().await.unwrap();
        assert_eq!(state.generic_documents[&first.id].version, 4);
        assert!(state.generic_documents[&first.id].retired_seq.is_some());
        session.close().await.unwrap();
        let session = DurableSession::open(Box::new(JournalStorage::open(&path).unwrap()))
            .await
            .unwrap();
        assert_eq!(session.snapshot().await.unwrap(), state);
        let mut historical = first.clone();
        historical.retired_seq = state.generic_documents[&first.id].retired_seq;
        assert_eq!(
            session
                .document(first.address.clone(), DocumentPoint::At(first.created_seq))
                .await
                .unwrap(),
            Some(historical)
        );
        assert_eq!(
            session
                .typed_document(&current(), child_id, None, DocumentPoint::Current)
                .await
                .unwrap()
                .unwrap()["count"],
            5
        );
        session.close().await.unwrap();
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[tokio::test]
    async fn raw_whole_bases_can_upgrade_but_downgrades_and_identity_changes_reject() {
        let session = DurableSession::open(Box::new(MemoryStorage::new()))
            .await
            .unwrap();
        let first = initialise(&session).await;
        let state = session.snapshot().await.unwrap();
        let mut raw = first.clone();
        raw.version = 3;
        raw.updated_seq = CommitSeq::new(state.next_seq).unwrap();
        raw.value = json!({"raw":true});
        session
            .commit(CommitBatch {
                seq: raw.updated_seq,
                next_id: state.next_id,
                next_seq: state.next_seq + 1,
                generic_documents: vec![raw],
                conversations: vec![],
                entries: vec![],
                tasks: vec![],
                submissions: vec![],
                documents: vec![],
            })
            .await
            .unwrap();
        let state = session.snapshot().await.unwrap();
        for mode in 0..6 {
            let mut raw = state.generic_documents[&first.id].clone();
            raw.updated_seq = CommitSeq::new(state.next_seq).unwrap();
            match mode {
                0 => raw.version = 2,
                1 => raw.version = 0,
                2 => raw.address.kind = "other".into(),
                3 => raw.created_seq = raw.updated_seq,
                4 => raw.history = DocumentHistory::Latest,
                _ => raw.fork = DocumentFork::Current,
            }
            assert!(
                session
                    .commit(CommitBatch {
                        seq: raw.updated_seq,
                        next_id: state.next_id,
                        next_seq: state.next_seq + 1,
                        generic_documents: vec![raw],
                        conversations: vec![],
                        entries: vec![],
                        tasks: vec![],
                        submissions: vec![],
                        documents: vec![]
                    })
                    .await
                    .is_err()
            );
            assert_eq!(session.snapshot().await.unwrap(), state);
        }
        assert!(
            session
                .transact_entries(|tx| tx.put_document(DocumentDraft {
                    address: token(1).address(root(), None)?,
                    version: 4,
                    history: DocumentHistory::Rewindable,
                    fork: DocumentFork::AsOf,
                    value: json!({})
                }))
                .await
                .is_err(),
            "untyped convenience updates cannot change version"
        );
        assert_eq!(session.snapshot().await.unwrap(), state);
        session.close().await.unwrap();
    }
    #[tokio::test]
    async fn multiple_token_upgrades_in_one_batch_keep_only_final_revision() {
        let session = DurableSession::open(Box::new(MemoryStorage::new()))
            .await
            .unwrap();
        let first = initialise(&session).await;
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let count = calls.clone();
        let v2 = token(2).with_migration(move |mut value, version| {
            assert_eq!(version, 1);
            count.fetch_add(1, Ordering::SeqCst);
            value["two"] = json!(true);
            Ok(value)
        });
        let v3 = token(3).with_migration(|mut value, version| {
            assert_eq!(version, 2);
            value["three"] = json!(true);
            Ok(value)
        });
        session
            .transact_entries(move |tx| {
                tx.edit_document(&v2, root(), None, &(), |_| Ok(()))?;
                tx.edit_document(&v2, root(), None, &(), |_| Ok(()))?;
                tx.edit_document(&v3, root(), None, &(), |value| {
                    value["count"] = json!(5);
                    Ok(())
                })
            })
            .await
            .unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        let state = session.snapshot().await.unwrap();
        let revisions = &state.document_revisions[&first.id];
        assert_eq!(revisions.len(), 2);
        let final_revision = revisions.last_key_value().unwrap().1;
        assert_eq!(final_revision.version, 3);
        assert_eq!(
            final_revision.value,
            json!({"count":5,"two":true,"three":true})
        );
        session.close().await.unwrap();
    }
    #[tokio::test]
    async fn latest_family_migrations_and_current_forks_keep_upgraded_versions() {
        let session = DurableSession::open(Box::new(MemoryStorage::new()))
            .await
            .unwrap();
        let old = DocumentDefinition::family(
            "custom.family.migration",
            1,
            DocumentHistory::Latest,
            DocumentFork::Current,
            |seed: &u32| Ok(json!({"seed":seed})),
        )
        .unwrap();
        let tx_token = old.clone();
        let cutoff = session
            .transact_entries(move |tx| {
                tx.edit_document(&tx_token, root(), Some("one"), &1, |_| Ok(()))?;
                tx.edit_document(&tx_token, root(), Some("two"), &2, |_| Ok(()))?;
                tx.append_entry(root(), EntryDraft::new("family-cut"))
            })
            .await
            .unwrap();
        let new = DocumentDefinition::family(
            "custom.family.migration",
            2,
            DocumentHistory::Latest,
            DocumentFork::Current,
            |seed: &u32| Ok(json!({"seed":seed})),
        )
        .unwrap()
        .with_migration(|mut value, _| {
            value["migrated"] = json!(true);
            Ok(value)
        });
        let tx_token = new.clone();
        session
            .transact_entries(move |tx| {
                tx.edit_document(&tx_token, root(), Some("one"), &99, |_| Ok(()))
            })
            .await
            .unwrap();
        let child = session
            .fork_conversation(root(), cutoff.id, ConversationOwnership::Ownerless)
            .await
            .unwrap();
        let copied = session
            .document(
                new.address(child.id, Some("one")).unwrap(),
                DocumentPoint::Current,
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(copied.version, 2);
        assert_eq!(copied.value, json!({"seed":1,"migrated":true}));
        let untouched = session
            .document(
                old.address(child.id, Some("two")).unwrap(),
                DocumentPoint::Current,
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(untouched.version, 1);
        assert!(
            session
                .snapshot()
                .await
                .unwrap()
                .document_revisions
                .is_empty(),
            "latest documents retain no historical values"
        );
        assert!(
            session
                .document(
                    new.address(root(), Some("one")).unwrap(),
                    DocumentPoint::At(cutoff.created_seq)
                )
                .await
                .is_err()
        );
        session.close().await.unwrap();
    }

    #[tokio::test]
    async fn repeated_persisted_migrations_profile_workload() {
        let session = DurableSession::open(Box::new(MemoryStorage::new()))
            .await
            .unwrap();
        session
            .transact_entries(|tx| {
                tx.edit_document(&token(1), root(), None, &(), |value| {
                    value["text"] = json!("x".repeat(4096));
                    Ok(())
                })
            })
            .await
            .unwrap();
        for version in 2..=65 {
            let definition = token(version).with_migration(move |mut value, previous| {
                assert_eq!(previous, version - 1);
                value["count"] = json!(version);
                Ok(value)
            });
            session
                .transact_entries(move |tx| {
                    tx.edit_document(&definition, root(), None, &(), |_| Ok(()))
                })
                .await
                .unwrap();
        }
        let state = session.snapshot().await.unwrap();
        let record = state.generic_documents.values().next().unwrap();
        assert_eq!(record.version, 65);
        assert_eq!(record.value["count"], 65);
        assert_eq!(state.document_revisions[&record.id].len(), 65);
        session.close().await.unwrap();
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
                    1 => {
                        self.admitted.notify_one();
                        self.release.notified().await;
                    }
                    2 => return Err(DurableError::Rejected("storage rejected".into())),
                    3 => return Err(DurableError::Uncertain("storage uncertain".into())),
                    _ => {}
                };
                self.memory.commit(claim, batch).await
            })
        }
    }
    #[tokio::test]
    async fn rejected_storage_keeps_version_and_uncertainty_poisons_without_publication() {
        let mode = Arc::new(AtomicU8::new(0));
        let session = DurableSession::open(Box::new(ControlledStorage {
            memory: MemoryStorage::new(),
            mode: mode.clone(),
            admitted: Arc::new(Notify::new()),
            release: Arc::new(Notify::new()),
        }))
        .await
        .unwrap();
        let first = initialise(&session).await;
        let before = session.snapshot().await.unwrap();
        let mut watch = session.watch_document(first.address).await.unwrap();
        mode.store(2, Ordering::SeqCst);
        assert!(
            session
                .transact_entries(|tx| tx.edit_document(&current(), root(), None, &(), |_| Ok(())))
                .await
                .is_err()
        );
        assert_eq!(session.snapshot().await.unwrap(), before);
        assert!(
            tokio::time::timeout(Duration::from_millis(10), watch.next())
                .await
                .is_err()
        );
        mode.store(3, Ordering::SeqCst);
        assert!(
            session
                .transact_entries(|tx| tx.edit_document(&current(), root(), None, &(), |_| Ok(())))
                .await
                .is_err()
        );
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(2), watch.next())
                .await
                .unwrap(),
            Some(DocumentEvent::End(DocumentWatchEnd::Poisoned))
        );
        session.close().await.unwrap();
    }
    #[tokio::test]
    async fn admitted_migration_survives_caller_drop_and_publishes_after_settlement() {
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
        let first = initialise(&session).await;
        let mut watch = session.watch_document(first.address).await.unwrap();
        mode.store(1, Ordering::SeqCst);
        let writer = {
            let session = session.clone();
            tokio::spawn(async move {
                session
                    .transact_entries(|tx| {
                        tx.edit_document(&current(), root(), None, &(), |_| Ok(()))
                    })
                    .await
            })
        };
        admitted.notified().await;
        assert_eq!(watch.value().unwrap().version, 1);
        assert!(
            tokio::time::timeout(Duration::from_millis(10), watch.next())
                .await
                .is_err()
        );
        writer.abort();
        assert!(writer.await.unwrap_err().is_cancelled());
        release.notify_one();
        assert_eq!(frame(&mut watch).await.version, 3);
        assert_eq!(
            session
                .snapshot()
                .await
                .unwrap()
                .generic_documents
                .values()
                .next()
                .unwrap()
                .version,
            3
        );
        watch.stop();
        session.close().await.unwrap();
    }
}
