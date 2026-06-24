//! Test-for-test port of upstream `test/bedrock-thinking-payload.test.ts`
//! (`@earendil-works/pi-ai` v0.80.2) — the `additionalModelRequestFields` thinking
//! cases (the live max-tokens E2E case is N/A without credentials).

#[cfg(test)]
mod tests {
    use crate::provider::bedrock::bedrock_thinking_fields;
    use crate::registry::get_model;
    use crate::types::{Model, StreamOptions, ThinkingLevel};
    use serde_json::{json, Value};

    fn fields(model: &Model, reasoning: ThinkingLevel) -> Value {
        let opts = StreamOptions { reasoning: Some(reasoning), ..Default::default() };
        bedrock_thinking_fields(model, &opts).expect("thinking fields").0
    }

    fn opus_4_8_global() -> Model {
        // Upstream builds it by overriding the opus-4-6 base id/name to 4.8.
        let mut m = get_model("amazon-bedrock", "global.anthropic.claude-opus-4-6-v1").unwrap();
        m.id = "global.anthropic.claude-opus-4-8-v1".into();
        m.name = "Claude Opus 4.8 (Global)".into();
        m
    }

    #[test]
    fn uses_adaptive_thinking_for_claude_opus_4_8_when_reasoning_enabled() {
        let f = fields(&opus_4_8_global(), ThinkingLevel::High);
        assert_eq!(f["thinking"], json!({"type": "adaptive", "display": "summarized"}));
        assert_eq!(f["output_config"], json!({"effort": "high"}));
        assert!(f.get("anthropic_beta").is_none());
    }

    #[test]
    fn maps_xhigh_reasoning_to_effort_xhigh_for_claude_opus_4_8() {
        let f = fields(&opus_4_8_global(), ThinkingLevel::XHigh);
        assert_eq!(f["thinking"], json!({"type": "adaptive", "display": "summarized"}));
        assert_eq!(f["output_config"], json!({"effort": "xhigh"}));
    }

    #[test]
    fn uses_adaptive_thinking_for_claude_fable_5_when_reasoning_enabled() {
        let m = get_model("amazon-bedrock", "global.anthropic.claude-fable-5").unwrap();
        let f = fields(&m, ThinkingLevel::High);
        assert_eq!(f["thinking"], json!({"type": "adaptive", "display": "summarized"}));
        assert_eq!(f["output_config"], json!({"effort": "high"}));
    }

    #[test]
    fn maps_xhigh_reasoning_to_effort_xhigh_for_claude_fable_5() {
        let m = get_model("amazon-bedrock", "global.anthropic.claude-fable-5").unwrap();
        let f = fields(&m, ThinkingLevel::XHigh);
        assert_eq!(f["thinking"], json!({"type": "adaptive", "display": "summarized"}));
        assert_eq!(f["output_config"], json!({"effort": "xhigh"}));
    }

    #[test]
    fn omits_display_for_govcloud_model_ids_on_non_adaptive_claude_thinking() {
        let mut m = get_model("amazon-bedrock", "us.anthropic.claude-sonnet-4-5-20250929-v1:0").unwrap();
        m.id = "us-gov.anthropic.claude-sonnet-4-5-20250929-v1:0".into();
        let f = fields(&m, ThinkingLevel::High);
        // Non-adaptive: budget-based thinking, and GovCloud omits display.
        assert_eq!(f["thinking"]["type"], json!("enabled"));
        assert!(f["thinking"].get("display").is_none(), "GovCloud must omit thinking.display: {f}");
        assert!(f.get("output_config").is_none());
    }

    #[test]
    fn omits_display_for_govcloud_regions_on_adaptive_claude_thinking() {
        let mut m = opus_4_8_global();
        m.id = "us-gov.anthropic.claude-opus-4-8-v1:0".into();
        let f = fields(&m, ThinkingLevel::High);
        assert_eq!(f["thinking"]["type"], json!("adaptive"));
        assert!(f["thinking"].get("display").is_none(), "GovCloud must omit thinking.display: {f}");
        assert_eq!(f["output_config"], json!({"effort": "high"}));
    }

    // --- application inference profile (opaque ARN id; model.name identifies the model) ---

    fn app_profile(base_id: &str, name: &str) -> Model {
        let mut m = get_model("amazon-bedrock", base_id).unwrap();
        m.id = "arn:aws:bedrock:us-east-1:123456789012:application-inference-profile/my-profile".into();
        m.name = name.into();
        m
    }

    #[test]
    fn uses_adaptive_thinking_when_model_name_identifies_the_model_but_arn_does_not() {
        let m = app_profile("global.anthropic.claude-opus-4-6-v1", "Claude Opus 4.6");
        let f = fields(&m, ThinkingLevel::High);
        assert_eq!(f["thinking"], json!({"type": "adaptive", "display": "summarized"}));
        assert_eq!(f["output_config"], json!({"effort": "high"}));
    }

    #[test]
    fn falls_back_to_fixed_budget_thinking_for_non_adaptive_claude_via_model_name() {
        let m = app_profile("us.anthropic.claude-sonnet-4-5-20250929-v1:0", "Claude Sonnet 4.5");
        let f = fields(&m, ThinkingLevel::High);
        assert_eq!(f["thinking"]["type"], json!("enabled"));
        assert!(f["thinking"]["budget_tokens"].is_number());
        assert_eq!(f["anthropic_beta"], json!(["interleaved-thinking-2025-05-14"]));
    }

    #[test]
    fn injects_cache_point_on_last_user_message_when_model_name_identifies_supported_claude() {
        use crate::provider::bedrock::build_bedrock_messages;
        use crate::types::{Context, ContentBlock, Message, Role};
        use aws_sdk_bedrockruntime::types::ContentBlock as BedrockContent;
        let m = app_profile("global.anthropic.claude-opus-4-6-v1", "Claude Sonnet 4.6");
        let ctx = Context {
            system_prompt: Some("You are helpful.".into()), tools: Vec::new(),
            messages: vec![Message {
                role: Role::User, content: vec![ContentBlock::Text { text: "Hello".into(), text_signature: None }],
                timestamp: 0, api: None, provider: None, model: None, response_id: None,
                response_model: None, diagnostics: Vec::new(), usage: None,
                stop_reason: None, error_message: None,
                tool_call_id: None, tool_name: None, is_error: false, details: None,
            }],
        };
        // Default (short) retention -> caching on for a supported Claude model.
        let msgs = build_bedrock_messages(&ctx.messages, &m, &StreamOptions::default()).unwrap();
        let last = msgs.last().unwrap();
        let has_cache_point = last.content().iter().any(|b| matches!(b, BedrockContent::CachePoint(_)));
        assert!(has_cache_point, "last user message must carry a cache point");
    }
}
