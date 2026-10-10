//! Production payload coverage for upstream v1.1.0 sampling precedence.

use crate::simple_options::{clamp_thinking_level, resolve_sampling_params};
use crate::types::{Context, Model, ModelThinkingLevel, StreamOptions, ThinkingLevel};
use serde_json::{Value, json};
use std::collections::HashMap;

fn model(api: &str) -> Model {
    let mut model = crate::models_generated::builtin_models()
        .into_iter()
        .find(|model| model.provider == "openai" && model.id == "gpt-4o-mini")
        .unwrap();
    model.api = api.into();
    model.reasoning = true;
    model.thinking_level_map = None;
    model.sampling_params = Some(json!({"temperature":1.0,"top_p":0.95}));
    model.sampling_params_by_thinking_level = Some(HashMap::from([
        ("off".into(), json!({"temperature":0.7})),
        ("low".into(), json!({"temperature":0.6,"top_k":64})),
        ("medium".into(), json!({"temperature":0.8})),
        ("high".into(), json!({"temperature":0.9})),
    ]));
    model
}

fn payload(model: &Model, options: &StreamOptions) -> Value {
    let context = Context {
        system_prompt: None,
        messages: vec![crate::user_message("Hello")],
        tools: Vec::new(),
    };
    if model.api == "openai-completions" {
        crate::provider::openai::build_payload(
            model,
            &context,
            options,
            &crate::compat::detect_compat(model),
        )
    } else {
        crate::provider::responses::build_responses_payload(model, &context, options)
    }
}

#[test]
fn direct_payloads_merge_model_effective_level_and_request_in_order() {
    for api in [
        "openai-completions",
        "openai-responses",
        "azure-openai-responses",
    ] {
        let model = model(api);
        let options = StreamOptions {
            temperature: Some(0.0),
            reasoning: Some(ThinkingLevel::Low),
            sampling_params: Some(json!({"top_p":0.5})),
            ..Default::default()
        };
        let body = payload(&model, &options);
        assert_eq!(body["temperature"], json!(0.6), "{api}");
        assert_eq!(body["top_p"], json!(0.5), "{api}");
        assert_eq!(body["top_k"], json!(64), "{api}");
        assert_eq!(
            model.sampling_params.as_ref().unwrap()["top_p"],
            json!(0.95)
        );
    }
}

#[test]
fn disabled_reasoning_uses_off_defaults_and_request_overrides_named_fields() {
    for api in [
        "openai-completions",
        "openai-responses",
        "azure-openai-responses",
    ] {
        let mut model = model(api);
        model.reasoning = false;
        let body = payload(&model, &StreamOptions::default());
        assert_eq!(body["temperature"], json!(0.7), "{api}");
        let body = payload(
            &model,
            &StreamOptions {
                temperature: Some(0.0),
                sampling_params: Some(json!({"temperature":0.4})),
                ..Default::default()
            },
        );
        assert_eq!(body["temperature"], json!(0.4), "{api}");
    }
}

#[test]
fn summary_only_responses_select_medium_sampling_defaults() {
    for api in ["openai-responses", "azure-openai-responses"] {
        let model = model(api);
        let body = payload(
            &model,
            &StreamOptions {
                reasoning_summary: Some("auto".into()),
                ..Default::default()
            },
        );
        assert_eq!(body["temperature"], json!(0.8), "{api}");
        assert_eq!(body["reasoning"]["effort"], json!("medium"), "{api}");
    }
}

#[test]
fn sampling_uses_clamped_logical_level_and_does_not_mutate_model() {
    let mut model = model("openai-completions");
    model.thinking_level_map = Some(HashMap::from([
        ("low".into(), None),
        ("medium".into(), None),
        ("high".into(), Some("provider-high".into())),
    ]));
    assert_eq!(
        clamp_thinking_level(&model, &ModelThinkingLevel::Low),
        ModelThinkingLevel::High
    );
    let body = payload(
        &model,
        &StreamOptions {
            reasoning: Some(ThinkingLevel::Low),
            ..Default::default()
        },
    );
    assert_eq!(body["temperature"], json!(0.9));
    model.sampling_params = None;
    model.sampling_params_by_thinking_level = None;
    assert_eq!(
        resolve_sampling_params(&model, &ModelThinkingLevel::Off, None),
        None
    );
}
