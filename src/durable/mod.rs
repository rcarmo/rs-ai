//! Native durable R1a storage/session foundation.
//!
//! R1a provides atomic mixed-record commits, memory and persistent framed-journal
//! storage, bounded owned settlement, poisoning and close fencing. Model/tool
//! execution and a useful root-conversation harness are mandatory R1b/R1c work.

pub mod session;
pub mod storage;
pub mod types;

pub use session::DurableSession;
pub use storage::journal::JournalStorage;
pub use storage::memory::MemoryStorage;
pub use storage::{DurableStorage, StorageSnapshot};
pub use types::*;
