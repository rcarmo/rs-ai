//! Test-for-test port of upstream `test/openai-completions-cache-control-format.test.ts`
//! (`@earendil-works/pi-ai` v0.80.2).
//!
//! When `compat.cacheControlFormat === "anthropic"`, Anthropic-style
//! `cache_control: {type:"ephemeral"}` markers are applied to the instruction
//! (system/developer) message, the (last) tool, and the last user message; with
//! `cacheRetention: "none"` the markers are omitted (content stays a string).

#[cfg(test)]
mod tests {
    use crate::compat::detect_compat;
    use crate::provider::openai::build_payload;
    use crate::registry::get_model;
    use crate::types::{
        CacheRetention, ContentBlock, Context, Message, Model, ModelCompat, ModelCost, Role,
        StreamOptions,
    };
    use serde_json::{Value, json};

    fn custom_anthropic_cc_model() -> Model {
        Model {
            id: "custom-qwen".into(),
            name: "Custom Qwen".into(),
            api: "openai-completions".into(),
            provider: "openrouter".into(),
            base_url: "https://example.com/v1".into(),
            reasoning: true,
            thinking_level_map: None,
            input: vec!["text".into()],
            cost: ModelCost::default(),
            context_window: 128000,
            max_tokens: 32000,
            sampling_params: None,
            headers: None,
            api_key: None,
            compat: ModelCompat {
                cache_control_format: Some("anthropic".into()),
                ..Default::default()
            },
        }
    }

    fn ctx() -> Context {
        Context {
            system_prompt: Some("System prompt".into()),
            tools: vec![crate::types::Tool {
                name: "read".into(),
                description: "Read a file".into(),
                parameters: json!({"type": "object", "properties": {"path": {"type": "string"}}}),
                constrained_sampling: None,
            }],
            messages: vec![Message {
                role: Role::User,
                content: vec![ContentBlock::Text {
                    text: "Hello".into(),
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

    fn payload(model: &Model, retention: Option<CacheRetention>) -> Value {
        let opts = StreamOptions {
            cache_retention: retention,
            ..Default::default()
        };
        build_payload(model, &ctx(), &opts, &detect_compat(model))
    }

    fn instruction_message(p: &Value) -> &Value {
        p["messages"]
            .as_array()
            .unwrap()
            .iter()
            .find(|m| {
                matches!(
                    m.get("role").and_then(|r| r.as_str()),
                    Some("system") | Some("developer")
                )
            })
            .expect("instruction message")
    }

    fn expect_anthropic_cache_markers(p: &Value) {
        let instr = instruction_message(p);
        assert!(
            instr["content"].is_array(),
            "instruction content must be an array"
        );
        assert_eq!(
            instr["content"][0]["cache_control"],
            json!({"type": "ephemeral"})
        );

        let tools = p["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0]["cache_control"], json!({"type": "ephemeral"}));

        let messages = p["messages"].as_array().unwrap();
        let last = messages.last().unwrap();
        assert_eq!(last["role"], json!("user"));
        assert!(last["content"].is_array());
        assert_eq!(
            last["content"][0]["cache_control"],
            json!({"type": "ephemeral"})
        );
    }

    #[test]
    fn applies_anthropic_style_cache_markers_when_model_compat_enables_them() {
        let p = payload(&custom_anthropic_cc_model(), None);
        expect_anthropic_cache_markers(&p);
    }

    #[test]
    fn preserves_anthropic_style_cache_markers_for_openrouter_anthropic_models() {
        let model = get_model("openrouter", "anthropic/claude-sonnet-4").expect("catalog model");
        let p = payload(&model, None);
        expect_anthropic_cache_markers(&p);
    }

    #[test]
    fn omits_anthropic_style_cache_markers_when_cache_retention_is_none() {
        let p = payload(&custom_anthropic_cc_model(), Some(CacheRetention::None));
        let instr = instruction_message(&p);
        assert!(
            !instr["content"].is_array(),
            "instruction content stays a string when retention is none"
        );
        assert!(p["tools"][0].get("cache_control").is_none());
        assert!(p["messages"].as_array().unwrap().last().unwrap()["content"].is_string());
    }
}
