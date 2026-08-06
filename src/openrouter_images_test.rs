//! Test-for-test port of upstream `test/openrouter-images.test.ts`
//! (`@earendil-works/pi-ai` v0.80.2). The abort-signal case is N/A (rs-ai has no
//! AbortSignal; it uses native future-drop), the rest are ported via wiremock.

#[cfg(test)]
mod tests {
    use crate::images::openrouter::{ImagesOptions, generate_openrouter};
    use crate::images::types::{ImageInput, ImageOutput, ImagesContext, ImagesModel};
    use crate::types::{ModelCost, StopReason};
    use serde_json::Value;
    use std::sync::{Arc, Mutex};
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn model(base_url: &str, output: Vec<&str>) -> ImagesModel {
        ImagesModel {
            id: "google/gemini-3.1-flash-image-preview".into(),
            name: "Gemini".into(),
            api: "openrouter-images".into(),
            provider: "openrouter".into(),
            base_url: base_url.into(),
            input: vec!["text".into(), "image".into()],
            output: output.into_iter().map(String::from).collect(),
            cost: ModelCost::default(),
        }
    }

    fn ctx() -> ImagesContext {
        ImagesContext {
            input: vec![ImageInput::Text {
                text: "Generate a dog".into(),
            }],
        }
    }

    fn opts(captured: Arc<Mutex<Option<Value>>>) -> ImagesOptions {
        ImagesOptions {
            api_key: Some("test".into()),
            headers: None,
            timeout: None,
            max_retries: 0,
            max_retry_delay_ms: 0,
            telemetry_context: None,
            on_payload: Some(Arc::new(move |p: Value, _m: &ImagesModel| {
                *captured.lock().unwrap() = Some(p.clone());
                Ok(p)
            })),
            on_response: None,
        }
    }

    const RESPONSE: &str = r#"{"id":"img-1","usage":{"prompt_tokens":12,"completion_tokens":34,"prompt_tokens_details":{"cached_tokens":0}},"choices":[{"message":{"content":"Here is your image.","images":[{"image_url":"data:image/png;base64,ZmFrZS1wbmc="}]}}]}"#;

    async fn server() -> MockServer {
        let s = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "application/json")
                    .set_body_string(RESPONSE),
            )
            .mount(&s)
            .await;
        s
    }

    #[tokio::test]
    async fn returns_text_plus_images_in_final_output() {
        let s = server().await;
        let captured: Arc<Mutex<Option<Value>>> = Arc::new(Mutex::new(None));
        let out = generate_openrouter(
            &model(&s.uri(), vec!["text", "image"]),
            &ctx(),
            &opts(captured.clone()),
        )
        .await;

        assert!(matches!(out.stop_reason, StopReason::Stop));
        assert_eq!(out.response_id.as_deref(), Some("img-1"));
        match &out.output[0] {
            ImageOutput::Text { text } => assert_eq!(text, "Here is your image."),
            other => panic!("expected text, got {other:?}"),
        }
        match &out.output[1] {
            ImageOutput::Image { data, mime_type } => {
                assert_eq!(mime_type, "image/png");
                assert_eq!(data, "ZmFrZS1wbmc=");
            }
            other => panic!("expected image, got {other:?}"),
        }

        let p = captured.lock().unwrap().clone().expect("payload");
        assert_eq!(p["stream"], serde_json::json!(false));
        assert_eq!(p["modalities"], serde_json::json!(["image", "text"]));
        assert_eq!(
            p["messages"][0]["content"][0]["type"],
            serde_json::json!("text")
        );
        assert_eq!(
            p["messages"][0]["content"][0]["text"],
            serde_json::json!("Generate a dog")
        );
    }

    #[tokio::test]
    async fn generate_images_resolves_the_final_assistant_images_result() {
        let s = server().await;
        let captured: Arc<Mutex<Option<Value>>> = Arc::new(Mutex::new(None));
        let out =
            generate_openrouter(&model(&s.uri(), vec!["image"]), &ctx(), &opts(captured)).await;
        assert!(
            out.output
                .iter()
                .any(|o| matches!(o, ImageOutput::Image { .. }))
        );
    }

    /// Port of `provider-error-body-passthrough.test.ts`: a 403 from a gateway
    /// carrying the real reason in the body must surface status + body, not an
    /// opaque "403 status code (no body)" message. rs-ai reads `resp.text()`
    /// directly, so we assert the real-transport contract via wiremock.
    #[tokio::test]
    async fn surfaces_http_body_reason_instead_of_opaque_sdk_message() {
        let s = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(403)
                    .insert_header("content-type", "application/json")
                    .set_body_string(r#"{"error":"blocked by gateway WAF"}"#),
            )
            .mount(&s)
            .await;
        let captured: Arc<Mutex<Option<Value>>> = Arc::new(Mutex::new(None));
        let out =
            generate_openrouter(&model(&s.uri(), vec!["image"]), &ctx(), &opts(captured)).await;
        assert!(matches!(out.stop_reason, StopReason::Error));
        let msg = out.error_message.expect("error message");
        assert!(msg.contains("403"), "status surfaced: {msg}");
        assert!(
            msg.contains("blocked by gateway WAF"),
            "body reason surfaced: {msg}"
        );
        assert_ne!(msg, "403 status code (no body)");
    }
}
