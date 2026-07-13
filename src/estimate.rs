//! Context token estimation — port of upstream `utils/estimate.ts` (v0.80.3).
//!
//! Heuristic estimator used to fit `max_tokens` inside the model context window
//! (see `simple_options::clamp_max_tokens_to_context`). Mirrors upstream's
//! char-per-token ratio, image char weighting, last-assistant-usage anchoring,
//! and prefix (system + tools) accounting exactly.

use crate::types::{ContentBlock, Context, Message, Role, StopReason, Tool, Usage};

const CHARS_PER_TOKEN: usize = 4;
const ESTIMATED_IMAGE_CHARS: usize = 4800;

/// Result of estimating a context's token footprint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextEstimate {
    pub tokens: u32,
    pub usage_tokens: u32,
    pub trailing_tokens: u32,
    /// Index of the anchoring last-assistant message, or `None` when none had usage.
    pub last_usage_index: Option<usize>,
}

/// `usage.totalTokens || input + output + cacheRead + cacheWrite`.
pub fn calculate_context_tokens(usage: &Usage) -> u32 {
    if usage.total_tokens > 0 {
        usage.total_tokens
    } else {
        usage.input + usage.output + usage.cache_read + usage.cache_write
    }
}

fn ceil_div(chars: usize, by: usize) -> u32 {
    chars.div_ceil(by) as u32
}

pub fn estimate_text_tokens(text: &str) -> u32 {
    ceil_div(text.len(), CHARS_PER_TOKEN)
}

fn content_chars(content: &[ContentBlock]) -> usize {
    content
        .iter()
        .map(|b| match b {
            ContentBlock::Text { text, .. } => text.len(),
            _ => ESTIMATED_IMAGE_CHARS,
        })
        .sum()
}

/// Text + image content tokens (used for user / toolResult messages, which carry
/// only text/image blocks).
pub fn estimate_text_and_image_content_tokens(content: &[ContentBlock]) -> u32 {
    ceil_div(content_chars(content), CHARS_PER_TOKEN)
}

pub fn estimate_message_tokens(message: &Message) -> u32 {
    estimate_message_tokens_with_tools(message, &[])
}

fn estimate_added_tool_tokens(message: &Message, tools: &[Tool]) -> u32 {
    if message.added_tool_names.is_empty() || tools.is_empty() {
        return 0;
    }
    let added: Vec<&Tool> = message
        .added_tool_names
        .iter()
        .filter_map(|name| tools.iter().find(|t| &t.name == name))
        .collect();
    if added.is_empty() {
        return 0;
    }
    let json = serde_json::to_string(&added).unwrap_or_else(|_| "undefined".into());
    estimate_text_tokens(&json)
}

fn estimate_message_tokens_with_tools(message: &Message, tools: &[Tool]) -> u32 {
    if matches!(message.role, Role::User | Role::ToolResult) {
        return estimate_text_and_image_content_tokens(&message.content)
            + estimate_added_tool_tokens(message, tools);
    }
    let mut chars = 0usize;
    for block in &message.content {
        match block {
            ContentBlock::Text { text, .. } => chars += text.len(),
            ContentBlock::Thinking { thinking, .. } => chars += thinking.len(),
            ContentBlock::ToolCall {
                name, arguments, ..
            } => {
                chars += name.len();
                chars += serde_json::to_string(arguments)
                    .unwrap_or_else(|_| "undefined".into())
                    .len();
            }
            ContentBlock::Image { .. } => chars += ESTIMATED_IMAGE_CHARS,
        }
    }
    ceil_div(chars, CHARS_PER_TOKEN)
}

/// Most recent *applicable* assistant message with positive usage that did not
/// abort/error. v0.80.6: an assistant usage block only describes the current
/// prefix if no newer prefix message (e.g. a compaction summary) was inserted
/// after it — tracked via `latest_prefix_timestamp`.
fn last_assistant_usage(messages: &[Message]) -> Option<(usize, &Usage)> {
    let mut latest_prefix_timestamp = i64::MIN;
    let mut usage_info: Option<(usize, &Usage)> = None;
    for (i, message) in messages.iter().enumerate() {
        if message.role == Role::Assistant {
            // A newer prefix message was inserted after this response (for example,
            // a compaction summary), so its usage cannot describe the current prefix.
            let usage_applies_to_prefix = message.timestamp >= latest_prefix_timestamp;
            if usage_applies_to_prefix
                && !matches!(
                    message.stop_reason,
                    Some(StopReason::Aborted) | Some(StopReason::Error)
                )
                && let Some(usage) = &message.usage
                && calculate_context_tokens(usage) > 0
            {
                usage_info = Some((i, usage));
            }
        }
        latest_prefix_timestamp = latest_prefix_timestamp.max(message.timestamp);
    }
    usage_info
}

fn estimate_messages(messages: &[Message], tools: &[Tool]) -> ContextEstimate {
    if let Some((index, usage)) = last_assistant_usage(messages) {
        let usage_tokens = calculate_context_tokens(usage);
        let trailing_tokens: u32 = messages[index + 1..]
            .iter()
            .map(|m| estimate_message_tokens_with_tools(m, tools))
            .sum();
        return ContextEstimate {
            tokens: usage_tokens + trailing_tokens,
            usage_tokens,
            trailing_tokens,
            last_usage_index: Some(index),
        };
    }
    let tokens: u32 = messages
        .iter()
        .map(|m| estimate_message_tokens_with_tools(m, tools))
        .sum();
    ContextEstimate {
        tokens,
        usage_tokens: 0,
        trailing_tokens: tokens,
        last_usage_index: None,
    }
}

/// Estimate a full context's token footprint. When no last-assistant usage
/// anchors the estimate, the system prompt and tool schemas are added as prefix.
pub fn estimate_context_tokens(context: &Context) -> ContextEstimate {
    let estimate = estimate_messages(&context.messages, &context.tools);
    if estimate.last_usage_index.is_some() {
        return estimate;
    }
    let mut prefix_tokens = context
        .system_prompt
        .as_deref()
        .map_or(0, estimate_text_tokens);
    if !context.tools.is_empty() {
        let tools_json =
            serde_json::to_string(&context.tools).unwrap_or_else(|_| "undefined".into());
        prefix_tokens += estimate_text_tokens(&tools_json);
    }
    ContextEstimate {
        tokens: estimate.tokens + prefix_tokens,
        usage_tokens: estimate.usage_tokens,
        trailing_tokens: estimate.trailing_tokens + prefix_tokens,
        last_usage_index: estimate.last_usage_index,
    }
}
