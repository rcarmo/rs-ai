#[cfg(test)]
mod tests {
    use crate::durable::*;
    use serde_json::json;
    use std::sync::Arc;

    #[tokio::test]
    async fn transaction_reads_then_appends_references_as_one_adopted_commit() {
        let session = DurableSession::open(Box::new(MemoryStorage::new()))
            .await
            .unwrap();
        let conversation = ConversationId::new(1).unwrap();
        let mut watch = session.watch().await.unwrap();
        watch.next().await.unwrap();
        let (mut first, second) = session.transact_entries(move |tx| {
            assert!(tx.entries(conversation, &EntryQuery::default())?.items.is_empty());
            assert!(tx.tasks(None, &TaskQuery::default())?.items.is_empty());
            assert!(tx.submissions(None, &SubmissionQuery::default())?.items.is_empty());
            assert!(tx.entry(conversation, EntryId::new(1)?)?.is_none());
            let mut draft = EntryDraft::new("note"); draft.data = Some(json!({"text":"detached"})); draft.model = Some(vec![crate::user_message("original")]);
            let first = tx.append_entry(conversation, draft)?;
            assert!(matches!(tx.entry(conversation, first.id), Err(DurableError::Rejected(error)) if error == "table read after write"));
            let mut second = EntryDraft::new("summary"); second.head = Some(ContextHead::Entry(first.id));
            second.model = Some(vec![crate::user_message("summary")]);
            second.edits = vec![ContextEdit::Replace { target:first.id, messages:vec![crate::user_message("replacement")] }];
            let second = tx.append_entry(conversation, second)?;
            Ok((first, second))
        }).await.unwrap();
        assert_eq!(first.created_seq, second.created_seq);
        assert_eq!((first.id.get(), second.id.get()), (1, 2));
        assert!(
            matches!(watch.next().await, Some(DurableEvent::Commit(batch)) if batch.entries.len()==2 && batch.seq.get()==1)
        );
        first.value = json!(null);
        assert_eq!(
            session
                .entry(conversation, first.id)
                .await
                .unwrap()
                .unwrap()
                .value["data"]["text"],
            "detached"
        );
        let messages = session.message_context(conversation, None).await.unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(session.snapshot().await.unwrap().next_seq, 2);
        let before = session.snapshot().await.unwrap();
        let count = session
            .transact_entries(move |tx| {
                Ok(tx
                    .entries(conversation, &EntryQuery::default())?
                    .items
                    .len())
            })
            .await
            .unwrap();
        assert_eq!(count, 2);
        assert_eq!(session.snapshot().await.unwrap(), before);
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(10), watch.next())
                .await
                .is_err()
        );
        session.close().await.unwrap();
    }

    #[tokio::test]
    async fn callback_error_panic_and_caught_staging_failure_roll_back() {
        let session = DurableSession::open(Box::new(MemoryStorage::new()))
            .await
            .unwrap();
        let conversation = ConversationId::new(1).unwrap();
        let before = session.snapshot().await.unwrap();
        let mut watch = session.watch().await.unwrap();
        watch.next().await.unwrap();
        let result = session
            .transact_entries(move |tx| {
                tx.append_entry(conversation, EntryDraft::new("staged"))?;
                Err::<(), _>(DurableError::Rejected("callback error".into()))
            })
            .await;
        assert!(matches!(result, Err(DurableError::Rejected(error)) if error=="callback error"));
        assert_eq!(session.snapshot().await.unwrap(), before);
        let result = session
            .transact_entries(move |tx| -> Result<(), DurableError> {
                tx.append_entry(conversation, EntryDraft::new("staged"))?;
                panic!("callback failure")
            })
            .await;
        assert!(matches!(result,Err(DurableError::Rejected(error)) if error.contains("panicked")));
        assert_eq!(session.snapshot().await.unwrap(), before);
        assert!(
            session
                .transact_entries(move |tx| {
                    tx.append_entry(conversation, EntryDraft::new("staged"))?;
                    assert!(
                        tx.append_entry(conversation, EntryDraft::new("user"))
                            .is_err()
                    );
                    Ok(())
                })
                .await
                .is_err()
        );
        assert_eq!(session.snapshot().await.unwrap(), before);
        assert!(
            session
                .transact_entries(move |tx| {
                    let first = tx.append_entry(conversation, EntryDraft::new("staged"))?;
                    let mut draft = EntryDraft::new("foreign-reference");
                    draft.head = Some(ContextHead::Entry(first.id));
                    tx.append_entry(ConversationId::new(2)?, draft)?;
                    Ok(())
                })
                .await
                .is_err()
        );
        assert_eq!(session.snapshot().await.unwrap(), before);
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(10), watch.next())
                .await
                .is_err()
        );
        assert_eq!(
            session
                .append_entry(conversation, EntryDraft::new("valid"))
                .await
                .unwrap()
                .id
                .get(),
            1
        );
        session.close().await.unwrap();
        assert!(matches!(
            session.transact_entries(|_| Ok(())).await,
            Err(DurableError::Closed)
        ));
    }

    #[tokio::test]
    async fn transaction_staging_limits_reject_atomically_even_if_caught() {
        let session = DurableSession::open(Box::new(MemoryStorage::new()))
            .await
            .unwrap();
        let before = session.snapshot().await.unwrap();
        let result = session
            .transact_entries(|tx| {
                tx.append_entry(ConversationId::new(1)?, EntryDraft::new("small"))?;
                let mut data = json!(true);
                for _ in 0..129 {
                    data = json!([data]);
                }
                let mut invalid = EntryDraft::new("deep");
                invalid.data = Some(data);
                assert!(tx.append_entry(ConversationId::new(1)?, invalid).is_err());
                Ok(())
            })
            .await;
        assert!(result.is_err());
        assert_eq!(session.snapshot().await.unwrap(), before);
        session.close().await.unwrap();
    }

    #[tokio::test]
    async fn concurrent_entry_transactions_have_serialized_reads_and_unique_ids() {
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
                    .transact_entries(|tx| {
                        let conversation = ConversationId::new(1)?;
                        let count = tx
                            .entries(
                                conversation,
                                &EntryQuery {
                                    scan: ScanOptions {
                                        limit: 64,
                                        ..Default::default()
                                    },
                                    ..Default::default()
                                },
                            )?
                            .items
                            .len();
                        let first = tx.append_entry(conversation, EntryDraft::new("one"))?;
                        let second = tx.append_entry(conversation, EntryDraft::new("two"))?;
                        assert_eq!(first.id.get() as usize, count + 1);
                        Ok((first, second))
                    })
                    .await
                    .unwrap()
            }));
        }
        let mut records = vec![];
        for job in jobs {
            let (first, second) = job.await.unwrap();
            assert_eq!(first.created_seq, second.created_seq);
            records.extend([first, second]);
        }
        records.sort_by_key(|entry| entry.id);
        for (index, entry) in records.iter().enumerate() {
            assert_eq!(entry.id.get(), index as u64 + 1);
        }
        let state = session.snapshot().await.unwrap();
        assert_eq!(state.next_id, 33);
        assert_eq!(state.next_seq, 17);
        session.close().await.unwrap();
    }

    #[tokio::test]
    async fn entry_transaction_journal_reopens_whole_batch_and_read_only_does_not_write() {
        let root = std::env::temp_dir().join(format!(
            "rs-ai-entry-transaction-{}",
            crate::utils::uuidv7()
        ));
        std::fs::create_dir(&root).unwrap();
        let path = root.join("journal");
        let session = DurableSession::open(Box::new(JournalStorage::open(&path).unwrap()))
            .await
            .unwrap();
        session
            .transact_entries(|tx| {
                let conversation = ConversationId::new(1)?;
                tx.append_entry(conversation, EntryDraft::new("one"))?;
                let mut reset = EntryDraft::new("reset");
                reset.head = Some(ContextHead::SelfEntry(SelfHead::SelfEntry));
                reset.model = Some(vec![crate::user_message("handoff")]);
                tx.append_entry(conversation, reset)?;
                Ok(())
            })
            .await
            .unwrap();
        session.close().await.unwrap();
        let reopened = DurableSession::open(Box::new(JournalStorage::open(&path).unwrap()))
            .await
            .unwrap();
        assert_eq!(
            reopened
                .transact_entries(|tx| Ok(tx
                    .entries(ConversationId::new(1)?, &EntryQuery::default())?
                    .items
                    .len()))
                .await
                .unwrap(),
            2
        );
        assert_eq!(
            reopened.snapshot().await.unwrap().last_seq.unwrap().get(),
            1
        );
        assert_eq!(
            reopened
                .message_context(ConversationId::new(1).unwrap(), None)
                .await
                .unwrap()
                .len(),
            1
        );
        reopened.close().await.unwrap();
        std::fs::remove_dir_all(root).unwrap();
    }
}
