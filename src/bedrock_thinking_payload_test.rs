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
}
