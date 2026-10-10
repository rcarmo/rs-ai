//! Process-local typed tokens for native conversation documents. No registry or
//! tracked drafts; read migrations are detached and never rewrite the journal.
use super::documents::*;
use super::types::*;
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;
use std::sync::Arc;

type Initial<D, I> = Arc<dyn Fn(&I) -> Result<D, DurableError> + Send + Sync>;
type Migration<D> = Arc<dyn Fn(Value, u32) -> Result<D, DurableError> + Send + Sync>;

/// A conversation singleton (`I = ()`) or keyed family token. Cloning shares
/// callbacks without requiring D or I to implement Clone. Tokens are not stored.
pub struct DocumentDefinition<D, I = ()> {
    kind: String,
    version: u32,
    history: DocumentHistory,
    fork: DocumentFork,
    family: bool,
    initial: Initial<D, I>,
    migrate: Option<Migration<D>>,
}
impl<D, I> Clone for DocumentDefinition<D, I> {
    fn clone(&self) -> Self {
        Self {
            kind: self.kind.clone(),
            version: self.version,
            history: self.history,
            fork: self.fork,
            family: self.family,
            initial: self.initial.clone(),
            migrate: self.migrate.clone(),
        }
    }
}
impl<D> DocumentDefinition<D> {
    pub fn new(
        kind: impl Into<String>,
        version: u32,
        history: DocumentHistory,
        fork: DocumentFork,
        initial: impl Fn() -> Result<D, DurableError> + Send + Sync + 'static,
    ) -> Result<Self, DurableError> {
        Self::build(
            kind.into(),
            version,
            history,
            fork,
            false,
            Arc::new(move |_| initial()),
        )
    }
}
impl<D, I> DocumentDefinition<D, I> {
    pub fn family(
        kind: impl Into<String>,
        version: u32,
        history: DocumentHistory,
        fork: DocumentFork,
        initial: impl Fn(&I) -> Result<D, DurableError> + Send + Sync + 'static,
    ) -> Result<Self, DurableError> {
        Self::build(kind.into(), version, history, fork, true, Arc::new(initial))
    }
    fn build(
        kind: String,
        version: u32,
        history: DocumentHistory,
        fork: DocumentFork,
        family: bool,
        initial: Initial<D, I>,
    ) -> Result<Self, DurableError> {
        // Use the native shape contract without calling user code at definition time.
        GenericDocumentRecord {
            id: DocumentId::new(1)?,
            address: DocumentAddress {
                conversation_id: ConversationId::new(1)?,
                kind: kind.clone(),
                key: None,
            },
            version,
            history,
            fork,
            value: serde_json::json!({}),
            created_seq: CommitSeq::new(1)?,
            updated_seq: CommitSeq::new(1)?,
            retired_seq: None,
        }
        .shape()?;
        Ok(Self {
            kind,
            version,
            history,
            fork,
            family,
            initial,
            migrate: None,
        })
    }
    /// Receive an owned old JSON object and its persisted version. The returned
    /// typed value is validated, detached and never persisted by a read.
    pub fn with_migration(
        mut self,
        migrate: impl Fn(Value, u32) -> Result<D, DurableError> + Send + Sync + 'static,
    ) -> Self {
        self.migrate = Some(Arc::new(migrate));
        self
    }
    pub fn kind(&self) -> &str {
        &self.kind
    }
    pub fn version(&self) -> u32 {
        self.version
    }
    pub fn history(&self) -> DocumentHistory {
        self.history
    }
    pub fn fork(&self) -> DocumentFork {
        self.fork
    }
    pub fn address(
        &self,
        conversation_id: ConversationId,
        key: Option<&str>,
    ) -> Result<DocumentAddress, DurableError> {
        if self.family != key.is_some() {
            return Err(DurableError::Rejected(
                "document singleton/family key mismatch".into(),
            ));
        }
        if key.is_some_and(|key| key.len() > 1024) {
            return Err(DurableError::Rejected(
                "document key exceeds 1024 bytes".into(),
            ));
        }
        Ok(DocumentAddress {
            conversation_id,
            kind: self.kind.clone(),
            key: key.map(str::to_owned),
        })
    }
    pub(crate) fn check(&self, record: &GenericDocumentRecord) -> Result<(), DurableError> {
        if record.address.kind != self.kind
            || record.address.key.is_some() != self.family
            || record.history != self.history
            || record.fork != self.fork
        {
            return Err(DurableError::Rejected(
                "document does not match supplied definition semantics".into(),
            ));
        }
        if record.version > self.version {
            return Err(DurableError::Rejected(format!(
                "document has newer version {} than {}",
                record.version, self.version
            )));
        }
        if record.version < self.version && self.migrate.is_none() {
            return Err(DurableError::Rejected(format!(
                "document requires migration from version {}",
                record.version
            )));
        }
        Ok(())
    }
    pub(crate) fn initial(&self, seed: &I) -> Result<D, DurableError> {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| (self.initial)(seed)))
            .unwrap_or_else(|_| {
                Err(DurableError::Rejected(
                    "document initializer panicked".into(),
                ))
            })
    }
}
impl<D: Serialize, I> DocumentDefinition<D, I> {
    pub(crate) fn encode(&self, value: &D) -> Result<Value, DurableError> {
        let value = serde_json::to_value(value)
            .map_err(|error| DurableError::Rejected(error.to_string()))?;
        if !value.is_object() {
            return Err(DurableError::Rejected(
                "document value must be a JSON object".into(),
            ));
        }
        validate_json_shape("document", &value, MAX_DOCUMENT_BYTES)?;
        super::documents::validate_size(&value)?;
        Ok(value)
    }
}
impl<D: Serialize + DeserializeOwned, I> DocumentDefinition<D, I> {
    pub fn decode(
        &self,
        record: Option<&GenericDocumentRecord>,
    ) -> Result<Option<D>, DurableError> {
        let Some(record) = record else {
            return Ok(None);
        };
        self.check(record)?;
        if record.version == self.version {
            return D::deserialize(&record.value).map(Some).map_err(|error| {
                DurableError::Rejected(format!("invalid typed document: {error}"))
            });
        }
        let value = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.migrate.as_ref().unwrap()(record.value.clone(), record.version)
        }))
        .unwrap_or_else(|_| Err(DurableError::Rejected("document migration panicked".into())))?;
        self.encode(&value)?;
        Ok(Some(value))
    }
}
