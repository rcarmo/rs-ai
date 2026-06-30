//! Provider HTTP error formatting — port of upstream `utils/error-body.ts` (v0.80.3).
//!
//! Upstream `normalizeProviderError`/`formatProviderError` exist because JS SDKs
//! hide the HTTP error body under SDK-specific fields. rs-ai's reqwest path reads
//! the body directly (`resp.text()`), so only the portable display contract is
//! ported here: compose `"{status}: {body}"` (or `"{prefix} ({status}): {body}"`
//! for branded providers), with the body trimmed and truncated to the cap.

/// Maximum characters of an error body surfaced in a provider error message.
pub const MAX_PROVIDER_ERROR_BODY_CHARS: usize = 4000;

/// Truncate `text` to `max_chars`, appending the upstream `... [truncated N chars]`
/// marker when shortened. Counts by `char` (error bodies are ASCII/JSON in practice).
pub fn truncate_error_text(text: &str, max_chars: usize) -> String {
    let count = text.chars().count();
    if count <= max_chars {
        return text.to_string();
    }
    let head: String = text.chars().take(max_chars).collect();
    format!("{head}... [truncated {} chars]", count - max_chars)
}

/// Compose a provider HTTP error display string from a status code and raw body.
/// `prefix` brands the message for providers that do so (OpenAI/Azure Responses).
pub fn format_provider_http_error(status: u16, body: &str, prefix: Option<&str>) -> String {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        // No body to surface. rs-ai has no separate SDK message, so emit the status
        // (with prefix when branded) alone.
        return match prefix {
            Some(p) => format!("{p} ({status})"),
            None => status.to_string(),
        };
    }
    let body = truncate_error_text(trimmed, MAX_PROVIDER_ERROR_BODY_CHARS);
    match prefix {
        Some(p) => format!("{p} ({status}): {body}"),
        None => format!("{status}: {body}"),
    }
}
