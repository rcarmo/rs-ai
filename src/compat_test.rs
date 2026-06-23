#[cfg(test)]
mod tests {
    use crate::compat::*;
    use crate::types::{Model, ModelCost};

    fn model_with(provider: &str, base_url: &str, id: &str) -> Model {
        Model {
            id: id.into(),
            name: "Test".into(),
            api: "openai-completions".into(),
            provider: provider.into(),
            base_url: base_url.into(),
            reasoning: false,
            thinking_level_map: None,
            input: vec!["text".into()],
            cost: ModelCost::default(),
            context_window: 128000,
            max_tokens: 4096,
            headers: None,
            api_key: None,
            compat: Default::default(),
        }
    }

    /// 0.80.2 restored runtime provider/baseUrl compat detection (detectCompat): a model
    /// with no explicit compat resolves based on its provider/URL.
    #[test]
    fn test_detect_compat_from_provider_and_url() {
        // Standard OpenAI: all defaults on, openai thinking format.
        let c = detect_compat(&model_with("openai", "https://api.openai.com/v1", "gpt-4o"));
        assert_eq!(c.thinking_format.as_deref(), Some("openai"));
        assert_eq!(c.supports_store, Some(true));
        assert_eq!(c.supports_developer_role, Some(true));
        assert_eq!(c.supports_reasoning_effort, Some(true));
        assert_eq!(c.supports_strict_mode, Some(true));
        assert_eq!(c.max_tokens_field.as_deref(), Some("max_completion_tokens"));

        // DeepSeek: non-standard, deepseek thinking format + reasoning-content flag.
        let c = detect_compat(&model_with("deepseek", "https://api.deepseek.com/v1", "deepseek-v4"));
        assert_eq!(c.thinking_format.as_deref(), Some("deepseek"));
        assert_eq!(c.supports_store, Some(false));
        assert_eq!(c.requires_reasoning_content_on_assistant_messages, Some(true));
        assert_eq!(c.supports_reasoning_effort, Some(true)); // deepseek is not in the no-effort set

        // Z.ai: non-standard, zai thinking format, no reasoning effort.
        let c = detect_compat(&model_with("zai", "https://api.z.ai/api/paas/v4", "glm"));
        assert_eq!(c.thinking_format.as_deref(), Some("zai"));
        assert_eq!(c.supports_store, Some(false));
        assert_eq!(c.supports_reasoning_effort, Some(false));

        // Together: non-standard, max_tokens field, no strict mode, no long cache.
        let c = detect_compat(&model_with("together", "https://api.together.ai/v1", "deepseek-ai/DeepSeek-R1"));
        assert_eq!(c.thinking_format.as_deref(), Some("together"));
        assert_eq!(c.max_tokens_field.as_deref(), Some("max_tokens"));
        assert_eq!(c.supports_strict_mode, Some(false));
        assert_eq!(c.supports_long_cache_retention, Some(false));

        // Nvidia: non-standard, max_tokens, no reasoning effort, no strict mode, no long cache.
        let c = detect_compat(&model_with("nvidia", "https://integrate.api.nvidia.com/v1", "nim"));
        assert_eq!(c.max_tokens_field.as_deref(), Some("max_tokens"));
        assert_eq!(c.supports_reasoning_effort, Some(false));
        assert_eq!(c.supports_strict_mode, Some(false));
        assert_eq!(c.supports_long_cache_retention, Some(false));

        // xAI/Grok: non-standard, no reasoning effort, openai thinking format.
        let c = detect_compat(&model_with("xai", "https://api.x.ai/v1", "grok-2"));
        assert_eq!(c.supports_store, Some(false));
        assert_eq!(c.supports_reasoning_effort, Some(false));
        assert_eq!(c.thinking_format.as_deref(), Some("openai"));

        // ant-ling: non-standard, ant-ling thinking format, no reasoning effort, max_tokens.
        let c = detect_compat(&model_with("ant-ling", "https://api.ant-ling.com/v1", "ling"));
        assert_eq!(c.thinking_format.as_deref(), Some("ant-ling"));
        assert_eq!(c.supports_reasoning_effort, Some(false));
        assert_eq!(c.max_tokens_field.as_deref(), Some("max_tokens"));
        assert_eq!(c.supports_long_cache_retention, Some(false));

        // OpenRouter generic model: openrouter thinking format, developer role off (non-anthropic/openai id).
        let c = detect_compat(&model_with("openrouter", "https://openrouter.ai/api/v1", "meta/llama"));
        assert_eq!(c.thinking_format.as_deref(), Some("openrouter"));
        assert_eq!(c.supports_store, Some(true)); // openrouter is not in the non-standard set
        assert_eq!(c.supports_developer_role, Some(false)); // openrouter, non-anthropic/openai id
        assert_eq!(c.cache_control_format, None);

        // OpenRouter anthropic/* model: developer role on, anthropic cache-control format.
        let c = detect_compat(&model_with("openrouter", "https://openrouter.ai/api/v1", "anthropic/claude-3-5-sonnet"));
        assert_eq!(c.supports_developer_role, Some(true));
        assert_eq!(c.cache_control_format.as_deref(), Some("anthropic"));
    }

    /// Per-model compat overlays the detected base, mirroring getCompat's
    /// `model.compat.X ?? detected.X`.
    #[test]
    fn test_model_compat_overlays_detected() {
        let mut m = model_with("openai", "https://api.openai.com/v1", "custom-model");
        m.compat.thinking_format = Some("deepseek".into());
        m.compat.supports_reasoning_effort = Some(false);
        m.compat.max_tokens_field = Some("max_tokens".into());
        m.compat.requires_reasoning_content_on_assistant_messages = Some(true);
        let c = detect_compat(&m);
        assert_eq!(c.thinking_format.as_deref(), Some("deepseek"));
        assert_eq!(c.supports_reasoning_effort, Some(false));
        assert_eq!(c.max_tokens_field.as_deref(), Some("max_tokens"));
        assert_eq!(c.requires_reasoning_content_on_assistant_messages, Some(true));
        // Untouched fields keep the detected (openai-standard) default.
        assert_eq!(c.supports_developer_role, Some(true));
        assert_eq!(c.supports_store, Some(true));
    }

    #[test]
    fn test_compat_merge() {
        let m = model_with("openai", "https://api.openai.com/v1", "gpt-4o");
        let overrides = OpenAICompletionsCompat {
            supports_temperature: Some(false),
            ..Default::default()
        };
        let c = detect_compat_for_model(&m, Some(&overrides));
        assert_eq!(c.supports_temperature, Some(false));
        assert_eq!(c.supports_developer_role, Some(true)); // base preserved
    }
}
