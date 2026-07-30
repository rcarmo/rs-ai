//! Test-for-test port of upstream `test/openai-responses-message-id.test.ts`
//! (`@earendil-works/pi-ai` v0.80.2).
//!
//! A cross-model assistant turn (anthropic-sourced) replayed to an openai-codex
//! Responses model must give each emitted text `message` item a unique fallback
//! id: `msg_pi_<msgIndex>` then `msg_pi_<msgIndex>_<blockIndex>`. The assistant
//! is message index 1 (after the user), so the two text blocks (the thinking
//! block downgrades to text cross-model) become `msg_pi_1` and `msg_pi_1_1`.

#[cfg(test)]
mod tests {
    use crate::provider::responses::build_responses_payload;
    use crate::types::{
        ContentBlock, Context, Message, Model, ModelCost, Role, StopReason, StreamOptions,
    };

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

    #[test]
    fn generates_unique_fallback_message_ids_for_multiple_text_blocks_in_one_turn() {
        let assistant = Message {
            role: Role::Assistant,
            content: vec![
                ContentBlock::Thinking {
                    thinking: "private reasoning".into(),
                    thinking_signature: None,
                    redacted: false,
                },
                ContentBlock::Text {
                    text: "visible answer".into(),
                    text_signature: None,
                },
            ],
            timestamp: 0,
            api: Some("anthropic-messages".into()),
            provider: Some("anthropic".into()),
            model: Some("claude-opus-4-8".into()),
            response_id: None,
            response_model: None,
            diagnostics: Vec::new(),
            usage: None,
            stop_reason: Some(StopReason::Stop),
            error_message: None,
            raw_stop_reason: None,
            tool_call_id: None,
            tool_name: None,
            is_error: false,
            details: None,
            added_tool_names: Vec::new(),
        };
        let ctx = Context {
            system_prompt: Some("You are concise.".into()),
            tools: Vec::new(),
            messages: vec![
                Message {
                    role: Role::User,
                    content: vec![ContentBlock::Text {
                        text: "hello".into(),
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
            ],
        };

        let payload = build_responses_payload(&codex_model(), &ctx, &StreamOptions::default());
        let input = payload["input"].as_array().expect("input array");
        let message_ids: Vec<String> = input
            .iter()
            .filter(|item| item.get("type").and_then(|t| t.as_str()) == Some("message"))
            .filter_map(|item| {
                item.get("id")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            })
            .collect();

        assert_eq!(
            message_ids,
            vec!["msg_pi_1".to_string(), "msg_pi_1_1".to_string()]
        );
        let unique: std::collections::HashSet<_> = message_ids.iter().collect();
        assert_eq!(
            unique.len(),
            message_ids.len(),
            "fallback message ids must be unique"
        );
    }
}
