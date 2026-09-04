//! Test-for-test port of upstream
//! `test/google-shared-gemini3-unsigned-tool-call.test.ts` (`@earendil-works/pi-ai` v0.80.2).
//!
//! Unsigned (cross-model) Google/Vertex tool calls get no `thoughtSignature` and
//! no `skip_thought_signature_validator`; a valid same-provider/model signature
//! is preserved (only on the block that carried it); non-Gemini-3 models add none.

#[cfg(test)]
mod tests {
    use crate::provider::google::build_google_payload_public;
    use crate::types::{
        ContentBlock, Context, Message, Model, ModelCost, Role, StopReason, StreamOptions,
    };
    use serde_json::Value;
    use std::collections::HashMap;

    fn gemini_model(api: &str, provider: &str, id: &str) -> Model {
        Model {
            id: id.into(),
            name: "Gemini 3 Pro Preview".into(),
            api: api.into(),
            provider: provider.into(),
            base_url: "https://example.com".into(),
            reasoning: true,
            thinking_level_map: None,
            input: vec!["text".into()],
            cost: ModelCost::default(),
            context_window: 128000,
            max_tokens: 8192,
            sampling_params: None,
            headers: None,
            api_key: None,
            compat: Default::default(),
        }
    }

    fn tool_call(id: &str, command: &str, sig: Option<&str>) -> ContentBlock {
        let mut args = HashMap::new();
        args.insert("command".to_string(), serde_json::json!(command));
        ContentBlock::ToolCall {
            id: id.into(),
            name: "bash".into(),
            arguments: args,
            thought_signature: sig.map(|s| s.into()),

            namespace: None,
        }
    }

    /// `assistant_id` is the assistant message's recorded model id (differs from the
    /// target id => cross-model). `thought_signature` is set on the first tool call.
    fn ctx(model: &Model, assistant_id: &str, thought_signature: Option<&str>) -> Context {
        let assistant = Message {
            role: Role::Assistant,
            content: vec![
                tool_call("call_1", "echo hi", thought_signature),
                tool_call("call_2", "ls -la", None),
            ],
            timestamp: 0,
            api: Some(model.api.clone()),
            provider: Some(model.provider.clone()),
            model: Some(assistant_id.into()),
            response_id: None,
            response_model: None,
            provider_thinking_level: None,
            diagnostics: Vec::new(),
            usage: None,
            stop_reason: Some(StopReason::ToolUse),
            deferred: None,
            error_message: None,
            raw_stop_reason: None,
            end_turn: None,
            tool_call_id: None,
            tool_name: None,
            is_error: false,
            details: None,
            added_tool_names: Vec::new(),
        };
        Context {
            system_prompt: None,
            tools: Vec::new(),
            messages: vec![
                Message {
                    role: Role::User,
                    content: vec![ContentBlock::Text {
                        text: "Hi".into(),
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
                },
                assistant,
            ],
        }
    }

    fn model_turn(payload: &Value) -> Value {
        payload["contents"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c.get("role").and_then(|r| r.as_str()) == Some("model"))
            .cloned()
            .expect("a model turn")
    }

    fn function_call_ids(turn: &Value) -> Vec<Option<String>> {
        turn["parts"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|p| p.get("functionCall"))
            .map(|fc| fc.get("id").and_then(|v| v.as_str()).map(|s| s.to_string()))
            .collect()
    }

    fn function_call_sigs(turn: &Value) -> Vec<Option<String>> {
        turn["parts"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|p| p.get("functionCall").is_some())
            .map(|p| {
                p.get("thoughtSignature")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            })
            .collect()
    }

    #[test]
    fn no_skip_validator_for_unsigned_google_gen_ai_tool_calls() {
        let model = gemini_model("google-generative-ai", "google", "gemini-3-pro-preview");
        let p = build_google_payload_public(
            &model,
            &ctx(&model, "other-model", None),
            &StreamOptions::default(),
        );
        let turn = model_turn(&p);
        assert_eq!(
            function_call_ids(&turn),
            vec![Some("call_1".into()), Some("call_2".into())]
        );
        let sigs = function_call_sigs(&turn);
        assert_eq!(sigs, vec![None, None]);
        assert!(
            !serde_json::to_string(&turn)
                .unwrap()
                .contains("skip_thought_signature_validator")
        );
        assert!(
            !serde_json::to_string(&turn)
                .unwrap()
                .contains("Historical context")
        );
    }

    #[test]
    fn no_skip_validator_for_unsigned_vertex_tool_calls() {
        let model = gemini_model("google-vertex", "google-vertex", "gemini-3-pro-preview");
        let p = build_google_payload_public(
            &model,
            &ctx(&model, "other-model", None),
            &StreamOptions::default(),
        );
        let turn = model_turn(&p);
        assert_eq!(function_call_sigs(&turn), vec![None, None]);
        assert!(
            !serde_json::to_string(&turn)
                .unwrap()
                .contains("skip_thought_signature_validator")
        );
    }

    #[test]
    fn preserves_valid_thought_signature_for_same_provider_and_model() {
        let model = gemini_model("google-generative-ai", "google", "gemini-3-pro-preview");
        let valid = "AAAAAAAAAAAAAAAAAAAAAA==";
        let p = build_google_payload_public(
            &model,
            &ctx(&model, "gemini-3-pro-preview", Some(valid)),
            &StreamOptions::default(),
        );
        let turn = model_turn(&p);
        assert_eq!(
            function_call_sigs(&turn),
            vec![Some(valid.to_string()), None]
        );
    }

    #[test]
    fn no_thought_signature_for_non_gemini_3_models() {
        let model = gemini_model("google-generative-ai", "google", "gemini-2.5-flash");
        let p = build_google_payload_public(
            &model,
            &ctx(&model, "other-model", None),
            &StreamOptions::default(),
        );
        let turn = model_turn(&p);
        assert!(function_call_sigs(&turn).iter().all(|s| s.is_none()));
    }
}
