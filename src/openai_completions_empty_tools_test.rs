//! Test-for-test port of upstream `test/openai-completions-empty-tools.test.ts`
//! (`@earendil-works/pi-ai` v0.80.2) — the payload-shape cases.
//!
//! The Cloudflare-AI-Gateway client-construction cases (baseURL,
//! cf-aig-authorization / session-affinity default headers) are env-coupled and
//! assert the constructed HTTP client options rather than the request body; they
//! are covered separately by rs-ai's cloudflare URL/header resolution and are not
//! re-ported here (env-mutation races). The tools/max-tokens body cases are ported.

#[cfg(test)]
mod tests {
    use crate::compat::detect_compat;
    use crate::provider::openai::build_payload;
    use crate::registry::get_model;
    use crate::types::{Context, ContentBlock, Message, Model, Role, StopReason, StreamOptions};
    use serde_json::Value;
    use std::collections::HashMap;

    fn gpt_4o_mini() -> Model {
        get_model("openai", "gpt-4o-mini").expect("catalog gpt-4o-mini")
    }

    fn user(text: &str) -> Message {
        Message {
            role: Role::User,
            content: vec![ContentBlock::Text { text: text.into(), text_signature: None }],
            timestamp: 0, api: None, provider: None, model: None, response_id: None,
            response_model: None, diagnostics: Vec::new(), usage: None,
            stop_reason: None, error_message: None,
            tool_call_id: None, tool_name: None, is_error: false, details: None,
        }
    }

    fn payload(ctx: &Context, opts: &StreamOptions) -> Value {
        let m = gpt_4o_mini();
        build_payload(&m, ctx, opts, &detect_compat(&m))
    }

    #[test]
    fn omits_tools_field_when_context_tools_is_empty() {
        let ctx = Context { system_prompt: None, tools: Vec::new(), messages: vec![user("hi")] };
        let p = payload(&ctx, &StreamOptions::default());
        assert!(p.get("tools").is_none());
    }

    #[test]
    fn omits_tools_field_when_context_tools_is_undefined() {
        // rs-ai has no separate undefined/empty distinction; an absent tools list is empty.
        let ctx = Context { system_prompt: None, tools: Vec::new(), messages: vec![user("hi")] };
        let p = payload(&ctx, &StreamOptions::default());
        assert!(p.get("tools").is_none());
    }

    #[test]
    fn does_not_send_default_max_token_fields() {
        let ctx = Context { system_prompt: None, tools: Vec::new(), messages: vec![user("hi")] };
        let p = payload(&ctx, &StreamOptions::default());
        assert!(p.get("max_tokens").is_none());
        assert!(p.get("max_completion_tokens").is_none());
    }

    #[test]
    fn sends_explicit_max_tokens_as_max_completion_tokens() {
        let ctx = Context { system_prompt: None, tools: Vec::new(), messages: vec![user("hi")] };
        let opts = StreamOptions { max_tokens: Some(1234), ..Default::default() };
        let p = payload(&ctx, &opts);
        assert!(p.get("max_tokens").is_none());
        assert_eq!(p["max_completion_tokens"], serde_json::json!(1234));
    }

    #[test]
    fn still_emits_tools_empty_array_when_conversation_has_tool_history() {
        let assistant = Message {
            role: Role::Assistant,
            content: vec![ContentBlock::ToolCall { id: "t1".into(), name: "noop".into(), arguments: HashMap::new(), thought_signature: None }],
            timestamp: 0,
            api: Some("openai-completions".into()), provider: Some("openai".into()), model: Some("gpt-4o-mini".into()),
            response_id: None, response_model: None, diagnostics: Vec::new(), usage: None,
            stop_reason: Some(StopReason::ToolUse), error_message: None,
            tool_call_id: None, tool_name: None, is_error: false, details: None,
        };
        let tool_result = Message {
            role: Role::ToolResult,
            content: vec![ContentBlock::Text { text: "done".into(), text_signature: None }],
            timestamp: 0, api: None, provider: None, model: None, response_id: None,
            response_model: None, diagnostics: Vec::new(), usage: None,
            stop_reason: None, error_message: None,
            tool_call_id: Some("t1".into()), tool_name: Some("noop".into()), is_error: false, details: None,
        };
        let ctx = Context {
            system_prompt: None, tools: Vec::new(),
            messages: vec![user("use the tool"), assistant, tool_result],
        };
        let p = payload(&ctx, &StreamOptions::default());
        assert_eq!(p["tools"], serde_json::json!([]), "tool history must keep an empty tools array");
    }
}
