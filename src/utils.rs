//! Utility helpers: sanitize, hashing, Copilot headers.

use std::collections::HashMap;

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
pub fn copilot_dynamic_headers(messages: &[crate::types::Message]) -> Vec<(&'static str, &'static str)> {
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
    use crate::types::{Role, ContentBlock};
    messages.iter().any(|m| {
        matches!(m.role, Role::User | Role::ToolResult)
            && m.content.iter().any(|c| matches!(c, ContentBlock::Image { .. }))
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
        use crate::types::{Message, Role, ContentBlock};
        fn msg(role: Role, content: Vec<ContentBlock>) -> Message {
            Message {
                role, content, timestamp: 0,
                api: None, provider: None, model: None, response_id: None, response_model: None,
                diagnostics: Vec::new(), usage: None, stop_reason: None, error_message: None,
                tool_call_id: None, tool_name: None, is_error: false, details: None,
            }
        }
        // Last message from user -> initiator user, no vision.
        let h = copilot_dynamic_headers(&[msg(Role::User, vec![ContentBlock::Text { text: "hi".into(), text_signature: None }])]);
        assert!(h.contains(&("X-Initiator", "user")));
        assert!(h.contains(&("Openai-Intent", "conversation-edits")));
        assert!(!h.iter().any(|(k, _)| *k == "Copilot-Vision-Request"));
        // Last message from assistant -> initiator agent; user image -> vision header.
        let h2 = copilot_dynamic_headers(&[
            msg(Role::User, vec![ContentBlock::Image { data: "a".into(), mime_type: "image/png".into() }]),
            msg(Role::Assistant, vec![ContentBlock::Text { text: "ok".into(), text_signature: None }]),
        ]);
        assert!(h2.contains(&("X-Initiator", "agent")));
        assert!(h2.contains(&("Copilot-Vision-Request", "true")));
    }

    #[test]
    fn test_infer_copilot_initiator_and_vision() {
        use crate::types::{Message, Role, ContentBlock};
        fn msg(role: Role, content: Vec<ContentBlock>) -> Message {
            Message {
                role, content, timestamp: 0,
                api: None, provider: None, model: None, response_id: None, response_model: None,
                diagnostics: Vec::new(), usage: None, stop_reason: None, error_message: None,
                tool_call_id: None, tool_name: None, is_error: false, details: None,
            }
        }
        // empty -> user
        assert_eq!(infer_copilot_initiator(&[]), "user");
        // last user -> user
        assert_eq!(infer_copilot_initiator(&[msg(Role::User, vec![])]), "user");
        // last assistant -> agent
        assert_eq!(infer_copilot_initiator(&[msg(Role::Assistant, vec![])]), "agent");
        // last toolResult -> agent
        assert_eq!(infer_copilot_initiator(&[msg(Role::ToolResult, vec![])]), "agent");
        // vision: user image
        assert!(has_copilot_vision_input(&[msg(Role::User, vec![ContentBlock::Image { data: "a".into(), mime_type: "image/png".into() }])]));
        // vision: toolResult image
        assert!(has_copilot_vision_input(&[msg(Role::ToolResult, vec![ContentBlock::Image { data: "a".into(), mime_type: "image/png".into() }])]));
        // no vision: text only
        assert!(!has_copilot_vision_input(&[msg(Role::User, vec![ContentBlock::Text { text: "hi".into(), text_signature: None }])]));
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
    h1 = (h1 ^ (h1 >> 16)).wrapping_mul(2_246_822_507) ^ (h2 ^ (h2 >> 13)).wrapping_mul(3_266_489_909);
    h2 = (h2 ^ (h2 >> 16)).wrapping_mul(2_246_822_507) ^ (h1 ^ (h1 >> 13)).wrapping_mul(3_266_489_909);
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
/// environment (mirrors upstream `resolveCloudflareBaseUrl`).
/// Substitute `{VAR}` placeholders in a Cloudflare base URL from environment
/// variables. Mirrors upstream `resolveCloudflareBaseUrl`: a referenced variable
/// that is unset or empty is an error (the request can't be built).
pub fn resolve_cloudflare_base_url(base_url: &str, provider: &str) -> Result<String, String> {
    if !base_url.contains('{') {
        return Ok(base_url.to_string());
    }
    let mut out = String::with_capacity(base_url.len());
    let bytes = base_url.as_bytes();
    let mut i = 0;
    while i < base_url.len() {
        if bytes[i] == b'{'
            && let Some(end) = base_url[i + 1..].find('}') {
                let name = &base_url[i + 1..i + 1 + end];
                if name.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
                    && name.chars().next().map(|c| c.is_ascii_uppercase() || c == '_').unwrap_or(false)
                {
                    match std::env::var(name) {
                        Ok(value) if !value.is_empty() => out.push_str(&value),
                        _ => return Err(format!("{name} is required for provider {provider} but is not set.")),
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
