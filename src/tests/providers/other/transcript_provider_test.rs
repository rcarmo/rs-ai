use indexmap::IndexMap;
use serde_json::{Value, json};

use crate::compat::detect_compat;
use crate::provider::anthropic::build_anthropic_payload;
use crate::provider::openai::build_payload;
use crate::provider::responses::build_responses_payload;
use crate::transcript::system_message;
use crate::types::{
    Context, Model, ModelCompat, ModelCost, StreamOptions, Tool, ToolReference, user_message,
};

fn tool(name: &str) -> Tool {
    Tool {
        name: name.into(),
        description: format!("{name} tool"),
        parameters: json!({"type": "object", "properties": {}}),
        constrained_sampling: None,
    }
}

fn model(api: &str, provider: &str, compat: ModelCompat) -> Model {
    Model {
        id: "test-model".into(),
        name: "Test model".into(),
        api: api.into(),
        provider: provider.into(),
        base_url: "http://127.0.0.1:9".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec!["text".into()],
        input_limits: None,
        prompt_cache: None,
        enabled: None,
        lab: None,
        providers: None,
        cost: ModelCost::default(),
        context_window: 100_000,
        max_tokens: 1_000,
        sampling_params: None,
        headers: None,
        api_key: Some("test-key".into()),
        compat,
    }
}

fn transcript(removal: bool) -> Context {
    let base = tool("base_tool");
    let late = tool("late_tool");
    let mut initial_sections = IndexMap::new();
    initial_sections.insert("rules".into(), Some("<rules>old</rules>".into()));
    initial_sections.insert("docs".into(), Some("<docs>read</docs>".into()));
    let mut updated_sections = IndexMap::new();
    updated_sections.insert("rules".into(), Some("<rules>new</rules>".into()));
    updated_sections.insert("docs".into(), None);
    Context {
        system_prompt: None,
        messages: vec![
            system_message("base prompt", Some(initial_sections), vec![base], vec![]),
            user_message("before"),
            system_message(
                "updated guidance",
                Some(updated_sections),
                vec![late],
                removal
                    .then(|| ToolReference {
                        name: "base_tool".into(),
                    })
                    .into_iter()
                    .collect(),
            ),
        ],
        tools: vec![],
    }
}

fn names(values: &[Value], pointer: &str) -> Vec<String> {
    values
        .iter()
        .filter_map(|value| {
            value
                .pointer(pointer)
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .collect()
}

#[test]
fn unsupported_openai_compatible_collapses_to_current_prompt_and_tools() {
    let model = model(
        "openai-completions",
        "custom-provider",
        ModelCompat::default(),
    );
    let payload = build_payload(
        &model,
        &transcript(true),
        &StreamOptions::default(),
        &detect_compat(&model),
    );
    assert_eq!(payload["messages"][0]["role"], "developer");
    assert_eq!(
        payload["messages"][0]["content"],
        "base prompt\n\nupdated guidance\n\n<rules>new</rules>"
    );
    assert_eq!(
        names(payload["tools"].as_array().unwrap(), "/function/name"),
        ["late_tool"]
    );
}

#[test]
fn openai_responses_anchors_additive_tools_but_falls_back_for_removals() {
    let model = model(
        "openai-responses",
        "openai",
        ModelCompat {
            supports_mid_convo_system_messages: Some(true),
            supports_additional_tools: Some(true),
            ..Default::default()
        },
    );
    let additive = build_responses_payload(&model, &transcript(false), &StreamOptions::default());
    assert_eq!(
        names(additive["tools"].as_array().unwrap(), "/function/name"),
        ["base_tool"]
    );
    let additions = additive["input"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["type"] == "additional_tools")
        .unwrap();
    assert_eq!(
        names(additions["tools"].as_array().unwrap(), "/function/name"),
        ["late_tool"]
    );
    assert_eq!(
        additive["input"]
            .as_array()
            .unwrap()
            .iter()
            .map(|item| {
                item.get("type")
                    .and_then(Value::as_str)
                    .or_else(|| item.get("role").and_then(Value::as_str))
                    .unwrap()
            })
            .collect::<Vec<_>>(),
        ["developer", "user", "additional_tools", "developer"]
    );

    let fallback = build_responses_payload(&model, &transcript(true), &StreamOptions::default());
    assert_eq!(
        names(fallback["tools"].as_array().unwrap(), "/function/name"),
        ["late_tool"]
    );
    assert!(
        !fallback["input"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["type"] == "additional_tools")
    );
}

#[test]
fn anthropic_preserves_native_updates_and_collapses_when_unsupported() {
    let native = model(
        "anthropic-messages",
        "anthropic",
        ModelCompat {
            supports_mid_convo_system_messages: Some(true),
            supports_mid_convo_tool_changes: Some(true),
            ..Default::default()
        },
    );
    let payload = build_anthropic_payload(&native, &transcript(true), &StreamOptions::default());
    assert_eq!(
        payload["system"][0]["text"],
        "base prompt\n\n<rules>old</rules>\n\n<docs>read</docs>"
    );
    let update = payload["messages"].as_array().unwrap().last().unwrap();
    assert_eq!(update["role"], "system");
    assert_eq!(update["content"][1]["type"], "tool_removal");
    assert_eq!(update["content"][2]["type"], "tool_addition");

    let unsupported = model("anthropic-messages", "anthropic", ModelCompat::default());
    let payload =
        build_anthropic_payload(&unsupported, &transcript(true), &StreamOptions::default());
    assert_eq!(
        payload["system"][0]["text"],
        "base prompt\n\nupdated guidance\n\n<rules>new</rules>"
    );
    assert_eq!(
        names(payload["tools"].as_array().unwrap(), "/name"),
        ["late_tool"]
    );
    assert_eq!(
        payload["messages"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|message| message["role"] == "system")
            .count(),
        0
    );
}
