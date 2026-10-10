#[cfg(test)]
mod tests {
    use crate::durable::*;
    use serde_json::json;

    fn batch(id: u64) -> CommitBatch {
        let seq = CommitSeq::new(id).unwrap();
        CommitBatch {
            seq,
            next_id: id + 1,
            next_seq: id + 1,
            entries: vec![EntryRecord {
                id: EntryId::new(id).unwrap(),
                conversation_id: ConversationId::new(1).unwrap(),
                kind: "user".into(),
                value: json!({"text":format!("entry-{id}")}),
                by_task_id: None,
                created_seq: seq,
            }],
            tasks: vec![],
            submissions: vec![],
            documents: vec![],
        }
    }

    #[tokio::test]
    async fn watch_attaches_with_snapshot_then_delivers_only_adopted_commits_and_end() {
        let session = DurableSession::open(Box::new(MemoryStorage::new()))
            .await
            .unwrap();
        session.commit(batch(1)).await.unwrap();
        let mut watch = session.watch().await.unwrap();
        let Some(DurableEvent::Snapshot(mut initial)) = watch.next().await else {
            panic!("initial snapshot")
        };
        assert_eq!(initial.entries.len(), 1);
        initial.entries.clear();
        session.commit(batch(2)).await.unwrap();
        let Some(DurableEvent::Commit(commit)) = watch.next().await else {
            panic!("commit event")
        };
        assert_eq!(commit.seq.get(), 2);
        assert_eq!(session.snapshot().await.unwrap().entries.len(), 2);
        assert!(session.commit(batch(2)).await.is_err());
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(10), watch.next())
                .await
                .is_err()
        );
        session.close().await.unwrap();
        assert_eq!(
            watch.next().await,
            Some(DurableEvent::End(WatchEnd::Closed))
        );
        assert_eq!(watch.next().await, None);
        assert!(matches!(session.watch().await, Err(DurableError::Closed)));
    }

    #[tokio::test]
    async fn stopped_and_dropped_watches_release_subscription_slots() {
        let session = DurableSession::open(Box::new(MemoryStorage::new()))
            .await
            .unwrap();
        let mut watches = Vec::new();
        for _ in 0..64 {
            watches.push(session.watch().await.unwrap());
        }
        assert!(matches!(
            session.watch().await,
            Err(DurableError::Rejected(_))
        ));
        watches[0].stop();
        let watch = session.watch().await.unwrap();
        drop(watch);
        drop(watches);
        let mut watch = session.watch().await.unwrap();
        assert!(matches!(
            watch.next().await,
            Some(DurableEvent::Snapshot(_))
        ));
        session.close().await.unwrap();
        assert_eq!(
            watch.next().await,
            Some(DurableEvent::End(WatchEnd::Closed))
        );
    }

    #[tokio::test]
    async fn slow_watch_overflow_reconciles_snapshot_without_affecting_commits() {
        let session = DurableSession::open(Box::new(MemoryStorage::new()))
            .await
            .unwrap();
        let mut watch = session.watch().await.unwrap();
        for id in 1..=70 {
            session.commit(batch(id)).await.unwrap();
        }
        let Some(DurableEvent::Snapshot(snapshot)) = watch.next().await else {
            panic!("overflow reconciles")
        };
        assert_eq!(snapshot.last_seq.unwrap().get(), 64);
        assert_eq!(snapshot.entries.len(), 64);
        for seq in 65..=70 {
            let Some(DurableEvent::Commit(commit)) = watch.next().await else {
                panic!("post-reconcile delta")
            };
            assert_eq!(commit.seq.get(), seq);
        }
        assert_eq!(session.snapshot().await.unwrap().entries.len(), 70);
        watch.stop();
        assert_eq!(
            watch.next().await,
            Some(DurableEvent::End(WatchEnd::Stopped))
        );
        assert_eq!(watch.next().await, None);
        session.close().await.unwrap();
    }
}
