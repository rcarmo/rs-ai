use crate::events::Event;
use crate::provider::anthropic::{
    anthropic_beta_features, build_anthropic_payload, stream_anthropic,
};
use crate::types::{Context, Model, ModelCompat, ModelCost, StreamOptions, ThinkingLevel};
use futures::StreamExt;
use serde_json::json;
use std::collections::HashMap;
use wiremock::matchers::method;
use wiremock::{Mock, MockServer, ResponseTemplate};

fn managed_model(base_url: &str) -> Model {
    Model {
        id: "claude-fable-5-1".into(),
        name: "Claude Fable 5.1".into(),
        api: crate::types::api::ANTHROPIC_MESSAGES.into(),
        provider: "anthropic".into(),
        base_url: base_url.into(),
        reasoning: true,
        thinking_level_map: Some(HashMap::from([
            ("off".into(), None),
            ("low".into(), Some("low".into())),
            ("medium".into(), Some("medium".into())),
            ("high".into(), Some("high".into())),
            ("xhigh".into(), Some("xhigh".into())),
            ("max".into(), Some("max".into())),
        ])),
        input: vec!["text".into()],
        cost: ModelCost::default(),
        context_window: 200000,
        max_tokens: 4096,
        sampling_params: None,
        headers: None,
        api_key: Some("test".into()),
        compat: ModelCompat {
            force_adaptive_thinking: Some(true),
            supports_mid_convo_effort: Some(true),
            ..Default::default()
        },
    }
}

fn ctx() -> Context {
    Context {
        system_prompt: None,
        messages: vec![crate::types::user_message("Hello")],
        tools: vec![],
    }
}

#[test]
fn anthropic_mid_conversation_effort_payload_uses_binding_and_high_output_config() {
    let model = managed_model("https://example.invalid");
    let payload = build_anthropic_payload(
        &model,
        &ctx(),
        &StreamOptions {
            reasoning: Some(ThinkingLevel::Low),
            ..Default::default()
        },
    );
    assert_eq!(payload["thinking"]["type"], json!("adaptive"));
    assert_eq!(payload["thinking"]["display"], json!("summarized"));
    assert_eq!(
        payload["thinking"]["block_binding"],
        json!({"prefix_mismatch_behavior":"drop_block"})
    );
    assert_eq!(payload["output_config"], json!({"effort":"high"}));
    assert!(payload.get("temperature").is_none());
}

#[test]
fn anthropic_mid_conversation_effort_beta_headers_are_enabled() {
    let model = managed_model("https://example.invalid");
    let betas = anthropic_beta_features(&model, &ctx(), false, true);
    assert!(betas.contains(&"mid-conversation-output-config-2026-07-01"));
    assert!(betas.contains(&"thinking-binding-controls-2026-08-01"));
}

#[tokio::test]
async fn anthropic_start_partial_preserves_provider_thinking_level() {
    let server = MockServer::start().await;
    let body = concat!(
        "event: message_start\n",
        "data: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_1\",\"model\":\"claude-fable-5-1\",\"usage\":{\"input_tokens\":1,\"output_tokens\":0}}}\n\n",
        "event: message_delta\n",
        "data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":1}}\n\n",
        "event: message_stop\n",
        "data: {\"type\":\"message_stop\"}\n\n"
    );
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_string(body),
        )
        .mount(&server)
        .await;
    let model = managed_model(&server.uri());
    let context = ctx();
    let options = StreamOptions {
        reasoning: Some(ThinkingLevel::Max),
        ..Default::default()
    };
    let mut stream = stream_anthropic(&model, &context, &options);
    let mut saw_start = false;
    let mut done = None;
    while let Some(event) = stream.next().await {
        match event {
            Event::Start { partial } => {
                saw_start = true;
                assert_eq!(partial.provider_thinking_level.as_deref(), Some("max"));
            }
            Event::Done { message, .. } => done = Some(message),
            Event::Error { error, .. } => panic!("unexpected error: {error}"),
            _ => {}
        }
    }
    assert!(saw_start);
    assert_eq!(
        done.expect("done").provider_thinking_level.as_deref(),
        Some("max")
    );
}
