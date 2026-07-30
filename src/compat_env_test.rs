//! Test-for-test port of upstream `test/compat-env.test.ts`
//! (`@earendil-works/pi-ai` v0.80.2): an unknown api dispatches through the
//! registered ApiProvider, and the request apiKey is passed through.

#[cfg(test)]
mod tests {
    use crate::events::Event;
    use crate::registry::{ApiProvider, register_api, stream};
    use crate::types::{
        ContentBlock, Context, Message, Model, ModelCost, Role, StopReason, StreamOptions,
    };
    use std::sync::{Arc, Mutex};
    use tokio_stream::StreamExt;

    const CAPTURE_API: &str = "compat-env-capture-api";

    struct CaptureProvider {
        captured: Arc<Mutex<Option<String>>>,
    }

    impl ApiProvider for CaptureProvider {
        fn api(&self) -> &str {
            CAPTURE_API
        }
        fn stream<'a>(
            &self,
            model: &'a Model,
            _context: &'a Context,
            opts: &'a StreamOptions,
        ) -> std::pin::Pin<Box<dyn futures::Stream<Item = Event> + Send + 'a>> {
            *self.captured.lock().unwrap() = opts.api_key.clone();
            let msg = Message {
                role: Role::Assistant,
                content: vec![ContentBlock::Text {
                    text: "ok".into(),
                    text_signature: None,
                }],
                timestamp: 0,
                api: Some(model.api.clone()),
                provider: Some(model.provider.clone()),
                model: Some(model.id.clone()),
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
            Box::pin(futures::stream::iter(vec![
                Event::Start {
                    partial: msg.clone(),
                },
                Event::Done {
                    reason: StopReason::Stop,
                    message: msg,
                },
            ]))
        }
    }

    #[tokio::test]
    async fn dispatches_unknown_providers_through_the_api_registry_with_request_key() {
        let captured: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
        register_api(Arc::new(CaptureProvider {
            captured: captured.clone(),
        }));

        let model = Model {
            id: "test-model".into(),
            name: "Test Model".into(),
            api: CAPTURE_API.into(),
            provider: "custom-openai".into(),
            base_url: "https://example.test/v1".into(),
            reasoning: false,
            thinking_level_map: None,
            input: vec!["text".into()],
            cost: ModelCost::default(),
            context_window: 128000,
            max_tokens: 4096,
            headers: None,
            api_key: None,
            compat: Default::default(),
        };
        let ctx = Context {
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
                raw_stop_reason: None,
                tool_call_id: None,
                tool_name: None,
                is_error: false,
                details: None,
                added_tool_names: Vec::new(),
            }],
        };
        let opts = StreamOptions {
            api_key: Some("request-key".into()),
            ..Default::default()
        };
        let mut s = stream(&model, &ctx, &opts);
        while let Some(evt) = s.next().await {
            if matches!(evt, Event::Done { .. } | Event::Error { .. }) {
                break;
            }
        }
        assert_eq!(captured.lock().unwrap().as_deref(), Some("request-key"));
    }
}
