//! Azure v1.1.0 provider routing, endpoint overlays and deployment identity.
use crate::events::Event;
use crate::types::*;
use futures::StreamExt;
use serde_json::{Value, json};
use std::collections::HashMap;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn model() -> Model {
    let mut model = crate::models_generated::builtin_models()
        .into_iter()
        .find(|m| m.provider == "openai" && m.id == "gpt-4o-mini")
        .unwrap();
    model.provider = provider_id::AZURE.into();
    model.api = api::OPENAI_COMPLETIONS.into();
    model.base_url.clear();
    model.reasoning = true;
    model.thinking_level_map = Some(HashMap::from([
        ("high".into(), Some("high".into())),
        ("xhigh".into(), None),
        ("max".into(), None),
    ]));
    model.compat = ModelCompat {
        supports_developer_role: Some(false),
        supports_mid_convo_system_messages: Some(true),
        supports_reasoning_effort: Some(true),
        thinking_format: Some("openai".into()),
        ..Default::default()
    };
    model
}
fn context() -> Context {
    Context {
        system_prompt: Some("sys".into()),
        messages: vec![crate::user_message("hello")],
        tools: vec![],
    }
}

#[tokio::test]
async fn azure_completions_maps_deployment_preserves_catalog_identity_and_omits_cache_params() {
    let server = MockServer::start().await;
    Mock::given(method("POST")).and(path("/openai/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).insert_header("content-type","text/event-stream").set_body_string(concat!(
            "data: {\"id\":\"request\",\"model\":\"deployment\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"ok\"},\"finish_reason\":null}]}\n\n",
            "data: {\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":1,\"completion_tokens\":1}}\n\n",
            "data: [DONE]\n\n"
        ))).expect(1).mount(&server).await;
    let model = model();
    let ctx = context();
    let options = StreamOptions {
        azure_base_url: Some(format!("{}/openai/v1", server.uri())),
        azure_deployment_name: Some("deployment".into()),
        api_key: Some("secret".into()),
        reasoning: Some(ThinkingLevel::Max),
        cache_retention: Some(CacheRetention::Long),
        session_id: Some("session".into()),
        on_payload: Some(std::sync::Arc::new(|body, _| {
            assert_eq!(body["model"], "deployment");
            Ok(body)
        })),
        ..Default::default()
    };
    let events = crate::registry::stream(&model, &ctx, &options)
        .collect::<Vec<_>>()
        .await;
    let final_message = events
        .iter()
        .find_map(|event| {
            if let Event::Done { message, .. } = event {
                Some(message)
            } else {
                None
            }
        })
        .unwrap();
    assert_eq!(final_message.model.as_deref(), Some("gpt-4o-mini"));
    let req = server.received_requests().await.unwrap();
    let body: Value = serde_json::from_slice(&req[0].body).unwrap();
    assert_eq!(body["model"], "deployment");
    assert_eq!(body["reasoning_effort"], "high");
    assert_eq!(body["messages"][0]["role"], "system");
    assert!(body.get("thinking").is_none());
    assert!(body.get("prompt_cache_key").is_none());
    assert!(body.get("prompt_cache_retention").is_none());
}

#[test]
fn per_request_azure_options_override_environment_without_global_mutation() {
    let model = model();
    let options = StreamOptions {
        azure_base_url: Some("https://explicit.services.ai.azure.com".into()),
        azure_api_version: Some("2026-01-01".into()),
        azure_deployment_name: Some("explicit".into()),
        env: Some(HashMap::from([
            (
                "AZURE_OPENAI_BASE_URL".into(),
                "https://overlay.openai.azure.com".into(),
            ),
            (
                "AZURE_OPENAI_DEPLOYMENT_NAME_MAP".into(),
                "gpt-4o-mini=mapped".into(),
            ),
        ])),
        ..Default::default()
    };
    assert_eq!(
        crate::provider::azure_config::config(&model, &options).unwrap(),
        (
            "https://explicit.services.ai.azure.com/openai/v1".into(),
            "2026-01-01".into()
        )
    );
    assert_eq!(
        crate::provider::azure_config::deployment(&model, &options),
        "explicit"
    );
    let mut options = options;
    options.azure_base_url = None;
    options.azure_deployment_name = None;
    assert_eq!(
        crate::provider::azure_config::deployment(&model, &options),
        "mapped"
    );
    assert_eq!(
        crate::provider::azure_config::config(&model, &options)
            .unwrap()
            .0,
        "https://overlay.openai.azure.com/openai/v1"
    );
    options.env = Some(HashMap::from([
        ("AZURE_OPENAI_BASE_URL".into(), "".into()),
        ("AZURE_OPENAI_RESOURCE_NAME".into(), "".into()),
    ]));
    assert!(
        crate::provider::azure_config::config(&model, &options)
            .unwrap_err()
            .contains("base URL is required")
    );
}

#[tokio::test]
async fn azure_responses_uses_api_key_version_and_request_deployment_overlays() {
    let server = MockServer::start().await;
    Mock::given(method("POST")).and(path("/v1/responses"))
        .respond_with(ResponseTemplate::new(200).insert_header("content-type","text/event-stream").set_body_string("data: {\"type\":\"response.completed\",\"response\":{\"id\":\"r\",\"status\":\"completed\",\"output\":[],\"usage\":{\"input_tokens\":1,\"output_tokens\":0}}}\n\n"))
        .expect(1).mount(&server).await;
    let mut model = model();
    model.api = api::AZURE_OPENAI_RESPONSES.into();
    let options = StreamOptions {
        azure_base_url: Some(format!("{}/v1", server.uri())),
        azure_api_version: Some("preview".into()),
        azure_deployment_name: Some("mapped".into()),
        env: Some(HashMap::from([(
            "AZURE_OPENAI_API_KEY".into(),
            "env-key".into(),
        )])),
        ..Default::default()
    };
    let ctx = context();
    let events = crate::registry::stream(&model, &ctx, &options)
        .collect::<Vec<_>>()
        .await;
    assert!(
        events
            .iter()
            .any(|event| matches!(event, Event::Done { .. })),
        "{events:?}"
    );
    let requests = server.received_requests().await.unwrap();
    let body: Value = serde_json::from_slice(&requests[0].body).unwrap();
    assert_eq!(body["model"], json!("mapped"));
    assert_eq!(requests[0].headers.get("api-key").unwrap(), "env-key");
    assert_eq!(requests[0].url.query(), Some("api-version=preview"));
}
