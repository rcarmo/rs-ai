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

    /// 0.80.x removed runtime URL/provider compat detection: a model with no explicit
    /// compat resolves to the OpenAI-standard DEFAULT_COMPAT regardless of provider/URL.
    #[test]
    fn test_default_compat_ignores_provider_and_url() {
        for (provider, url, id) in [
            ("openai", "https://api.openai.com/v1", "gpt-4o"),
            ("deepseek", "https://api.deepseek.com/v1", "deepseek-v4"),
            ("zai", "https://api.z.ai/api/paas/v4", "glm"),
            ("ollama", "http://localhost:11434/v1", "llama3"),
            ("nvidia", "https://integrate.api.nvidia.com/v1", "nim"),
            ("together", "https://api.together.ai/v1", "deepseek-ai/DeepSeek-R1"),
            ("xai", "https://api.x.ai/v1", "grok-2"),
            ("ant-ling", "https://api.ant-ling.com/v1", "ling"),
            ("openrouter", "https://openrouter.ai/api/v1", "meta/llama"),
        ] {
            let c = detect_compat(&model_with(provider, url, id));
            assert_eq!(c.thinking_format.as_deref(), Some("openai"), "{provider} thinking_format");
            assert_eq!(c.supports_store, Some(true), "{provider} supports_store");
            assert_eq!(c.supports_developer_role, Some(true), "{provider} supports_developer_role");
            assert_eq!(c.supports_reasoning_effort, Some(true), "{provider} supports_reasoning_effort");
            assert_eq!(c.supports_strict_mode, Some(true), "{provider} supports_strict_mode");
            assert_eq!(c.supports_long_cache_retention, Some(true), "{provider} supports_long_cache_retention");
            assert_eq!(c.max_tokens_field.as_deref(), Some("max_completion_tokens"), "{provider} max_tokens_field");
        }
    }

    /// Per-model compat (baked into the catalog) overlays DEFAULT_COMPAT, mirroring
    /// getCompat's `model.compat.X ?? DEFAULT_COMPAT.X`.
    #[test]
    fn test_model_compat_overlays_default() {
        let mut m = model_with("deepseek", "https://api.deepseek.com/v1", "deepseek-chat");
        m.compat.thinking_format = Some("deepseek".into());
        m.compat.supports_reasoning_effort = Some(false);
        m.compat.max_tokens_field = Some("max_tokens".into());
        m.compat.requires_reasoning_content_on_assistant_messages = Some(true);
        let c = detect_compat(&m);
        assert_eq!(c.thinking_format.as_deref(), Some("deepseek"));
        assert_eq!(c.supports_reasoning_effort, Some(false));
        assert_eq!(c.max_tokens_field.as_deref(), Some("max_tokens"));
        assert_eq!(c.requires_reasoning_content_on_assistant_messages, Some(true));
        // Untouched fields keep the OpenAI-standard default.
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
