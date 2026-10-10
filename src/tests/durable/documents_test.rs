#[cfg(test)]
mod tests {
    use crate::durable::*;
    use serde_json::json;

    fn address(conversation: ConversationId, kind: &str) -> DocumentAddress {
        DocumentAddress {
            conversation_id: conversation,
            kind: kind.into(),
            key: None,
        }
    }
    fn draft(
        conversation: ConversationId,
        kind: &str,
        value: i64,
        history: DocumentHistory,
        fork: DocumentFork,
    ) -> DocumentDraft {
        DocumentDraft {
            address: address(conversation, kind),
            version: 1,
            history,
            fork,
            value: json!({"value":value}),
        }
    }
    #[tokio::test]
    async fn atomic_whole_value_updates_historical_reads_retirement_and_recreation() {
        let session = DurableSession::open(Box::new(MemoryStorage::new()))
            .await
            .unwrap();
        let root = ConversationId::new(1).unwrap();
        let first = session
            .transact_entries(move |tx| {
                let first = tx.put_document(draft(
                    root,
                    "custom.doc",
                    1,
                    DocumentHistory::Rewindable,
                    DocumentFork::AsOf,
                ))?;
                assert_eq!(
                    tx.document(&first.address, DocumentPoint::Current)?
                        .unwrap()
                        .value["value"],
                    1
                );
                tx.append_entry(root, EntryDraft::new("note"))?;
                tx.put_document(draft(
                    root,
                    "custom.doc",
                    2,
                    DocumentHistory::Rewindable,
                    DocumentFork::AsOf,
                ))?;
                Ok(first)
            })
            .await
            .unwrap();
        assert_eq!(
            session
                .document(first.address.clone(), DocumentPoint::At(first.created_seq))
                .await
                .unwrap()
                .unwrap()
                .value["value"],
            2
        );
        session
            .transact_entries(move |tx| {
                tx.put_document(draft(
                    root,
                    "custom.doc",
                    3,
                    DocumentHistory::Rewindable,
                    DocumentFork::AsOf,
                ))
            })
            .await
            .unwrap();
        assert_eq!(
            session
                .document(first.address.clone(), DocumentPoint::At(first.created_seq))
                .await
                .unwrap()
                .unwrap()
                .value["value"],
            2
        );
        let mut current = session
            .document(first.address.clone(), DocumentPoint::Current)
            .await
            .unwrap()
            .unwrap();
        current.value = json!(null);
        assert_eq!(
            session
                .document(first.address.clone(), DocumentPoint::Current)
                .await
                .unwrap()
                .unwrap()
                .value["value"],
            3
        );
        let retire = first.address.clone();
        session
            .transact_entries(move |tx| tx.retire_document(&retire))
            .await
            .unwrap();
        assert!(
            session
                .document(first.address.clone(), DocumentPoint::Current)
                .await
                .unwrap()
                .is_none()
        );
        assert_eq!(
            session
                .document(first.address.clone(), DocumentPoint::At(first.created_seq))
                .await
                .unwrap()
                .unwrap()
                .value["value"],
            2
        );
        let new = session
            .transact_entries(move |tx| {
                tx.put_document(draft(
                    root,
                    "custom.doc",
                    4,
                    DocumentHistory::Rewindable,
                    DocumentFork::AsOf,
                ))
            })
            .await
            .unwrap();
        assert_ne!(new.id, first.id);
        session.close().await.unwrap();
        assert!(matches!(
            session
                .document(first.address, DocumentPoint::Current)
                .await,
            Err(DurableError::Closed)
        ));
    }
    #[tokio::test]
    async fn fork_selects_as_of_entry_owner_current_parent_and_skips_initial() {
        let dir =
            std::env::temp_dir().join(format!("rs-ai-document-fork-{}", crate::utils::uuidv7()));
        std::fs::create_dir(&dir).unwrap();
        let path = dir.join("journal");
        let session = DurableSession::open(Box::new(JournalStorage::open(&path).unwrap()))
            .await
            .unwrap();
        let root = ConversationId::new(1).unwrap();
        let cutoff = session
            .transact_entries(move |tx| {
                tx.put_document(draft(
                    root,
                    "asof",
                    1,
                    DocumentHistory::Rewindable,
                    DocumentFork::AsOf,
                ))?;
                tx.put_document(draft(
                    root,
                    "current",
                    10,
                    DocumentHistory::Latest,
                    DocumentFork::Current,
                ))?;
                tx.put_document(draft(
                    root,
                    "initial",
                    100,
                    DocumentHistory::Latest,
                    DocumentFork::Initial,
                ))?;
                tx.append_entry(root, EntryDraft::new("cutoff"))
            })
            .await
            .unwrap();
        session
            .transact_entries(move |tx| {
                tx.put_document(draft(
                    root,
                    "asof",
                    2,
                    DocumentHistory::Rewindable,
                    DocumentFork::AsOf,
                ))?;
                tx.put_document(draft(
                    root,
                    "current",
                    20,
                    DocumentHistory::Latest,
                    DocumentFork::Current,
                ))?;
                Ok(())
            })
            .await
            .unwrap();
        let child = session
            .fork_conversation(root, cutoff.id, ConversationOwnership::Ownerless)
            .await
            .unwrap();
        assert_eq!(
            session
                .document(address(child.id, "asof"), DocumentPoint::Current)
                .await
                .unwrap()
                .unwrap()
                .value["value"],
            1
        );
        assert_eq!(
            session
                .document(address(child.id, "current"), DocumentPoint::Current)
                .await
                .unwrap()
                .unwrap()
                .value["value"],
            20
        );
        assert!(
            session
                .document(address(child.id, "initial"), DocumentPoint::Current)
                .await
                .unwrap()
                .is_none()
        );
        let child_id = child.id;
        session
            .transact_entries(move |tx| {
                tx.put_document(draft(
                    child_id,
                    "current",
                    30,
                    DocumentHistory::Latest,
                    DocumentFork::Current,
                ))
            })
            .await
            .unwrap();
        let nested = session
            .fork_conversation(child.id, cutoff.id, ConversationOwnership::Ownerless)
            .await
            .unwrap();
        assert_eq!(
            session
                .document(address(nested.id, "asof"), DocumentPoint::Current)
                .await
                .unwrap()
                .unwrap()
                .value["value"],
            1
        );
        assert_eq!(
            session
                .document(address(nested.id, "current"), DocumentPoint::Current)
                .await
                .unwrap()
                .unwrap()
                .value["value"],
            30
        );
        assert_eq!(
            session
                .document(address(root, "current"), DocumentPoint::Current)
                .await
                .unwrap()
                .unwrap()
                .value["value"],
            20
        );
        session.close().await.unwrap();
        let reopened = DurableSession::open(Box::new(JournalStorage::open(&path).unwrap()))
            .await
            .unwrap();
        assert_eq!(
            reopened
                .document(address(nested.id, "asof"), DocumentPoint::Current)
                .await
                .unwrap()
                .unwrap()
                .value["value"],
            1
        );
        reopened.close().await.unwrap();
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[tokio::test]
    async fn update_and_retire_in_same_commit_preserves_final_as_of_value() {
        let session = DurableSession::open(Box::new(MemoryStorage::new()))
            .await
            .unwrap();
        let root = ConversationId::new(1).unwrap();
        let first = session
            .transact_entries(move |tx| {
                tx.put_document(draft(
                    root,
                    "retired",
                    1,
                    DocumentHistory::Rewindable,
                    DocumentFork::AsOf,
                ))
            })
            .await
            .unwrap();
        let before = session.snapshot().await.unwrap();
        session
            .transact_entries(move |tx| {
                tx.put_document(draft(
                    root,
                    "retired",
                    2,
                    DocumentHistory::Rewindable,
                    DocumentFork::AsOf,
                ))?;
                tx.retire_document(&address(root, "retired"))?;
                Ok(())
            })
            .await
            .unwrap();
        assert!(
            session
                .document(address(root, "retired"), DocumentPoint::Current)
                .await
                .unwrap()
                .is_none()
        );
        assert_eq!(
            session
                .document(
                    address(root, "retired"),
                    DocumentPoint::At(first.created_seq)
                )
                .await
                .unwrap()
                .unwrap()
                .value["value"],
            1
        );
        let current = session.snapshot().await.unwrap();
        assert_eq!(current.generic_documents[&first.id].value["value"], 2);
        assert_ne!(before, current);
        assert!(
            session
                .document(
                    address(root, "retired"),
                    DocumentPoint::At(CommitSeq::new(999).unwrap())
                )
                .await
                .is_err()
        );
        session.close().await.unwrap();
    }

    #[tokio::test]
    async fn keyed_documents_and_duplicate_fork_addresses_are_checked_atomically() {
        let session = DurableSession::open(Box::new(MemoryStorage::new()))
            .await
            .unwrap();
        let root = ConversationId::new(1).unwrap();
        let cutoff = session
            .transact_entries(move |tx| {
                let mut one = draft(
                    root,
                    "family",
                    1,
                    DocumentHistory::Rewindable,
                    DocumentFork::AsOf,
                );
                one.address.key = Some("one".into());
                tx.put_document(one)?;
                let mut two = draft(
                    root,
                    "family",
                    2,
                    DocumentHistory::Rewindable,
                    DocumentFork::AsOf,
                );
                two.address.key = Some("two".into());
                tx.put_document(two)?;
                tx.append_entry(root, EntryDraft::new("cut"))
            })
            .await
            .unwrap();
        let child = session
            .fork_conversation(root, cutoff.id, ConversationOwnership::Ownerless)
            .await
            .unwrap();
        for key in ["one", "two"] {
            let mut lookup = address(child.id, "family");
            lookup.key = Some(key.into());
            assert!(
                session
                    .document(lookup, DocumentPoint::Current)
                    .await
                    .unwrap()
                    .is_some()
            );
        }
        let child_id = child.id;
        session
            .transact_entries(move |tx| {
                let mut conflict = draft(
                    child_id,
                    "family",
                    3,
                    DocumentHistory::Latest,
                    DocumentFork::Current,
                );
                conflict.address.key = Some("one".into());
                // Existing copied definition cannot change; retire then recreate in a later commit.
                let lookup = conflict.address.clone();
                tx.retire_document(&lookup)?;
                Ok(())
            })
            .await
            .unwrap();
        session
            .transact_entries(move |tx| {
                let mut conflict = draft(
                    child_id,
                    "family",
                    3,
                    DocumentHistory::Latest,
                    DocumentFork::Current,
                );
                conflict.address.key = Some("one".into());
                tx.put_document(conflict)
            })
            .await
            .unwrap();
        let before = session.snapshot().await.unwrap();
        assert!(
            session
                .fork_conversation(child.id, cutoff.id, ConversationOwnership::Ownerless)
                .await
                .is_err()
        );
        assert_eq!(session.snapshot().await.unwrap(), before);
        let lookup = address(root, "family");
        assert!(
            session
                .document(lookup, DocumentPoint::Current)
                .await
                .unwrap()
                .is_none()
        );
        session.close().await.unwrap();
    }

    #[tokio::test]
    async fn repeated_document_updates_and_fork_copies_profile_workload() {
        let session = DurableSession::open(Box::new(MemoryStorage::new()))
            .await
            .unwrap();
        let root = ConversationId::new(1).unwrap();
        let cutoff = session
            .transact_entries(move |tx| {
                let mut value = draft(
                    root,
                    "profile-doc",
                    0,
                    DocumentHistory::Rewindable,
                    DocumentFork::Current,
                );
                value.value = json!({"text":"x".repeat(4096),"index":0});
                tx.put_document(value)?;
                tx.append_entry(root, EntryDraft::new("cut"))
            })
            .await
            .unwrap();
        for index in 1..=64 {
            session
                .transact_entries(move |tx| {
                    let mut value = draft(
                        root,
                        "profile-doc",
                        index,
                        DocumentHistory::Rewindable,
                        DocumentFork::Current,
                    );
                    value.value = json!({"text":"x".repeat(4096),"index":index});
                    tx.put_document(value)
                })
                .await
                .unwrap();
        }
        for _ in 0..16 {
            let child = session
                .fork_conversation(root, cutoff.id, ConversationOwnership::Ownerless)
                .await
                .unwrap();
            assert_eq!(
                session
                    .document(address(child.id, "profile-doc"), DocumentPoint::Current)
                    .await
                    .unwrap()
                    .unwrap()
                    .value["index"],
                64
            );
        }
        assert_eq!(
            session.snapshot().await.unwrap().generic_documents.len(),
            17
        );
        session.close().await.unwrap();
    }

    #[tokio::test]
    async fn document_pages_filter_membership_and_preserve_historical_cursor_direction() {
        let session = DurableSession::open(Box::new(MemoryStorage::new()))
            .await
            .unwrap();
        let root = ConversationId::new(1).unwrap();
        let records = session
            .transact_entries(move |tx| {
                let mut records = vec![];
                for index in 0..3 {
                    let mut value = draft(
                        root,
                        "family",
                        index,
                        DocumentHistory::Rewindable,
                        DocumentFork::AsOf,
                    );
                    value.address.key = Some(index.to_string());
                    records.push(tx.put_document(value)?);
                }
                Ok(records)
            })
            .await
            .unwrap();
        let first_seq = records[0].created_seq;
        let mut query = DocumentQuery::current(root);
        query.kind = Some("family".into());
        query.fork = Some(DocumentFork::AsOf);
        query.scan.limit = 1;
        query.scan.order = Some(ScanOrder::Descending);
        let mut page = session.documents(query.clone()).await.unwrap();
        assert_eq!(page.items[0].id, records[2].id);
        page.items[0].value = json!(null);
        query.scan.cursor = page.cursor;
        query.scan.order = None;
        query.scan.limit = 10;
        assert_eq!(
            session
                .documents(query.clone())
                .await
                .unwrap()
                .items
                .iter()
                .map(|record| record.id)
                .collect::<Vec<_>>(),
            [records[1].id, records[0].id]
        );
        query.scan.order = Some(ScanOrder::Ascending);
        assert!(session.documents(query).await.is_err());
        let retired = records[1].address.clone();
        session
            .transact_entries(move |tx| tx.retire_document(&retired))
            .await
            .unwrap();
        let mut history = DocumentQuery::current(root);
        history.point = DocumentPoint::At(first_seq);
        assert_eq!(session.documents(history).await.unwrap().items.len(), 3);
        assert_eq!(
            session
                .documents(DocumentQuery::current(root))
                .await
                .unwrap()
                .items
                .len(),
            2
        );
        let mut missing = DocumentQuery::current(root);
        missing.fork = Some(DocumentFork::Initial);
        assert!(session.documents(missing).await.unwrap().items.is_empty());
        session
            .transact_entries(move |tx| {
                assert_eq!(tx.documents(&DocumentQuery::current(root))?.items.len(), 2);
                tx.append_entry(root, EntryDraft::new("write"))?;
                assert!(tx.documents(&DocumentQuery::current(root)).is_err());
                Ok(())
            })
            .await
            .unwrap();
        session.close().await.unwrap();
        assert!(matches!(
            session.documents(DocumentQuery::current(root)).await,
            Err(DurableError::Closed)
        ));
    }

    #[tokio::test]
    async fn caught_invalid_definition_and_raw_identity_changes_are_atomic() {
        let session = DurableSession::open(Box::new(MemoryStorage::new()))
            .await
            .unwrap();
        let root = ConversationId::new(1).unwrap();
        let original = session
            .transact_entries(move |tx| {
                tx.put_document(draft(
                    root,
                    "checked",
                    1,
                    DocumentHistory::Rewindable,
                    DocumentFork::AsOf,
                ))
            })
            .await
            .unwrap();
        let before = session.snapshot().await.unwrap();
        let mut watch = session.watch().await.unwrap();
        watch.next().await.unwrap();
        assert!(
            session
                .transact_entries(move |tx| {
                    tx.put_document(draft(
                        root,
                        "checked",
                        2,
                        DocumentHistory::Rewindable,
                        DocumentFork::AsOf,
                    ))?;
                    let mut invalid = draft(
                        root,
                        "wrong",
                        3,
                        DocumentHistory::Latest,
                        DocumentFork::Initial,
                    );
                    invalid.value = json!(null);
                    assert!(tx.put_document(invalid).is_err());
                    Ok(())
                })
                .await
                .is_err()
        );
        for change in 0..4 {
            let mut record = original.clone();
            record.updated_seq = CommitSeq::new(before.next_seq).unwrap();
            match change {
                0 => record.version = 2,
                1 => record.address.kind = "changed".into(),
                2 => record.created_seq = record.updated_seq,
                _ => record.fork = DocumentFork::Current,
            };
            assert!(
                session
                    .commit(CommitBatch {
                        generic_documents: vec![record],
                        conversations: vec![],
                        seq: CommitSeq::new(before.next_seq).unwrap(),
                        next_id: before.next_id,
                        next_seq: before.next_seq + 1,
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
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(10), watch.next())
                .await
                .is_err()
        );
        session.close().await.unwrap();
    }

    #[tokio::test]
    async fn invalid_documents_and_callback_failure_never_partially_adopt() {
        let session = DurableSession::open(Box::new(MemoryStorage::new()))
            .await
            .unwrap();
        let root = ConversationId::new(1).unwrap();
        let before = session.snapshot().await.unwrap();
        for invalid in [
            draft(
                root,
                "pi.provider",
                1,
                DocumentHistory::Latest,
                DocumentFork::Initial,
            ),
            draft(
                root,
                "bad-policy",
                1,
                DocumentHistory::Latest,
                DocumentFork::AsOf,
            ),
            draft(
                ConversationId::new(999).unwrap(),
                "foreign",
                1,
                DocumentHistory::Latest,
                DocumentFork::Current,
            ),
        ] {
            assert!(
                session
                    .transact_entries(move |tx| {
                        tx.append_entry(root, EntryDraft::new("staged"))?;
                        tx.put_document(invalid.clone())?;
                        Ok(())
                    })
                    .await
                    .is_err()
            );
            assert_eq!(session.snapshot().await.unwrap(), before);
        }
        assert!(
            session
                .transact_entries(move |tx| {
                    tx.put_document(draft(
                        root,
                        "valid",
                        1,
                        DocumentHistory::Latest,
                        DocumentFork::Initial,
                    ))?;
                    Err::<(), _>(DurableError::Rejected("callback".into()))
                })
                .await
                .is_err()
        );
        assert_eq!(session.snapshot().await.unwrap(), before);
        let doc = session
            .transact_entries(move |tx| {
                tx.put_document(draft(
                    root,
                    "valid",
                    1,
                    DocumentHistory::Latest,
                    DocumentFork::Initial,
                ))
            })
            .await
            .unwrap();
        assert!(
            session
                .document(doc.address, DocumentPoint::At(doc.created_seq))
                .await
                .is_err()
        );
        session.close().await.unwrap();
    }
}
