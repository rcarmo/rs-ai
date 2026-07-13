//! Test-for-test port of upstream `test/anthropic-thinking-disable.test.ts`
//! (`@earendil-works/pi-ai` v0.80.2) — the deterministic payload cases.
//!
//! The live E2E case (model must emit no thinking + 40 "pong"s) is N/A without
//! credentials.

#[cfg(test)]
mod tests {
    use crate::provider::anthropic::build_anthropic_payload;
    use crate::registry::get_model;
    use crate::types::{ContentBlock, Context, Message, Model, Role, StreamOptions, ThinkingLevel};
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
                tool_call_id: None,
                tool_name: None,
                is_error: false,
                details: None,
                added_tool_names: Vec::new(),
            }],
        }
    }

    fn anthropic(id: &str) -> Model {
        get_model("anthropic", id).unwrap_or_else(|| panic!("catalog anthropic/{id}"))
    }

    fn payload(id: &str, reasoning: Option<ThinkingLevel>) -> Value {
        let opts = StreamOptions {
            reasoning,
            ..Default::default()
        };
        build_anthropic_payload(&anthropic(id), &ctx(), &opts)
    }

    #[test]
    fn sends_thinking_disabled_for_budget_reasoning_models_when_off() {
        let p = payload("claude-sonnet-4-5", None);
        assert_eq!(p["thinking"], json!({"type": "disabled"}));
        assert!(p.get("output_config").is_none());
    }

    #[test]
    fn sends_thinking_disabled_for_adaptive_reasoning_models_when_off() {
        let p = payload("claude-opus-4-6", None);
        assert_eq!(p["thinking"], json!({"type": "disabled"}));
        assert!(p.get("output_config").is_none());
    }

    #[test]
    fn sends_thinking_disabled_for_claude_opus_4_8_when_off() {
        let p = payload("claude-opus-4-8", None);
        assert_eq!(p["thinking"], json!({"type": "disabled"}));
        assert!(p.get("output_config").is_none());
    }

    #[test]
    fn omits_thinking_disabled_for_claude_fable_5_when_off() {
        let p = payload("claude-fable-5", None);
        assert!(p.get("thinking").is_none());
        assert!(p.get("output_config").is_none());
    }

    #[test]
    fn uses_adaptive_thinking_for_claude_opus_4_8_when_reasoning_enabled() {
        let p = payload("claude-opus-4-8", Some(ThinkingLevel::High));
        assert_eq!(
            p["thinking"],
            json!({"type": "adaptive", "display": "summarized"})
        );
        assert_eq!(p["output_config"], json!({"effort": "high"}));
    }

    #[test]
    fn uses_adaptive_thinking_for_claude_sonnet_5_when_reasoning_enabled() {
        // v0.80.3: claude-sonnet-5 ships compat.forceAdaptiveThinking=true.
        let p = payload("claude-sonnet-5", Some(ThinkingLevel::High));
        assert_eq!(
            p["thinking"],
            json!({"type": "adaptive", "display": "summarized"})
        );
        assert_eq!(p["output_config"], json!({"effort": "high"}));
    }

    #[test]
    fn clamps_request_max_tokens_to_remaining_context() {
        // Shared canon clamp boundary (docs/local-tests-shared.md §B):
        // contextWindow=5000, "hello" (5 chars -> 2 tokens), maxTokens=2000.
        // used = 2 + CONTEXT_SAFETY_TOKENS(4096) = 4098; available = 5000-4098 = 902;
        // min(2000, 902) = 902. Asserted on the actual anthropic request param.
        let mut model = anthropic("claude-haiku-4-5");
        model.reasoning = false; // isolate the clamp from thinking-budget adjustment
        model.context_window = 5000;
        model.max_tokens = 8192;
        let ctx = Context {
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
                diagnostics: Vec::new(),
                usage: None,
                stop_reason: None,
                error_message: None,
                tool_call_id: None,
                tool_name: None,
                is_error: false,
                details: None,
                added_tool_names: Vec::new(),
            }],
        };
        let opts = StreamOptions {
            max_tokens: Some(2000),
            ..Default::default()
        };
        let p = build_anthropic_payload(&model, &ctx, &opts);
        assert_eq!(p["max_tokens"], json!(902));
    }

    #[test]
    fn maps_xhigh_reasoning_to_effort_xhigh_for_claude_opus_4_8() {
        let p = payload("claude-opus-4-8", Some(ThinkingLevel::XHigh));
        assert_eq!(
            p["thinking"],
            json!({"type": "adaptive", "display": "summarized"})
        );
        assert_eq!(p["output_config"], json!({"effort": "xhigh"}));
    }
}
