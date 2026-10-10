//! Bounded native committed-state watches. Full upstream AgentEvent derivation
//! (partial/retry/inbox/tool-progress/compaction events) is not implemented here.
use super::storage::StorageSnapshot;
use super::types::{CommitBatch, MAX_COMMIT_BYTES};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use tokio::sync::Notify;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WatchEnd {
    Stopped,
    Closed,
    Poisoned,
}

#[derive(Clone, Debug, PartialEq)]
pub enum DurableEvent {
    Snapshot(StorageSnapshot),
    Commit(CommitBatch),
    End(WatchEnd),
}

struct State {
    events: VecDeque<(DurableEvent, usize)>,
    bytes: usize,
    end: Option<WatchEnd>,
    end_delivered: bool,
}

pub(crate) struct WatchQueue {
    state: Mutex<State>,
    notify: Notify,
}
impl WatchQueue {
    pub(crate) fn new(snapshot: StorageSnapshot) -> Arc<Self> {
        Arc::new(Self {
            state: Mutex::new(State {
                events: VecDeque::from([(DurableEvent::Snapshot(snapshot), 0)]),
                bytes: 0,
                end: None,
                end_delivered: false,
            }),
            notify: Notify::new(),
        })
    }
    pub(crate) fn active(&self) -> bool {
        self.state.lock().unwrap().end.is_none()
    }
    pub(crate) fn publish(&self, batch: &CommitBatch, snapshot: &StorageSnapshot, size: usize) {
        let mut state = self.state.lock().unwrap();
        if state.end.is_some() {
            return;
        }
        if state.events.len() >= 64 || state.bytes.saturating_add(size) > MAX_COMMIT_BYTES {
            // Reconcile from current adopted state instead of silently losing
            // deltas. A snapshot can exceed the delta queue's byte limit.
            state.events.clear();
            state.bytes = 0;
            state
                .events
                .push_back((DurableEvent::Snapshot(snapshot.clone()), 0));
        } else {
            state
                .events
                .push_back((DurableEvent::Commit(batch.clone()), size));
            state.bytes = state.bytes.saturating_add(size);
        }
        drop(state);
        self.notify.notify_one();
    }
    pub(crate) fn finish(&self, end: WatchEnd) {
        let mut state = self.state.lock().unwrap();
        if state.end.is_some() {
            return;
        }
        if end == WatchEnd::Stopped {
            state.events.clear();
            state.bytes = 0;
        }
        state.end = Some(end);
        drop(state);
        self.notify.notify_one();
    }
}

/// A single-consumer watch acquired atomically with its initial snapshot on the
/// session queue. Slow readers receive replacement snapshots on overflow.
pub struct DurableWatch {
    pub(crate) queue: Arc<WatchQueue>,
}
impl DurableWatch {
    pub async fn next(&mut self) -> Option<DurableEvent> {
        loop {
            let notified = self.queue.notify.notified();
            {
                let mut state = self.queue.state.lock().unwrap();
                if let Some((event, size)) = state.events.pop_front() {
                    state.bytes = state.bytes.saturating_sub(size);
                    return Some(event);
                }
                if let Some(end) = state.end {
                    if state.end_delivered {
                        return None;
                    }
                    state.end_delivered = true;
                    return Some(DurableEvent::End(end));
                }
            }
            notified.await;
        }
    }
    pub fn stop(&self) {
        self.queue.finish(WatchEnd::Stopped);
    }
}
impl Drop for DurableWatch {
    fn drop(&mut self) {
        self.stop();
    }
}

/// RAII owner closes every receiver if the session worker unwinds unexpectedly.
pub(crate) struct WatchRegistry {
    pub(crate) values: Vec<std::sync::Weak<WatchQueue>>,
    pub(crate) documents: Vec<std::sync::Weak<super::document_watch::DocumentQueue>>,
}
impl WatchRegistry {
    pub(crate) fn new() -> Self {
        Self {
            values: Vec::new(),
            documents: Vec::new(),
        }
    }
    pub(crate) fn finish(&mut self, end: WatchEnd) {
        finish_all(&mut self.values, end);
        let end = match end {
            WatchEnd::Stopped => super::document_watch::DocumentWatchEnd::Stopped,
            WatchEnd::Closed => super::document_watch::DocumentWatchEnd::Closed,
            WatchEnd::Poisoned => super::document_watch::DocumentWatchEnd::Poisoned,
        };
        for watch in self.documents.drain(..).filter_map(|watch| watch.upgrade()) {
            watch.finish(end);
        }
    }
}
impl Drop for WatchRegistry {
    fn drop(&mut self) {
        self.finish(WatchEnd::Poisoned);
    }
}

pub(crate) fn finish_all(watches: &mut Vec<std::sync::Weak<WatchQueue>>, end: WatchEnd) {
    for watch in watches.drain(..).filter_map(|watch| watch.upgrade()) {
        watch.finish(end);
    }
}
