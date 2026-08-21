//! Deferred/message-anchored tool loading helpers (upstream deferred-tools).

use crate::types::{ContentBlock, Context, Message, Model, Role, Tool};
use std::collections::{HashMap, HashSet};

pub(crate) fn anthropic_supports_tool_references(model: &Model) -> bool {
    if let Some(v) = model.compat.supports_tool_references {
        return v;
    }
    if model.api != "anthropic-messages" {
        return false;
    }
    let id = model.id.as_str();
    !(id.contains("haiku") || id == "claude-sonnet-4-20250514")
}

pub(crate) fn openai_supports_additional_tools(model: &Model) -> bool {
    if let Some(v) = model.compat.supports_additional_tools {
        return v;
    }
    match model.api.as_str() {
        "openai-responses" => matches!(model.id.as_str(), "gpt-5.4" | "gpt-5.4-codex"),
        _ => false,
    }
}

pub(crate) fn openai_supports_tool_search(model: &Model) -> bool {
    if let Some(v) = model.compat.supports_tool_search {
        return v;
    }
    match model.api.as_str() {
        "openai-responses" => matches!(model.id.as_str(), "gpt-5.4" | "gpt-5.4-codex"),
        "openai-codex-responses" => model.id == "gpt-5.4",
        _ => false,
    }
}

pub(crate) fn canonical_tool_name(name: &str, anthropic_oauth: bool) -> String {
    if anthropic_oauth {
        crate::provider::anthropic::to_claude_code_name(name)
    } else {
        name.to_string()
    }
}

pub(crate) fn active_tools(context: &Context, anthropic_oauth: bool) -> Vec<Tool> {
    let mut by_name: HashMap<String, Tool> = HashMap::new();
    for tool in &context.tools {
        let mut t = tool.clone();
        t.name = canonical_tool_name(&t.name, anthropic_oauth);
        by_name.insert(t.name.clone(), t);
    }
    let mut out: Vec<Tool> = by_name.into_values().collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    // Preserve context order after canonicalization where possible; replacement keeps canonical definition.
    let mut ordered = Vec::new();
    let mut seen = HashSet::new();
    for tool in &context.tools {
        let name = canonical_tool_name(&tool.name, anthropic_oauth);
        if seen.insert(name.clone())
            && let Some(t) = out.iter().find(|t| t.name == name)
        {
            ordered.push(t.clone());
        }
    }
    ordered
}

fn tool_calls_in(message: &Message, anthropic_oauth: bool) -> Vec<String> {
    message
        .content
        .iter()
        .filter_map(|b| match b {
            ContentBlock::ToolCall { name, .. } => Some(canonical_tool_name(name, anthropic_oauth)),
            _ => None,
        })
        .collect()
}

pub(crate) fn deferred_tool_names_at(
    context: &Context,
    marker_index: usize,
    anthropic_oauth: bool,
) -> Vec<String> {
    let active = active_tools(context, anthropic_oauth);
    let active_names: HashSet<String> = active.iter().map(|t| t.name.clone()).collect();
    let Some(marker) = context.messages.get(marker_index) else {
        return Vec::new();
    };
    let mut marked: Vec<String> = marker
        .added_tool_names
        .iter()
        .map(|n| canonical_tool_name(n, anthropic_oauth))
        .filter(|n| active_names.contains(n))
        .collect();
    marked.sort();
    marked.dedup();
    if marked.is_empty() || marked.len() == active_names.len() {
        return Vec::new();
    }
    let used_before: HashSet<String> = context.messages[..marker_index]
        .iter()
        .flat_map(|m| tool_calls_in(m, anthropic_oauth))
        .collect();
    marked
        .into_iter()
        .filter(|n| !used_before.contains(n))
        .collect()
}

pub(crate) fn immediate_and_deferred_tools(
    context: &Context,
    anthropic_oauth: bool,
    supported: bool,
) -> (Vec<Tool>, HashSet<String>) {
    let active = active_tools(context, anthropic_oauth);
    if !supported {
        return (active, HashSet::new());
    }
    let mut all_deferred = HashSet::new();
    for (i, msg) in context.messages.iter().enumerate() {
        if msg.role == Role::ToolResult {
            all_deferred.extend(deferred_tool_names_at(context, i, anthropic_oauth));
        }
    }
    if all_deferred.is_empty() || all_deferred.len() == active.len() {
        return (active, HashSet::new());
    }
    let immediate = active
        .into_iter()
        .filter(|t| !all_deferred.contains(&t.name))
        .collect();
    (immediate, all_deferred)
}

pub(crate) fn tool_by_name(context: &Context, name: &str, anthropic_oauth: bool) -> Option<Tool> {
    active_tools(context, anthropic_oauth)
        .into_iter()
        .find(|t| t.name == name)
}
