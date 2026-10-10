//! Test-for-test port of upstream `test/mistral-reasoning-mode.test.ts`
//! (`@earendil-works/pi-ai` v0.80.2).
//!
//! Verifies Mistral reasoning-mode selection: `reasoning_effort` for the
//! effort-based models, `prompt_mode: "reasoning"` for Magistral models, and
//! the prompt-cache-key gating. Payloads are captured via the `on_payload`
//! hook against a non-routable base URL (the request fails after capture,
//! mirroring the upstream `baseUrl: "http://127.0.0.1:9"` pattern).

#[cfg(test)]
mod tests {
    use crate::events::Event;
    use crate::provider::mistral::stream_mistral;
    use crate::registry::get_model;
    use crate::types::{
        CacheRetention, Context, Message, Model, Role, StreamOptions, ThinkingLevel,
    };
    use serde_json::Value;
    use std::sync::{Arc, Mutex};
    use tokio_stream::StreamExt;

    fn make_context() -> Context {
        Context {
            system_prompt: None,
            tools: Vec::new(),
            messages: vec![Message {
                role: Role::User,
                content: vec![crate::types::ContentBlock::Text {
                    text: "Hello".into(),
                    text_signature: None,
                }],
                timestamp: 0,
                duration_ms: None,
                api: None,
                provider: None,
                model: None,
                response_id: None,
                response_model: None,
                provider_thinking_level: None,
                thinking_level: None,
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
                nested_calls: None,
                added_tool_names: Vec::new(),
                sections: None,
                tools_added: Vec::new(),
                tools_removed: Vec::new(),
            }],
        }
    }

    /// Drive the Mistral stream against a non-routable URL, capturing the wire
    /// payload via `on_payload` before the request fails.
    async fn capture_payload(mut model: Model, mut opts: StreamOptions) -> Value {
        model.base_url = "http://127.0.0.1:9".into();
        model.api_key = Some("fake-key".into());
        let captured: Arc<Mutex<Option<Value>>> = Arc::new(Mutex::new(None));
        let sink = captured.clone();
        opts.on_payload = Some(Arc::new(move |p: Value, _m: &Model| {
            *sink.lock().unwrap() = Some(p.clone());
            Ok(p)
        }));
        let ctx = make_context();
        let mut stream = stream_mistral(&model, &ctx, &opts);
        while let Some(evt) = stream.next().await {
            if matches!(evt, Event::Done { .. } | Event::Error { .. }) {
                break;
            }
        }
        captured
            .lock()
            .unwrap()
            .clone()
            .expect("payload captured before request failure")
    }

    #[tokio::test]
    async fn v110_server_error_finish_reason_preserves_raw_reason_and_content() {
        use wiremock::matchers::method;
        use wiremock::{Mock, MockServer, ResponseTemplate};
        let server = MockServer::start().await;
        Mock::given(method("POST")).respond_with(ResponseTemplate::new(200)
            .insert_header("content-type", "text/event-stream")
            .set_body_string(concat!(
                "data: {\"choices\":[{\"delta\":{\"content\":\"partial\"},\"finish_reason\":null}]}\n\n",
                "data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"error\"}],\"usage\":{\"prompt_tokens\":2,\"completion_tokens\":1,\"total_tokens\":3}}\n\n",
                "data: [DONE]\n\n",
            ))).mount(&server).await;
        let mut model = mistral("mistral-small-2603");
        model.base_url = server.uri();
        model.api_key = Some("test-key".into());
        let context = make_context();
        let options = StreamOptions::default();
        let events = stream_mistral(&model, &context, &options)
            .collect::<Vec<_>>()
            .await;
        let Some(Event::Error {
            message: Some(message),
            ..
        }) = events.last()
        else {
            panic!("expected terminal assistant error: {events:?}")
        };
        assert_eq!(message.raw_stop_reason.as_deref(), Some("error"));
        assert_eq!(message.stop_reason, Some(crate::types::StopReason::Error));
        assert_eq!(
            message.error_message.as_deref(),
            Some("Provider stopped with: error (server error)")
        );
        assert!(
            matches!(message.content.first(), Some(crate::types::ContentBlock::Text { text, .. }) if text == "partial")
        );
        assert_eq!(message.usage.as_ref().unwrap().total_tokens, 3);
    }

    fn mistral(id: &str) -> Model {
        get_model("mistral", id).unwrap_or_else(|| panic!("missing catalog model mistral/{id}"))
    }

    #[tokio::test]
    async fn uses_reasoning_effort_for_mistral_small_4() {
        let p = capture_payload(
            mistral("mistral-small-2603"),
            StreamOptions {
                reasoning: Some(ThinkingLevel::Medium),
                ..Default::default()
            },
        )
        .await;
        assert_eq!(
            p.get("reasoning_effort").and_then(|v| v.as_str()),
            Some("high")
        );
        assert!(p.get("prompt_mode").is_none());
    }

    #[tokio::test]
    async fn omits_reasoning_controls_for_mistral_small_4_when_thinking_off() {
        let p = capture_payload(mistral("mistral-small-2603"), StreamOptions::default()).await;
        assert!(p.get("reasoning_effort").is_none());
        assert!(p.get("prompt_mode").is_none());
    }

    #[tokio::test]
    async fn uses_prompt_mode_for_magistral_reasoning_models() {
        let p = capture_payload(
            mistral("magistral-medium-latest"),
            StreamOptions {
                reasoning: Some(ThinkingLevel::Medium),
                ..Default::default()
            },
        )
        .await;
        assert_eq!(
            p.get("prompt_mode").and_then(|v| v.as_str()),
            Some("reasoning")
        );
        assert!(p.get("reasoning_effort").is_none());
    }

    #[tokio::test]
    async fn uses_reasoning_effort_for_mistral_medium_3_5() {
        let p = capture_payload(
            mistral("mistral-medium-3.5"),
            StreamOptions {
                reasoning: Some(ThinkingLevel::Medium),
                ..Default::default()
            },
        )
        .await;
        assert_eq!(
            p.get("reasoning_effort").and_then(|v| v.as_str()),
            Some("high")
        );
        assert!(p.get("prompt_mode").is_none());
    }

    #[tokio::test]
    async fn omits_reasoning_controls_for_mistral_medium_3_5_when_thinking_off() {
        let p = capture_payload(mistral("mistral-medium-3.5"), StreamOptions::default()).await;
        assert!(p.get("reasoning_effort").is_none());
        assert!(p.get("prompt_mode").is_none());
    }

    #[tokio::test]
    async fn uses_the_session_id_as_prompt_cache_key() {
        let p = capture_payload(
            mistral("mistral-large-latest"),
            StreamOptions {
                session_id: Some("session-123".into()),
                ..Default::default()
            },
        )
        .await;
        assert_eq!(
            p.get("prompt_cache_key").and_then(|v| v.as_str()),
            Some("session-123")
        );
    }

    #[tokio::test]
    async fn omits_prompt_cache_key_when_cache_retention_disabled() {
        let p = capture_payload(
            mistral("mistral-large-latest"),
            StreamOptions {
                session_id: Some("session-123".into()),
                cache_retention: Some(CacheRetention::None),
                ..Default::default()
            },
        )
        .await;
        assert!(p.get("prompt_cache_key").is_none());
    }
}
