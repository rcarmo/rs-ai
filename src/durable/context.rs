//! Native durable message reconstruction for the single-conversation entry log.
//! Fork ancestry, edits and head markers are separate, unimplemented contracts.
use super::model::{DurableContent, DurableMessage, ModelIntent, to_message_for_durable};
use super::storage::StorageSnapshot;
use super::types::*;
use crate::types::{ContentBlock, Message, Role};
use serde_json::{Value, json};

pub(crate) fn messages(
    snapshot: &StorageSnapshot,
    conversation: ConversationId,
    at: Option<EntryId>,
) -> Result<Vec<Message>, DurableError> {
    if let Some(at) = at
        && !snapshot
            .entries
            .get(&at)
            .is_some_and(|entry| entry.conversation_id == conversation)
    {
        return Err(DurableError::Rejected(format!(
            "entry {} is not visible in this conversation",
            at.get()
        )));
    }
    derive(
        snapshot,
        snapshot.entries.values().filter(|entry| {
            entry.conversation_id == conversation && at.is_none_or(|at| entry.id <= at)
        }),
    )
}

pub(crate) fn for_task(
    snapshot: &StorageSnapshot,
    task_id: TaskId,
) -> Result<Vec<Message>, DurableError> {
    let target = snapshot
        .entries
        .values()
        .find(|entry| entry.by_task_id == Some(task_id) && entry.kind == "user")
        .ok_or_else(|| DurableError::Corrupt("generation lacks input entry".into()))?;
    // Queued inputs can be allocated before a previous generation's answer.
    // Keep that settled generation, but exclude other queued user entries.
    let entries = snapshot
        .entries
        .values()
        .filter(|entry| {
            if entry.conversation_id != target.conversation_id {
                return false;
            }
            if entry.kind == "user" {
                return entry.id <= target.id
                    && (entry.id == target.id
                        || !snapshot.submissions.values().any(|submission| {
                            submission.entry_id == entry.id && submission.status == "pending"
                        }));
            }
            let parent = entry
                .by_task_id
                .and_then(|id| snapshot.tasks.get(&id))
                .map(|task| task.owner_task_id.unwrap_or(task.id));
            parent.map_or(entry.id < target.id, |id| id < task_id)
        })
        .filter(|entry| entry.id != target.id)
        .chain(std::iter::once(target));
    // The native FIFO scheduler places the active input after completed turns,
    // even when its durable ID was allocated while a prior turn was running.
    derive(snapshot, entries)
}

fn derive<'a>(
    snapshot: &StorageSnapshot,
    entries: impl Iterator<Item = &'a EntryRecord>,
) -> Result<Vec<Message>, DurableError> {
    let mut result = Vec::new();
    for entry in entries {
        let mut message = match entry.kind.as_str() {
            "user" => to_message_for_durable(&DurableMessage {
                role: "user".into(),
                text: text(entry)?,
            }),
            "assistant" => {
                if let Some(message) = entry.value.get("message") {
                    let message: Message =
                        serde_json::from_value(message.clone()).map_err(|_| {
                            DurableError::Corrupt("invalid assistant context message".into())
                        })?;
                    if message.role != Role::Assistant {
                        return Err(DurableError::Corrupt(
                            "assistant entry has wrong role".into(),
                        ));
                    }
                    message
                } else {
                    let mut message = to_message_for_durable(&DurableMessage {
                        role: "assistant".into(),
                        text: text(entry)?,
                    });
                    if let Some(content) = entry.value.get("content") {
                        let content: Vec<DurableContent> = serde_json::from_value(content.clone())
                            .map_err(|_| {
                                DurableError::Corrupt("invalid durable assistant content".into())
                            })?;
                        if !content.is_empty() {
                            message.content = content
                                .into_iter()
                                .map(|content| match content {
                                    DurableContent::Text { text } => ContentBlock::Text {
                                        text,
                                        text_signature: None,
                                    },
                                    DurableContent::Thinking { thinking, redacted } => {
                                        ContentBlock::Thinking {
                                            thinking,
                                            redacted: Some(redacted),
                                            thinking_signature: None,
                                        }
                                    }
                                })
                                .collect();
                        }
                    }
                    message.response_id = entry
                        .value
                        .get("response_id")
                        .and_then(Value::as_str)
                        .map(str::to_owned);
                    if let Some(task) = entry.by_task_id.and_then(|id| snapshot.tasks.get(&id)) {
                        let intent: ModelIntent = serde_json::from_value(task.input.clone())
                            .map_err(|_| {
                                DurableError::Corrupt("invalid assistant model identity".into())
                            })?;
                        message.api = Some(intent.model.api().into());
                        message.provider = Some(intent.model.provider().into());
                        message.model = Some(intent.model.id().into());
                    }
                    message
                }
            }
            "tool_result" => {
                let id = entry
                    .value
                    .get("tool_call_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| DurableError::Corrupt("tool result lacks call id".into()))?;
                let name = entry
                    .value
                    .get("tool_name")
                    .and_then(Value::as_str)
                    .ok_or_else(|| DurableError::Corrupt("tool result lacks tool name".into()))?;
                let value = entry
                    .value
                    .get("result")
                    .ok_or_else(|| DurableError::Corrupt("tool result lacks value".into()))?;
                let mut message = crate::user_message(&value.to_string());
                message.role = Role::ToolResult;
                message.tool_call_id = Some(id.into());
                message.tool_name = Some(name.into());
                message.is_error = entry
                    .value
                    .get("is_error")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                message.duration_ms = entry.value.get("durationMs").and_then(Value::as_u64);
                message
            }
            _ => continue,
        };
        // Legacy text entries have no provider timestamp. Do not invent timing
        // for reconstructed messages or count them as newly streamed responses.
        if entry.value.get("message").is_none() {
            message.timestamp = 0;
        }
        result.push(message);
    }
    Ok(order_tool_results(result))
}

fn text(entry: &EntryRecord) -> Result<String, DurableError> {
    entry
        .value
        .get("text")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| DurableError::Corrupt("context entry lacks text".into()))
}

/// Associate only the first matching result before the next assistant, emit in
/// call order, synthesize missing results and discard orphan/duplicate results.
pub(crate) fn order_tool_results(messages: Vec<Message>) -> Vec<Message> {
    let mut result = Vec::new();
    for (index, message) in messages.iter().enumerate() {
        if message.role == Role::ToolResult {
            continue;
        }
        result.push(message.clone());
        if message.role != Role::Assistant {
            continue;
        }
        let next = messages[index + 1..]
            .iter()
            .position(|message| message.role == Role::Assistant)
            .map_or(messages.len(), |next| index + 1 + next);
        for block in &message.content {
            let ContentBlock::ToolCall { id, name, .. } = block else {
                continue;
            };
            let found = messages[index + 1..next].iter().find(|message| {
                message.role == Role::ToolResult && message.tool_call_id.as_deref() == Some(id)
            });
            result.push(found.cloned().unwrap_or_else(|| {
                let mut missing = crate::user_message("Missing tool result");
                missing.role = Role::ToolResult;
                missing.timestamp = message.timestamp;
                missing.tool_call_id = Some(id.clone());
                missing.tool_name = Some(name.clone());
                missing.is_error = true;
                missing.details = Some(json!({"reason":"missing_result"}));
                missing
            }));
        }
    }
    result
}
