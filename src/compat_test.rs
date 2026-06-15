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

    #[test]
    fn test_openai_defaults() {
        let m = model_with("openai", "https://api.openai.com/v1", "gpt-4o");
        let c = detect_compat(&m);
        assert_eq!(c.supports_developer_role, Some(true));
        assert_eq!(c.max_tokens_field.as_deref(), Some("max_completion_tokens"));
    }

    #[test]
    fn test_ollama_detection() {
        let m = model_with("ollama", "http://localhost:11434/v1", "llama3");
        let c = detect_compat(&m);
        assert_eq!(c.supports_strict_mode, Some(false));
        assert_eq!(c.requires_tool_result_name, Some(true));
        assert_eq!(c.max_tokens_field.as_deref(), Some("max_tokens"));
    }

    #[test]
    fn test_remote_11434_not_ollama() {
        let m = model_with("custom", "https://example.com:11434/v1", "model");
        let c = detect_compat(&m);
        assert_eq!(c.max_tokens_field.as_deref(), Some("max_completion_tokens"));
    }

    #[test]
    fn test_openrouter_developer_role() {
        let m = model_with("openrouter", "https://openrouter.ai/api/v1", "meta/llama");
        let c = detect_compat(&m);
        assert_eq!(c.supports_developer_role, Some(false)); // non-anthropic/openai prefix

        let m2 = model_with("openrouter", "https://openrouter.ai/api/v1", "anthropic/claude");
        let c2 = detect_compat(&m2);
        assert_eq!(c2.supports_developer_role, Some(true)); // anthropic prefix
    }

    #[test]
    fn test_deepseek_thinking() {
        let m = model_with("deepseek", "https://api.deepseek.com/v1", "deepseek-v4");
        let c = detect_compat(&m);
        assert_eq!(c.thinking_format.as_deref(), Some("deepseek"));
        assert_eq!(c.requires_reasoning_content_on_assistant_messages, Some(true));
    }

    #[test]
    fn test_zai_detection_precise() {
        // Standard z.ai endpoint -> zai format.
        let m = model_with("zai", "https://api.z.ai/api/paas/v4", "glm");
        assert_eq!(detect_compat(&m).thinking_format.as_deref(), Some("zai"));
        // An unrelated domain that merely contains the substring "z.ai" must NOT match
        // (upstream uses "api.z.ai", not a broad "z.ai" contains).
        let m2 = model_with("custom", "https://xyz.ai/v1", "m");
        assert_eq!(detect_compat(&m2).thinking_format.as_deref(), Some("openai"));
        assert_eq!(detect_compat(&m2).supports_store, Some(true));
    }

    #[test]
    fn test_xiaomi_not_auto_detected_as_deepseek() {
        // Upstream does NOT detect xiaomi as deepseek; xiaomi models carry an explicit
        // compat (thinkingFormat/requiresReasoningContent). A bare xiaomi model must be
        // treated as standard (store + developer role enabled, openai thinking format).
        let m = model_with("xiaomi", "https://api.xiaomimimo.com/v1", "mimo");
        let c = detect_compat(&m);
        assert_eq!(c.thinking_format.as_deref(), Some("openai"));
        assert_eq!(c.supports_store, Some(true));
        assert_eq!(c.supports_developer_role, Some(true));
        assert_eq!(c.requires_reasoning_content_on_assistant_messages, None);

        // The registry's explicit compat still yields the deepseek format via the override.
        let mut m2 = model_with("xiaomi", "https://api.xiaomimimo.com/v1", "mimo");
        m2.compat.thinking_format = Some("deepseek".into());
        m2.compat.requires_reasoning_content_on_assistant_messages = Some(true);
        let c2 = detect_compat(&m2);
        assert_eq!(c2.thinking_format.as_deref(), Some("deepseek"));
        assert_eq!(c2.requires_reasoning_content_on_assistant_messages, Some(true));
        // Still standard (explicit format compat doesn't make it non-standard).
        assert_eq!(c2.supports_store, Some(true));
    }

    #[test]
    fn test_ant_ling() {
        let m = model_with("ant-ling", "https://api.ant-ling.com/v1", "ling");
        let c = detect_compat(&m);
        assert_eq!(c.thinking_format.as_deref(), Some("ant-ling"));
        assert_eq!(c.max_tokens_field.as_deref(), Some("max_tokens"));
    }

    #[test]
    fn test_nvidia() {
        let m = model_with("nvidia", "https://integrate.api.nvidia.com/v1", "nim");
        let c = detect_compat(&m);
        assert_eq!(c.supports_store, Some(false));
        assert_eq!(c.max_tokens_field.as_deref(), Some("max_tokens"));
        assert_eq!(c.supports_reasoning_effort, Some(false));
        assert_eq!(c.supports_strict_mode, Some(false));
        assert_eq!(c.supports_long_cache_retention, Some(false));
    }

    #[test]
    fn test_grok_no_reasoning_effort() {
        let m = model_with("xai", "https://api.x.ai/v1", "grok-2");
        let c = detect_compat(&m);
        assert_eq!(c.supports_reasoning_effort, Some(false));
    }

    #[test]
    fn test_together_detection() {
        let m = model_with("together", "https://api.together.ai/v1", "deepseek-ai/DeepSeek-R1");
        let c = detect_compat(&m);
        assert_eq!(c.thinking_format.as_deref(), Some("together"));
        assert_eq!(c.supports_reasoning_effort, Some(false));
        assert_eq!(c.supports_strict_mode, Some(false));
        assert_eq!(c.supports_long_cache_retention, Some(false));
        assert_eq!(c.max_tokens_field.as_deref(), Some("max_tokens"));
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

    #[test]
    fn test_model_compat_overrides_detection() {
        // A model declaring compat flags must override URL/provider detection,
        // mirroring upstream getCompat overlaying model.compat onto detected defaults.
        let mut m = model_with("openai", "https://api.openai.com/v1", "gpt-4o");
        m.compat.max_tokens_field = Some("max_tokens".into());
        m.compat.supports_reasoning_effort = Some(false);
        m.compat.thinking_format = Some("deepseek".into());
        let c = detect_compat(&m);
        assert_eq!(c.max_tokens_field.as_deref(), Some("max_tokens"));
        assert_eq!(c.supports_reasoning_effort, Some(false));
        assert_eq!(c.thinking_format.as_deref(), Some("deepseek"));
        // Untouched flag still comes from detection.
        assert_eq!(c.supports_developer_role, Some(true));
    }
}
