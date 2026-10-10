//! Native durable message reconstruction for the single-conversation entry log.
//! Native context-update entries adapt heads/edits; fork ancestry remains absent.
use super::model::{DurableContent, DurableMessage, ModelIntent, to_message_for_durable};
use super::storage::StorageSnapshot;
use super::types::*;
use crate::types::{ContentBlock, Message, Role, StopReason};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase", deny_unknown_fields)]
pub enum ContextEdit {
    Omit {
        target: EntryId,
    },
    Replace {
        target: EntryId,
        messages: Vec<Message>,
    },
}
impl ContextEdit {
    fn target(&self) -> EntryId {
        match self {
            Self::Omit { target } | Self::Replace { target, .. } => *target,
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ContextHead {
    Entry(EntryId),
    SelfEntry(SelfHead),
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum SelfHead {
    #[serde(rename = "self")]
    SelfEntry,
}

/// Native log contribution. Heads select a retained lower bound; edits can omit
/// or replace a visible earlier entry. These values never dispatch model work.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContextUpdate {
    #[serde(default)]
    pub messages: Vec<Message>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head: Option<ContextHead>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub edits: Vec<ContextEdit>,
}

pub(crate) fn validate_update(
    snapshot: &StorageSnapshot,
    conversation: ConversationId,
    id: EntryId,
    value: &Value,
) -> Result<(), DurableError> {
    let update: ContextUpdate = serde_json::from_value(value.clone())
        .map_err(|_| DurableError::Rejected("invalid native context update".into()))?;
    validate_contribution(snapshot, conversation, id, &update, false)
}

pub(crate) fn validate_contribution(
    snapshot: &StorageSnapshot,
    conversation: ConversationId,
    id: EntryId,
    update: &ContextUpdate,
    allow_self: bool,
) -> Result<(), DurableError> {
    if update.messages.len() > 4096 || update.edits.len() > 4096 {
        return Err(DurableError::Rejected(
            "too many context messages/edits".into(),
        ));
    }
    let visible = |target: EntryId| {
        snapshot
            .entries
            .get(&target)
            .is_some_and(|entry| target < id && entry.conversation_id == conversation)
    };
    if let Some(ContextHead::Entry(target)) = update.head
        && !(visible(target) || (allow_self && target == id))
    {
        return Err(DurableError::Rejected(
            "context head is not a visible prior entry".into(),
        ));
    }
    for edit in &update.edits {
        if !visible(edit.target()) {
            return Err(DurableError::Rejected(
                "context edit target is not a visible prior entry".into(),
            ));
        }
        if matches!(edit, ContextEdit::Replace { messages, .. } if messages.len() > 4096) {
            return Err(DurableError::Rejected(
                "too many replacement messages".into(),
            ));
        }
    }
    Ok(())
}

/// The newest native head marker and its resolved retained lower bound.
#[derive(Clone, Debug, PartialEq)]
pub struct ContextMarker {
    pub entry: EntryRecord,
    pub head: EntryId,
}

/// Detached active entries and their model contributions. Contributions align
/// with entries before tool-result repair and leading-system promotion.
#[derive(Clone, Debug, Default)]
pub struct ContextView {
    pub head: Option<ContextMarker>,
    pub entries: Vec<EntryRecord>,
    pub contributions: Vec<Vec<Message>>,
    pub messages: Vec<Message>,
}

pub(crate) fn view(
    snapshot: &StorageSnapshot,
    conversation: ConversationId,
    at: Option<EntryId>,
) -> Result<ContextView, DurableError> {
    read(snapshot, conversation, at, true)
}

pub(crate) fn messages(
    snapshot: &StorageSnapshot,
    conversation: ConversationId,
    at: Option<EntryId>,
) -> Result<Vec<Message>, DurableError> {
    Ok(read(snapshot, conversation, at, false)?.messages)
}

fn read(
    snapshot: &StorageSnapshot,
    conversation: ConversationId,
    at: Option<EntryId>,
    collect_view: bool,
) -> Result<ContextView, DurableError> {
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
        collect_view,
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
    Ok(derive(snapshot, entries, false)?.messages)
}

fn derive<'a>(
    snapshot: &StorageSnapshot,
    entries: impl Iterator<Item = &'a EntryRecord>,
    collect_view: bool,
) -> Result<ContextView, DurableError> {
    let entries = entries.collect::<Vec<_>>();
    let mut head = None;
    let mut marker = None;
    let mut edits = HashMap::new();
    let mut updates = HashMap::new();
    for entry in &entries {
        let update: ContextUpdate = if entry.kind == "context" {
            serde_json::from_value(entry.value.clone())
                .map_err(|_| DurableError::Corrupt("invalid persisted context update".into()))?
        } else if !super::entries::native_kind(&entry.kind) {
            super::entries::EntryPayload::decode(&entry.value)
                .map_err(|_| DurableError::Corrupt("invalid persisted generic entry".into()))?
                .context()
        } else {
            continue;
        };
        if let Some(value) = update.head {
            head = Some(match value {
                ContextHead::Entry(id) => id,
                ContextHead::SelfEntry(_) => entry.id,
            });
            marker = Some(entry.id);
        }
        updates.insert(entry.id, update);
    }
    // Only edits inside the retained range count. A later reset must not
    // resurrect edits from transcript entries before its chosen head.
    for entry in &entries {
        if head.is_none_or(|head| entry.id >= head)
            && let Some(update) = updates.get(&entry.id)
        {
            for edit in &update.edits {
                edits.insert(edit.target(), edit.clone());
            }
        }
    }
    // Upstream selectActive places the newest head marker first, followed by
    // retained non-head entries. Its position is not its chronological log ID.
    // Edits from older markers in the retained range still participate above.
    let active = entries
        .iter()
        .copied()
        .filter(|entry| marker == Some(entry.id))
        .chain(entries.iter().copied().filter(|entry| {
            head.is_none_or(|head| entry.id >= head)
                && updates
                    .get(&entry.id)
                    .is_none_or(|update| update.head.is_none())
        }));
    let mut view = ContextView::default();
    if collect_view && let Some(marker) = marker {
        view.head = entries
            .iter()
            .find(|entry| entry.id == marker)
            .map(|entry| ContextMarker {
                entry: (*entry).clone(),
                head: head.expect("head marker has a resolved head"),
            });
    }
    let mut result = Vec::new();
    for entry in active {
        let start = result.len();
        // Append directly to the final buffer. Full-view callers additionally
        // detach the contributed slice; messages-only readers need no scratch.
        contribution(
            snapshot,
            entry,
            updates.get(&entry.id),
            edits.get(&entry.id),
            &mut result,
        )?;
        if collect_view {
            view.entries.push(entry.clone());
            view.contributions.push(result[start..].to_vec());
        }
    }
    // Repair results before testing the leading-system prefix: orphan results
    // can otherwise hide a baseline system message behind earlier user inputs.
    let mut result = order_tool_results(result);
    // Lead with baseline system metadata only while preceding messages are user
    // inputs, matching upstream provider prompt/cache prefix ordering.
    if let Some(index) = result.iter().position(|message| message.role != Role::User)
        && result[index].role == Role::System
        && index > 0
    {
        let system = result.remove(index);
        result.insert(0, system);
    }
    view.messages = result;
    Ok(view)
}

fn included(message: &Message) -> bool {
    message.role != Role::Assistant
        || !matches!(
            message.stop_reason,
            Some(StopReason::Aborted | StopReason::Error | StopReason::Deferred)
        )
}

fn contribution(
    snapshot: &StorageSnapshot,
    entry: &EntryRecord,
    update: Option<&ContextUpdate>,
    edit: Option<&ContextEdit>,
    result: &mut Vec<Message>,
) -> Result<(), DurableError> {
    if let Some(edit) = edit {
        if let ContextEdit::Replace { messages, .. } = edit {
            result.extend(messages.iter().filter(|message| included(message)).cloned());
        }
        return Ok(());
    }
    if let Some(update) = update {
        result.extend(
            update
                .messages
                .iter()
                .filter(|message| included(message))
                .cloned(),
        );
        return Ok(());
    }
    let mut message = match entry.kind.as_str() {
        "user" => to_message_for_durable(&DurableMessage {
            role: "user".into(),
            text: text(entry)?,
        }),
        "assistant" => {
            if let Some(message) = entry.value.get("message") {
                let message: Message = serde_json::from_value(message.clone()).map_err(|_| {
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
                message.duration_ms = entry.value.get("durationMs").and_then(Value::as_u64);
                message.stop_reason = entry
                    .value
                    .get("stop_reason")
                    .map(|value| {
                        serde_json::from_value(value.clone()).map_err(|_| {
                            DurableError::Corrupt("invalid assistant stop reason".into())
                        })
                    })
                    .transpose()?;
                message.response_id = entry
                    .value
                    .get("response_id")
                    .and_then(Value::as_str)
                    .map(str::to_owned);
                if let Some(task) = entry.by_task_id.and_then(|id| snapshot.tasks.get(&id)) {
                    let intent: ModelIntent =
                        serde_json::from_value(task.input.clone()).map_err(|_| {
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
        _ => return Ok(()),
    };
    // Legacy text entries have no provider timestamp. Do not invent timing
    // for reconstructed messages or count them as newly streamed responses.
    if entry.value.get("message").is_none() {
        message.timestamp = 0;
    }
    if included(&message) {
        result.push(message);
    }
    Ok(())
}

pub(crate) fn text_messages(messages: Vec<Message>) -> Vec<DurableMessage> {
    messages
        .into_iter()
        .filter_map(|message| {
            let role = match message.role {
                Role::User => "user",
                Role::Assistant => "assistant",
                _ => return None,
            };
            let text = message
                .content
                .into_iter()
                .filter_map(|block| match block {
                    ContentBlock::Text { text, .. } => Some(text),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("");
            if text.is_empty() {
                return None;
            }
            Some(DurableMessage {
                role: role.into(),
                text,
            })
        })
        .collect()
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
    // The usual text-only transcript needs neither repair nor a second deep
    // clone of every message. Preserve the owned buffer unchanged.
    if !messages.iter().any(|message| {
        message.role == Role::ToolResult
            || (message.role == Role::Assistant
                && message
                    .content
                    .iter()
                    .any(|block| matches!(block, ContentBlock::ToolCall { .. })))
    }) {
        return messages;
    }
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
                let mut missing = crate::user_message(
                    "Tool result unavailable: history ends before this call completed.",
                );
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
