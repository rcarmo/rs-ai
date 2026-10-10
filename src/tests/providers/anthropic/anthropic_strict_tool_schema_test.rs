//! Adaptation of upstream v0.99.2 `anthropic-strict-tool-schema.test.ts`.

#[cfg(test)]
mod tests {
    use crate::provider::anthropic::{anthropic_tool_strict_mode, build_anthropic_payload};
    use crate::types::{Context, Model, ModelCompat, ModelCost, StreamOptions, Tool};
    use serde_json::{Value, json};

    fn model() -> Model {
        Model {
            id: "claude-test".into(),
            name: "Claude Test".into(),
            api: "anthropic-messages".into(),
            provider: "anthropic".into(),
            base_url: "https://api.anthropic.test".into(),
            reasoning: false,
            thinking_level_map: None,
            input: vec!["text".into()],
            input_limits: None,
            prompt_cache: None,
            enabled: None,
            lab: None,
            providers: None,
            cost: ModelCost::default(),
            context_window: 200_000,
            max_tokens: 8192,
            sampling_params: None,
            sampling_params_by_thinking_level: None,
            headers: None,
            api_key: None,
            compat: ModelCompat {
                supports_strict_tools: Some(true),
                ..Default::default()
            },
        }
    }

    fn tool(parameters: Value, strict: &str) -> Tool {
        Tool {
            name: "lookup".into(),
            description: "Lookup".into(),
            parameters,
            constrained_sampling: Some(json!({"type":"json_schema", "strict": strict})),
        }
    }

    fn payload(tool: Tool) -> Value {
        build_anthropic_payload(
            &model(),
            &Context {
                system_prompt: None,
                messages: Vec::new(),
                tools: vec![tool],
            },
            &StreamOptions::default(),
        )
    }

    #[test]
    fn strict_preferred_preserves_supported_schema_constraints() {
        let body = payload(tool(
            json!({
                "type":"object",
                "title":"LookupInput",
                "properties": {
                    "url": {"type":"string", "format":"uri", "minLength": 4, "pattern":"^https://"},
                    "tags": {"type":"array", "items":{"type":"string"}, "minItems":1},
                    "optional": {"type":"string"}
                },
                "required":["url", "tags"]
            }),
            "prefer",
        ));
        let sent = &body["tools"][0];
        assert_eq!(sent["strict"], true);
        assert_eq!(sent["input_schema"]["title"], "LookupInput");
        assert_eq!(sent["input_schema"]["properties"]["url"]["format"], "uri");
        assert_eq!(sent["input_schema"]["properties"]["tags"]["minItems"], 1);
        assert_eq!(sent["input_schema"]["additionalProperties"], false);
        assert!(
            sent["input_schema"]["required"]
                .as_array()
                .unwrap()
                .iter()
                .any(|item| item == "optional")
        );
        assert_eq!(
            sent["input_schema"]["properties"]["optional"]["anyOf"][1]["type"],
            "null"
        );
    }

    #[test]
    fn strict_preferred_falls_back_for_anthropic_rejected_keywords() {
        for (label, parameters) in [
            (
                "numeric bounds",
                json!({"type":"object","properties":{"n":{"type":"number","minimum":0,"maximum":10}}}),
            ),
            (
                "array minItems 2",
                json!({"type":"object","properties":{"xs":{"type":"array","items":{"type":"string"},"minItems":2}}}),
            ),
            (
                "unsupported format",
                json!({"type":"object","properties":{"pattern":{"type":"string","format":"regex"}}}),
            ),
        ] {
            let body = payload(tool(parameters.clone(), "prefer"));
            let sent = &body["tools"][0];
            assert!(sent.get("strict").is_none(), "{label} stayed strict");
            assert_eq!(sent["input_schema"]["type"], "object");
            assert_eq!(
                sent["input_schema"]["properties"], parameters["properties"],
                "{label} changed the legacy schema"
            );
            assert!(sent["input_schema"].get("title").is_none());
        }
    }

    #[test]
    fn strict_required_rejects_anthropic_unsupported_schema() {
        let required = tool(
            json!({"type":"object","properties":{"n":{"type":"number","minimum":0}}}),
            "require",
        );
        let error = anthropic_tool_strict_mode(&required, true).unwrap_err();
        assert!(error.contains("minimum schemas are unsupported by this provider"));
    }
}
