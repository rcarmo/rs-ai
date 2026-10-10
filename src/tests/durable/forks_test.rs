#[cfg(test)]
mod tests {
    use crate::durable::*;
    use serde_json::json;

    async fn append(
        session: &DurableSession,
        conversation: ConversationId,
        text: &str,
    ) -> EntryRecord {
        let mut draft = EntryDraft::new("note");
        draft.model = Some(vec![crate::user_message(text)]);
        session.append_entry(conversation, draft).await.unwrap()
    }
    fn texts(messages: Vec<crate::types::Message>) -> Vec<String> {
        messages
            .into_iter()
            .map(|message| match message.content.into_iter().next() {
                Some(crate::types::ContentBlock::Text { text, .. }) => text,
                _ => String::new(),
            })
            .collect()
    }
    #[tokio::test]
    async fn fork_cutoff_hides_later_parent_and_sibling_writes_and_preserves_ranges() {
        let session = DurableSession::open(Box::new(MemoryStorage::new()))
            .await
            .unwrap();
        let root = ConversationId::new(1).unwrap();
        let first = append(&session, root, "first").await;
        let second = append(&session, root, "second").await;
        let child = session
            .fork_conversation(root, second.id, ConversationOwnership::Ownerless)
            .await
            .unwrap();
        assert_eq!(child.parent.as_ref().unwrap().at, second.id);
        assert_eq!(
            texts(session.message_context(child.id, None).await.unwrap()),
            ["first", "second"]
        );
        let parent_tail = append(&session, root, "parent-later").await;
        assert!(
            session
                .entry(child.id, parent_tail.id)
                .await
                .unwrap()
                .is_none()
        );
        let local = append(&session, child.id, "child-local").await;
        let sibling = session
            .fork_conversation(root, first.id, ConversationOwnership::Ownerless)
            .await
            .unwrap();
        append(&session, sibling.id, "sibling-local").await;
        assert_eq!(
            texts(session.message_context(child.id, None).await.unwrap()),
            ["first", "second", "child-local"]
        );
        let nested = session
            .fork_conversation(child.id, first.id, ConversationOwnership::Ownerless)
            .await
            .unwrap();
        assert_eq!(
            texts(session.message_context(nested.id, None).await.unwrap()),
            ["first"]
        );
        assert_eq!(
            session
                .entry(child.id, first.id)
                .await
                .unwrap()
                .unwrap()
                .conversation_id,
            root
        );
        let mut page = session
            .entries(
                child.id,
                EntryQuery {
                    scan: ScanOptions {
                        limit: 1,
                        ..Default::default()
                    },
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert_eq!(page.items[0].id, local.id);
        page.items[0].value = json!(null);
        let rest = session
            .entries(
                child.id,
                EntryQuery {
                    scan: ScanOptions {
                        cursor: page.cursor,
                        ..Default::default()
                    },
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert_eq!(
            rest.items.iter().map(|entry| entry.id).collect::<Vec<_>>(),
            [second.id, first.id]
        );
        assert_eq!(
            texts(
                session
                    .message_context(child.id, Some(first.id))
                    .await
                    .unwrap()
            ),
            ["first"]
        );
        assert!(
            session
                .message_context(child.id, Some(parent_tail.id))
                .await
                .is_err()
        );
        session.close().await.unwrap();
    }
    #[tokio::test]
    async fn fork_context_edits_ancestor_without_mutating_parent_and_reopens() {
        let root_dir = std::env::temp_dir().join(format!("rs-ai-fork-{}", crate::utils::uuidv7()));
        std::fs::create_dir(&root_dir).unwrap();
        let path = root_dir.join("journal");
        let session = DurableSession::open(Box::new(JournalStorage::open(&path).unwrap()))
            .await
            .unwrap();
        let root = ConversationId::new(1).unwrap();
        let first = append(&session, root, "ancestor").await;
        let child = session
            .transact_entries(move |tx| {
                let child =
                    tx.fork_conversation(root, first.id, ConversationOwnership::Ownerless)?;
                let mut summary = EntryDraft::new("summary");
                summary.head = Some(ContextHead::Entry(first.id));
                summary.model = Some(vec![crate::user_message("summary")]);
                summary.edits = vec![ContextEdit::Replace {
                    target: first.id,
                    messages: vec![crate::user_message("replacement")],
                }];
                tx.append_entry(child.id, summary)?;
                Ok(child)
            })
            .await
            .unwrap();
        assert_eq!(
            texts(session.message_context(child.id, None).await.unwrap()),
            ["summary", "replacement"]
        );
        assert_eq!(
            texts(session.message_context(root, None).await.unwrap()),
            ["ancestor"]
        );
        session.close().await.unwrap();
        let reopened = DurableSession::open(Box::new(JournalStorage::open(&path).unwrap()))
            .await
            .unwrap();
        assert_eq!(
            reopened.conversation(child.id).await.unwrap().unwrap(),
            child
        );
        assert_eq!(
            texts(reopened.message_context(child.id, None).await.unwrap()),
            ["summary", "replacement"]
        );
        reopened
            .transact_entries(move |tx| {
                assert_eq!(tx.entry(child.id, first.id)?.unwrap().conversation_id, root);
                Ok(())
            })
            .await
            .unwrap();
        reopened.close().await.unwrap();
        std::fs::remove_dir_all(root_dir).unwrap();
    }
    #[tokio::test]
    async fn invalid_fork_cutoffs_and_foreign_context_references_reject_atomically() {
        let session = DurableSession::open(Box::new(MemoryStorage::new()))
            .await
            .unwrap();
        let root = ConversationId::new(1).unwrap();
        let first = append(&session, root, "first").await;
        let child = session
            .fork_conversation(root, first.id, ConversationOwnership::Ownerless)
            .await
            .unwrap();
        let hidden = append(&session, root, "hidden").await;
        let before = session.snapshot().await.unwrap();
        assert!(
            session
                .fork_conversation(child.id, hidden.id, ConversationOwnership::Ownerless)
                .await
                .is_err()
        );
        assert!(
            session
                .fork_conversation(
                    ConversationId::new(999).unwrap(),
                    first.id,
                    ConversationOwnership::Ownerless
                )
                .await
                .is_err()
        );
        assert!(
            session
                .fork_conversation(
                    root,
                    EntryId::new(999).unwrap(),
                    ConversationOwnership::Ownerless
                )
                .await
                .is_err()
        );
        let mut bad = EntryDraft::new("bad-edit");
        bad.edits = vec![ContextEdit::Omit { target: hidden.id }];
        assert!(session.append_entry(child.id, bad).await.is_err());
        assert_eq!(session.snapshot().await.unwrap(), before);
        session.close().await.unwrap();
    }
}
