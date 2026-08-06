//! Test-for-test port of upstream
//! `test/openai-responses-empty-tool-result.test.ts` (`@earendil-works/pi-ai` v0.80.5).
//!
//! A blank text-only tool result (no images) serializes as the "(no tool output)"
//! placeholder in the `function_call_output` item, not the image placeholder.

#[cfg(test)]
mod tests {
    use crate::provider::responses::build_responses_payload;
    use crate::types::{
        ContentBlock, Context, Message, Model, ModelCost, Role, StopReason, StreamOptions,
    };
    use std::collections::HashMap;

    fn model() -> Model {
        Model {
            id: "gpt-4o-mini".into(),
            name: "GPT-4o Mini".into(),
            api: "openai-responses".into(),
            provider: "openai".into(),
            base_url: "https://api.openai.com/v1".into(),
            reasoning: false,
            thinking_level_map: None,
            input: vec!["text".into()],
            cost: ModelCost::default(),
            context_window: 128000,
            max_tokens: 16000,
            sampling_params: None,
            headers: None,
            api_key: None,
            compat: Default::default(),
        }
    }

    #[test]
    fn uses_no_tool_output_placeholder_for_empty_tool_results_without_images() {
        let assistant = Message {
            role: Role::Assistant,
            content: vec![ContentBlock::ToolCall {
                id: "tool-1".into(),
                name: "bash".into(),
                arguments: HashMap::new(),
                thought_signature: None,
            }],
            timestamp: 0,
            api: Some("openai-responses".into()),
            provider: Some("openai".into()),
            model: Some("gpt-4o-mini".into()),
            response_id: None,
            response_model: None,
            diagnostics: Vec::new(),
            usage: None,
            stop_reason: Some(StopReason::ToolUse),
            error_message: None,
            raw_stop_reason: None,
            tool_call_id: None,
            tool_name: None,
            is_error: false,
            details: None,
            added_tool_names: Vec::new(),
        };
        let tool_result = Message {
            role: Role::ToolResult,
            content: vec![ContentBlock::Text {
                text: "".into(),
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
            tool_call_id: Some("tool-1".into()),
            tool_name: Some("bash".into()),
            is_error: false,
            details: None,
            added_tool_names: Vec::new(),
        };
        let ctx = Context {
            system_prompt: None,
            tools: Vec::new(),
            messages: vec![
                Message {
                    role: Role::User,
                    content: vec![ContentBlock::Text {
                        text: "Run the command".into(),
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
                },
                assistant,
                tool_result,
            ],
        };

        let payload = build_responses_payload(&model(), &ctx, &StreamOptions::default());
        let input = payload["input"].as_array().expect("input array");
        let fco = input
            .iter()
            .find(|i| i.get("type").and_then(|t| t.as_str()) == Some("function_call_output"))
            .expect("a function_call_output item");
        let output = fco["output"].as_str().expect("string output");
        assert_eq!(output, "(no tool output)");
        assert!(!output.contains("see attached image"));
    }
}
