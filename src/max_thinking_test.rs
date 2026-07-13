//! Test-for-test port of upstream `test/max-thinking.test.ts` (`@earendil-works/pi-ai` v0.80.6).

#[cfg(test)]
mod tests {
    use crate::provider::codex::build_codex_payload;
    use crate::registry::get_model;
    use crate::simple_options::{clamp_thinking_level, get_supported_thinking_levels};
    use crate::types::{
        ContentBlock, Context, Message, Model, ModelCost, ModelThinkingLevel, Role, StreamOptions,
        ThinkingLevel,
    };
    use std::collections::HashMap;

    fn ordinary_reasoning_model(map: Option<HashMap<String, Option<String>>>) -> Model {
        Model {
            id: "ordinary-reasoning".into(),
            name: "Ordinary Reasoning".into(),
            api: "openai-completions".into(),
            provider: "test".into(),
            base_url: "https://example.com/v1".into(),
            reasoning: true,
            thinking_level_map: map,
            input: vec!["text".into()],
            cost: ModelCost::default(),
            context_window: 128_000,
            max_tokens: 4096,
            headers: None,
            api_key: None,
            compat: Default::default(),
        }
    }

    #[test]
    fn max_is_opt_in_for_ordinary_reasoning_models() {
        let model = ordinary_reasoning_model(None);
        assert_eq!(
            get_supported_thinking_levels(&model),
            vec![
                ModelThinkingLevel::Off,
                ModelThinkingLevel::Minimal,
                ModelThinkingLevel::Low,
                ModelThinkingLevel::Medium,
                ModelThinkingLevel::High,
            ]
        );
        assert_eq!(
            clamp_thinking_level(&model, &ModelThinkingLevel::Max),
            ModelThinkingLevel::High
        );
    }

    #[test]
    fn exposes_xhigh_and_max_for_openai_codex_gpt_5_6_variants() {
        for model_id in ["gpt-5.6-luna", "gpt-5.6-sol", "gpt-5.6-terra"] {
            let model = get_model("openai-codex", model_id)
                .unwrap_or_else(|| panic!("openai-codex/{model_id}"));
            let map = model.thinking_level_map.as_ref().expect("thinkingLevelMap");
            assert_eq!(
                map.get("xhigh"),
                Some(&Some("xhigh".to_string())),
                "{model_id}"
            );
            assert_eq!(map.get("max"), Some(&Some("max".to_string())), "{model_id}");
            assert_eq!(
                get_supported_thinking_levels(&model),
                vec![
                    ModelThinkingLevel::Off,
                    ModelThinkingLevel::Minimal,
                    ModelThinkingLevel::Low,
                    ModelThinkingLevel::Medium,
                    ModelThinkingLevel::High,
                    ModelThinkingLevel::XHigh,
                    ModelThinkingLevel::Max,
                ],
                "{model_id}"
            );
        }
    }

    #[test]
    fn supports_a_hole_between_high_and_max() {
        let model = ordinary_reasoning_model(Some(HashMap::from([
            ("xhigh".to_string(), None),
            ("max".to_string(), Some("max".to_string())),
        ])));
        assert_eq!(
            get_supported_thinking_levels(&model),
            vec![
                ModelThinkingLevel::Off,
                ModelThinkingLevel::Minimal,
                ModelThinkingLevel::Low,
                ModelThinkingLevel::Medium,
                ModelThinkingLevel::High,
                ModelThinkingLevel::Max,
            ]
        );
        assert_eq!(
            clamp_thinking_level(&model, &ModelThinkingLevel::XHigh),
            ModelThinkingLevel::Max
        );
    }

    #[test]
    fn sends_max_to_the_codex_responses_api() {
        let model = get_model("openai-codex", "gpt-5.6-sol").expect("openai-codex/gpt-5.6-sol");
        let context = Context {
            system_prompt: Some("You are a helpful assistant.".into()),
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
                tool_call_id: None,
                tool_name: None,
                is_error: false,
                details: None,
            }],
        };
        let opts = StreamOptions {
            reasoning: Some(ThinkingLevel::Max),
            ..Default::default()
        };
        let payload = build_codex_payload(&model, &context, &opts);
        assert_eq!(
            payload["reasoning"],
            serde_json::json!({ "effort": "max", "summary": "auto" })
        );
    }
}
