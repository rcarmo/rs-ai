//! Generic entry drafts alongside the existing native execution record kinds.
//! The native journal stores generic fields in EntryRecord.value, not as the
//! upstream flat EntryRecord wire format.
use super::context::{ContextEdit, ContextHead, ContextUpdate};
use super::types::*;
use crate::types::Message;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A passive transcript entry with host-defined kind and optional model context.
/// Native execution kinds are reserved; upstream `pi.*` kinds are permitted.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EntryDraft {
    pub kind: String,
    #[serde(
        default,
        deserialize_with = "present_data",
        skip_serializing_if = "Option::is_none"
    )]
    pub data: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<Vec<Message>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head: Option<ContextHead>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub edits: Vec<ContextEdit>,
}

impl EntryDraft {
    pub fn new(kind: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            data: None,
            model: None,
            head: None,
            edits: Vec::new(),
        }
    }

    pub(crate) fn into_record(
        self,
        conversation: ConversationId,
        id: EntryId,
        seq: CommitSeq,
    ) -> Result<EntryRecord, DurableError> {
        validate_kind(&self.kind)?;
        if native_kind(&self.kind) {
            return Err(DurableError::Rejected(
                "native execution entry kind is reserved".into(),
            ));
        }
        if let Some(data) = &self.data {
            validate_json_shape("entry data", data, MAX_ENTRY_BYTES)?;
        }
        let head = self.head.map(|head| match head {
            ContextHead::SelfEntry(_) => ContextHead::Entry(id),
            other => other,
        });
        let value = serde_json::to_value(EntryPayload {
            data: self.data,
            model: self.model,
            head,
            edits: self.edits,
        })
        .map_err(|error| DurableError::Rejected(error.to_string()))?;
        Ok(EntryRecord {
            id,
            conversation_id: conversation,
            kind: self.kind,
            value,
            by_task_id: None,
            created_seq: seq,
        })
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct EntryPayload {
    #[serde(
        default,
        deserialize_with = "present_data",
        skip_serializing_if = "Option::is_none"
    )]
    data: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    model: Option<Vec<Message>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    head: Option<ContextHead>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    edits: Vec<ContextEdit>,
}

impl EntryPayload {
    pub(crate) fn decode(value: &Value) -> Result<Self, DurableError> {
        serde_json::from_value(value.clone())
            .map_err(|_| DurableError::Rejected("invalid generic entry payload".into()))
    }
    pub(crate) fn context(self) -> ContextUpdate {
        ContextUpdate {
            messages: self.model.unwrap_or_default(),
            head: self.head,
            edits: self.edits,
        }
    }
}

// Missing data and explicitly supplied JSON null have different wire meaning.
fn present_data<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<Value>, D::Error> {
    Value::deserialize(deserializer).map(Some)
}

pub(crate) fn native_kind(kind: &str) -> bool {
    matches!(
        kind,
        "user" | "assistant" | "tool_result" | "model_error" | "context"
    )
}

pub(crate) fn validate_kind(kind: &str) -> Result<(), DurableError> {
    if kind.is_empty() {
        return Err(DurableError::Rejected(
            "entry kind must not be empty".into(),
        ));
    }
    if kind.len() > 64 {
        return Err(DurableError::TooLarge {
            field: "entry kind",
            size: kind.len(),
            limit: 64,
        });
    }
    Ok(())
}
