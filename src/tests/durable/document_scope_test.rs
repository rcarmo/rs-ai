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
    fn address(scope: DocumentScope) -> DocumentAddress {
        DocumentAddress {
            scope,
            kind: "custom.scoped".into(),
            key: None,
        }
    }
    fn draft(scope: DocumentScope, n: i64) -> DocumentDraft {
        DocumentDraft {
            address: address(scope),
            version: 1,
            history: DocumentHistory::Latest,
            fork: DocumentFork::Initial,
            value: json!({"n":n}),
        }
    }
    fn token(kind: DocumentScopeKind) -> DocumentDefinition<Value> {
        DocumentDefinition::new(
            "custom.scoped",
            1,
            DocumentHistory::Latest,
            DocumentFork::Initial,
            || Ok(json!({"n":1})),
        )
        .unwrap()
        .with_scope(kind)
        .unwrap()
    }
    fn batch(state: &StorageSnapshot) -> CommitBatch {
        CommitBatch {
            seq: CommitSeq::new(state.next_seq).unwrap(),
            next_id: state.next_id,
            next_seq: state.next_seq + 1,
            generic_documents: vec![],
            tasks: vec![],
            entries: vec![],
            conversations: vec![],
            submissions: vec![],
            documents: vec![],
        }
    }
    fn task(id: TaskId, seq: CommitSeq, state: TaskState) -> TaskRecord {
        let terminal = state.terminal();
        TaskRecord {
            id,
            conversation_id: root(),
            kind: "generation".into(),
            version: 1,
            owner_task_id: None,
            state,
            input: json!({}),
            checkpoint: json!({}),
            outcome: terminal.then(|| json!({"ok":true})),
            abort_requested: false,
            started_at: None,
            ended_at: None,
            updated_seq: seq,
        }
    }
    async fn create_task(session: &DurableSession) -> TaskId {
        let state = session.snapshot().await.unwrap();
        let mut write = batch(&state);
        let id = TaskId::new(state.next_id).unwrap();
        write.next_id += 1;
        write.tasks.push(task(id, write.seq, TaskState::Pending));
        session.commit(write).await.unwrap();
        id
    }
    async fn change_task(session: &DurableSession, id: TaskId, state: TaskState) {
        let snapshot = session.snapshot().await.unwrap();
        let mut write = batch(&snapshot);
        let mut record = snapshot.tasks[&id].clone();
        record.outcome = state.terminal().then(|| json!({"ok":true}));
        record.state = state;
        record.updated_seq = write.seq;
        write.tasks.push(record);
        session.commit(write).await.unwrap();
    }
    async fn put(session: &DurableSession, scope: DocumentScope, n: i64) -> GenericDocumentRecord {
        session
            .transact_entries(move |tx| tx.put_document(draft(scope, n)))
            .await
            .unwrap()
    }

    #[test]
    fn address_wire_preserves_legacy_conversations_and_rejects_ambiguous_owners() {
        let legacy = json!({"conversation_id":1,"kind":"custom.scoped"});
        let record: DocumentAddress = serde_json::from_value(legacy.clone()).unwrap();
        assert_eq!(
            record,
            address(DocumentScope::Conversation {
                conversation_id: root()
            })
        );
        assert_eq!(serde_json::to_value(record).unwrap(), legacy);
        for scope in [
            DocumentScope::Session,
            DocumentScope::Task {
                task_id: TaskId::new(2).unwrap(),
            },
        ] {
            let record = address(scope);
            let wire = serde_json::to_value(&record).unwrap();
            assert!(wire.get("conversation_id").is_none());
            assert_eq!(
                serde_json::from_value::<DocumentAddress>(wire).unwrap(),
                record
            );
        }
        for invalid in [
            json!({"kind":"x"}),
            json!({"kind":"x","conversation_id":1,"scope":{"kind":"session"}}),
            json!({"kind":"x","scope":{"kind":"task"}}),
            json!({"kind":"x","scope":{"kind":"other"}}),
        ] {
            assert!(serde_json::from_value::<DocumentAddress>(invalid).is_err());
        }
    }
    #[tokio::test]
    async fn session_conversation_and_task_addresses_are_independent_and_queries_are_scoped() {
        let session = DurableSession::open(Box::new(MemoryStorage::new()))
            .await
            .unwrap();
        let id = create_task(&session).await;
        let scopes = [
            DocumentScope::Session,
            DocumentScope::Conversation {
                conversation_id: root(),
            },
            DocumentScope::Task { task_id: id },
        ];
        for (index, scope) in scopes.into_iter().enumerate() {
            put(&session, scope, index as i64).await;
        }
        for (index, scope) in scopes.into_iter().enumerate() {
            let items = session
                .documents(DocumentQuery::scoped(scope))
                .await
                .unwrap()
                .items;
            assert_eq!(items.len(), 1);
            assert_eq!(items[0].address.scope, scope);
            assert_eq!(items[0].value, json!({"n":index}));
        }
        assert_eq!(
            session.snapshot().await.unwrap().conversations.len(),
            1,
            "task IDs must not infer conversation scopes"
        );
        session.close().await.unwrap();
    }
    #[tokio::test]
    async fn non_conversation_tokens_validate_scope_and_current_only_policies_before_initialising()
    {
        let session = DurableSession::open(Box::new(MemoryStorage::new()))
            .await
            .unwrap();
        let definition = token(DocumentScopeKind::Session);
        assert!(definition.address(root(), None).is_err());
        assert!(
            definition
                .address_scoped(
                    DocumentScope::Task {
                        task_id: TaskId::new(999).unwrap()
                    },
                    None
                )
                .is_err()
        );
        assert!(
            DocumentDefinition::<Value>::new(
                "custom",
                1,
                DocumentHistory::Rewindable,
                DocumentFork::AsOf,
                || panic!("must not initialise")
            )
            .unwrap()
            .with_scope(DocumentScopeKind::Task)
            .is_err()
        );
        let tx_token = definition.clone();
        session
            .transact_entries(move |tx| {
                tx.edit_document_scoped(&tx_token, DocumentScope::Session, None, &(), |value| {
                    value["n"] = json!(2);
                    Ok(())
                })
            })
            .await
            .unwrap();
        assert_eq!(
            session
                .typed_document_scoped(
                    &definition,
                    DocumentScope::Session,
                    None,
                    DocumentPoint::Current
                )
                .await
                .unwrap(),
            Some(json!({"n":2}))
        );
        let seq = session.snapshot().await.unwrap().last_seq.unwrap();
        for scope in [
            DocumentScope::Session,
            DocumentScope::Task {
                task_id: TaskId::new(999).unwrap(),
            },
        ] {
            assert!(
                session
                    .document(address(scope), DocumentPoint::At(seq))
                    .await
                    .is_err()
            );
            let mut query = DocumentQuery::scoped(scope);
            query.point = DocumentPoint::At(seq);
            assert!(session.documents(query).await.is_err());
        }
        let raw = session
            .document(address(DocumentScope::Session), DocumentPoint::Current)
            .await
            .unwrap()
            .unwrap();
        assert!(
            token(DocumentScopeKind::Conversation)
                .decode(Some(&raw))
                .is_err()
        );
        let before = session.snapshot().await.unwrap();
        assert!(
            session
                .transact_entries(|tx| {
                    let mut value = draft(DocumentScope::Session, 3);
                    value.history = DocumentHistory::Rewindable;
                    tx.put_document(value)
                })
                .await
                .is_err()
        );
        assert_eq!(session.snapshot().await.unwrap(), before);
        session.close().await.unwrap();
    }
    #[tokio::test]
    async fn task_documents_require_existing_nonterminal_owner_and_retire_with_all_terminal_outcomes()
     {
        for terminal in [TaskState::Succeeded, TaskState::Failed, TaskState::Aborted] {
            let session = DurableSession::open(Box::new(MemoryStorage::new()))
                .await
                .unwrap();
            let before = session.snapshot().await.unwrap();
            assert!(
                session
                    .transact_entries(|tx| tx.put_document(draft(
                        DocumentScope::Task {
                            task_id: TaskId::new(999).unwrap()
                        },
                        0
                    )))
                    .await
                    .is_err()
            );
            assert_eq!(session.snapshot().await.unwrap(), before);
            let id = create_task(&session).await;
            let scope = DocumentScope::Task { task_id: id };
            let original = put(&session, scope, 1).await;
            let mut watch = session.watch_document(address(scope)).await.unwrap();
            change_task(&session, id, TaskState::Running).await;
            assert!(
                tokio::time::timeout(Duration::from_millis(10), watch.next())
                    .await
                    .is_err()
            );
            change_task(&session, id, terminal).await;
            let state = session.snapshot().await.unwrap();
            let seq = state.tasks[&id].updated_seq;
            assert_eq!(state.generic_documents[&original.id].retired_seq, Some(seq));
            assert!(
                session
                    .document(address(scope), DocumentPoint::Current)
                    .await
                    .unwrap()
                    .is_none()
            );
            assert_eq!(
                tokio::time::timeout(Duration::from_secs(2), watch.next())
                    .await
                    .unwrap(),
                Some(DocumentEvent::Replacement {
                    record: None,
                    seq,
                    reconciled: false
                })
            );
            assert_eq!(
                watch.next().await,
                Some(DocumentEvent::End(DocumentWatchEnd::Retired))
            );
            assert!(
                session
                    .transact_entries(move |tx| tx.put_document(draft(scope, 2)))
                    .await
                    .is_err()
            );
            let definition = token(DocumentScopeKind::Task);
            assert!(
                session
                    .transact_entries(move |tx| tx.edit_document_scoped(
                        &definition,
                        scope,
                        None,
                        &(),
                        |_| -> Result<(), DurableError> {
                            panic!("must not initialise terminal owner")
                        }
                    ))
                    .await
                    .is_err()
            );
            session.close().await.unwrap();
        }
    }
    #[tokio::test]
    async fn raw_owner_creation_and_terminal_settlement_in_same_batch_retire_new_documents() {
        let session = DurableSession::open(Box::new(MemoryStorage::new()))
            .await
            .unwrap();
        let state = session.snapshot().await.unwrap();
        let mut write = batch(&state);
        let owner = TaskId::new(1).unwrap();
        let scope = DocumentScope::Task { task_id: owner };
        write.next_id = 3;
        write.tasks.push(task(owner, write.seq, TaskState::Pending));
        write.generic_documents.push(GenericDocumentRecord {
            id: DocumentId::new(2).unwrap(),
            address: address(scope),
            version: 1,
            history: DocumentHistory::Latest,
            fork: DocumentFork::Initial,
            value: json!({"n":1}),
            created_seq: write.seq,
            updated_seq: write.seq,
            retired_seq: None,
        });
        session.commit(write).await.unwrap();
        change_task(&session, owner, TaskState::Running).await;
        let state = session.snapshot().await.unwrap();
        let mut write = batch(&state);
        let mut owner_record = state.tasks[&owner].clone();
        owner_record.state = TaskState::Succeeded;
        owner_record.updated_seq = write.seq;
        owner_record.outcome = Some(json!({}));
        write.tasks.push(owner_record);
        let new = GenericDocumentRecord {
            id: DocumentId::new(state.next_id).unwrap(),
            address: DocumentAddress {
                key: Some("new".into()),
                ..address(scope)
            },
            version: 1,
            history: DocumentHistory::Latest,
            fork: DocumentFork::Initial,
            value: json!({"n":2}),
            created_seq: write.seq,
            updated_seq: write.seq,
            retired_seq: None,
        };
        let new_id = new.id;
        write.next_id += 1;
        write.generic_documents.push(new);
        session.commit(write).await.unwrap();
        let state = session.snapshot().await.unwrap();
        assert_eq!(
            state.generic_documents[&new_id].created_seq,
            state.generic_documents[&new_id].retired_seq.unwrap()
        );
        assert!(
            session
                .documents(DocumentQuery::scoped(scope))
                .await
                .unwrap()
                .items
                .is_empty()
        );
        session.close().await.unwrap();
    }
    #[tokio::test]
    async fn direct_storage_terminal_batches_must_retire_owned_documents_and_identity_cannot_change()
     {
        let session = DurableSession::open(Box::new(MemoryStorage::new()))
            .await
            .unwrap();
        let id = create_task(&session).await;
        let scope = DocumentScope::Task { task_id: id };
        let document = put(&session, scope, 1).await;
        change_task(&session, id, TaskState::Running).await;
        let state = session.snapshot().await.unwrap();
        let mut direct = batch(&state);
        let mut owner = state.tasks[&id].clone();
        owner.state = TaskState::Succeeded;
        owner.updated_seq = direct.seq;
        owner.outcome = Some(json!({}));
        direct.tasks.push(owner);
        let mut copy = state.clone();
        assert!(copy.apply(&direct).is_err());
        assert_eq!(copy, state);
        let mut retired = document.clone();
        retired.updated_seq = direct.seq;
        retired.retired_seq = Some(direct.seq);
        direct.generic_documents.push(retired);
        copy.apply(&direct).unwrap();
        assert!(copy.generic_documents[&document.id].retired_seq.is_some());
        let mut changed = document;
        changed.updated_seq = CommitSeq::new(state.next_seq).unwrap();
        changed.address.scope = DocumentScope::Session;
        assert!(
            session
                .commit(CommitBatch {
                    generic_documents: vec![changed],
                    ..batch(&state)
                })
                .await
                .is_err()
        );
        assert_eq!(session.snapshot().await.unwrap(), state);
        session.close().await.unwrap();
    }
    #[tokio::test]
    async fn scoped_families_migrate_recreate_and_rollback_related_writes() {
        let session = DurableSession::open(Box::new(MemoryStorage::new()))
            .await
            .unwrap();
        let id = create_task(&session).await;
        for scope in [DocumentScope::Session, DocumentScope::Task { task_id: id }] {
            let old = DocumentDefinition::family(
                "custom.scoped.family",
                1,
                DocumentHistory::Latest,
                DocumentFork::Initial,
                |seed: &i64| Ok(json!({"seed":seed})),
            )
            .unwrap()
            .with_scope(scope.kind())
            .unwrap();
            let tx_token = old.clone();
            session
                .transact_entries(move |tx| {
                    tx.edit_document_scoped(&tx_token, scope, Some("one"), &1, |_| Ok(()))?;
                    tx.edit_document_scoped(&tx_token, scope, Some("one"), &99, |value| {
                        assert_eq!(value["seed"], 1);
                        Ok(())
                    })?;
                    tx.edit_document_scoped(&tx_token, scope, Some("two"), &2, |_| Ok(()))
                })
                .await
                .unwrap();
            let newer = DocumentDefinition::family(
                "custom.scoped.family",
                2,
                DocumentHistory::Latest,
                DocumentFork::Initial,
                |seed: &i64| Ok(json!({"seed":seed})),
            )
            .unwrap()
            .with_scope(scope.kind())
            .unwrap()
            .with_migration(|mut value, _| {
                value["upgraded"] = json!(true);
                Ok(value)
            });
            let before = session.snapshot().await.unwrap();
            assert_eq!(
                session
                    .typed_document_scoped(&newer, scope, Some("one"), DocumentPoint::Current)
                    .await
                    .unwrap(),
                Some(json!({"seed":1,"upgraded":true}))
            );
            assert_eq!(session.snapshot().await.unwrap(), before);
            let tx_token = newer.clone();
            let failed: Result<(), DurableError> = session
                .transact_entries(move |tx| {
                    tx.edit_document_scoped(&tx_token, scope, Some("one"), &7, |_| Ok(()))?;
                    tx.append_entry(root(), EntryDraft::new("rolled-back"))?;
                    Err(DurableError::Rejected("rollback scoped migration".into()))
                })
                .await;
            assert!(failed.is_err());
            assert_eq!(session.snapshot().await.unwrap(), before);
            let tx_token = newer.clone();
            session
                .transact_entries(move |tx| {
                    tx.edit_document_scoped(&tx_token, scope, Some("one"), &7, |_| Ok(()))
                })
                .await
                .unwrap();
            assert_eq!(
                session
                    .document(
                        newer.address_scoped(scope, Some("one")).unwrap(),
                        DocumentPoint::Current
                    )
                    .await
                    .unwrap()
                    .unwrap()
                    .version,
                2
            );
            let tx_token = newer.clone();
            session
                .transact_entries(move |tx| {
                    tx.retire_document(&tx_token.address_scoped(scope, Some("one"))?)?;
                    tx.edit_document_scoped(&tx_token, scope, Some("one"), &3, |_| Ok(()))
                })
                .await
                .unwrap();
            assert_eq!(
                session
                    .typed_document_scoped(&newer, scope, Some("one"), DocumentPoint::Current)
                    .await
                    .unwrap(),
                Some(json!({"seed":3}))
            );
        }
        session.close().await.unwrap();
    }
    struct Controlled {
        memory: MemoryStorage,
        mode: Arc<AtomicU8>,
        admitted: Arc<Notify>,
        release: Arc<Notify>,
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
                    1 => return Err(DurableError::Rejected("terminal rejected".into())),
                    2 => return Err(DurableError::Uncertain("terminal uncertain".into())),
                    3 => {
                        self.admitted.notify_one();
                        self.release.notified().await;
                    }
                    _ => {}
                };
                self.memory.commit(claim, batch).await
            })
        }
    }
    async fn controlled() -> (
        Arc<DurableSession>,
        Arc<AtomicU8>,
        Arc<Notify>,
        Arc<Notify>,
        TaskId,
    ) {
        let mode = Arc::new(AtomicU8::new(0));
        let admitted = Arc::new(Notify::new());
        let release = Arc::new(Notify::new());
        let session = Arc::new(
            DurableSession::open(Box::new(Controlled {
                memory: MemoryStorage::new(),
                mode: mode.clone(),
                admitted: admitted.clone(),
                release: release.clone(),
            }))
            .await
            .unwrap(),
        );
        let id = create_task(&session).await;
        put(&session, DocumentScope::Task { task_id: id }, 1).await;
        change_task(&session, id, TaskState::Running).await;
        (session, mode, admitted, release, id)
    }
    fn terminal_batch(state: &StorageSnapshot, id: TaskId) -> CommitBatch {
        let mut write = batch(state);
        let mut record = state.tasks[&id].clone();
        record.state = TaskState::Succeeded;
        record.updated_seq = write.seq;
        record.outcome = Some(json!({}));
        write.tasks.push(record);
        write
    }
    #[tokio::test]
    async fn terminal_storage_rejection_rolls_back_and_uncertainty_ends_watch_without_false_retirement()
     {
        let (session, mode, _, _, id) = controlled().await;
        let before = session.snapshot().await.unwrap();
        let mut watch = session
            .watch_document(address(DocumentScope::Task { task_id: id }))
            .await
            .unwrap();
        mode.store(1, Ordering::SeqCst);
        assert!(session.commit(terminal_batch(&before, id)).await.is_err());
        assert_eq!(session.snapshot().await.unwrap(), before);
        assert!(
            tokio::time::timeout(Duration::from_millis(10), watch.next())
                .await
                .is_err()
        );
        mode.store(2, Ordering::SeqCst);
        assert!(session.commit(terminal_batch(&before, id)).await.is_err());
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(2), watch.next())
                .await
                .unwrap(),
            Some(DocumentEvent::End(DocumentWatchEnd::Poisoned))
        );
        session.close().await.unwrap();
    }
    #[tokio::test]
    async fn terminal_retirement_admission_survives_cancellation_and_unrelated_scopes_stay_live() {
        let (session, mode, admitted, release, id) = controlled().await;
        put(&session, DocumentScope::Session, 2).await;
        let state = session.snapshot().await.unwrap();
        let mut watch = session
            .watch_document(address(DocumentScope::Task { task_id: id }))
            .await
            .unwrap();
        mode.store(3, Ordering::SeqCst);
        let writer = {
            let session = session.clone();
            tokio::spawn(async move { session.commit(terminal_batch(&state, id)).await })
        };
        admitted.notified().await;
        assert!(watch.value().is_some());
        assert!(
            tokio::time::timeout(Duration::from_millis(10), watch.next())
                .await
                .is_err()
        );
        writer.abort();
        assert!(writer.await.unwrap_err().is_cancelled());
        release.notify_one();
        assert!(matches!(
            tokio::time::timeout(Duration::from_secs(2), watch.next())
                .await
                .unwrap(),
            Some(DocumentEvent::Replacement { record: None, .. })
        ));
        assert_eq!(
            watch.next().await,
            Some(DocumentEvent::End(DocumentWatchEnd::Retired))
        );
        assert!(
            session
                .document(address(DocumentScope::Session), DocumentPoint::Current)
                .await
                .unwrap()
                .is_some()
        );
        session.close().await.unwrap();
    }

    #[tokio::test]
    async fn session_documents_survive_reopen_and_neither_session_nor_task_documents_are_fork_copied()
     {
        let dir = std::env::temp_dir().join(format!("rs-ai-scopes-{}", crate::utils::uuidv7()));
        std::fs::create_dir(&dir).unwrap();
        let path = dir.join("journal");
        let session = DurableSession::open(Box::new(JournalStorage::open(&path).unwrap()))
            .await
            .unwrap();
        let session_doc = put(&session, DocumentScope::Session, 7).await;
        let id = create_task(&session).await;
        let task_doc = put(&session, DocumentScope::Task { task_id: id }, 8).await;
        let cut = session
            .append_entry(root(), EntryDraft::new("cut"))
            .await
            .unwrap();
        let child = session
            .fork_conversation(root(), cut.id, ConversationOwnership::Ownerless)
            .await
            .unwrap();
        assert!(
            session
                .documents(DocumentQuery::current(child.id))
                .await
                .unwrap()
                .items
                .is_empty()
        );
        let state = session.snapshot().await.unwrap();
        session.close().await.unwrap();
        let session = DurableSession::open(Box::new(JournalStorage::open(&path).unwrap()))
            .await
            .unwrap();
        assert_eq!(session.snapshot().await.unwrap(), state);
        assert_eq!(
            session
                .document(address(DocumentScope::Session), DocumentPoint::Current)
                .await
                .unwrap(),
            Some(session_doc)
        );
        assert_eq!(
            session
                .document(
                    address(DocumentScope::Task { task_id: id }),
                    DocumentPoint::Current
                )
                .await
                .unwrap(),
            Some(task_doc)
        );
        change_task(&session, id, TaskState::Running).await;
        change_task(&session, id, TaskState::Aborted).await;
        session.close().await.unwrap();
        let session = DurableSession::open(Box::new(JournalStorage::open(&path).unwrap()))
            .await
            .unwrap();
        assert!(
            session
                .document(address(DocumentScope::Session), DocumentPoint::Current)
                .await
                .unwrap()
                .is_some()
        );
        assert!(
            session
                .document(
                    address(DocumentScope::Task { task_id: id }),
                    DocumentPoint::Current
                )
                .await
                .unwrap()
                .is_none()
        );
        session.close().await.unwrap();
        std::fs::remove_dir_all(dir).unwrap();
    }
}
