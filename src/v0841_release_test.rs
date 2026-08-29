use crate::provider::openai::build_payload;
use crate::types::{Context, StreamOptions, ThinkingLevel};
use serde_json::json;
use std::collections::HashSet;

fn user_context() -> Context {
    Context {
        system_prompt: None,
        messages: vec![crate::types::user_message("Hi")],
        tools: vec![],
    }
}

const INDIVIDUAL_MODELS: &[&str] = &[
    "deepseek-v4-flash-0731",
    "deepseek-v4-pro",
    "deepseek-v4-pro-0813",
    "glm-5.2",
    "qwen3.6-flash",
    "qwen3.7-max",
    "qwen3.7-plus",
    "qwen3.8-max",
];

#[test]
fn release_pinned_catalog_counts_include_individual_and_batch_aliases() {
    let all = crate::models_generated::builtin_models();
    let pairs = all
        .iter()
        .map(|model| (model.provider.as_str(), model.id.as_str()))
        .collect::<HashSet<_>>();
    let providers = all
        .iter()
        .map(|model| model.provider.as_str())
        .collect::<HashSet<_>>();
    let apis = all
        .iter()
        .map(|model| model.api.as_str())
        .collect::<HashSet<_>>();

    assert_eq!(pairs.len(), 1290);
    assert_eq!(providers.len(), 39);
    assert_eq!(apis.len(), 9);

    let batch_aliases = all
        .iter()
        .filter(|model| model.id.contains(":batch"))
        .map(|model| format!("{}/{}", model.provider, model.id))
        .collect::<HashSet<_>>();
    assert_eq!(batch_aliases.len(), 40);
    assert!(batch_aliases.contains("openrouter/anthropic/claude-opus-5:batch"));
    assert!(batch_aliases.contains("openrouter/deepseek/deepseek-v4-pro-0813:batch"));
    assert!(batch_aliases.contains("openrouter/z-ai/glm-5.3-flash:batch"));

    let image_pairs = crate::images::list_image_models(None)
        .into_iter()
        .map(|model| (model.provider, model.id))
        .collect::<HashSet<_>>();
    assert_eq!(image_pairs.len(), 50);
}

#[test]
fn qwen_token_plan_individual_catalog_env_and_endpoint_match_v0841() {
    let providers = crate::registry::list_providers();
    assert!(providers.contains(&"qwen-token-plan-individual".to_string()));
    assert_eq!(
        crate::env::api_key_env_vars("qwen-token-plan-individual"),
        Some(&["QWEN_TOKEN_PLAN_API_KEY"][..])
    );

    let actual = crate::registry::list_models(Some("qwen-token-plan-individual"))
        .into_iter()
        .map(|model| model.id)
        .collect::<HashSet<_>>();
    let expected = INDIVIDUAL_MODELS
        .iter()
        .map(|id| (*id).to_string())
        .collect::<HashSet<_>>();
    assert_eq!(actual, expected);
    assert!(!actual.contains("qwen3.8-max-preview"));

    for model_id in INDIVIDUAL_MODELS {
        let model = crate::registry::get_model("qwen-token-plan-individual", model_id).unwrap();
        assert_eq!(model.api, crate::types::api::OPENAI_COMPLETIONS);
        assert_eq!(
            model.base_url,
            "https://token-plan.ap-southeast-1.maas.aliyuncs.com/compatible-mode/v1"
        );
        assert_eq!(model.compat.thinking_format.as_deref(), Some("qwen"));
        assert_eq!(model.compat.supports_developer_role, Some(false));
        assert_eq!(model.compat.supports_store, Some(false));
        assert!(model.reasoning);
    }
}

#[test]
fn qwen_token_plan_individual_reasoning_payloads_match_v0841() {
    for model_id in ["deepseek-v4-flash-0731", "deepseek-v4-pro", "glm-5.2"] {
        let model = crate::registry::get_model("qwen-token-plan-individual", model_id).unwrap();
        assert_eq!(model.compat.supports_reasoning_effort, Some(true));
        let levels = model.thinking_level_map.as_ref().unwrap();
        assert_eq!(levels.get("high").and_then(|v| v.as_deref()), Some("high"));
        assert_eq!(levels.get("max").and_then(|v| v.as_deref()), Some("max"));
        assert!(matches!(levels.get("low"), Some(None)));
        assert!(matches!(levels.get("medium"), Some(None)));
        assert!(matches!(levels.get("xhigh"), Some(None)));

        let payload = build_payload(
            &model,
            &user_context(),
            &StreamOptions {
                reasoning: Some(ThinkingLevel::High),
                ..Default::default()
            },
            &crate::compat::detect_compat(&model),
        );
        assert_eq!(payload["enable_thinking"], json!(true));
        assert_eq!(payload["reasoning_effort"], json!("high"));
        assert!(payload.get("thinking").is_none());
    }

    for model_id in ["qwen3.6-flash", "qwen3.7-max", "qwen3.7-plus"] {
        let model = crate::registry::get_model("qwen-token-plan-individual", model_id).unwrap();
        assert_eq!(model.compat.supports_reasoning_effort, Some(false));
        assert!(model.thinking_level_map.is_none());
        let payload = build_payload(
            &model,
            &user_context(),
            &StreamOptions {
                reasoning: Some(ThinkingLevel::High),
                ..Default::default()
            },
            &crate::compat::detect_compat(&model),
        );
        assert_eq!(payload["enable_thinking"], json!(true));
        assert!(payload.get("reasoning_effort").is_none());
        assert!(payload.get("thinking").is_none());
    }

    let qwen38 = crate::registry::get_model("qwen-token-plan-individual", "qwen3.8-max").unwrap();
    let levels = qwen38.thinking_level_map.as_ref().unwrap();
    assert_eq!(levels.get("low").and_then(|v| v.as_deref()), Some("low"));
    assert_eq!(
        levels.get("medium").and_then(|v| v.as_deref()),
        Some("medium")
    );
    assert_eq!(
        levels.get("xhigh").and_then(|v| v.as_deref()),
        Some("xhigh")
    );
    assert!(matches!(levels.get("high"), Some(None)));
    assert!(matches!(levels.get("max"), Some(None)));
    let payload = build_payload(
        &qwen38,
        &user_context(),
        &StreamOptions {
            reasoning: Some(ThinkingLevel::XHigh),
            ..Default::default()
        },
        &crate::compat::detect_compat(&qwen38),
    );
    assert_eq!(payload["enable_thinking"], json!(true));
    assert_eq!(payload["reasoning_effort"], json!("xhigh"));
    assert!(payload.get("thinking").is_none());
}
