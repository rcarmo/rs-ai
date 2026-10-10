//! Synchronous entry, conversation and whole-value document transactions.
//! Task writes, tracked document drafts and async callback operations are absent.
use super::documents::*;
use super::entries::EntryDraft;
use super::storage::{StorageSnapshot, scan::*};
use super::types::*;

// Count the same serialized record bytes as staging previously encoded, but
// retain no throwaway byte buffer. Final storage validation still encodes.
fn entry_size(entry: &EntryRecord) -> Result<usize, DurableError> {
    struct Counter(usize);
    impl std::io::Write for Counter {
        fn write(&mut self, input: &[u8]) -> std::io::Result<usize> {
            let next = self.0.saturating_add(input.len());
            if next > MAX_ENTRY_BYTES {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::FileTooLarge,
                    "entry exceeds byte limit",
                ));
            }
            self.0 = next;
            Ok(input.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut counter = Counter(0);
    serde_json::to_writer(&mut counter, entry).map_err(|error| {
        if error.io_error_kind() == Some(std::io::ErrorKind::FileTooLarge) {
            DurableError::TooLarge {
                field: "entry",
                size: counter.0.saturating_add(1),
                limit: MAX_ENTRY_BYTES,
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
    fn staging_counter_matches_encoded_record_with_escaping_and_unicode() {
        let entry = EntryRecord {
            id: EntryId::new(1).unwrap(),
            conversation_id: ConversationId::new(1).unwrap(),
            kind: "custom".into(),
            value: serde_json::json!({"data":"quotes\"\n\t\\ unicode 𐐀"}),
            by_task_id: None,
            created_seq: CommitSeq::new(1).unwrap(),
        };
        assert_eq!(
            entry_size(&entry).unwrap(),
            serde_json::to_vec(&entry).unwrap().len()
        );
        let mut oversized = entry;
        oversized.value = serde_json::json!("x".repeat(MAX_ENTRY_BYTES));
        assert!(matches!(
            entry_size(&oversized),
            Err(DurableError::TooLarge { .. })
        ));
    }
}

/// Read committed tables before the first append, then stage passive entries.
/// The handle borrows the admitted snapshot and cannot outlive its callback.
/// Callbacks must be short and nonblocking; do not call the owning session.
pub struct EntryTransaction<'a> {
    state: &'a StorageSnapshot,
    batch: CommitBatch,
    writing: bool,
    failure: Option<DurableError>,
    staged_bytes: usize,
    document_bytes: usize,
    task_scope: Option<(TaskId, ConversationId)>,
}
impl<'a> EntryTransaction<'a> {
    pub(crate) fn new(
        state: &'a StorageSnapshot,
        task_id: Option<TaskId>,
    ) -> Result<Self, DurableError> {
        let task_scope = task_id
            .map(|id| {
                let task = state
                    .tasks
                    .get(&id)
                    .ok_or_else(|| DurableError::Rejected("transaction task missing".into()))?;
                if task.state.terminal() {
                    return Err(DurableError::Rejected(
                        "transaction task is terminal".into(),
                    ));
                }
                Ok((id, task.conversation_id))
            })
            .transpose()?;
        Ok(Self {
            state,
            batch: CommitBatch {
                generic_documents: vec![],
                conversations: vec![],
                seq: CommitSeq::new(state.next_seq)?,
                next_id: state.next_id,
                next_seq: state.next_seq,
                entries: vec![],
                tasks: vec![],
                submissions: vec![],
                documents: vec![],
            },
            writing: false,
            failure: None,
            staged_bytes: 0,
            document_bytes: 0,
            task_scope,
        })
    }
    fn read(&self) -> Result<(), DurableError> {
        if self.writing {
            Err(DurableError::Rejected("table read after write".into()))
        } else {
            Ok(())
        }
    }
    pub fn conversation(
        &self,
        id: ConversationId,
    ) -> Result<Option<ConversationRecord>, DurableError> {
        self.read()?;
        Ok(self.state.conversations.get(&id).cloned())
    }
    pub fn conversations(
        &self,
        query: &ConversationQuery,
    ) -> Result<ScanPage<ConversationRecord>, DurableError> {
        self.read()?;
        self.state.query_conversations(query)
    }
    /// Create an immutable native membership record in this atomic batch.
    pub fn create_conversation(
        &mut self,
        ownership: ConversationOwnership,
    ) -> Result<ConversationRecord, DurableError> {
        self.create_membership(ownership, None)
    }
    /// Fork committed visible history through an inclusive cutoff. Generic
    /// conversation documents follow persisted fork policies; built-ins do not.
    pub fn fork_conversation(
        &mut self,
        parent: ConversationId,
        at: EntryId,
        ownership: ConversationOwnership,
    ) -> Result<ConversationRecord, DurableError> {
        self.create_membership(
            ownership,
            Some(ConversationParent {
                conversation_id: parent,
                at,
            }),
        )
    }
    fn create_membership(
        &mut self,
        ownership: ConversationOwnership,
        parent: Option<ConversationParent>,
    ) -> Result<ConversationRecord, DurableError> {
        self.writing = true;
        if let Some(error) = &self.failure {
            return Err(error.clone());
        }
        let result = (|| {
            if self.batch.conversations.len() >= 4096 {
                return Err(DurableError::Rejected(
                    "too many staged conversations".into(),
                ));
            }
            if let Some(parent) = &parent
                && (!self
                    .state
                    .conversations
                    .contains_key(&parent.conversation_id)
                    || self
                        .state
                        .visible_entry(parent.conversation_id, parent.at)?
                        .is_none())
            {
                return Err(DurableError::Rejected(
                    "invalid conversation fork parent/cutoff".into(),
                ));
            }
            let owner = match ownership {
                ConversationOwnership::Ownerless => None,
                ConversationOwnership::Task { task_id } => {
                    let task = self.state.tasks.get(&task_id).ok_or_else(|| {
                        DurableError::Rejected("conversation owner task missing".into())
                    })?;
                    if task.state.terminal() {
                        return Err(DurableError::Rejected(
                            "conversation owner task is terminal".into(),
                        ));
                    }
                    Some(ConversationOwner {
                        conversation_id: task.conversation_id,
                        task_id,
                    })
                }
            };
            let mut next = self.batch.next_id.max(2);
            while self
                .state
                .conversations
                .contains_key(&ConversationId::new(next)?)
            {
                next = next
                    .checked_add(1)
                    .filter(|id| *id <= MAX_ID)
                    .ok_or_else(|| DurableError::Range("conversation id overflow".into()))?;
            }
            let id = ConversationId::new(next)?;
            let next_id = next
                .checked_add(1)
                .filter(|id| *id <= MAX_ID)
                .ok_or_else(|| DurableError::Range("next_id overflow".into()))?;
            let record = ConversationRecord {
                id,
                parent,
                owner,
                created_seq: Some(self.batch.seq),
            };
            self.batch.next_id = next_id;
            self.batch.conversations.push(record.clone());
            if let Some(parent) = &record.parent {
                let cutoff = self
                    .state
                    .visible_entry(parent.conversation_id, parent.at)?
                    .expect("validated fork entry");
                let mut copies = Vec::new();
                for document in self.state.generic_documents.values() {
                    let point = match document.fork {
                        DocumentFork::AsOf
                            if document.address.conversation_id == cutoff.conversation_id
                                && document.alive(DocumentPoint::At(cutoff.created_seq)) =>
                        {
                            DocumentPoint::At(cutoff.created_seq)
                        }
                        DocumentFork::Current
                            if document.address.conversation_id == parent.conversation_id
                                && document.alive(DocumentPoint::Current) =>
                        {
                            DocumentPoint::Current
                        }
                        _ => continue,
                    };
                    if copies.len() >= 4096 {
                        return Err(DurableError::Rejected(
                            "too many fork document copies".into(),
                        ));
                    }
                    copies.push((document.id, point));
                }
                let mut addresses = std::collections::HashSet::new();
                for (source_id, point) in copies {
                    let source = self
                        .state
                        .document_value(&self.state.generic_documents[&source_id], point)?;
                    let copy = DocumentDraft {
                        address: DocumentAddress {
                            conversation_id: id,
                            kind: source.address.kind,
                            key: source.address.key,
                        },
                        version: source.version,
                        history: source.history,
                        fork: source.fork,
                        value: source.value,
                    };
                    if !addresses.insert(copy.address.clone()) {
                        return Err(DurableError::Rejected(
                            "fork selects duplicate document address".into(),
                        ));
                    }
                    self.put_document(copy)?;
                }
            }
            Ok(record)
        })();
        if let Err(error) = &result {
            self.failure = Some(error.clone());
        }
        result
    }

    /// Scan committed document membership before writes. Direct document access
    /// remains available after writes and observes staged values.
    pub fn documents(
        &self,
        query: &DocumentQuery,
    ) -> Result<ScanPage<GenericDocumentRecord>, DurableError> {
        self.read()?;
        self.state.query_documents(query)
    }

    /// Documents are available after table writes; reads prefer staged values.
    pub fn document(
        &self,
        address: &DocumentAddress,
        point: DocumentPoint,
    ) -> Result<Option<GenericDocumentRecord>, DurableError> {
        if matches!(point, DocumentPoint::Current) {
            return Ok(self.current_document(address).cloned());
        }
        self.state.document(address, point)
    }
    fn current_document(&self, address: &DocumentAddress) -> Option<&GenericDocumentRecord> {
        if let Some(record) = self
            .batch
            .generic_documents
            .iter()
            .find(|record| record.address == *address)
        {
            return record.retired_seq.is_none().then_some(record);
        }
        self.state
            .generic_documents
            .values()
            .find(|record| record.address == *address && record.retired_seq.is_none())
    }
    /// Acquire or lazily initialise a typed native document, edit an owned value
    /// synchronously and stage its whole-value replacement. No borrowed draft
    /// escapes; caught callback/validation failures still roll back this Tx.
    /// Seeds are ignored for existing members. Cross-version writes reject.
    pub fn edit_document<D, I, T>(
        &mut self,
        definition: &super::document_definition::DocumentDefinition<D, I>,
        conversation: ConversationId,
        key: Option<&str>,
        seed: &I,
        edit: impl FnOnce(&mut D) -> Result<T, DurableError>,
    ) -> Result<T, DurableError>
    where
        D: serde::Serialize + serde::de::DeserializeOwned,
    {
        if let Some(error) = &self.failure {
            return Err(error.clone());
        }
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let address = definition.address(conversation, key)?;
            let current = self.current_document(&address);
            if let Some(record) = current {
                definition.check(record)?;
                if record.version != definition.version() {
                    return Err(DurableError::Rejected(
                        "document migration persistence unsupported".into(),
                    ));
                }
            }
            let mut value = match current {
                Some(record) => definition.decode(Some(record))?.unwrap(),
                None => {
                    let value = definition.initial(seed)?;
                    definition.encode(&value)?;
                    value
                }
            };
            let output = edit(&mut value)?;
            let encoded = definition.encode(&value)?;
            // Native whole-value edits suppress unchanged writes. Version/delta
            // structural no-op semantics require a separate tracked-draft API.
            if current
                .as_ref()
                .is_none_or(|record| record.value != encoded)
            {
                self.put_document(DocumentDraft {
                    address,
                    version: definition.version(),
                    history: definition.history(),
                    fork: definition.fork(),
                    value: encoded,
                })?;
            }
            Ok(output)
        }))
        .unwrap_or_else(|_| {
            Err(DurableError::Rejected(
                "document edit callback panicked".into(),
            ))
        });
        if let Err(error) = &result {
            self.failure = Some(error.clone());
        }
        result
    }

    pub fn put_document(
        &mut self,
        draft: DocumentDraft,
    ) -> Result<GenericDocumentRecord, DurableError> {
        self.writing = true;
        if let Some(error) = &self.failure {
            return Err(error.clone());
        }
        let result = (|| {
            if self.batch.generic_documents.len() >= 4096 {
                return Err(DurableError::Rejected("too many staged documents".into()));
            }
            // Borrow only identity metadata; replacement never needs a clone
            // of the previous whole JSON value.
            let current = self
                .batch
                .generic_documents
                .iter()
                .find(|record| record.address == draft.address)
                .or_else(|| {
                    self.state.generic_documents.values().find(|record| {
                        record.address == draft.address && record.retired_seq.is_none()
                    })
                })
                .filter(|record| record.retired_seq.is_none());
            let record = if let Some(current) = current {
                if current.version != draft.version
                    || current.history != draft.history
                    || current.fork != draft.fork
                {
                    return Err(DurableError::Rejected(
                        "document definition identity changed".into(),
                    ));
                }
                GenericDocumentRecord {
                    id: current.id,
                    address: current.address.clone(),
                    version: current.version,
                    history: current.history,
                    fork: current.fork,
                    value: draft.value,
                    created_seq: current.created_seq,
                    updated_seq: self.batch.seq,
                    retired_seq: None,
                }
            } else {
                if self
                    .batch
                    .generic_documents
                    .iter()
                    .any(|record| record.address == draft.address)
                {
                    return Err(DurableError::Rejected(
                        "recreation in retirement transaction unsupported".into(),
                    ));
                }
                let id = DocumentId::new(self.batch.next_id)?;
                self.batch.next_id = id
                    .get()
                    .checked_add(1)
                    .filter(|id| *id <= MAX_ID)
                    .ok_or_else(|| DurableError::Range("next_id overflow".into()))?;
                GenericDocumentRecord {
                    id,
                    address: draft.address,
                    version: draft.version,
                    history: draft.history,
                    fork: draft.fork,
                    value: draft.value,
                    created_seq: self.batch.seq,
                    updated_seq: self.batch.seq,
                    retired_seq: None,
                }
            };
            record.shape()?;
            let size = super::documents::validate_size(&record.value)?;
            let previous_size = self
                .batch
                .generic_documents
                .iter()
                .find(|previous| previous.id == record.id)
                .map(|previous| super::documents::validate_size(&previous.value))
                .transpose()?
                .unwrap_or(0);
            let total = self
                .document_bytes
                .saturating_sub(previous_size)
                .saturating_add(size);
            if total.saturating_add(self.staged_bytes) > MAX_COMMIT_BYTES {
                return Err(DurableError::TooLarge {
                    field: "staged documents",
                    size: total,
                    limit: MAX_COMMIT_BYTES,
                });
            }
            self.document_bytes = total;
            if let Some(index) = self
                .batch
                .generic_documents
                .iter()
                .position(|previous| previous.id == record.id)
            {
                self.batch.generic_documents[index] = record.clone();
            } else {
                self.batch.generic_documents.push(record.clone());
            }
            Ok(record)
        })();
        if let Err(error) = &result {
            self.failure = Some(error.clone());
        }
        result
    }
    pub fn retire_document(&mut self, address: &DocumentAddress) -> Result<bool, DurableError> {
        self.writing = true;
        if let Some(error) = &self.failure {
            return Err(error.clone());
        }
        let result = (|| {
            let Some(mut record) = self.document(address, DocumentPoint::Current)? else {
                return Ok(false);
            };
            if record.created_seq == self.batch.seq {
                return Err(DurableError::Rejected(
                    "retire newly staged document unsupported".into(),
                ));
            }
            if !self
                .batch
                .generic_documents
                .iter()
                .any(|previous| previous.id == record.id)
            {
                let total = self
                    .document_bytes
                    .saturating_add(super::documents::validate_size(&record.value)?);
                if total.saturating_add(self.staged_bytes) > MAX_COMMIT_BYTES {
                    return Err(DurableError::Rejected(
                        "staged documents exceed byte limit".into(),
                    ));
                }
                self.document_bytes = total;
            }
            record.updated_seq = self.batch.seq;
            record.retired_seq = Some(self.batch.seq);
            if let Some(index) = self
                .batch
                .generic_documents
                .iter()
                .position(|previous| previous.id == record.id)
            {
                self.batch.generic_documents[index] = record;
            } else {
                self.batch.generic_documents.push(record);
            }
            Ok(true)
        })();
        if let Err(error) = &result {
            self.failure = Some(error.clone());
        }
        result
    }

    pub fn entry(
        &self,
        conversation: ConversationId,
        id: EntryId,
    ) -> Result<Option<EntryRecord>, DurableError> {
        self.read()?;
        Ok(self.state.visible_entry(conversation, id)?.cloned())
    }
    pub fn entries(
        &self,
        conversation: ConversationId,
        query: &EntryQuery,
    ) -> Result<ScanPage<EntryRecord>, DurableError> {
        self.read()?;
        self.state.query_entries(conversation, query)
    }
    pub fn task(&self, id: TaskId) -> Result<Option<TaskRecord>, DurableError> {
        self.read()?;
        Ok(self.state.tasks.get(&id).cloned())
    }
    pub fn submission(&self, id: SubmissionId) -> Result<Option<SubmissionRecord>, DurableError> {
        self.read()?;
        Ok(self.state.submissions.get(&id).cloned())
    }
    pub fn submission_by_request(
        &self,
        conversation: ConversationId,
        request_id: &str,
    ) -> Result<Option<SubmissionRecord>, DurableError> {
        self.read()?;
        Ok(self
            .state
            .request_ids
            .get(&(conversation, request_id.to_owned()))
            .and_then(|id| self.state.submissions.get(id))
            .cloned())
    }
    pub fn tasks(
        &self,
        conversation: Option<ConversationId>,
        query: &TaskQuery,
    ) -> Result<ScanPage<TaskRecord>, DurableError> {
        self.read()?;
        self.state.query_tasks_in(conversation, query)
    }
    pub fn submissions(
        &self,
        conversation: Option<ConversationId>,
        query: &SubmissionQuery,
    ) -> Result<ScanPage<SubmissionRecord>, DurableError> {
        self.read()?;
        self.state.query_submissions_in(conversation, query)
    }
    /// Stage one detached entry. Subsequent appends may refer to its identity.
    /// Any staging error makes the transaction fail even if the callback catches it.
    pub fn append_entry(
        &mut self,
        conversation: ConversationId,
        draft: EntryDraft,
    ) -> Result<EntryRecord, DurableError> {
        self.writing = true;
        if let Some(error) = &self.failure {
            return Err(error.clone());
        }
        let result = (|| {
            if self
                .task_scope
                .is_some_and(|(_, scope)| scope != conversation)
            {
                return Err(DurableError::Rejected(
                    "entry outside transaction task conversation".into(),
                ));
            }
            let id = EntryId::new(self.batch.next_id)?;
            let next_id = id
                .get()
                .checked_add(1)
                .filter(|id| *id <= MAX_ID)
                .ok_or_else(|| DurableError::Range("next_id overflow".into()))?;
            let mut entry = draft.into_record(conversation, id, self.batch.seq)?;
            entry.by_task_id = self.task_scope.map(|(task, _)| task);
            validate_json_shape("entry", &entry.value, MAX_ENTRY_BYTES)?;
            let encoded_bytes = entry_size(&entry)?;
            let bytes = self.staged_bytes.saturating_add(encoded_bytes);
            if bytes.saturating_add(self.document_bytes) > MAX_COMMIT_BYTES {
                return Err(DurableError::TooLarge {
                    field: "staged entries",
                    size: bytes,
                    limit: MAX_COMMIT_BYTES,
                });
            }
            self.batch.next_id = next_id;
            self.staged_bytes = bytes;
            self.batch.entries.push(entry.clone());
            Ok(entry)
        })();
        if let Err(error) = &result {
            self.failure = Some(error.clone());
        }
        result
    }
    pub(crate) fn finish(mut self) -> Result<Option<CommitBatch>, DurableError> {
        if let Some(error) = self.failure {
            return Err(error);
        }
        if self.batch.entries.is_empty()
            && self.batch.conversations.is_empty()
            && self.batch.generic_documents.is_empty()
        {
            return Ok(None);
        }
        self.batch.next_seq = self
            .batch
            .seq
            .get()
            .checked_add(1)
            .filter(|seq| *seq <= MAX_ID)
            .ok_or_else(|| DurableError::Range("next_seq overflow".into()))?;
        // One final validation covers ordered references, aggregate framing and
        // all staged records before storage is admitted. No partial adoption.
        super::storage::validate_batch(self.state, &self.batch)?;
        Ok(Some(self.batch))
    }
}
