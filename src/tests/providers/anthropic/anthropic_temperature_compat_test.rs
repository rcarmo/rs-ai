//! Test-for-test port of upstream `test/anthropic-temperature-compat.test.ts`
//! (`@earendil-works/pi-ai` v0.80.2).
//!
//! Temperature is omitted from the Anthropic payload for models whose compat
//! disables it (Claude Opus 4.7/4.8 and custom `supportsTemperature:false`) and
//! kept for those that allow it (Opus 4.6, Sonnet 4.6).

#[cfg(test)]
mod tests {
    use crate::provider::anthropic::build_anthropic_payload;
    use crate::registry::get_model;
    use crate::types::{
        ContentBlock, Context, Message, Model, ModelCompat, ModelCost, Role, StreamOptions,
    };
    use serde_json::Value;

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

    fn payload(model: &Model, temperature: f64) -> Value {
        let opts = StreamOptions {
            temperature: Some(temperature),
            ..Default::default()
        };
        build_anthropic_payload(model, &ctx(), &opts)
    }

    fn anthropic(id: &str) -> Model {
        get_model("anthropic", id).unwrap_or_else(|| panic!("catalog anthropic/{id}"))
    }

    #[test]
    fn omits_temperature_for_claude_opus_4_7() {
        assert!(
            payload(&anthropic("claude-opus-4-7"), 0.0)
                .get("temperature")
                .is_none()
        );
    }

    #[test]
    fn omits_temperature_for_claude_opus_4_8() {
        assert!(
            payload(&anthropic("claude-opus-4-8"), 0.0)
                .get("temperature")
                .is_none()
        );
    }

    #[test]
    fn omits_default_temperature_for_claude_opus_4_7() {
        assert!(
            payload(&anthropic("claude-opus-4-7"), 1.0)
                .get("temperature")
                .is_none()
        );
    }

    #[test]
    fn keeps_temperature_for_claude_opus_4_6() {
        assert_eq!(
            payload(&anthropic("claude-opus-4-6"), 0.0)["temperature"],
            serde_json::json!(0.0)
        );
    }

    #[test]
    fn keeps_temperature_for_claude_sonnet_4_6() {
        assert_eq!(
            payload(&anthropic("claude-sonnet-4-6"), 0.0)["temperature"],
            serde_json::json!(0.0)
        );
    }

    #[test]
    fn omits_temperature_for_custom_models_with_supports_temperature_disabled() {
        let model = Model {
            id: "vendor--claude-opus-4-7".into(),
            name: "Vendor Proxy Opus 4.7".into(),
            api: "anthropic-messages".into(),
            provider: "vendor-proxy".into(),
            base_url: "http://127.0.0.1:9".into(),
            reasoning: true,
            thinking_level_map: None,
            input: vec!["text".into()],
            cost: ModelCost::default(),
            context_window: 200000,
            max_tokens: 32000,
            sampling_params: None,
            headers: None,
            api_key: None,
            compat: ModelCompat {
                supports_temperature: Some(false),
                ..Default::default()
            },
        };
        assert!(payload(&model, 0.0).get("temperature").is_none());
    }
}
