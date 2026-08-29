use crate::events::Event;
use crate::provider::mistral::stream_mistral;
use crate::provider::openai::build_payload;
use crate::provider::responses::build_responses_payload;
use crate::types::{Context, Model, ModelCompat, ModelCost, StreamOptions, ThinkingLevel};
use futures::StreamExt;
use serde_json::json;
use std::collections::{HashMap, HashSet};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

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

fn thinking_map(pairs: &[(&str, Option<&str>)]) -> HashMap<String, Option<String>> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.map(str::to_string)))
        .collect()
}

#[test]
fn release_pinned_catalog_counts_match_v0844() {
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
    assert_eq!(
        pairs.iter().filter(|(_, id)| id.contains(":batch")).count(),
        40
    );

    let image_pairs = crate::images::list_image_models(None)
        .into_iter()
        .map(|model| (model.provider, model.id))
        .collect::<HashSet<_>>();
    assert_eq!(image_pairs.len(), 50);
    assert!(image_pairs.contains(&("openrouter".to_string(), "meta/muse-image".to_string())));
    assert!(image_pairs.contains(&(
        "openrouter".to_string(),
        "recraft/recraft-v4-vector".to_string()
    )));
}

#[test]
fn tool_choice_none_serializes_without_tools() {
    let ctx = user_context();
    let opts = StreamOptions {
        tool_choice: Some(json!("none")),
        ..Default::default()
    };
    let openai = test_model(
        crate::types::api::OPENAI_COMPLETIONS,
        "openai",
        "https://example.invalid/v1",
    );
    let payload = build_payload(&openai, &ctx, &opts, &crate::compat::detect_compat(&openai));
    assert_eq!(payload["tool_choice"], json!("none"));
    assert!(payload.get("tools").is_none());

    let responses = test_model(
        crate::types::api::OPENAI_RESPONSES,
        "openai",
        "https://example.invalid/v1",
    );
    let payload = build_responses_payload(&responses, &ctx, &opts);
    assert_eq!(payload["tool_choice"], json!("none"));
    assert!(payload.get("tools").is_none());
}

#[test]
fn openrouter_mandatory_and_optional_reasoning_payloads_match_v0844() {
    let mut mandatory = test_model(
        crate::types::api::OPENAI_COMPLETIONS,
        "openrouter",
        "https://example.invalid/v1",
    );
    mandatory.reasoning = true;
    mandatory.compat.thinking_format = Some("openrouter".into());
    mandatory.thinking_level_map = Some(thinking_map(&[
        ("off", None),
        ("minimal", None),
        ("low", Some("low")),
        ("medium", None),
        ("high", Some("high")),
        ("xhigh", None),
        ("max", Some("max")),
    ]));
    let ctx = user_context();
    let payload = build_payload(
        &mandatory,
        &ctx,
        &StreamOptions::default(),
        &crate::compat::detect_compat(&mandatory),
    );
    assert!(payload.get("reasoning").is_none());
    let payload = build_payload(
        &mandatory,
        &ctx,
        &StreamOptions {
            reasoning: Some(ThinkingLevel::Low),
            ..Default::default()
        },
        &crate::compat::detect_compat(&mandatory),
    );
    assert_eq!(payload["reasoning"], json!({"effort":"low"}));

    let mut optional = mandatory.clone();
    optional.thinking_level_map = None;
    let payload = build_payload(
        &optional,
        &ctx,
        &StreamOptions::default(),
        &crate::compat::detect_compat(&optional),
    );
    assert_eq!(payload["reasoning"], json!({"effort":"none"}));
}

#[test]
fn cloudflare_workers_ai_models_are_mirrored_into_gateway_compat_catalog() {
    let model = crate::registry::get_model(
        "cloudflare-ai-gateway",
        "workers-ai/@cf/meta/llama-3.3-70b-instruct-fp8-fast",
    )
    .expect("gateway workers-ai model");
    assert_eq!(model.api, crate::types::api::OPENAI_COMPLETIONS);
    assert!(model.base_url.ends_with("/compat"));
    let ids = crate::registry::list_models(Some("cloudflare-ai-gateway"))
        .into_iter()
        .map(|m| m.id)
        .collect::<Vec<_>>();
    let unique = ids.iter().collect::<HashSet<_>>();
    assert_eq!(unique.len(), ids.len(), "gateway catalog must be deduped");
}

#[test]
fn zai_coding_plan_glm_5_3_cost_matches_v0844() {
    let model =
        crate::registry::get_model("zai-coding-cn", "glm-5.3").expect("zai-coding-cn glm-5.3");
    assert_eq!(model.cost.input, 1.4);
    assert_eq!(model.cost.output, 4.4);
    assert_eq!(model.cost.cache_read, 0.26);
    assert_eq!(model.cost.cache_write, 0.0);
}

#[tokio::test]
async fn mistral_indexed_tool_call_fragments_merge_without_repeated_ids_or_names() {
    let server = MockServer::start().await;
    let body = concat!(
        "data: {\"id\":\"m\",\"choices\":[{\"index\":0,\"finish_reason\":null,\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"tool_1\",\"function\":{\"name\":\"lookup\",\"arguments\":\"{\\\"q\\\":\"}}]}}]}\n\n",
        "data: {\"id\":\"m\",\"choices\":[{\"index\":0,\"finish_reason\":null,\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"name\":\"\",\"arguments\":\"\\\"rust\\\"}\"}}]}}]}\n\n",
        "data: {\"id\":\"m\",\"choices\":[{\"index\":0,\"finish_reason\":\"tool_calls\",\"delta\":{}}]}\n\n",
        "data: [DONE]\n\n"
    );
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_string(body),
        )
        .mount(&server)
        .await;
    let mut model = test_model(
        crate::types::api::MISTRAL_CONVERSATIONS,
        "mistral",
        &server.uri(),
    );
    model.id = "mistral-test".into();
    let ctx = user_context();
    let opts = StreamOptions::default();
    let mut stream = stream_mistral(&model, &ctx, &opts);
    let mut done = None;
    while let Some(event) = stream.next().await {
        match event {
            Event::Done { message, .. } => done = Some(message),
            Event::Error { error, .. } => panic!("unexpected error: {error}"),
            _ => {}
        }
    }
    let message = done.expect("done");
    let tool = message
        .content
        .iter()
        .find_map(|block| match block {
            crate::types::ContentBlock::ToolCall {
                id,
                name,
                arguments,
                ..
            } => Some((id, name, arguments)),
            _ => None,
        })
        .expect("tool call");
    assert_eq!(tool.0, "tool_1");
    assert_eq!(tool.1, "lookup");
    assert_eq!(tool.2.get("q"), Some(&json!("rust")));
}
