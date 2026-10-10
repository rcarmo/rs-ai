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
/// Final content and its schema version at one committed revision. The live
/// incarnation's version may advance without relabelling historical values.
#[derive(Clone, Debug, PartialEq)]
pub struct DocumentRevision {
    pub version: u32,
    pub value: Value,
}

#[derive(Clone, Debug)]
pub struct DocumentQuery {
    pub conversation_id: ConversationId,
    pub kind: Option<String>,
    pub fork: Option<DocumentFork>,
    pub point: DocumentPoint,
    pub scan: super::storage::scan::ScanOptions,
}
impl DocumentQuery {
    pub fn current(conversation_id: ConversationId) -> Self {
        Self {
            conversation_id,
            kind: None,
            fork: None,
            point: DocumentPoint::Current,
            scan: Default::default(),
        }
    }
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

#[cfg(test)]
mod size_tests {
    use super::*;
    #[test]
    fn counted_document_bytes_match_escaped_json_and_enforce_limit() {
        let value = serde_json::json!({"text":"quotes\"\n\t\\ unicode 𐐀"});
        assert_eq!(
            validate_size(&value).unwrap(),
            serde_json::to_vec(&value).unwrap().len()
        );
        assert!(matches!(
            validate_size(&serde_json::json!({"text":"x".repeat(MAX_DOCUMENT_BYTES)})),
            Err(DurableError::TooLarge { .. })
        ));
    }
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
    pub fn query_documents(
        &self,
        query: &DocumentQuery,
    ) -> Result<super::storage::scan::ScanPage<GenericDocumentRecord>, DurableError> {
        use super::storage::scan::{ScanCursor, ScanOrder, ScanPage, start};
        self.check_document_point(query.point)?;
        let (order, after) = start(&query.scan, ScanOrder::Ascending)?;
        let matches = |record: &&GenericDocumentRecord| {
            record.address.conversation_id == query.conversation_id
                && query
                    .kind
                    .as_ref()
                    .is_none_or(|kind| record.address.kind == *kind)
                && query.fork.is_none_or(|fork| record.fork == fork)
                && record.alive(query.point)
                && after.is_none_or(|after| match order {
                    ScanOrder::Ascending => record.id.get() > after,
                    ScanOrder::Descending => record.id.get() < after,
                })
        };
        let mut records: Box<dyn Iterator<Item = &GenericDocumentRecord> + '_> = match order {
            ScanOrder::Ascending => Box::new(self.generic_documents.values().filter(matches)),
            ScanOrder::Descending => {
                Box::new(self.generic_documents.values().rev().filter(matches))
            }
        };
        let mut items = Vec::new();
        for record in records.by_ref().take(query.scan.limit) {
            items.push(self.document_value(record, query.point)?);
        }
        let cursor = if records.next().is_some() {
            items.last().map(|record| ScanCursor {
                after: record.id.get(),
                order: Some(order),
            })
        } else {
            None
        };
        Ok(ScanPage { items, cursor })
    }
    fn check_document_point(&self, point: DocumentPoint) -> Result<(), DurableError> {
        if let DocumentPoint::At(seq) = point
            && self.last_seq.is_none_or(|last| seq > last)
        {
            return Err(DurableError::Range(
                "document point exceeds committed history".into(),
            ));
        }
        Ok(())
    }
    pub fn document(
        &self,
        address: &DocumentAddress,
        point: DocumentPoint,
    ) -> Result<Option<GenericDocumentRecord>, DurableError> {
        self.check_document_point(point)?;
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
                version: revision.1.version,
                history: record.history,
                fork: record.fork,
                value: revision.1.value.clone(),
                created_seq: record.created_seq,
                updated_seq: *revision.0,
                retired_seq: record.retired_seq,
            });
        }
        Ok(record.clone())
    }
}
