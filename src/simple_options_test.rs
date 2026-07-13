#[cfg(test)]
mod tests {
    use crate::simple_options::*;
    use crate::types::{Model, ModelCost, ModelThinkingLevel, ThinkingLevel};
    use std::collections::HashMap;

    fn reasoning_model(map: Option<HashMap<String, Option<String>>>) -> Model {
        Model {
            id: "test".into(),
            name: "Test".into(),
            api: "openai-completions".into(),
            provider: "openai".into(),
            base_url: "".into(),
            reasoning: true,
            thinking_level_map: map,
            input: vec!["text".into()],
            cost: ModelCost::default(),
            context_window: 128000,
            max_tokens: 16384,
            headers: None,
            api_key: None,
            compat: Default::default(),
        }
    }

    #[test]
    fn test_supported_levels_default() {
        let model = reasoning_model(None);
        let levels = get_supported_thinking_levels(&model);
        assert!(levels.contains(&ModelThinkingLevel::Off));
        assert!(levels.contains(&ModelThinkingLevel::Medium));
        assert!(!levels.contains(&ModelThinkingLevel::XHigh)); // xhigh must be explicit
    }

    #[test]
    fn test_supported_levels_with_map() {
        let map = HashMap::from([
            ("off".into(), None), // disabled
            ("low".into(), None), // disabled
            ("high".into(), Some("high".into())),
            ("xhigh".into(), Some("max".into())),
        ]);
        let model = reasoning_model(Some(map));
        let levels = get_supported_thinking_levels(&model);
        assert!(!levels.contains(&ModelThinkingLevel::Off));
        assert!(!levels.contains(&ModelThinkingLevel::Low));
        assert!(levels.contains(&ModelThinkingLevel::High));
        assert!(levels.contains(&ModelThinkingLevel::XHigh));
        assert!(levels.contains(&ModelThinkingLevel::Minimal)); // not in map = supported
    }

    #[test]
    fn test_clamp_prefers_upgrade() {
        let map = HashMap::from([
            ("off".into(), None),
            ("low".into(), None),
            ("medium".into(), None),
            ("high".into(), Some("high".into())),
        ]);
        let model = reasoning_model(Some(map));
        // Request medium (disabled). Available = [minimal, high].
        // Upstream clamps upward first -> high.
        let result = clamp_thinking_level(&model, &ModelThinkingLevel::Medium);
        assert_eq!(result, ModelThinkingLevel::High);
    }

    #[test]
    fn test_clamp_upgrades_when_no_lower() {
        let map = HashMap::from([
            ("off".into(), None),
            ("minimal".into(), None),
            ("low".into(), None),
            ("medium".into(), None),
            ("high".into(), Some("high".into())),
        ]);
        let model = reasoning_model(Some(map));
        // Only high is available → must upgrade
        let result = clamp_thinking_level(&model, &ModelThinkingLevel::Low);
        assert_eq!(result, ModelThinkingLevel::High);
    }

    #[test]
    fn test_map_thinking_level() {
        let map = HashMap::from([("high".into(), Some("custom_value".into()))]);
        let model = reasoning_model(Some(map));
        let mapped = map_thinking_level(&model, &ModelThinkingLevel::High);
        assert_eq!(mapped, Some("custom_value".into()));
    }

    #[test]
    fn test_supports_xhigh() {
        let model_no = reasoning_model(None);
        assert!(!supports_xhigh(&model_no));

        let map = HashMap::from([("xhigh".into(), Some("max".into()))]);
        let model_yes = reasoning_model(Some(map));
        assert!(supports_xhigh(&model_yes));
    }

    #[test]
    fn test_clamp_reasoning() {
        assert_eq!(clamp_reasoning(&ThinkingLevel::XHigh), ThinkingLevel::High);
        assert_eq!(
            clamp_reasoning(&ThinkingLevel::Medium),
            ThinkingLevel::Medium
        );
    }

    #[test]
    fn test_adjust_max_tokens() {
        let budgets = default_thinking_budgets();
        let (max, budget) =
            adjust_max_tokens_for_thinking(Some(4096), 16384, &ThinkingLevel::Medium, &budgets);
        assert!(max <= 16384);
        assert!(budget > 0);
        assert!(budget <= max);
    }

    #[test]
    fn test_calculate_cost() {
        let model = reasoning_model(None);
        let model = Model {
            cost: ModelCost {
                input: 3.0,
                output: 15.0,
                ..Default::default()
            },
            ..model
        };
        let usage = crate::types::Usage {
            input: 1000,
            output: 500,
            ..Default::default()
        };
        let cost = calculate_cost(&model, &usage);
        assert!((cost.input - 0.003).abs() < 0.0001);
        assert!((cost.output - 0.0075).abs() < 0.0001);
    }

    #[test]
    fn test_calculate_cost_1h_cache_write() {
        // 1h cache writes are charged at 2x base input; the remaining cacheWrite at the cacheWrite rate.
        let model = reasoning_model(None);
        let model = Model {
            cost: ModelCost {
                input: 3.0,
                output: 15.0,
                cache_write: 3.75,
                cache_read: 0.3,
                tiers: vec![],
            },
            ..model
        };
        // 1000 total cache-write tokens, of which 400 are 1h writes.
        let usage = crate::types::Usage {
            cache_write: 1000,
            cache_write_1h: Some(400),
            ..Default::default()
        };
        let cost = calculate_cost(&model, &usage);
        // short = 600 @ 3.75 + long = 400 @ (3.0*2) = (2250 + 2400)/1e6
        let expected = (3.75 * 600.0 + 3.0 * 2.0 * 400.0) / 1_000_000.0;
        assert!(
            (cost.cache_write - expected).abs() < 1e-9,
            "got {}",
            cost.cache_write
        );
        // Without the 1h split it would have been the flat 1000 @ 3.75.
        assert!((cost.cache_write - 1000.0 * 3.75 / 1_000_000.0).abs() > 1e-9);
    }

    #[test]
    fn test_map_openai_finish_reason() {
        use crate::types::StopReason;
        assert_eq!(map_openai_finish_reason("stop").0, StopReason::Stop);
        assert_eq!(map_openai_finish_reason("end").0, StopReason::Stop);
        assert_eq!(map_openai_finish_reason("length").0, StopReason::Length);
        assert_eq!(
            map_openai_finish_reason("function_call").0,
            StopReason::ToolUse
        );
        assert_eq!(
            map_openai_finish_reason("tool_calls").0,
            StopReason::ToolUse
        );
        let (r, msg) = map_openai_finish_reason("content_filter");
        assert_eq!(r, StopReason::Error);
        assert!(msg.unwrap().contains("content_filter"));
        let (r2, msg2) = map_openai_finish_reason("some_unknown");
        assert_eq!(r2, StopReason::Error);
        assert!(msg2.unwrap().contains("some_unknown"));
    }

    #[test]
    fn test_parse_openai_usage_subtracts_cache_and_computes_cost() {
        let model = reasoning_model(None);
        let model = Model {
            cost: ModelCost {
                input: 3.0,
                output: 15.0,
                cache_read: 0.3,
                cache_write: 0.0,
                tiers: vec![],
            },
            ..model
        };
        let raw = serde_json::json!({
            "prompt_tokens": 1000,
            "completion_tokens": 200,
            "prompt_tokens_details": { "cached_tokens": 400 }
        });
        let usage = parse_openai_usage(&raw, &model);
        assert_eq!(usage.cache_read, 400);
        assert_eq!(usage.input, 600); // 1000 - 400 cached
        assert_eq!(usage.output, 200);
        assert_eq!(usage.total_tokens, 1200); // 600 + 200 + 400
        // cost: input 600*3/1e6 + output 200*15/1e6 + cache_read 400*0.3/1e6
        assert!((usage.cost.input - 0.0018).abs() < 1e-6);
        assert!((usage.cost.cache_read - 0.00012).abs() < 1e-7);
    }

    #[test]
    fn test_parse_responses_usage_cache() {
        let model = reasoning_model(None);
        let raw = serde_json::json!({
            "input_tokens": 500, "output_tokens": 100, "total_tokens": 600,
            "input_tokens_details": { "cached_tokens": 200 }
        });
        let usage = parse_responses_usage(&raw, &model);
        assert_eq!(usage.cache_read, 200);
        assert_eq!(usage.input, 300); // 500 - 200
        assert_eq!(usage.output, 100);
    }

    #[test]
    fn test_apply_service_tier_pricing() {
        let mut model = reasoning_model(None);
        model.cost = ModelCost {
            input: 1.0,
            output: 2.0,
            cache_read: 0.5,
            cache_write: 0.0,
            tiers: vec![],
        };
        let base = crate::types::Usage {
            input: 1_000_000,
            output: 1_000_000,
            cache_read: 1_000_000,
            cache_write: 0,
            cache_write_1h: None,
            reasoning: None,
            total_tokens: 3_000_000,
            cost: Default::default(),
        };
        // flex halves the cost.
        let mut u = base.clone();
        u.cost = calculate_cost(&model, &u);
        let full_total = u.cost.total;
        apply_service_tier_pricing(&model, &mut u, Some("flex"));
        assert!((u.cost.total - full_total * 0.5).abs() < 1e-9);
        // priority doubles.
        let mut u = base.clone();
        u.cost = calculate_cost(&model, &u);
        apply_service_tier_pricing(&model, &mut u, Some("priority"));
        assert!((u.cost.total - full_total * 2.0).abs() < 1e-9);
        // default tier leaves cost unchanged.
        let mut u = base.clone();
        u.cost = calculate_cost(&model, &u);
        apply_service_tier_pricing(&model, &mut u, None);
        assert!((u.cost.total - full_total).abs() < 1e-9);
    }

    #[test]
    fn clamp_unknown_context_window_only_floors() {
        let mut model = reasoning_model(None);
        model.context_window = 0;
        let ctx = crate::types::Context {
            system_prompt: None,
            tools: Vec::new(),
            messages: Vec::new(),
        };
        assert_eq!(clamp_max_tokens_to_context(&model, &ctx, 5000), 5000);
        assert_eq!(clamp_max_tokens_to_context(&model, &ctx, 0), 1); // floored to MIN_MAX_TOKENS
    }

    #[test]
    fn openai_usage_captures_reasoning_tokens() {
        let model = reasoning_model(None);
        let raw = serde_json::json!({
            "prompt_tokens": 100, "completion_tokens": 50,
            "completion_tokens_details": { "reasoning_tokens": 30 }
        });
        assert_eq!(parse_openai_usage(&raw, &model).reasoning, Some(30));
        // absent details -> Some(0) (mirrors `|| 0`).
        let raw2 = serde_json::json!({ "prompt_tokens": 100, "completion_tokens": 50 });
        assert_eq!(parse_openai_usage(&raw2, &model).reasoning, Some(0));
    }

    #[test]
    fn responses_usage_captures_reasoning_tokens() {
        let model = reasoning_model(None);
        let raw = serde_json::json!({
            "input_tokens": 100, "output_tokens": 50,
            "output_tokens_details": { "reasoning_tokens": 12 }
        });
        assert_eq!(parse_responses_usage(&raw, &model).reasoning, Some(12));
        let raw2 = serde_json::json!({ "input_tokens": 100, "output_tokens": 50 });
        assert_eq!(parse_responses_usage(&raw2, &model).reasoning, Some(0));
    }

    #[test]
    fn clamp_fits_max_tokens_under_context_window() {
        let mut model = reasoning_model(None);
        let ctx = crate::types::Context {
            system_prompt: None,
            tools: Vec::new(),
            messages: Vec::new(),
        };
        // empty context -> estimate.tokens = 0, available = cw - 4096.
        model.context_window = 200000;
        assert_eq!(clamp_max_tokens_to_context(&model, &ctx, 8192), 8192); // available 195904 >= 8192
        model.context_window = 5000;
        assert_eq!(clamp_max_tokens_to_context(&model, &ctx, 8192), 904); // available 5000-4096
        model.context_window = 4096;
        assert_eq!(clamp_max_tokens_to_context(&model, &ctx, 8192), 1); // available max(1, 0)
    }
}
