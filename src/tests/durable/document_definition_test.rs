#[cfg(test)]
mod tests {
    use crate::durable::*;
    use serde::{Deserialize, Serialize};
    use serde_json::{Value, json};
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    use std::time::Duration;

    // Deliberately not Clone: cloning a token must not require cloning values.
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct Counter {
        count: u64,
    }
    fn root() -> ConversationId {
        ConversationId::new(1).unwrap()
    }
    fn token(version: u32) -> DocumentDefinition<Counter> {
        DocumentDefinition::new(
            "custom.typed",
            version,
            DocumentHistory::Rewindable,
            DocumentFork::AsOf,
            || Ok(Counter { count: 0 }),
        )
        .unwrap()
    }
    #[tokio::test]
    async fn lazy_initialisation_duplicate_access_noop_suppression_and_detached_reads() {
        let calls = Arc::new(AtomicUsize::new(0));
        let count = calls.clone();
        let definition = DocumentDefinition::new(
            "custom.typed",
            1,
            DocumentHistory::Rewindable,
            DocumentFork::AsOf,
            move || {
                count.fetch_add(1, Ordering::SeqCst);
                Ok(Counter { count: 0 })
            },
        )
        .unwrap();
        let session = DurableSession::open(Box::new(MemoryStorage::new()))
            .await
            .unwrap();
        let before = session.snapshot().await.unwrap();
        assert_eq!(
            session
                .typed_document(&definition, root(), None, DocumentPoint::Current)
                .await
                .unwrap(),
            None
        );
        assert_eq!(session.snapshot().await.unwrap(), before);
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        let tx_token = definition.clone();
        session
            .transact_entries(move |tx| {
                tx.edit_document(&tx_token, root(), None, &(), |value| {
                    value.count += 1;
                    Ok(())
                })?;
                tx.edit_document(&tx_token, root(), None, &(), |value| {
                    value.count += 1;
                    Ok(())
                })
            })
            .await
            .unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        let state = session.snapshot().await.unwrap();
        assert_eq!(state.generic_documents.len(), 1);
        assert_eq!(state.next_id, 2);
        let mut detached = session
            .typed_document(&definition, root(), None, DocumentPoint::Current)
            .await
            .unwrap()
            .unwrap();
        detached.count = 999;
        assert_eq!(detached.count, 999);
        assert_eq!(
            session
                .typed_document(&definition, root(), None, DocumentPoint::Current)
                .await
                .unwrap(),
            Some(Counter { count: 2 })
        );
        let mut watch = session
            .watch_document(definition.address(root(), None).unwrap())
            .await
            .unwrap();
        let tx_token = definition.clone();
        session
            .transact_entries(move |tx| {
                tx.edit_document(&tx_token, root(), None, &(), |value| {
                    value.count += 1;
                    value.count -= 1;
                    Ok(())
                })?;
                assert_eq!(
                    tx.entries(root(), &Default::default())?.items.len(),
                    0,
                    "unchanged edits do not enter table-write phase"
                );
                Ok(())
            })
            .await
            .unwrap();
        assert_eq!(session.snapshot().await.unwrap(), state);
        assert!(
            tokio::time::timeout(Duration::from_millis(10), watch.next())
                .await
                .is_err()
        );
        watch.stop();
        session.close().await.unwrap();
    }
    #[tokio::test]
    async fn family_first_seed_key_validation_and_independent_members() {
        let calls = Arc::new(AtomicUsize::new(0));
        let count = calls.clone();
        #[derive(Serialize, Deserialize, Debug, PartialEq)]
        struct Member {
            seed: String,
            hits: u32,
        }
        struct Seed(String); // Also deliberately not Clone.
        let definition = DocumentDefinition::family(
            "custom.member",
            1,
            DocumentHistory::Latest,
            DocumentFork::Current,
            move |seed: &Seed| {
                count.fetch_add(1, Ordering::SeqCst);
                Ok(Member {
                    seed: seed.0.clone(),
                    hits: 0,
                })
            },
        )
        .unwrap();
        assert!(definition.address(root(), None).is_err());
        assert!(definition.address(root(), Some(&"x".repeat(1025))).is_err());
        assert!(token(1).address(root(), Some("key")).is_err());
        let session = DurableSession::open(Box::new(MemoryStorage::new()))
            .await
            .unwrap();
        let tx_token = definition.clone();
        session
            .transact_entries(move |tx| {
                tx.edit_document(
                    &tx_token,
                    root(),
                    Some(""),
                    &Seed("first".into()),
                    |value| {
                        value.hits += 1;
                        Ok(())
                    },
                )?;
                tx.edit_document(
                    &tx_token,
                    root(),
                    Some(""),
                    &Seed("ignored".into()),
                    |value| {
                        value.hits += 1;
                        Ok(())
                    },
                )?;
                tx.edit_document(
                    &tx_token,
                    root(),
                    Some("other"),
                    &Seed("second".into()),
                    |value| {
                        value.hits += 1;
                        Ok(())
                    },
                )
            })
            .await
            .unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        assert_eq!(
            session
                .typed_document(&definition, root(), Some(""), DocumentPoint::Current)
                .await
                .unwrap(),
            Some(Member {
                seed: "first".into(),
                hits: 2
            })
        );
        assert_eq!(
            session
                .typed_document(&definition, root(), Some("other"), DocumentPoint::Current)
                .await
                .unwrap(),
            Some(Member {
                seed: "second".into(),
                hits: 1
            })
        );
        session.close().await.unwrap();
    }
    #[tokio::test]
    async fn token_policies_versions_and_type_mismatch_reject_before_edit() {
        let session = DurableSession::open(Box::new(MemoryStorage::new()))
            .await
            .unwrap();
        let definition = token(2);
        session
            .transact_entries(move |tx| {
                tx.edit_document(&definition, root(), None, &(), |_| Ok(()))
            })
            .await
            .unwrap();
        let state = session.snapshot().await.unwrap();
        for definition in [
            token(1),
            token(3),
            DocumentDefinition::new(
                "custom.typed",
                2,
                DocumentHistory::Latest,
                DocumentFork::Initial,
                || Ok(Counter { count: 0 }),
            )
            .unwrap(),
        ] {
            assert!(
                session
                    .typed_document(&definition, root(), None, DocumentPoint::Current)
                    .await
                    .is_err()
            );
            assert!(
                session
                    .transact_entries(move |tx| tx.edit_document(
                        &definition,
                        root(),
                        None,
                        &(),
                        |_| -> Result<(), DurableError> {
                            panic!("must not edit incompatible document")
                        }
                    ))
                    .await
                    .is_err()
            );
            assert_eq!(session.snapshot().await.unwrap(), state);
        }
        let wrong: DocumentDefinition<Value> = DocumentDefinition::new(
            "other.kind",
            2,
            DocumentHistory::Rewindable,
            DocumentFork::AsOf,
            || Ok(json!({})),
        )
        .unwrap();
        assert!(
            wrong
                .decode(state.generic_documents.values().next())
                .is_err()
        );
        #[derive(Serialize, Deserialize)]
        struct Different {
            missing: String,
        }
        let wrong = DocumentDefinition::new(
            "custom.typed",
            2,
            DocumentHistory::Rewindable,
            DocumentFork::AsOf,
            || {
                Ok(Different {
                    missing: String::new(),
                })
            },
        )
        .unwrap();
        assert!(
            session
                .typed_document(&wrong, root(), None, DocumentPoint::Current)
                .await
                .is_err()
        );
        session.close().await.unwrap();
    }
    #[tokio::test]
    async fn read_migrations_are_detached_historical_and_do_not_persist_or_publish() {
        let session = DurableSession::open(Box::new(MemoryStorage::new()))
            .await
            .unwrap();
        let old = token(1);
        session
            .transact_entries(move |tx| {
                tx.edit_document(&old, root(), None, &(), |value| {
                    value.count = 4;
                    Ok(())
                })
            })
            .await
            .unwrap();
        let state = session.snapshot().await.unwrap();
        let first = state.last_seq.unwrap();
        let calls = Arc::new(AtomicUsize::new(0));
        let count = calls.clone();
        let new = token(2).with_migration(move |mut value, version| {
            assert_eq!(version, 1);
            count.fetch_add(1, Ordering::SeqCst);
            value["count"] = json!(value["count"].as_u64().unwrap() + 10);
            serde_json::from_value(value).map_err(|e| DurableError::Rejected(e.to_string()))
        });
        let mut watch = session
            .watch_document(new.address(root(), None).unwrap())
            .await
            .unwrap();
        for point in [DocumentPoint::Current, DocumentPoint::At(first)] {
            assert_eq!(
                session
                    .typed_document(&new, root(), None, point)
                    .await
                    .unwrap(),
                Some(Counter { count: 14 })
            );
        }
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        assert_eq!(session.snapshot().await.unwrap(), state);
        let tx_token = new.clone();
        let err = session
            .transact_entries(move |tx| tx.edit_document(&tx_token, root(), None, &(), |_| Ok(())))
            .await
            .unwrap_err();
        assert_eq!(
            err,
            DurableError::Rejected("document migration persistence unsupported".into())
        );
        assert!(
            tokio::time::timeout(Duration::from_millis(10), watch.next())
                .await
                .is_err()
        );
        assert_eq!(session.snapshot().await.unwrap(), state);
        watch.stop();
        session.close().await.unwrap();
    }
    #[tokio::test]
    async fn caught_initializer_edit_and_encoding_failures_rollback_all_staging() {
        let session = DurableSession::open(Box::new(MemoryStorage::new()))
            .await
            .unwrap();
        let before = session.snapshot().await.unwrap();
        for mode in 0..6 {
            let definition: DocumentDefinition<Value> = DocumentDefinition::new(
                "custom.failed",
                1,
                DocumentHistory::Latest,
                DocumentFork::Initial,
                move || match mode {
                    0 => Err(DurableError::Rejected("initial failed".into())),
                    1 => panic!("initial panic"),
                    2 => Ok(json!([])),
                    _ => Ok(json!({})),
                },
            )
            .unwrap();
            assert!(
                session
                    .transact_entries(move |tx| {
                        tx.append_entry(root(), EntryDraft::new("must-rollback"))?;
                        let failed = tx.edit_document(&definition, root(), None, &(), |value| {
                            if mode == 3 {
                                panic!("edit panic")
                            };
                            if mode == 5 {
                                return Err(DurableError::Rejected("edit failed".into()));
                            }
                            *value = json!({"text":"x".repeat(MAX_DOCUMENT_BYTES)});
                            Ok(())
                        });
                        assert!(failed.is_err());
                        Ok(()) // catching errors cannot commit prior writes
                    })
                    .await
                    .is_err()
            );
            assert_eq!(session.snapshot().await.unwrap(), before);
        }
        session.close().await.unwrap();
    }
    #[tokio::test]
    async fn migration_failures_and_invalid_results_leave_original_state_usable() {
        let session = DurableSession::open(Box::new(MemoryStorage::new()))
            .await
            .unwrap();
        let old = token(1);
        session
            .transact_entries(move |tx| tx.edit_document(&old, root(), None, &(), |_| Ok(())))
            .await
            .unwrap();
        let state = session.snapshot().await.unwrap();
        for mode in 0..4 {
            let definition: DocumentDefinition<Value> = DocumentDefinition::new(
                "custom.typed",
                2,
                DocumentHistory::Rewindable,
                DocumentFork::AsOf,
                || Ok(json!({})),
            )
            .unwrap()
            .with_migration(move |_, _| match mode {
                0 => Err(DurableError::Rejected("migration failed".into())),
                1 => panic!("migration panic"),
                2 => Ok(json!(null)),
                _ => Ok(json!({"text":"x".repeat(MAX_DOCUMENT_BYTES)})),
            });
            assert!(
                session
                    .typed_document(&definition, root(), None, DocumentPoint::Current)
                    .await
                    .is_err()
            );
            assert_eq!(session.snapshot().await.unwrap(), state);
        }
        assert_eq!(
            session
                .typed_document(&token(1), root(), None, DocumentPoint::Current)
                .await
                .unwrap(),
            Some(Counter { count: 0 })
        );
        session.close().await.unwrap();
    }
    #[tokio::test]
    async fn typed_history_and_fork_copies_keep_source_and_child_independent() {
        let session = DurableSession::open(Box::new(MemoryStorage::new()))
            .await
            .unwrap();
        let definition = token(1);
        let tx_token = definition.clone();
        let cutoff = session
            .transact_entries(move |tx| {
                tx.edit_document(&tx_token, root(), None, &(), |value| {
                    value.count = 7;
                    Ok(())
                })?;
                tx.append_entry(root(), EntryDraft::new("cut"))
            })
            .await
            .unwrap();
        let tx_token = definition.clone();
        session
            .transact_entries(move |tx| {
                tx.edit_document(&tx_token, root(), None, &(), |value| {
                    value.count = 8;
                    Ok(())
                })
            })
            .await
            .unwrap();
        let child = session
            .fork_conversation(root(), cutoff.id, ConversationOwnership::Ownerless)
            .await
            .unwrap();
        let child_id = child.id;
        let tx_token = definition.clone();
        session
            .transact_entries(move |tx| {
                tx.append_entry(child_id, EntryDraft::new("before-document-edit"))?;
                tx.edit_document(&tx_token, child_id, None, &(), |value| {
                    assert_eq!(value.count, 7);
                    value.count = 9;
                    Ok(())
                })
            })
            .await
            .unwrap();
        assert_eq!(
            session
                .typed_document(&definition, root(), None, DocumentPoint::Current)
                .await
                .unwrap(),
            Some(Counter { count: 8 })
        );
        assert_eq!(
            session
                .typed_document(
                    &definition,
                    root(),
                    None,
                    DocumentPoint::At(cutoff.created_seq)
                )
                .await
                .unwrap(),
            Some(Counter { count: 7 })
        );
        assert_eq!(
            session
                .typed_document(&definition, child_id, None, DocumentPoint::Current)
                .await
                .unwrap(),
            Some(Counter { count: 9 })
        );
        session.close().await.unwrap();
    }

    #[tokio::test]
    async fn repeated_typed_edits_and_migrated_reads_profile_workload() {
        #[derive(Serialize, Deserialize)]
        struct Payload {
            text: String,
            index: u32,
        }
        let definition = DocumentDefinition::new(
            "custom.profile",
            1,
            DocumentHistory::Latest,
            DocumentFork::Current,
            || {
                Ok(Payload {
                    text: "x".repeat(4096),
                    index: 0,
                })
            },
        )
        .unwrap();
        let session = DurableSession::open(Box::new(MemoryStorage::new()))
            .await
            .unwrap();
        for index in 0..=64 {
            let definition = definition.clone();
            session
                .transact_entries(move |tx| {
                    tx.edit_document(&definition, root(), None, &(), |value| {
                        value.index = index;
                        Ok(())
                    })
                })
                .await
                .unwrap();
        }
        let new = DocumentDefinition::new(
            "custom.profile",
            2,
            DocumentHistory::Latest,
            DocumentFork::Current,
            || {
                Ok(Payload {
                    text: String::new(),
                    index: 0,
                })
            },
        )
        .unwrap()
        .with_migration(|mut value, version| {
            assert_eq!(version, 1);
            value["index"] = json!(value["index"].as_u64().unwrap() + 1);
            serde_json::from_value(value).map_err(|e| DurableError::Rejected(e.to_string()))
        });
        for _ in 0..64 {
            let value = session
                .typed_document(&new, root(), None, DocumentPoint::Current)
                .await
                .unwrap()
                .unwrap();
            assert_eq!(value.index, 65);
            assert_eq!(value.text.len(), 4096);
        }
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
            1
        );
        session.close().await.unwrap();
    }

    #[tokio::test]
    async fn fresh_tokens_reopen_history_and_retirement_reinitialises_one_new_incarnation() {
        let dir =
            std::env::temp_dir().join(format!("rs-ai-typed-document-{}", crate::utils::uuidv7()));
        std::fs::create_dir(&dir).unwrap();
        let path = dir.join("journal");
        let session = DurableSession::open(Box::new(JournalStorage::open(&path).unwrap()))
            .await
            .unwrap();
        let definition = token(1);
        session
            .transact_entries(move |tx| {
                tx.edit_document(&definition, root(), None, &(), |value| {
                    value.count = 3;
                    Ok(())
                })
            })
            .await
            .unwrap();
        let state = session.snapshot().await.unwrap();
        let first = state.generic_documents.values().next().unwrap().clone();
        session
            .transact_entries(|tx| tx.retire_document(&token(1).address(root(), None)?))
            .await
            .unwrap();
        assert_eq!(
            session
                .typed_document(&token(1), root(), None, DocumentPoint::Current)
                .await
                .unwrap(),
            None
        );
        session
            .transact_entries(|tx| {
                tx.edit_document(&token(1), root(), None, &(), |value| {
                    assert_eq!(value.count, 0);
                    value.count = 9;
                    Ok(())
                })
            })
            .await
            .unwrap();
        let newer = session
            .document(
                token(1).address(root(), None).unwrap(),
                DocumentPoint::Current,
            )
            .await
            .unwrap()
            .unwrap();
        assert_ne!(first.id, newer.id);
        session.close().await.unwrap();
        let session = DurableSession::open(Box::new(JournalStorage::open(&path).unwrap()))
            .await
            .unwrap();
        assert_eq!(
            session
                .typed_document(&token(1), root(), None, DocumentPoint::Current)
                .await
                .unwrap(),
            Some(Counter { count: 9 })
        );
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
            Some(Counter { count: 3 })
        );
        session.close().await.unwrap();
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn token_validation_does_not_call_initializers_or_need_registries() {
        for (kind, version, history, fork) in [
            ("", 1, DocumentHistory::Latest, DocumentFork::Initial),
            ("pi.live", 1, DocumentHistory::Latest, DocumentFork::Initial),
            ("custom", 0, DocumentHistory::Latest, DocumentFork::Initial),
            ("custom", 1, DocumentHistory::Latest, DocumentFork::AsOf),
        ] {
            assert!(
                DocumentDefinition::<Counter>::new(kind, version, history, fork, || panic!(
                    "definition must not initialise"
                ))
                .is_err()
            );
        }
        let definition = token(1);
        let other = token(1);
        assert_eq!(definition.kind(), other.kind());
        assert_eq!(definition.version(), 1);
    }
}
