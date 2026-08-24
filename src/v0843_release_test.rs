use crate::events::Event;
use crate::provider::anthropic::stream_anthropic;
use crate::provider::google::{build_google_payload_public, resolve_google_thinking_level};
use crate::provider::mistral::stream_mistral;
use crate::provider::openai::stream_openai;
use crate::provider::responses::{
    build_responses_payload, stream_azure_responses, stream_responses,
};
use crate::types::{
    Context, Model, ModelCompat, ModelCost, ModelThinkingLevel, StreamOptions, ThinkingBudgets,
    ThinkingLevel, Tool,
};
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

fn thinking_map(pairs: &[(&str, Option<&str>)]) -> HashMap<String, Option<String>> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.map(str::to_string)))
        .collect()
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
        api_key: Some("test-key".into()),
        compat: ModelCompat::default(),
    }
}

fn tool(name: &str) -> Tool {
    Tool {
        name: name.into(),
        description: format!("{name} tool"),
        parameters: json!({"type":"object","properties":{"q":{"type":"string"}}}),
        constrained_sampling: None,
    }
}

fn google_model(
    api: &str,
    provider: &str,
    id: &str,
    map: Option<HashMap<String, Option<String>>>,
) -> Model {
    Model {
        id: id.into(),
        name: id.into(),
        api: api.into(),
        provider: provider.into(),
        base_url: "https://example.invalid/v1".into(),
        reasoning: true,
        thinking_level_map: map,
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
fn release_pinned_catalog_counts_match_v0843() {
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
    assert_eq!(pairs.len(), 1312);
    assert_eq!(providers.len(), 39);
    assert_eq!(apis.len(), 9);
    assert_eq!(
        pairs.iter().filter(|(_, id)| id.contains(":batch")).count(),
        60
    );
    let image_pairs = crate::images::list_image_models(None)
        .into_iter()
        .map(|model| (model.provider, model.id))
        .collect::<HashSet<_>>();
    assert_eq!(image_pairs.len(), 45);
}

#[test]
fn google_thinking_level_resolver_matches_v0843() {
    let base = google_model(
        "google-generative-ai",
        "test-google",
        "gemini-3.7-flash",
        None,
    );
    assert_eq!(
        resolve_google_thinking_level(&base, &ModelThinkingLevel::Off).unwrap(),
        "high"
    );
    assert_eq!(
        resolve_google_thinking_level(&base, &ModelThinkingLevel::Minimal).unwrap(),
        "minimal"
    );
    assert_eq!(
        resolve_google_thinking_level(&base, &ModelThinkingLevel::Low).unwrap(),
        "low"
    );
    assert_eq!(
        resolve_google_thinking_level(&base, &ModelThinkingLevel::Medium).unwrap(),
        "medium"
    );
    assert_eq!(
        resolve_google_thinking_level(&base, &ModelThinkingLevel::High).unwrap(),
        "high"
    );

    let mapped = google_model(
        "google-generative-ai",
        "test-google",
        "gemini-3.7-flash",
        Some(thinking_map(&[
            ("high", Some("LOW")),
            ("xhigh", Some("HIGH")),
            ("max", Some("MEDIUM")),
        ])),
    );
    assert_eq!(
        resolve_google_thinking_level(&mapped, &ModelThinkingLevel::High).unwrap(),
        "low"
    );
    assert_eq!(
        resolve_google_thinking_level(&mapped, &ModelThinkingLevel::XHigh).unwrap(),
        "high"
    );
    assert_eq!(
        resolve_google_thinking_level(&mapped, &ModelThinkingLevel::Max).unwrap(),
        "medium"
    );

    let invalid = google_model(
        "google-generative-ai",
        "test-google",
        "gemini-3.7-flash",
        Some(thinking_map(&[("xhigh", Some("extreme"))])),
    );
    assert_eq!(
        resolve_google_thinking_level(&invalid, &ModelThinkingLevel::XHigh).unwrap_err(),
        "Unsupported Google thinking level mapping for test-google/gemini-3.7-flash: xhigh -> extreme"
    );
    assert_eq!(
        resolve_google_thinking_level(&base, &ModelThinkingLevel::Max).unwrap_err(),
        "Unsupported Google thinking level mapping for test-google/gemini-3.7-flash: max -> undefined"
    );
}

#[test]
fn google_payload_uses_mapped_levels_and_token_budgets() {
    let ctx = user_context();
    let flash = google_model(
        "google-generative-ai",
        "test-google",
        "gemini-3.7-flash",
        Some(thinking_map(&[
            ("xhigh", Some("high")),
            ("max", Some("high")),
        ])),
    );
    let payload = build_google_payload_public(
        &flash,
        &ctx,
        &StreamOptions {
            reasoning: Some(ThinkingLevel::XHigh),
            ..Default::default()
        },
    );
    assert_eq!(
        payload["generationConfig"]["thinkingConfig"],
        json!({"includeThoughts": true, "thinkingLevel": "HIGH"})
    );

    let budget_model = google_model(
        "google-generative-ai",
        "test-google",
        "gemini-2.5-flash",
        Some(thinking_map(&[("xhigh", Some("high"))])),
    );
    let payload = build_google_payload_public(
        &budget_model,
        &ctx,
        &StreamOptions {
            reasoning: Some(ThinkingLevel::XHigh),
            thinking_budgets: Some(ThinkingBudgets {
                high: Some(1234),
                ..Default::default()
            }),
            ..Default::default()
        },
    );
    assert_eq!(
        payload["generationConfig"]["thinkingConfig"],
        json!({"includeThoughts": true, "thinkingBudget": 1234})
    );

    let vertex = google_model(
        "google-vertex",
        "test-vertex",
        "gemini-2.5-flash",
        Some(thinking_map(&[("max", Some("high"))])),
    );
    let payload = build_google_payload_public(
        &vertex,
        &ctx,
        &StreamOptions {
            reasoning: Some(ThinkingLevel::Max),
            thinking_budgets: Some(ThinkingBudgets {
                high: Some(4321),
                ..Default::default()
            }),
            ..Default::default()
        },
    );
    assert_eq!(
        payload["generationConfig"]["thinkingConfig"],
        json!({"includeThoughts": true, "thinkingBudget": 4321})
    );
}

#[test]
fn azure_responses_payload_forwards_provider_neutral_tool_choice() {
    let mut ctx = user_context();
    ctx.tools = vec![tool("lookup")];
    let model = test_model(
        crate::types::api::AZURE_OPENAI_RESPONSES,
        "azure-openai-responses",
        "https://example.openai.azure.com/openai/v1",
    );
    let payload = build_responses_payload(
        &model,
        &ctx,
        &StreamOptions {
            tool_choice: Some(json!("none")),
            ..Default::default()
        },
    );
    assert_eq!(payload["tool_choice"], json!("none"));
    assert_eq!(payload["tools"].as_array().unwrap().len(), 1);
}

async fn collect_terminal_user_agent(
    mut stream: std::pin::Pin<Box<dyn futures::Stream<Item = Event> + Send + '_>>,
) {
    while let Some(event) = stream.next().await {
        if let Event::Error { error, .. } = event {
            panic!("unexpected stream error: {error}");
        }
    }
}

#[tokio::test]
async fn openai_responses_uses_pi_user_agent_by_default_and_allows_override() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/responses"))
        .respond_with(ResponseTemplate::new(200).insert_header("content-type", "text/event-stream").set_body_string(
            "data: {\"type\":\"response.completed\",\"response\":{\"id\":\"r\",\"status\":\"completed\"}}\n\ndata: [DONE]\n\n",
        ))
        .mount(&server)
        .await;
    let model = test_model(crate::types::api::OPENAI_RESPONSES, "openai", &server.uri());
    let ctx = user_context();
    collect_terminal_user_agent(stream_responses(&model, &ctx, &StreamOptions::default())).await;
    let requests = server.received_requests().await.unwrap();
    assert_eq!(
        requests[0].headers["user-agent"].to_str().unwrap(),
        crate::utils::pi_runtime_user_agent()
    );

    let opts = StreamOptions {
        headers: Some(HashMap::from([(
            "User-Agent".into(),
            "custom-agent".into(),
        )])),
        ..Default::default()
    };
    collect_terminal_user_agent(stream_responses(&model, &ctx, &opts)).await;
    let requests = server.received_requests().await.unwrap();
    assert_eq!(
        requests[1].headers["user-agent"].to_str().unwrap(),
        "custom-agent"
    );
}

#[tokio::test]
async fn azure_responses_uses_pi_user_agent_and_preserves_tool_choice_on_wire() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/responses"))
        .respond_with(ResponseTemplate::new(200).insert_header("content-type", "text/event-stream").set_body_string(
            "data: {\"type\":\"response.completed\",\"response\":{\"id\":\"r\",\"status\":\"completed\"}}\n\ndata: [DONE]\n\n",
        ))
        .mount(&server)
        .await;
    let mut model = test_model(
        crate::types::api::AZURE_OPENAI_RESPONSES,
        "azure-openai-responses",
        &server.uri(),
    );
    model.id = "deployment".into();
    let mut ctx = user_context();
    ctx.tools = vec![tool("lookup")];
    let opts = StreamOptions {
        tool_choice: Some(json!("required")),
        ..Default::default()
    };
    collect_terminal_user_agent(stream_azure_responses(&model, &ctx, &opts)).await;
    let requests = server.received_requests().await.unwrap();
    assert_eq!(
        requests[0].headers["user-agent"].to_str().unwrap(),
        crate::utils::pi_runtime_user_agent()
    );
    let body: serde_json::Value = serde_json::from_slice(&requests[0].body).unwrap();
    assert_eq!(body["tool_choice"], json!("required"));
    assert_eq!(body["tools"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn completions_anthropic_and_mistral_default_user_agent_can_be_overridden() {
    let ctx = user_context();

    let openai_server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).insert_header("content-type", "text/event-stream").set_body_string(
            "data: {\"id\":\"c\",\"choices\":[{\"delta\":{\"content\":\"ok\"},\"finish_reason\":null,\"index\":0}]}\n\ndata: {\"id\":\"c\",\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\",\"index\":0}]}\n\ndata: [DONE]\n\n",
        ))
        .mount(&openai_server)
        .await;
    let openai = test_model(
        crate::types::api::OPENAI_COMPLETIONS,
        "openai",
        &openai_server.uri(),
    );
    collect_terminal_user_agent(stream_openai(&openai, &ctx, &StreamOptions::default())).await;
    let opts = StreamOptions {
        headers: Some(HashMap::from([(
            "User-Agent".into(),
            "custom-agent".into(),
        )])),
        ..Default::default()
    };
    collect_terminal_user_agent(stream_openai(&openai, &ctx, &opts)).await;
    let requests = openai_server.received_requests().await.unwrap();
    assert_eq!(
        requests[0].headers["user-agent"].to_str().unwrap(),
        crate::utils::pi_runtime_user_agent()
    );
    assert_eq!(
        requests[1].headers["user-agent"].to_str().unwrap(),
        "custom-agent"
    );

    let anthropic_server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).insert_header("content-type", "text/event-stream").set_body_string(
            "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"m\",\"model\":\"model\",\"usage\":{\"input_tokens\":1,\"output_tokens\":0},\"content\":[]}}\n\nevent: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":1}}\n\nevent: message_stop\ndata: {\"type\":\"message_stop\"}\n\n",
        ))
        .mount(&anthropic_server)
        .await;
    let anthropic = test_model(
        crate::types::api::ANTHROPIC_MESSAGES,
        "anthropic",
        &anthropic_server.uri(),
    );
    collect_terminal_user_agent(stream_anthropic(
        &anthropic,
        &ctx,
        &StreamOptions::default(),
    ))
    .await;
    let requests = anthropic_server.received_requests().await.unwrap();
    assert_eq!(
        requests[0].headers["user-agent"].to_str().unwrap(),
        crate::utils::pi_runtime_user_agent()
    );

    let mistral_server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).insert_header("content-type", "text/event-stream").set_body_string(
            "data: {\"choices\":[{\"index\":0,\"finish_reason\":null,\"delta\":{\"content\":\"ok\"}}]}\n\ndata: {\"choices\":[{\"index\":0,\"finish_reason\":\"stop\",\"delta\":{}}]}\n\ndata: [DONE]\n\n",
        ))
        .mount(&mistral_server)
        .await;
    let mistral = test_model(
        crate::types::api::MISTRAL_CONVERSATIONS,
        "mistral",
        &mistral_server.uri(),
    );
    collect_terminal_user_agent(stream_mistral(&mistral, &ctx, &StreamOptions::default())).await;
    let requests = mistral_server.received_requests().await.unwrap();
    assert_eq!(
        requests[0].headers["user-agent"].to_str().unwrap(),
        crate::utils::pi_runtime_user_agent()
    );
}
