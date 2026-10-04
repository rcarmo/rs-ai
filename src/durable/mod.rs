//! Native durable R1a/R1b/R1c vertical.
//!
//! R1a provides atomic storage/session semantics. R1b adds persistent model
//! generation. R1c adds owned versioned tools, conservative replay, bottom-up
//! abort and non-aborting close drain.

pub mod harness;
pub mod model;
pub mod session;
pub mod storage;
pub mod submission;
pub mod tool;
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
pub use tool::{
    DurableTool, DurableToolCall, DurableToolRegistry, ReplayPolicy, ToolBinding, ToolExecution,
    ToolFailure, ToolIntent, ToolOutput,
};
pub use types::*;
