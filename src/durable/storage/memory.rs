use super::*;
use std::sync::Mutex;

struct Inner {
    snapshot: StorageSnapshot,
    writer: Option<u64>,
    closed: bool,
}

pub struct MemoryStorage {
    state: Mutex<Inner>,
}

impl MemoryStorage {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(Inner {
                snapshot: StorageSnapshot::empty(),
                writer: None,
                closed: false,
            }),
        }
    }

    fn check(inner: &Inner, claim: &WriterClaim) -> Result<(), DurableError> {
        if inner.closed || inner.writer != Some(claim.id()) {
            Err(DurableError::Closed)
        } else {
            Ok(())
        }
    }
}
impl Default for MemoryStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl DurableStorage for MemoryStorage {
    fn claim_writer(&self) -> Result<WriterClaim, DurableError> {
        let mut inner = self.state.lock().unwrap();
        if inner.closed || inner.writer.is_some() {
            return Err(DurableError::Rejected(
                "storage writer already claimed".into(),
            ));
        }
        let claim = WriterClaim::fresh();
        inner.writer = Some(claim.id());
        Ok(claim)
    }

    fn load<'a>(&'a self, claim: &'a WriterClaim) -> StorageFuture<'a, StorageSnapshot> {
        Box::pin(async move {
            let inner = self.state.lock().unwrap();
            Self::check(&inner, claim)?;
            Ok(inner.snapshot.clone())
        })
    }

    fn commit<'a>(&'a self, claim: &'a WriterClaim, batch: CommitBatch) -> StorageFuture<'a, ()> {
        Box::pin(async move {
            let mut inner = self.state.lock().unwrap();
            Self::check(&inner, claim)?;
            let mut staged = inner.snapshot.clone();
            staged.apply(&batch)?;
            inner.snapshot = staged;
            Ok(())
        })
    }

    fn close<'a>(&'a self, claim: &'a WriterClaim) -> StorageFuture<'a, ()> {
        Box::pin(async move {
            let mut inner = self.state.lock().unwrap();
            Self::check(&inner, claim)?;
            inner.closed = true;
            inner.writer = None;
            Ok(())
        })
    }
}
