use super::events::{DurableWatch, WatchEnd, WatchQueue, WatchRegistry, finish_all};
use crate::durable::storage::scan::{
    ConversationQuery, EntryQuery, ScanPage, SubmissionQuery, TaskQuery,
};
use crate::durable::storage::{DurableStorage, StorageSnapshot, WriterClaim};
use crate::durable::types::*;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::{Notify, mpsc, oneshot, watch};

type TransactionCallback = Box<
    dyn FnOnce(&StorageSnapshot) -> Result<(Option<CommitBatch>, TransactionReply), DurableError>
        + Send,
>;
type TransactionReply = Box<dyn FnOnce(Result<(), DurableError>) + Send>;

enum CommitReply {
    Snapshot(oneshot::Sender<Result<StorageSnapshot, DurableError>>),
    Entry(
        EntryRecord,
        oneshot::Sender<Result<EntryRecord, DurableError>>,
    ),
    Transaction(TransactionReply),
}
impl CommitReply {
    fn is_closed(&self) -> bool {
        match self {
            Self::Snapshot(reply) => reply.is_closed(),
            Self::Entry(_, reply) => reply.is_closed(),
            Self::Transaction(_) => false, // callback already admitted
        }
    }
    fn adopted(self, state: &StorageSnapshot) {
        match self {
            Self::Snapshot(reply) => {
                let _ = reply.send(Ok(state.clone()));
            }
            Self::Entry(entry, reply) => {
                let _ = reply.send(Ok(entry));
            }
            Self::Transaction(reply) => reply(Ok(())),
        }
    }
    fn send(self, result: Result<StorageSnapshot, DurableError>) -> Result<(), ()> {
        match self {
            Self::Snapshot(reply) => reply.send(result).map_err(|_| ()),
            Self::Entry(entry, reply) => reply.send(result.map(|_| entry)).map_err(|_| ()),
            Self::Transaction(reply) => {
                reply(result.map(|_| ()));
                Ok(())
            }
        }
    }
}

enum StorageCommand {
    Load(oneshot::Sender<Result<StorageSnapshot, DurableError>>),
    Commit(CommitBatch, oneshot::Sender<Result<(), DurableError>>),
    Close(oneshot::Sender<Result<(), DurableError>>),
}

enum SessionCommand {
    Commit(CommitBatch, CommitReply),
    Transaction(TransactionCallback, oneshot::Sender<DurableError>),
    Append(
        ConversationId,
        super::entries::EntryDraft,
        oneshot::Sender<Result<EntryRecord, DurableError>>,
    ),
    Snapshot(oneshot::Sender<Result<StorageSnapshot, DurableError>>),
    Document(
        super::documents::DocumentAddress,
        super::documents::DocumentPoint,
        oneshot::Sender<Result<Option<super::documents::GenericDocumentRecord>, DurableError>>,
    ),
    Documents(
        super::documents::DocumentQuery,
        oneshot::Sender<Result<ScanPage<super::documents::GenericDocumentRecord>, DurableError>>,
    ),
    Conversation(
        ConversationId,
        oneshot::Sender<Result<Option<ConversationRecord>, DurableError>>,
    ),
    Conversations(
        ConversationQuery,
        oneshot::Sender<Result<ScanPage<ConversationRecord>, DurableError>>,
    ),
    Entry(
        ConversationId,
        EntryId,
        oneshot::Sender<Result<Option<EntryRecord>, DurableError>>,
    ),
    Entries(
        ConversationId,
        EntryQuery,
        oneshot::Sender<Result<ScanPage<EntryRecord>, DurableError>>,
    ),
    Tasks(
        Option<ConversationId>,
        TaskQuery,
        oneshot::Sender<Result<ScanPage<TaskRecord>, DurableError>>,
    ),
    Submissions(
        Option<ConversationId>,
        SubmissionQuery,
        oneshot::Sender<Result<ScanPage<SubmissionRecord>, DurableError>>,
    ),
    Context(
        ConversationId,
        Option<EntryId>,
        oneshot::Sender<Result<Vec<crate::types::Message>, DurableError>>,
    ),
    ContextView(
        ConversationId,
        Option<EntryId>,
        oneshot::Sender<Result<super::context::ContextView, DurableError>>,
    ),
    Watch(oneshot::Sender<Result<DurableWatch, DurableError>>),
    #[cfg(test)]
    CacheStats(oneshot::Sender<(usize, usize)>),
}

/// Wall-clock source used for durable task lifecycle timestamps.
pub type LifecycleClock = Arc<dyn Fn() -> i64 + Send + Sync>;

#[derive(Clone, Debug)]
pub struct SessionSettings {
    /// Retention after a native conversation becomes idle. Zero drops idle
    /// ranges immediately; bounded reuse still applies while tasks are busy.
    pub context_retention: Duration,
}
impl Default for SessionSettings {
    fn default() -> Self {
        Self {
            context_retention: Duration::from_secs(10 * 60),
        }
    }
}

struct ContextCache {
    values: HashMap<ConversationId, (Option<Instant>, super::context::MessageRange, usize)>,
    #[cfg(test)]
    derivations: usize,
}
impl ContextCache {
    fn new() -> Self {
        Self {
            values: HashMap::new(),
            #[cfg(test)]
            derivations: 0,
        }
    }
    fn expire(&mut self, retention: Duration) {
        self.values.retain(|_, (idle_since, _, _)| {
            idle_since.is_none_or(|idle_since| idle_since.elapsed() < retention)
        });
    }
    fn reconcile(&mut self, state: &StorageSnapshot, retention: Duration) {
        self.expire(retention);
        let now = Instant::now();
        self.values.retain(|conversation, (idle_since, _, _)| {
            if busy(state, *conversation) {
                *idle_since = None;
                true
            } else if retention.is_zero() {
                false
            } else {
                idle_since.get_or_insert(now);
                true
            }
        });
    }

    fn retained_bytes(&self) -> usize {
        self.values
            .values()
            .map(|(_, _, bytes)| *bytes)
            .fold(0usize, usize::saturating_add)
    }
    fn deadline(&self, retention: Duration) -> Option<Instant> {
        self.values
            .values()
            .filter_map(|(idle_since, _, _)| {
                idle_since.and_then(|since| since.checked_add(retention))
            })
            .min()
    }
}

fn busy(state: &StorageSnapshot, conversation: ConversationId) -> bool {
    state
        .tasks
        .values()
        .any(|task| task.conversation_id == conversation && !task.state.terminal())
}

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
        Self::open_with_settings(storage, now, SessionSettings::default()).await
    }

    pub async fn open_with_settings(
        storage: Box<dyn DurableStorage>,
        now: LifecycleClock,
        settings: SessionSettings,
    ) -> Result<Self, DurableError> {
        let claim = storage.claim_writer()?;
        let (storage_tx, mut storage_rx) = mpsc::channel::<StorageCommand>(16);
        let storage_task = tokio::spawn(async move {
            storage_worker(storage, claim, &mut storage_rx).await;
        });
        let snapshot = match storage_load(&storage_tx).await {
            Ok(mut snapshot) => {
                snapshot.infer_conversations();
                snapshot
            }
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
                &settings,
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
            .send(SessionCommand::Commit(batch, CommitReply::Snapshot(reply)))
            .await
            .map_err(|_| DurableError::Closed)?;
        result.await.unwrap_or(Err(DurableError::Closed))
    }

    /// Allocate and append a passive generic entry on the session mutation line.
    /// Caller cancellation before admission tombstones the request; afterwards
    /// storage settlement and publication survive caller drop.
    pub async fn append_entry(
        &self,
        conversation: ConversationId,
        draft: super::entries::EntryDraft,
    ) -> Result<EntryRecord, DurableError> {
        if self.sealed.load(Ordering::Acquire) {
            return Err(DurableError::Closed);
        }
        let (reply, result) = oneshot::channel();
        self.tx
            .send(SessionCommand::Append(conversation, draft, reply))
            .await
            .map_err(|_| DurableError::Closed)?;
        result.await.unwrap_or(Err(DurableError::Closed))
    }

    /// Run a synchronous read-then-append callback atomically on the session
    /// line. Callback error/panic discards staged writes; once admitted, caller
    /// drop cannot cancel settlement. Do not block or reenter this session.
    pub async fn transact_entries<T: Send + 'static>(
        &self,
        callback: impl for<'a> FnOnce(
            &mut super::transaction::EntryTransaction<'a>,
        ) -> Result<T, DurableError>
        + Send
        + 'static,
    ) -> Result<T, DurableError> {
        self.transact_entries_scoped(None, callback).await
    }

    /// Attribute staged entries to a nonterminal native task. Appends outside
    /// that task's conversation reject; missing/terminal scopes never run callbacks.
    pub async fn transact_task_entries<T: Send + 'static>(
        &self,
        task_id: TaskId,
        callback: impl for<'a> FnOnce(
            &mut super::transaction::EntryTransaction<'a>,
        ) -> Result<T, DurableError>
        + Send
        + 'static,
    ) -> Result<T, DurableError> {
        self.transact_entries_scoped(Some(task_id), callback).await
    }

    async fn transact_entries_scoped<T: Send + 'static>(
        &self,
        task_id: Option<TaskId>,
        callback: impl for<'a> FnOnce(
            &mut super::transaction::EntryTransaction<'a>,
        ) -> Result<T, DurableError>
        + Send
        + 'static,
    ) -> Result<T, DurableError> {
        if self.sealed.load(Ordering::Acquire) {
            return Err(DurableError::Closed);
        }
        let (reply, mut result) = oneshot::channel();
        let (error_tx, mut errors) = oneshot::channel();
        let operation: TransactionCallback = Box::new(move |state| {
            let mut transaction = super::transaction::EntryTransaction::new(state, task_id)?;
            let value = callback(&mut transaction)?;
            let batch = transaction.finish()?;
            let complete: TransactionReply = Box::new(move |settled| {
                let _ = reply.send(settled.map(|_| value));
            });
            Ok((batch, complete))
        });
        self.tx
            .send(SessionCommand::Transaction(operation, error_tx))
            .await
            .map_err(|_| DurableError::Closed)?;
        tokio::select! {
            // A failed callback drops its result sender; prefer its actual error
            // over interpreting that drop as a closed session.
            biased;
            error = &mut errors => match error {
                Ok(error) => Err(error),
                Err(_) => result.await.unwrap_or(Err(DurableError::Closed)),
            },
            result = &mut result => match result {
                Ok(result) => result,
                Err(_) => Err(errors.await.unwrap_or(DurableError::Closed)),
            },
        }
    }

    pub async fn create_conversation(
        &self,
        ownership: ConversationOwnership,
    ) -> Result<ConversationRecord, DurableError> {
        self.transact_entries(move |tx| tx.create_conversation(ownership))
            .await
    }
    pub async fn fork_conversation(
        &self,
        parent: ConversationId,
        at: EntryId,
        ownership: ConversationOwnership,
    ) -> Result<ConversationRecord, DurableError> {
        self.transact_entries(move |tx| tx.fork_conversation(parent, at, ownership))
            .await
    }
    pub async fn conversation(
        &self,
        id: ConversationId,
    ) -> Result<Option<ConversationRecord>, DurableError> {
        if self.sealed.load(Ordering::Acquire) {
            return Err(DurableError::Closed);
        }
        let (reply, result) = oneshot::channel();
        self.tx
            .send(SessionCommand::Conversation(id, reply))
            .await
            .map_err(|_| DurableError::Closed)?;
        result.await.unwrap_or(Err(DurableError::Closed))
    }
    pub async fn conversations(
        &self,
        query: ConversationQuery,
    ) -> Result<ScanPage<ConversationRecord>, DurableError> {
        if self.sealed.load(Ordering::Acquire) {
            return Err(DurableError::Closed);
        }
        let (reply, result) = oneshot::channel();
        self.tx
            .send(SessionCommand::Conversations(query, reply))
            .await
            .map_err(|_| DurableError::Closed)?;
        result.await.unwrap_or(Err(DurableError::Closed))
    }

    pub async fn documents(
        &self,
        query: super::documents::DocumentQuery,
    ) -> Result<ScanPage<super::documents::GenericDocumentRecord>, DurableError> {
        if self.sealed.load(Ordering::Acquire) {
            return Err(DurableError::Closed);
        }
        let (reply, result) = oneshot::channel();
        self.tx
            .send(SessionCommand::Documents(query, reply))
            .await
            .map_err(|_| DurableError::Closed)?;
        result.await.unwrap_or(Err(DurableError::Closed))
    }

    pub async fn document(
        &self,
        address: super::documents::DocumentAddress,
        point: super::documents::DocumentPoint,
    ) -> Result<Option<super::documents::GenericDocumentRecord>, DurableError> {
        if self.sealed.load(Ordering::Acquire) {
            return Err(DurableError::Closed);
        }
        let (reply, result) = oneshot::channel();
        self.tx
            .send(SessionCommand::Document(address, point, reply))
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

    /// Read one detached entry visible in the native conversation. Missing or
    /// foreign IDs return None; admission serializes with committed appends.
    pub async fn entry(
        &self,
        conversation: ConversationId,
        id: EntryId,
    ) -> Result<Option<EntryRecord>, DurableError> {
        if self.sealed.load(Ordering::Acquire) {
            return Err(DurableError::Closed);
        }
        let (reply, result) = oneshot::channel();
        self.tx
            .send(SessionCommand::Entry(conversation, id, reply))
            .await
            .map_err(|_| DurableError::Closed)?;
        result.await.unwrap_or(Err(DurableError::Closed))
    }

    pub async fn entries(
        &self,
        conversation: ConversationId,
        query: EntryQuery,
    ) -> Result<ScanPage<EntryRecord>, DurableError> {
        if self.sealed.load(Ordering::Acquire) {
            return Err(DurableError::Closed);
        }
        let (reply, result) = oneshot::channel();
        self.tx
            .send(SessionCommand::Entries(conversation, query, reply))
            .await
            .map_err(|_| DurableError::Closed)?;
        result.await.unwrap_or(Err(DurableError::Closed))
    }

    pub async fn tasks(
        &self,
        conversation: ConversationId,
        query: TaskQuery,
    ) -> Result<ScanPage<TaskRecord>, DurableError> {
        self.tasks_in(Some(conversation), query).await
    }

    /// Query one native conversation or all conversations when omitted.
    pub async fn tasks_in(
        &self,
        conversation: Option<ConversationId>,
        query: TaskQuery,
    ) -> Result<ScanPage<TaskRecord>, DurableError> {
        if self.sealed.load(Ordering::Acquire) {
            return Err(DurableError::Closed);
        }
        let (reply, result) = oneshot::channel();
        self.tx
            .send(SessionCommand::Tasks(conversation, query, reply))
            .await
            .map_err(|_| DurableError::Closed)?;
        result.await.unwrap_or(Err(DurableError::Closed))
    }

    pub async fn submissions(
        &self,
        conversation: ConversationId,
        query: SubmissionQuery,
    ) -> Result<ScanPage<SubmissionRecord>, DurableError> {
        self.submissions_in(Some(conversation), query).await
    }

    /// Query one native conversation or all conversations when omitted.
    pub async fn submissions_in(
        &self,
        conversation: Option<ConversationId>,
        query: SubmissionQuery,
    ) -> Result<ScanPage<SubmissionRecord>, DurableError> {
        if self.sealed.load(Ordering::Acquire) {
            return Err(DurableError::Closed);
        }
        let (reply, result) = oneshot::channel();
        self.tx
            .send(SessionCommand::Submissions(conversation, query, reply))
            .await
            .map_err(|_| DurableError::Closed)?;
        result.await.unwrap_or(Err(DurableError::Closed))
    }

    /// Derive native messages on the session line; current reads extend retained
    /// ranges after adoption. Historical reads never replace the current cache.
    pub async fn message_context(
        &self,
        conversation: ConversationId,
        at: Option<EntryId>,
    ) -> Result<Vec<crate::types::Message>, DurableError> {
        if self.sealed.load(Ordering::Acquire) {
            return Err(DurableError::Closed);
        }
        let (reply, result) = oneshot::channel();
        self.tx
            .send(SessionCommand::Context(conversation, at, reply))
            .await
            .map_err(|_| DurableError::Closed)?;
        result.await.unwrap_or(Err(DurableError::Closed))
    }

    /// Detached active entries, aligned contributions and reconstructed messages.
    /// Runs on the session line; historical cuts are inclusive and never dispatch.
    pub async fn context_view(
        &self,
        conversation: ConversationId,
        at: Option<EntryId>,
    ) -> Result<super::context::ContextView, DurableError> {
        if self.sealed.load(Ordering::Acquire) {
            return Err(DurableError::Closed);
        }
        let (reply, result) = oneshot::channel();
        self.tx
            .send(SessionCommand::ContextView(conversation, at, reply))
            .await
            .map_err(|_| DurableError::Closed)?;
        result.await.unwrap_or(Err(DurableError::Closed))
    }

    /// Atomically acquire current state and subscribe to subsequent adoption.
    pub async fn watch(&self) -> Result<DurableWatch, DurableError> {
        if self.sealed.load(Ordering::Acquire) {
            return Err(DurableError::Closed);
        }
        let (reply, result) = oneshot::channel();
        self.tx
            .send(SessionCommand::Watch(reply))
            .await
            .map_err(|_| DurableError::Closed)?;
        result.await.unwrap_or(Err(DurableError::Closed))
    }

    #[cfg(test)]
    pub(crate) async fn context_cache_stats(&self) -> (usize, usize) {
        let (reply, result) = oneshot::channel();
        self.tx
            .send(SessionCommand::CacheStats(reply))
            .await
            .unwrap();
        result.await.unwrap()
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
    settings: &SessionSettings,
) -> Result<(), DurableError> {
    let mut poisoned = false;
    let mut cache = ContextCache::new();
    let mut watches = WatchRegistry::new();
    loop {
        if sealed.load(Ordering::Acquire) {
            finish_all(&mut watches.values, WatchEnd::Closed);
            cache.values.clear();
            reject_unadmitted(rx);
            return storage_close(storage).await;
        }
        let expiry = cache.deadline(settings.context_retention);
        tokio::select! {
            biased;
            _ = async {
                if let Some(expiry) = expiry { tokio::time::sleep_until(expiry.into()).await; }
                else { std::future::pending::<()>().await; }
            } => { cache.expire(settings.context_retention); }
            _ = close_notify.notified() => {
                if sealed.load(Ordering::Acquire) {
                    finish_all(&mut watches.values, WatchEnd::Closed);
                    cache.values.clear();
                    reject_unadmitted(rx);
                    return storage_close(storage).await;
                }
            }
            command = rx.recv() => {
                let Some(command) = command else { finish_all(&mut watches.values, WatchEnd::Closed); cache.values.clear(); return storage_close(storage).await; };
                if sealed.load(Ordering::Acquire) {
                    finish_all(&mut watches.values, WatchEnd::Closed);
                    cache.values.clear();
                    reject(command);
                    reject_unadmitted(rx);
                    return storage_close(storage).await;
                }
                // Assign generic identities on the same line as adoption, never
                // from a caller snapshot that concurrent appends could stale.
                let command = match command {
                    SessionCommand::Transaction(callback, errors) => {
                        if errors.is_closed() { continue; }
                        if poisoned { let _ = errors.send(DurableError::Poisoned); continue; }
                        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| callback(&state))) {
                            Ok(Ok((Some(batch), reply))) => SessionCommand::Commit(batch, CommitReply::Transaction(reply)),
                            Ok(Ok((None, reply))) => { reply(Ok(())); continue; }
                            Ok(Err(error)) => { let _ = errors.send(error); continue; }
                            Err(_) => { let _ = errors.send(DurableError::Rejected("entry transaction callback panicked".into())); continue; }
                        }
                    }
                    SessionCommand::Append(conversation, draft, reply) => {
                        if reply.is_closed() { continue; }
                        if poisoned { let _ = reply.send(Err(DurableError::Poisoned)); continue; }
                        let record = (|| {
                            let id = EntryId::new(state.next_id)?;
                            let seq = CommitSeq::new(state.next_seq)?;
                            let next_id = id.get().checked_add(1).filter(|id| *id <= MAX_ID).ok_or_else(|| DurableError::Range("next_id overflow".into()))?;
                            let next_seq = seq.get().checked_add(1).filter(|seq| *seq <= MAX_ID).ok_or_else(|| DurableError::Range("next_seq overflow".into()))?;
                            Ok((draft.into_record(conversation, id, seq)?, next_id, next_seq))
                        })();
                        match record {
                            Ok((entry, next_id, next_seq)) => SessionCommand::Commit(CommitBatch { generic_documents: vec![], conversations: vec![],
                                seq: entry.created_seq, next_id, next_seq, entries: vec![entry.clone()], tasks: vec![], submissions: vec![], documents: vec![],
                            }, CommitReply::Entry(entry, reply)),
                            Err(error) => { let _ = reply.send(Err(error)); continue; }
                        }
                    }
                    command => command,
                };
                match command {
                    SessionCommand::Append(_, _, _) | SessionCommand::Transaction(_, _) => unreachable!("write normalized to commit"),
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
                        // Native legacy assistants derive model identity from
                        // task.input. Preserve ranges only while that input is
                        // unchanged; lifecycle/checkpoint-only writes are safe.
                        let invalidate = batch.tasks.iter().filter(|task| state.tasks.get(&task.id).is_some_and(|previous| previous.input != task.input))
                            .map(|task| task.conversation_id).collect::<Vec<_>>();
                        watches.values.retain(|watch: &std::sync::Weak<WatchQueue>| watch.upgrade().is_some_and(|watch| watch.active()));
                        let publication = (!watches.values.is_empty()).then(|| {
                            let size = serde_json::to_vec(&batch).map_or(MAX_COMMIT_BYTES, |bytes| bytes.len());
                            (batch.clone(), size)
                        });
                        let (done, result) = oneshot::channel();
                        if storage.send(StorageCommand::Commit(batch, done)).await.is_err() {
                            finish_all(&mut watches.values, WatchEnd::Poisoned);
                            cache.values.clear();
                            poisoned = true;
                            let _ = reply.send(Err(DurableError::Poisoned));
                            continue;
                        }
                        // Admission occurred. This settlement cannot be cancelled by
                        // caller drop or by close; close is observed on the next loop.
                        match result.await.unwrap_or(Err(DurableError::Uncertain("storage worker stopped".into()))) {
                            Ok(()) => match storage_load(storage).await {
                                Ok(adopted) => {
                                    state = adopted;
                                    state.infer_conversations();
                                    if !invalidate.is_empty() {
                                        cache.values.retain(|conversation, _| state.history_bounds(*conversation)
                                            .is_ok_and(|history| !invalidate.iter().any(|changed| history.contains_key(changed))));
                                    }
                                    cache.reconcile(&state, settings.context_retention);
                                    watches.values.retain(|watch: &std::sync::Weak<WatchQueue>| {
                                        if let Some(watch) = watch.upgrade() { if let Some((batch, size)) = &publication { watch.publish(batch, &state, *size); } true } else { false }
                                    });
                                    reply.adopted(&state);
                                }
                                Err(error) => { finish_all(&mut watches.values, WatchEnd::Poisoned); cache.values.clear(); poisoned = true; let _ = reply.send(Err(error)); }
                            },
                            Err(DurableError::Uncertain(error)) => {
                                finish_all(&mut watches.values, WatchEnd::Poisoned);
                                cache.values.clear();
                                poisoned = true;
                                let _ = reply.send(Err(DurableError::Uncertain(error)));
                            }
                            Err(error) => { let _ = reply.send(Err(error)); }
                        }
                    }
                    SessionCommand::Snapshot(reply) => {
                        let _ = reply.send(if poisoned { Err(DurableError::Poisoned) } else { Ok(state.clone()) });
                    }
                    SessionCommand::Documents(query, reply) => {
                        let _ = reply.send(if poisoned { Err(DurableError::Poisoned) } else { state.query_documents(&query) });
                    }
                    SessionCommand::Document(address, point, reply) => {
                        let _ = reply.send(if poisoned { Err(DurableError::Poisoned) } else { state.document(&address, point) });
                    }
                    SessionCommand::Conversation(id, reply) => {
                        let _ = reply.send(if poisoned { Err(DurableError::Poisoned) } else { Ok(state.conversations.get(&id).cloned()) });
                    }
                    SessionCommand::Conversations(query, reply) => {
                        let _ = reply.send(if poisoned { Err(DurableError::Poisoned) } else { state.query_conversations(&query) });
                    }
                    SessionCommand::Entry(conversation, id, reply) => {
                        let result = if poisoned { Err(DurableError::Poisoned) }
                            else { state.visible_entry(conversation, id).map(|entry| entry.cloned()) };
                        let _ = reply.send(result);
                    }
                    SessionCommand::Entries(conversation, query, reply) => {
                        let _ = reply.send(if poisoned { Err(DurableError::Poisoned) } else { state.query_entries(conversation, &query) });
                    }
                    SessionCommand::Tasks(conversation, query, reply) => {
                        let _ = reply.send(if poisoned { Err(DurableError::Poisoned) } else { state.query_tasks_in(conversation, &query) });
                    }
                    SessionCommand::Submissions(conversation, query, reply) => {
                        let _ = reply.send(if poisoned { Err(DurableError::Poisoned) } else { state.query_submissions_in(conversation, &query) });
                    }
                    SessionCommand::Context(conversation, at, reply) => {
                        if poisoned { cache.values.clear(); let _ = reply.send(Err(DurableError::Poisoned)); continue; }
                        cache.expire(settings.context_retention);
                        let is_busy = busy(&state, conversation);
                        if at.is_some() || (settings.context_retention.is_zero() && !is_busy) {
                            #[cfg(test)] { cache.derivations += 1; }
                            let _ = reply.send(super::context::messages(&state, conversation, at));
                            continue;
                        }
                        let retained = cache.values.remove(&conversation);
                        let idle_since = if is_busy { None } else { Some(retained.as_ref().and_then(|(since, _, _)| *since).unwrap_or_else(Instant::now)) };
                        let result = if let Some((_, mut range, bytes)) = retained {
                            match range.extend(&state, conversation) {
                                Ok(changed) => {
                                    #[cfg(test)] { if changed { cache.derivations += 1; } }
                                    let bytes = if changed { range.retained_bytes() } else { Some(bytes) };
                                    Ok(Some((range, bytes)))
                                }
                                Err(error) => Err(error),
                            }
                        } else {
                            #[cfg(test)] { cache.derivations += 1; }
                            super::context::MessageRange::build(&state, conversation).map(|range| range.map(|range| {
                                let bytes = range.retained_bytes();
                                (range, bytes)
                            }))
                        };
                        match result {
                            Ok(Some((range, bytes))) => {
                                let messages = range.messages.clone();
                                if !messages.is_empty() && cache.values.len() < 64 && let Some(bytes) = bytes
                                    && cache.retained_bytes().saturating_add(bytes) <= MAX_COMMIT_BYTES
                                {
                                    cache.values.insert(conversation, (idle_since, range, bytes));
                                }
                                let _ = reply.send(Ok(messages));
                            }
                            Ok(None) => { let _ = reply.send(Ok(Vec::new())); }
                            Err(error) => { let _ = reply.send(Err(error)); }
                        }
                    }
                    SessionCommand::ContextView(conversation, at, reply) => {
                        let result = if poisoned { Err(DurableError::Poisoned) }
                            else { super::context::view(&state, conversation, at) };
                        let _ = reply.send(result);
                    }
                    SessionCommand::Watch(reply) => {
                        if poisoned { let _ = reply.send(Err(DurableError::Poisoned)); continue; }
                        watches.values.retain(|watch| watch.upgrade().is_some_and(|watch| watch.active()));
                        if watches.values.len() >= 64 { let _ = reply.send(Err(DurableError::Rejected("too many active watches".into()))); continue; }
                        let queue = WatchQueue::new(state.clone());
                        watches.values.push(Arc::downgrade(&queue));
                        let _ = reply.send(Ok(DurableWatch { queue }));
                    }
                    #[cfg(test)]
                    SessionCommand::CacheStats(reply) => { cache.expire(settings.context_retention); let _ = reply.send((cache.values.len(), cache.derivations)); }
                }
            }
        }
    }
}

fn reject(command: SessionCommand) {
    match command {
        SessionCommand::Transaction(_, errors) => {
            let _ = errors.send(DurableError::Closed);
        }
        SessionCommand::Append(_, _, reply) => {
            let _ = reply.send(Err(DurableError::Closed));
        }
        SessionCommand::Commit(_, reply) => {
            let _ = reply.send(Err(DurableError::Closed));
        }
        SessionCommand::Snapshot(reply) => {
            let _ = reply.send(Err(DurableError::Closed));
        }
        SessionCommand::Documents(_, reply) => {
            let _ = reply.send(Err(DurableError::Closed));
        }
        SessionCommand::Document(_, _, reply) => {
            let _ = reply.send(Err(DurableError::Closed));
        }
        SessionCommand::Conversation(_, reply) => {
            let _ = reply.send(Err(DurableError::Closed));
        }
        SessionCommand::Conversations(_, reply) => {
            let _ = reply.send(Err(DurableError::Closed));
        }
        SessionCommand::Entry(_, _, reply) => {
            let _ = reply.send(Err(DurableError::Closed));
        }
        SessionCommand::Entries(_, _, reply) => {
            let _ = reply.send(Err(DurableError::Closed));
        }
        SessionCommand::Tasks(_, _, reply) => {
            let _ = reply.send(Err(DurableError::Closed));
        }
        SessionCommand::Submissions(_, _, reply) => {
            let _ = reply.send(Err(DurableError::Closed));
        }
        SessionCommand::Context(_, _, reply) => {
            let _ = reply.send(Err(DurableError::Closed));
        }
        SessionCommand::ContextView(_, _, reply) => {
            let _ = reply.send(Err(DurableError::Closed));
        }
        SessionCommand::Watch(reply) => {
            let _ = reply.send(Err(DurableError::Closed));
        }
        #[cfg(test)]
        SessionCommand::CacheStats(reply) => {
            let _ = reply.send((0, 0));
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
