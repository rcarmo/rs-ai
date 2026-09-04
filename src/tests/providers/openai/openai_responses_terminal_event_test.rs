//! Test-for-test port of upstream `test/openai-responses-terminal-event.test.ts`
//! (`@earendil-works/pi-ai` v0.80.2).
//!
//! The Responses stream must end on a terminal event: a stream that EOFs early
//! errors with "OpenAI Responses stream ended before a terminal response event";
//! completed -> stop, incomplete -> length, failed -> the provider error. Driven
//! through `stream_responses` + a wiremock SSE endpoint; rs-ai conveys the
//! terminal error via the event, so the helper maps it to (reason, error, msg).

#[cfg(test)]
mod tests {
    use crate::events::Event;
    use crate::provider::responses::stream_responses;
    use crate::types::{
        ContentBlock, Context, Message, Model, ModelCost, Role, StopReason, StreamOptions,
    };
    use tokio_stream::StreamExt;
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn model(base_url: &str) -> Model {
        Model {
            id: "gpt-5-mini".into(),
            name: "GPT-5 Mini".into(),
            api: "openai-responses".into(),
            provider: "openai".into(),
            base_url: base_url.into(),
            reasoning: true,
            thinking_level_map: None,
            input: vec!["text".into()],
            cost: ModelCost::default(),
            context_window: 400000,
            max_tokens: 128000,
            sampling_params: None,
            headers: None,
            api_key: Some("test".into()),
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
            }],
        }
    }

    async fn run(body: String) -> (StopReason, Option<String>, Message) {
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
        let mut stream = stream_responses(&m, &c, &opts);
        let mut out = None;
        while let Some(evt) = stream.next().await {
            match evt {
                Event::Done { reason, message } => {
                    out = Some((reason, message.error_message.clone(), message))
                }
                Event::Error {
                    reason,
                    error,
                    message,
                } => {
                    out = Some((
                        reason,
                        Some(error.to_string()),
                        message.unwrap_or_else(default_msg),
                    ))
                }
                _ => {}
            }
        }
        out.expect("a terminal event")
    }

    fn default_msg() -> Message {
        Message {
            role: Role::Assistant,
            content: Vec::new(),
            timestamp: 0,
            api: None,
            provider: None,
            model: None,
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
        }
    }

    #[tokio::test]
    async fn rejects_streams_that_end_before_a_terminal_response_event() {
        let body = concat!(
            "data: {\"type\":\"response.created\",\"response\":{\"id\":\"resp_early_eof\"}}\n\n",
            "data: {\"type\":\"response.output_item.added\",\"output_index\":0,\"item\":{\"type\":\"reasoning\",\"id\":\"rs_early_eof\",\"summary\":[]}}\n\n",
            "data: {\"type\":\"response.reasoning_text.delta\",\"output_index\":0,\"content_index\":0,\"item_id\":\"rs_early_eof\",\"delta\":\"partial reasoning before the stream ends\"}\n\n",
        ).to_string();
        let (reason, err, _m) = run(body).await;
        assert!(matches!(reason, StopReason::Error));
        assert_eq!(
            err.as_deref(),
            Some("OpenAI Responses stream ended before a terminal response event")
        );
    }

    #[tokio::test]
    async fn finalizes_completed_terminal_events_as_stop() {
        let body = concat!(
            "data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_completed\",\"status\":\"completed\",\"usage\":{\"input_tokens\":20,\"output_tokens\":7,\"total_tokens\":27,\"input_tokens_details\":{\"cached_tokens\":2}}}}\n\n",
            "data: [DONE]\n\n",
        ).to_string();
        let (reason, _err, m) = run(body).await;
        assert!(matches!(reason, StopReason::Stop));
        assert_eq!(m.response_id.as_deref(), Some("resp_completed"));
        let u = m.usage.unwrap();
        assert_eq!(
            (
                u.input,
                u.output,
                u.cache_read,
                u.cache_write,
                u.total_tokens
            ),
            (18, 7, 2, 0, 27)
        );
    }

    #[tokio::test]
    async fn finalizes_incomplete_max_output_terminal_events_as_length_stops() {
        let body = concat!(
            "data: {\"type\":\"response.incomplete\",\"response\":{\"id\":\"resp_incomplete\",\"status\":\"incomplete\",\"incomplete_details\":{\"reason\":\"max_output_tokens\"},\"usage\":{\"input_tokens\":30,\"output_tokens\":12,\"total_tokens\":42,\"input_tokens_details\":{\"cached_tokens\":5}}}}\n\n",
            "data: [DONE]\n\n",
        ).to_string();
        let (reason, _err, m) = run(body).await;
        assert!(matches!(reason, StopReason::Length));
        assert_eq!(
            m.raw_stop_reason.as_deref(),
            Some("incomplete.max_output_tokens")
        );
        assert_eq!(m.response_id.as_deref(), Some("resp_incomplete"));
        let u = m.usage.unwrap();
        assert_eq!(
            (
                u.input,
                u.output,
                u.cache_read,
                u.cache_write,
                u.total_tokens
            ),
            (25, 12, 5, 0, 42)
        );
    }

    #[tokio::test]
    async fn incomplete_non_max_output_reason_is_error_with_raw_reason() {
        let body = concat!(
            "data: {\"type\":\"response.incomplete\",\"response\":{\"id\":\"resp_filter\",\"status\":\"incomplete\",\"incomplete_details\":{\"reason\":\"content_filter\"}}}\n\n",
            "data: [DONE]\n\n",
        ).to_string();
        let (reason, err, m) = run(body).await;
        assert!(matches!(reason, StopReason::Error));
        assert_eq!(
            m.raw_stop_reason.as_deref(),
            Some("incomplete.content_filter")
        );
        assert_eq!(err.as_deref(), Some("Response incomplete: content_filter"));
    }

    #[tokio::test]
    async fn rejects_failed_terminal_events_with_the_provider_error() {
        let body = "data: {\"type\":\"response.failed\",\"response\":{\"id\":\"resp_failed\",\"status\":\"failed\",\"error\":{\"code\":\"server_error\",\"message\":\"boom\"}}}\n\n".to_string();
        let (reason, err, _m) = run(body).await;
        assert!(matches!(reason, StopReason::Error));
        assert!(
            err.as_deref()
                .is_some_and(|e| e.contains("server_error") && e.contains("boom")),
            "got: {err:?}"
        );
    }
}
