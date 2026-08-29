//! Test-for-test port of upstream `test/overflow.test.ts`
//! (`@earendil-works/pi-ai` v0.80.2): `isContextOverflow`.
//!
//! rs-ai's `is_context_overflow(msg, model)` takes the model (for its context
//! window) rather than a bare integer, so each case builds a model with the
//! given window.

#[cfg(test)]
mod tests {
    use crate::context::is_context_overflow;
    use crate::types::{Message, Model, ModelCost, Role, StopReason, Usage};

    fn model(context_window: u32) -> Model {
        Model {
            id: "m".into(),
            name: "m".into(),
            api: "openai-completions".into(),
            provider: "p".into(),
            base_url: "https://x".into(),
            reasoning: false,
            thinking_level_map: None,
            input: vec!["text".into()],
            cost: ModelCost::default(),
            context_window,
            max_tokens: 4096,
            sampling_params: None,
            headers: None,
            api_key: None,
            compat: Default::default(),
        }
    }

    fn error_message(err: &str) -> Message {
        Message {
            role: Role::Assistant,
            content: Vec::new(),
            timestamp: 0,
            api: Some("openai-completions".into()),
            provider: Some("ollama".into()),
            model: Some("qwen3.5:35b".into()),
            response_id: None,
            response_model: None,
            diagnostics: Vec::new(),
            usage: Some(Usage::default()),
            stop_reason: Some(StopReason::Error),
            deferred: None,
            error_message: Some(err.into()),
            raw_stop_reason: None,
            end_turn: None,
            tool_call_id: None,
            tool_name: None,
            is_error: false,
            details: None,
            added_tool_names: Vec::new(),
        }
    }

    fn length_message(input: u32, cache_read: u32, output: u32) -> Message {
        Message {
            role: Role::Assistant,
            content: Vec::new(),
            timestamp: 0,
            api: Some("openai-completions".into()),
            provider: Some("xiaomi".into()),
            model: Some("mimo-v2.5-pro".into()),
            response_id: None,
            response_model: None,
            diagnostics: Vec::new(),
            usage: Some(Usage {
                input,
                output,
                cache_read,
                cache_write: 0,
                cache_write_1h: None,
                reasoning: None,
                total_tokens: input + cache_read + output,
                cost: Default::default(),
            }),
            stop_reason: Some(StopReason::Length),
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

    fn overflow(msg: &Message, ctx: u32) -> bool {
        is_context_overflow(msg, &model(ctx))
    }

    #[test]
    fn detects_explicit_ollama_prompt_too_long() {
        assert!(overflow(
            &error_message("400 `prompt too long; exceeded max context length by 100918 tokens`"),
            32768
        ));
    }

    #[test]
    fn detects_together_ai_context_length_errors() {
        assert!(overflow(
            &error_message(
                "400 The input (516368 tokens) is longer than the model's context length (262144 tokens)."
            ),
            262144
        ));
    }

    #[test]
    fn detects_litellm_wrapped_openai_maximum_context_length() {
        assert!(overflow(
            &error_message(
                "Error: 503 litellm.ServiceUnavailableError: litellm.MidStreamFallbackError: litellm.APIConnectionError: APIConnectionError: OpenAIException - Requested token count exceeds the model's maximum context length of 131072 tokens."
            ),
            131072
        ));
    }

    #[test]
    fn detects_openai_compatible_parenthesized_maximum_context_length() {
        assert!(overflow(
            &error_message(
                "Error: 400 Input length (265330) exceeds model's maximum context length (262144)."
            ),
            262144
        ));
    }

    #[test]
    fn detects_openrouter_poolside_maximum_allowed_input_length() {
        assert!(overflow(
            &error_message(
                "Provider returned error: Input length 131393 exceeds the maximum allowed input length of 131040 tokens."
            ),
            131072
        ));
    }

    #[test]
    fn ignores_generic_non_overflow_ollama_errors() {
        assert!(!overflow(
            &error_message("500 `model runner crashed unexpectedly`"),
            32768
        ));
    }

    #[test]
    fn ignores_bedrock_throttling_too_many_tokens() {
        assert!(!overflow(
            &error_message("Throttling error: Too many tokens, please wait before trying again."),
            200000
        ));
    }

    #[test]
    fn ignores_bedrock_service_unavailable() {
        assert!(!overflow(
            &error_message("Service unavailable: The service is temporarily unavailable."),
            200000
        ));
    }

    #[test]
    fn ignores_generic_rate_limit_errors() {
        assert!(!overflow(
            &error_message("Rate limit exceeded, please retry after 30 seconds."),
            200000
        ));
    }

    #[test]
    fn ignores_http_429_style_errors() {
        assert!(!overflow(
            &error_message("Too many requests. Please slow down."),
            200000
        ));
    }

    #[test]
    fn detects_xiaomi_style_length_stop_with_zero_output_and_filled_context() {
        assert!(overflow(&length_message(58, 1048512, 0), 1048576));
    }

    #[test]
    fn ignores_normal_length_stops_with_output() {
        assert!(!overflow(&length_message(1000, 0, 4096), 200000));
    }

    #[test]
    fn ignores_length_stops_far_below_context() {
        assert!(!overflow(&length_message(100, 0, 0), 200000));
    }

    #[test]
    fn detects_ds4_configured_context_size_errors() {
        // v0.80.5: DS4 server phrasing, with and without thousands separators.
        assert!(overflow(
            &error_message(
                "400 Prompt has 256468 tokens, but the configured context size is 256000 tokens"
            ),
            256000,
        ));
        assert!(overflow(
            &error_message(
                "Prompt has 5,958,968 tokens, but the configured context size is 256,000 tokens"
            ),
            256000,
        ));
    }
}
