//! Adaptation of @go-ai `TestProcessSSEStreamAttachesPendingEncryptedReasoningDetails`
//! (`inference/provider/openai/openai_payload_test.go`) into idiomatic Rust.
//!
//! An openai-completions stream that emits an encrypted `reasoning_details`
//! entry keyed by a tool-call id must attach that encrypted blob as the matching
//! tool call's `thought_signature` (order-independent: the detail can arrive
//! before the tool call). The signature is an opaque replay blob, so we assert
//! its decoded fields rather than a byte-exact key order (serde sorts object
//! keys; go-ai preserves insertion order).

#[cfg(test)]
mod tests {
    use crate::events::Event;
    use crate::provider::openai::stream_openai;
    use crate::types::{ContentBlock, Context, Message, Model, ModelCost, Role, StreamOptions};
    use tokio_stream::StreamExt;
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn model(base_url: &str) -> Model {
        Model {
            id: "requested-model".into(),
            name: "M".into(),
            api: "openai-completions".into(),
            provider: "openai".into(),
            base_url: base_url.into(),
            reasoning: true,
            thinking_level_map: None,
            input: vec!["text".into()],
            cost: ModelCost::default(),
            context_window: 128000,
            max_tokens: 4096,
            sampling_params: None,
            headers: None,
            api_key: Some("k".into()),
            compat: Default::default(),
        }
    }

    fn ctx() -> Context {
        Context {
            system_prompt: None,
            tools: Vec::new(),
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
                deferred: None,
                error_message: None,
                raw_stop_reason: None,
                end_turn: None,
                tool_call_id: None,
                tool_name: None,
                is_error: false,
                details: None,
                added_tool_names: Vec::new(),
            }],
        }
    }

    #[tokio::test]
    async fn attaches_pending_encrypted_reasoning_details_to_tool_call() {
        let body = concat!(
            "data: {\"choices\":[{\"index\":0,\"delta\":{\"reasoning_details\":[{\"type\":\"reasoning.encrypted\",\"id\":\"call_1\",\"data\":\"secret\"}]}}]}\n\n",
            "data: {\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_1\",\"type\":\"function\",\"function\":{\"name\":\"lookup\",\"arguments\":\"{\\\"q\\\":\\\"hi\\\"}\"}}]}}]}\n\n",
            "data: {\"choices\":[{\"index\":0,\"finish_reason\":\"tool_calls\",\"delta\":{}}]}\n\n",
            "data: [DONE]\n\n",
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
        let c = ctx();
        let opts = StreamOptions::default();
        let mut stream = stream_openai(&m, &c, &opts);
        let mut done_msg = None;
        while let Some(evt) = stream.next().await {
            if let Event::Done { message, .. } = evt {
                done_msg = Some(message);
            }
        }
        let msg = done_msg.expect("a Done event");
        let thinking_signature = msg
            .content
            .iter()
            .find_map(|block| match block {
                ContentBlock::Thinking {
                    thinking_signature, ..
                } => thinking_signature.as_deref(),
                _ => None,
            })
            .expect("encrypted reasoning is also preserved as replay reasoning_details");
        let preserved: serde_json::Value =
            serde_json::from_str(thinking_signature).expect("thinking signature is JSON");
        assert_eq!(
            preserved,
            serde_json::json!([{ "type": "reasoning.encrypted", "id": "call_1", "data": "secret" }])
        );

        let tool_call = msg
            .content
            .iter()
            .find(|block| matches!(block, ContentBlock::ToolCall { .. }))
            .expect("expected a tool-call block");
        match tool_call {
            ContentBlock::ToolCall {
                name,
                thought_signature,
                ..
            } => {
                assert_eq!(name, "lookup");
                let sig = thought_signature
                    .as_deref()
                    .expect("encrypted reasoning attached as thought_signature");
                // Opaque blob: assert decoded fields (key order is serializer-defined).
                let v: serde_json::Value = serde_json::from_str(sig).expect("signature is JSON");
                assert_eq!(v["type"], serde_json::json!("reasoning.encrypted"));
                assert_eq!(v["id"], serde_json::json!("call_1"));
                assert_eq!(v["data"], serde_json::json!("secret"));
            }
            other => panic!("expected toolCall, got {other:?}"),
        }
    }
}
