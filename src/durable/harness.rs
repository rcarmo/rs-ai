use crate::durable::model::{
    DurableMessage, DurableModelRunner, ModelIntent, ModelRun, ModelTerminal, PinnedModel,
    PinnedOptions,
};
use crate::durable::session::DurableSession;
use crate::durable::storage::DurableStorage;
use crate::durable::submission::{
    SubmissionHandle, SubmissionView, SubmitRequest, build_initial_batch, reacquire, view,
};
use crate::durable::tool::{DurableToolRegistry, ToolIntent, prepare_intent, replay_registration};
use crate::durable::types::*;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::{Mutex, Notify, oneshot, watch};

const ROOT_CONVERSATION_ID: u64 = 1;
type AbortResult = Option<Result<(), DurableError>>;
type AbortReceiver = watch::Receiver<AbortResult>;
type AbortMap = HashMap<(SubmissionId, TaskId), AbortReceiver>;

/// Inclusive context cut for the native single-conversation text subset.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ContextOptions {
    pub at: Option<EntryId>,
}

pub struct DurableHarness {
    inner: Arc<Inner>,
}

struct Inner {
    session: Arc<DurableSession>,
    runner: Arc<dyn DurableModelRunner>,
    model: PinnedModel,
    options: PinnedOptions,
    tools: Arc<DurableToolRegistry>,
    sealed: AtomicBool,
    operations: Mutex<()>,
    admissions: Mutex<Vec<tokio::task::JoinHandle<()>>>,
    scheduled: Mutex<HashSet<TaskId>>,
    running: Mutex<HashSet<TaskId>>,
    tool_cancels: Mutex<HashMap<TaskId, watch::Sender<bool>>>,
    executor_notify: Notify,
    executor_stop: AtomicBool,
    executor_task: Mutex<Option<tokio::task::JoinHandle<()>>>,
    revision_tx: watch::Sender<u64>,
    executor_error_tx: watch::Sender<Option<DurableError>>,
    executor_error_rx: watch::Receiver<Option<DurableError>>,
    close_tx: watch::Sender<Option<Result<(), DurableError>>>,
    close_rx: watch::Receiver<Option<Result<(), DurableError>>>,
    aborts: Mutex<AbortMap>,
    #[cfg(test)]
    phase_barrier: Mutex<Option<(oneshot::Sender<()>, oneshot::Receiver<()>)>>,
    #[cfg(test)]
    tool_phase_barrier: Mutex<Option<(oneshot::Sender<()>, oneshot::Receiver<()>)>>,
    #[cfg(test)]
    successor_phase_barrier: Mutex<Option<(oneshot::Sender<()>, oneshot::Receiver<()>)>>,
}

impl DurableHarness {
    pub async fn open(
        storage: Box<dyn DurableStorage>,
        runner: Arc<dyn DurableModelRunner>,
        model: PinnedModel,
        options: PinnedOptions,
    ) -> Result<Self, DurableError> {
        Self::open_with_tools(
            storage,
            runner,
            model,
            options,
            Arc::new(DurableToolRegistry::default()),
        )
        .await
    }

    pub async fn open_with_tools(
        storage: Box<dyn DurableStorage>,
        runner: Arc<dyn DurableModelRunner>,
        model: PinnedModel,
        options: PinnedOptions,
        tools: Arc<DurableToolRegistry>,
    ) -> Result<Self, DurableError> {
        model.validate()?;
        options.validate()?;
        tools.seal();
        let session = Arc::new(DurableSession::open(storage).await?);
        if let Err(error) = reconcile_running(&session).await {
            let _ = session.close().await;
            return Err(error);
        }
        let (revision_tx, _) = watch::channel(0u64);
        let (executor_error_tx, executor_error_rx) = watch::channel(None);
        let (close_tx, close_rx) = watch::channel(None);
        let inner = Arc::new(Inner {
            session,
            runner,
            model,
            options,
            tools,
            sealed: AtomicBool::new(false),
            operations: Mutex::new(()),
            admissions: Mutex::new(Vec::new()),
            scheduled: Mutex::new(HashSet::new()),
            running: Mutex::new(HashSet::new()),
            tool_cancels: Mutex::new(HashMap::new()),
            executor_notify: Notify::new(),
            executor_stop: AtomicBool::new(false),
            executor_task: Mutex::new(None),
            revision_tx,
            executor_error_tx,
            executor_error_rx,
            close_tx,
            close_rx,
            aborts: Mutex::new(HashMap::new()),
            #[cfg(test)]
            phase_barrier: Mutex::new(None),
            #[cfg(test)]
            tool_phase_barrier: Mutex::new(None),
            #[cfg(test)]
            successor_phase_barrier: Mutex::new(None),
        });
        let worker_inner = inner.clone();
        let worker = tokio::spawn(async move { executor_worker(worker_inner).await });
        let supervisor_inner = inner.clone();
        *inner.executor_task.lock().await = Some(tokio::spawn(async move {
            let result = match worker.await {
                Ok(result) => result,
                Err(_) => Err(DurableError::Poisoned),
            };
            if let Err(error) = result {
                if supervisor_inner.executor_error_rx.borrow().is_none() {
                    let _ = supervisor_inner.executor_error_tx.send(Some(error));
                }
                supervisor_inner
                    .executor_stop
                    .store(true, Ordering::Release);
                bump_revision(&supervisor_inner);
            }
        }));
        Ok(Self { inner })
    }

    pub async fn submit(&self, request: SubmitRequest) -> Result<SubmissionHandle, DurableError> {
        self.start_submission(request).await
    }

    pub async fn follow_up(
        &self,
        request: SubmitRequest,
    ) -> Result<SubmissionHandle, DurableError> {
        self.start_submission(request).await
    }

    async fn start_submission(
        &self,
        request: SubmitRequest,
    ) -> Result<SubmissionHandle, DurableError> {
        self.ensure_open()?;
        let (reply, result) = oneshot::channel();
        let mut admissions = self.inner.admissions.lock().await;
        self.ensure_open()?;
        let inner = self.inner.clone();
        admissions.push(tokio::spawn(async move {
            let outcome = admit_submission(inner, request).await;
            let _ = reply.send(outcome);
        }));
        drop(admissions);
        result.await.unwrap_or(Err(DurableError::Poisoned))
    }

    pub async fn resume(&self, handle: SubmissionHandle) -> Result<(), DurableError> {
        self.ensure_open()?;
        let _operation = self.inner.operations.lock().await;
        self.ensure_open()?;
        let snapshot = self.inner.session.snapshot().await?;
        let submission = snapshot
            .submissions
            .get(&handle.id)
            .ok_or_else(|| DurableError::Rejected("unknown submission".into()))?;
        if submission.status == "pending" {
            wake(&self.inner).await?;
        }
        Ok(())
    }

    pub async fn abort(&self, handle: SubmissionHandle) -> Result<(), DurableError> {
        self.ensure_open()?;
        let mut result = {
            let mut aborts = self.inner.aborts.lock().await;
            let key = (handle.id, handle.task_id);
            if let Some(existing) = aborts.get(&key) {
                existing.clone()
            } else {
                let (tx, rx) = watch::channel(None);
                aborts.insert(key, rx.clone());
                let inner = self.inner.clone();
                tokio::spawn(async move {
                    let outcome = abort_owned(inner, handle).await;
                    let _ = tx.send(Some(outcome));
                });
                rx
            }
        };
        loop {
            if let Some(outcome) = result.borrow().clone() {
                return outcome;
            }
            result.changed().await.map_err(|_| DurableError::Poisoned)?;
        }
    }

    pub async fn wait(&self, handle: SubmissionHandle) -> Result<SubmissionView, DurableError> {
        let mut revision = self.inner.revision_tx.subscribe();
        let mut executor_error = self.inner.executor_error_rx.clone();
        loop {
            if let Some(error) = executor_error.borrow().clone() {
                return Err(error);
            }
            let snapshot = self.inner.session.snapshot().await?;
            let result = view(&snapshot, conversation_id()?, handle.id)?;
            if result.status != "pending" {
                return Ok(result);
            }
            tokio::select! {
                changed = revision.changed() => changed.map_err(|_| DurableError::Closed)?,
                changed = executor_error.changed() => changed.map_err(|_| DurableError::Closed)?,
            }
        }
    }

    pub async fn inspect(&self, id: SubmissionId) -> Result<SubmissionView, DurableError> {
        let snapshot = self.inner.session.snapshot().await?;
        view(&snapshot, conversation_id()?, id)
    }

    pub async fn context(&self) -> Result<Vec<DurableMessage>, DurableError> {
        self.context_with_options(ContextOptions::default()).await
    }

    pub async fn context_with_options(
        &self,
        options: ContextOptions,
    ) -> Result<Vec<DurableMessage>, DurableError> {
        let snapshot = self.inner.session.snapshot().await?;
        context_from_snapshot(&snapshot, options)
    }

    pub async fn inspect_documents(
        &self,
    ) -> Result<std::collections::BTreeMap<String, Value>, DurableError> {
        let conversation = conversation_id()?;
        let snapshot = self.inner.session.snapshot().await?;
        Ok(snapshot
            .documents
            .iter()
            .filter(|((id, _), _)| *id == conversation)
            .map(|((_, kind), document)| (kind.clone(), document.value.clone()))
            .collect())
    }

    pub async fn passive_write(&self, content: String) -> Result<EntryId, DurableError> {
        self.ensure_open()?;
        if content.is_empty() || content.len() > MAX_SUBMISSION_BYTES {
            return Err(DurableError::Rejected("invalid passive content".into()));
        }
        let _operation = self.inner.operations.lock().await;
        self.ensure_open()?;
        let snapshot = self.inner.session.snapshot().await?;
        if snapshot
            .submissions
            .values()
            .any(|value| value.status == "pending")
        {
            return Err(DurableError::Rejected("conversation busy".into()));
        }
        let seq = CommitSeq::new(snapshot.next_seq)?;
        let id = EntryId::new(snapshot.next_id)?;
        self.inner
            .session
            .commit(CommitBatch {
                seq,
                next_id: increment(id.get())?,
                next_seq: increment(seq.get())?,
                entries: vec![EntryRecord {
                    id,
                    conversation_id: conversation_id()?,
                    kind: "user".into(),
                    value: json!({"text":content,"passive":true}),
                    by_task_id: None,
                    created_seq: seq,
                }],
                tasks: vec![],
                submissions: vec![],
                documents: vec![],
            })
            .await?;
        bump_revision(&self.inner);
        Ok(id)
    }

    #[cfg(test)]
    pub(crate) async fn test_snapshot(
        &self,
    ) -> Result<crate::durable::storage::StorageSnapshot, DurableError> {
        self.inner.session.snapshot().await
    }

    #[cfg(test)]
    pub(crate) fn is_sealed(&self) -> bool {
        self.inner.sealed.load(Ordering::Acquire)
    }

    #[cfg(test)]
    pub(crate) async fn inject_successor_phase_barrier(
        &self,
    ) -> (oneshot::Receiver<()>, oneshot::Sender<()>) {
        let (selected_tx, selected_rx) = oneshot::channel();
        let (release_tx, release_rx) = oneshot::channel();
        *self.inner.successor_phase_barrier.lock().await = Some((selected_tx, release_rx));
        (selected_rx, release_tx)
    }

    #[cfg(test)]
    pub(crate) async fn inject_tool_phase_barrier(
        &self,
    ) -> (oneshot::Receiver<()>, oneshot::Sender<()>) {
        let (selected_tx, selected_rx) = oneshot::channel();
        let (release_tx, release_rx) = oneshot::channel();
        *self.inner.tool_phase_barrier.lock().await = Some((selected_tx, release_rx));
        (selected_rx, release_tx)
    }

    #[cfg(test)]
    pub(crate) async fn inject_phase_barrier(
        &self,
    ) -> (oneshot::Receiver<()>, oneshot::Sender<()>) {
        let (selected_tx, selected_rx) = oneshot::channel();
        let (release_tx, release_rx) = oneshot::channel();
        *self.inner.phase_barrier.lock().await = Some((selected_tx, release_rx));
        (selected_rx, release_tx)
    }

    pub async fn close(&self) -> Result<(), DurableError> {
        if !self.inner.sealed.swap(true, Ordering::AcqRel) {
            let inner = self.inner.clone();
            tokio::spawn(async move {
                let result = close_owned(inner.clone()).await;
                let _ = inner.close_tx.send(Some(result));
            });
        }
        let mut result = self.inner.close_rx.clone();
        loop {
            if let Some(outcome) = result.borrow().clone() {
                return outcome;
            }
            result.changed().await.map_err(|_| DurableError::Poisoned)?;
        }
    }

    fn ensure_open(&self) -> Result<(), DurableError> {
        if self.inner.sealed.load(Ordering::Acquire) {
            Err(DurableError::Closed)
        } else if let Some(error) = self.inner.executor_error_rx.borrow().clone() {
            Err(error)
        } else {
            Ok(())
        }
    }
}

async fn abort_owned(inner: Arc<Inner>, handle: SubmissionHandle) -> Result<(), DurableError> {
    let child_ids = {
        let _operation = inner.operations.lock().await;
        let snapshot = inner.session.snapshot().await?;
        let submission = snapshot
            .submissions
            .get(&handle.id)
            .ok_or_else(|| DurableError::Rejected("unknown submission".into()))?;
        let mapped_task = snapshot
            .entries
            .get(&submission.entry_id)
            .and_then(|entry| entry.by_task_id);
        if mapped_task != Some(handle.task_id) {
            return Err(DurableError::Rejected("submission handle mismatch".into()));
        }
        let parent = snapshot
            .tasks
            .get(&handle.task_id)
            .cloned()
            .ok_or_else(|| DurableError::Rejected("unknown generation task".into()))?;
        if parent.kind != "generation" {
            return Err(DurableError::Rejected(
                "abort target is not generation".into(),
            ));
        }
        if parent.state.terminal() {
            return Ok(());
        }
        let seq = CommitSeq::new(snapshot.next_seq)?;
        let mut parent_update = parent;
        parent_update.abort_requested = true;
        parent_update.updated_seq = seq;
        let mut updates = vec![parent_update];
        let mut children = Vec::new();
        for child in snapshot
            .tasks
            .values()
            .filter(|task| task.owner_task_id == Some(handle.task_id) && !task.state.terminal())
        {
            let mut child = child.clone();
            child.abort_requested = true;
            child.updated_seq = seq;
            children.push(child.id);
            updates.push(child);
        }
        inner
            .session
            .commit(CommitBatch {
                seq,
                next_id: snapshot.next_id,
                next_seq: increment(seq.get())?,
                entries: vec![],
                tasks: updates,
                submissions: vec![],
                documents: vec![],
            })
            .await?;
        children
    };
    {
        let cancels = inner.tool_cancels.lock().await;
        for child_id in child_ids {
            if let Some(cancel) = cancels.get(&child_id) {
                let _ = cancel.send(true);
            }
        }
    }
    bump_revision(&inner);
    inner.executor_notify.notify_one();
    let mut revision = inner.revision_tx.subscribe();
    let mut executor_error = inner.executor_error_rx.clone();
    loop {
        if let Some(error) = executor_error.borrow().clone() {
            return Err(error);
        }
        let snapshot = inner.session.snapshot().await?;
        let submission = snapshot
            .submissions
            .get(&handle.id)
            .ok_or_else(|| DurableError::Corrupt("abort submission missing".into()))?;
        if submission.status != "pending" {
            return Ok(());
        }
        let parent = snapshot
            .tasks
            .get(&handle.task_id)
            .cloned()
            .ok_or_else(|| DurableError::Corrupt("abort parent missing".into()))?;
        if inner.sealed.load(Ordering::Acquire)
            && !inner.running.lock().await.contains(&handle.task_id)
        {
            drop(snapshot);
            if parent.state == TaskState::Completing {
                abort_completing(&inner, parent).await?;
            } else {
                abort_pending_generation(&inner, parent).await?;
            }
            continue;
        }
        tokio::select! {changed=revision.changed()=>changed.map_err(|_|DurableError::Closed)?,changed=executor_error.changed()=>changed.map_err(|_|DurableError::Closed)?,}
    }
}

async fn admit_submission(
    inner: Arc<Inner>,
    request: SubmitRequest,
) -> Result<SubmissionHandle, DurableError> {
    let _operation = inner.operations.lock().await;
    if inner.sealed.load(Ordering::Acquire) {
        return Err(DurableError::Closed);
    }
    let snapshot = inner.session.snapshot().await?;
    let conversation = conversation_id()?;
    if let Some(handle) = reacquire(&snapshot, conversation, &request)? {
        if snapshot
            .submissions
            .get(&handle.id)
            .is_some_and(|value| value.status == "pending")
        {
            wake(&inner).await?;
        }
        return Ok(handle);
    }
    let (batch, handle) = build_initial_batch(
        &snapshot,
        conversation,
        &request,
        inner.model.clone(),
        inner.options.clone(),
    )?;
    inner.session.commit(batch).await?;
    bump_revision(&inner);
    wake(&inner).await?;
    Ok(handle)
}

async fn wake(inner: &Arc<Inner>) -> Result<(), DurableError> {
    if let Some(error) = inner.executor_error_rx.borrow().clone() {
        return Err(error);
    }
    if inner.executor_stop.load(Ordering::Acquire) {
        return Err(DurableError::Closed);
    }
    inner.executor_notify.notify_one();
    Ok(())
}

async fn executor_worker(inner: Arc<Inner>) -> Result<(), DurableError> {
    loop {
        inner.executor_notify.notified().await;
        if inner.executor_stop.load(Ordering::Acquire) {
            return Ok(());
        }
        loop {
            if inner.sealed.load(Ordering::Acquire) {
                break;
            }
            let task_id = match next_pending_task(&inner).await {
                Ok(Some(task_id)) => task_id,
                Ok(None) => break,
                Err(error) => {
                    let _ = inner.executor_error_tx.send(Some(error.clone()));
                    inner.executor_stop.store(true, Ordering::Release);
                    bump_revision(&inner);
                    return Err(error);
                }
            };
            inner.running.lock().await.insert(task_id);
            let state = inner
                .session
                .snapshot()
                .await?
                .tasks
                .get(&task_id)
                .map(|task| task.state.clone());
            let result = if state == Some(TaskState::Completing) {
                resume_completing(inner.clone(), task_id).await
            } else {
                execute(inner.clone(), task_id).await
            };
            inner.running.lock().await.remove(&task_id);
            inner.scheduled.lock().await.remove(&task_id);
            if let Err(error) = result {
                let _ = inner.executor_error_tx.send(Some(error.clone()));
                inner.executor_stop.store(true, Ordering::Release);
                bump_revision(&inner);
                return Err(error);
            }
            bump_revision(&inner);
        }
    }
}

async fn next_pending_task(inner: &Arc<Inner>) -> Result<Option<TaskId>, DurableError> {
    let snapshot = inner.session.snapshot().await?;
    let running = inner.running.lock().await;
    let mut scheduled = inner.scheduled.lock().await;
    Ok(snapshot
        .tasks
        .values()
        .filter(|task| {
            task.kind == "generation"
                && matches!(task.state, TaskState::Pending | TaskState::Completing)
        })
        .map(|task| task.id)
        .filter(|id| !running.contains(id) && !scheduled.contains(id))
        .min_by_key(|id| id.get())
        .inspect(|id| {
            scheduled.insert(*id);
        }))
}

async fn close_owned(inner: Arc<Inner>) -> Result<(), DurableError> {
    // Linearise the seal against provider-phase admission. A task already holding
    // this gate is admitted and will drain; a selected task still waiting for the
    // gate observes sealed and remains pending for reopen.
    {
        let _seal_barrier = inner.operations.lock().await;
    }
    // Join the admission line without retaining its mutex while tasks complete.
    loop {
        let admissions = {
            let mut guard = inner.admissions.lock().await;
            std::mem::take(&mut *guard)
        };
        if admissions.is_empty() {
            break;
        }
        for task in admissions {
            let _ = task.await;
        }
    }
    let aborts = {
        inner
            .aborts
            .lock()
            .await
            .values()
            .cloned()
            .collect::<Vec<_>>()
    };
    for mut abort in aborts {
        loop {
            if abort.borrow().is_some() {
                break;
            }
            abort.changed().await.map_err(|_| DurableError::Poisoned)?;
        }
    }
    inner.executor_stop.store(true, Ordering::Release);
    inner.executor_notify.notify_one();
    let worker_error = if let Some(task) = inner.executor_task.lock().await.take() {
        if task.await.is_err() {
            Some(DurableError::Poisoned)
        } else {
            None
        }
    } else {
        None
    };
    let executor_error = { inner.executor_error_rx.borrow().clone() };
    let close_result = inner.session.close().await;
    if let Some(error) = worker_error.or(executor_error) {
        Err(error)
    } else {
        close_result
    }
}

async fn execute(inner: Arc<Inner>, task_id: TaskId) -> Result<(), DurableError> {
    #[cfg(test)]
    if let Some((selected, release)) = inner.phase_barrier.lock().await.take() {
        let _ = selected.send(());
        let _ = release.await;
    }
    let intent = {
        let _operation = inner.operations.lock().await;
        let snapshot = inner.session.snapshot().await?;
        let task = snapshot
            .tasks
            .get(&task_id)
            .cloned()
            .ok_or_else(|| DurableError::Corrupt("scheduled task missing".into()))?;
        if task.state.terminal() {
            return Ok(());
        }
        if task.abort_requested {
            drop(_operation);
            return abort_pending_generation(&inner, task).await;
        }
        if inner.sealed.load(Ordering::Acquire) {
            return Ok(());
        }
        let mut intent: ModelIntent = serde_json::from_value(task.input.clone())
            .map_err(|_| DurableError::Corrupt("invalid persisted model intent".into()))?;
        if intent.context.is_empty() {
            let context = context_for_task(&snapshot, task_id)?;
            intent.context_cutoff = u32::try_from(context.len())
                .map_err(|_| DurableError::Range("context length overflow".into()))?;
            intent.context = context;
        }
        if intent.offered_tools.is_empty() {
            intent.offered_tools = inner
                .tools
                .bindings()
                .into_iter()
                .map(|binding| binding.definition)
                .collect();
        }
        intent.validate()?;
        if inner.sealed.load(Ordering::Acquire) {
            return Ok(());
        }
        let seq = CommitSeq::new(snapshot.next_seq)?;
        let (session_id, provider_document) =
            super::provider::prepare_provider_session(&snapshot, task.conversation_id, seq)?;
        if intent
            .provider_session_id
            .as_ref()
            .is_some_and(|existing| existing != &session_id)
        {
            return Err(DurableError::Corrupt(
                "model intent provider identity conflict".into(),
            ));
        }
        intent.provider_session_id = Some(session_id);
        intent.validate()?;
        let mut running = task;
        running.input = serde_json::to_value(&intent)
            .map_err(|error| DurableError::Rejected(error.to_string()))?;
        running.state = TaskState::Running;
        running.checkpoint = json!({"phase":"running","logical_attempt":intent.logical_attempt});
        running.updated_seq = seq;
        inner
            .session
            .commit(CommitBatch {
                seq,
                next_id: snapshot.next_id,
                next_seq: increment(seq.get())?,
                entries: vec![],
                tasks: vec![running],
                submissions: vec![],
                documents: provider_document.into_iter().collect(),
            })
            .await?;
        bump_revision(&inner);
        intent
    };
    let run = inner.runner.run(intent).await;
    settle(&inner, task_id, run).await
}

async fn settle(inner: &Arc<Inner>, task_id: TaskId, run: ModelRun) -> Result<(), DurableError> {
    if let Err(error) = run.validate() {
        if let Some(usage) = rejected_tool_usage(&run) {
            let task = inner
                .session
                .snapshot()
                .await?
                .tasks
                .get(&task_id)
                .cloned()
                .ok_or_else(|| DurableError::Corrupt("generation task missing".into()))?;
            return settle_model_rejection(inner, task, usage, "durable_tool_rejected").await;
        }
        return Err(error);
    }
    let _operation = inner.operations.lock().await;
    let snapshot = inner.session.snapshot().await?;
    let task = snapshot
        .tasks
        .get(&task_id)
        .cloned()
        .ok_or_else(|| DurableError::Corrupt("generation task missing at settlement".into()))?;
    if task.state.terminal() {
        return Ok(());
    }
    if task.abort_requested {
        let billed_usage = terminal_usage_value(run.terminal.as_ref())?;
        drop(_operation);
        return abort_completing_with_usage(inner, task, billed_usage).await;
    }
    let input_entry_id = snapshot
        .entries
        .values()
        .find(|entry| entry.by_task_id == Some(task_id) && entry.kind == "user")
        .map(|entry| entry.id)
        .ok_or_else(|| DurableError::Corrupt("generation lacks input entry".into()))?;
    let submission = snapshot
        .submissions
        .values()
        .find(|value| value.entry_id == input_entry_id)
        .cloned()
        .ok_or_else(|| DurableError::Corrupt("generation lacks submission".into()))?;
    let terminal = if run.terminal_count == 1 {
        run.terminal.unwrap_or(ModelTerminal::Malformed {
            code: "missing_terminal".into(),
        })
    } else {
        ModelTerminal::Malformed {
            code: if run.terminal_count == 0 {
                "missing_terminal"
            } else {
                "duplicate_terminal"
            }
            .into(),
        }
    };
    let seq = CommitSeq::new(snapshot.next_seq)?;
    let mut settled_task = task;
    let mut settled_submission = submission;
    let mut entries = Vec::new();
    let outcome = match terminal {
        ModelTerminal::Answer {
            content,
            text,
            usage,
            response_id,
            stop_reason,
        } => {
            let answer_id = EntryId::new(snapshot.next_id)?;
            entries.push(EntryRecord {
                id: answer_id,
                conversation_id: settled_task.conversation_id,
                kind: "assistant".into(),
                value: json!({
                    "text":text,"content":content,
                    "model": serde_json::from_value::<ModelIntent>(settled_task.input.clone()).map_err(|_| DurableError::Corrupt("invalid persisted model intent".into()))?.model,
                    "response_id":response_id,"stop_reason":stop_reason,"usage":usage,
                }),
                by_task_id: Some(task_id),
                created_seq: seq,
            });
            settled_task.state = TaskState::Succeeded;
            settled_submission.status = "done".into();
            settled_submission.answer_id = Some(answer_id);
            json!({"status":"done","usage":usage})
        }
        ModelTerminal::ToolCalls {
            calls,
            usage,
            assistant,
        } => {
            drop(_operation);
            return settle_tool_round(inner, settled_task, calls, usage, assistant).await;
        }
        ModelTerminal::BilledError { code, usage } => {
            settled_task.state = TaskState::Failed;
            settled_submission.status = "failed".into();
            settled_submission.reason = Some(json!({"code":code}));
            json!({"status":"failed","code":code,"usage":usage})
        }
        ModelTerminal::UnsupportedToolCall => {
            settled_task.state = TaskState::Failed;
            settled_submission.status = "failed".into();
            settled_submission.reason = Some(json!({"code":"durable_tool_unsupported_r1b"}));
            json!({"status":"failed","code":"durable_tool_unsupported_r1b"})
        }
        ModelTerminal::UnsupportedDeferred => {
            settled_task.state = TaskState::Failed;
            settled_submission.status = "failed".into();
            settled_submission.reason = Some(json!({"code":"durable_deferred_unsupported"}));
            json!({"status":"failed","code":"durable_deferred_unsupported"})
        }
        ModelTerminal::Malformed { code } => {
            settled_task.state = TaskState::Failed;
            settled_submission.status = "failed".into();
            settled_submission.reason = Some(json!({"code":code}));
            json!({"status":"failed","code":code})
        }
    };
    settled_task.checkpoint = json!({"phase":"terminal"});
    settled_task.outcome = Some(outcome);
    settled_task.updated_seq = seq;
    settled_submission.updated_seq = seq;
    let next_id = entries
        .first()
        .map(|entry| increment(entry.id.get()))
        .transpose()?
        .unwrap_or(snapshot.next_id);
    let turn_usage = settled_task
        .outcome
        .as_ref()
        .and_then(|value| value.get("usage"))
        .cloned();
    let aggregate = aggregate_usage(&snapshot, turn_usage.as_ref())?;
    let model = serde_json::from_value::<ModelIntent>(settled_task.input.clone())
        .map_err(|_| DurableError::Corrupt("invalid persisted model intent".into()))?
        .model;
    let mut pending = snapshot
        .submissions
        .values()
        .filter(|value| value.status == "pending" && value.id != settled_submission.id)
        .map(|value| value.id.get())
        .collect::<Vec<_>>();
    pending.sort_unstable();
    let next_live = snapshot
        .submissions
        .values()
        .filter(|value| value.status == "pending" && value.id != settled_submission.id)
        .min_by_key(|value| value.id.get())
        .and_then(|value| {
            snapshot
                .entries
                .get(&value.entry_id)
                .and_then(|entry| entry.by_task_id)
                .map(|task_id| (value.id, task_id))
        });
    let live_value = next_live.map_or_else(
        || json!({"submission_id":settled_submission.id.get(),"task_id":task_id.get(),"status":settled_submission.status}),
        |(submission_id, next_task_id)| json!({"submission_id":submission_id.get(),"task_id":next_task_id.get(),"status":"pending"}),
    );
    inner
        .session
        .commit(CommitBatch {
            seq,
            next_id,
            next_seq: increment(seq.get())?,
            entries,
            tasks: vec![settled_task],
            submissions: vec![settled_submission],
            documents: vec![
                DocumentRecord {
                    conversation_id: conversation_id()?,
                    kind: "pi.live".into(),
                    version: 1,
                    value: live_value,
                    updated_seq: seq,
                },
                DocumentRecord {
                    conversation_id: conversation_id()?,
                    kind: "pi.inbox".into(),
                    version: 1,
                    value: json!({"pending":pending}),
                    updated_seq: seq,
                },
                DocumentRecord {
                    conversation_id: conversation_id()?,
                    kind: "pi.usage".into(),
                    version: 1,
                    value: json!({"task_id":task_id.get(),"model":model,"turn":turn_usage,"aggregate":aggregate}),
                    updated_seq: seq,
                },
            ],
        })
        .await?;
    bump_revision(inner);
    Ok(())
}

fn rejected_tool_usage(run: &ModelRun) -> Option<crate::durable::model::DurableUsage> {
    if run.terminal_count != 1 {
        return None;
    }
    match run.terminal.as_ref() {
        Some(ModelTerminal::ToolCalls { usage, .. }) if usage.validate().is_ok() => {
            Some(usage.clone())
        }
        _ => None,
    }
}

async fn settle_model_rejection(
    inner: &Arc<Inner>,
    mut task: TaskRecord,
    usage: crate::durable::model::DurableUsage,
    code: &str,
) -> Result<(), DurableError> {
    let _operation = inner.operations.lock().await;
    let snapshot = inner.session.snapshot().await?;
    task = snapshot
        .tasks
        .get(&task.id)
        .cloned()
        .ok_or_else(|| DurableError::Corrupt("rejected generation missing".into()))?;
    let seq = CommitSeq::new(snapshot.next_seq)?;
    let mut submission = snapshot
        .submissions
        .values()
        .find(|s| snapshot.entries.get(&s.entry_id).and_then(|e| e.by_task_id) == Some(task.id))
        .cloned()
        .ok_or_else(|| DurableError::Corrupt("submission missing".into()))?;
    let prior_model_usage = task.checkpoint.get("model_usage").cloned();
    task.state = TaskState::Failed;
    task.checkpoint = json!({"phase":"terminal"});
    task.outcome = Some(json!({"status":"failed","code":code,"usage":usage}));
    task.updated_seq = seq;
    submission.status = "failed".into();
    submission.reason = Some(json!({"code":code}));
    submission.updated_seq = seq;
    let usage_value =
        serde_json::to_value(&usage).map_err(|error| DurableError::Rejected(error.to_string()))?;
    let mut aggregate = aggregate_usage(&snapshot, prior_model_usage.as_ref())?;
    for child in snapshot
        .tasks
        .values()
        .filter(|value| value.owner_task_id == Some(task.id))
    {
        if let Some(child_usage) = child.outcome.as_ref().and_then(|value| value.get("usage")) {
            aggregate = aggregate_usage_value(&aggregate, Some(child_usage))?;
        }
    }
    aggregate = aggregate_usage_value(&aggregate, Some(&usage_value))?;
    let documents = terminal_documents(
        &snapshot,
        &task,
        &submission,
        "failed",
        aggregate,
        Some(usage_value),
        seq,
    )?;
    inner
        .session
        .commit(CommitBatch {
            seq,
            next_id: snapshot.next_id,
            next_seq: increment(seq.get())?,
            entries: vec![],
            tasks: vec![task],
            submissions: vec![submission],
            documents,
        })
        .await?;
    bump_revision(inner);
    Ok(())
}

async fn settle_tool_round(
    inner: &Arc<Inner>,
    mut parent: TaskRecord,
    calls: Vec<crate::durable::tool::DurableToolCall>,
    model_usage: crate::durable::model::DurableUsage,
    assistant: Value,
) -> Result<(), DurableError> {
    let round = parent
        .checkpoint
        .get("tool_round")
        .and_then(Value::as_u64)
        .unwrap_or(0) as u32
        + 1;
    if round > crate::durable::tool::MAX_TOOL_ROUNDS {
        return settle_model_rejection(inner, parent, model_usage, "durable_tool_rejected").await;
    }
    let mut prepared = Vec::new();
    {
        let _operation = inner.operations.lock().await;
        let snapshot = inner.session.snapshot().await?;
        let seq = CommitSeq::new(snapshot.next_seq)?;
        let mut next_id = snapshot.next_id;
        let mut children = Vec::new();
        for call in &calls {
            let Some(registered) = inner.tools.get(&call.name) else {
                drop(_operation);
                return settle_model_rejection(inner, parent, model_usage, "durable_tool_rejected")
                    .await;
            };
            let child_id = TaskId::new(next_id)?;
            next_id = increment(next_id)?;
            let intent = match prepare_intent(&registered.binding, call, child_id.get(), 1) {
                Ok(intent) => intent,
                Err(DurableError::Rejected(_) | DurableError::TooLarge { .. }) => {
                    drop(_operation);
                    return settle_model_rejection(
                        inner,
                        parent,
                        model_usage,
                        "durable_tool_rejected",
                    )
                    .await;
                }
                Err(error) => return Err(error),
            };
            children.push(TaskRecord {
                id: child_id,
                conversation_id: parent.conversation_id,
                kind: "tool".into(),
                version: 1,
                owner_task_id: Some(parent.id),
                state: TaskState::Pending,
                input: serde_json::to_value(&intent)
                    .map_err(|e| DurableError::Rejected(e.to_string()))?,
                checkpoint: json!({"phase":"pending"}),
                outcome: None,
                abort_requested: false,
                started_at: None,
                ended_at: None,
                updated_seq: seq,
            });
            prepared.push((child_id, intent, false));
        }
        let model_usage_value = serde_json::to_value(&model_usage)
            .map_err(|error| DurableError::Rejected(error.to_string()))?;
        let previous_model_usage = parent
            .checkpoint
            .get("model_usage")
            .cloned()
            .unwrap_or_else(empty_usage_aggregate);
        let cumulative_model_usage =
            aggregate_usage_value(&previous_model_usage, Some(&model_usage_value))?;
        let child_ids = prepared
            .iter()
            .map(|(id, _, _)| id.get())
            .collect::<Vec<_>>();
        parent.state = TaskState::Completing;
        parent.checkpoint = json!({"phase":"completing","tool_round":round,"child_ids":child_ids,"assistant":assistant,"model_usage":cumulative_model_usage});
        parent.updated_seq = seq;
        let mut tasks = vec![parent.clone()];
        tasks.extend(children);
        inner
            .session
            .commit(CommitBatch {
                seq,
                next_id,
                next_seq: increment(seq.get())?,
                entries: vec![],
                tasks,
                submissions: vec![],
                documents: vec![],
            })
            .await?;
        bump_revision(inner);
    }

    let mut tool_messages = Vec::new();
    for (child_id, intent, recovered) in prepared {
        let current_parent = inner
            .session
            .snapshot()
            .await?
            .tasks
            .get(&parent.id)
            .cloned()
            .ok_or_else(|| DurableError::Corrupt("parent missing before tool admission".into()))?;
        if current_parent.abort_requested {
            return abort_completing(inner, current_parent).await;
        }
        #[cfg(test)]
        if let Some((selected, release)) = inner.tool_phase_barrier.lock().await.take() {
            let _ = selected.send(());
            let _ = release.await;
        }
        let registered = if recovered {
            replay_registration(&inner.tools, &intent)
                .map_err(|reason| DurableError::Rejected(reason.into()))?
        } else {
            inner
                .tools
                .get(&intent.name)
                .ok_or_else(|| DurableError::Rejected("missing admitted tool".into()))?
        };
        let (cancel_tx, cancel) = watch::channel(false);
        inner.tool_cancels.lock().await.insert(child_id, cancel_tx);
        {
            let _operation = inner.operations.lock().await;
            if inner.sealed.load(Ordering::Acquire) {
                inner.tool_cancels.lock().await.remove(&child_id);
                return Ok(());
            }
            let snapshot = inner.session.snapshot().await?;
            let current_parent = snapshot.tasks.get(&parent.id).ok_or_else(|| {
                DurableError::Corrupt("parent missing before tool admission".into())
            })?;
            if current_parent.abort_requested {
                inner.tool_cancels.lock().await.remove(&child_id);
                drop(_operation);
                return abort_completing(inner, current_parent.clone()).await;
            }
            let mut child = snapshot
                .tasks
                .get(&child_id)
                .cloned()
                .ok_or_else(|| DurableError::Corrupt("tool child missing".into()))?;
            if child.state != TaskState::Pending {
                return Err(DurableError::Corrupt("tool child not pending".into()));
            }
            let seq = CommitSeq::new(snapshot.next_seq)?;
            child.state = TaskState::Running;
            child.checkpoint = json!({"phase":"running"});
            child.updated_seq = seq;
            inner
                .session
                .commit(CommitBatch {
                    seq,
                    next_id: snapshot.next_id,
                    next_seq: increment(seq.get())?,
                    entries: vec![],
                    tasks: vec![child],
                    submissions: vec![],
                    documents: vec![],
                })
                .await?;
            bump_revision(inner);
        }
        let execution = crate::durable::tool::ToolExecution {
            durable_tool_id: intent.durable_tool_id.clone(),
            durable_idempotency_key: intent.durable_idempotency_key.clone(),
            arguments: intent.execution_arguments.clone(),
            cancel,
        };
        let started = std::time::Instant::now();
        let terminal = registered.executor.execute(execution).await;
        let elapsed_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        let mut duration_ms = Some(elapsed_ms);
        inner.tool_cancels.lock().await.remove(&child_id);
        let (value, is_error, usage, outcome) = match terminal {
            Ok(output) => match output.validate() {
                Ok(()) => (
                    output.value,
                    false,
                    output.usage.clone(),
                    json!({"status":"succeeded","usage":output.usage}),
                ),
                Err(_) => (
                    json!({"code":"invalid_tool_usage"}),
                    true,
                    None,
                    json!({"status":"failed","code":"invalid_tool_usage"}),
                ),
            },
            Err(failure) => match failure.validate() {
                Ok(()) => (
                    json!({"code":failure.code}),
                    true,
                    failure.usage.clone(),
                    json!({"status":"failed","code":failure.code,"usage":failure.usage}),
                ),
                Err(_) => (
                    json!({"code":"invalid_tool_usage"}),
                    true,
                    None,
                    json!({"status":"failed","code":"invalid_tool_usage"}),
                ),
            },
        };
        {
            let _operation = inner.operations.lock().await;
            let snapshot = inner.session.snapshot().await?;
            let seq = CommitSeq::new(snapshot.next_seq)?;
            let entry_id = EntryId::new(snapshot.next_id)?;
            let mut child = snapshot
                .tasks
                .get(&child_id)
                .cloned()
                .ok_or_else(|| DurableError::Corrupt("tool child missing at outcome".into()))?;
            child.state = if child.abort_requested {
                TaskState::Aborted
            } else if is_error {
                TaskState::Failed
            } else {
                TaskState::Succeeded
            };
            if child.abort_requested {
                duration_ms = None;
            }
            child.checkpoint = json!({"phase":"terminal"});
            child.outcome = Some(outcome);
            child.updated_seq = seq;
            let entry = EntryRecord {
                id: entry_id,
                conversation_id: parent.conversation_id,
                kind: "tool_result".into(),
                value: tool_result_value(&intent, &value, is_error, &usage, duration_ms),
                by_task_id: Some(child_id),
                created_seq: seq,
            };
            inner
                .session
                .commit(CommitBatch {
                    seq,
                    next_id: increment(entry_id.get())?,
                    next_seq: increment(seq.get())?,
                    entries: vec![entry],
                    tasks: vec![child],
                    submissions: vec![],
                    documents: vec![],
                })
                .await?;
            bump_revision(inner);
        }
        tool_messages.push(tool_result_message(&intent, value, is_error, duration_ms));
    }

    let current = inner.session.snapshot().await?;
    let parent = current
        .tasks
        .get(&parent.id)
        .cloned()
        .ok_or_else(|| DurableError::Corrupt("completing parent missing".into()))?;
    if parent.state != TaskState::Completing {
        return Err(DurableError::Corrupt("parent not completing".into()));
    }
    if parent.abort_requested {
        return abort_completing(inner, parent).await;
    }
    let mut intent: ModelIntent = serde_json::from_value(parent.input.clone())
        .map_err(|_| DurableError::Corrupt("invalid parent model intent".into()))?;
    let mut native = intent.native_messages.take().unwrap_or_else(|| {
        intent
            .context
            .iter()
            .map(super::model::to_message_for_durable)
            .collect()
    });
    native.push(
        serde_json::from_value(assistant)
            .map_err(|_| DurableError::Rejected("invalid assistant tool-call message".into()))?,
    );
    native.extend(tool_messages);
    intent.native_messages = Some(native);
    intent.logical_attempt = increment(u64::from(intent.logical_attempt))?
        .try_into()
        .map_err(|_| DurableError::Range("logical attempt overflow".into()))?;
    intent.context_cutoff = intent.context_cutoff.saturating_add(1);
    intent.validate()?;
    let parent = persist_successor_intent(inner, parent, &intent, round).await?;
    #[cfg(test)]
    if let Some((selected, release)) = inner.successor_phase_barrier.lock().await.take() {
        let _ = selected.send(());
        let _ = release.await;
    }
    admit_successor_provider(inner, parent, intent).await
}

async fn admit_successor_provider(
    inner: &Arc<Inner>,
    parent: TaskRecord,
    mut intent: ModelIntent,
) -> Result<(), DurableError> {
    let current = {
        let _operation = inner.operations.lock().await;
        let snapshot = inner.session.snapshot().await?;
        let mut current =
            snapshot.tasks.get(&parent.id).cloned().ok_or_else(|| {
                DurableError::Corrupt("successor parent missing at admission".into())
            })?;
        if current.abort_requested {
            drop(_operation);
            return abort_completing(inner, current).await;
        }
        if inner.sealed.load(Ordering::Acquire) {
            return Ok(());
        }
        let seq = CommitSeq::new(snapshot.next_seq)?;
        let (session_id, provider_document) =
            super::provider::prepare_provider_session(&snapshot, current.conversation_id, seq)?;
        if intent
            .provider_session_id
            .as_ref()
            .is_some_and(|existing| existing != &session_id)
        {
            return Err(DurableError::Corrupt(
                "successor provider identity conflict".into(),
            ));
        }
        if intent.provider_session_id.is_none() || provider_document.is_some() {
            intent.provider_session_id = Some(session_id);
            intent.validate()?;
            current.input = serde_json::to_value(&intent)
                .map_err(|error| DurableError::Rejected(error.to_string()))?;
            current.updated_seq = seq;
            inner
                .session
                .commit(CommitBatch {
                    seq,
                    next_id: snapshot.next_id,
                    next_seq: increment(seq.get())?,
                    entries: vec![],
                    tasks: vec![current.clone()],
                    submissions: vec![],
                    documents: provider_document.into_iter().collect(),
                })
                .await?;
            bump_revision(inner);
        }
        current
    };
    let run = inner.runner.run(intent).await;
    settle_completing(inner, current, run).await
}

async fn resume_completing(inner: Arc<Inner>, parent_id: TaskId) -> Result<(), DurableError> {
    let snapshot = inner.session.snapshot().await?;
    let parent = snapshot
        .tasks
        .get(&parent_id)
        .cloned()
        .ok_or_else(|| DurableError::Rejected("unknown completing generation".into()))?;
    if parent.state != TaskState::Completing {
        return Err(DurableError::Rejected(
            "generation is not completing".into(),
        ));
    }
    if parent.checkpoint.get("phase").and_then(Value::as_str) == Some("successor") {
        if parent.abort_requested {
            return abort_completing(&inner, parent).await;
        }
        let intent: ModelIntent = serde_json::from_value(parent.input.clone())
            .map_err(|_| DurableError::Corrupt("invalid committed successor intent".into()))?;
        return admit_successor_provider(&inner, parent, intent).await;
    }
    let child_ids = parent
        .checkpoint
        .get("child_ids")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            DurableError::Corrupt("completing parent lacks ordered child list".into())
        })?;
    let children = child_ids
        .iter()
        .map(|value| {
            let id = TaskId::new(
                value
                    .as_u64()
                    .ok_or_else(|| DurableError::Corrupt("invalid child id".into()))?,
            )?;
            snapshot
                .tasks
                .get(&id)
                .filter(|task| task.owner_task_id == Some(parent_id))
                .cloned()
                .ok_or_else(|| DurableError::Corrupt("ordered child missing".into()))
        })
        .collect::<Result<Vec<_>, DurableError>>()?;
    let mut messages = Vec::new();
    for child in children {
        let intent: ToolIntent = serde_json::from_value(child.input.clone())
            .map_err(|_| DurableError::Corrupt("invalid persisted tool intent".into()))?;
        if child.state.terminal() {
            let entry = snapshot
                .entries
                .values()
                .find(|entry| entry.by_task_id == Some(child.id) && entry.kind == "tool_result")
                .ok_or_else(|| DurableError::Corrupt("terminal tool lacks result entry".into()))?;
            let value = entry
                .value
                .get("result")
                .cloned()
                .ok_or_else(|| DurableError::Corrupt("tool result lacks value".into()))?;
            let is_error = entry
                .value
                .get("is_error")
                .and_then(Value::as_bool)
                .unwrap_or(true);
            let duration_ms = entry.value.get("durationMs").and_then(Value::as_u64);
            messages.push(tool_result_message(&intent, value, is_error, duration_ms));
            continue;
        }
        let current_parent = inner
            .session
            .snapshot()
            .await?
            .tasks
            .get(&parent_id)
            .cloned()
            .ok_or_else(|| DurableError::Corrupt("parent missing during recovery".into()))?;
        if current_parent.abort_requested {
            return abort_completing(&inner, current_parent).await;
        }
        let (cancel_tx, cancel) = watch::channel(false);
        inner.tool_cancels.lock().await.insert(child.id, cancel_tx);
        {
            let _operation = inner.operations.lock().await;
            if inner.sealed.load(Ordering::Acquire) {
                inner.tool_cancels.lock().await.remove(&child.id);
                return Ok(());
            }
            let current = inner.session.snapshot().await?;
            let admitted_parent = current.tasks.get(&parent_id).ok_or_else(|| {
                DurableError::Corrupt("parent missing at recovered admission".into())
            })?;
            if admitted_parent.abort_requested {
                inner.tool_cancels.lock().await.remove(&child.id);
                drop(_operation);
                return abort_completing(&inner, admitted_parent.clone()).await;
            }
            let seq = CommitSeq::new(current.next_seq)?;
            let mut admitted = current
                .tasks
                .get(&child.id)
                .cloned()
                .ok_or_else(|| DurableError::Corrupt("recovered child missing".into()))?;
            if admitted.state != TaskState::Pending {
                return Err(DurableError::Corrupt("recovered child not pending".into()));
            }
            admitted.state = TaskState::Running;
            admitted.checkpoint = json!({"phase":"running","recovered":true});
            admitted.updated_seq = seq;
            inner
                .session
                .commit(CommitBatch {
                    seq,
                    next_id: current.next_id,
                    next_seq: increment(seq.get())?,
                    entries: vec![],
                    tasks: vec![admitted],
                    submissions: vec![],
                    documents: vec![],
                })
                .await?;
            bump_revision(&inner);
        }
        match replay_registration(&inner.tools, &intent) {
            Ok(registered) => {
                let started = std::time::Instant::now();
                let result = registered
                    .executor
                    .execute(crate::durable::tool::ToolExecution {
                        durable_tool_id: intent.durable_tool_id.clone(),
                        durable_idempotency_key: intent.durable_idempotency_key.clone(),
                        arguments: intent.execution_arguments.clone(),
                        cancel,
                    })
                    .await;
                let elapsed_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
                inner.tool_cancels.lock().await.remove(&child.id);
                let aborted = inner
                    .session
                    .snapshot()
                    .await?
                    .tasks
                    .get(&child.id)
                    .is_some_and(|task| task.abort_requested);
                let duration_ms = (!aborted).then_some(elapsed_ms);
                let (value, is_error, usage) = match result {
                    Ok(output) => match output.validate() {
                        Ok(()) => (output.value, false, output.usage),
                        Err(_) => (json!({"code":"invalid_tool_usage"}), true, None),
                    },
                    Err(failure) => match failure.validate() {
                        Ok(()) => (json!({"code":failure.code}), true, failure.usage),
                        Err(_) => (json!({"code":"invalid_tool_usage"}), true, None),
                    },
                };
                settle_recovered_child(
                    &inner,
                    child,
                    intent.clone(),
                    value.clone(),
                    is_error,
                    usage,
                    duration_ms,
                )
                .await?;
                messages.push(tool_result_message(&intent, value, is_error, duration_ms));
            }
            Err(reason) => {
                inner.tool_cancels.lock().await.remove(&child.id);
                let value = crate::durable::tool::interrupted_payload(&intent, reason);
                settle_recovered_child(
                    &inner,
                    child,
                    intent.clone(),
                    value.clone(),
                    true,
                    None,
                    None,
                )
                .await?;
                messages.push(tool_result_message(&intent, value, true, None));
            }
        }
    }
    let snapshot = inner.session.snapshot().await?;
    let parent = snapshot
        .tasks
        .get(&parent_id)
        .cloned()
        .ok_or_else(|| DurableError::Corrupt("completing parent missing".into()))?;
    if parent.abort_requested {
        return abort_completing(&inner, parent).await;
    }
    let mut intent: ModelIntent = serde_json::from_value(parent.input.clone())
        .map_err(|_| DurableError::Corrupt("invalid parent intent".into()))?;
    let phase = parent.checkpoint.get("phase").and_then(Value::as_str);
    let round = parent
        .checkpoint
        .get("tool_round")
        .and_then(Value::as_u64)
        .unwrap_or(1) as u32;
    let parent = if phase == Some("successor") {
        parent
    } else {
        let assistant = parent.checkpoint.get("assistant").cloned().ok_or_else(|| {
            DurableError::Corrupt("completing parent lacks assistant call".into())
        })?;
        let mut native = intent.native_messages.take().unwrap_or_else(|| {
            intent
                .context
                .iter()
                .map(super::model::to_message_for_durable)
                .collect()
        });
        native.push(
            serde_json::from_value(assistant)
                .map_err(|_| DurableError::Corrupt("invalid stored assistant call".into()))?,
        );
        native.extend(messages);
        intent.native_messages = Some(native);
        intent.logical_attempt = increment(u64::from(intent.logical_attempt))?
            .try_into()
            .map_err(|_| DurableError::Range("logical attempt overflow".into()))?;
        intent.validate()?;
        persist_successor_intent(&inner, parent, &intent, round).await?
    };
    #[cfg(test)]
    if let Some((selected, release)) = inner.successor_phase_barrier.lock().await.take() {
        let _ = selected.send(());
        let _ = release.await;
    }
    admit_successor_provider(&inner, parent, intent).await
}

async fn persist_successor_intent(
    inner: &Arc<Inner>,
    parent: TaskRecord,
    intent: &ModelIntent,
    round: u32,
) -> Result<TaskRecord, DurableError> {
    let _operation = inner.operations.lock().await;
    let snapshot = inner.session.snapshot().await?;
    let seq = CommitSeq::new(snapshot.next_seq)?;
    let mut update = snapshot
        .tasks
        .get(&parent.id)
        .cloned()
        .ok_or_else(|| DurableError::Corrupt("parent missing before successor".into()))?;
    update.input =
        serde_json::to_value(intent).map_err(|e| DurableError::Rejected(e.to_string()))?;
    let model_usage = update.checkpoint.get("model_usage").cloned();
    update.checkpoint = json!({"phase":"successor","tool_round":round,"model_usage":model_usage});
    update.updated_seq = seq;
    inner
        .session
        .commit(CommitBatch {
            seq,
            next_id: snapshot.next_id,
            next_seq: increment(seq.get())?,
            entries: vec![],
            tasks: vec![update.clone()],
            submissions: vec![],
            documents: vec![],
        })
        .await?;
    bump_revision(inner);
    Ok(update)
}

async fn settle_recovered_child(
    inner: &Arc<Inner>,
    mut child: TaskRecord,
    intent: ToolIntent,
    value: Value,
    is_error: bool,
    usage: Option<crate::durable::model::DurableUsage>,
    duration_ms: Option<u64>,
) -> Result<(), DurableError> {
    let _operation = inner.operations.lock().await;
    let snapshot = inner.session.snapshot().await?;
    let seq = CommitSeq::new(snapshot.next_seq)?;
    let entry_id = EntryId::new(snapshot.next_id)?;
    child = snapshot
        .tasks
        .get(&child.id)
        .cloned()
        .ok_or_else(|| DurableError::Corrupt("recovered running child missing".into()))?;
    child.state = if is_error {
        TaskState::Failed
    } else {
        TaskState::Succeeded
    };
    child.checkpoint = json!({"phase":"terminal","recovered":true});
    child.outcome = Some(json!({"status":if is_error{"failed"}else{"succeeded"},"usage":usage}));
    child.updated_seq = seq;
    let entry = EntryRecord {
        id: entry_id,
        conversation_id: child.conversation_id,
        kind: "tool_result".into(),
        value: tool_result_value(&intent, &value, is_error, &usage, duration_ms),
        by_task_id: Some(child.id),
        created_seq: seq,
    };
    inner
        .session
        .commit(CommitBatch {
            seq,
            next_id: increment(entry_id.get())?,
            next_seq: increment(seq.get())?,
            entries: vec![entry],
            tasks: vec![child],
            submissions: vec![],
            documents: vec![],
        })
        .await?;
    bump_revision(inner);
    Ok(())
}

fn terminal_documents(
    snapshot: &crate::durable::storage::StorageSnapshot,
    parent: &TaskRecord,
    submission: &SubmissionRecord,
    status: &str,
    aggregate: Value,
    final_turn: Option<Value>,
    seq: CommitSeq,
) -> Result<Vec<DocumentRecord>, DurableError> {
    let mut pending = snapshot
        .submissions
        .values()
        .filter(|value| value.status == "pending" && value.id != submission.id)
        .map(|value| value.id.get())
        .collect::<Vec<_>>();
    pending.sort_unstable();
    let next_live = snapshot
        .submissions
        .values()
        .filter(|value| value.status == "pending" && value.id != submission.id)
        .min_by_key(|value| value.id.get())
        .and_then(|value| {
            snapshot
                .entries
                .get(&value.entry_id)
                .and_then(|entry| entry.by_task_id)
                .map(|task_id| (value.id, task_id))
        });
    let live=next_live.map_or_else(||json!({"submission_id":submission.id.get(),"task_id":parent.id.get(),"status":status}),|(submission_id,task_id)|json!({"submission_id":submission_id.get(),"task_id":task_id.get(),"status":"pending"}));
    Ok(vec![
        DocumentRecord {
            conversation_id: parent.conversation_id,
            kind: "pi.live".into(),
            version: 1,
            value: live,
            updated_seq: seq,
        },
        DocumentRecord {
            conversation_id: parent.conversation_id,
            kind: "pi.inbox".into(),
            version: 1,
            value: json!({"pending":pending}),
            updated_seq: seq,
        },
        DocumentRecord {
            conversation_id: parent.conversation_id,
            kind: "pi.usage".into(),
            version: 1,
            value: json!({"task_id":parent.id.get(),"aggregate":aggregate,"final_turn":final_turn}),
            updated_seq: seq,
        },
    ])
}

async fn abort_pending_generation(
    inner: &Arc<Inner>,
    mut parent: TaskRecord,
) -> Result<(), DurableError> {
    let snapshot = inner.session.snapshot().await?;
    let seq = CommitSeq::new(snapshot.next_seq)?;
    let mut submission = snapshot
        .submissions
        .values()
        .find(|s| snapshot.entries.get(&s.entry_id).and_then(|e| e.by_task_id) == Some(parent.id))
        .cloned()
        .ok_or_else(|| DurableError::Corrupt("submission missing".into()))?;
    parent.state = TaskState::Aborted;
    parent.checkpoint = json!({"phase":"terminal"});
    parent.outcome = Some(json!({"status":"aborted"}));
    parent.updated_seq = seq;
    submission.status = "aborted".into();
    submission.reason = Some(json!({"code":"aborted"}));
    submission.updated_seq = seq;
    let aggregate = aggregate_usage(&snapshot, None)?;
    let documents = terminal_documents(
        &snapshot,
        &parent,
        &submission,
        "aborted",
        aggregate,
        None,
        seq,
    )?;
    inner
        .session
        .commit(CommitBatch {
            seq,
            next_id: snapshot.next_id,
            next_seq: increment(seq.get())?,
            entries: vec![],
            tasks: vec![parent],
            submissions: vec![submission],
            documents,
        })
        .await?;
    bump_revision(inner);
    Ok(())
}

async fn abort_completing(inner: &Arc<Inner>, parent: TaskRecord) -> Result<(), DurableError> {
    abort_completing_with_usage(inner, parent, None).await
}

async fn abort_completing_with_usage(
    inner: &Arc<Inner>,
    mut parent: TaskRecord,
    billed_usage: Option<Value>,
) -> Result<(), DurableError> {
    let _operation = inner.operations.lock().await;
    let snapshot = inner.session.snapshot().await?;
    let seq = CommitSeq::new(snapshot.next_seq)?;
    let mut child_updates = Vec::new();
    for child in snapshot
        .tasks
        .values()
        .filter(|task| task.owner_task_id == Some(parent.id) && !task.state.terminal())
    {
        let mut child = child.clone();
        child.state = TaskState::Aborted;
        child.checkpoint = json!({"phase":"terminal"});
        child.outcome = Some(json!({"status":"aborted"}));
        child.abort_requested = true;
        child.updated_seq = seq;
        child_updates.push(child);
    }
    let mut submission = snapshot
        .submissions
        .values()
        .find(|s| snapshot.entries.get(&s.entry_id).and_then(|e| e.by_task_id) == Some(parent.id))
        .cloned()
        .ok_or_else(|| DurableError::Corrupt("submission missing".into()))?;
    let mut aggregate = aggregate_usage(&snapshot, parent.checkpoint.get("model_usage"))?;
    for child in snapshot
        .tasks
        .values()
        .filter(|value| value.owner_task_id == Some(parent.id))
    {
        if let Some(usage) = child.outcome.as_ref().and_then(|value| value.get("usage")) {
            aggregate = aggregate_usage_value(&aggregate, Some(usage))?;
        }
    }
    if let Some(usage) = billed_usage.as_ref() {
        aggregate = aggregate_usage_value(&aggregate, Some(usage))?;
    }
    parent.state = TaskState::Aborted;
    parent.checkpoint = json!({"phase":"terminal"});
    parent.outcome = Some(json!({"status":"aborted","usage":billed_usage}));
    parent.updated_seq = seq;
    submission.status = "aborted".into();
    submission.reason = Some(json!({"code":"aborted"}));
    submission.updated_seq = seq;
    let documents = terminal_documents(
        &snapshot,
        &parent,
        &submission,
        "aborted",
        aggregate,
        billed_usage,
        seq,
    )?;
    inner
        .session
        .commit(CommitBatch {
            seq,
            next_id: snapshot.next_id,
            next_seq: increment(seq.get())?,
            entries: vec![],
            tasks: {
                let mut tasks = vec![parent.clone()];
                tasks.extend(child_updates);
                tasks
            },
            submissions: vec![submission],
            documents,
        })
        .await?;
    bump_revision(inner);
    Ok(())
}

async fn settle_completing(
    inner: &Arc<Inner>,
    parent: TaskRecord,
    run: ModelRun,
) -> Result<(), DurableError> {
    if let Err(error) = run.validate() {
        if let Some(usage) = rejected_tool_usage(&run) {
            return settle_model_rejection(inner, parent, usage, "durable_tool_rejected").await;
        }
        return Err(error);
    }
    let _operation = inner.operations.lock().await;
    let snapshot = inner.session.snapshot().await?;
    let current = snapshot
        .tasks
        .get(&parent.id)
        .cloned()
        .ok_or_else(|| DurableError::Corrupt("completing parent missing".into()))?;
    if current.state != TaskState::Completing {
        return Err(DurableError::Corrupt("parent not completing".into()));
    }
    if current.abort_requested {
        let billed_usage = terminal_usage_value(run.terminal.as_ref())?;
        drop(_operation);
        return abort_completing_with_usage(inner, current, billed_usage).await;
    }
    if let (
        1,
        Some(ModelTerminal::ToolCalls {
            calls,
            usage,
            assistant,
        }),
    ) = (run.terminal_count, run.terminal.clone())
    {
        drop(_operation);
        return Box::pin(settle_tool_round(inner, current, calls, usage, assistant)).await;
    }
    match run.terminal {
        Some(ModelTerminal::Answer {
            content,
            text,
            usage,
            response_id,
            stop_reason,
        }) if run.terminal_count == 1 => {
            let submission = snapshot
                .submissions
                .values()
                .find(|s| {
                    snapshot.entries.get(&s.entry_id).and_then(|e| e.by_task_id) == Some(parent.id)
                })
                .cloned()
                .ok_or_else(|| DurableError::Corrupt("submission missing".into()))?;
            let seq = CommitSeq::new(snapshot.next_seq)?;
            let answer_id = EntryId::new(snapshot.next_id)?;
            let initial_usage = current.checkpoint.get("model_usage").cloned();
            let mut task = current;
            task.state = TaskState::Succeeded;
            task.checkpoint = json!({"phase":"terminal"});
            task.outcome = Some(json!({"status":"done","usage":usage}));
            task.updated_seq = seq;
            let mut submission = submission;
            submission.status = "done".into();
            submission.answer_id = Some(answer_id);
            submission.updated_seq = seq;
            let entry = EntryRecord {
                id: answer_id,
                conversation_id: task.conversation_id,
                kind: "assistant".into(),
                value: json!({"text":text,"content":content,"response_id":response_id,"stop_reason":stop_reason,"usage":usage}),
                by_task_id: Some(task.id),
                created_seq: seq,
            };
            let mut aggregate = aggregate_usage(&snapshot, initial_usage.as_ref())?;
            for child in snapshot
                .tasks
                .values()
                .filter(|value| value.owner_task_id == Some(parent.id))
            {
                if let Some(tool_usage) =
                    child.outcome.as_ref().and_then(|value| value.get("usage"))
                {
                    aggregate = aggregate_usage_value(&aggregate, Some(tool_usage))?;
                }
            }
            aggregate = aggregate_usage_value(
                &aggregate,
                Some(
                    &serde_json::to_value(&usage)
                        .map_err(|e| DurableError::Rejected(e.to_string()))?,
                ),
            )?;
            let final_turn = serde_json::to_value(&usage)
                .map_err(|error| DurableError::Rejected(error.to_string()))?;
            let documents = terminal_documents(
                &snapshot,
                &task,
                &submission,
                "done",
                aggregate,
                Some(final_turn),
                seq,
            )?;
            inner
                .session
                .commit(CommitBatch {
                    seq,
                    next_id: increment(answer_id.get())?,
                    next_seq: increment(seq.get())?,
                    entries: vec![entry],
                    tasks: vec![task],
                    submissions: vec![submission],
                    documents,
                })
                .await?;
            bump_revision(inner);
            Ok(())
        }
        other => {
            let seq = CommitSeq::new(snapshot.next_seq)?;
            let initial_usage = current.checkpoint.get("model_usage").cloned();
            let (code, final_usage) = match other {
                Some(ModelTerminal::BilledError { code, usage }) => (
                    code,
                    Some(
                        serde_json::to_value(usage)
                            .map_err(|error| DurableError::Rejected(error.to_string()))?,
                    ),
                ),
                _ => ("successor_model_error".into(), None),
            };
            let mut task = current;
            task.state = TaskState::Failed;
            task.checkpoint = json!({"phase":"terminal"});
            task.outcome = Some(json!({"status":"failed","code":code,"usage":final_usage}));
            task.updated_seq = seq;
            let mut submission = snapshot
                .submissions
                .values()
                .find(|s| {
                    snapshot.entries.get(&s.entry_id).and_then(|e| e.by_task_id) == Some(parent.id)
                })
                .cloned()
                .ok_or_else(|| DurableError::Corrupt("submission missing".into()))?;
            submission.status = "failed".into();
            submission.reason = Some(json!({"code":code}));
            submission.updated_seq = seq;
            let mut aggregate = aggregate_usage(&snapshot, initial_usage.as_ref())?;
            for child in snapshot
                .tasks
                .values()
                .filter(|value| value.owner_task_id == Some(parent.id))
            {
                if let Some(usage) = child.outcome.as_ref().and_then(|value| value.get("usage")) {
                    aggregate = aggregate_usage_value(&aggregate, Some(usage))?;
                }
            }
            if let Some(usage) = final_usage.as_ref() {
                aggregate = aggregate_usage_value(&aggregate, Some(usage))?;
            }
            let documents = terminal_documents(
                &snapshot,
                &task,
                &submission,
                "failed",
                aggregate,
                final_usage,
                seq,
            )?;
            inner
                .session
                .commit(CommitBatch {
                    seq,
                    next_id: snapshot.next_id,
                    next_seq: increment(seq.get())?,
                    entries: vec![],
                    tasks: vec![task],
                    submissions: vec![submission],
                    documents,
                })
                .await?;
            bump_revision(inner);
            Ok(())
        }
    }
}

fn terminal_usage_value(terminal: Option<&ModelTerminal>) -> Result<Option<Value>, DurableError> {
    match terminal {
        Some(
            ModelTerminal::Answer { usage, .. }
            | ModelTerminal::BilledError { usage, .. }
            | ModelTerminal::ToolCalls { usage, .. },
        ) => {
            Ok(Some(serde_json::to_value(usage).map_err(|error| {
                DurableError::Rejected(error.to_string())
            })?))
        }
        _ => Ok(None),
    }
}

fn tool_result_value(
    intent: &ToolIntent,
    value: &Value,
    is_error: bool,
    usage: &Option<crate::durable::model::DurableUsage>,
    duration_ms: Option<u64>,
) -> Value {
    let mut result = json!({"tool_call_id":intent.provider_call_id,"tool_name":intent.name,"result":value,"is_error":is_error,"usage":usage});
    if let Some(duration) = duration_ms {
        result["durationMs"] = json!(duration);
    }
    result
}

fn tool_result_message(
    intent: &ToolIntent,
    value: Value,
    is_error: bool,
    duration_ms: Option<u64>,
) -> crate::types::Message {
    let mut message = crate::types::user_message("");
    message.role = crate::types::Role::ToolResult;
    message.duration_ms = duration_ms;
    message.content = vec![crate::types::ContentBlock::Text {
        text: value.to_string(),
        text_signature: None,
    }];
    message.tool_call_id = Some(intent.provider_call_id.clone());
    message.tool_name = Some(intent.name.clone());
    message.is_error = is_error;
    message
}

async fn reconcile_running(session: &Arc<DurableSession>) -> Result<(), DurableError> {
    let snapshot = session.snapshot().await?;
    let running = snapshot
        .tasks
        .values()
        .filter(|task| task.state == TaskState::Running)
        .cloned()
        .collect::<Vec<_>>();
    if running.is_empty() {
        return Ok(());
    }
    let seq = CommitSeq::new(snapshot.next_seq)?;
    let tasks = running
        .into_iter()
        .map(|mut task| {
            task.state = TaskState::Pending;
            task.checkpoint = json!({"phase":"pending","recovered":true});
            task.updated_seq = seq;
            task
        })
        .collect();
    session
        .commit(CommitBatch {
            seq,
            next_id: snapshot.next_id,
            next_seq: increment(seq.get())?,
            entries: vec![],
            tasks,
            submissions: vec![],
            documents: vec![],
        })
        .await?;
    Ok(())
}

fn context_for_task(
    snapshot: &crate::durable::storage::StorageSnapshot,
    task_id: TaskId,
) -> Result<Vec<DurableMessage>, DurableError> {
    let target_entry = snapshot
        .entries
        .values()
        .find(|entry| entry.by_task_id == Some(task_id) && entry.kind == "user")
        .ok_or_else(|| DurableError::Corrupt("generation lacks input entry".into()))?;
    let submissions = snapshot
        .submissions
        .values()
        .filter(|submission| submission.conversation_id == target_entry.conversation_id)
        .map(|submission| (submission.entry_id, submission))
        .collect::<std::collections::HashMap<_, _>>();
    let mut entries = snapshot
        .entries
        .values()
        .filter(|entry| {
            entry.conversation_id == target_entry.conversation_id
                && entry.id.get() <= target_entry.id.get()
        })
        .collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.id.get());
    let mut context = Vec::new();
    for entry in entries {
        if entry.kind != "user" {
            continue;
        }
        let input = entry
            .value
            .get("text")
            .and_then(Value::as_str)
            .ok_or_else(|| DurableError::Corrupt("invalid context user entry".into()))?;
        context.push(DurableMessage {
            role: "user".into(),
            text: input.into(),
        });
        if entry.id == target_entry.id {
            break;
        }
        if let Some(answer_id) = submissions
            .get(&entry.id)
            .and_then(|submission| submission.answer_id)
        {
            let answer = snapshot
                .entries
                .get(&answer_id)
                .and_then(|value| value.value.get("text"))
                .and_then(Value::as_str)
                .ok_or_else(|| DurableError::Corrupt("invalid submission answer entry".into()))?;
            context.push(DurableMessage {
                role: "assistant".into(),
                text: answer.into(),
            });
        }
    }
    if context.last().is_none_or(|message| {
        message.text
            != target_entry
                .value
                .get("text")
                .and_then(Value::as_str)
                .unwrap_or_default()
    }) {
        return Err(DurableError::Corrupt(
            "target submission missing from context".into(),
        ));
    }
    Ok(context)
}

fn context_from_snapshot(
    snapshot: &crate::durable::storage::StorageSnapshot,
    options: ContextOptions,
) -> Result<Vec<DurableMessage>, DurableError> {
    let conversation = conversation_id()?;
    let cut = options
        .at
        .map(|at| {
            snapshot
                .entries
                .get(&at)
                .filter(|entry| entry.conversation_id == conversation)
                .map(|entry| (entry.created_seq, entry.id.get()))
                .ok_or_else(|| {
                    DurableError::Rejected(format!(
                        "entry {} is not visible in this conversation",
                        at.get()
                    ))
                })
        })
        .transpose()?;
    let mut entries = snapshot
        .entries
        .values()
        .filter(|entry| entry.conversation_id == conversation)
        .filter(|entry| cut.is_none_or(|cut| (entry.created_seq, entry.id.get()) <= cut))
        .filter_map(|entry| {
            let role = match entry.kind.as_str() {
                "user" => "user",
                "assistant" => "assistant",
                _ => return None,
            };
            entry.value.get("text").and_then(Value::as_str).map(|text| {
                (
                    entry.created_seq,
                    entry.id.get(),
                    DurableMessage {
                        role: role.into(),
                        text: text.into(),
                    },
                )
            })
        })
        .collect::<Vec<_>>();
    entries.sort_by_key(|(seq, id, _)| (*seq, *id));
    Ok(entries.into_iter().map(|(_, _, value)| value).collect())
}

fn aggregate_usage(
    snapshot: &crate::durable::storage::StorageSnapshot,
    turn: Option<&Value>,
) -> Result<Value, DurableError> {
    let previous = snapshot
        .documents
        .get(&(
            conversation_id().expect("constant valid"),
            "pi.usage".into(),
        ))
        .and_then(|document| document.value.get("aggregate"));
    let number = |value: Option<&Value>, field: &str| {
        value
            .and_then(|value| value.get(field))
            .and_then(Value::as_u64)
            .unwrap_or(0)
    };
    let cost = |value: Option<&Value>, field: &str| {
        value
            .and_then(|value| value.get("cost"))
            .and_then(|value| value.get(field))
            .and_then(Value::as_f64)
            .unwrap_or(0.0)
    };
    let add = |field| {
        number(previous, field)
            .checked_add(number(turn, field))
            .ok_or_else(|| DurableError::Range(format!("usage {field} overflow")))
    };
    let add_cost = |field: &str| {
        let total = previous
            .and_then(|value| value.get("cost"))
            .and_then(|value| value.get(field))
            .and_then(Value::as_f64)
            .unwrap_or(0.0)
            + cost(turn, field);
        if total.is_finite() && total >= 0.0 {
            Ok(total)
        } else {
            Err(DurableError::Rejected(
                "usage cost aggregate overflow".into(),
            ))
        }
    };
    Ok(json!({
        "input": add("input")?,
        "output": add("output")?,
        "cache_read": add("cache_read")?,
        "cache_write": add("cache_write")?,
        "cache_write_1h": add("cache_write_1h")?,
        "reasoning": add("reasoning")?,
        "total_tokens": add("total_tokens")?,
        "cost": {
            "input": add_cost("input")?,
            "output": add_cost("output")?,
            "cache_read": add_cost("cache_read")?,
            "cache_write": add_cost("cache_write")?,
            "total": add_cost("total")?,
        },
    }))
}

fn empty_usage_aggregate() -> Value {
    json!({"input":0,"output":0,"cache_read":0,"cache_write":0,"cache_write_1h":0,"reasoning":0,"total_tokens":0,"cost":{"input":0.0,"output":0.0,"cache_read":0.0,"cache_write":0.0,"total":0.0}})
}

fn aggregate_usage_value(previous: &Value, turn: Option<&Value>) -> Result<Value, DurableError> {
    let number = |value: Option<&Value>, field: &str| {
        value
            .and_then(|v| v.get(field))
            .and_then(Value::as_u64)
            .unwrap_or(0)
    };
    let add = |field| {
        number(Some(previous), field)
            .checked_add(number(turn, field))
            .ok_or_else(|| DurableError::Range(format!("usage {field} overflow")))
    };
    let cost = |value: Option<&Value>, field: &str| {
        value
            .and_then(|v| v.get("cost"))
            .and_then(|v| v.get(field))
            .and_then(Value::as_f64)
            .unwrap_or(0.0)
    };
    let add_cost = |field: &str| {
        let total = cost(Some(previous), field) + cost(turn, field);
        if total.is_finite() && total >= 0.0 {
            Ok(total)
        } else {
            Err(DurableError::Rejected(
                "usage cost aggregate overflow".into(),
            ))
        }
    };
    Ok(
        json!({"input":add("input")?,"output":add("output")?,"cache_read":add("cache_read")?,"cache_write":add("cache_write")?,"cache_write_1h":add("cache_write_1h")?,"reasoning":add("reasoning")?,"total_tokens":add("total_tokens")?,"cost":{"input":add_cost("input")?,"output":add_cost("output")?,"cache_read":add_cost("cache_read")?,"cache_write":add_cost("cache_write")?,"total":add_cost("total")?}}),
    )
}

fn bump_revision(inner: &Arc<Inner>) {
    let next = inner.revision_tx.borrow().wrapping_add(1);
    let _ = inner.revision_tx.send(next);
}

fn conversation_id() -> Result<ConversationId, DurableError> {
    ConversationId::new(ROOT_CONVERSATION_ID)
}

fn increment(value: u64) -> Result<u64, DurableError> {
    value
        .checked_add(1)
        .filter(|value| *value <= MAX_ID)
        .ok_or_else(|| DurableError::Range("durable counter overflow".into()))
}
