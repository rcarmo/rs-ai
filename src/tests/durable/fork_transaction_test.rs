#[cfg(test)]
mod tests {
    use crate::durable::storage::{StorageFuture, WriterClaim};
    use crate::durable::*;
    use serde_json::{Value, json};
    use std::sync::{
        Arc,
        atomic::{AtomicU8, AtomicUsize, Ordering},
    };
    use std::time::Duration;
    use tokio::sync::Notify;
    fn root() -> ConversationId {
        ConversationId::new(1).unwrap()
    }
    fn address(owner: ConversationId, kind: &str) -> DocumentAddress {
        DocumentAddress {
            scope: DocumentScope::Conversation {
                conversation_id: owner,
            },
            kind: kind.into(),
            key: None,
        }
    }
    fn draft(owner: ConversationId, kind: &str, fork: DocumentFork, n: i64) -> DocumentDraft {
        DocumentDraft {
            address: address(owner, kind),
            version: 1,
            history: DocumentHistory::Rewindable,
            fork,
            value: json!({"n":n}),
        }
    }
    fn definition(kind: &str, version: u32, fork: DocumentFork) -> DocumentDefinition<Value> {
        DocumentDefinition::new(kind, version, DocumentHistory::Rewindable, fork, || {
            Ok(json!({"n":0}))
        })
        .unwrap()
    }
    async fn seed(session: &DurableSession) -> EntryRecord {
        session
            .transact_entries(|tx| {
                tx.put_document(draft(root(), "asof", DocumentFork::AsOf, 1))?;
                tx.put_document(draft(root(), "current", DocumentFork::Current, 2))?;
                tx.put_document(draft(root(), "initial", DocumentFork::Initial, 3))?;
                tx.append_entry(root(), EntryDraft::new("cut"))
            })
            .await
            .unwrap()
    }
    async fn setup() -> (DurableSession, EntryRecord) {
        let session = DurableSession::open(Box::new(MemoryStorage::new()))
            .await
            .unwrap();
        let cutoff = seed(&session).await;
        (session, cutoff)
    }
    #[tokio::test]
    async fn source_writes_before_or_after_fork_reject_all_staging_and_publish_nothing() {
        let (session, cutoff) = setup().await;
        let before = session.snapshot().await.unwrap();
        let mut watch = session.watch().await.unwrap();
        assert!(matches!(
            watch.next().await,
            Some(DurableEvent::Snapshot(_))
        ));
        for (kind, fork) in [
            ("asof", DocumentFork::AsOf),
            ("current", DocumentFork::Current),
        ] {
            for before_fork in [false, true] {
                let at = cutoff.id;
                assert!(
                    session
                        .transact_entries(move |tx| {
                            if before_fork {
                                tx.put_document(draft(root(), kind, fork, 99))?;
                            }
                            tx.fork_conversation(root(), at, ConversationOwnership::Ownerless)?;
                            if !before_fork {
                                tx.put_document(draft(root(), kind, fork, 99))?;
                            }
                            tx.append_entry(root(), EntryDraft::new("rolled-back"))?;
                            Ok(())
                        })
                        .await
                        .is_err()
                );
                assert_eq!(session.snapshot().await.unwrap(), before);
            }
        }
        assert!(
            tokio::time::timeout(Duration::from_millis(10), watch.next())
                .await
                .is_err()
        );
        let child = session
            .fork_conversation(root(), cutoff.id, ConversationOwnership::Ownerless)
            .await
            .unwrap();
        assert_eq!(
            session
                .document(address(child.id, "current"), DocumentPoint::Current)
                .await
                .unwrap()
                .unwrap()
                .value,
            json!({"n":2})
        );
        watch.stop();
        session.close().await.unwrap();
    }
    #[tokio::test]
    async fn retirement_recreation_and_version_only_migration_of_selected_sources_reject() {
        let (session, cutoff) = setup().await;
        let before = session.snapshot().await.unwrap();
        for mode in 0..3 {
            let at = cutoff.id;
            assert!(
                session
                    .transact_entries(move |tx| {
                        tx.fork_conversation(root(), at, ConversationOwnership::Ownerless)?;
                        match mode {
                            0 => {
                                tx.retire_document(&address(root(), "asof"))?;
                            }
                            1 => {
                                tx.retire_document(&address(root(), "current"))?;
                                tx.put_document(draft(
                                    root(),
                                    "current",
                                    DocumentFork::Current,
                                    9,
                                ))?;
                            }
                            _ => {
                                let new = definition("asof", 2, DocumentFork::AsOf)
                                    .with_migration(|value, _| Ok(value));
                                tx.edit_document(&new, root(), None, &(), |_| Ok(()))?;
                            }
                        }
                        Ok(())
                    })
                    .await
                    .is_err()
            );
            assert_eq!(session.snapshot().await.unwrap(), before);
        }
        session.close().await.unwrap();
    }
    #[tokio::test]
    async fn new_current_policy_parent_members_conflict_even_when_absent_from_selection() {
        let (session, cutoff) = setup().await;
        let before = session.snapshot().await.unwrap();
        for before_fork in [false, true] {
            let at = cutoff.id;
            assert!(
                session
                    .transact_entries(move |tx| {
                        if before_fork {
                            tx.put_document(draft(
                                root(),
                                "new-current",
                                DocumentFork::Current,
                                9,
                            ))?;
                        }
                        tx.fork_conversation(root(), at, ConversationOwnership::Ownerless)?;
                        if !before_fork {
                            tx.put_document(draft(
                                root(),
                                "new-current",
                                DocumentFork::Current,
                                9,
                            ))?;
                        }
                        Ok(())
                    })
                    .await
                    .is_err()
            );
            assert_eq!(session.snapshot().await.unwrap(), before);
        }
        session.close().await.unwrap();
    }
    #[tokio::test]
    async fn unchanged_typed_source_reads_and_unrelated_writes_are_allowed() {
        let (session, cutoff) = setup().await;
        let at = cutoff.id;
        let child = session
            .transact_entries(move |tx| {
                tx.edit_document(
                    &definition("current", 1, DocumentFork::Current),
                    root(),
                    None,
                    &(),
                    |value| {
                        assert_eq!(value["n"], 2);
                        Ok(())
                    },
                )?;
                let child = tx.fork_conversation(root(), at, ConversationOwnership::Ownerless)?;
                tx.edit_document(
                    &definition("asof", 1, DocumentFork::AsOf),
                    root(),
                    None,
                    &(),
                    |value| {
                        assert_eq!(value["n"], 1);
                        Ok(())
                    },
                )?;
                tx.put_document(draft(root(), "initial", DocumentFork::Initial, 4))?;
                tx.put_document(DocumentDraft {
                    address: DocumentAddress {
                        scope: DocumentScope::Session,
                        kind: "session-unrelated".into(),
                        key: None,
                    },
                    version: 1,
                    history: DocumentHistory::Latest,
                    fork: DocumentFork::Initial,
                    value: json!({}),
                })?;
                let independent = tx.create_conversation(ConversationOwnership::Ownerless)?;
                tx.put_document(draft(independent.id, "current", DocumentFork::Current, 5))?;
                tx.append_entry(root(), EntryDraft::new("parent-after-cutoff"))?;
                Ok(child)
            })
            .await
            .unwrap();
        assert_eq!(
            session
                .document(address(child.id, "asof"), DocumentPoint::Current)
                .await
                .unwrap()
                .unwrap()
                .value,
            json!({"n":1})
        );
        assert!(
            session
                .document(address(child.id, "initial"), DocumentPoint::Current)
                .await
                .unwrap()
                .is_none()
        );
        assert_eq!(
            session
                .document(address(root(), "initial"), DocumentPoint::Current)
                .await
                .unwrap()
                .unwrap()
                .value,
            json!({"n":4})
        );
        session.close().await.unwrap();
    }
    #[tokio::test]
    async fn child_copy_edits_migrations_and_retirement_recreation_coalesce_without_changing_parent()
     {
        let (session, cutoff) = setup().await;
        let at = cutoff.id;
        let child = session
            .transact_entries(move |tx| {
                let child = tx.fork_conversation(root(), at, ConversationOwnership::Ownerless)?;
                let new = definition("asof", 2, DocumentFork::AsOf).with_migration(
                    |mut value, version| {
                        assert_eq!(version, 1);
                        value["migrated"] = json!(true);
                        Ok(value)
                    },
                );
                tx.edit_document(&new, child.id, None, &(), |value| {
                    value["n"] = json!(9);
                    Ok(())
                })?;
                tx.retire_document(&address(child.id, "current"))?;
                tx.put_document(draft(child.id, "current", DocumentFork::Current, 8))?;
                Ok(child)
            })
            .await
            .unwrap();
        let copied = session
            .document(address(child.id, "asof"), DocumentPoint::Current)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(copied.version, 2);
        assert_eq!(copied.value, json!({"n":9,"migrated":true}));
        assert_eq!(
            session
                .document(address(root(), "asof"), DocumentPoint::Current)
                .await
                .unwrap()
                .unwrap()
                .version,
            1
        );
        let state = session.snapshot().await.unwrap();
        assert_eq!(state.document_revisions[&copied.id].len(), 1);
        assert_eq!(
            state
                .generic_documents
                .values()
                .filter(|document| document.address == address(child.id, "current"))
                .count(),
            2
        );
        session.close().await.unwrap();
    }
    #[tokio::test]
    async fn nested_forks_guard_cutoff_owner_asof_and_immediate_parent_current_sources() {
        let (session, cutoff) = setup().await;
        let parent = session
            .fork_conversation(root(), cutoff.id, ConversationOwnership::Ownerless)
            .await
            .unwrap();
        let parent_id = parent.id;
        let before = session.snapshot().await.unwrap();
        for mode in 0..2 {
            let at = cutoff.id;
            assert!(
                session
                    .transact_entries(move |tx| {
                        tx.fork_conversation(parent_id, at, ConversationOwnership::Ownerless)?;
                        if mode == 0 {
                            tx.put_document(draft(root(), "asof", DocumentFork::AsOf, 9))?;
                        } else {
                            tx.put_document(draft(parent_id, "current", DocumentFork::Current, 9))?;
                        }
                        Ok(())
                    })
                    .await
                    .is_err()
            );
            assert_eq!(session.snapshot().await.unwrap(), before);
        }
        let at = cutoff.id;
        let grandchild = session
            .transact_entries(move |tx| {
                let child =
                    tx.fork_conversation(parent_id, at, ConversationOwnership::Ownerless)?;
                tx.put_document(draft(root(), "current", DocumentFork::Current, 7))?;
                tx.put_document(draft(parent_id, "asof", DocumentFork::AsOf, 6))?;
                Ok(child)
            })
            .await
            .unwrap();
        assert_eq!(
            session
                .document(address(grandchild.id, "asof"), DocumentPoint::Current)
                .await
                .unwrap()
                .unwrap()
                .value,
            json!({"n":1})
        );
        session.close().await.unwrap();
    }
    #[tokio::test]
    async fn unselected_recreated_asof_incarnation_can_change_beside_historical_fork() {
        let (session, cutoff) = setup().await;
        session
            .transact_entries(|tx| {
                tx.retire_document(&address(root(), "asof"))?;
                tx.put_document(draft(root(), "asof", DocumentFork::AsOf, 5))
            })
            .await
            .unwrap();
        let at = cutoff.id;
        let child = session
            .transact_entries(move |tx| {
                tx.put_document(draft(root(), "asof", DocumentFork::AsOf, 6))?;
                tx.fork_conversation(root(), at, ConversationOwnership::Ownerless)
            })
            .await
            .unwrap();
        assert_eq!(
            session
                .document(address(child.id, "asof"), DocumentPoint::Current)
                .await
                .unwrap()
                .unwrap()
                .value,
            json!({"n":1})
        );
        assert_eq!(
            session
                .document(address(root(), "asof"), DocumentPoint::Current)
                .await
                .unwrap()
                .unwrap()
                .value,
            json!({"n":6})
        );
        session.close().await.unwrap();
    }
    #[tokio::test]
    async fn multiple_forks_track_all_sources_and_do_not_create_cross_child_conflicts() {
        let (session, cutoff) = setup().await;
        let at = cutoff.id;
        let before = session.snapshot().await.unwrap();
        assert!(
            session
                .transact_entries(move |tx| {
                    tx.fork_conversation(root(), at, ConversationOwnership::Ownerless)?;
                    tx.fork_conversation(root(), at, ConversationOwnership::Ownerless)?;
                    tx.put_document(draft(root(), "current", DocumentFork::Current, 99))
                })
                .await
                .is_err()
        );
        assert_eq!(session.snapshot().await.unwrap(), before);
        let (one, two) = session
            .transact_entries(move |tx| {
                let one = tx.fork_conversation(root(), at, ConversationOwnership::Ownerless)?;
                let two = tx.fork_conversation(root(), at, ConversationOwnership::Ownerless)?;
                tx.put_document(draft(one.id, "current", DocumentFork::Current, 7))?;
                tx.put_document(draft(two.id, "current", DocumentFork::Current, 8))?;
                Ok((one, two))
            })
            .await
            .unwrap();
        assert_ne!(one.id, two.id);
        assert_eq!(
            session
                .document(address(one.id, "current"), DocumentPoint::Current)
                .await
                .unwrap()
                .unwrap()
                .value,
            json!({"n":7})
        );
        assert_eq!(
            session
                .document(address(two.id, "current"), DocumentPoint::Current)
                .await
                .unwrap()
                .unwrap()
                .value,
            json!({"n":8})
        );
        session.close().await.unwrap();
    }
    struct ControlledStorage {
        memory: MemoryStorage,
        mode: Arc<AtomicU8>,
        commits: Arc<AtomicUsize>,
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
                self.commits.fetch_add(1, Ordering::SeqCst);
                match self.mode.load(Ordering::SeqCst) {
                    1 => return Err(DurableError::Rejected("fork storage rejected".into())),
                    2 => return Err(DurableError::Uncertain("fork storage uncertain".into())),
                    3 => {
                        self.admitted.notify_one();
                        self.release.notified().await;
                    }
                    _ => {}
                }
                self.memory.commit(claim, batch).await
            })
        }
    }
    #[tokio::test]
    async fn fork_source_conflicts_reject_before_storage_and_failures_never_publish_copies() {
        let mode = Arc::new(AtomicU8::new(0));
        let count = Arc::new(AtomicUsize::new(0));
        let session = DurableSession::open(Box::new(ControlledStorage {
            memory: MemoryStorage::new(),
            mode: mode.clone(),
            commits: count.clone(),
            admitted: Arc::new(Notify::new()),
            release: Arc::new(Notify::new()),
        }))
        .await
        .unwrap();
        let cutoff = seed(&session).await;
        let before = session.snapshot().await.unwrap();
        let calls = count.load(Ordering::SeqCst);
        let mut watch = session.watch().await.unwrap();
        assert!(matches!(
            watch.next().await,
            Some(DurableEvent::Snapshot(_))
        ));
        let at = cutoff.id;
        assert!(
            session
                .transact_entries(move |tx| {
                    tx.fork_conversation(root(), at, ConversationOwnership::Ownerless)?;
                    tx.put_document(draft(root(), "current", DocumentFork::Current, 9))
                })
                .await
                .is_err()
        );
        assert_eq!(
            count.load(Ordering::SeqCst),
            calls,
            "conflict never reaches storage"
        );
        assert_eq!(session.snapshot().await.unwrap(), before);
        mode.store(1, Ordering::SeqCst);
        assert!(
            session
                .fork_conversation(root(), at, ConversationOwnership::Ownerless)
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
                .fork_conversation(root(), at, ConversationOwnership::Ownerless)
                .await
                .is_err()
        );
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(2), watch.next())
                .await
                .unwrap(),
            Some(DurableEvent::End(WatchEnd::Poisoned))
        );
        session.close().await.unwrap();
    }
    #[tokio::test]
    async fn admitted_fork_survives_caller_drop_and_replays_final_child_overrides() {
        let dir =
            std::env::temp_dir().join(format!("rs-ai-fork-transaction-{}", crate::utils::uuidv7()));
        std::fs::create_dir(&dir).unwrap();
        let path = dir.join("journal");
        let session = DurableSession::open(Box::new(JournalStorage::open(&path).unwrap()))
            .await
            .unwrap();
        let cutoff = seed(&session).await;
        let at = cutoff.id;
        let child = session
            .transact_entries(move |tx| {
                let child = tx.fork_conversation(root(), at, ConversationOwnership::Ownerless)?;
                let new =
                    definition("asof", 2, DocumentFork::AsOf).with_migration(|value, _| Ok(value));
                tx.edit_document(&new, child.id, None, &(), |value| {
                    value["n"] = json!(9);
                    Ok(())
                })?;
                tx.retire_document(&address(child.id, "current"))?;
                tx.put_document(draft(child.id, "current", DocumentFork::Current, 8))?;
                Ok(child)
            })
            .await
            .unwrap();
        let before = session.snapshot().await.unwrap();
        session.close().await.unwrap();
        let session = DurableSession::open(Box::new(JournalStorage::open(&path).unwrap()))
            .await
            .unwrap();
        assert_eq!(session.snapshot().await.unwrap(), before);
        assert_eq!(
            session
                .document(address(child.id, "asof"), DocumentPoint::Current)
                .await
                .unwrap()
                .unwrap()
                .version,
            2
        );
        assert_eq!(
            session
                .document(address(child.id, "current"), DocumentPoint::Current)
                .await
                .unwrap()
                .unwrap()
                .value,
            json!({"n":8})
        );
        session.close().await.unwrap();
        std::fs::remove_dir_all(dir).unwrap();
        let mode = Arc::new(AtomicU8::new(0));
        let admitted = Arc::new(Notify::new());
        let release = Arc::new(Notify::new());
        let session = Arc::new(
            DurableSession::open(Box::new(ControlledStorage {
                memory: MemoryStorage::new(),
                mode: mode.clone(),
                commits: Arc::new(AtomicUsize::new(0)),
                admitted: admitted.clone(),
                release: release.clone(),
            }))
            .await
            .unwrap(),
        );
        let cutoff = seed(&session).await;
        let mut watch = session.watch().await.unwrap();
        assert!(matches!(
            watch.next().await,
            Some(DurableEvent::Snapshot(_))
        ));
        mode.store(3, Ordering::SeqCst);
        let writer = {
            let session = session.clone();
            tokio::spawn(async move {
                session
                    .fork_conversation(root(), cutoff.id, ConversationOwnership::Ownerless)
                    .await
            })
        };
        admitted.notified().await;
        assert!(
            tokio::time::timeout(Duration::from_millis(10), watch.next())
                .await
                .is_err()
        );
        writer.abort();
        assert!(writer.await.unwrap_err().is_cancelled());
        release.notify_one();
        let Some(DurableEvent::Commit(batch)) =
            tokio::time::timeout(Duration::from_secs(2), watch.next())
                .await
                .unwrap()
        else {
            panic!("admitted fork must publish")
        };
        assert_eq!(batch.conversations.len(), 1);
        assert_eq!(batch.generic_documents.len(), 2);
        let state = session.snapshot().await.unwrap();
        assert_eq!(state.conversations.len(), 2);
        assert_eq!(state.generic_documents.len(), 5);
        watch.stop();
        session.close().await.unwrap();
    }
}
