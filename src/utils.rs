//! Utility helpers: sanitize, hashing, Copilot headers.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

/// Sanitize surrogate pairs from a string (replaces unpaired surrogates with replacement char).
pub fn sanitize_surrogates(s: &str) -> String {
    // Rust strings are valid UTF-8 by construction, so surrogates cannot appear.
    // This is a no-op in Rust but exists for API parity with the Go/TS versions.
    s.to_string()
}

/// Simple hash of a string (FNV-1a inspired, for cache keys).
pub fn hash_string(s: &str) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in s.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

static UUIDV7_LAST_ORDINARY_MS: AtomicU64 = AtomicU64::new(0);
static UUIDV7_SEQUENCE: AtomicU64 = AtomicU64::new(u64::MAX);

const MAX_UUID_V7_TIMESTAMP: u64 = 0xffff_ffff_ffff;
const MAX_UUID_V7_SEQUENCE: u64 = (1_u64 << 41) - 1;

/// Extract and join text blocks from message content (upstream `contentText`).
pub fn content_text(content: &[crate::types::ContentBlock], separator: &str) -> String {
    content
        .iter()
        .filter_map(|block| match block {
            crate::types::ContentBlock::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join(separator)
}

/// Generate a time-ordered UUIDv7 (RFC 9562 layout; mirrors upstream utility).
pub fn uuidv7() -> String {
    uuidv7_inner(crate::utils::now_millis().max(0) as u64, true)
}

/// Generate a UUIDv7 for an explicit millisecond timestamp.
pub fn uuidv7_with_timestamp(timestamp_ms: u64) -> String {
    uuidv7_inner(timestamp_ms, false)
}

fn uuidv7_inner(timestamp_ms: u64, ordinary_timestamp: bool) -> String {
    use rand::RngCore;
    assert!(
        timestamp_ms <= MAX_UUID_V7_TIMESTAMP,
        "UUIDv7 timestamp must be an integer between 0 and 281474976710655"
    );
    let mut random = [0u8; 16];
    rand::rngs::OsRng.fill_bytes(&mut random);

    // Upstream v0.85.0 preserves caller-supplied timestamps for follower ids;
    // only ordinary Date.now()-style calls are made monotonic against the last
    // ordinary timestamp.
    let ts = if ordinary_timestamp {
        let mut last = UUIDV7_LAST_ORDINARY_MS.load(Ordering::SeqCst);
        loop {
            let effective = timestamp_ms.max(last);
            match UUIDV7_LAST_ORDINARY_MS.compare_exchange(
                last,
                effective,
                Ordering::SeqCst,
                Ordering::SeqCst,
            ) {
                Ok(_) => break effective,
                Err(observed) => last = observed,
            }
        }
    } else {
        timestamp_ms
    };

    let seed = ((random[1] as u64) << 32)
        | ((random[2] as u64) << 24)
        | ((random[3] as u64) << 16)
        | ((random[4] as u64) << 8)
        | random[5] as u64;
    let sequence = loop {
        let current = UUIDV7_SEQUENCE.load(Ordering::SeqCst);
        if current == u64::MAX {
            if UUIDV7_SEQUENCE
                .compare_exchange(current, seed, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
            {
                break seed;
            }
            continue;
        }
        assert!(
            current < MAX_UUID_V7_SEQUENCE,
            "UUIDv7 generator sequence exhausted"
        );
        let next = current + 1;
        if UUIDV7_SEQUENCE
            .compare_exchange(current, next, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            break next;
        }
    };

    let mut bytes = [0u8; 16];
    bytes[0] = ((ts >> 40) & 0xff) as u8;
    bytes[1] = ((ts >> 32) & 0xff) as u8;
    bytes[2] = ((ts >> 24) & 0xff) as u8;
    bytes[3] = ((ts >> 16) & 0xff) as u8;
    bytes[4] = ((ts >> 8) & 0xff) as u8;
    bytes[5] = (ts & 0xff) as u8;
    bytes[6] = 0x70 | (((sequence >> 37) & 0x0f) as u8);
    bytes[7] = ((sequence >> 29) & 0xff) as u8;
    bytes[8] = 0x80 | (((sequence >> 23) & 0x3f) as u8);
    bytes[9] = ((sequence >> 15) & 0xff) as u8;
    bytes[10] = ((sequence >> 7) & 0xff) as u8;
    bytes[11] = (((sequence & 0x7f) << 1) as u8) | (random[11] & 0x01);
    bytes[12..16].copy_from_slice(&random[12..16]);
    let hex = bytes.map(|b| format!("{b:02x}"));
    format!(
        "{}{}{}{}-{}{}-{}{}-{}{}-{}{}{}{}{}{}",
        hex[0],
        hex[1],
        hex[2],
        hex[3],
        hex[4],
        hex[5],
        hex[6],
        hex[7],
        hex[8],
        hex[9],
        hex[10],
        hex[11],
        hex[12],
        hex[13],
        hex[14],
        hex[15]
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrammarConstrainedSampling {
    pub format: String,
    pub definition: String,
    pub input_property: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GrammarToolInputJsonBuffer {
    pub input: String,
    pub started: bool,
    pub closed: bool,
}

pub fn get_grammar_tool_input(
    tool_name: &str,
    arguments: &serde_json::Value,
    input_property: &str,
) -> Result<String, String> {
    arguments
        .get(input_property)
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .ok_or_else(|| format!("Grammar tool call \"{tool_name}\" requires argument \"{input_property}\" to be a string."))
}

pub fn append_grammar_tool_input_json_delta(
    buffer: &mut GrammarToolInputJsonBuffer,
    input_property: &str,
    next_input: &str,
    close: bool,
) -> Result<Option<String>, String> {
    if buffer.closed {
        if close && next_input == buffer.input {
            return Ok(None);
        }
        return Err(format!(
            "grammar tool input for property \"{input_property}\" changed after it was closed"
        ));
    }
    if !next_input.starts_with(&buffer.input) {
        return Err(format!(
            "grammar tool input for property \"{input_property}\" changed non-monotonically"
        ));
    }
    let input_delta = &next_input[buffer.input.len()..];
    if !close && input_delta.is_empty() {
        return Ok(None);
    }
    let mut delta = String::new();
    if !buffer.started {
        delta.push('{');
        delta.push_str(&serde_json::to_string(input_property).unwrap());
        delta.push_str(":\"");
        buffer.started = true;
    }
    let encoded = serde_json::to_string(input_delta).unwrap();
    delta.push_str(&encoded[1..encoded.len() - 1]);
    buffer.input = next_input.to_string();
    if close {
        delta.push_str("\"}");
        buffer.closed = true;
    }
    Ok(Some(delta))
}

fn infer_grammar_input_property(tool: &crate::types::Tool) -> Result<String, String> {
    let schema = &tool.parameters;
    if schema.get("type").and_then(|v| v.as_str()) != Some("object") {
        return Err("grammar constrained sampling requires an object parameter schema".into());
    }
    let required = schema
        .get("required")
        .and_then(|v| v.as_array())
        .ok_or_else(|| {
            "grammar constrained sampling requires exactly one required string property".to_string()
        })?;
    if required.len() != 1 || !required[0].is_string() {
        return Err(
            "grammar constrained sampling requires exactly one required string property".into(),
        );
    }
    let input_property = required[0].as_str().unwrap();
    let prop = schema
        .pointer(&format!("/properties/{input_property}"))
        .ok_or_else(|| {
            format!("grammar constrained sampling requires a properties entry for {input_property}")
        })?;
    if prop.get("type").and_then(|v| v.as_str()) != Some("string") {
        return Err(format!(
            "grammar constrained sampling property {input_property} must have type string"
        ));
    }
    Ok(input_property.to_string())
}

pub fn resolve_grammar_constrained_sampling(
    tool: &crate::types::Tool,
    supports_openai_grammar_tools: bool,
) -> Result<Option<GrammarConstrainedSampling>, String> {
    let Some(config) = tool.constrained_sampling.as_ref() else {
        return Ok(None);
    };
    if config.get("type").and_then(|v| v.as_str()) != Some("grammar") {
        return Ok(None);
    }
    if !supports_openai_grammar_tools {
        return Ok(None);
    }
    let variants = config.get("variants").unwrap_or(&serde_json::Value::Null);
    let lark = variants
        .get("openai_lark")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty());
    let regex = variants
        .get("openai_regex")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty());
    let (format, definition) = if let Some(d) = lark {
        ("lark", d)
    } else if let Some(d) = regex {
        ("regex", d)
    } else {
        return Err(format!(
            "Tool \"{}\" cannot use grammar constrained sampling: no supported grammar variant was provided.",
            tool.name
        ));
    };
    let input_property = infer_grammar_input_property(tool).map_err(|e| {
        format!(
            "Tool \"{}\" cannot use grammar constrained sampling: {e}.",
            tool.name
        )
    })?;
    Ok(Some(GrammarConstrainedSampling {
        format: format.into(),
        definition: definition.into(),
        input_property,
    }))
}

pub fn openai_tool_value(
    tool: &crate::types::Tool,
    supports_grammar: bool,
    supports_strict: bool,
    default_strict: bool,
) -> Result<serde_json::Value, String> {
    if let Some(grammar) = resolve_grammar_constrained_sampling(tool, supports_grammar)? {
        return Ok(serde_json::json!({
            "type": "custom",
            "name": tool.name,
            "description": tool.description,
            "format": {"type": "grammar", "syntax": grammar.format, "definition": grammar.definition},
        }));
    }
    let strict =
        resolve_json_schema_strict_sampling(tool, supports_strict)?.unwrap_or(default_strict);
    let parameters = if strict {
        make_strict_json_schema(&tool.parameters)?
    } else {
        tool.parameters.clone()
    };
    let mut function = serde_json::json!({"name": tool.name, "description": tool.description, "parameters": parameters});
    if supports_strict {
        function["strict"] = serde_json::json!(strict);
    }
    Ok(serde_json::json!({"type":"function", "function": function}))
}

fn schema_object(value: &serde_json::Value) -> Option<&serde_json::Map<String, serde_json::Value>> {
    value.as_object()
}

fn schema_types(schema: &serde_json::Value) -> Vec<&str> {
    match schema.get("type") {
        Some(serde_json::Value::String(s)) => vec![s.as_str()],
        Some(serde_json::Value::Array(arr)) => arr.iter().filter_map(|v| v.as_str()).collect(),
        _ => Vec::new(),
    }
}

fn schema_allows_null(schema: &serde_json::Value) -> bool {
    if schema.get("type").and_then(|v| v.as_str()) == Some("null") {
        return true;
    }
    if schema_types(schema).contains(&"null") {
        return true;
    }
    if schema.get("const") == Some(&serde_json::Value::Null) {
        return true;
    }
    if let Some(arr) = schema.get("enum").and_then(|v| v.as_array())
        && arr.iter().any(|v| v.is_null())
    {
        return true;
    }
    schema
        .get("anyOf")
        .and_then(|v| v.as_array())
        .is_some_and(|arr| arr.iter().any(schema_allows_null))
}

fn is_structured_schema(schema: &serde_json::Value) -> bool {
    schema_object(schema).is_some_and(|obj| {
        let types = schema_types(schema);
        types.contains(&"object")
            || types.contains(&"array")
            || obj.contains_key("properties")
            || obj.contains_key("items")
    })
}

pub fn make_strict_json_schema(schema: &serde_json::Value) -> Result<serde_json::Value, String> {
    let mut cloned = schema.clone();
    make_json_schema_node_strict(&mut cloned)?;
    if cloned.get("type").and_then(|v| v.as_str()) != Some("object") {
        return Err("root schema must have type object".into());
    }
    Ok(cloned)
}

fn make_json_schema_node_strict(schema: &mut serde_json::Value) -> Result<(), String> {
    let Some(obj) = schema.as_object_mut() else {
        return Err("boolean schemas are unsupported".into());
    };
    const UNSUPPORTED: &[&str] = &[
        "$ref",
        "$defs",
        "definitions",
        "allOf",
        "oneOf",
        "patternProperties",
        "dependentSchemas",
        "dependencies",
        "unevaluatedProperties",
        "propertyNames",
        "contains",
        "prefixItems",
        "not",
        "if",
        "then",
        "else",
    ];
    for key in UNSUPPORTED {
        if obj.contains_key(*key) {
            return Err(format!("{key} schemas are unsupported"));
        }
    }
    if let Some(any_of) = obj.get_mut("anyOf") {
        let Some(arr) = any_of.as_array_mut() else {
            return Err("anyOf must contain at least one schema".into());
        };
        if arr.is_empty() {
            return Err("anyOf must contain at least one schema".into());
        }
        for variant in arr {
            if is_structured_schema(variant) {
                return Err("object and array unions are unsupported".into());
            }
            make_json_schema_node_strict(variant)?;
        }
    }
    if let Some(items) = obj.get_mut("items") {
        if items.is_array() {
            return Err("tuple schemas are unsupported".into());
        }
        make_json_schema_node_strict(items)?;
    }
    let is_object = obj.get("type").and_then(|v| v.as_str()) == Some("object");
    if obj.contains_key("properties") && !is_object {
        return Err("properties require type object".into());
    }
    if !is_object {
        return Ok(());
    }
    if obj
        .get("additionalProperties")
        .is_some_and(|v| v != &serde_json::Value::Bool(false))
    {
        return Err("schema-valued or true additionalProperties is unsupported".into());
    }
    let required_values = obj
        .get("required")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let properties = match obj.get_mut("properties") {
        Some(serde_json::Value::Object(props)) => props,
        Some(_) => return Err("object properties must be a schema map".into()),
        None => {
            obj.insert("properties".into(), serde_json::json!({}));
            obj.get_mut("properties").unwrap().as_object_mut().unwrap()
        }
    };
    if !required_values.iter().all(|v| v.is_string()) {
        return Err("object required must be a string array".into());
    }
    let mut required = required_values
        .iter()
        .filter_map(|v| v.as_str().map(ToOwned::to_owned))
        .collect::<std::collections::HashSet<_>>();
    if required.iter().any(|key| !properties.contains_key(key)) {
        return Err("required contains an unknown property".into());
    }
    let property_names = properties.keys().cloned().collect::<Vec<_>>();
    for key in &property_names {
        let property = properties.get_mut(key).unwrap();
        make_json_schema_node_strict(property)?;
        if !required.contains(key) && !schema_allows_null(property) {
            let old = property.clone();
            *property = serde_json::json!({"anyOf": [old, {"type": "null"}]});
        }
        required.insert(key.clone());
    }
    obj.insert(
        "required".into(),
        serde_json::Value::Array(
            property_names
                .into_iter()
                .map(serde_json::Value::String)
                .collect(),
        ),
    );
    obj.insert(
        "additionalProperties".into(),
        serde_json::Value::Bool(false),
    );
    Ok(())
}

/// Resolve JSON-schema constrained sampling strict mode for a tool.
pub fn resolve_json_schema_strict_sampling(
    tool: &crate::types::Tool,
    supports_strict_mode: bool,
) -> Result<Option<bool>, String> {
    let Some(config) = tool.constrained_sampling.as_ref() else {
        return Ok(None);
    };
    if config.get("type").and_then(|v| v.as_str()) != Some("json_schema") {
        return Ok(None);
    }
    if supports_strict_mode {
        return Ok(Some(true));
    }
    if config.get("strict").and_then(|v| v.as_str()) == Some("require") {
        return Err(format!(
            "Tool \"{}\" requires JSON-schema constrained sampling, but strict tools are unsupported.",
            tool.name
        ));
    }
    Ok(None)
}

/// Build the Pi runtime User-Agent used by Kimi/Codex provider transports.
pub fn pi_runtime_user_agent() -> String {
    let platform = match std::env::consts::OS {
        "macos" => "darwin",
        "windows" => "win32",
        other => other,
    };
    let arch = match std::env::consts::ARCH {
        "x86_64" => "x64",
        "aarch64" => "arm64",
        other => other,
    };
    let release = std::fs::read_to_string("/proc/sys/kernel/osrelease")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| {
            std::process::Command::new("uname")
                .arg("-r")
                .output()
                .ok()
                .filter(|out| out.status.success())
                .and_then(|out| String::from_utf8(out.stdout).ok())
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        })
        .unwrap_or_else(|| "unknown".to_string());
    format!("pi ({platform} {release}; {arch})")
}

/// Generate GitHub Copilot headers.
pub fn copilot_headers() -> HashMap<String, String> {
    HashMap::from([
        ("Copilot-Integration-Id".into(), "vscode-chat".into()),
        ("Editor-Plugin-Version".into(), "copilot-chat/0.35.0".into()),
        ("Editor-Version".into(), "vscode/1.107.0".into()),
        ("User-Agent".into(), "GitHubCopilotChat/0.35.0".into()),
    ])
}

/// Generate GitHub Copilot headers with an intent field.
pub fn copilot_headers_with_intent(intent: &str) -> HashMap<String, String> {
    let mut h = copilot_headers();
    h.insert("openai-intent".into(), intent.into());
    h
}

/// GitHub Copilot dynamic per-request headers (mirrors buildCopilotDynamicHeaders):
/// X-Initiator (agent when the last message isn't from the user), Openai-Intent,
/// and Copilot-Vision-Request when any user/tool-result message carries an image.
pub fn copilot_dynamic_headers(
    messages: &[crate::types::Message],
) -> Vec<(&'static str, &'static str)> {
    let mut headers = vec![
        ("X-Initiator", infer_copilot_initiator(messages)),
        ("Openai-Intent", "conversation-edits"),
    ];
    if has_copilot_vision_input(messages) {
        headers.push(("Copilot-Vision-Request", "true"));
    }
    headers
}

/// Mirrors upstream `inferCopilotInitiator`: "agent" when the last message is not
/// from the user (follow-up after assistant/tool messages), else "user".
pub fn infer_copilot_initiator(messages: &[crate::types::Message]) -> &'static str {
    use crate::types::Role;
    match messages.last() {
        Some(m) if m.role != Role::User => "agent",
        _ => "user",
    }
}

/// Mirrors upstream `hasCopilotVisionInput`: true when any user/toolResult message
/// carries an image content block.
pub fn has_copilot_vision_input(messages: &[crate::types::Message]) -> bool {
    use crate::types::{ContentBlock, Role};
    messages.iter().any(|m| {
        matches!(m.role, Role::User | Role::ToolResult)
            && m.content
                .iter()
                .any(|c| matches!(c, ContentBlock::Image { .. }))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_surrogates_noop() {
        assert_eq!(sanitize_surrogates("Hello 🙈"), "Hello 🙈");
    }

    #[test]
    fn test_hash_string_deterministic() {
        let h1 = hash_string("test");
        let h2 = hash_string("test");
        assert_eq!(h1, h2);
        assert_ne!(h1, hash_string("other"));
    }

    #[test]
    fn test_copilot_headers() {
        let h = copilot_headers();
        assert_eq!(h.get("User-Agent").unwrap(), "GitHubCopilotChat/0.35.0");
    }

    #[test]
    fn test_copilot_headers_with_intent() {
        let h = copilot_headers_with_intent("chat");
        assert_eq!(h.get("openai-intent").unwrap(), "chat");
    }

    #[test]
    fn test_copilot_dynamic_headers() {
        use crate::types::{ContentBlock, Message, Role};
        fn msg(role: Role, content: Vec<ContentBlock>) -> Message {
            Message {
                role,
                content,
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
            }
        }
        // Last message from user -> initiator user, no vision.
        let h = copilot_dynamic_headers(&[msg(
            Role::User,
            vec![ContentBlock::Text {
                text: "hi".into(),
                text_signature: None,
            }],
        )]);
        assert!(h.contains(&("X-Initiator", "user")));
        assert!(h.contains(&("Openai-Intent", "conversation-edits")));
        assert!(!h.iter().any(|(k, _)| *k == "Copilot-Vision-Request"));
        // Last message from assistant -> initiator agent; user image -> vision header.
        let h2 = copilot_dynamic_headers(&[
            msg(
                Role::User,
                vec![ContentBlock::Image {
                    data: "a".into(),
                    mime_type: "image/png".into(),
                }],
            ),
            msg(
                Role::Assistant,
                vec![ContentBlock::Text {
                    text: "ok".into(),
                    text_signature: None,
                }],
            ),
        ]);
        assert!(h2.contains(&("X-Initiator", "agent")));
        assert!(h2.contains(&("Copilot-Vision-Request", "true")));
    }

    #[test]
    fn test_infer_copilot_initiator_and_vision() {
        use crate::types::{ContentBlock, Message, Role};
        fn msg(role: Role, content: Vec<ContentBlock>) -> Message {
            Message {
                role,
                content,
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
            }
        }
        // empty -> user
        assert_eq!(infer_copilot_initiator(&[]), "user");
        // last user -> user
        assert_eq!(infer_copilot_initiator(&[msg(Role::User, vec![])]), "user");
        // last assistant -> agent
        assert_eq!(
            infer_copilot_initiator(&[msg(Role::Assistant, vec![])]),
            "agent"
        );
        // last toolResult -> agent
        assert_eq!(
            infer_copilot_initiator(&[msg(Role::ToolResult, vec![])]),
            "agent"
        );
        // vision: user image
        assert!(has_copilot_vision_input(&[msg(
            Role::User,
            vec![ContentBlock::Image {
                data: "a".into(),
                mime_type: "image/png".into()
            }]
        )]));
        // vision: toolResult image
        assert!(has_copilot_vision_input(&[msg(
            Role::ToolResult,
            vec![ContentBlock::Image {
                data: "a".into(),
                mime_type: "image/png".into()
            }]
        )]));
        // no vision: text only
        assert!(!has_copilot_vision_input(&[msg(
            Role::User,
            vec![ContentBlock::Text {
                text: "hi".into(),
                text_signature: None
            }]
        )]));
    }
}

/// Short hash (first 8 hex chars of FNV hash).
pub fn short_hash(s: &str) -> String {
    // Exact port of upstream `shortHash` (utils/hash.js): a cyrb53-style 64-bit
    // double hash emitting base-36 of two unsigned 32-bit halves. `charCodeAt`
    // iterates UTF-16 code units and `Math.imul` is 32-bit wrapping multiply.
    let mut h1: u32 = 0xdead_beef;
    let mut h2: u32 = 0x41c6_ce57;
    for ch in s.encode_utf16() {
        let ch = ch as u32;
        h1 = (h1 ^ ch).wrapping_mul(2_654_435_761);
        h2 = (h2 ^ ch).wrapping_mul(1_597_334_677);
    }
    h1 = (h1 ^ (h1 >> 16)).wrapping_mul(2_246_822_507)
        ^ (h2 ^ (h2 >> 13)).wrapping_mul(3_266_489_909);
    h2 = (h2 ^ (h2 >> 16)).wrapping_mul(2_246_822_507)
        ^ (h1 ^ (h1 >> 13)).wrapping_mul(3_266_489_909);
    format!("{}{}", to_base36(u64::from(h2)), to_base36(u64::from(h1)))
}

/// Encode a u64 in lowercase base36 (mirrors JS `Number.toString(36)`).
pub(crate) fn to_base36(mut n: u64) -> String {
    const ALPHABET: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    if n == 0 {
        return "0".to_string();
    }
    let mut s = Vec::new();
    while n > 0 {
        s.push(ALPHABET[(n % 36) as usize]);
        n /= 36;
    }
    s.reverse();
    String::from_utf8(s).unwrap()
}

/// Check if a provider is a Cloudflare provider.
pub fn is_cloudflare_provider(provider: &str) -> bool {
    provider == "cloudflare-workers-ai" || provider == "cloudflare-ai-gateway"
}

/// Resolve a Cloudflare base URL, substituting `{ENV_VAR}` placeholders from the
/// environment (mirrors upstream `resolveCloudflareModel`). Unresolved placeholders
/// are intentionally preserved so request dispatch can keep a model unchanged when
/// provider env is absent.
pub fn resolve_cloudflare_base_url(base_url: &str, _provider: &str) -> Result<String, String> {
    if !base_url.contains('{') {
        return Ok(base_url.to_string());
    }
    let mut out = String::with_capacity(base_url.len());
    let bytes = base_url.as_bytes();
    let mut i = 0;
    while i < base_url.len() {
        if bytes[i] == b'{'
            && let Some(end) = base_url[i + 1..].find('}')
        {
            let name = &base_url[i + 1..i + 1 + end];
            if name
                .chars()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
                && name
                    .chars()
                    .next()
                    .map(|c| c.is_ascii_uppercase() || c == '_')
                    .unwrap_or(false)
            {
                match std::env::var(name) {
                    Ok(value) if !value.is_empty() => out.push_str(&value),
                    _ => {
                        out.push('{');
                        out.push_str(name);
                        out.push('}');
                    }
                }
                i = i + 1 + end + 1;
                continue;
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    Ok(out)
}

/// Format a thrown/panic value as a string (Rust equivalent: just Display).
pub fn format_thrown_value(err: &dyn std::fmt::Display) -> String {
    err.to_string()
}

/// Current unix time in milliseconds.
pub fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}
