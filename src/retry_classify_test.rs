//! Tests for `retry::is_retryable_assistant_error` (port of upstream
//! `utils/retry.ts` classification behavior, v0.80.3).

#[cfg(test)]
mod tests {
    use crate::retry::is_retryable_assistant_error;
    use crate::types::{Message, Role, StopReason};

    fn err_msg(error: Option<&str>, stop: Option<StopReason>) -> Message {
        Message {
            role: Role::Assistant,
            content: Vec::new(),
            timestamp: 0,
            api: None,
            provider: None,
            model: None,
            response_id: None,
            response_model: None,
            diagnostics: Vec::new(),
            usage: None,
            stop_reason: stop,
            error_message: error.map(|s| s.to_string()),
            tool_call_id: None,
            tool_name: None,
            is_error: false,
            details: None,
        }
    }

    #[test]
    fn requires_error_stop_reason_and_message() {
        // Not an error stop -> false even with retryable text.
        assert!(!is_retryable_assistant_error(&err_msg(Some("overloaded"), Some(StopReason::Stop))));
        // Error stop but no message -> false.
        assert!(!is_retryable_assistant_error(&err_msg(None, Some(StopReason::Error))));
    }

    #[test]
    fn retryable_transient_provider_errors() {
        for text in [
            "Overloaded",                       // case-insensitive
            "429 Too Many Requests",
            "rate limit exceeded",
            "rate-limit hit",
            "ratelimit",
            "503 Service Unavailable",
            "internal server error",
            "Provider returned error",
            "fetch failed",
            "upstream connect error",
            "socket hang up",
            "Request timed out",
            "request timeout",
            "connection refused",
            "WebSocket closed unexpectedly",
            "Anthropic stream ended before message_stop",
            "stream ended without a stop reason",
            "http2 request did not get a response",
            "you can retry your request",
        ] {
            assert!(
                is_retryable_assistant_error(&err_msg(Some(text), Some(StopReason::Error))),
                "expected retryable: {text:?}"
            );
        }
    }

    #[test]
    fn non_retryable_quota_and_billing_limits_win() {
        for text in [
            "GoUsageLimitError: monthly cap",
            "FreeUsageLimitError",
            "Monthly usage limit reached",
            "please top up your available balance",
            "insufficient_quota",
            "out of budget",
            "quota exceeded",
            "billing issue",
        ] {
            assert!(
                !is_retryable_assistant_error(&err_msg(Some(text), Some(StopReason::Error))),
                "expected non-retryable: {text:?}"
            );
        }
    }

    #[test]
    fn non_retryable_pattern_overrides_retryable_text() {
        // Contains both "429" (retryable) and "insufficient_quota" (non-retryable) ->
        // non-retryable takes precedence (checked first).
        let m = err_msg(Some("429 insufficient_quota: out of credits"), Some(StopReason::Error));
        assert!(!is_retryable_assistant_error(&m));
    }

    #[test]
    fn unrelated_error_is_not_retryable() {
        let m = err_msg(Some("invalid api key"), Some(StopReason::Error));
        assert!(!is_retryable_assistant_error(&m));
    }
}
