//! Test-for-test port (payload substance) of upstream
//! `test/openai-responses-tool-result-images.test.ts` (`@earendil-works/pi-ai` v0.80.2).
//!
//! The live end-to-end assertions (model must answer "red"/"circle") are N/A
//! without credentials; the portable substance is that a tool-result image is
//! placed in the `function_call_output` item's `output` content array (as
//! `input_text` + `input_image`), and not leaked into a later user message.

#[cfg(test)]
mod tests {
    use crate::provider::responses::build_responses_payload;
    use crate::types::{
        ContentBlock, Context, Message, Model, ModelCost, Role, StopReason, StreamOptions,
    };
    use std::collections::HashMap;

    fn image_responses_model() -> Model {
        Model {
            id: "gpt-5-mini".into(),
            name: "GPT-5 Mini".into(),
            api: "openai-responses".into(),
            provider: "openai".into(),
            base_url: "https://api.openai.com/v1".into(),
            reasoning: true,
            thinking_level_map: None,
            input: vec!["text".into(), "image".into()],
            cost: ModelCost::default(),
            context_window: 400000,
            max_tokens: 128000,
            headers: None,
            api_key: None,
            compat: Default::default(),
        }
    }

    #[test]
    fn sends_tool_result_images_in_function_call_output() {
        let assistant = Message {
            role: Role::Assistant,
            content: vec![ContentBlock::ToolCall {
                id: "call_1".into(),
                name: "get_circle_with_description".into(),
                arguments: HashMap::new(),
                thought_signature: None,
            }],
            timestamp: 0,
            api: Some("openai-responses".into()),
            provider: Some("openai".into()),
            model: Some("gpt-5-mini".into()),
            response_id: None,
            response_model: None,
            diagnostics: Vec::new(),
            usage: None,
            stop_reason: Some(StopReason::ToolUse),
            error_message: None,
            tool_call_id: None,
            tool_name: None,
            is_error: false,
            details: None,
        };
        let tool_text = "A red circle with a diameter of 100 pixels.";
        let tool_result = Message {
            role: Role::ToolResult,
            content: vec![
                ContentBlock::Text {
                    text: tool_text.into(),
                    text_signature: None,
                },
                ContentBlock::Image {
                    data: "ZmFrZQ==".into(),
                    mime_type: "image/png".into(),
                },
            ],
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
            tool_call_id: Some("call_1".into()),
            tool_name: Some("get_circle_with_description".into()),
            is_error: false,
            details: None,
        };
        let ctx = Context {
            system_prompt: Some("You are a helpful assistant.".into()),
            tools: Vec::new(),
            messages: vec![
                Message {
                    role: Role::User,
                    content: vec![ContentBlock::Text {
                        text: "Call the tool.".into(),
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
                },
                assistant,
                tool_result,
            ],
        };

        let payload =
            build_responses_payload(&image_responses_model(), &ctx, &StreamOptions::default());
        let input = payload["input"].as_array().expect("input array");
        let fco_index = input
            .iter()
            .position(|i| i.get("type").and_then(|t| t.as_str()) == Some("function_call_output"))
            .expect("a function_call_output item");
        let output = input[fco_index]["output"]
            .as_array()
            .expect("function_call_output.output is a content array");

        let text_item = output
            .iter()
            .find(|i| i.get("type").and_then(|t| t.as_str()) == Some("input_text"));
        let image_item = output
            .iter()
            .find(|i| i.get("type").and_then(|t| t.as_str()) == Some("input_image"));
        let text_item = text_item.expect("an input_text item");
        let image_item = image_item.expect("an input_image item");
        assert!(text_item["text"].as_str().unwrap().contains(tool_text));
        assert!(
            image_item["image_url"]
                .as_str()
                .unwrap()
                .starts_with("data:image/png;base64,")
        );

        // The image must not leak into a later user message.
        let later_user_with_image = input[fco_index + 1..].iter().any(|i| {
            i.get("role").and_then(|r| r.as_str()) == Some("user")
                && serde_json::to_string(i).unwrap().contains("input_image")
        });
        assert!(
            !later_user_with_image,
            "tool-result image must stay in function_call_output"
        );
    }
}
