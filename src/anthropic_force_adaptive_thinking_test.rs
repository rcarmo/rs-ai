//! Test-for-test port of upstream `test/anthropic-force-adaptive-thinking.test.ts`
//! (`@earendil-works/pi-ai` v0.80.2).

#[cfg(test)]
mod tests {
    use crate::provider::anthropic::build_anthropic_payload;
    use crate::registry::get_model;
    use crate::types::{
        ContentBlock, Context, Message, Model, ModelCompat, ModelCost, Role, StreamOptions,
        ThinkingLevel,
    };
    use serde_json::{Value, json};

    fn ctx() -> Context {
        Context {
            system_prompt: None,
            tools: Vec::new(),
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
                diagnostics: Vec::new(),
                usage: None,
                stop_reason: None,
                error_message: None,
                raw_stop_reason: None,
                tool_call_id: None,
                tool_name: None,
                is_error: false,
                details: None,
                added_tool_names: Vec::new(),
            }],
        }
    }

    fn custom_model(compat: ModelCompat) -> Model {
        Model {
            id: "vendor--claude-opus-latest".into(),
            name: "Vendor Proxy Opus Latest".into(),
            api: "anthropic-messages".into(),
            provider: "vendor-proxy".into(),
            base_url: "http://127.0.0.1:9".into(),
            reasoning: true,
            thinking_level_map: None,
            input: vec!["text".into()],
            cost: ModelCost::default(),
            context_window: 200000,
            max_tokens: 32000,
            headers: None,
            api_key: None,
            compat,
        }
    }

    fn payload(model: &Model, reasoning: Option<ThinkingLevel>) -> Value {
        let opts = StreamOptions {
            reasoning,
            ..Default::default()
        };
        build_anthropic_payload(model, &ctx(), &opts)
    }

    #[test]
    fn sends_legacy_thinking_payload_for_custom_model_ids_by_default() {
        let p = payload(
            &custom_model(ModelCompat::default()),
            Some(ThinkingLevel::Medium),
        );
        assert_eq!(p["thinking"]["type"], json!("enabled"));
        assert!(p.get("output_config").is_none());
    }

    #[test]
    fn sends_adaptive_thinking_payload_when_force_adaptive_thinking_true() {
        let compat = ModelCompat {
            force_adaptive_thinking: Some(true),
            ..Default::default()
        };
        let p = payload(&custom_model(compat), Some(ThinkingLevel::Medium));
        assert_eq!(
            p["thinking"],
            json!({"type": "adaptive", "display": "summarized"})
        );
        assert_eq!(p["output_config"], json!({"effort": "medium"}));
    }

    #[test]
    fn uses_adaptive_thinking_with_native_xhigh_effort_for_claude_fable_5() {
        let model = get_model("anthropic", "claude-fable-5").unwrap();
        let p = payload(&model, Some(ThinkingLevel::XHigh));
        assert_eq!(
            p["thinking"],
            json!({"type": "adaptive", "display": "summarized"})
        );
        assert_eq!(p["output_config"], json!({"effort": "xhigh"}));
    }

    #[test]
    fn allows_builtin_adaptive_models_to_opt_out_with_force_adaptive_thinking_false() {
        let mut model = get_model("anthropic", "claude-opus-4-8").unwrap();
        model.compat = ModelCompat {
            force_adaptive_thinking: Some(false),
            ..Default::default()
        };
        let p = payload(&model, Some(ThinkingLevel::Medium));
        assert_eq!(p["thinking"]["type"], json!("enabled"));
        assert!(p.get("output_config").is_none());
    }

    #[test]
    fn preserves_thinking_disabled_when_reasoning_off_regardless_of_override() {
        let compat = ModelCompat {
            force_adaptive_thinking: Some(true),
            ..Default::default()
        };
        let p = payload(&custom_model(compat), None);
        assert_eq!(p["thinking"], json!({"type": "disabled"}));
        assert!(p.get("output_config").is_none());
    }
}
