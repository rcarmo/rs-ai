//! Native durable R1a/R1b foundation.
//!
//! R1a provides atomic storage/session semantics. R1b adds persistent no-tool
//! generation through the existing rs-ai model registry. Executable tool
//! ownership and safe replay remain mandatory R1c work.

pub mod harness;
pub mod model;
pub mod session;
pub mod storage;
pub mod submission;
pub mod types;

pub use harness::DurableHarness;
pub use model::{
    DurableContent, DurableMessage, DurableModelRunner, DurableUsage, ModelIntent, ModelRun,
    ModelTerminal, PinnedModel, PinnedOptions, RegistryModelRunner,
};
pub use session::DurableSession;
pub use storage::journal::JournalStorage;
pub use storage::memory::MemoryStorage;
pub use storage::{DurableStorage, StorageSnapshot};
pub use submission::{SubmissionHandle, SubmissionView, SubmitRequest};
pub use types::*;
