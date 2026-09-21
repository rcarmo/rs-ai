//! Transcript normalization and system-message replay (upstream v0.87.0).

use crate::types::{ContentBlock, Context, Message, Role, Tool, ToolReference};
use indexmap::IndexMap;

fn text_content(blocks: &[ContentBlock]) -> String {
    blocks
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Construct a transcript system message.
pub fn system_message(
    content: impl Into<String>,
    sections: Option<IndexMap<String, Option<String>>>,
    tools_added: Vec<Tool>,
    tools_removed: Vec<ToolReference>,
) -> Message {
    let content = content.into();
    Message {
        role: Role::System,
        content: if content.is_empty() {
            Vec::new()
        } else {
            vec![ContentBlock::Text {
                text: content,
                text_signature: None,
            }]
        },
        timestamp: 0,
        api: None,
        provider: None,
        model: None,
        response_id: None,
        response_model: None,
        provider_thinking_level: None,
        diagnostics: Vec::new(),
        usage: None,
        stop_reason: None,
        deferred: None,
        error_message: None,
        raw_stop_reason: None,
        end_turn: None,
        tool_call_id: None,
        tool_name: None,
        is_error: false,
        details: None,
        added_tool_names: Vec::new(),
        sections,
        tools_added,
        tools_removed,
    }
}

/// Build the leading system message for legacy prompt/tool fields.
pub fn create_initial_system_message(
    system_prompt: Option<&str>,
    tools: &[Tool],
) -> Option<Message> {
    let has_prompt = system_prompt.is_some_and(|prompt| !prompt.is_empty());
    if !has_prompt && tools.is_empty() {
        return None;
    }
    Some(system_message(
        system_prompt.unwrap_or_default(),
        None,
        tools.to_vec(),
        Vec::new(),
    ))
}

/// Fold legacy Context prompt/tool fields into a leading system message.
pub fn normalize_context(context: &Context) -> Context {
    let mut messages = Vec::with_capacity(context.messages.len() + 1);
    if let Some(initial) =
        create_initial_system_message(context.system_prompt.as_deref(), &context.tools)
    {
        messages.push(initial);
    }
    messages.extend(context.messages.clone());
    Context {
        system_prompt: None,
        messages,
        tools: Vec::new(),
    }
}

/// Return the leading system message, if present.
pub fn get_initial_system_message(messages: &[Message]) -> Option<&Message> {
    messages
        .first()
        .filter(|message| message.role == Role::System)
}

/// Drop the leading system message for APIs that carry the prompt separately.
pub fn without_initial_system_message(messages: &[Message]) -> Vec<Message> {
    if get_initial_system_message(messages).is_some() {
        messages[1..].to_vec()
    } else {
        messages.to_vec()
    }
}

/// Resolve the currently available tool set after replaying transcript deltas.
pub fn get_current_tools(messages: &[Message]) -> Vec<Tool> {
    let mut tools: IndexMap<String, Tool> = IndexMap::new();
    for message in messages
        .iter()
        .filter(|message| message.role == Role::System)
    {
        for removed in &message.tools_removed {
            tools.shift_remove(&removed.name);
        }
        for added in &message.tools_added {
            tools.insert(added.name.clone(), added.clone());
        }
    }
    tools.into_values().collect()
}

/// Render the full text represented by one system message.
pub fn get_system_message_text(message: &Message) -> String {
    let mut parts = Vec::new();
    let content = text_content(&message.content);
    if !content.is_empty() {
        parts.push(content);
    }
    if let Some(sections) = &message.sections {
        parts.extend(sections.values().filter_map(Clone::clone));
    }
    parts.join("\n\n")
}

/// Render an in-place system update for providers that accept later system messages.
pub fn render_system_message_update(message: &Message) -> String {
    let mut parts = Vec::new();
    let content = text_content(&message.content);
    if !content.is_empty() {
        parts.push(content);
    }
    if let Some(sections) = &message.sections {
        for (name, value) in sections {
            parts.push(match value {
                Some(value) => format!("Updated system prompt section \"{name}\":\n\n{value}"),
                None => format!("Removed system prompt section \"{name}\"."),
            });
        }
    }
    parts.join("\n\n")
}

/// Replay every system message into one leading message holding current prompt and tools.
pub fn get_current_system_message(messages: &[Message]) -> Option<Message> {
    let mut content = Vec::new();
    let mut sections: IndexMap<String, Option<String>> = IndexMap::new();
    let mut timestamp = None;
    for message in messages
        .iter()
        .filter(|message| message.role == Role::System)
    {
        timestamp.get_or_insert(message.timestamp);
        let text = text_content(&message.content);
        if !text.is_empty() {
            content.push(text);
        }
        if let Some(updates) = &message.sections {
            for (name, value) in updates {
                if let Some(value) = value {
                    sections.insert(name.clone(), Some(value.clone()));
                } else {
                    sections.shift_remove(name);
                }
            }
        }
    }
    let tools = get_current_tools(messages);
    if timestamp.is_none() && tools.is_empty() {
        return None;
    }
    let mut current = system_message(
        content.join("\n\n"),
        (!sections.is_empty()).then_some(sections),
        tools,
        Vec::new(),
    );
    current.timestamp = timestamp.unwrap_or(0);
    Some(current)
}

/// Render the current system prompt text after replaying every system message.
pub fn get_current_system_prompt(messages: &[Message]) -> String {
    get_current_system_message(messages)
        .as_ref()
        .map(get_system_message_text)
        .unwrap_or_default()
}

/// Collapse updates into one leading system message and preserve all non-system messages.
pub fn collapse_system_messages(context: &Context) -> Context {
    let head = get_current_system_message(&context.messages);
    let mut messages = Vec::with_capacity(context.messages.len());
    if let Some(head) = head {
        messages.push(head);
    }
    messages.extend(
        context
            .messages
            .iter()
            .filter(|message| message.role != Role::System)
            .cloned(),
    );
    Context {
        system_prompt: None,
        messages,
        tools: Vec::new(),
    }
}

/// Keep later system messages in place when supported; otherwise collapse them.
pub fn resolve_transcript(context: &Context, supports_mid_convo: Option<bool>) -> Context {
    if supports_mid_convo == Some(true) {
        context.clone()
    } else {
        collapse_system_messages(context)
    }
}

/// Normalize legacy fields and apply the model's transcript compatibility mode.
pub fn resolve_context(context: &Context, supports_mid_convo: Option<bool>) -> Context {
    resolve_transcript(&normalize_context(context), supports_mid_convo)
}

/// Compare complete model-facing tool declarations by JSON representation.
pub fn declarations_equal(left: &Tool, right: &Tool) -> bool {
    serde_json::to_value(left).ok() == serde_json::to_value(right).ok()
}

#[derive(Debug, Clone)]
pub struct ToolStateChanges {
    pub tools_added: Vec<Tool>,
    pub tools_removed: Vec<ToolReference>,
}

/// Compare two complete tool states; a changed declaration is removed then added.
pub fn get_tool_state_changes(previous: &[Tool], current: &[Tool]) -> ToolStateChanges {
    let previous_by_name: IndexMap<&str, &Tool> = previous
        .iter()
        .map(|tool| (tool.name.as_str(), tool))
        .collect();
    let current_by_name: IndexMap<&str, &Tool> = current
        .iter()
        .map(|tool| (tool.name.as_str(), tool))
        .collect();
    ToolStateChanges {
        tools_added: current
            .iter()
            .filter(|tool| {
                previous_by_name
                    .get(tool.name.as_str())
                    .is_none_or(|previous| !declarations_equal(previous, tool))
            })
            .cloned()
            .collect(),
        tools_removed: previous
            .iter()
            .filter(|tool| {
                current_by_name
                    .get(tool.name.as_str())
                    .is_none_or(|current| !declarations_equal(tool, current))
            })
            .map(|tool| ToolReference {
                name: tool.name.clone(),
            })
            .collect(),
    }
}

/// Every referenced tool declaration in first-declaration order.
pub fn get_declared_tools(messages: &[Message]) -> Vec<Tool> {
    let mut tools: IndexMap<String, Tool> = IndexMap::new();
    for message in messages
        .iter()
        .filter(|message| message.role == Role::System)
    {
        for tool in &message.tools_added {
            tools.insert(tool.name.clone(), tool.clone());
        }
    }
    tools.into_values().collect()
}

/// Whether a name was declared twice with different model-facing definitions.
pub fn has_tool_redefinitions(messages: &[Message]) -> bool {
    let mut declared: IndexMap<&str, &Tool> = IndexMap::new();
    for message in messages
        .iter()
        .filter(|message| message.role == Role::System)
    {
        for tool in &message.tools_added {
            if declared
                .get(tool.name.as_str())
                .is_some_and(|previous| !declarations_equal(previous, tool))
            {
                return true;
            }
            declared.insert(tool.name.as_str(), tool);
        }
    }
    false
}

/// Whether history contains a removal or same-name redeclaration.
pub fn has_non_additive_tool_changes(messages: &[Message]) -> bool {
    let mut declared = std::collections::HashSet::new();
    for message in messages
        .iter()
        .filter(|message| message.role == Role::System)
    {
        if !message.tools_removed.is_empty() {
            return true;
        }
        for tool in &message.tools_added {
            if !declared.insert(tool.name.as_str()) {
                return true;
            }
        }
    }
    false
}

#[derive(Debug, Clone)]
pub struct TranscriptTools {
    pub request_tools: Vec<Tool>,
    pub anchors_additions: bool,
}

#[derive(Debug, Clone)]
pub struct PreparedTranscript {
    /// Legacy-shaped context consumed by the existing provider converters. The leading
    /// system message is represented by `system_prompt`/`tools`; supported later system
    /// messages remain in `messages`.
    pub context: Context,
    pub anchors_additions: bool,
}

/// Split declarations between request-level tools and in-place additions.
pub fn resolve_transcript_tools(
    messages: &[Message],
    supports_tool_additions: bool,
) -> TranscriptTools {
    let anchors_additions = supports_tool_additions && !has_non_additive_tool_changes(messages);
    let request_tools = if anchors_additions {
        get_initial_system_message(messages)
            .map(|message| message.tools_added.clone())
            .unwrap_or_default()
    } else {
        get_current_tools(messages)
    };
    TranscriptTools {
        request_tools,
        anchors_additions,
    }
}

/// Normalize, replay as required by compatibility, and split the leading system message
/// back into the request-level fields used by provider payload builders.
pub fn prepare_transcript(
    context: &Context,
    supports_mid_convo: Option<bool>,
    supports_tool_additions: bool,
) -> PreparedTranscript {
    let normalized = normalize_context(context);
    let resolved = resolve_transcript(&normalized, supports_mid_convo);
    let tools = resolve_transcript_tools(&resolved.messages, supports_tool_additions);
    let system_prompt = get_initial_system_message(&resolved.messages)
        .map(get_system_message_text)
        .filter(|prompt| !prompt.is_empty());
    let messages = without_initial_system_message(&resolved.messages);
    PreparedTranscript {
        context: Context {
            system_prompt,
            messages,
            tools: tools.request_tools,
        },
        anchors_additions: tools.anchors_additions,
    }
}
