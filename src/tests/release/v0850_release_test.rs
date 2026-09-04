use crate::http_proxy::resolve_http_proxy_url_for_target;
use crate::provider::openai::build_payload;
use crate::provider::responses::build_responses_payload;
use crate::types::{Context, Model, ModelCompat, ModelCost, StreamOptions};
use serde_json::json;
use std::collections::{HashMap, HashSet};

fn user_context() -> Context {
    Context {
        system_prompt: None,
        messages: vec![crate::types::user_message("Hi")],
        tools: vec![],
    }
}

fn test_model(api: &str, provider: &str, base_url: &str) -> Model {
    Model {
        id: "model".into(),
        name: "model".into(),
        api: api.into(),
        provider: provider.into(),
        base_url: base_url.into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec!["text".into()],
        cost: ModelCost::default(),
        context_window: 128000,
        max_tokens: 4096,
        sampling_params: None,
        headers: None,
        api_key: Some("test".into()),
        compat: ModelCompat::default(),
    }
}

#[test]
fn release_pinned_catalog_counts_match_v0850() {
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
    assert_eq!(pairs.len(), 1336);
    assert_eq!(providers.len(), 39);
    assert_eq!(apis.len(), 9);
    assert_eq!(
        pairs.iter().filter(|(_, id)| id.contains(":batch")).count(),
        66
    );
    assert!(pairs.contains(&("openrouter", "anthropic/claude-fable-5.1")));
    assert!(pairs.contains(&("openrouter", "anthropic/claude-fable-5.1:batch")));
    assert!(pairs.contains(&("openrouter", "google/gemini-3.8-flash:batch")));
    assert!(pairs.contains(&("openrouter", "x-ai/grok-4.3:batch")));
    assert!(pairs.contains(&("qwen-token-plan-individual", "qwen3.8-flash")));

    let image_pairs = crate::images::list_image_models(None)
        .into_iter()
        .map(|model| (model.provider, model.id))
        .collect::<HashSet<_>>();
    assert_eq!(image_pairs.len(), 50);
}

#[test]
fn openai_completions_vllm_priority_serializes_top_level_priority() {
    let mut model = test_model(
        crate::types::api::OPENAI_COMPLETIONS,
        "vllm",
        "https://example.invalid/v1",
    );
    model.compat.vllm_priority = Some(5);
    let payload = build_payload(
        &model,
        &user_context(),
        &StreamOptions::default(),
        &crate::compat::detect_compat(&model),
    );
    assert_eq!(payload["priority"], json!(5));

    model.compat.vllm_priority = None;
    let payload = build_payload(
        &model,
        &user_context(),
        &StreamOptions::default(),
        &crate::compat::detect_compat(&model),
    );
    assert!(payload.get("priority").is_none());
}

#[test]
fn openai_responses_max_output_tokens_respects_compat_flag() {
    let mut model = test_model(
        crate::types::api::OPENAI_RESPONSES,
        "openai",
        "https://example.invalid/v1",
    );
    let ctx = user_context();
    let opts = StreamOptions {
        max_tokens: Some(8),
        ..Default::default()
    };
    let payload = build_responses_payload(&model, &ctx, &opts);
    assert_eq!(payload["max_output_tokens"], json!(16));

    model.compat.supports_max_output_tokens = Some(false);
    let payload = build_responses_payload(&model, &ctx, &opts);
    assert!(payload.get("max_output_tokens").is_none());
}

#[test]
fn message_serializes_provider_thinking_level_camel_case() {
    let mut message = crate::types::user_message("hi");
    message.role = crate::types::Role::Assistant;
    message.provider_thinking_level = Some("high".into());
    let encoded = serde_json::to_value(&message).unwrap();
    assert_eq!(encoded["providerThinkingLevel"], json!("high"));
    assert!(encoded.get("provider_thinking_level").is_none());
}

#[test]
fn uuidv7_accepts_explicit_timestamp_and_rejects_overflow() {
    let timestamp = 0x0123_4567_89ab;
    let uuid = crate::utils::uuidv7_with_timestamp(timestamp);
    let parsed = u64::from_str_radix(&uuid.replace('-', "")[..12], 16).unwrap();
    assert_eq!(parsed, timestamp);
    let max = crate::utils::uuidv7_with_timestamp((1_u64 << 48) - 1);
    assert_eq!(
        u64::from_str_radix(&max.replace('-', "")[..12], 16).unwrap(),
        (1_u64 << 48) - 1
    );
    let result = std::panic::catch_unwind(|| crate::utils::uuidv7_with_timestamp(1_u64 << 48));
    assert!(result.is_err());
}

#[test]
fn no_proxy_matches_uppercase_suffix_and_port_rules() {
    let env = HashMap::from([
        (
            "HTTPS_PROXY".to_string(),
            "http://proxy.example:8080".to_string(),
        ),
        (
            "NO_PROXY".to_string(),
            ".example.com,api.internal:8443".to_string(),
        ),
    ]);
    assert!(
        resolve_http_proxy_url_for_target("https://service.example.com", Some(&env))
            .unwrap()
            .is_none()
    );
    assert!(
        resolve_http_proxy_url_for_target("https://api.internal:8443", Some(&env))
            .unwrap()
            .is_none()
    );
    assert!(
        resolve_http_proxy_url_for_target("https://api.internal:443", Some(&env))
            .unwrap()
            .is_some()
    );
}
