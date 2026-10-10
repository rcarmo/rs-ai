use crate::durable::model::{ModelIntent, PinnedModel, PinnedOptions};
use crate::durable::storage::StorageSnapshot;
use crate::durable::types::*;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SubmitRequest {
    pub request_id: String,
    pub content: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SubmissionHandle {
    pub id: SubmissionId,
    pub task_id: TaskId,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SubmissionView {
    pub id: SubmissionId,
    pub task_id: TaskId,
    pub request_id: String,
    pub status: String,
    pub content: String,
    pub answer: Option<String>,
    pub reason: Option<Value>,
    pub usage: Option<Value>,
}

pub(crate) fn build_initial_batch(
    snapshot: &StorageSnapshot,
    conversation_id: ConversationId,
    request: &SubmitRequest,
    model: PinnedModel,
    options: PinnedOptions,
) -> Result<(CommitBatch, SubmissionHandle), DurableError> {
    validate_request(request)?;
    model.validate()?;
    options.validate()?;
    let seq = CommitSeq::new(snapshot.next_seq)?;
    let entry_id = EntryId::new(snapshot.next_id)?;
    let task_id = TaskId::new(next(snapshot.next_id)?)?;
    let submission_id = SubmissionId::new(next(task_id.get())?)?;
    let next_id = next(submission_id.get())?;
    let (session_id, provider_document) =
        super::provider::prepare_provider_session(snapshot, conversation_id, seq)?;
    let intent = ModelIntent {
        model,
        options,
        provider_session_id: Some(session_id),
        offered_tools: vec![],
        system_prompt: None,
        context: vec![],
        native_messages: None,
        context_cutoff: 1,
        logical_attempt: 1,
    };
    intent.validate()?;
    let agent_value = json!({"model":intent.model.clone(),"options":intent.options.clone()});
    let mut pending = snapshot
        .submissions
        .values()
        .filter(|value| value.conversation_id == conversation_id && value.status == "pending")
        .map(|value| value.id.get())
        .collect::<Vec<_>>();
    pending.push(submission_id.get());
    pending.sort_unstable();
    let live_value = if pending.len() == 1 {
        json!({"submission_id":submission_id.get(),"task_id":task_id.get(),"status":"pending"})
    } else {
        snapshot
            .documents
            .get(&(conversation_id, "pi.live".into()))
            .map(|document| document.value.clone())
            .ok_or_else(|| DurableError::Corrupt("queued submission lacks live document".into()))?
    };
    let intent_value =
        serde_json::to_value(&intent).map_err(|error| DurableError::Rejected(error.to_string()))?;
    let mut batch = CommitBatch {
        seq,
        next_id,
        next_seq: next(seq.get())?,
        entries: vec![EntryRecord {
            id: entry_id,
            conversation_id,
            kind: "user".into(),
            value: json!({"text": request.content}),
            by_task_id: Some(task_id),
            created_seq: seq,
        }],
        tasks: vec![TaskRecord {
            id: task_id,
            conversation_id,
            kind: "generation".into(),
            version: 1,
            owner_task_id: None,
            state: TaskState::Pending,
            input: intent_value,
            checkpoint: json!({"phase":"pending","logical_attempt":1}),
            outcome: None,
            abort_requested: false,
            started_at: None,
            ended_at: None,
            updated_seq: seq,
        }],
        submissions: vec![SubmissionRecord {
            id: submission_id,
            conversation_id,
            request_id: Some(request.request_id.clone()),
            status: "pending".into(),
            entry_id,
            answer_id: None,
            reason: None,
            updated_seq: seq,
        }],
        documents: vec![
            DocumentRecord {
                conversation_id,
                kind: "pi.agent".into(),
                version: 1,
                value: agent_value,
                updated_seq: seq,
            },
            DocumentRecord {
                conversation_id,
                kind: "pi.live".into(),
                version: 1,
                value: live_value,
                updated_seq: seq,
            },
            DocumentRecord {
                conversation_id,
                kind: "pi.inbox".into(),
                version: 1,
                value: json!({"pending":pending}),
                updated_seq: seq,
            },
        ],
    };
    if let Some(document) = provider_document {
        batch.documents.push(document);
    }
    Ok((
        batch,
        SubmissionHandle {
            id: submission_id,
            task_id,
        },
    ))
}

pub(crate) fn reacquire(
    snapshot: &StorageSnapshot,
    conversation_id: ConversationId,
    request: &SubmitRequest,
) -> Result<Option<SubmissionHandle>, DurableError> {
    let Some(id) = snapshot
        .request_ids
        .get(&(conversation_id, request.request_id.clone()))
        .copied()
    else {
        return Ok(None);
    };
    let record = snapshot
        .submissions
        .get(&id)
        .ok_or_else(|| DurableError::Corrupt("request index lacks submission".into()))?;
    let entry = snapshot
        .entries
        .get(&record.entry_id)
        .ok_or_else(|| DurableError::Corrupt("submission lacks input entry".into()))?;
    if entry.value.get("text").and_then(Value::as_str) != Some(request.content.as_str()) {
        return Err(DurableError::Rejected("request_id payload conflict".into()));
    }
    let task_id = entry
        .by_task_id
        .ok_or_else(|| DurableError::Corrupt("submission input lacks generation task".into()))?;
    let task = snapshot
        .tasks
        .get(&task_id)
        .filter(|task| task.conversation_id == conversation_id && task.kind == "generation")
        .ok_or_else(|| DurableError::Corrupt("submission lacks generation task".into()))?;
    Ok(Some(SubmissionHandle {
        id,
        task_id: task.id,
    }))
}

pub(crate) fn view(
    snapshot: &StorageSnapshot,
    conversation_id: ConversationId,
    id: SubmissionId,
) -> Result<SubmissionView, DurableError> {
    let record = snapshot
        .submissions
        .get(&id)
        .filter(|record| record.conversation_id == conversation_id)
        .ok_or_else(|| DurableError::Rejected("unknown submission".into()))?;
    let entry = snapshot
        .entries
        .get(&record.entry_id)
        .ok_or_else(|| DurableError::Corrupt("submission lacks input entry".into()))?;
    let content = entry
        .value
        .get("text")
        .and_then(Value::as_str)
        .ok_or_else(|| DurableError::Corrupt("invalid input entry".into()))?
        .to_string();
    let answer = record
        .answer_id
        .and_then(|answer_id| snapshot.entries.get(&answer_id))
        .and_then(|entry| entry.value.get("text"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let task_id = entry
        .by_task_id
        .ok_or_else(|| DurableError::Corrupt("submission input lacks generation task".into()))?;
    let task = snapshot
        .tasks
        .get(&task_id)
        .filter(|task| task.conversation_id == conversation_id && task.kind == "generation")
        .ok_or_else(|| DurableError::Corrupt("submission lacks generation task".into()))?;
    Ok(SubmissionView {
        id,
        task_id: task.id,
        request_id: record.request_id.clone().unwrap_or_default(),
        status: record.status.clone(),
        content,
        answer,
        reason: record.reason.clone(),
        usage: task
            .outcome
            .as_ref()
            .and_then(|value| value.get("usage"))
            .cloned(),
    })
}

fn validate_request(request: &SubmitRequest) -> Result<(), DurableError> {
    if request.request_id.is_empty() || request.request_id.len() > MAX_SUBMISSION_BYTES {
        return Err(DurableError::Rejected("invalid request_id".into()));
    }
    if request.content.is_empty() || request.content.len() > MAX_SUBMISSION_BYTES {
        return Err(DurableError::TooLarge {
            field: "submission content",
            size: request.content.len(),
            limit: MAX_SUBMISSION_BYTES,
        });
    }
    Ok(())
}

fn next(value: u64) -> Result<u64, DurableError> {
    value
        .checked_add(1)
        .filter(|value| *value <= MAX_ID)
        .ok_or_else(|| DurableError::Range("durable id overflow".into()))
}
