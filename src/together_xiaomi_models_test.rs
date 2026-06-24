//! Test-for-test ports of upstream `test/together-models.test.ts` and
//! `test/xiaomi-models.test.ts` (`@earendil-works/pi-ai` v0.80.2) — catalog
//! metadata + env-key resolution.

#[cfg(test)]
mod tests {
    use crate::env::get_env_api_key;
    use crate::registry::{get_model, list_models};
    use std::collections::HashMap;

    fn tlm(pairs: &[(&str, Option<&str>)]) -> HashMap<String, Option<String>> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.map(String::from))).collect()
    }

    #[test]
    fn together_registers_default_kimi_k2_6_via_openai_completions() {
        let m = get_model("together", "moonshotai/Kimi-K2.6").expect("together kimi");
        assert_eq!(m.api, "openai-completions");
        assert_eq!(m.provider, "together");
        assert_eq!(m.base_url, "https://api.together.ai/v1");
        assert!(m.reasoning);
        assert_eq!(m.thinking_level_map, Some(tlm(&[("minimal", None), ("low", None), ("medium", None)])));
        assert_eq!(m.input, vec!["text".to_string(), "image".to_string()]);
        assert_eq!(m.context_window, 262144);
        assert_eq!(m.max_tokens, 131000);
        assert_eq!(m.cost.input, 1.2);
        assert_eq!(m.cost.output, 4.5);
        assert_eq!(m.cost.cache_read, 0.2);
        assert_eq!(m.cost.cache_write, 0.0);
        assert_eq!(m.compat.supports_developer_role, Some(false));
        assert_eq!(m.compat.supports_reasoning_effort, Some(false));
        assert_eq!(m.compat.max_tokens_field.as_deref(), Some("max_tokens"));
        assert_eq!(m.compat.thinking_format.as_deref(), Some("together"));
        assert_eq!(m.compat.supports_strict_mode, Some(false));
        assert_eq!(m.compat.supports_long_cache_retention, Some(false));
    }

    #[test]
    fn together_models_reasoning_controls() {
        let gpt_oss = get_model("together", "openai/gpt-oss-120b").unwrap();
        assert_eq!(gpt_oss.thinking_level_map, Some(tlm(&[("off", None), ("minimal", None)])));
        assert_eq!(gpt_oss.compat.supports_reasoning_effort, Some(true));
        assert_eq!(gpt_oss.compat.thinking_format.as_deref(), Some("openai"));

        let deepseek = get_model("together", "deepseek-ai/DeepSeek-V4-Pro").unwrap();
        assert_eq!(deepseek.thinking_level_map, Some(tlm(&[
            ("minimal", None), ("low", None), ("medium", None), ("high", Some("high")), ("xhigh", None),
        ])));
        assert_eq!(deepseek.compat.supports_reasoning_effort, Some(true));
        assert_eq!(deepseek.compat.thinking_format.as_deref(), Some("together"));

        let minimax = get_model("together", "MiniMaxAI/MiniMax-M2.7").unwrap();
        assert_eq!(minimax.thinking_level_map, Some(tlm(&[("off", None), ("minimal", None), ("low", None), ("medium", None)])));
        assert!(minimax.compat.thinking_format.is_none());
        assert_eq!(minimax.compat.supports_reasoning_effort, Some(false));
    }

    #[test]
    fn together_resolves_api_key_from_env() {
        unsafe { std::env::set_var("TOGETHER_API_KEY", "test-together-key"); }
        let got = get_env_api_key("together");
        unsafe { std::env::remove_var("TOGETHER_API_KEY"); }
        assert_eq!(got.as_deref(), Some("test-together-key"));
    }

    #[test]
    fn xiaomi_keeps_mimo_v2_flash_on_the_api_billing_provider() {
        assert!(get_model("xiaomi", "mimo-v2-flash").is_some());
    }

    #[test]
    fn xiaomi_omits_mimo_v2_flash_from_token_plan_providers() {
        for provider in ["xiaomi-token-plan-cn", "xiaomi-token-plan-ams", "xiaomi-token-plan-sgp"] {
            let has = list_models(Some(provider)).iter().any(|m| m.id == "mimo-v2-flash");
            assert!(!has, "{provider} must omit mimo-v2-flash");
        }
    }
}
