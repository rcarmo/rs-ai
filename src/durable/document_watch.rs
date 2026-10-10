//! Pull-based exact-commit watches of generic document incarnations.
//! Frames are whole-value replacements, not upstream delta/listener callbacks.
use super::documents::GenericDocumentRecord;
use super::types::{CommitSeq, DocumentId, MAX_COMMIT_BYTES};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use tokio::sync::Notify;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DocumentWatchEnd {
    Stopped,
    Retired,
    Closed,
    Poisoned,
}
#[derive(Clone, Debug, PartialEq)]
pub enum DocumentEvent {
    /// None is the retirement frame. Reconciled means overflow replaced queued
    /// undelivered frames with this latest committed value.
    Replacement {
        record: Option<GenericDocumentRecord>,
        seq: CommitSeq,
        reconciled: bool,
    },
    End(DocumentWatchEnd),
}
struct Frame {
    record: Option<Arc<GenericDocumentRecord>>,
    seq: CommitSeq,
    reconciled: bool,
    bytes: usize,
}
struct State {
    value: Option<Arc<GenericDocumentRecord>>,
    pending: VecDeque<Frame>,
    bytes: usize,
    end: Option<DocumentWatchEnd>,
    retiring: bool,
    end_delivered: bool,
}
pub(crate) struct DocumentQueue {
    pub(crate) id: DocumentId,
    state: Mutex<State>,
    notify: Notify,
}
impl DocumentQueue {
    pub(crate) fn new(record: GenericDocumentRecord) -> Arc<Self> {
        Arc::new(Self {
            id: record.id,
            state: Mutex::new(State {
                value: Some(Arc::new(record)),
                pending: VecDeque::new(),
                bytes: 0,
                end: None,
                retiring: false,
                end_delivered: false,
            }),
            notify: Notify::new(),
        })
    }
    pub(crate) fn active(&self) -> bool {
        self.state.lock().unwrap().end.is_none()
    }
    pub(crate) fn publish(&self, record: Arc<GenericDocumentRecord>, size: usize) {
        let mut state = self.state.lock().unwrap();
        if state.end.is_some() || state.retiring {
            return;
        }
        let retired = record.retired_seq.is_some();
        let reconciled =
            state.pending.len() >= 100 || state.bytes.saturating_add(size) > MAX_COMMIT_BYTES;
        if reconciled {
            state.pending.clear();
            state.bytes = 0;
        }
        let seq = record.updated_seq;
        let bytes = if retired { 0 } else { size };
        state.pending.push_back(Frame {
            record: (!retired).then_some(record),
            seq,
            reconciled,
            bytes,
        });
        state.bytes = state.bytes.saturating_add(bytes);
        state.retiring = retired;
        drop(state);
        self.notify.notify_one();
    }
    pub(crate) fn finish(&self, end: DocumentWatchEnd) {
        let mut state = self.state.lock().unwrap();
        if state.end.is_some() {
            return;
        }
        state.pending.clear();
        state.bytes = 0;
        state.end = Some(end);
        drop(state);
        self.notify.notify_one();
    }
}
/// Atomically attached to one live incarnation. `value` reflects the acquired
/// value or last consumed frame, not later queued commits. Drop/stop detach.
pub struct DocumentWatch {
    pub(crate) queue: Arc<DocumentQueue>,
}
impl DocumentWatch {
    pub fn id(&self) -> DocumentId {
        self.queue.id
    }
    pub fn value(&self) -> Option<GenericDocumentRecord> {
        self.queue
            .state
            .lock()
            .unwrap()
            .value
            .as_ref()
            .map(|record| (**record).clone())
    }
    pub async fn next(&mut self) -> Option<DocumentEvent> {
        loop {
            let notified = self.queue.notify.notified();
            {
                let mut state = self.queue.state.lock().unwrap();
                if let Some(frame) = state.pending.pop_front() {
                    state.bytes = state.bytes.saturating_sub(frame.bytes);
                    state.value = frame.record;
                    if state.value.is_none() {
                        state.end = Some(DocumentWatchEnd::Retired);
                    }
                    return Some(DocumentEvent::Replacement {
                        record: state.value.as_ref().map(|record| (**record).clone()),
                        seq: frame.seq,
                        reconciled: frame.reconciled,
                    });
                }
                if let Some(end) = state.end {
                    if state.end_delivered {
                        return None;
                    }
                    state.end_delivered = true;
                    return Some(DocumentEvent::End(end));
                }
            }
            notified.await;
        }
    }
    pub fn stop(&self) {
        self.queue.finish(DocumentWatchEnd::Stopped);
    }
}
impl Drop for DocumentWatch {
    fn drop(&mut self) {
        self.stop();
    }
}

// Conservative encoded-byte budget: JSON escaping needs at most six bytes
// per UTF-8 byte; 32 bounds a JSON number; 1 KiB covers fixed record metadata.
// Compute once per adopted revision without reserializing or allocating. This
// can reconcile early and excludes Rust/allocator overhead and delivered value.
pub(crate) fn record_size(record: &GenericDocumentRecord) -> usize {
    fn string_size(value: &str) -> usize {
        value.len().saturating_mul(6).saturating_add(2)
    }
    fn value_size(value: &serde_json::Value) -> usize {
        use serde_json::Value;
        match value {
            Value::Null | Value::Bool(_) => 5,
            Value::Number(_) => 32,
            Value::String(value) => string_size(value),
            Value::Array(values) => values.iter().fold(2usize, |size, value| {
                size.saturating_add(value_size(value)).saturating_add(1)
            }),
            Value::Object(values) => values.iter().fold(2usize, |size, (key, value)| {
                size.saturating_add(string_size(key))
                    .saturating_add(value_size(value))
                    .saturating_add(2)
            }),
        }
    }
    1024usize
        .saturating_add(string_size(&record.address.kind))
        .saturating_add(record.address.key.as_deref().map_or(4, string_size))
        .saturating_add(value_size(&record.value))
}
