//! Release-pinned v0.87.0 catalog metadata coverage.

#[cfg(test)]
mod tests {
    use crate::types::{Model, provider_id};
    use serde_json::{Value, json};

    #[test]
    fn official_prompt_cache_and_new_compat_round_trip_unchanged() {
        let official = json!({
            "id": "claude-fable-5",
            "name": "Claude Fable 5",
            "api": "anthropic-messages",
            "provider": "anthropic",
            "baseUrl": "https://api.anthropic.com",
            "reasoning": true,
            "input": ["text", "image"],
            "inputLimits": {
                "maxBytes": 33554432,
                "images": {"maxCount": 600}
            },
            "cost": {
                "input": 10,
                "output": 50,
                "cacheRead": 1,
                "cacheWrite": 12.5,
                "tiers": [{
                    "inputTokensAbove": 200000,
                    "input": 20,
                    "output": 75,
                    "cacheRead": 2,
                    "cacheWrite": 25
                }]
            },
            "promptCache": {"short": 300, "long": 3600},
            "contextWindow": 1000000,
            "maxTokens": 128000,
            "thinkingLevelMap": {
                "off": null,
                "minimal": null,
                "low": null,
                "medium": "medium",
                "high": "high",
                "xhigh": "max",
                "max": "max"
            },
            "compat": {
                "forceAdaptiveThinking": true,
                "supportsMidConvoSystemMessages": true,
                "supportsMidConvoToolChanges": true,
                "supportsStrictTools": true,
                "allowedFallbackModels": ["anthropic/claude-opus-4-8", "anthropic/claude-opus-5"]
            }
        });

        let model: Model = serde_json::from_value(official.clone()).unwrap();
        assert_eq!(
            model.prompt_cache,
            Some(json!({"short": 300, "long": 3600}))
        );
        assert_eq!(
            model.input_limits,
            Some(json!({"maxBytes": 33554432, "images": {"maxCount": 600}}))
        );
        assert_eq!(model.compat.supports_mid_convo_system_messages, Some(true));
        assert_eq!(model.compat.supports_mid_convo_tool_changes, Some(true));
        assert_eq!(model.compat.supports_strict_tools, Some(true));

        let encoded = serde_json::to_value(&model).unwrap();
        for key in ["inputLimits", "promptCache", "thinkingLevelMap", "compat"] {
            assert_eq!(encoded.get(key), official.get(key), "field {key}");
        }
        assert_eq!(encoded["provider"], Value::String("anthropic".into()));
    }

    #[test]
    fn vision_models_preserve_default_resize_and_direct_provider_limits() {
        let default_resize = json!({
            "maxWidth": 2000,
            "maxHeight": 2000,
            "maxBytes": 4_718_592,
            "jpegQuality": 80,
        });
        for model in crate::models_generated::builtin_models()
            .into_iter()
            .filter(|model| model.input.iter().any(|input| input == "image"))
        {
            assert_eq!(
                model
                    .input_limits
                    .as_ref()
                    .and_then(|limits| limits.pointer("/images/resize")),
                Some(&default_resize),
                "{}/{}",
                model.provider,
                model.id
            );
        }

        let limits = |provider: &str, id: &str| {
            crate::registry::get_model(provider, id)
                .unwrap_or_else(|| panic!("missing {provider}/{id}"))
                .input_limits
                .unwrap()
        };
        let haiku = limits("anthropic", "claude-haiku-4-5");
        assert_eq!(haiku["maxRequestBytes"], 32 * 1024 * 1024);
        assert_eq!(haiku["images"]["maxPerRequest"], 100);
        assert_eq!(
            limits("anthropic", "claude-opus-5")["images"]["maxPerRequest"],
            600
        );
        assert_eq!(
            limits("amazon-bedrock", "anthropic.claude-haiku-4-5-20251001-v1:0")["images"]["maxPerMessage"],
            20
        );
        let openai = limits("openai", "gpt-4o");
        assert_eq!(openai["maxRequestBytes"], 512 * 1024 * 1024);
        assert_eq!(openai["images"]["maxPerRequest"], 1500);
        let google = limits("google", "gemini-2.5-flash");
        assert_eq!(google["maxRequestBytes"], 20 * 1024 * 1024);
        assert_eq!(google["images"]["maxPerRequest"], 3600);
        assert_eq!(
            limits("openrouter", "openai/gpt-4o"),
            json!({"images": {"resize": default_resize}})
        );
    }

    #[test]
    fn meta_provider_and_v0870_catalog_counts_are_present() {
        let meta = crate::registry::get_model(provider_id::META, "muse-spark-1.3")
            .expect("meta/muse-spark-1.3");
        assert_eq!(meta.api, crate::types::api::OPENAI_RESPONSES);
        assert_eq!(meta.base_url, "https://api.meta.ai/v1");
        assert_eq!(meta.context_window, 1_048_576);
        assert_eq!(meta.max_tokens, 131_072);
        assert_eq!(
            crate::env::api_key_env_vars(provider_id::META),
            Some(&["META_API_KEY"][..])
        );
        let runtime = crate::models_runtime::ModelsRuntime::new();
        runtime.populate_builtin_fallbacks();
        assert!(runtime.provider_has_oauth(provider_id::META));

        let models = crate::models_generated::builtin_models();
        let providers = models
            .iter()
            .map(|m| m.provider.as_str())
            .collect::<std::collections::HashSet<_>>();
        let apis = models
            .iter()
            .map(|m| m.api.as_str())
            .collect::<std::collections::HashSet<_>>();
        assert_eq!(models.len(), 1495);
        assert_eq!(providers.len(), 41);
        assert_eq!(apis.len(), 10);
        assert_eq!(
            crate::images::models_generated::builtin_image_models().len(),
            55
        );
    }

    #[test]
    fn radius_default_gateway_has_public_baseline_and_custom_gateway_does_not() {
        crate::registry::register_builtin_models();
        let default = crate::registry::radius_baseline_models(crate::oauth::DEFAULT_RADIUS_GATEWAY);
        assert!(default.iter().any(|model| model.id == "balanced"));
        assert!(!default.is_empty());
        assert!(crate::registry::radius_baseline_models("http://localhost:8788").is_empty());
    }
}
