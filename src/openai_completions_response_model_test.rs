//! Test-for-test port of upstream `test/openai-completions-response-model.test.ts`
//! (`@earendil-works/pi-ai` v0.80.2).
//!
//! Router/virtual ids (OpenRouter `auto`) keep `model` pinned to the requested
//! id and surface the routed concrete id on `responseModel`; a chunk echoing the
//! requested id, or an empty/missing chunk model, leaves `responseModel` unset.

#[cfg(test)]
mod tests {
    use crate::events::Event;
    use crate::provider::openai::stream_openai;
    use crate::types::{ContentBlock, Context, Message, Model, ModelCost, Role, StreamOptions};
    use tokio_stream::StreamExt;
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn openrouter_auto(base_url: &str) -> Model {
        Model {
            id: "openrouter/auto".into(),
            name: "OpenRouter Auto".into(),
            api: "openai-completions".into(),
            provider: "openrouter".into(),
            base_url: base_url.into(),
            reasoning: false,
            thinking_level_map: None,
            input: vec!["text".into()],
            cost: ModelCost::default(),
            context_window: 200000,
            max_tokens: 8192,
            headers: None,
            api_key: Some("test".into()),
            compat: Default::default(),
        }
    }

    fn user_ctx() -> Context {
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
                error_message: None,
                tool_call_id: None,
                tool_name: None,
                is_error: false,
                details: None,
                added_tool_names: Vec::new(),
            }],
        }
    }

    async fn run(body: String) -> crate::types::Message {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_string(body),
            )
            .mount(&server)
            .await;
        let m = openrouter_auto(&server.uri());
        let c = user_ctx();
        let opts = StreamOptions::default();
        let mut stream = stream_openai(&m, &c, &opts);
        let mut msg = None;
        while let Some(evt) = stream.next().await {
            if let Event::Done { message, .. } = evt {
                msg = Some(message);
            }
        }
        msg.expect("Done")
    }

    #[tokio::test]
    async fn surfaces_routed_chunk_model_on_response_model_without_changing_model() {
        let body = concat!(
            "data: {\"id\":\"chatcmpl-1\",\"model\":\"anthropic/claude-opus-4.8\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"hi\"}}]}\n\n",
            "data: {\"id\":\"chatcmpl-1\",\"model\":\"anthropic/claude-opus-4.8\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":10,\"completion_tokens\":5,\"prompt_tokens_details\":{\"cached_tokens\":0},\"completion_tokens_details\":{\"reasoning_tokens\":0}}}\n\n",
            "data: [DONE]\n\n",
        ).to_string();
        let m = run(body).await;
        assert_eq!(m.model.as_deref(), Some("openrouter/auto"));
        assert_eq!(
            m.response_model.as_deref(),
            Some("anthropic/claude-opus-4.8")
        );
        assert_eq!(m.provider.as_deref(), Some("openrouter"));
        assert!(matches!(
            m.stop_reason,
            Some(crate::types::StopReason::Stop)
        ));
    }

    #[tokio::test]
    async fn leaves_response_model_unset_when_chunks_echo_the_requested_id() {
        let body = concat!(
            "data: {\"id\":\"chatcmpl-2\",\"model\":\"openrouter/auto\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"hi\"}}]}\n\n",
            "data: {\"id\":\"chatcmpl-2\",\"model\":\"openrouter/auto\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":1,\"completion_tokens\":1,\"prompt_tokens_details\":{\"cached_tokens\":0},\"completion_tokens_details\":{\"reasoning_tokens\":0}}}\n\n",
            "data: [DONE]\n\n",
        ).to_string();
        let m = run(body).await;
        assert_eq!(m.model.as_deref(), Some("openrouter/auto"));
        assert!(m.response_model.is_none());
    }

    #[tokio::test]
    async fn ignores_empty_or_missing_chunk_model() {
        let body = concat!(
            "data: {\"id\":\"chatcmpl-3\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"hi\"}}]}\n\n",
            "data: {\"id\":\"chatcmpl-3\",\"model\":\"\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"!\"}}]}\n\n",
            "data: {\"id\":\"chatcmpl-3\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":1,\"completion_tokens\":2,\"prompt_tokens_details\":{\"cached_tokens\":0},\"completion_tokens_details\":{\"reasoning_tokens\":0}}}\n\n",
            "data: [DONE]\n\n",
        ).to_string();
        let m = run(body).await;
        assert_eq!(m.model.as_deref(), Some("openrouter/auto"));
        assert!(m.response_model.is_none());
    }
}
