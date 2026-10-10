//! Native conversation-scoped whole-value document incarnations. Built-in
//! runtime documents keep their existing storage and are not copied by this API.
use super::storage::StorageSnapshot;
use super::types::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DocumentHistory {
    Latest,
    Rewindable,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum DocumentFork {
    #[serde(rename = "initial")]
    Initial,
    #[serde(rename = "current")]
    Current,
    #[serde(rename = "asOf")]
    AsOf,
}
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub struct DocumentAddress {
    pub conversation_id: ConversationId,
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DocumentDraft {
    pub address: DocumentAddress,
    pub version: u32,
    pub history: DocumentHistory,
    pub fork: DocumentFork,
    pub value: Value,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GenericDocumentRecord {
    pub id: DocumentId,
    pub address: DocumentAddress,
    pub version: u32,
    pub history: DocumentHistory,
    pub fork: DocumentFork,
    pub value: Value,
    pub created_seq: CommitSeq,
    pub updated_seq: CommitSeq,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retired_seq: Option<CommitSeq>,
}
#[derive(Clone, Copy, Debug)]
pub enum DocumentPoint {
    Current,
    At(CommitSeq),
}

pub(crate) fn validate_size(value: &Value) -> Result<usize, DurableError> {
    struct Counter(usize);
    impl std::io::Write for Counter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            let next = self.0.saturating_add(bytes.len());
            if next > MAX_DOCUMENT_BYTES {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::FileTooLarge,
                    "document size limit",
                ));
            }
            self.0 = next;
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut counter = Counter(0);
    serde_json::to_writer(&mut counter, value).map_err(|error| {
        if error.io_error_kind() == Some(std::io::ErrorKind::FileTooLarge) {
            DurableError::TooLarge {
                field: "document",
                size: counter.0.saturating_add(1),
                limit: MAX_DOCUMENT_BYTES,
            }
        } else {
            DurableError::Rejected(error.to_string())
        }
    })?;
    Ok(counter.0)
}

impl GenericDocumentRecord {
    pub(crate) fn alive(&self, point: DocumentPoint) -> bool {
        match point {
            DocumentPoint::Current => self.retired_seq.is_none(),
            DocumentPoint::At(seq) => {
                self.created_seq <= seq && self.retired_seq.is_none_or(|retired| seq < retired)
            }
        }
    }
    pub(crate) fn shape(&self) -> Result<(), DurableError> {
        super::entries::validate_kind(&self.address.kind)?;
        if matches!(
            self.address.kind.as_str(),
            "pi.live" | "pi.usage" | "pi.checkpoint" | "pi.agent" | "pi.inbox" | "pi.provider"
        ) {
            return Err(DurableError::Rejected(
                "runtime document kind is reserved".into(),
            ));
        }
        if self.version == 0
            || (self.fork == DocumentFork::AsOf && self.history != DocumentHistory::Rewindable)
        {
            return Err(DurableError::Rejected(
                "invalid document version/history/fork policy".into(),
            ));
        }
        if self
            .address
            .key
            .as_ref()
            .is_some_and(|key| key.len() > 1024)
        {
            return Err(DurableError::Rejected(
                "document key exceeds 1024 bytes".into(),
            ));
        }
        if !self.value.is_object() {
            return Err(DurableError::Rejected(
                "document value must be a JSON object".into(),
            ));
        }
        validate_json_shape("document", &self.value, MAX_DOCUMENT_BYTES)?;
        Ok(())
    }
}
impl StorageSnapshot {
    pub fn document(
        &self,
        address: &DocumentAddress,
        point: DocumentPoint,
    ) -> Result<Option<GenericDocumentRecord>, DurableError> {
        if let DocumentPoint::At(seq) = point
            && self.last_seq.is_none_or(|last| seq > last)
        {
            return Err(DurableError::Range(
                "document point exceeds committed history".into(),
            ));
        }
        let record = self
            .generic_documents
            .values()
            .find(|record| record.address == *address && record.alive(point));
        record
            .map(|record| self.document_value(record, point))
            .transpose()
    }
    pub(crate) fn document_value(
        &self,
        record: &GenericDocumentRecord,
        point: DocumentPoint,
    ) -> Result<GenericDocumentRecord, DurableError> {
        if let DocumentPoint::At(seq) = point {
            if record.history != DocumentHistory::Rewindable {
                return Err(DurableError::Rejected(
                    "latest-only document has no historical content".into(),
                ));
            }
            let revision = self
                .document_revisions
                .get(&record.id)
                .and_then(|revisions| revisions.range(..=seq).next_back())
                .ok_or_else(|| {
                    DurableError::Corrupt("rewindable document revision missing".into())
                })?;
            return Ok(GenericDocumentRecord {
                id: record.id,
                address: record.address.clone(),
                version: record.version,
                history: record.history,
                fork: record.fork,
                value: revision.1.clone(),
                created_seq: record.created_seq,
                updated_seq: *revision.0,
                retired_seq: record.retired_seq,
            });
        }
        Ok(record.clone())
    }
}
