//! Deterministic production-stream coverage for upstream
//! `test/pre-generation-error.test.ts`.
//!
//! Rust exposes provider pre-generation/auth failures as the first `Event::Error`
//! from the concrete stream entrypoint rather than a synchronous JS throw.

use crate::events::Event;
use crate::provider::anthropic::stream_anthropic;
use crate::provider::codex::stream_codex;
use crate::provider::google::stream_google;
use crate::provider::mistral::stream_mistral;
use crate::provider::openai::stream_openai;
use crate::provider::pi_messages::stream_pi_messages;
use crate::provider::responses::{stream_azure_responses, stream_responses};
use crate::types::{ContentBlock, Context, Message, Model, ModelCost, Role, StreamOptions};
use tokio_stream::StreamExt;

fn model(api: &str, provider: &str) -> Model {
    Model {
        id: "missing-auth-model".into(),
        name: "Missing Auth Model".into(),
        api: api.into(),
        provider: provider.into(),
        base_url: "https://example.invalid/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec!["text".into()],
        cost: ModelCost::default(),
        context_window: 128000,
        max_tokens: 4096,
        sampling_params: None,
        headers: None,
        api_key: None,
        compat: Default::default(),
    }
}

fn ctx() -> Context {
    Context {
        system_prompt: None,
        tools: Vec::new(),
        messages: vec![Message {
            role: Role::User,
            content: vec![ContentBlock::Text {
                text: "hello".into(),
                text_signature: None,
            }],
            timestamp: 0,
            api: None,
            provider: None,
            model: None,
            response_id: None,
            response_model: None,
            provider_thinking_level: None,
            diagnostics: Vec::new(),
            usage: None,
            stop_reason: None,
            deferred: None,
            error_message: None,
            raw_stop_reason: None,
            end_turn: None,
            tool_call_id: None,
            tool_name: None,
            is_error: false,
            details: None,
            added_tool_names: Vec::new(),
        }],
    }
}

async fn first_error(
    mut stream: std::pin::Pin<Box<dyn futures::Stream<Item = Event> + Send + '_>>,
) -> String {
    match stream.next().await.expect("first event") {
        Event::Error { error, message, .. } => {
            assert!(
                message.is_none(),
                "pre-generation error should not fabricate a partial message"
            );
            error.to_string()
        }
        other => panic!("expected first pre-generation event to be error, got {other:?}"),
    }
}

#[tokio::test]
async fn missing_auth_surfaces_first_error_from_concrete_provider_streams() {
    let c = ctx();
    let opts = StreamOptions::default();

    let cases = [
        (
            first_error(stream_openai(
                &model(crate::types::api::OPENAI_COMPLETIONS, "missing-openai"),
                &c,
                &opts,
            ))
            .await,
            "No API key for provider: missing-openai",
        ),
        (
            first_error(stream_responses(
                &model(crate::types::api::OPENAI_RESPONSES, "missing-responses"),
                &c,
                &opts,
            ))
            .await,
            "No API key for provider: missing-responses",
        ),
        (
            first_error(stream_azure_responses(
                &model(
                    crate::types::api::AZURE_OPENAI_RESPONSES,
                    "missing-azure-responses",
                ),
                &c,
                &opts,
            ))
            .await,
            "No API key for provider: missing-azure-responses",
        ),
        (
            first_error(stream_codex(
                &model(crate::types::api::OPENAI_CODEX_RESPONSES, "missing-codex"),
                &c,
                &opts,
            ))
            .await,
            "No API key for provider: missing-codex",
        ),
        (
            first_error(stream_anthropic(
                &model(crate::types::api::ANTHROPIC_MESSAGES, "missing-anthropic"),
                &c,
                &opts,
            ))
            .await,
            "No API key for provider: missing-anthropic",
        ),
        (
            first_error(stream_google(
                &model(crate::types::api::GOOGLE_GENERATIVE_AI, "missing-google"),
                &c,
                &opts,
            ))
            .await,
            "No API key for provider: missing-google",
        ),
        (
            first_error(stream_mistral(
                &model(crate::types::api::MISTRAL_CONVERSATIONS, "missing-mistral"),
                &c,
                &opts,
            ))
            .await,
            "No API key for provider: missing-mistral",
        ),
        (
            first_error(stream_pi_messages(
                &model(crate::types::api::PI_MESSAGES, "missing-pi-messages"),
                &c,
                &opts,
            ))
            .await,
            "No API key provided for provider \"missing-pi-messages\"",
        ),
    ];

    for (actual, expected) in cases {
        assert_eq!(actual, expected);
    }
}
