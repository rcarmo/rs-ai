//! Additional coverage tests matching Go test categories.

#[cfg(test)]
mod tests {
    use crate::retry::*;
    use crate::context::*;
    use crate::simple_options::*;
    use crate::compaction::*;
    use crate::logger::*;
    use crate::utils::*;
    use crate::types::*;
    use std::time::Duration;

    // --- Retry tests ---

    #[test]
    fn test_default_retry_config() {
        let cfg = default_retry_config();
        assert_eq!(cfg.max_retries, 3);
        assert_eq!(cfg.backoff_multiplier, 2.0);
        assert_eq!(cfg.max_retry_delay_ms, 60_000);
    }

    #[test]
    fn test_no_retry_config() {
        let cfg = no_retry_config();
        assert_eq!(cfg.max_retries, 0);
    }

    #[test]
    fn test_backoff_increases() {
        let cfg = default_retry_config();
        let d0 = compute_backoff(0, &cfg);
        let d1 = compute_backoff(1, &cfg);
        let d2 = compute_backoff(2, &cfg);
        assert!(d1 > d0);
        assert!(d2 > d1);
    }

    #[test]
    fn test_backoff_caps_at_max() {
        let cfg = RetryConfig { max_delay: Duration::from_secs(2), ..default_retry_config() };
        let d = compute_backoff(10, &cfg);
        assert!(d.as_secs_f64() <= 2.0);
    }

    #[test]
    fn test_retryable_status_codes() {
        assert!(is_retryable_status(429));
        assert!(is_retryable_status(500));
        assert!(is_retryable_status(502));
        assert!(is_retryable_status(503));
        assert!(is_retryable_status(504));
        assert!(!is_retryable_status(200));
        assert!(!is_retryable_status(400));
        assert!(!is_retryable_status(401));
        assert!(!is_retryable_status(404));
    }

    #[test]
    fn test_parse_retry_after_seconds() {
        assert_eq!(parse_retry_after("30"), Some(Duration::from_secs(30)));
        assert_eq!(parse_retry_after("1"), Some(Duration::from_secs(1)));
        assert_eq!(parse_retry_after("invalid"), None);
        assert_eq!(parse_retry_after(""), None);
    }

    // --- Cost/Token tests ---

    #[test]
    fn test_calculate_cost_basic() {
        let model = Model {
            cost: ModelCost { input: 3.0, output: 15.0, cache_read: 0.3, cache_write: 3.75 },
            ..test_model_base()
        };
        let usage = Usage { input: 1000, output: 500, cache_read: 200, cache_write: 50, total_tokens: 1750, ..Default::default() };
        let cost = calculate_cost(&model, &usage);
        assert!((cost.input - 0.003).abs() < 0.0001);
        assert!((cost.output - 0.0075).abs() < 0.0001);
        assert!((cost.cache_read - 0.00006).abs() < 0.00001);
        assert!(cost.total > 0.0);
    }

    #[test]
    fn test_estimate_tokens_basic() {
        let ctx = Context {
            system_prompt: Some("System prompt with some text.".into()),
            messages: vec![user_message("Hello, how are you doing today?")],
            tools: vec![],
        };
        let tokens = estimate_tokens(&ctx);
        assert!(tokens > 10);
        assert!(tokens < 100);
    }

    // --- Context overflow tests ---

    #[test]
    fn test_overflow_stop_reason_length() {
        let model = Model { context_window: 100, ..test_model_base() };
        // Length stop with output==0 and input filling the window is overflow.
        let msg = Message {
            stop_reason: Some(StopReason::Length),
            usage: Some(Usage { input: 100, output: 0, ..Default::default() }),
            ..base_msg()
        };
        assert!(is_context_overflow(&msg, &model));
    }

    #[test]
    fn test_overflow_error_message() {
        let model = test_model_base();
        let msg = Message {
            stop_reason: Some(StopReason::Error),
            error_message: Some("context_length_exceeded: reduce your input".into()),
            ..base_msg()
        };
        assert!(is_context_overflow(&msg, &model));
    }

    #[test]
    fn test_overflow_token_limit() {
        let model = Model { context_window: 100, ..test_model_base() };
        // Silent overflow: successful stop but input exceeds the context window.
        let msg = Message {
            stop_reason: Some(StopReason::Stop),
            usage: Some(Usage { input: 110, output: 0, total_tokens: 110, ..Default::default() }),
            ..base_msg()
        };
        assert!(is_context_overflow(&msg, &model));
    }

    #[test]
    fn test_no_overflow_normal() {
        let model = test_model_base();
        let msg = Message {
            stop_reason: Some(StopReason::Stop),
            ..base_msg()
        };
        assert!(!is_context_overflow(&msg, &model));
    }

    // --- Logger tests ---

    #[test]
    fn test_stderr_logger_does_not_panic() {
        let logger = new_stderr_logger(LogLevel::Warn);
        logger.log(LogLevel::Debug, "should not print", &[]);
        logger.log(LogLevel::Warn, "should print", &[("key", "val")]);
        logger.log(LogLevel::Error, "error", &[]);
    }

    #[test]
    fn test_get_logger_noop_default() {
        // Default logger is a no-op; just verify it doesn't panic
        log_debug("test", &[]);
        log_info("test", &[]);
        log_warn("test", &[]);
        log_error("test", &[]);
    }

    // --- Utils tests ---

    #[test]
    fn test_hash_deterministic() {
        assert_eq!(hash_string("hello"), hash_string("hello"));
        assert_ne!(hash_string("hello"), hash_string("world"));
    }

    #[test]
    fn test_short_hash() {
        // Exact upstream shortHash values (cyrb53-style base-36 double hash).
        assert_eq!(short_hash("test"), "1mibwv417rfy6h");
        assert_eq!(short_hash("fc_abc"), "1qaoofs1vxahbg");
        assert_eq!(short_hash(""), "k4n83c7h0j2b");
        assert_eq!(short_hash("hello world"), "n7rb4n1m39uz8");
        assert_eq!(short_hash("test"), short_hash("test"));
    }

    #[test]
    fn test_is_cloudflare_provider() {
        assert!(is_cloudflare_provider("cloudflare-workers-ai"));
        assert!(is_cloudflare_provider("cloudflare-ai-gateway"));
        assert!(!is_cloudflare_provider("openai"));
    }

    #[test]
    fn test_resolve_cloudflare_base_url_substitutes_env() {
        unsafe { std::env::set_var("RS_AI_TEST_ACCT", "acct123"); }
        let resolved = resolve_cloudflare_base_url("https://gateway.ai.cloudflare.com/v1/{RS_AI_TEST_ACCT}/openai", "cloudflare-ai-gateway");
        assert_eq!(resolved.unwrap(), "https://gateway.ai.cloudflare.com/v1/acct123/openai");
        // No placeholders -> pass-through.
        assert_eq!(resolve_cloudflare_base_url("https://example.com", "openai").unwrap(), "https://example.com");
        unsafe { std::env::remove_var("RS_AI_TEST_ACCT"); }
        // A missing/unset variable is an error (matches upstream throw).
        let err = resolve_cloudflare_base_url("https://x/{RS_AI_DEFINITELY_UNSET_VAR}/y", "cloudflare-workers-ai").unwrap_err();
        assert!(err.contains("RS_AI_DEFINITELY_UNSET_VAR is required for provider cloudflare-workers-ai"), "{err}");
    }

    #[test]
    fn test_clamp_prompt_cache_key_char_safe() {
        let long = "é".repeat(100); // multi-byte chars; must not panic
        let clamped = crate::prompt_cache::clamp_openai_prompt_cache_key(&long);
        assert_eq!(clamped.chars().count(), 64);
    }

    #[test]
    fn test_copilot_headers_structure() {
        let h = copilot_headers();
        assert!(h.contains_key("User-Agent"));
        assert!(h.contains_key("Copilot-Integration-Id"));
        let h2 = copilot_headers_with_intent("completion");
        assert_eq!(h2["openai-intent"], "completion");
    }

    #[test]
    fn test_sanitize_surrogates_noop_for_valid_utf8() {
        assert_eq!(sanitize_surrogates("Hello 🙈"), "Hello 🙈");
        assert_eq!(sanitize_surrogates("plain"), "plain");
        assert_eq!(sanitize_surrogates(""), "");
    }

    // --- Max tokens / thinking adjustment ---

    #[test]
    fn test_adjust_max_tokens_basic() {
        let budgets = default_thinking_budgets();
        // base+budget = 4096+16384 = 20480 < model cap 32000; 20480 > 16384 so budget stays.
        let (max, budget) = adjust_max_tokens_for_thinking(Some(4096), 32000, &ThinkingLevel::High, &budgets);
        assert_eq!((max, budget), (20480, 16384));
    }

    #[test]
    fn test_adjust_max_tokens_capped() {
        let budgets = default_thinking_budgets();
        // base+budget = 20480 clamped to model cap 8000; 8000 <= 16384 -> budget = 8000-1024.
        let (max, budget) = adjust_max_tokens_for_thinking(Some(4096), 8000, &ThinkingLevel::High, &budgets);
        assert_eq!((max, budget), (8000, 6976));
    }

    #[test]
    fn test_adjust_max_tokens_no_cap_uses_model_cap() {
        let budgets = default_thinking_budgets();
        // None caller cap -> model cap; 32000 > 16384 so high budget stays full.
        let (max, budget) = adjust_max_tokens_for_thinking(None, 32000, &ThinkingLevel::High, &budgets);
        assert_eq!((max, budget), (32000, 16384));
    }

    #[test]
    fn test_adjust_max_tokens_xhigh_clamps_to_high_budget() {
        let budgets = default_thinking_budgets();
        // clampReasoning(xhigh) -> high (16384), same as the basic high case.
        let (max, budget) = adjust_max_tokens_for_thinking(Some(4096), 32000, &ThinkingLevel::XHigh, &budgets);
        assert_eq!((max, budget), (20480, 16384));
    }

    // --- Helpers ---

    fn test_model_base() -> Model {
        Model {
            id: "test".into(), name: "Test".into(), api: "openai-completions".into(),
            provider: "openai".into(), base_url: "".into(), reasoning: false,
            thinking_level_map: None, input: vec!["text".into()],
            cost: ModelCost::default(), context_window: 128000, max_tokens: 4096,
            headers: None, api_key: None,
            compat: Default::default(),
        }
    }

    fn base_msg() -> Message {
        Message {
            role: Role::Assistant,
            content: vec![ContentBlock::Text { text: "hi".into(), text_signature: None }],
            timestamp: 0,
            api: None, provider: None, model: None, response_id: None,
            response_model: None,
            diagnostics: Vec::new(),
            usage: None, stop_reason: None, error_message: None,
            tool_call_id: None, tool_name: None, is_error: false,
            details: None,
        }
    }
}
