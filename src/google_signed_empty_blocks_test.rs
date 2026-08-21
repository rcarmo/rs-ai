//! v0.84.0 Google history replay regressions for signed empty blocks.

#[cfg(test)]
mod tests {
    use crate::provider::google::build_google_payload_public;
    use crate::types::{
        ContentBlock, Context, Message, Model, ModelCost, Role, StopReason, StreamOptions,
    };

    fn model(id: &str) -> Model {
        Model {
            id: id.into(),
            name: id.into(),
            api: "google-generative-ai".into(),
            provider: "google".into(),
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

    fn assistant(blocks: Vec<ContentBlock>, model_id: &str) -> Message {
        Message {
            role: Role::Assistant,
            content: blocks,
            timestamp: 0,
            api: Some("google-generative-ai".into()),
            provider: Some("google".into()),
            model: Some(model_id.into()),
            response_id: None,
            response_model: None,
            diagnostics: Vec::new(),
            usage: None,
            stop_reason: Some(StopReason::Stop),
            deferred: None,
            error_message: None,
            raw_stop_reason: None,
            end_turn: None,
            tool_call_id: None,
            tool_name: None,
            is_error: false,
            details: None,
            added_tool_names: Vec::new(),
        }
    }

    #[test]
    fn preserves_same_model_empty_text_and_thinking_blocks_when_signed() {
        let m = model("gemini-3-pro-preview");
        let p = build_google_payload_public(
            &m,
            &Context {
                system_prompt: None,
                messages: vec![assistant(
                    vec![
                        ContentBlock::Text {
                            text: "".into(),
                            text_signature: Some("AAAAAAAAAAAAAAAAAAAAAA==".into()),
                        },
                        ContentBlock::Thinking {
                            thinking: "".into(),
                            thinking_signature: Some("BBBBBBBBBBBBBBBBBBBBBB==".into()),
                            redacted: false,
                        },
                    ],
                    "gemini-3-pro-preview",
                )],
                tools: vec![],
            },
            &StreamOptions::default(),
        );
        let parts = p["contents"][0]["parts"].as_array().unwrap();
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0]["text"], "");
        assert_eq!(parts[0]["thoughtSignature"], "AAAAAAAAAAAAAAAAAAAAAA==");
        assert_eq!(parts[1]["text"], "");
        assert_eq!(parts[1]["thought"], true);
        assert_eq!(parts[1]["thoughtSignature"], "BBBBBBBBBBBBBBBBBBBBBB==");
    }

    #[test]
    fn drops_cross_model_empty_signed_thinking_because_signature_is_unusable() {
        let m = model("gemini-3-pro-preview");
        let p = build_google_payload_public(
            &m,
            &Context {
                system_prompt: None,
                messages: vec![assistant(
                    vec![ContentBlock::Thinking {
                        thinking: "".into(),
                        thinking_signature: Some("BBBBBBBBBBBBBBBBBBBBBB==".into()),
                        redacted: false,
                    }],
                    "other-model",
                )],
                tools: vec![],
            },
            &StreamOptions::default(),
        );
        assert_eq!(p["contents"].as_array().unwrap().len(), 0);
    }
}
