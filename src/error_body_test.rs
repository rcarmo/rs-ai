//! Tests for `error_body` provider HTTP error formatting (port of upstream
//! `utils/error-body.ts` display behavior, v0.80.3).

#[cfg(test)]
mod tests {
    use crate::error_body::{
        format_provider_http_error, truncate_error_text, MAX_PROVIDER_ERROR_BODY_CHARS,
    };

    #[test]
    fn formats_status_and_body_without_prefix() {
        assert_eq!(
            format_provider_http_error(403, "{\"error\":\"forbidden\"}", None),
            "403: {\"error\":\"forbidden\"}"
        );
    }

    #[test]
    fn formats_branded_prefix_for_responses_and_azure() {
        assert_eq!(
            format_provider_http_error(429, "rate limited", Some("OpenAI API error")),
            "OpenAI API error (429): rate limited"
        );
        assert_eq!(
            format_provider_http_error(500, "boom", Some("Azure OpenAI API error")),
            "Azure OpenAI API error (500): boom"
        );
    }

    #[test]
    fn trims_body_and_handles_empty() {
        assert_eq!(format_provider_http_error(503, "   spaced   ", None), "503: spaced");
        // Empty/whitespace-only body collapses to status (with prefix when branded).
        assert_eq!(format_provider_http_error(503, "   ", None), "503");
        assert_eq!(
            format_provider_http_error(503, "", Some("OpenAI API error")),
            "OpenAI API error (503)"
        );
    }

    #[test]
    fn truncates_overlong_bodies_to_cap() {
        let body = "x".repeat(MAX_PROVIDER_ERROR_BODY_CHARS + 25);
        let out = format_provider_http_error(400, &body, None);
        let expected_body = format!(
            "{}... [truncated 25 chars]",
            "x".repeat(MAX_PROVIDER_ERROR_BODY_CHARS)
        );
        assert_eq!(out, format!("400: {expected_body}"));
    }

    #[test]
    fn truncate_helper_is_noop_under_cap() {
        assert_eq!(truncate_error_text("short", 4000), "short");
        assert_eq!(truncate_error_text("abcdef", 3), "abc... [truncated 3 chars]");
    }
}
