//! Synchronous entry transactions executed on the session mutation line.
//! Task/document/conversation writes and async callback operations are absent.
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
    pub fn entry(
        &self,
        conversation: ConversationId,
        id: EntryId,
    ) -> Result<Option<EntryRecord>, DurableError> {
        self.read()?;
        Ok(self
            .state
            .entries
            .get(&id)
            .filter(|entry| entry.conversation_id == conversation)
            .cloned())
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
            if bytes > MAX_COMMIT_BYTES {
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
        if self.batch.entries.is_empty() {
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
