use crate::events::Event;
use crate::provider::openai::{build_payload, stream_openai};
use crate::provider::responses::build_responses_payload;
use crate::types::{
    ContentBlock, Context, Model, ModelCompat, ModelCost, StopReason, StreamOptions,
    ThinkingBudgets, ThinkingLevel, api, provider_id,
};
use futures::StreamExt;
use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn user_context() -> Context {
    Context {
        system_prompt: None,
        messages: vec![crate::types::user_message("Hello")],
        tools: vec![],
    }
}

fn completions_model(provider: &str, id: &str) -> Model {
    Model {
        id: id.into(),
        name: id.into(),
        api: api::OPENAI_COMPLETIONS.into(),
        provider: provider.into(),
        base_url: "http://127.0.0.1:9/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec!["text".into()],
        cost: ModelCost::default(),
        context_window: 128000,
        max_tokens: 16384,
        sampling_params: None,
        headers: None,
        api_key: Some("test".into()),
        compat: ModelCompat::default(),
    }
}

#[test]
fn release_pinned_catalog_has_no_unpinned_batch_aliases() {
    use std::collections::HashSet;

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
    assert_eq!(pairs.len(), 1153);
    assert_eq!(providers.len(), 38);
    assert_eq!(apis.len(), 9);
    assert!(
        all.iter().all(|model| !model.id.contains(":batch")),
        "release-pinned v0.84 catalog must not include fresh OpenRouter :batch aliases"
    );

    let image_pairs = crate::images::list_image_models(None)
        .into_iter()
        .map(|model| (model.provider, model.id))
        .collect::<HashSet<_>>();
    assert_eq!(image_pairs.len(), 42);
}

#[test]
fn sampling_params_merge_and_override_openai_compatible_payloads() {
    let mut model = completions_model("custom-provider", "custom-model");
    model.sampling_params = Some(json!({"top_p": 0.95, "min_p": 0.05, "temperature": 0.25}));
    let opts = StreamOptions {
        temperature: Some(0.0),
        sampling_params: Some(json!({"top_p": 0.5, "top_k": 0, "min_p": 0})),
        ..Default::default()
    };
    let compat = crate::compat::detect_compat(&model);
    let payload = build_payload(&model, &user_context(), &opts, &compat);

    assert_eq!(payload["top_p"], json!(0.5));
    assert_eq!(payload["top_k"], json!(0));
    assert_eq!(payload["min_p"], json!(0));
    assert_eq!(payload["temperature"], json!(0.25));

    let responses = Model {
        api: api::OPENAI_RESPONSES.into(),
        provider: provider_id::OPENAI.into(),
        base_url: "https://api.openai.com/v1".into(),
        ..model
    };
    let responses_payload = build_responses_payload(&responses, &user_context(), &opts);
    assert_eq!(responses_payload["top_p"], json!(0.5));
    assert_eq!(responses_payload["top_k"], json!(0));
    assert_eq!(responses_payload["min_p"], json!(0));
    assert_eq!(responses_payload["temperature"], json!(0.25));
}

#[test]
fn baseten_catalog_and_reasoning_payload_match_v0840() {
    let glm = crate::registry::get_model("baseten", "zai-org/GLM-5.2").unwrap();
    assert_eq!(glm.api, api::OPENAI_COMPLETIONS);
    assert_eq!(glm.provider, "baseten");
    assert_eq!(glm.base_url, "https://inference.baseten.co/v1");
    assert!(glm.reasoning);
    assert_eq!(glm.context_window, 1_048_576);
    assert_eq!(glm.max_tokens, 262_144);
    assert_eq!(glm.cost.input, 1.4);
    assert_eq!(glm.cost.output, 4.4);
    assert_eq!(glm.cost.cache_read, 0.3);
    assert_eq!(glm.compat.thinking_format.as_deref(), Some("baseten"));
    assert_eq!(glm.compat.supports_reasoning_effort, Some(true));
    assert_eq!(glm.compat.max_tokens_field.as_deref(), Some("max_tokens"));
    assert_eq!(glm.compat.supports_long_cache_retention, Some(false));
    assert_eq!(
        glm.thinking_level_map
            .as_ref()
            .and_then(|m| m.get("max"))
            .cloned()
            .flatten()
            .as_deref(),
        Some("max")
    );

    let opts = StreamOptions {
        reasoning: Some(ThinkingLevel::High),
        ..Default::default()
    };
    let compat = crate::compat::detect_compat(&glm);
    let payload = build_payload(&glm, &user_context(), &opts, &compat);
    assert_eq!(
        payload["chat_template_args"],
        json!({"enable_thinking": true})
    );
    assert_eq!(payload["reasoning_effort"], json!("high"));

    let off_payload = build_payload(&glm, &user_context(), &StreamOptions::default(), &compat);
    assert_eq!(
        off_payload["chat_template_args"],
        json!({"enable_thinking": false})
    );
    assert_eq!(off_payload["reasoning_effort"], json!("none"));

    let kimi = crate::registry::get_model("baseten", "moonshotai/Kimi-K2.6").unwrap();
    assert_eq!(kimi.input, vec!["text".to_string(), "image".to_string()]);
    assert_eq!(kimi.compat.supports_reasoning_effort, Some(false));
    assert_eq!(kimi.compat.thinking_format.as_deref(), Some("baseten"));
    assert_eq!(
        crate::simple_options::get_supported_thinking_levels(&kimi)
            .into_iter()
            .map(|level| level.to_string())
            .collect::<Vec<_>>(),
        vec!["off", "high"]
    );
    let kimi_payload = build_payload(
        &kimi,
        &user_context(),
        &StreamOptions {
            reasoning: Some(ThinkingLevel::High),
            ..Default::default()
        },
        &crate::compat::detect_compat(&kimi),
    );
    assert_eq!(
        kimi_payload["chat_template_args"],
        json!({"enable_thinking": true})
    );
    assert!(kimi_payload.get("reasoning_effort").is_none());
}

#[test]
fn vllm_thinking_token_budget_edge_matrix() {
    let mut model = completions_model("local-vllm", "zai-org/glm-5.2");
    model.reasoning = true;
    model.max_tokens = 16384;
    model.compat = ModelCompat {
        max_tokens_field: Some("max_tokens".into()),
        thinking_format: Some("zai".into()),
        supports_thinking_token_budget: Some(true),
        ..Default::default()
    };
    let compat = crate::compat::detect_compat(&model);

    let medium = build_payload(
        &model,
        &user_context(),
        &StreamOptions {
            reasoning: Some(ThinkingLevel::Medium),
            thinking_budgets: Some(ThinkingBudgets {
                medium: Some(4096),
                ..Default::default()
            }),
            ..Default::default()
        },
        &compat,
    );
    assert_eq!(medium["thinking_token_budget"], json!(4096));

    let unsupported = build_payload(
        &Model {
            compat: ModelCompat {
                thinking_format: Some("zai".into()),
                ..Default::default()
            },
            ..model.clone()
        },
        &user_context(),
        &StreamOptions {
            reasoning: Some(ThinkingLevel::Medium),
            thinking_budgets: Some(ThinkingBudgets {
                medium: Some(4096),
                ..Default::default()
            }),
            ..Default::default()
        },
        &crate::compat::detect_compat(&Model {
            compat: ModelCompat {
                thinking_format: Some("zai".into()),
                ..Default::default()
            },
            ..model.clone()
        }),
    );
    assert!(unsupported.get("thinking_token_budget").is_none());

    let off = build_payload(
        &model,
        &user_context(),
        &StreamOptions {
            thinking_budgets: Some(ThinkingBudgets {
                high: Some(8192),
                ..Default::default()
            }),
            ..Default::default()
        },
        &compat,
    );
    assert!(off.get("thinking_token_budget").is_none());

    let xhigh = build_payload(
        &model,
        &user_context(),
        &StreamOptions {
            reasoning: Some(ThinkingLevel::XHigh),
            thinking_budgets: Some(ThinkingBudgets {
                high: Some(8192),
                ..Default::default()
            }),
            ..Default::default()
        },
        &compat,
    );
    let max = build_payload(
        &model,
        &user_context(),
        &StreamOptions {
            reasoning: Some(ThinkingLevel::Max),
            thinking_budgets: Some(ThinkingBudgets {
                high: Some(8192),
                ..Default::default()
            }),
            ..Default::default()
        },
        &compat,
    );
    assert_eq!(xhigh["thinking_token_budget"], json!(8192));
    assert_eq!(max["thinking_token_budget"], json!(8192));

    let high_default = build_payload(
        &model,
        &user_context(),
        &StreamOptions {
            reasoning: Some(ThinkingLevel::High),
            ..Default::default()
        },
        &compat,
    );
    assert_eq!(high_default["thinking_token_budget"], json!(16384 - 1024));

    let caller_ceiling = build_payload(
        &model,
        &user_context(),
        &StreamOptions {
            reasoning: Some(ThinkingLevel::High),
            max_tokens: Some(4096),
            thinking_budgets: Some(ThinkingBudgets {
                high: Some(8192),
                ..Default::default()
            }),
            ..Default::default()
        },
        &compat,
    );
    assert_eq!(caller_ceiling["thinking_token_budget"], json!(4096 - 1024));
}

#[test]
fn nullable_anyof_oneof_preserves_matching_null_before_coercion() {
    let tool = crate::types::Tool {
        name: "echo".into(),
        description: "Echo tool".into(),
        parameters: json!({
            "type": "object",
            "properties": {"value": {"anyOf": [{"type":"number"}, {"type":"null"}]}},
            "required": ["value"]
        }),
        constrained_sampling: None,
    };
    let out = crate::validation::validate_tool_arguments(&tool, &json!({"value": null})).unwrap();
    assert_eq!(out, json!({"value": null}));

    let one_of_tool = crate::types::Tool {
        parameters: json!({
            "type": "object",
            "properties": {"value": {"oneOf": [{"type":"number"}, {"type":"null"}]}},
            "required": ["value"]
        }),
        ..tool.clone()
    };
    let out =
        crate::validation::validate_tool_arguments(&one_of_tool, &json!({"value": null})).unwrap();
    assert_eq!(out, json!({"value": null}));

    let coerced =
        crate::validation::validate_tool_arguments(&tool, &json!({"value": "42"})).unwrap();
    assert_eq!(coerced, json!({"value": 42}));
}

#[tokio::test]
async fn supports_finish_reason_false_infers_terminal_stop_or_tool_use() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            "data: {\"id\":\"r1\",\"choices\":[{\"delta\":{\"content\":\"ok\"}}]}\n\ndata: [DONE]\n\n",
        ))
        .mount(&server)
        .await;
    let mut model = completions_model("no-finish", "no-finish-model");
    model.base_url = format!("{}/v1", server.uri());
    model.compat.supports_finish_reason = Some(false);
    let opts = StreamOptions {
        api_key: Some("test".into()),
        ..Default::default()
    };
    let ctx = user_context();
    let mut stream = stream_openai(&model, &ctx, &opts);
    let mut done = None;
    while let Some(event) = stream.next().await {
        match event {
            Event::Done { reason, message } => done = Some((reason, message)),
            Event::Error { error, .. } => panic!("unexpected error: {error}"),
            _ => {}
        }
    }
    let (reason, message) = done.expect("done event");
    assert_eq!(reason, StopReason::Stop);
    assert_eq!(message.stop_reason, Some(StopReason::Stop));
    assert_eq!(
        message.content.iter().find_map(|block| match block {
            ContentBlock::Text { text, .. } => Some(text.as_str()),
            _ => None,
        }),
        Some("ok")
    );

    let tool_server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_1\",\"function\":{\"name\":\"search\",\"arguments\":\"{\\\"q\\\":\\\"x\\\"}\"}}]}}]}\n\ndata: [DONE]\n\n",
        ))
        .mount(&tool_server)
        .await;
    drop(stream);
    let mut tool_model = model;
    tool_model.base_url = format!("{}/v1", tool_server.uri());
    let tool_ctx = user_context();
    let mut stream = stream_openai(&tool_model, &tool_ctx, &opts);
    let mut reason = None;
    while let Some(event) = stream.next().await {
        if let Event::Done { reason: r, .. } = event {
            reason = Some(r);
        }
    }
    assert_eq!(reason, Some(StopReason::ToolUse));
}
