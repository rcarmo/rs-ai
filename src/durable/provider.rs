//! Persisted provider-facing identity of one durable conversation.

use crate::durable::storage::StorageSnapshot;
use crate::durable::types::{CommitSeq, ConversationId, DocumentRecord, DurableError};
use serde_json::json;

pub(crate) fn validate_session_id(value: &str) -> Result<(), DurableError> {
    let valid = value.len() == 36
        && value.bytes().enumerate().all(|(index, byte)| {
            if matches!(index, 8 | 13 | 18 | 23) {
                byte == b'-'
            } else {
                byte.is_ascii_hexdigit()
            }
        });
    if valid {
        Ok(())
    } else {
        Err(DurableError::Rejected(
            "invalid provider session UUID".into(),
        ))
    }
}

/// Return the existing identity or prepare its first atomic document write.
/// The caller must commit the returned record before dispatching effects.
pub(crate) fn prepare_provider_session(
    snapshot: &StorageSnapshot,
    conversation_id: ConversationId,
    seq: CommitSeq,
) -> Result<(String, Option<DocumentRecord>), DurableError> {
    if let Some(document) = snapshot
        .documents
        .get(&(conversation_id, "pi.provider".into()))
    {
        if document.version != 1 {
            return Err(DurableError::Corrupt(
                "unsupported provider document version".into(),
            ));
        }
        let session_id = document
            .value
            .get("sessionId")
            .and_then(|value| value.as_str())
            .ok_or_else(|| DurableError::Corrupt("provider document lacks sessionId".into()))?;
        validate_session_id(session_id)
            .map_err(|_| DurableError::Corrupt("invalid persisted provider session UUID".into()))?;
        return Ok((session_id.to_owned(), None));
    }
    let session_id = crate::utils::uuidv7();
    let document = DocumentRecord {
        conversation_id,
        kind: "pi.provider".into(),
        version: 1,
        value: json!({"sessionId":session_id}),
        updated_seq: seq,
    };
    Ok((session_id, Some(document)))
}
