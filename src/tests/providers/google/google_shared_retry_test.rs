//! v0.84.0 Google shared request retry regression.
//!
//! Ports upstream `test/google-shared-retry.test.ts` to rs-ai's real Google REST
//! stream call site. Upstream wraps `@google/genai` request calls with
//! `retryGoogleRequest`; rs-ai wraps the `stream_google` HTTP request directly.

#[cfg(test)]
mod tests {
    use crate::events::Event;
    use crate::provider::google::{google_retry_config_from_options, stream_google};
    use crate::types::{ContentBlock, Context, Message, Model, ModelCost, Role, StreamOptions};
    use futures::StreamExt;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::sync::watch;
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

    fn model(base_url: &str) -> Model {
        Model {
            id: "gemini-2.5-flash".into(),
            name: "Gemini".into(),
            api: "google-generative-ai".into(),
            provider: "google".into(),
            base_url: base_url.into(),
            reasoning: false,
            thinking_level_map: None,
            input: vec!["text".into()],
            cost: ModelCost::default(),
            context_window: 128000,
            max_tokens: 4096,
            sampling_params: None,
            headers: None,
            api_key: Some("test-key".into()),
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

    fn ok_sse() -> ResponseTemplate {
        ResponseTemplate::new(200)
            .insert_header("content-type", "text/event-stream")
            .set_body_string(
                "data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"ok\"}]},\"finishReason\":\"STOP\"}],\"usageMetadata\":{\"promptTokenCount\":1,\"candidatesTokenCount\":1,\"totalTokenCount\":2}}\n\n",
            )
    }

    async fn run_text_base(base_url: &str, opts: StreamOptions) -> (String, Option<String>) {
        let model = model(base_url);
        let context = ctx();
        let mut stream = stream_google(&model, &context, &opts);
        let mut text = String::new();
        let mut error = None;
        while let Some(evt) = stream.next().await {
            match evt {
                Event::TextDelta { delta } => text.push_str(&delta),
                Event::Error { error: err, .. } => error = Some(err.to_string()),
                _ => {}
            }
        }
        (text, error)
    }

    async fn run_text(server: &MockServer, opts: StreamOptions) -> (String, Option<String>) {
        run_text_base(&server.uri(), opts).await
    }

    struct Sequence {
        calls: Arc<AtomicUsize>,
        first_status: u16,
        first_headers: Vec<(&'static str, &'static str)>,
    }

    impl Respond for Sequence {
        fn respond(&self, _request: &Request) -> ResponseTemplate {
            let n = self.calls.fetch_add(1, Ordering::SeqCst);
            if n == 0 {
                let mut r =
                    ResponseTemplate::new(self.first_status).set_body_string("google error");
                for (k, v) in &self.first_headers {
                    r = r.insert_header(*k, *v);
                }
                r
            } else {
                ok_sse()
            }
        }
    }

    async fn mount_sequence(
        server: &MockServer,
        first_status: u16,
        first_headers: Vec<(&'static str, &'static str)>,
    ) -> Arc<AtomicUsize> {
        let calls = Arc::new(AtomicUsize::new(0));
        Mock::given(method("POST"))
            .respond_with(Sequence {
                calls: calls.clone(),
                first_status,
                first_headers,
            })
            .mount(server)
            .await;
        calls
    }

    #[tokio::test]
    async fn retries_headers_less_google_status_429_once_when_max_retries_is_one() {
        let server = MockServer::start().await;
        let calls = mount_sequence(&server, 429, Vec::new()).await;
        let (text, err) = run_text(
            &server,
            StreamOptions {
                max_retries: Some(1),
                ..Default::default()
            },
        )
        .await;
        assert_eq!(text, "ok");
        assert!(err.is_none(), "unexpected error: {err:?}");
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn does_not_retry_google_429_when_max_retries_is_unset() {
        let server = MockServer::start().await;
        let calls = mount_sequence(&server, 429, Vec::new()).await;
        let (text, err) = run_text(&server, StreamOptions::default()).await;
        assert!(text.is_empty());
        assert!(err.unwrap().contains("429"));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn does_not_retry_google_non_retryable_status_400_even_with_budget() {
        let server = MockServer::start().await;
        let calls = mount_sequence(&server, 400, Vec::new()).await;
        let (text, err) = run_text(
            &server,
            StreamOptions {
                max_retries: Some(2),
                ..Default::default()
            },
        )
        .await;
        assert!(text.is_empty());
        assert!(err.unwrap().contains("400"));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn google_retry_config_defaults_to_500ms_backoff_and_honors_delay_cap() {
        let opts = StreamOptions {
            max_retries: Some(1),
            max_retry_delay_ms: Some(1500),
            ..Default::default()
        };
        let cfg = google_retry_config_from_options(&opts);
        assert_eq!(cfg.max_retries, 1);
        assert_eq!(cfg.initial_delay, std::time::Duration::from_millis(500));
        assert_eq!(cfg.max_retry_delay_ms, 1500);
    }

    #[tokio::test]
    async fn google_retry_after_delay_cap_fails_without_second_attempt() {
        let server = MockServer::start().await;
        let calls = mount_sequence(&server, 429, vec![("retry-after-ms", "250")]).await;
        let (text, err) = run_text(
            &server,
            StreamOptions {
                max_retries: Some(1),
                max_retry_delay_ms: Some(100),
                ..Default::default()
            },
        )
        .await;
        assert!(text.is_empty());
        assert!(
            err.unwrap()
                .contains("Server requested 1s retry delay (max: 1s)"),
            "delay-cap error must surface"
        );
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn google_retry_backoff_honors_caller_cancellation() {
        let server = MockServer::start().await;
        let calls = mount_sequence(&server, 429, Vec::new()).await;
        let (tx, rx) = watch::channel(false);
        let server_uri = server.uri();
        let task = tokio::spawn(async move {
            run_text_base(
                &server_uri,
                StreamOptions {
                    max_retries: Some(1),
                    cancel: Some(rx),
                    ..Default::default()
                },
            )
            .await
        });
        tokio::time::timeout(std::time::Duration::from_millis(250), async {
            while calls.load(Ordering::SeqCst) == 0 {
                tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            }
        })
        .await
        .expect("first Google request should start");
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        tx.send(true).unwrap();
        let (_text, err) = task.await.unwrap();
        assert_eq!(err.as_deref(), Some("Request aborted"));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }
}
