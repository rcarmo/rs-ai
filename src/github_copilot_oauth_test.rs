//! Test-for-test port (deterministic substance) of upstream
//! `test/github-copilot-oauth.test.ts` (`@earendil-works/pi-ai` v0.80.2).
//!
//! The full interactive device-code login orchestration (slow_down interval
//! increase, timeout, onDeviceCode callbacks) is part of the interactive-OAuth
//! surface; rs-ai has the primitives (`start_github_device_flow`,
//! `poll_github_device_token`, the generic `poll_oauth_device_code_flow`, and
//! the untrusted-verification_uri rejection — see `oauth.rs` tests). The portable
//! deterministic piece here is the model-picker catalog filter and the
//! untrusted-uri guard.

#[cfg(test)]
mod tests {
    use crate::oauth::{is_selectable_copilot_model, selectable_copilot_model_ids};
    use serde_json::json;

    #[test]
    fn filters_models_to_the_authenticated_account_picker_catalog() {
        let data = vec![
            json!({"id": "gpt-4.1", "model_picker_enabled": true, "capabilities": {"supports": {"tool_calls": true}}}),
            json!({"id": "claude-opus-4.7", "model_picker_enabled": true, "policy": {"state": "disabled"}, "capabilities": {"supports": {"tool_calls": true}}}),
            json!({"id": "gpt-5.4-nano", "model_picker_enabled": false, "capabilities": {"supports": {"tool_calls": true}}}),
        ];
        assert_eq!(selectable_copilot_model_ids(&data), vec!["gpt-4.1".to_string()]);
    }

    #[test]
    fn is_selectable_requires_picker_enabled_not_disabled_and_tool_calls() {
        // picker enabled + tool calls + no disabled policy -> selectable.
        assert!(is_selectable_copilot_model(&json!({"model_picker_enabled": true, "capabilities": {"supports": {"tool_calls": true}}})));
        // policy disabled -> not selectable.
        assert!(!is_selectable_copilot_model(&json!({"model_picker_enabled": true, "policy": {"state": "disabled"}, "capabilities": {"supports": {"tool_calls": true}}})));
        // picker disabled -> not selectable.
        assert!(!is_selectable_copilot_model(&json!({"model_picker_enabled": false, "capabilities": {"supports": {"tool_calls": true}}})));
        // no tool-call support -> not selectable.
        assert!(!is_selectable_copilot_model(&json!({"model_picker_enabled": true, "capabilities": {"supports": {"tool_calls": false}}})));
    }
}
