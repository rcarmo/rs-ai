use crate::durable::types::*;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::future::Future;
use std::io::Write;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};

pub mod journal;
pub mod memory;
pub mod scan;

pub type StorageFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, DurableError>> + Send + 'a>>;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct StorageSnapshot {
    pub next_id: u64,
    pub next_seq: u64,
    pub last_seq: Option<CommitSeq>,
    pub entries: BTreeMap<EntryId, EntryRecord>,
    pub conversations: BTreeMap<ConversationId, ConversationRecord>,
    pub tasks: BTreeMap<TaskId, TaskRecord>,
    pub submissions: BTreeMap<SubmissionId, SubmissionRecord>,
    pub documents: BTreeMap<(ConversationId, String), DocumentRecord>,
    pub request_ids: HashMap<(ConversationId, String), SubmissionId>,
}

impl StorageSnapshot {
    pub fn empty() -> Self {
        let root = ConversationId::new(1).expect("reserved root");
        Self {
            next_id: 1,
            next_seq: 1,
            conversations: BTreeMap::from([(
                root,
                ConversationRecord {
                    id: root,
                    owner: None,
                    created_seq: None,
                },
            )]),
            ..Self::default()
        }
    }

    /// Legacy scopes are reconstructed without allocating or rewriting IDs.
    pub(crate) fn infer_conversations(&mut self) {
        let ids = std::iter::once(ConversationId::new(1).expect("root"))
            .chain(self.entries.values().map(|entry| entry.conversation_id))
            .chain(self.tasks.values().map(|task| task.conversation_id))
            .chain(
                self.submissions
                    .values()
                    .map(|submission| submission.conversation_id),
            )
            .chain(
                self.documents
                    .values()
                    .map(|document| document.conversation_id),
            );
        for id in ids {
            self.conversations.entry(id).or_insert(ConversationRecord {
                id,
                owner: None,
                created_seq: None,
            });
        }
    }

    pub fn apply(&mut self, batch: &CommitBatch) -> Result<(), DurableError> {
        validate_batch(self, batch)?;
        self.apply_validated(batch);
        Ok(())
    }

    /// Call only after validation against this unchanged snapshot. Insertion is
    /// infallible at the Result boundary, so rejected batches never mutate state.
    pub(super) fn apply_validated(&mut self, batch: &CommitBatch) {
        for record in &batch.conversations {
            self.conversations.insert(record.id, record.clone());
        }
        for record in &batch.entries {
            self.entries.insert(record.id, record.clone());
        }
        for record in &batch.tasks {
            self.tasks.insert(record.id, record.clone());
        }
        for record in &batch.submissions {
            if let Some(request_id) = &record.request_id {
                self.request_ids
                    .insert((record.conversation_id, request_id.clone()), record.id);
            }
            self.submissions.insert(record.id, record.clone());
        }
        for record in &batch.documents {
            self.documents.insert(
                (record.conversation_id, record.kind.clone()),
                record.clone(),
            );
        }
        // Reconstruct only newly mentioned legacy scopes, not the entire log
        // on every adoption. Explicit creations were installed above.
        for id in batch
            .entries
            .iter()
            .map(|record| record.conversation_id)
            .chain(batch.tasks.iter().map(|record| record.conversation_id))
            .chain(
                batch
                    .submissions
                    .iter()
                    .map(|record| record.conversation_id),
            )
            .chain(batch.documents.iter().map(|record| record.conversation_id))
        {
            self.conversations.entry(id).or_insert(ConversationRecord {
                id,
                owner: None,
                created_seq: None,
            });
        }
        self.next_id = batch.next_id;
        self.next_seq = batch.next_seq;
        self.last_seq = Some(batch.seq);
    }
}

static NEXT_WRITER_CLAIM: AtomicU64 = AtomicU64::new(1);

#[derive(Debug)]
pub struct WriterClaim {
    id: u64,
}

impl WriterClaim {
    #[doc(hidden)]
    pub fn fresh() -> Self {
        Self {
            id: NEXT_WRITER_CLAIM.fetch_add(1, Ordering::Relaxed),
        }
    }
    pub(crate) fn id(&self) -> u64 {
        self.id
    }
}

pub trait DurableStorage: Send + Sync + 'static {
    fn claim_writer(&self) -> Result<WriterClaim, DurableError>;
    fn load<'a>(&'a self, claim: &'a WriterClaim) -> StorageFuture<'a, StorageSnapshot>;
    fn commit<'a>(&'a self, claim: &'a WriterClaim, batch: CommitBatch) -> StorageFuture<'a, ()>;
    fn close<'a>(&'a self, claim: &'a WriterClaim) -> StorageFuture<'a, ()>;
}

fn bounded(field: &'static str, size: usize, limit: usize) -> Result<(), DurableError> {
    if size > limit {
        Err(DurableError::TooLarge { field, size, limit })
    } else {
        Ok(())
    }
}

struct LimitedWriter {
    bytes: Vec<u8>,
    limit: usize,
    field: &'static str,
}

impl Write for LimitedWriter {
    fn write(&mut self, input: &[u8]) -> std::io::Result<usize> {
        let next = self.bytes.len().saturating_add(input.len());
        if next > self.limit {
            return Err(std::io::Error::new(
                std::io::ErrorKind::FileTooLarge,
                format!("{} exceeds {} bytes", self.field, self.limit),
            ));
        }
        self.bytes.extend_from_slice(input);
        Ok(input.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn encode_limited<T: serde::Serialize>(
    field: &'static str,
    value: &T,
    limit: usize,
) -> Result<Vec<u8>, DurableError> {
    let mut writer = LimitedWriter {
        bytes: Vec::new(),
        limit,
        field,
    };
    serde_json::to_writer(&mut writer, value).map_err(|error| {
        if writer.bytes.len() >= limit
            || error.io_error_kind() == Some(std::io::ErrorKind::FileTooLarge)
        {
            DurableError::TooLarge {
                field,
                size: writer.bytes.len().saturating_add(1),
                limit,
            }
        } else {
            DurableError::Rejected(error.to_string())
        }
    })?;
    Ok(writer.bytes)
}

fn bounded_text(field: &'static str, value: &str, limit: usize) -> Result<(), DurableError> {
    bounded(field, value.len(), limit)
}

fn require_kind(field: &'static str, value: &str, allowed: &[&str]) -> Result<(), DurableError> {
    bounded_text(field, value, 64)?;
    if allowed.contains(&value) {
        Ok(())
    } else {
        Err(DurableError::Rejected(format!("unknown {field}: {value}")))
    }
}

fn validate_submission_shape(submission: &SubmissionRecord) -> Result<(), DurableError> {
    match submission.status.as_str() {
        "pending" if submission.answer_id.is_none() && submission.reason.is_none() => Ok(()),
        "done" if submission.answer_id.is_some() && submission.reason.is_none() => Ok(()),
        "failed" | "aborted" if submission.answer_id.is_none() && submission.reason.is_some() => {
            Ok(())
        }
        "pending" | "done" | "failed" | "aborted" => Err(DurableError::Rejected(
            "submission status/answer/reason mismatch".into(),
        )),
        _ => Err(DurableError::Rejected("unknown submission status".into())),
    }
}

pub fn validate_batch(
    snapshot: &StorageSnapshot,
    batch: &CommitBatch,
) -> Result<Vec<u8>, DurableError> {
    let expected_next_seq = snapshot
        .next_seq
        .checked_add(1)
        .filter(|value| *value <= MAX_ID)
        .ok_or_else(|| DurableError::Range("next_seq overflow".into()))?;
    if batch.seq.get() != snapshot.next_seq || batch.next_seq != expected_next_seq {
        return Err(DurableError::Rejected(
            "commit sequence gap or rollback".into(),
        ));
    }
    if batch.next_id == 0 || batch.next_id > MAX_ID {
        return Err(DurableError::Range("next_id outside 1..=i64::MAX".into()));
    }
    if batch.next_id < snapshot.next_id {
        return Err(DurableError::Rejected("next_id rollback".into()));
    }

    // Entries, tasks and submissions draw from one durable record-ID namespace.
    // Existing task/submission IDs may be updated only in their original type.
    let mut allocated_ids = snapshot
        .entries
        .keys()
        .map(|id| id.get())
        .chain(snapshot.tasks.keys().map(|id| id.get()))
        .chain(snapshot.submissions.keys().map(|id| id.get()))
        .chain(
            snapshot
                .conversations
                .values()
                .filter(|record| record.created_seq.is_some())
                .map(|record| record.id.get()),
        )
        .collect::<HashSet<_>>();
    let mut max_new_id = 0u64;
    let mut new_conversations = HashSet::new();
    for conversation in &batch.conversations {
        if conversation.created_seq != Some(batch.seq)
            || conversation.id.get() < snapshot.next_id
            || conversation.id.get() == 1
            || snapshot.conversations.contains_key(&conversation.id)
            || !new_conversations.insert(conversation.id)
            || !allocated_ids.insert(conversation.id.get())
        {
            return Err(DurableError::Rejected(
                "invalid or duplicate conversation creation".into(),
            ));
        }
        max_new_id = max_new_id.max(conversation.id.get());
    }
    // Entry writes are ordered within a transaction. Later writes can refer to
    // an earlier entry in this batch, but never a forward/foreign entry.
    let has_context_references = batch
        .entries
        .iter()
        .any(|entry| entry.kind == "context" || !super::entries::native_kind(&entry.kind));
    let mut batch_prior_entries = HashMap::<EntryId, &EntryRecord>::new();

    for entry in &batch.entries {
        super::entries::validate_kind(&entry.kind)?;
        if entry.conversation_id.get() > MAX_ID {
            return Err(DurableError::Range("conversation id outside range".into()));
        }
        // Reject excessive JSON before cloning/deserializing context payloads.
        validate_json_shape("entry", &entry.value, MAX_ENTRY_BYTES)?;
        let _ = encode_limited("entry", &entry.value, MAX_ENTRY_BYTES)?;
        let visible = |target: EntryId| {
            snapshot
                .entries
                .get(&target)
                .or_else(|| batch_prior_entries.get(&target).copied())
                .is_some_and(|prior| prior.conversation_id == entry.conversation_id)
        };
        if entry.kind == "context" {
            super::context::validate_update(visible, entry.id, &entry.value)?;
        }
        if !super::entries::native_kind(&entry.kind) {
            let update = super::entries::EntryPayload::decode(&entry.value)?.context();
            if matches!(update.head, Some(super::context::ContextHead::SelfEntry(_))) {
                return Err(DurableError::Rejected(
                    "generic stored head must be resolved".into(),
                ));
            }
            super::context::validate_contribution(visible, entry.id, &update, true)?;
        }
        if entry.id.get() < snapshot.next_id {
            return Err(DurableError::Rejected("entry id is below next_id".into()));
        }
        if !allocated_ids.insert(entry.id.get()) {
            return Err(DurableError::Rejected("duplicate durable record id".into()));
        }
        max_new_id = max_new_id.max(entry.id.get());
        if entry.created_seq != batch.seq {
            return Err(DurableError::Rejected("entry sequence mismatch".into()));
        }
        if has_context_references {
            batch_prior_entries.insert(entry.id, entry);
        }
    }

    let mut task_ids = snapshot.tasks.keys().copied().collect::<HashSet<_>>();
    let mut task_batch_ids = HashSet::new();
    for task in &batch.tasks {
        require_kind("task kind", &task.kind, &["generation", "tool"])?;
        if task.version == 0 {
            return Err(DurableError::Rejected("task version must be >= 1".into()));
        }
        if task.started_at.is_some_and(|time| time < 0)
            || task.ended_at.is_some_and(|time| time < 0)
            || (!task.state.terminal() && task.ended_at.is_some())
        {
            return Err(DurableError::Rejected(
                "invalid task lifecycle timestamps".into(),
            ));
        }
        validate_json_shape("task input", &task.input, MAX_TASK_FIELD_BYTES)?;
        validate_json_shape("task checkpoint", &task.checkpoint, MAX_TASK_FIELD_BYTES)?;
        let _ = encode_limited("task input", &task.input, MAX_TASK_FIELD_BYTES)?;
        let _ = encode_limited("task checkpoint", &task.checkpoint, MAX_TASK_FIELD_BYTES)?;
        if let Some(outcome) = &task.outcome {
            validate_json_shape("task outcome", outcome, MAX_TASK_FIELD_BYTES)?;
            let _ = encode_limited("task outcome", outcome, MAX_TASK_FIELD_BYTES)?;
        }
        if !task_batch_ids.insert(task.id) {
            return Err(DurableError::Rejected("duplicate task update".into()));
        }
        if let Some(previous) = snapshot.tasks.get(&task.id) {
            if previous.state.terminal() {
                return Err(DurableError::Rejected("terminal task is immutable".into()));
            }
            if !valid_transition(&previous.state, &task.state) {
                return Err(DurableError::Rejected(
                    "invalid task state transition".into(),
                ));
            }
            if previous.started_at.is_some() && previous.started_at != task.started_at {
                return Err(DurableError::Rejected("task startedAt is immutable".into()));
            }
            if previous.conversation_id != task.conversation_id
                || previous.kind != task.kind
                || previous.version != task.version
                || previous.owner_task_id != task.owner_task_id
            {
                return Err(DurableError::Rejected(
                    "immutable task identity changed".into(),
                ));
            }
        } else {
            if task.id.get() < snapshot.next_id {
                return Err(DurableError::Rejected("task id is below next_id".into()));
            }
            if !allocated_ids.insert(task.id.get()) {
                return Err(DurableError::Rejected("duplicate durable record id".into()));
            }
            max_new_id = max_new_id.max(task.id.get());
            task_ids.insert(task.id);
        }
        if task.updated_seq != batch.seq {
            return Err(DurableError::Rejected("task sequence mismatch".into()));
        }
        if task.state.terminal() != task.outcome.is_some() {
            return Err(DurableError::Rejected(
                "terminal task/outcome mismatch".into(),
            ));
        }
    }
    let tasks_by_id = snapshot
        .tasks
        .iter()
        .map(|(id, task)| (*id, task))
        .chain(batch.tasks.iter().map(|task| (task.id, task)))
        .collect::<HashMap<_, _>>();
    for conversation in &batch.conversations {
        if let Some(owner) = &conversation.owner {
            let task = tasks_by_id
                .get(&owner.task_id)
                .ok_or_else(|| DurableError::Rejected("conversation owner task missing".into()))?;
            if task.state.terminal()
                || task.conversation_id != owner.conversation_id
                || owner.conversation_id == conversation.id
                || !(snapshot.conversations.contains_key(&owner.conversation_id)
                    || new_conversations.contains(&owner.conversation_id))
            {
                return Err(DurableError::Rejected("invalid conversation owner".into()));
            }
            let mut seen = HashSet::from([conversation.id]);
            let mut ancestor = Some(owner.conversation_id);
            while let Some(id) = ancestor {
                if !seen.insert(id) {
                    return Err(DurableError::Rejected(
                        "conversation ownership cycle".into(),
                    ));
                }
                ancestor = batch
                    .conversations
                    .iter()
                    .find(|record| record.id == id)
                    .or_else(|| snapshot.conversations.get(&id))
                    .and_then(|record| record.owner.as_ref().map(|owner| owner.conversation_id));
            }
        }
    }
    for entry in &batch.entries {
        if let Some(task_id) = entry.by_task_id {
            let task = tasks_by_id
                .get(&task_id)
                .ok_or_else(|| DurableError::Rejected("invalid entry task reference".into()))?;
            if task.conversation_id != entry.conversation_id {
                return Err(DurableError::Rejected(
                    "cross-conversation entry task reference".into(),
                ));
            }
        }
    }
    for task in &batch.tasks {
        if let Some(owner) = task.owner_task_id {
            let owner_task = tasks_by_id
                .get(&owner)
                .ok_or_else(|| DurableError::Rejected("invalid task owner".into()))?;
            if owner == task.id || owner_task.conversation_id != task.conversation_id {
                return Err(DurableError::Rejected("invalid task owner".into()));
            }
            let mut cursor = Some(owner);
            let mut seen = HashSet::from([task.id]);
            while let Some(id) = cursor {
                if !seen.insert(id) {
                    return Err(DurableError::Rejected("task owner cycle".into()));
                }
                cursor = tasks_by_id.get(&id).and_then(|record| record.owner_task_id);
            }
        }
    }

    let entries_by_id = snapshot
        .entries
        .iter()
        .map(|(id, entry)| (*id, entry))
        .chain(batch.entries.iter().map(|entry| (entry.id, entry)))
        .collect::<HashMap<_, _>>();
    let mut submission_batch_ids = HashSet::new();
    let mut request_keys = HashSet::new();
    for submission in &batch.submissions {
        if !submission_batch_ids.insert(submission.id) {
            return Err(DurableError::Rejected("duplicate submission update".into()));
        }
        bounded_text("submission status", &submission.status, 16)?;
        validate_submission_shape(submission)?;
        if let Some(reason) = &submission.reason {
            validate_json_shape("submission reason", reason, MAX_SUBMISSION_BYTES)?;
            let _ = encode_limited("submission reason", reason, MAX_SUBMISSION_BYTES)?;
        }
        if let Some(request_id) = &submission.request_id {
            bounded_text("request_id", request_id, MAX_SUBMISSION_BYTES)?;
            let key = (submission.conversation_id, request_id.clone());
            if !request_keys.insert(key.clone()) {
                return Err(DurableError::Rejected("duplicate request_id update".into()));
            }
            if snapshot
                .request_ids
                .get(&key)
                .is_some_and(|existing_id| *existing_id != submission.id)
            {
                return Err(DurableError::Rejected("duplicate request_id".into()));
            }
        }
        let input_entry = entries_by_id
            .get(&submission.entry_id)
            .ok_or_else(|| DurableError::Rejected("invalid submission entry reference".into()))?;
        if input_entry.conversation_id != submission.conversation_id {
            return Err(DurableError::Rejected(
                "cross-conversation submission entry".into(),
            ));
        }
        if let Some(answer_id) = submission.answer_id {
            let answer = entries_by_id.get(&answer_id).ok_or_else(|| {
                DurableError::Rejected("invalid submission answer reference".into())
            })?;
            if answer.conversation_id != submission.conversation_id {
                return Err(DurableError::Rejected(
                    "cross-conversation submission answer".into(),
                ));
            }
        }
        if let Some(previous) = snapshot.submissions.get(&submission.id) {
            if previous.conversation_id != submission.conversation_id
                || previous.request_id != submission.request_id
                || previous.entry_id != submission.entry_id
            {
                return Err(DurableError::Rejected(
                    "immutable submission identity changed".into(),
                ));
            }
            if previous.status != "pending" {
                return Err(DurableError::Rejected(
                    "terminal submission is immutable".into(),
                ));
            }
        } else {
            if submission.id.get() < snapshot.next_id {
                return Err(DurableError::Rejected(
                    "submission id is below next_id".into(),
                ));
            }
            if !allocated_ids.insert(submission.id.get()) {
                return Err(DurableError::Rejected("duplicate durable record id".into()));
            }
            max_new_id = max_new_id.max(submission.id.get());
        }
        if submission.updated_seq != batch.seq {
            return Err(DurableError::Rejected(
                "submission sequence mismatch".into(),
            ));
        }
    }
    let expected_next_id = if max_new_id == 0 {
        snapshot.next_id
    } else {
        max_new_id
            .checked_add(1)
            .filter(|value| *value <= MAX_ID)
            .ok_or_else(|| DurableError::Range("next_id overflow".into()))?
    };
    if batch.next_id != expected_next_id {
        return Err(DurableError::Rejected(
            "next_id is not the durable high-water mark".into(),
        ));
    }

    let mut document_keys = HashSet::new();
    for document in &batch.documents {
        require_kind(
            "document kind",
            &document.kind,
            &[
                "pi.live",
                "pi.usage",
                "pi.checkpoint",
                "pi.agent",
                "pi.inbox",
                "pi.provider",
            ],
        )?;
        if document.version == 0 {
            return Err(DurableError::Rejected(
                "document version must be >= 1".into(),
            ));
        }
        validate_json_shape("document", &document.value, MAX_DOCUMENT_BYTES)?;
        let _ = encode_limited("document", &document.value, MAX_DOCUMENT_BYTES)?;
        if !document_keys.insert((document.conversation_id, document.kind.as_str())) {
            return Err(DurableError::Rejected("duplicate document update".into()));
        }
        if document.updated_seq != batch.seq {
            return Err(DurableError::Rejected("document sequence mismatch".into()));
        }
    }
    encode_limited("commit batch", batch, MAX_COMMIT_BYTES)
}

fn valid_transition(previous: &TaskState, next: &TaskState) -> bool {
    use TaskState::*;
    matches!(
        (previous, next),
        (Pending, Pending | Running | Aborted)
            | (
                Running,
                Pending | Running | Completing | Succeeded | Failed | Aborted
            )
            | (Completing, Completing | Succeeded | Failed | Aborted)
    )
}
