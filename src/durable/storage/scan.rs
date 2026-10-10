//! Detached, ordered scans of a session snapshot. Cursors carry their order;
//! legacy cursors without an order use the record type's default.
use super::StorageSnapshot;
use crate::durable::types::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScanOrder {
    Ascending,
    Descending,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScanCursor {
    pub after: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<ScanOrder>,
}

#[derive(Clone, Debug)]
pub struct ScanOptions {
    pub order: Option<ScanOrder>,
    pub cursor: Option<ScanCursor>,
    pub limit: usize,
}
impl Default for ScanOptions {
    fn default() -> Self {
        Self {
            order: None,
            cursor: None,
            limit: 100,
        }
    }
}

/// Inclusive entry bounds within one native conversation.
#[derive(Clone, Debug, Default)]
pub struct EntryQuery {
    pub min_entry_id: Option<EntryId>,
    pub max_entry_id: Option<EntryId>,
    pub kind: Option<String>,
    pub scan: ScanOptions,
}

#[derive(Clone, Debug, Default)]
pub struct TaskQuery {
    pub kind: Option<String>,
    pub state: Option<TaskState>,
    pub abort_requested: Option<bool>,
    pub scan: ScanOptions,
}

#[derive(Clone, Debug, Default)]
pub struct SubmissionQuery {
    pub status: Option<String>,
    pub scan: ScanOptions,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ScanPage<T> {
    pub items: Vec<T>,
    pub cursor: Option<ScanCursor>,
}

fn start(
    options: &ScanOptions,
    fallback: ScanOrder,
) -> Result<(ScanOrder, Option<u64>), DurableError> {
    if options.limit == 0 {
        return Err(DurableError::Range("scan limit must be positive".into()));
    }
    let Some(cursor) = options.cursor else {
        return Ok((options.order.unwrap_or(fallback), None));
    };
    // Upstream validates a safe integer. Rust durable IDs are positive i64;
    // zero is also a valid exclusive starting position.
    if cursor.after > MAX_ID {
        return Err(DurableError::Range("invalid storage cursor".into()));
    }
    let order = cursor.order.unwrap_or(fallback);
    if options.order.is_some_and(|requested| requested != order) {
        return Err(DurableError::Rejected(
            "cursor scan order conflicts with query".into(),
        ));
    }
    Ok((order, Some(cursor.after)))
}

fn page<'a, T: Clone + 'a>(
    records: impl Iterator<Item = (u64, &'a T)>,
    options: &ScanOptions,
    order: ScanOrder,
    after: Option<u64>,
) -> ScanPage<T> {
    let mut items = Vec::new();
    let mut last = None;
    let mut more = false;
    for (id, record) in records {
        if after.is_some_and(|after| match order {
            ScanOrder::Ascending => id <= after,
            ScanOrder::Descending => id >= after,
        }) {
            continue;
        }
        if items.len() == options.limit {
            more = true;
            break;
        }
        last = Some(id);
        items.push(record.clone());
    }
    ScanPage {
        items,
        cursor: if more {
            last.map(|after| ScanCursor {
                after,
                order: Some(order),
            })
        } else {
            None
        },
    }
}

impl StorageSnapshot {
    /// Entries default to descending ID order. Results own their JSON values.
    pub fn scan_entries(
        &self,
        conversation: ConversationId,
        options: &ScanOptions,
    ) -> Result<ScanPage<EntryRecord>, DurableError> {
        self.query_entries(
            conversation,
            &EntryQuery {
                scan: options.clone(),
                ..Default::default()
            },
        )
    }

    pub fn query_entries(
        &self,
        conversation: ConversationId,
        query: &EntryQuery,
    ) -> Result<ScanPage<EntryRecord>, DurableError> {
        use std::ops::Bound::{Included, Unbounded};
        let options = &query.scan;
        let (order, after) = start(options, ScanOrder::Descending)?;
        if query
            .min_entry_id
            .zip(query.max_entry_id)
            .is_some_and(|(min, max)| min > max)
        {
            return Err(DurableError::Range("entry minimum exceeds maximum".into()));
        }
        // Range-seek before filtering: later pages do not walk earlier entries.
        let mut min = query.min_entry_id.map(|id| id.get());
        let mut max = query.max_entry_id.map(|id| id.get());
        if let Some(after) = after {
            match order {
                ScanOrder::Ascending => min = Some(min.unwrap_or(0).max(after.saturating_add(1))),
                ScanOrder::Descending => {
                    if after == 0 {
                        return Ok(ScanPage {
                            items: vec![],
                            cursor: None,
                        });
                    }
                    max = Some(max.unwrap_or(MAX_ID).min(after - 1));
                }
            }
        }
        if min.is_some_and(|id| id > MAX_ID)
            || min.zip(max).is_some_and(|(min, max)| min > max)
            || max == Some(0)
        {
            return Ok(ScanPage {
                items: vec![],
                cursor: None,
            });
        }
        let lower = min
            .filter(|id| *id > 0)
            .map(|id| Included(EntryId::new(id).expect("validated bound")))
            .unwrap_or(Unbounded);
        let upper = max
            .map(|id| Included(EntryId::new(id).expect("validated bound")))
            .unwrap_or(Unbounded);
        let records = self
            .entries
            .range((lower, upper))
            .filter(move |(_, record)| {
                record.conversation_id == conversation
                    && query.kind.as_ref().is_none_or(|kind| record.kind == *kind)
            })
            .map(|(id, record)| (id.get(), record));
        Ok(match order {
            ScanOrder::Ascending => page(records, options, order, after),
            ScanOrder::Descending => page(records.rev(), options, order, after),
        })
    }
    /// Tasks default to ascending ID order.
    pub fn scan_tasks(
        &self,
        conversation: ConversationId,
        options: &ScanOptions,
    ) -> Result<ScanPage<TaskRecord>, DurableError> {
        self.query_tasks(
            conversation,
            &TaskQuery {
                scan: options.clone(),
                ..Default::default()
            },
        )
    }

    pub fn query_tasks(
        &self,
        conversation: ConversationId,
        query: &TaskQuery,
    ) -> Result<ScanPage<TaskRecord>, DurableError> {
        let options = &query.scan;
        let (order, after) = start(options, ScanOrder::Ascending)?;
        let records = self
            .tasks
            .iter()
            .filter(move |(_, record)| {
                record.conversation_id == conversation
                    && query.kind.as_ref().is_none_or(|kind| record.kind == *kind)
                    && query
                        .state
                        .as_ref()
                        .is_none_or(|state| record.state == *state)
                    && query
                        .abort_requested
                        .is_none_or(|abort| record.abort_requested == abort)
            })
            .map(|(id, record)| (id.get(), record));
        Ok(match order {
            ScanOrder::Ascending => page(records, options, order, after),
            ScanOrder::Descending => page(records.rev(), options, order, after),
        })
    }
    /// Submissions default to ascending ID order.
    pub fn scan_submissions(
        &self,
        conversation: ConversationId,
        options: &ScanOptions,
    ) -> Result<ScanPage<SubmissionRecord>, DurableError> {
        self.query_submissions(
            conversation,
            &SubmissionQuery {
                scan: options.clone(),
                ..Default::default()
            },
        )
    }

    pub fn query_submissions(
        &self,
        conversation: ConversationId,
        query: &SubmissionQuery,
    ) -> Result<ScanPage<SubmissionRecord>, DurableError> {
        let options = &query.scan;
        let (order, after) = start(options, ScanOrder::Ascending)?;
        let records = self
            .submissions
            .iter()
            .filter(move |(_, record)| {
                record.conversation_id == conversation
                    && query
                        .status
                        .as_ref()
                        .is_none_or(|status| record.status == *status)
            })
            .map(|(id, record)| (id.get(), record));
        Ok(match order {
            ScanOrder::Ascending => page(records, options, order, after),
            ScanOrder::Descending => page(records.rev(), options, order, after),
        })
    }
}
