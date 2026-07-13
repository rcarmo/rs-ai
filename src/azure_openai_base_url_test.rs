//! Test-for-test port of upstream `test/azure-openai-base-url.test.ts`
//! (`@earendil-works/pi-ai` v0.80.2).
//!
//! Upstream mocks the `AzureOpenAI` SDK client and inspects the constructed
//! `baseURL` plus the request params. rs-ai builds raw HTTP and resolves the
//! Azure base URL from env/model.baseUrl, so the base-URL normalization cases
//! are asserted directly against `normalize_azure_base_url` /
//! `resolve_azure_base_url_from`, and the request-shape cases
//! (prompt_cache_key clamp, store:false, invalid-URL error) are driven through
//! `stream_azure_responses` with the payload captured via `on_payload`.

#[cfg(test)]
mod tests {
    use crate::events::Event;
    use crate::provider::responses::{
        normalize_azure_base_url, resolve_azure_base_url_from, stream_azure_responses,
    };
    use crate::types::{ContentBlock, Context, Message, Model, ModelCost, Role, StreamOptions};
    use serde_json::Value;
    use std::sync::{Arc, Mutex};
    use tokio_stream::StreamExt;

    fn context() -> Context {
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
                error_message: None,
                tool_call_id: None,
                tool_name: None,
                is_error: false,
                details: None,
            }],
        }
    }

    fn azure_model(base_url: &str) -> Model {
        Model {
            id: "gpt-4o-mini".into(),
            name: "GPT-4o mini".into(),
            api: "azure-openai-responses".into(),
            provider: "azure-openai-responses".into(),
            base_url: base_url.into(),
            reasoning: false,
            thinking_level_map: None,
            input: vec!["text".into()],
            cost: ModelCost::default(),
            context_window: 128000,
            max_tokens: 8192,
            headers: None,
            api_key: Some("test-api-key".into()),
            compat: Default::default(),
        }
    }

    /// Capture the wire payload via `on_payload`; the request then fails against
    /// the non-routable host, but the hook has already run.
    async fn capture_payload(model: Model, mut opts: StreamOptions) -> Value {
        let captured: Arc<Mutex<Option<Value>>> = Arc::new(Mutex::new(None));
        let sink = captured.clone();
        opts.on_payload = Some(Arc::new(move |p: Value, _m: &Model| {
            *sink.lock().unwrap() = Some(p.clone());
            Ok(p)
        }));
        let ctx = context();
        let mut stream = stream_azure_responses(&model, &ctx, &opts);
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

    // --- base URL normalization (asserted directly) ---

    #[test]
    fn normalizes_cognitive_services_root_endpoints_to_openai_v1() {
        assert_eq!(
            normalize_azure_base_url(
                "https://marc-quicktests-resource.cognitiveservices.azure.com"
            ),
            Ok(
                "https://marc-quicktests-resource.cognitiveservices.azure.com/openai/v1"
                    .to_string()
            )
        );
    }

    #[test]
    fn normalizes_azure_openai_root_endpoints_to_openai_v1() {
        assert_eq!(
            normalize_azure_base_url("https://my-resource.openai.azure.com"),
            Ok("https://my-resource.openai.azure.com/openai/v1".to_string())
        );
    }

    #[test]
    fn normalizes_openai_to_openai_v1() {
        assert_eq!(
            normalize_azure_base_url("https://my-resource.cognitiveservices.azure.com/openai"),
            Ok("https://my-resource.cognitiveservices.azure.com/openai/v1".to_string())
        );
    }

    // --- v0.80.3: Microsoft Foundry (.ai.azure.com) normalization ---

    #[test]
    fn normalizes_microsoft_foundry_root_endpoints_to_openai_v1() {
        assert_eq!(
            normalize_azure_base_url("https://marc-quicktests-resource.ai.azure.com"),
            Ok("https://marc-quicktests-resource.ai.azure.com/openai/v1".to_string())
        );
    }

    #[test]
    fn normalizes_foundry_openai_v1_responses_to_openai_v1() {
        assert_eq!(
            normalize_azure_base_url(
                "https://my-resource.services.ai.azure.com/openai/v1/responses"
            ),
            Ok("https://my-resource.services.ai.azure.com/openai/v1".to_string())
        );
    }

    #[test]
    fn preserves_openai_v1_endpoints() {
        assert_eq!(
            normalize_azure_base_url("https://my-resource.cognitiveservices.azure.com/openai/v1"),
            Ok("https://my-resource.cognitiveservices.azure.com/openai/v1".to_string())
        );
    }

    #[test]
    fn preserves_explicit_non_azure_proxy_paths() {
        assert_eq!(
            normalize_azure_base_url("https://my-proxy.example.com/v1"),
            Ok("https://my-proxy.example.com/v1".to_string())
        );
    }

    #[test]
    fn strips_query_params_when_normalizing_azure_host_urls() {
        assert_eq!(
            normalize_azure_base_url(
                "https://my-resource.openai.azure.com/openai?api-version=2024-12-01"
            ),
            Ok("https://my-resource.openai.azure.com/openai/v1".to_string())
        );
    }

    #[test]
    fn preserves_query_params_on_non_azure_proxy_urls() {
        assert_eq!(
            normalize_azure_base_url("https://my-proxy.example.com/v1?custom=true"),
            Ok("https://my-proxy.example.com/v1?custom=true".to_string())
        );
    }

    #[test]
    fn builds_correct_default_url_from_resource_name() {
        // Mirrors AZURE_OPENAI_RESOURCE_NAME=my-resource.
        assert_eq!(
            resolve_azure_base_url_from(None, Some("my-resource"), ""),
            Ok(Some(
                "https://my-resource.openai.azure.com/openai/v1".to_string()
            ))
        );
    }

    // --- request shape (driven through the stream) ---

    #[tokio::test]
    async fn throws_on_invalid_urls() {
        let model = azure_model("not-a-url");
        let ctx = context();
        let opts = StreamOptions::default();
        let mut stream = stream_azure_responses(&model, &ctx, &opts);
        let mut err = None;
        while let Some(evt) = stream.next().await {
            if let Event::Error { error, .. } = evt {
                err = Some(error.to_string());
                break;
            }
        }
        let e = err.expect("expected an error for an invalid base URL");
        assert!(e.contains("Invalid Azure OpenAI base URL"), "got: {e}");
    }

    #[tokio::test]
    async fn clamps_prompt_cache_key_to_64_chars() {
        let opts = StreamOptions {
            session_id: Some("x".repeat(67)),
            ..Default::default()
        };
        let p = capture_payload(azure_model("http://127.0.0.1:9"), opts).await;
        assert_eq!(
            p.get("prompt_cache_key").and_then(|v| v.as_str()),
            Some("x".repeat(64).as_str())
        );
    }

    #[tokio::test]
    async fn disables_server_side_response_storage() {
        let p = capture_payload(azure_model("http://127.0.0.1:9"), StreamOptions::default()).await;
        assert_eq!(p.get("store").and_then(|v| v.as_bool()), Some(false));
    }
}
