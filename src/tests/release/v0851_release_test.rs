use crate::provider::responses::build_responses_payload;
use crate::simple_options::{
    clamp_thinking_level, get_supported_thinking_levels, map_thinking_level,
};
use crate::types::{CacheRetention, Context, ModelThinkingLevel, StreamOptions, provider_id};
use serde_json::json;
use std::collections::HashSet;

fn user_context() -> Context {
    Context {
        system_prompt: None,
        messages: vec![crate::types::user_message("Hi")],
        tools: vec![],
    }
}

#[test]
fn release_pinned_catalog_counts_match_v0851() {
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
    assert_eq!(pairs.len(), 1354);
    assert_eq!(providers.len(), 39);
    assert_eq!(apis.len(), 9);
    assert_eq!(
        pairs.iter().filter(|(_, id)| id.contains(":batch")).count(),
        68
    );

    for pair in [
        ("openai", "gpt-6-astra"),
        ("azure-openai-responses", "gpt-6-astra"),
        ("openai-codex", "gpt-6-astra"),
        ("opencode", "gpt-6-astra"),
        ("github-copilot", "gpt-6-astra"),
        ("openrouter", "openai/gpt-6-astra"),
        ("openrouter", "openai/gpt-6-astra-pro"),
        ("openrouter", "openai/gpt-6-astra:batch"),
        ("openrouter", "openai/gpt-6-astra-pro:batch"),
        ("vercel-ai-gateway", "openai/gpt-6-astra"),
        ("vercel-ai-gateway", "openai/gpt-6-astra-fast"),
    ] {
        assert!(pairs.contains(&pair), "missing generated pair {pair:?}");
    }

    let image_pairs = crate::images::list_image_models(None)
        .into_iter()
        .map(|model| (model.provider, model.id))
        .collect::<HashSet<_>>();
    assert_eq!(image_pairs.len(), 52);
    assert!(image_pairs.contains(&(
        "openrouter".to_string(),
        "microsoft/mai-image-2.6".to_string()
    )));
    assert!(image_pairs.contains(&(
        "openrouter".to_string(),
        "microsoft/mai-image-2.6-flash".to_string()
    )));
}

#[test]
fn gpt_6_astra_openai_catalog_and_cache_compat_match_v0851() {
    let model = crate::registry::get_model(provider_id::OPENAI, "gpt-6-astra").unwrap();
    assert_eq!(model.api, crate::types::api::OPENAI_RESPONSES);
    assert_eq!(model.name, "GPT-6 Astra");
    assert!(model.reasoning);
    assert_eq!(model.context_window, 272000);
    assert_eq!(model.max_tokens, 128000);
    assert_eq!(model.input, vec!["text".to_string(), "image".to_string()]);
    assert_eq!(model.cost.input, 10.0);
    assert_eq!(model.cost.output, 50.0);
    assert_eq!(model.cost.cache_read, 1.0);
    assert_eq!(model.cost.cache_write, 12.5);
    assert_eq!(model.cost.tiers.len(), 1);
    assert_eq!(model.cost.tiers[0].input_tokens_above, 272000);
    assert_eq!(model.cost.tiers[0].input, 20.0);
    assert_eq!(model.cost.tiers[0].output, 75.0);
    assert_eq!(model.cost.tiers[0].cache_read, 2.0);
    assert_eq!(model.cost.tiers[0].cache_write, 25.0);
    assert_eq!(model.compat.supports_explicit_prompt_cache_mode, Some(true));
    assert_eq!(model.compat.supports_openai_grammar_tools, Some(true));
    assert_eq!(model.compat.supports_additional_tools, Some(true));
    assert_eq!(model.compat.supports_tool_search, Some(true));

    let payload = build_responses_payload(
        &model,
        &user_context(),
        &StreamOptions {
            cache_retention: Some(CacheRetention::Long),
            session_id: Some("astra-session".into()),
            ..Default::default()
        },
    );
    assert_eq!(payload["prompt_cache_key"], json!("astra-session"));
    assert_eq!(payload["prompt_cache_options"], json!({"ttl": "30m"}));
    assert!(payload.get("prompt_cache_retention").is_none());
}

#[test]
fn gpt_6_astra_codex_catalog_and_additional_tool_flags_match_v0851() {
    let model = crate::registry::get_model(provider_id::OPENAI_CODEX, "gpt-6-astra").unwrap();
    assert_eq!(model.api, crate::types::api::OPENAI_CODEX_RESPONSES);
    assert_eq!(model.base_url, "https://chatgpt.com/backend-api");
    assert!(model.reasoning);
    assert_eq!(model.context_window, 272000);
    assert_eq!(model.max_tokens, 128000);
    assert_eq!(model.cost.input, 10.0);
    assert_eq!(model.cost.output, 50.0);
    assert_eq!(model.cost.cache_read, 1.0);
    assert_eq!(model.cost.cache_write, 12.5);
    assert_eq!(model.cost.tiers[0].input_tokens_above, 272000);
    assert_eq!(model.compat.supports_openai_grammar_tools, Some(true));
    assert_eq!(model.compat.supports_additional_tools, Some(true));
    assert_eq!(model.compat.supports_tool_search, Some(true));
}

#[test]
fn gpt_6_astra_thinking_levels_support_xhigh_and_max() {
    let openai = crate::registry::get_model(provider_id::OPENAI, "gpt-6-astra").unwrap();
    let codex = crate::registry::get_model(provider_id::OPENAI_CODEX, "gpt-6-astra").unwrap();

    assert_eq!(
        get_supported_thinking_levels(&openai),
        vec![
            ModelThinkingLevel::Low,
            ModelThinkingLevel::Medium,
            ModelThinkingLevel::High,
            ModelThinkingLevel::XHigh,
            ModelThinkingLevel::Max,
        ]
    );
    assert_eq!(
        clamp_thinking_level(&openai, &ModelThinkingLevel::Off),
        ModelThinkingLevel::Low
    );
    assert_eq!(
        map_thinking_level(&openai, &ModelThinkingLevel::Off),
        Some("low".into())
    );
    assert_eq!(
        map_thinking_level(&openai, &ModelThinkingLevel::Minimal),
        Some("low".into())
    );
    assert_eq!(
        map_thinking_level(&openai, &ModelThinkingLevel::Max),
        Some("max".into())
    );

    assert_eq!(
        get_supported_thinking_levels(&codex),
        vec![
            ModelThinkingLevel::Minimal,
            ModelThinkingLevel::Low,
            ModelThinkingLevel::Medium,
            ModelThinkingLevel::High,
            ModelThinkingLevel::XHigh,
            ModelThinkingLevel::Max,
        ]
    );
    assert_eq!(
        map_thinking_level(&codex, &ModelThinkingLevel::Minimal),
        Some("low".into())
    );
    assert_eq!(
        map_thinking_level(&codex, &ModelThinkingLevel::Max),
        Some("max".into())
    );
}

#[test]
fn gpt_6_astra_generated_alias_compat_is_release_pinned() {
    let opencode = crate::registry::get_model("opencode", "gpt-6-astra").unwrap();
    assert_eq!(opencode.api, crate::types::api::OPENAI_RESPONSES);
    assert_eq!(opencode.context_window, 1050000);
    assert_eq!(opencode.max_tokens, 128000);
    assert_eq!(
        opencode.compat.session_affinity_format.as_deref(),
        Some("openai-nosession")
    );
    assert_eq!(opencode.compat.supports_openai_grammar_tools, Some(true));

    let copilot = crate::registry::get_model(provider_id::GITHUB_COPILOT, "gpt-6-astra").unwrap();
    assert_eq!(copilot.api, crate::types::api::OPENAI_COMPLETIONS);
    assert_eq!(copilot.context_window, 1050000);
    assert_eq!(copilot.max_tokens, 128000);
    assert_eq!(copilot.compat.supports_store, Some(false));
    assert_eq!(copilot.compat.supports_developer_role, Some(false));
    assert_eq!(copilot.compat.supports_reasoning_effort, Some(false));

    let openrouter = crate::registry::get_model("openrouter", "openai/gpt-6-astra").unwrap();
    assert_eq!(openrouter.api, crate::types::api::OPENAI_COMPLETIONS);
    assert_eq!(openrouter.context_window, 1050000);
    assert_eq!(openrouter.max_tokens, 128000);
    assert_eq!(
        openrouter.compat.thinking_format.as_deref(),
        Some("openrouter")
    );
}
