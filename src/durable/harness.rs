use crate::durable::model::{
    DurableMessage, DurableModelRunner, ModelIntent, ModelRun, ModelTerminal, PinnedModel,
    PinnedOptions,
};
use crate::durable::session::DurableSession;
use crate::durable::storage::DurableStorage;
use crate::durable::submission::{
    SubmissionHandle, SubmissionView, SubmitRequest, build_initial_batch, reacquire, view,
};
use crate::durable::types::*;
use serde_json::{Value, json};
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::{Mutex, Notify, oneshot, watch};

const ROOT_CONVERSATION_ID: u64 = 1;

pub struct DurableHarness {
    inner: Arc<Inner>,
}

struct Inner {
    session: Arc<DurableSession>,
    runner: Arc<dyn DurableModelRunner>,
    model: PinnedModel,
    options: PinnedOptions,
    sealed: AtomicBool,
    operations: Mutex<()>,
    admissions: Mutex<Vec<tokio::task::JoinHandle<()>>>,
    scheduled: Mutex<HashSet<TaskId>>,
    running: Mutex<HashSet<TaskId>>,
    executor_notify: Notify,
    executor_stop: AtomicBool,
    executor_task: Mutex<Option<tokio::task::JoinHandle<()>>>,
    revision_tx: watch::Sender<u64>,
    executor_error_tx: watch::Sender<Option<DurableError>>,
    executor_error_rx: watch::Receiver<Option<DurableError>>,
    close_tx: watch::Sender<Option<Result<(), DurableError>>>,
    close_rx: watch::Receiver<Option<Result<(), DurableError>>>,
    #[cfg(test)]
    phase_barrier: Mutex<Option<(oneshot::Sender<()>, oneshot::Receiver<()>)>>,
}

impl DurableHarness {
    pub async fn open(
        storage: Box<dyn DurableStorage>,
        runner: Arc<dyn DurableModelRunner>,
        model: PinnedModel,
        options: PinnedOptions,
    ) -> Result<Self, DurableError> {
        model.validate()?;
        options.validate()?;
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
            sealed: AtomicBool::new(false),
            operations: Mutex::new(()),
            admissions: Mutex::new(Vec::new()),
            scheduled: Mutex::new(HashSet::new()),
            running: Mutex::new(HashSet::new()),
            executor_notify: Notify::new(),
            executor_stop: AtomicBool::new(false),
            executor_task: Mutex::new(None),
            revision_tx,
            executor_error_tx,
            executor_error_rx,
            close_tx,
            close_rx,
            #[cfg(test)]
            phase_barrier: Mutex::new(None),
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
        let snapshot = self.inner.session.snapshot().await?;
        context_from_snapshot(&snapshot)
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
    pub(crate) fn is_sealed(&self) -> bool {
        self.inner.sealed.load(Ordering::Acquire)
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
            let result = execute(inner.clone(), task_id).await;
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
        .filter(|task| task.kind == "generation" && task.state == TaskState::Pending)
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
        intent.validate()?;
        if inner.sealed.load(Ordering::Acquire) {
            return Ok(());
        }
        let seq = CommitSeq::new(snapshot.next_seq)?;
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
                documents: vec![],
            })
            .await?;
        bump_revision(&inner);
        intent
    };
    let run = inner.runner.run(intent).await;
    settle(&inner, task_id, run).await
}

async fn settle(inner: &Arc<Inner>, task_id: TaskId, run: ModelRun) -> Result<(), DurableError> {
    run.validate()?;
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
    let outcome;
    match terminal {
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
            outcome = json!({"status":"done","usage":usage});
        }
        ModelTerminal::BilledError { code, usage } => {
            settled_task.state = TaskState::Failed;
            settled_submission.status = "failed".into();
            settled_submission.reason = Some(json!({"code":code}));
            outcome = json!({"status":"failed","code":code,"usage":usage});
        }
        ModelTerminal::UnsupportedToolCall => {
            settled_task.state = TaskState::Failed;
            settled_submission.status = "failed".into();
            settled_submission.reason = Some(json!({"code":"durable_tool_unsupported_r1b"}));
            outcome = json!({"status":"failed","code":"durable_tool_unsupported_r1b"});
        }
        ModelTerminal::UnsupportedDeferred => {
            settled_task.state = TaskState::Failed;
            settled_submission.status = "failed".into();
            settled_submission.reason = Some(json!({"code":"durable_deferred_unsupported"}));
            outcome = json!({"status":"failed","code":"durable_deferred_unsupported"});
        }
        ModelTerminal::Malformed { code } => {
            settled_task.state = TaskState::Failed;
            settled_submission.status = "failed".into();
            settled_submission.reason = Some(json!({"code":code}));
            outcome = json!({"status":"failed","code":code});
        }
    }
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

async fn reconcile_running(session: &Arc<DurableSession>) -> Result<(), DurableError> {
    let snapshot = session.snapshot().await?;
    let running = snapshot
        .tasks
        .values()
        .filter(|task| task.kind == "generation" && task.state == TaskState::Running)
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
) -> Result<Vec<DurableMessage>, DurableError> {
    let conversation = conversation_id()?;
    let mut entries = snapshot
        .entries
        .values()
        .filter(|entry| entry.conversation_id == conversation)
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
