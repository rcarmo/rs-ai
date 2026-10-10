use crate::durable::storage::{DurableStorage, StorageSnapshot, WriterClaim};
use crate::durable::types::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::{Notify, mpsc, oneshot, watch};

enum StorageCommand {
    Load(oneshot::Sender<Result<StorageSnapshot, DurableError>>),
    Commit(CommitBatch, oneshot::Sender<Result<(), DurableError>>),
    Close(oneshot::Sender<Result<(), DurableError>>),
}

enum SessionCommand {
    Commit(
        CommitBatch,
        oneshot::Sender<Result<StorageSnapshot, DurableError>>,
    ),
    Snapshot(oneshot::Sender<Result<StorageSnapshot, DurableError>>),
}

/// Wall-clock source used for durable task lifecycle timestamps.
pub type LifecycleClock = Arc<dyn Fn() -> i64 + Send + Sync>;

pub struct DurableSession {
    tx: mpsc::Sender<SessionCommand>,
    sealed: Arc<AtomicBool>,
    close_notify: Arc<Notify>,
    close_result: watch::Receiver<Option<Result<(), DurableError>>>,
}

impl DurableSession {
    pub async fn open(storage: Box<dyn DurableStorage>) -> Result<Self, DurableError> {
        Self::open_with_clock(storage, Arc::new(crate::utils::now_millis)).await
    }

    pub async fn open_with_clock(
        storage: Box<dyn DurableStorage>,
        now: LifecycleClock,
    ) -> Result<Self, DurableError> {
        let claim = storage.claim_writer()?;
        let (storage_tx, mut storage_rx) = mpsc::channel::<StorageCommand>(16);
        let storage_task = tokio::spawn(async move {
            storage_worker(storage, claim, &mut storage_rx).await;
        });
        let snapshot = match storage_load(&storage_tx).await {
            Ok(snapshot) => snapshot,
            Err(error) => {
                let (reply, result) = oneshot::channel();
                let _ = storage_tx.send(StorageCommand::Close(reply)).await;
                let _ = result.await;
                drop(storage_tx);
                let _ = storage_task.await;
                return Err(error);
            }
        };

        let sealed = Arc::new(AtomicBool::new(false));
        let close_notify = Arc::new(Notify::new());
        let (close_result_tx, close_result) = watch::channel(None);
        let (tx, mut rx) = mpsc::channel::<SessionCommand>(64);
        let session_storage = storage_tx.clone();
        let worker_sealed = sealed.clone();
        let worker_close_notify = close_notify.clone();

        let (session_result_tx, session_result_rx) = oneshot::channel();
        let session_task = tokio::spawn(async move {
            let result = session_worker(
                snapshot,
                &mut rx,
                &session_storage,
                &worker_sealed,
                &worker_close_notify,
                &now,
            )
            .await;
            let _ = session_result_tx.send(result);
        });
        // This owned supervisor, not any close caller, joins both workers and
        // publishes one common outcome. Dropping every close future is safe.
        tokio::spawn(async move {
            let mut result = session_result_rx
                .await
                .unwrap_or(Err(DurableError::Poisoned));
            if session_task.await.is_err() {
                result = Err(DurableError::Poisoned);
            }
            if storage_task.await.is_err() {
                result = Err(DurableError::Poisoned);
            }
            let _ = close_result_tx.send(Some(result));
        });

        Ok(Self {
            tx,
            sealed,
            close_notify,
            close_result,
        })
    }

    pub async fn commit(&self, batch: CommitBatch) -> Result<StorageSnapshot, DurableError> {
        if self.sealed.load(Ordering::Acquire) {
            return Err(DurableError::Closed);
        }
        let (reply, result) = oneshot::channel();
        self.tx
            .send(SessionCommand::Commit(batch, reply))
            .await
            .map_err(|_| DurableError::Closed)?;
        result.await.unwrap_or(Err(DurableError::Closed))
    }

    pub async fn snapshot(&self) -> Result<StorageSnapshot, DurableError> {
        if self.sealed.load(Ordering::Acquire) {
            return Err(DurableError::Closed);
        }
        let (reply, result) = oneshot::channel();
        self.tx
            .send(SessionCommand::Snapshot(reply))
            .await
            .map_err(|_| DurableError::Closed)?;
        result.await.unwrap_or(Err(DurableError::Closed))
    }

    pub async fn close(&self) -> Result<(), DurableError> {
        let first = !self.sealed.swap(true, Ordering::AcqRel);
        if first {
            // notify_one stores a permit if the worker is settling an admitted
            // command; caller cancellation cannot retract the seal or permit.
            self.close_notify.notify_one();
        }
        let mut result = self.close_result.clone();
        loop {
            if let Some(outcome) = result.borrow().clone() {
                return outcome;
            }
            result.changed().await.map_err(|_| DurableError::Poisoned)?;
        }
    }
}

async fn storage_worker(
    storage: Box<dyn DurableStorage>,
    claim: WriterClaim,
    rx: &mut mpsc::Receiver<StorageCommand>,
) {
    while let Some(command) = rx.recv().await {
        match command {
            StorageCommand::Load(reply) => {
                let _ = reply.send(storage.load(&claim).await);
            }
            StorageCommand::Commit(batch, reply) => {
                let _ = reply.send(storage.commit(&claim, batch).await);
            }
            StorageCommand::Close(reply) => {
                let _ = reply.send(storage.close(&claim).await);
                break;
            }
        }
    }
}

async fn session_worker(
    mut state: StorageSnapshot,
    rx: &mut mpsc::Receiver<SessionCommand>,
    storage: &mpsc::Sender<StorageCommand>,
    sealed: &AtomicBool,
    close_notify: &Notify,
    now: &LifecycleClock,
) -> Result<(), DurableError> {
    let mut poisoned = false;
    loop {
        if sealed.load(Ordering::Acquire) {
            reject_unadmitted(rx);
            return storage_close(storage).await;
        }
        tokio::select! {
            biased;
            _ = close_notify.notified() => {
                if sealed.load(Ordering::Acquire) {
                    reject_unadmitted(rx);
                    return storage_close(storage).await;
                }
            }
            command = rx.recv() => {
                let Some(command) = command else { return storage_close(storage).await; };
                if sealed.load(Ordering::Acquire) {
                    reject(command);
                    reject_unadmitted(rx);
                    return storage_close(storage).await;
                }
                match command {
                    SessionCommand::Commit(mut batch, reply) => {
                        if reply.is_closed() { continue; }
                        if poisoned { let _ = reply.send(Err(DurableError::Poisoned)); continue; }
                        // Stamp at mutation-line admission, before storage settlement.
                        // Existing starts survive recovery; legacy records may be untimed.
                        for task in &mut batch.tasks {
                            if task.started_at.is_none()
                                && let Some(previous) = state.tasks.get(&task.id)
                            {
                                task.started_at = previous.started_at;
                            }
                            if task.state == TaskState::Running && task.started_at.is_none() {
                                task.started_at = Some(now());
                            }
                            if task.state.terminal() && task.ended_at.is_none() {
                                task.ended_at = Some(now());
                            }
                        }
                        let (done, result) = oneshot::channel();
                        if storage.send(StorageCommand::Commit(batch, done)).await.is_err() {
                            poisoned = true;
                            let _ = reply.send(Err(DurableError::Poisoned));
                            continue;
                        }
                        // Admission occurred. This settlement cannot be cancelled by
                        // caller drop or by close; close is observed on the next loop.
                        match result.await.unwrap_or(Err(DurableError::Uncertain("storage worker stopped".into()))) {
                            Ok(()) => match storage_load(storage).await {
                                Ok(adopted) => { state = adopted; let _ = reply.send(Ok(state.clone())); }
                                Err(error) => { poisoned = true; let _ = reply.send(Err(error)); }
                            },
                            Err(DurableError::Uncertain(error)) => {
                                poisoned = true;
                                let _ = reply.send(Err(DurableError::Uncertain(error)));
                            }
                            Err(error) => { let _ = reply.send(Err(error)); }
                        }
                    }
                    SessionCommand::Snapshot(reply) => {
                        let _ = reply.send(if poisoned { Err(DurableError::Poisoned) } else { Ok(state.clone()) });
                    }
                }
            }
        }
    }
}

fn reject(command: SessionCommand) {
    match command {
        SessionCommand::Commit(_, reply) => {
            let _ = reply.send(Err(DurableError::Closed));
        }
        SessionCommand::Snapshot(reply) => {
            let _ = reply.send(Err(DurableError::Closed));
        }
    }
}

fn reject_unadmitted(rx: &mut mpsc::Receiver<SessionCommand>) {
    rx.close();
    while let Ok(command) = rx.try_recv() {
        reject(command);
    }
}

async fn storage_load(tx: &mpsc::Sender<StorageCommand>) -> Result<StorageSnapshot, DurableError> {
    let (reply, result) = oneshot::channel();
    tx.send(StorageCommand::Load(reply))
        .await
        .map_err(|_| DurableError::Closed)?;
    result.await.unwrap_or(Err(DurableError::Closed))
}

async fn storage_close(tx: &mpsc::Sender<StorageCommand>) -> Result<(), DurableError> {
    let (reply, result) = oneshot::channel();
    tx.send(StorageCommand::Close(reply))
        .await
        .map_err(|_| DurableError::Closed)?;
    result.await.unwrap_or(Err(DurableError::Closed))
}
