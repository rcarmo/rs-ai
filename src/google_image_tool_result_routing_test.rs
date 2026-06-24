//! Test-for-test port of upstream
//! `test/google-shared-image-tool-result-routing.test.ts` (`@earendil-works/pi-ai` v0.80.2).
//!
//! Gemini 2.x Google API models route a tool-result image into a separate
//! synthetic user turn ("Tool result image:" + inlineData), splitting the
//! function responses; Gemini 3 nests the image inside the functionResponse.

#[cfg(test)]
mod tests {
    use crate::provider::google::build_google_payload_public;
    use crate::types::{Context, ContentBlock, Message, Model, ModelCost, Role, StopReason, StreamOptions};
    use serde_json::Value;
    use std::collections::HashMap;

    fn model(id: &str) -> Model {
        Model {
            id: id.into(), name: id.into(), api: "google-generative-ai".into(), provider: "google".into(),
            base_url: "https://example.com".into(), reasoning: true, thinking_level_map: None,
            input: vec!["text".into(), "image".into()], cost: ModelCost::default(),
            context_window: 128000, max_tokens: 8192, headers: None, api_key: None, compat: Default::default(),
        }
    }

    fn tool_call(id: &str, path: &str) -> ContentBlock {
        let mut args = HashMap::new();
        args.insert("path".to_string(), serde_json::json!(path));
        ContentBlock::ToolCall { id: id.into(), name: "read".into(), arguments: args, thought_signature: None }
    }

    fn tool_result(id: &str, content: Vec<ContentBlock>) -> Message {
        Message {
            role: Role::ToolResult, content, timestamp: 0,
            api: None, provider: None, model: None, response_id: None, response_model: None,
            diagnostics: Vec::new(), usage: None, stop_reason: None, error_message: None,
            tool_call_id: Some(id.into()), tool_name: Some("read".into()), is_error: false, details: None,
        }
    }

    fn ctx(m: &Model) -> Context {
        let assistant = Message {
            role: Role::Assistant,
            content: vec![tool_call("call_a", "a.txt"), tool_call("call_img", "image.png"), tool_call("call_b", "b.txt")],
            timestamp: 0,
            api: Some(m.api.clone()), provider: Some(m.provider.clone()), model: Some(m.id.clone()),
            response_id: None, response_model: None, diagnostics: Vec::new(), usage: None,
            stop_reason: Some(StopReason::ToolUse), error_message: None,
            tool_call_id: None, tool_name: None, is_error: false, details: None,
        };
        Context {
            system_prompt: None, tools: Vec::new(),
            messages: vec![
                Message { role: Role::User, content: vec![ContentBlock::Text { text: "read the files".into(), text_signature: None }],
                    timestamp: 0, api: None, provider: None, model: None, response_id: None, response_model: None,
                    diagnostics: Vec::new(), usage: None, stop_reason: None, error_message: None,
                    tool_call_id: None, tool_name: None, is_error: false, details: None },
                assistant,
                tool_result("call_a", vec![ContentBlock::Text { text: "alpha text".into(), text_signature: None }]),
                tool_result("call_img", vec![ContentBlock::Image { data: "abc".into(), mime_type: "image/png".into() }]),
                tool_result("call_b", vec![ContentBlock::Text { text: "beta text".into(), text_signature: None }]),
            ],
        }
    }

    fn contents(m: &Model) -> Vec<Value> {
        build_google_payload_public(m, &ctx(m), &StreamOptions::default())["contents"].as_array().unwrap().clone()
    }

    #[test]
    fn keeps_separate_synthetic_image_turn_for_gemini_2x_google_api_models() {
        let c = contents(&model("gemini-2.5-flash"));
        assert_eq!(c.len(), 5);
        assert!(c[2]["parts"].as_array().unwrap().iter().all(|p| p.get("functionResponse").is_some()));
        assert_eq!(c[3]["parts"][0]["text"], serde_json::json!("Tool result image:"));
        assert!(c[3]["parts"][1].get("inlineData").is_some());
        assert!(c[4]["parts"][0].get("functionResponse").is_some());
    }

    #[test]
    fn nests_image_tool_results_for_gemini_3_google_api_models() {
        let c = contents(&model("gemini-3-pro-preview"));
        assert_eq!(c.len(), 3);
        let tool_turn = &c[2];
        assert_eq!(tool_turn["parts"].as_array().unwrap().len(), 3);
        let image_response = &tool_turn["parts"][1]["functionResponse"];
        assert!(image_response.is_object());
        let parts = image_response["parts"].as_array().expect("nested functionResponse.parts");
        assert_eq!(parts.len(), 1);
        assert!(parts[0].get("inlineData").is_some());
    }
}
