//! Native durable R1a/R1b/R1c vertical.
//!
//! R1a provides atomic storage/session semantics. R1b adds persistent model
//! generation. R1c adds owned versioned tools, conservative replay, bottom-up
//! abort and non-aborting close drain.

pub mod context;
pub mod harness;
pub mod model;
pub mod models;
mod provider;
pub mod session;
pub mod storage;
pub mod submission;
pub mod tool;
pub mod types;

pub use context::{ContextEdit, ContextHead, ContextUpdate, SelfHead};
pub use harness::{ContextOptions, DurableHarness, HarnessServices};
pub use model::{
    DurableContent, DurableMessage, DurableModelRunner, DurableUsage, ModelIntent, ModelRun,
    ModelTerminal, PinnedModel, PinnedOptions, RegistryModelRunner,
};
pub use models::{DurableModels, RegistryModels};
pub use session::{DurableSession, LifecycleClock, SessionSettings};
pub use storage::journal::JournalStorage;
pub use storage::memory::MemoryStorage;
pub use storage::scan::{
    EntryQuery, ScanCursor, ScanOptions, ScanOrder, ScanPage, SubmissionQuery, TaskQuery,
};
pub use storage::{DurableStorage, StorageSnapshot};
pub use submission::{SubmissionHandle, SubmissionView, SubmitRequest};
pub use tool::{
    DurableTool, DurableToolCall, DurableToolRegistry, ReplayPolicy, ToolBinding, ToolExecution,
    ToolFailure, ToolIntent, ToolOutput,
};
pub use types::*;
