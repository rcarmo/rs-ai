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
    fn meta_provider_and_v0870_catalog_counts_are_present() {
        let meta = crate::registry::get_model(provider_id::META, "muse-spark-1.3")
            .expect("meta/muse-spark-1.3");
        assert_eq!(meta.api, crate::types::api::OPENAI_RESPONSES);
        assert_eq!(meta.base_url, "https://api.meta.ai/v1");
        assert_eq!(meta.context_window, 1_048_576);
        assert_eq!(meta.max_tokens, 131_072);

        let models = crate::models_generated::builtin_models();
        let providers = models
            .iter()
            .map(|m| m.provider.as_str())
            .collect::<std::collections::HashSet<_>>();
        let apis = models
            .iter()
            .map(|m| m.api.as_str())
            .collect::<std::collections::HashSet<_>>();
        assert_eq!(models.len(), 1445);
        assert_eq!(providers.len(), 41);
        assert_eq!(apis.len(), 10);
        assert_eq!(
            crate::images::models_generated::builtin_image_models().len(),
            54
        );
    }
}
