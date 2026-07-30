//! Adaptation of @go-ai local codex request-shape parity test
//! (`inference/provider/openaicodex/codex_request_test.go`:
//! `TestBuildCodexRequestMatchesPiaiShape`) into idiomatic Rust.
//!
//! Snapshots the OpenAI-Codex `response.create` request body against the
//! canonical pi-ai shape: stream/store, instructions, prompt_cache_key,
//! tool_choice, parallel_tool_calls, include, reasoning{effort,summary},
//! text{verbosity}, and the user-first input ordering.

#[cfg(test)]
mod tests {
    use crate::provider::codex::build_codex_payload;
    use crate::types::{
        ContentBlock, Context, Message, Model, ModelCost, Role, StreamOptions, ThinkingLevel, Tool,
    };

    fn codex_model() -> Model {
        Model {
            id: "gpt-5.4-mini".into(),
            name: "GPT-5.4 mini".into(),
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
    fn build_codex_request_matches_piai_shape() {
        let model = codex_model();
        let ctx = Context {
            system_prompt: Some("You are a helpful assistant.".into()),
            tools: vec![Tool {
                name: "shell".into(),
                description: "run shell".into(),
                parameters: serde_json::json!({"type": "object"}),
                constrained_sampling: None,
            }],
            messages: vec![Message {
                role: Role::User,
                content: vec![ContentBlock::Text {
                    text: "hi".into(),
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
            }],
        };
        let opts = StreamOptions {
            session_id: Some("sess-123".into()),
            reasoning: Some(ThinkingLevel::Minimal),
            reasoning_summary: Some("detailed".into()),
            text_verbosity: Some("medium".into()),
            ..Default::default()
        };

        let req = build_codex_payload(&model, &ctx, &opts);

        assert_eq!(req["stream"], serde_json::json!(true));
        assert_eq!(req["store"], serde_json::json!(false));
        assert_eq!(
            req["instructions"],
            serde_json::json!("You are a helpful assistant.")
        );
        assert_eq!(req["prompt_cache_key"], serde_json::json!("sess-123"));
        assert_eq!(req["tool_choice"], serde_json::json!("auto"));
        assert_eq!(req["parallel_tool_calls"], serde_json::json!(true));
        assert_eq!(
            req["include"],
            serde_json::json!(["reasoning.encrypted_content"])
        );

        // reasoning.effort is the clamped/mapped level ("minimal" with no map);
        // summary is the caller override.
        let reasoning = &req["reasoning"];
        assert_eq!(reasoning["effort"], serde_json::json!("minimal"));
        assert_eq!(reasoning["summary"], serde_json::json!("detailed"));

        // text.verbosity is the caller override.
        assert_eq!(req["text"]["verbosity"], serde_json::json!("medium"));

        // The system prompt is hoisted to `instructions` and stripped from input;
        // the first input item is the user message.
        let input = req["input"].as_array().expect("input array");
        assert!(!input.is_empty());
        assert_eq!(input[0]["role"], serde_json::json!("user"));
        assert!(
            input.iter().all(|m| !matches!(
                m.get("role").and_then(|r| r.as_str()),
                Some("system") | Some("developer")
            )),
            "system/developer roles must be stripped from codex input"
        );

        // Codex tool definitions carry strict: null (not false).
        let tools = req["tools"].as_array().expect("tools array");
        assert_eq!(tools[0]["strict"], serde_json::Value::Null);
    }
}
