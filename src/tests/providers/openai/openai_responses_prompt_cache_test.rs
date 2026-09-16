//! v0.85.1 OpenAI Responses prompt-cache serialization coverage.

#[cfg(test)]
mod tests {
    use crate::provider::responses::build_responses_payload;
    use crate::types::{
        CacheRetention, Context, Model, ModelCompat, ModelCost, StreamOptions, api, user_message,
    };
    use serde_json::json;

    fn context() -> Context {
        Context {
            system_prompt: Some("You are concise.".into()),
            messages: vec![user_message("Hello")],
            tools: vec![],
        }
    }

    fn responses_model(id: &str, compat: ModelCompat) -> Model {
        Model {
            id: id.into(),
            name: id.into(),
            api: api::OPENAI_RESPONSES.into(),
            provider: "openai".into(),
            base_url: "https://api.openai.com/v1".into(),
            reasoning: true,
            thinking_level_map: None,
            input: vec!["text".into(), "image".into()],
            cost: ModelCost::default(),
            context_window: 272000,
            max_tokens: 128000,
            sampling_params: None,
            headers: None,
            api_key: Some("test".into()),
            compat,
        }
    }

    fn payload_for(
        model: &Model,
        retention: CacheRetention,
        session_id: Option<&str>,
    ) -> serde_json::Value {
        let opts = StreamOptions {
            cache_retention: Some(retention),
            session_id: session_id.map(str::to_string),
            ..Default::default()
        };
        build_responses_payload(model, &context(), &opts)
    }

    #[test]
    fn explicit_cache_none_disables_implicit_writes_without_legacy_fields() {
        let model = responses_model(
            "gpt-6-astra",
            ModelCompat {
                supports_explicit_prompt_cache_mode: Some(true),
                supports_long_cache_retention: Some(true),
                ..Default::default()
            },
        );
        let payload = payload_for(&model, CacheRetention::None, Some("session-1"));
        assert!(payload.get("prompt_cache_key").is_none());
        assert!(payload.get("prompt_cache_retention").is_none());
        assert_eq!(payload["prompt_cache_options"], json!({"mode": "explicit"}));
    }

    #[test]
    fn explicit_cache_long_uses_ttl_without_legacy_retention() {
        let model = responses_model(
            "gpt-6-astra",
            ModelCompat {
                supports_explicit_prompt_cache_mode: Some(true),
                supports_long_cache_retention: Some(true),
                ..Default::default()
            },
        );
        let payload = payload_for(&model, CacheRetention::Long, Some("session-2"));
        assert_eq!(payload["prompt_cache_key"], json!("session-2"));
        assert!(payload.get("prompt_cache_retention").is_none());
        assert_eq!(payload["prompt_cache_options"], json!({"ttl": "30m"}));
    }

    #[test]
    fn explicit_cache_long_without_long_support_omits_ttl_and_legacy_retention() {
        let model = responses_model(
            "gpt-6-astra",
            ModelCompat {
                supports_explicit_prompt_cache_mode: Some(true),
                supports_long_cache_retention: Some(false),
                ..Default::default()
            },
        );
        let payload = payload_for(&model, CacheRetention::Long, Some("session-3"));
        assert_eq!(payload["prompt_cache_key"], json!("session-3"));
        assert!(payload.get("prompt_cache_retention").is_none());
        assert!(payload.get("prompt_cache_options").is_none());
    }

    #[test]
    fn explicit_cache_short_uses_key_without_options_or_retention() {
        let model = responses_model(
            "gpt-6-astra",
            ModelCompat {
                supports_explicit_prompt_cache_mode: Some(true),
                supports_long_cache_retention: Some(true),
                ..Default::default()
            },
        );
        let payload = payload_for(&model, CacheRetention::Short, Some("session-4"));
        assert_eq!(payload["prompt_cache_key"], json!("session-4"));
        assert!(payload.get("prompt_cache_retention").is_none());
        assert!(payload.get("prompt_cache_options").is_none());
    }

    #[test]
    fn legacy_long_models_retain_prompt_cache_retention_without_options() {
        let model = responses_model(
            "gpt-4o-mini",
            ModelCompat {
                supports_long_cache_retention: Some(true),
                ..Default::default()
            },
        );
        let payload = payload_for(&model, CacheRetention::Long, Some("session-5"));
        assert_eq!(payload["prompt_cache_key"], json!("session-5"));
        assert_eq!(payload["prompt_cache_retention"], json!("24h"));
        assert!(payload.get("prompt_cache_options").is_none());
    }

    #[test]
    fn models_that_reject_explicit_options_do_not_emit_them_for_none() {
        let model = responses_model("gpt-4o-mini", ModelCompat::default());
        let payload = payload_for(&model, CacheRetention::None, Some("session-6"));
        assert!(payload.get("prompt_cache_key").is_none());
        assert!(payload.get("prompt_cache_retention").is_none());
        assert!(payload.get("prompt_cache_options").is_none());
    }

    #[test]
    fn explicit_long_ttl_does_not_require_a_prompt_cache_key() {
        let model = responses_model(
            "gpt-6-astra",
            ModelCompat {
                supports_explicit_prompt_cache_mode: Some(true),
                supports_long_cache_retention: Some(true),
                ..Default::default()
            },
        );
        let payload = payload_for(&model, CacheRetention::Long, None);
        assert!(payload.get("prompt_cache_key").is_none());
        assert!(payload.get("prompt_cache_retention").is_none());
        assert_eq!(payload["prompt_cache_options"], json!({"ttl": "30m"}));
    }
}
