//! Test-for-test port of upstream `test/openai-responses-foreign-toolcall-id.test.ts`
//! (`@earendil-works/pi-ai` v0.80.2).
//!
//! A foreign (github-copilot-sourced) Copilot tool-call id of the form
//! `call_...|<long base64>` must be hashed into a bounded, Codex-safe
//! `fc_<shortHash(itemPart)>` item id when replayed to an openai-codex Responses
//! model. Exercised through the real `build_responses_payload` (rs-ai's
//! `convertResponsesMessages` equivalent) and the emitted `function_call` item.

#[cfg(test)]
mod tests {
    use crate::provider::responses::build_responses_payload;
    use crate::types::{Context, ContentBlock, Message, Model, ModelCost, Role, StreamOptions};
    use crate::utils::short_hash;
    use std::collections::HashMap;

    const COPILOT_RAW_TOOL_CALL_ID: &str = "call_4VnzVawQXPB9MgYib7CiQFEY|I9b95oN1wD/cHXKTw3PpRkL6KkCtzTJhUxMouMWYwHeTo2j3htzfSk7YPx2vifiIM4g3A8XXyOj8q4Bt6SLUG7gqY1E3ELkrkVQNHglRfUmWj84lqxJY+Puieb3VKyX0FB+83TUzn91cDMF/4gzt990IzqVrc+nIb9RRscRD070Du16q1glydVjWR0SBJsE6TbY/esOjFpqplogQqrajm1eI++f3eLi73R6q7hVusY0QbeFySVxABCjhN0lXB04caBe1rzHjYzul6MAXj7uq+0r17VLq+yrtyYhN12wkmFqHeqTyEei6EFPbMy24Nc+IbJlkP0OCg02W+gOnyBFcbi2ctvJFSOhSjt1CqBdqCnnhwUqXjbWiT0wh3DmLScRgTHmGkaI+oAcQQjfic65nxj+TnEkReA==";

    fn codex_model() -> Model {
        Model {
            id: "gpt-5.5".into(),
            name: "GPT-5.5".into(),
            api: "openai-codex-responses".into(),
            provider: "openai-codex".into(),
            base_url: "https://chatgpt.com/backend-api/codex".into(),
            reasoning: true,
            thinking_level_map: None,
            input: vec!["text".into()],
            cost: ModelCost::default(),
            context_window: 256000,
            max_tokens: 16384,
            headers: None,
            api_key: None,
            compat: Default::default(),
        }
    }

    fn assistant_with_foreign_toolcall() -> Message {
        let mut args = HashMap::new();
        args.insert("path".to_string(), serde_json::json!("src/styles/app.css"));
        Message {
            role: Role::Assistant,
            content: vec![ContentBlock::ToolCall {
                id: COPILOT_RAW_TOOL_CALL_ID.into(),
                name: "edit".into(),
                arguments: args,
                thought_signature: None,
            }],
            timestamp: 0,
            api: Some("openai-responses".into()),
            provider: Some("github-copilot".into()),
            model: Some("gpt-5.5".into()),
            response_id: None, response_model: None, diagnostics: Vec::new(), usage: None,
            stop_reason: Some(crate::types::StopReason::ToolUse), error_message: None,
            tool_call_id: None, tool_name: None, is_error: false, details: None,
        }
    }

    fn tool_result() -> Message {
        Message {
            role: Role::ToolResult,
            content: vec![ContentBlock::Text { text: "ok".into(), text_signature: None }],
            timestamp: 0,
            api: None, provider: None, model: None, response_id: None,
            response_model: None, diagnostics: Vec::new(), usage: None,
            stop_reason: None, error_message: None,
            tool_call_id: Some(COPILOT_RAW_TOOL_CALL_ID.into()), tool_name: Some("edit".into()),
            is_error: false, details: None,
        }
    }

    #[test]
    fn hashes_foreign_copilot_tool_item_ids_into_bounded_codex_safe_fc_hash_shape() {
        let model = codex_model();
        let ctx = Context {
            system_prompt: Some("You are concise.".into()),
            tools: Vec::new(),
            messages: vec![
                Message {
                    role: Role::User,
                    content: vec![ContentBlock::Text { text: "Use the tool.".into(), text_signature: None }],
                    timestamp: 0,
                    api: None, provider: None, model: None, response_id: None,
                    response_model: None, diagnostics: Vec::new(), usage: None,
                    stop_reason: None, error_message: None,
                    tool_call_id: None, tool_name: None, is_error: false, details: None,
                },
                assistant_with_foreign_toolcall(),
                tool_result(),
            ],
        };

        let payload = build_responses_payload(&model, &ctx, &StreamOptions::default());
        let input = payload["input"].as_array().expect("input array");
        let function_call = input.iter()
            .find(|item| item.get("type").and_then(|t| t.as_str()) == Some("function_call"))
            .expect("a function_call item");

        let item_part = COPILOT_RAW_TOOL_CALL_ID.split('|').nth(1).unwrap();
        let expected_item_id = format!("fc_{}", short_hash(item_part));
        let id = function_call["id"].as_str().expect("function_call id");

        assert_eq!(id, expected_item_id);
        assert!(id.len() <= 64, "item id must be bounded to 64 chars: {id}");
        assert!(
            id.strip_prefix("fc_").map(|rest| rest.chars().all(|c| c.is_ascii_alphanumeric())).unwrap_or(false),
            "item id must match ^fc_[A-Za-z0-9]+$: {id}"
        );
    }
}
