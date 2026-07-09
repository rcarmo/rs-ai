//! Test-for-test port of upstream `test/openai-completions-tool-result-images.test.ts`
//! (`@earendil-works/pi-ai` v0.80.2).
//!
//! Images carried by consecutive tool-result messages are batched into a single
//! trailing `user` message after the `tool` messages (image-capable model).

#[cfg(test)]
mod tests {
    use crate::compat::detect_compat;
    use crate::provider::openai::build_payload;
    use crate::registry::get_model;
    use crate::types::{Context, ContentBlock, Message, Model, Role, StopReason, StreamOptions};
    use std::collections::HashMap;

    fn image_model() -> Model {
        let mut m = get_model("openai", "gpt-4o-mini").expect("catalog gpt-4o-mini");
        m.input = vec!["text".into(), "image".into()];
        m
    }

    fn tool_call(id: &str, path: &str) -> ContentBlock {
        let mut args = HashMap::new();
        args.insert("path".to_string(), serde_json::json!(path));
        ContentBlock::ToolCall { id: id.into(), name: "read".into(), arguments: args, thought_signature: None }
    }

    fn tool_result(id: &str, ts: i64) -> Message {
        Message {
            role: Role::ToolResult,
            content: vec![
                ContentBlock::Text { text: "Read image file [image/png]".into(), text_signature: None },
                ContentBlock::Image { data: "ZmFrZQ==".into(), mime_type: "image/png".into() },
            ],
            timestamp: ts, api: None, provider: None, model: None, response_id: None,
            response_model: None, diagnostics: Vec::new(), usage: None,
            stop_reason: None, error_message: None,
            tool_call_id: Some(id.into()), tool_name: Some("read".into()), is_error: false, details: None,
        }
    }

    #[test]
    fn batches_tool_result_images_after_consecutive_tool_results() {
        let model = image_model();
        let assistant = Message {
            role: Role::Assistant,
            content: vec![tool_call("tool-1", "img-1.png"), tool_call("tool-2", "img-2.png")],
            timestamp: 0,
            api: Some(model.api.clone()), provider: Some(model.provider.clone()), model: Some(model.id.clone()),
            response_id: None, response_model: None, diagnostics: Vec::new(), usage: None,
            stop_reason: Some(StopReason::ToolUse), error_message: None,
            tool_call_id: None, tool_name: None, is_error: false, details: None,
        };
        let ctx = Context {
            system_prompt: None, tools: Vec::new(),
            messages: vec![
                Message {
                    role: Role::User, content: vec![ContentBlock::Text { text: "Read the images".into(), text_signature: None }],
                    timestamp: 0, api: None, provider: None, model: None, response_id: None,
                    response_model: None, diagnostics: Vec::new(), usage: None,
                    stop_reason: None, error_message: None,
                    tool_call_id: None, tool_name: None, is_error: false, details: None,
                },
                assistant,
                tool_result("tool-1", 1),
                tool_result("tool-2", 2),
            ],
        };

        let payload = build_payload(&model, &ctx, &StreamOptions::default(), &detect_compat(&model));
        let messages = payload["messages"].as_array().unwrap();
        let roles: Vec<&str> = messages.iter().map(|m| m["role"].as_str().unwrap()).collect();
        assert_eq!(roles, vec!["user", "assistant", "tool", "tool", "user"]);

        let image_msg = messages.last().unwrap();
        assert_eq!(image_msg["role"], serde_json::json!("user"));
        assert!(image_msg["content"].is_array());
        let image_parts = image_msg["content"].as_array().unwrap().iter()
            .filter(|p| p.get("type").and_then(|t| t.as_str()) == Some("image_url"))
            .count();
        assert_eq!(image_parts, 2);
    }

    #[test]
    fn uses_no_tool_output_placeholder_for_empty_tool_results_without_images() {
        // v0.80.5: a blank text-only tool result (no images) serializes as the
        // "(no tool output)" placeholder, not the image placeholder.
        let model = image_model();
        let assistant = Message {
            role: Role::Assistant,
            content: vec![tool_call("tool-1", "noop")],
            timestamp: 0,
            api: Some(model.api.clone()), provider: Some(model.provider.clone()), model: Some(model.id.clone()),
            response_id: None, response_model: None, diagnostics: Vec::new(), usage: None,
            stop_reason: Some(StopReason::ToolUse), error_message: None,
            tool_call_id: None, tool_name: None, is_error: false, details: None,
        };
        let empty_tool_result = Message {
            role: Role::ToolResult,
            content: vec![ContentBlock::Text { text: "".into(), text_signature: None }],
            timestamp: 1, api: None, provider: None, model: None, response_id: None,
            response_model: None, diagnostics: Vec::new(), usage: None,
            stop_reason: None, error_message: None,
            tool_call_id: Some("tool-1".into()), tool_name: Some("read".into()), is_error: false, details: None,
        };
        let ctx = Context {
            system_prompt: None, tools: Vec::new(),
            messages: vec![
                Message {
                    role: Role::User, content: vec![ContentBlock::Text { text: "Run it".into(), text_signature: None }],
                    timestamp: 0, api: None, provider: None, model: None, response_id: None,
                    response_model: None, diagnostics: Vec::new(), usage: None,
                    stop_reason: None, error_message: None,
                    tool_call_id: None, tool_name: None, is_error: false, details: None,
                },
                assistant,
                empty_tool_result,
            ],
        };

        let payload = build_payload(&model, &ctx, &StreamOptions::default(), &detect_compat(&model));
        let messages = payload["messages"].as_array().unwrap();
        let tool_msg = messages.iter().find(|m| m["role"].as_str() == Some("tool")).expect("a tool message");
        let content = tool_msg["content"].as_str().expect("string tool content");
        assert_eq!(content, "(no tool output)");
        assert!(!content.contains("see attached image"));
    }
}
