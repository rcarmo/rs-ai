//! Test-for-test port of upstream `test/openai-completions-reasoning-details.test.ts`
//! (`@earendil-works/pi-ai` v0.80.2).
//!
//! An encrypted `reasoning_details` entry arriving before its matching tool call
//! is attached as that tool call's `thought_signature`, and replaying the
//! assistant message serializes it back into the request `reasoning_details`.
//! The signature is an opaque blob (serde sorts object keys vs upstream's
//! insertion order), so we assert the decoded value, not a byte-exact string.

#[cfg(test)]
mod tests {
    use crate::compat::detect_compat;
    use crate::events::Event;
    use crate::provider::openai::{build_payload, stream_openai};
    use crate::types::{ContentBlock, Context, Model, ModelCost, StreamOptions, Tool};
    use serde_json::{Value, json};
    use tokio_stream::StreamExt;
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn model(base_url: &str) -> Model {
        Model {
            id: "google/gemini-test".into(),
            name: "Gemini Test".into(),
            api: "openai-completions".into(),
            provider: "openrouter".into(),
            base_url: base_url.into(),
            reasoning: true,
            thinking_level_map: None,
            input: vec!["text".into()],
            cost: ModelCost::default(),
            context_window: 100000,
            max_tokens: 4096,
            sampling_params: None,
            headers: None,
            api_key: Some("test".into()),
            compat: Default::default(),
        }
    }

    fn read_tool() -> Tool {
        Tool {
            name: "read".into(),
            description: "Read a file".into(),
            parameters: json!({"type": "object", "properties": {"path": {"type": "string"}}}),
            constrained_sampling: None,
        }
    }

    fn detail() -> Value {
        json!({"type": "reasoning.encrypted", "id": "call_1", "data": "encrypted-signature"})
    }

    #[test]
    fn replays_thinking_signature_reasoning_details_sequence() {
        let m = model("https://example.invalid/v1");
        let details = json!([
            {"type":"reasoning.text","text":"Chain","signature":"sig","id":"txt_1","format":"gemini","index":0},
            {"type":"reasoning.summary","summary":"Summary","id":"sum_1","format":"gemini","index":1}
        ]);
        let assistant = crate::types::Message {
            role: crate::types::Role::Assistant,
            content: vec![
                ContentBlock::Thinking {
                    thinking: "Chain\nSummary".into(),
                    thinking_signature: Some(details.to_string()),
                    redacted: None,
                },
                ContentBlock::Text {
                    text: "done".into(),
                    text_signature: None,
                },
            ],
            timestamp: 0,
            api: Some(m.api.clone()),
            provider: Some(m.provider.clone()),
            model: Some(m.id.clone()),
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
        };
        let payload = build_payload(
            &m,
            &Context {
                system_prompt: None,
                tools: Vec::new(),
                messages: vec![assistant],
            },
            &StreamOptions::default(),
            &detect_compat(&m),
        );
        let assistant_payload = payload["messages"]
            .as_array()
            .unwrap()
            .iter()
            .find(|msg| msg.get("role").and_then(|r| r.as_str()) == Some("assistant"))
            .expect("assistant payload");
        assert_eq!(assistant_payload["reasoning_details"], details);
        assert!(assistant_payload.get("reasoning").is_none());
    }

    #[tokio::test]
    async fn preserves_streamed_text_and_summary_reasoning_details_on_thinking() {
        let details = json!([
            {"type":"reasoning.text","text":"Chain","signature":"sig","id":"txt_1","format":"gemini","index":0},
            {"type":"reasoning.summary","summary":"Summary","id":"sum_1","format":"gemini","index":1}
        ]);
        let body = format!(
            "data: {{\"id\":\"chatcmpl-test\",\"model\":\"google/gemini-test\",\"choices\":[{{\"index\":0,\"delta\":{{\"reasoning_content\":\"Chain\",\"reasoning_details\":{details}}},\"finish_reason\":null}}]}}\n\n\
             data: {{\"id\":\"chatcmpl-test\",\"model\":\"google/gemini-test\",\"choices\":[{{\"index\":0,\"delta\":{{\"content\":\"done\"}},\"finish_reason\":null}}]}}\n\n\
             data: {{\"id\":\"chatcmpl-test\",\"model\":\"google/gemini-test\",\"choices\":[{{\"index\":0,\"delta\":{{}},\"finish_reason\":\"stop\"}}],\"usage\":{{\"prompt_tokens\":1,\"completion_tokens\":1}}}}\n\n\
             data: [DONE]\n\n"
        );
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_string(body),
            )
            .mount(&server)
            .await;
        let m = model(&server.uri());
        let ctx = Context {
            system_prompt: None,
            tools: Vec::new(),
            messages: Vec::new(),
        };
        let opts = StreamOptions::default();
        let mut stream = stream_openai(&m, &ctx, &opts);
        let mut assistant_msg = None;
        while let Some(evt) = stream.next().await {
            if let Event::Done { message, .. } = evt {
                assistant_msg = Some(message);
            }
        }
        let assistant = assistant_msg.expect("Done");
        let thinking = assistant
            .content
            .iter()
            .find_map(|b| match b {
                ContentBlock::Thinking {
                    thinking,
                    thinking_signature,
                    ..
                } => Some((thinking, thinking_signature)),
                _ => None,
            })
            .expect("thinking block");
        assert_eq!(thinking.0, "Chain");
        assert_eq!(
            serde_json::from_str::<Value>(thinking.1.as_deref().unwrap()).unwrap(),
            details
        );
    }

    #[tokio::test]
    async fn merges_adjacent_text_and_summary_reasoning_details_before_replay() {
        let expected = json!([
            {"type":"reasoning.text","text":"The user wants the time.","index":0,"signature":"sha256:text-signature","format":"openai-responses-v1"},
            {"type":"reasoning.summary","summary":"Looked up time.","index":0,"format":"openai-responses-v1"},
            detail(),
            {"type":"reasoning.summary","summary":"After encrypted block.","index":0,"format":"openai-responses-v1"}
        ]);
        let body = concat!(
            "data: {\"id\":\"chatcmpl-test\",\"model\":\"google/gemini-test\",\"choices\":[{\"index\":0,\"delta\":{\"reasoning_details\":[{\"type\":\"reasoning.text\",\"text\":\"The\",\"index\":0}]},\"finish_reason\":null}]}\n\n",
            "data: {\"id\":\"chatcmpl-test\",\"model\":\"google/gemini-test\",\"choices\":[{\"index\":0,\"delta\":{\"reasoning_details\":[{\"type\":\"reasoning.text\",\"text\":\" user wants the time.\",\"signature\":\"sha256:text-signature\",\"format\":\"openai-responses-v1\",\"index\":0}]},\"finish_reason\":null}]}\n\n",
            "data: {\"id\":\"chatcmpl-test\",\"model\":\"google/gemini-test\",\"choices\":[{\"index\":0,\"delta\":{\"reasoning_details\":[{\"type\":\"reasoning.summary\",\"summary\":\"Looked\",\"index\":0}]},\"finish_reason\":null}]}\n\n",
            "data: {\"id\":\"chatcmpl-test\",\"model\":\"google/gemini-test\",\"choices\":[{\"index\":0,\"delta\":{\"reasoning_details\":[{\"type\":\"reasoning.summary\",\"summary\":\" up time.\",\"format\":\"openai-responses-v1\",\"index\":0}]},\"finish_reason\":null}]}\n\n",
            "data: {\"id\":\"chatcmpl-test\",\"model\":\"google/gemini-test\",\"choices\":[{\"index\":0,\"delta\":{\"reasoning_details\":[{\"type\":\"reasoning.encrypted\",\"id\":\"call_1\",\"data\":\"encrypted-signature\"}]},\"finish_reason\":null}]}\n\n",
            "data: {\"id\":\"chatcmpl-test\",\"model\":\"google/gemini-test\",\"choices\":[{\"index\":0,\"delta\":{\"reasoning_details\":[{\"type\":\"reasoning.summary\",\"summary\":\"After encrypted block.\",\"format\":\"openai-responses-v1\",\"index\":0}]},\"finish_reason\":null}]}\n\n",
            "data: {\"id\":\"chatcmpl-test\",\"model\":\"google/gemini-test\",\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_1\",\"type\":\"function\",\"function\":{\"name\":\"read\",\"arguments\":\"{\\\"path\\\":\\\"README.md\\\"}\"}}]},\"finish_reason\":null}]}\n\n",
            "data: {\"id\":\"chatcmpl-test\",\"model\":\"google/gemini-test\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"tool_calls\"}]}\n\n",
            "data: [DONE]\n\n"
        );
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_string(body),
            )
            .mount(&server)
            .await;
        let m = model(&server.uri());
        let ctx = Context {
            system_prompt: None,
            tools: vec![read_tool()],
            messages: Vec::new(),
        };
        let opts = StreamOptions::default();
        let mut stream = stream_openai(&m, &ctx, &opts);
        let mut assistant_msg = None;
        while let Some(evt) = stream.next().await {
            if let Event::Done { message, .. } = evt {
                assistant_msg = Some(message);
            }
        }
        let assistant = assistant_msg.expect("Done");
        let thinking = assistant
            .content
            .iter()
            .find_map(|b| match b {
                ContentBlock::Thinking {
                    thinking_signature, ..
                } => thinking_signature.as_ref(),
                _ => None,
            })
            .expect("thinking signature");
        assert_eq!(serde_json::from_str::<Value>(thinking).unwrap(), expected);
        let payload = build_payload(
            &m,
            &Context {
                system_prompt: None,
                tools: vec![read_tool()],
                messages: vec![assistant],
            },
            &StreamOptions::default(),
            &detect_compat(&m),
        );
        let assistant_payload = payload["messages"]
            .as_array()
            .unwrap()
            .iter()
            .find(|msg| msg.get("role").and_then(|r| r.as_str()) == Some("assistant"))
            .unwrap();
        assert_eq!(assistant_payload["reasoning_details"], expected);
    }

    #[tokio::test]
    async fn preserves_reasoning_details_that_arrive_before_their_matching_tool_call() {
        // Stream: reasoning_details, then the matching tool call, then finish.
        let body = concat!(
            "data: {\"id\":\"chatcmpl-test\",\"model\":\"google/gemini-test\",\"choices\":[{\"index\":0,\"delta\":{\"reasoning_details\":[{\"type\":\"reasoning.encrypted\",\"id\":\"call_1\",\"data\":\"encrypted-signature\"}]},\"finish_reason\":null}]}\n\n",
            "data: {\"id\":\"chatcmpl-test\",\"model\":\"google/gemini-test\",\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_1\",\"type\":\"function\",\"function\":{\"name\":\"read\",\"arguments\":\"{\\\"path\\\":\\\"README.md\\\"}\"}}]},\"finish_reason\":null}]}\n\n",
            "data: {\"id\":\"chatcmpl-test\",\"model\":\"google/gemini-test\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"tool_calls\"}],\"usage\":{\"prompt_tokens\":1,\"completion_tokens\":1,\"prompt_tokens_details\":{\"cached_tokens\":0},\"completion_tokens_details\":{\"reasoning_tokens\":0}}}\n\n",
            "data: [DONE]\n\n",
        ).to_string();
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_string(body),
            )
            .mount(&server)
            .await;
        let m = model(&server.uri());
        let ctx = Context {
            system_prompt: None,
            tools: vec![read_tool()],
            messages: Vec::new(),
        };
        let opts = StreamOptions::default();
        let mut stream = stream_openai(&m, &ctx, &opts);
        let mut assistant_msg = None;
        while let Some(evt) = stream.next().await {
            if let Event::Done { message, .. } = evt {
                assistant_msg = Some(message);
            }
        }
        let assistant = assistant_msg.expect("Done");

        // The tool call carries the encrypted reasoning as its signature.
        let tc = assistant
            .content
            .iter()
            .find(|b| matches!(b, ContentBlock::ToolCall { .. }))
            .expect("toolCall");
        match tc {
            ContentBlock::ToolCall {
                id,
                name,
                arguments,
                thought_signature,

                namespace: None,
            } => {
                assert_eq!(id, "call_1");
                assert_eq!(name, "read");
                assert_eq!(
                    arguments.get("path").and_then(|v| v.as_str()),
                    Some("README.md")
                );
                let sig: Value =
                    serde_json::from_str(thought_signature.as_deref().expect("signature")).unwrap();
                assert_eq!(sig, detail());
            }
            _ => unreachable!(),
        }

        // Replaying the assistant message serializes the signature back to reasoning_details.
        let replay_ctx = Context {
            system_prompt: None,
            tools: vec![read_tool()],
            messages: vec![assistant],
        };
        let payload = build_payload(
            &m,
            &replay_ctx,
            &StreamOptions::default(),
            &detect_compat(&m),
        );
        let assistant_payload = payload["messages"]
            .as_array()
            .unwrap()
            .iter()
            .find(|msg| msg.get("role").and_then(|r| r.as_str()) == Some("assistant"))
            .expect("assistant payload");
        assert_eq!(assistant_payload["reasoning_details"], json!([detail()]));
    }
}
