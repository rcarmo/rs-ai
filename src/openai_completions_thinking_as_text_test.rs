//! Test-for-test port of upstream `test/openai-completions-thinking-as-text.test.ts`
//! (`@earendil-works/pi-ai` v0.80.2).
//!
//! For a same-model replay where `requiresThinkingAsText` is set, thinking blocks
//! serialize as assistant text parts. Cases 1-2 assert the serialized assistant
//! message via the real payload builder; case 3 confirms it reaches the endpoint
//! with the same body and yields a terminal Done.

#[cfg(test)]
mod tests {
    use crate::compat::detect_compat;
    use crate::events::Event;
    use crate::provider::openai::{build_payload, stream_openai};
    use crate::types::{
        ContentBlock, Context, Message, Model, ModelCompat, ModelCost, Role, StopReason,
        StreamOptions,
    };
    use serde_json::{Value, json};
    use tokio_stream::StreamExt;
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn model(base_url: &str) -> Model {
        Model {
            id: "repro-model".into(),
            name: "Repro Model".into(),
            api: "openai-completions".into(),
            provider: "repro-provider".into(),
            base_url: base_url.into(),
            reasoning: true,
            thinking_level_map: None,
            input: vec!["text".into()],
            cost: ModelCost::default(),
            context_window: 128000,
            max_tokens: 4096,
            sampling_params: None,
            headers: None,
            api_key: None,
            compat: ModelCompat {
                requires_thinking_as_text: Some(true),
                thinking_format: Some("openai".into()),
                ..Default::default()
            },
        }
    }

    fn assistant(content: Vec<ContentBlock>) -> Message {
        Message {
            role: Role::Assistant,
            content,
            timestamp: 2,
            api: Some("openai-completions".into()),
            provider: Some("repro-provider".into()),
            model: Some("repro-model".into()),
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

    fn user(text: &str, ts: i64) -> Message {
        Message {
            role: Role::User,
            content: vec![ContentBlock::Text {
                text: text.into(),
                text_signature: None,
            }],
            timestamp: ts,
            api: None,
            provider: None,
            model: None,
            response_id: None,
            response_model: None,
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
        }
    }

    fn ctx(a: Message) -> Context {
        Context {
            system_prompt: None,
            tools: Vec::new(),
            messages: vec![user("hello", 1), a, user("continue", 3)],
        }
    }

    fn assistant_message(p: &Value) -> &Value {
        &p["messages"][1]
    }

    #[test]
    fn serializes_same_model_thinking_plus_text_replay_as_assistant_text_parts() {
        let m = model("http://127.0.0.1:1");
        let c = ctx(assistant(vec![
            ContentBlock::Thinking {
                thinking: "internal reasoning".into(),
                thinking_signature: None,
                redacted: false,
            },
            ContentBlock::Text {
                text: "visible answer".into(),
                text_signature: None,
            },
        ]));
        let p = build_payload(&m, &c, &StreamOptions::default(), &detect_compat(&m));
        assert_eq!(
            *assistant_message(&p),
            json!({
                "role": "assistant",
                "content": [
                    {"type": "text", "text": "internal reasoning"},
                    {"type": "text", "text": "visible answer"},
                ],
            })
        );
    }

    #[test]
    fn serializes_same_model_thinking_only_replay_as_assistant_text_parts() {
        let m = model("http://127.0.0.1:1");
        let c = ctx(assistant(vec![ContentBlock::Thinking {
            thinking: "internal reasoning".into(),
            thinking_signature: None,
            redacted: false,
        }]));
        let p = build_payload(&m, &c, &StreamOptions::default(), &detect_compat(&m));
        assert_eq!(
            *assistant_message(&p),
            json!({
                "role": "assistant",
                "content": [{"type": "text", "text": "internal reasoning"}],
            })
        );
    }

    #[tokio::test]
    async fn reaches_the_endpoint_when_replay_contains_both_thinking_and_text() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_string(concat!(
                    "data: {\"id\":\"chatcmpl-repro\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":\"ok\"},\"finish_reason\":null}]}\n\n",
                    "data: {\"id\":\"chatcmpl-repro\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":1,\"completion_tokens\":1}}\n\n",
                    "data: [DONE]\n\n",
                )))
            .mount(&server)
            .await;
        let mut m = model(&server.uri());
        m.api_key = Some("test-key".into());
        let c = ctx(assistant(vec![
            ContentBlock::Thinking {
                thinking: "internal reasoning".into(),
                thinking_signature: None,
                redacted: false,
            },
            ContentBlock::Text {
                text: "visible answer".into(),
                text_signature: None,
            },
        ]));
        let opts = StreamOptions::default();
        let mut stream = stream_openai(&m, &c, &opts);
        let mut last = None;
        while let Some(evt) = stream.next().await {
            last = Some(evt);
        }
        assert!(
            matches!(last, Some(Event::Done { .. })),
            "terminal event is Done"
        );

        let reqs = server.received_requests().await.unwrap();
        let body: Value = serde_json::from_slice(&reqs.last().unwrap().body).unwrap();
        assert_eq!(
            body["messages"][1],
            json!({
                "role": "assistant",
                "content": [
                    {"type": "text", "text": "internal reasoning"},
                    {"type": "text", "text": "visible answer"},
                ],
            })
        );
    }
}
