#[cfg(test)]
mod tests {
    use crate::durable::storage::{StorageFuture, WriterClaim};
    use crate::durable::*;
    use serde_json::json;
    use std::sync::{Arc, Mutex};
    use tokio::sync::Notify;

    fn batch(seq: u64) -> CommitBatch {
        let s = CommitSeq::new(seq).unwrap();
        CommitBatch {
            seq: s,
            next_id: 2,
            next_seq: seq + 1,
            entries: vec![EntryRecord {
                id: EntryId::new(1).unwrap(),
                conversation_id: ConversationId::new(1).unwrap(),
                kind: "user".into(),
                value: json!({"seq":seq}),
                by_task_id: None,
                created_seq: s,
            }],
            tasks: vec![],
            submissions: vec![],
            documents: vec![],
        }
    }

    struct BarrierStorage {
        inner: MemoryStorage,
        inner_claim: WriterClaim,
        admitted: Arc<Notify>,
        release: Arc<Notify>,
        commits: Arc<Mutex<usize>>,
        uncertain: bool,
    }
    impl BarrierStorage {
        fn new(uncertain: bool) -> (Self, Arc<Notify>, Arc<Notify>, Arc<Mutex<usize>>) {
            let inner = MemoryStorage::new();
            let inner_claim = inner.claim_writer().unwrap();
            let admitted = Arc::new(Notify::new());
            let release = Arc::new(Notify::new());
            let commits = Arc::new(Mutex::new(0));
            (
                Self {
                    inner,
                    inner_claim,
                    admitted: admitted.clone(),
                    release: release.clone(),
                    commits: commits.clone(),
                    uncertain,
                },
                admitted,
                release,
                commits,
            )
        }
    }
    impl DurableStorage for BarrierStorage {
        fn claim_writer(&self) -> Result<WriterClaim, DurableError> {
            Ok(WriterClaim::fresh())
        }
        fn load<'a>(&'a self, _claim: &'a WriterClaim) -> StorageFuture<'a, StorageSnapshot> {
            self.inner.load(&self.inner_claim)
        }
        fn commit<'a>(&'a self, _claim: &'a WriterClaim, b: CommitBatch) -> StorageFuture<'a, ()> {
            Box::pin(async move {
                *self.commits.lock().unwrap() += 1;
                self.admitted.notify_waiters();
                self.release.notified().await;
                if self.uncertain {
                    return Err(DurableError::Uncertain("injected".into()));
                }
                self.inner.commit(&self.inner_claim, b).await
            })
        }
        fn close<'a>(&'a self, _claim: &'a WriterClaim) -> StorageFuture<'a, ()> {
            self.inner.close(&self.inner_claim)
        }
    }

    #[tokio::test]
    async fn caller_drop_after_admission_does_not_abandon_settlement_or_ordering() {
        let (storage, admitted, release, commits) = BarrierStorage::new(false);
        let session = Arc::new(DurableSession::open(Box::new(storage)).await.unwrap());
        let worker = session.clone();
        let task = tokio::spawn(async move { worker.commit(batch(1)).await });
        admitted.notified().await;
        task.abort();
        let _ = task.await;
        let close_session = session.clone();
        let close = tokio::spawn(async move { close_session.close().await });
        tokio::task::yield_now().await;
        assert!(!close.is_finished());
        release.notify_waiters();
        assert!(close.await.unwrap().is_ok());
        assert_eq!(*commits.lock().unwrap(), 1);
    }

    #[tokio::test]
    async fn close_seals_admission_waits_and_repeated_callers_share_outcome() {
        let (storage, admitted, release, _) = BarrierStorage::new(false);
        let session = Arc::new(DurableSession::open(Box::new(storage)).await.unwrap());
        let s = session.clone();
        let commit = tokio::spawn(async move { s.commit(batch(1)).await });
        admitted.notified().await;
        let s1 = session.clone();
        let close1 = tokio::spawn(async move { s1.close().await });
        let s2 = session.clone();
        let close2 = tokio::spawn(async move { s2.close().await });
        tokio::task::yield_now().await;
        assert!(matches!(
            session.commit(batch(2)).await,
            Err(DurableError::Closed)
        ));
        assert!(!close1.is_finished());
        release.notify_waiters();
        assert!(commit.await.unwrap().is_ok());
        assert!(close1.await.unwrap().is_ok());
        assert!(close2.await.unwrap().is_ok());
    }

    #[tokio::test]
    async fn dropped_close_caller_does_not_cancel_owned_close() {
        let (storage, admitted, release, _) = BarrierStorage::new(false);
        let session = Arc::new(DurableSession::open(Box::new(storage)).await.unwrap());
        let s = session.clone();
        let commit = tokio::spawn(async move { s.commit(batch(1)).await });
        admitted.notified().await;
        let s = session.clone();
        let dropped = tokio::spawn(async move { s.close().await });
        tokio::task::yield_now().await;
        dropped.abort();
        let _ = dropped.await;
        release.notify_waiters();
        assert!(commit.await.unwrap().is_ok());
        assert!(session.close().await.is_ok());
    }

    #[tokio::test]
    async fn uncertain_commit_poisons_reads_and_later_dispatch() {
        let (storage, admitted, release, _) = BarrierStorage::new(true);
        let session = Arc::new(DurableSession::open(Box::new(storage)).await.unwrap());
        let s = session.clone();
        let task = tokio::spawn(async move { s.commit(batch(1)).await });
        admitted.notified().await;
        release.notify_waiters();
        assert!(matches!(
            task.await.unwrap(),
            Err(DurableError::Uncertain(_))
        ));
        assert!(matches!(
            session.snapshot().await,
            Err(DurableError::Poisoned)
        ));
        assert!(matches!(
            session.commit(batch(1)).await,
            Err(DurableError::Poisoned)
        ));
        assert!(session.close().await.is_ok());
    }

    #[tokio::test]
    async fn actual_journal_admission_drop_close_and_successor_fence() {
        let path =
            std::env::temp_dir().join(format!("rs-ai-session-journal-{}", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let storage = JournalStorage::open(&path).unwrap();
        let (reached, release) = storage.inject_commit_barrier();
        let session = Arc::new(DurableSession::open(Box::new(storage)).await.unwrap());

        let first_session = session.clone();
        let first = tokio::spawn(async move { first_session.commit(batch(1)).await });
        reached.await.unwrap();
        first.abort();
        let _ = first.await;

        let queued_session = session.clone();
        let queued = tokio::spawn(async move { queued_session.commit(batch(2)).await });
        let close_session = session.clone();
        let close = tokio::spawn(async move { close_session.close().await });
        tokio::task::yield_now().await;
        assert!(matches!(
            session.commit(batch(2)).await,
            Err(DurableError::Closed)
        ));
        release.send(()).unwrap();
        assert!(matches!(queued.await.unwrap(), Err(DurableError::Closed)));
        assert!(close.await.unwrap().is_ok());

        let reopened = JournalStorage::open(&path).unwrap();
        let claim = reopened.claim_writer().unwrap();
        assert_eq!(reopened.load(&claim).await.unwrap().entries.len(), 1);
        reopened.close(&claim).await.unwrap();
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn dropped_before_dequeue_is_tombstoned() {
        let (storage, admitted, release, commits) = BarrierStorage::new(false);
        let session = Arc::new(DurableSession::open(Box::new(storage)).await.unwrap());
        let first = session.clone();
        let task = tokio::spawn(async move { first.commit(batch(1)).await });
        admitted.notified().await;
        let second = session.clone();
        let queued = tokio::spawn(async move { second.commit(batch(2)).await });
        queued.abort();
        let _ = queued.await;
        release.notify_waiters();
        assert!(task.await.unwrap().is_ok());
        tokio::task::yield_now().await;
        assert_eq!(*commits.lock().unwrap(), 1);
        session.close().await.unwrap();
    }
}
