use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::num::NonZeroU64;

pub const MAX_SUBMISSION_BYTES: usize = 1024 * 1024;
pub const MAX_ENTRY_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_DOCUMENT_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_TASK_FIELD_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_COMMIT_BYTES: usize = 32 * 1024 * 1024;
pub const MAX_ID: u64 = i64::MAX as u64;

macro_rules! durable_id {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
        #[serde(transparent)]
        pub struct $name(NonZeroU64);

        impl $name {
            pub fn new(value: u64) -> Result<Self, DurableError> {
                if value == 0 || value > MAX_ID {
                    return Err(DurableError::Range(format!(
                        "{} is outside 1..=i64::MAX",
                        stringify!($name)
                    )));
                }
                Ok(Self(NonZeroU64::new(value).expect("non-zero checked")))
            }
            pub fn get(self) -> u64 {
                self.0.get()
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                let value = u64::deserialize(deserializer)?;
                Self::new(value).map_err(serde::de::Error::custom)
            }
        }
    };
}

durable_id!(ConversationId);
durable_id!(EntryId);
durable_id!(TaskId);
durable_id!(SubmissionId);
durable_id!(CommitSeq);

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskState {
    Pending,
    Running,
    Completing,
    Succeeded,
    Failed,
    Aborted,
}

impl TaskState {
    pub fn terminal(&self) -> bool {
        matches!(self, Self::Succeeded | Self::Failed | Self::Aborted)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
pub enum ConversationOwnership {
    Ownerless,
    Task { task_id: TaskId },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ConversationOwner {
    pub conversation_id: ConversationId,
    pub task_id: TaskId,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ConversationParent {
    pub conversation_id: ConversationId,
    pub at: EntryId,
}

/// Native conversation membership. None creation sequence marks the reserved
/// root or an ownerless scope inferred from a pre-table journal.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ConversationRecord {
    pub id: ConversationId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<ConversationParent>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<ConversationOwner>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_seq: Option<CommitSeq>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EntryRecord {
    pub id: EntryId,
    pub conversation_id: ConversationId,
    pub kind: String,
    pub value: Value,
    pub by_task_id: Option<TaskId>,
    pub created_seq: CommitSeq,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TaskRecord {
    pub id: TaskId,
    pub conversation_id: ConversationId,
    pub kind: String,
    pub version: u32,
    pub owner_task_id: Option<TaskId>,
    pub state: TaskState,
    pub input: Value,
    pub checkpoint: Value,
    pub outcome: Option<Value>,
    pub abort_requested: bool,
    /// First running admission, retained across waits/recovery (wall-clock milliseconds).
    #[serde(
        default,
        rename = "startedAt",
        alias = "started_at",
        skip_serializing_if = "Option::is_none"
    )]
    pub started_at: Option<i64>,
    /// Terminal settlement time; older stored records may omit it.
    #[serde(
        default,
        rename = "endedAt",
        alias = "ended_at",
        skip_serializing_if = "Option::is_none"
    )]
    pub ended_at: Option<i64>,
    pub updated_seq: CommitSeq,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SubmissionRecord {
    pub id: SubmissionId,
    pub conversation_id: ConversationId,
    pub request_id: Option<String>,
    pub status: String,
    pub entry_id: EntryId,
    pub answer_id: Option<EntryId>,
    pub reason: Option<Value>,
    pub updated_seq: CommitSeq,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DocumentRecord {
    pub conversation_id: ConversationId,
    pub kind: String,
    pub version: u32,
    pub value: Value,
    pub updated_seq: CommitSeq,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CommitBatch {
    /// Empty omitted to preserve pre-conversation journal encoding.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conversations: Vec<ConversationRecord>,
    pub seq: CommitSeq,
    pub next_id: u64,
    pub next_seq: u64,
    #[serde(default)]
    pub entries: Vec<EntryRecord>,
    #[serde(default)]
    pub tasks: Vec<TaskRecord>,
    #[serde(default)]
    pub submissions: Vec<SubmissionRecord>,
    #[serde(default)]
    pub documents: Vec<DocumentRecord>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DurableError {
    Closed,
    Poisoned,
    Rejected(String),
    Uncertain(String),
    Corrupt(String),
    Range(String),
    TooLarge {
        field: &'static str,
        size: usize,
        limit: usize,
    },
    Io(String),
}

impl std::fmt::Display for DurableError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Closed => f.write_str("durable session is closed"),
            Self::Poisoned => f.write_str("durable session is poisoned; reopen required"),
            Self::Rejected(v) => write!(f, "durable commit rejected: {v}"),
            Self::Uncertain(v) => write!(f, "durable commit outcome uncertain: {v}"),
            Self::Corrupt(v) => write!(f, "durable storage corrupt: {v}"),
            Self::Range(v) => write!(f, "durable range error: {v}"),
            Self::TooLarge { field, size, limit } => {
                write!(f, "durable {field} is {size} bytes; limit is {limit}")
            }
            Self::Io(v) => write!(f, "durable I/O error: {v}"),
        }
    }
}
impl std::error::Error for DurableError {}

pub(crate) fn validate_json_shape(
    field: &'static str,
    value: &Value,
    byte_limit: usize,
) -> Result<(), DurableError> {
    const MAX_DEPTH: usize = 128;
    const MAX_MEMBERS: usize = 1_000_000;
    let mut stack = vec![(value, 1usize)];
    let mut members = 0usize;
    while let Some((current, depth)) = stack.pop() {
        if depth > MAX_DEPTH {
            return Err(DurableError::Rejected(format!(
                "{field} exceeds JSON depth {MAX_DEPTH}"
            )));
        }
        match current {
            Value::String(text) if text.len() > byte_limit => {
                return Err(DurableError::TooLarge {
                    field,
                    size: text.len(),
                    limit: byte_limit,
                });
            }
            Value::Array(values) => {
                members = members.saturating_add(values.len());
                stack.extend(values.iter().map(|value| (value, depth + 1)));
            }
            Value::Object(values) => {
                members = members.saturating_add(values.len());
                for (key, value) in values {
                    if key.len() > byte_limit {
                        return Err(DurableError::TooLarge {
                            field,
                            size: key.len(),
                            limit: byte_limit,
                        });
                    }
                    stack.push((value, depth + 1));
                }
            }
            _ => {}
        }
        if members > MAX_MEMBERS {
            return Err(DurableError::Rejected(format!(
                "{field} exceeds JSON member limit {MAX_MEMBERS}"
            )));
        }
    }
    Ok(())
}
