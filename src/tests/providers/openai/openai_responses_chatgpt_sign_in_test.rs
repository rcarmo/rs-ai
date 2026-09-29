//! v0.99.1 OpenAI Responses direct ChatGPT-token behavior.

#[cfg(test)]
mod tests {
    use crate::events::Event;
    use crate::provider::responses::stream_responses;
    use crate::types::{CacheRetention, Context, StreamOptions, user_message};
    use std::sync::{Arc, Mutex};
    use tokio_stream::StreamExt;
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn context() -> Context {
        Context {
            system_prompt: None,
            messages: vec![user_message("hi")],
            tools: vec![],
        }
    }

    async fn payload(api_key: &str, official: bool) -> serde_json::Value {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;
        let mut model = crate::registry::get_model("openai", "gpt-5-mini").unwrap();
        model.base_url = if official {
            // Preserve the exact official URL for sign-in detection while redirecting
            // transport through on_payload-only testing is not possible. Test the
            // predicate separately and use the internal payload hook for request shape.
            "https://api.openai.com/v1".into()
        } else {
            server.uri()
        };
        model.api_key = Some(api_key.into());
        let captured = Arc::new(Mutex::new(None));
        let slot = captured.clone();
        let options = StreamOptions {
            max_tokens: Some(1000),
            temperature: Some(0.5),
            cache_retention: Some(CacheRetention::Long),
            on_payload: Some(Arc::new(move |value, _| {
                *slot.lock().unwrap() = Some(value.clone());
                Err(Box::<dyn std::error::Error + Send + Sync>::from("stop"))
            })),
            ..Default::default()
        };
        let context = context();
        let mut stream = stream_responses(&model, &context, &options);
        while stream.next().await.is_some() {}
        captured.lock().unwrap().clone().unwrap()
    }

    #[tokio::test]
    async fn direct_chatgpt_tokens_omit_rejected_request_fields() {
        let mut model = crate::registry::get_model("openai", "gpt-5-mini").unwrap();
        model.base_url = "https://api.openai.com/v1".into();
        assert!(crate::provider::responses::is_chatgpt_sign_in(
            &model,
            "chatgpt-access-token"
        ));
        assert!(!crate::provider::responses::is_chatgpt_sign_in(
            &model,
            "sk-proj-test"
        ));
        let direct = payload("chatgpt-access-token", true).await;
        assert!(direct.get("max_output_tokens").is_none());
        assert!(direct.get("temperature").is_none());
        assert!(direct.get("prompt_cache_retention").is_none());
        assert!(direct.get("prompt_cache_options").is_none());
    }

    #[tokio::test]
    async fn api_keys_and_other_endpoints_keep_request_fields() {
        let api_key = payload("sk-proj-test", true).await;
        assert_eq!(api_key["max_output_tokens"], 1000);
        assert_eq!(api_key["temperature"], 0.5);
        assert_eq!(api_key["prompt_cache_retention"], "24h");

        let gateway = payload("gateway-key", false).await;
        assert_eq!(gateway["max_output_tokens"], 1000);
        assert_eq!(gateway["temperature"], 0.5);
        assert_eq!(gateway["prompt_cache_retention"], "24h");
    }

    async fn error_message(template: ResponseTemplate) -> String {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(template)
            .mount(&server)
            .await;
        let mut model = crate::registry::get_model("openai", "gpt-5-mini").unwrap();
        model.base_url = server.uri();
        model.api_key = Some("test".into());
        let context = context();
        let options = StreamOptions::default();
        let mut stream = stream_responses(&model, &context, &options);
        while let Some(event) = stream.next().await {
            if let Event::Error { error, .. } = event {
                return error.to_string();
            }
        }
        panic!("expected error")
    }

    #[tokio::test]
    async fn usage_limit_errors_link_to_chatgpt_usage_for_http_and_stream() {
        let body = serde_json::json!({"error":{
            "code":"subscription_sharing_usage_limit_exceeded",
            "message":"Usage limit reached.",
            "type":"rate_limit_error"
        }});
        let http = error_message(
            ResponseTemplate::new(429)
                .insert_header("content-type", "application/json")
                .set_body_json(body),
        )
        .await;
        assert!(http.contains("subscription_sharing_usage_limit_exceeded"));
        assert!(http.contains("https://chatgpt.com/settings/usage"));

        let event = serde_json::json!({
            "type":"response.failed",
            "response":{"id":"failed","status":"failed","error":{
                "code":"subscription_sharing_usage_limit_exceeded",
                "message":"Usage limit reached."
            }}
        });
        let sse = error_message(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_string(format!("event: response.failed\ndata: {event}\n\n")),
        )
        .await;
        assert!(sse.contains("subscription_sharing_usage_limit_exceeded: Usage limit reached."));
        assert!(sse.contains("https://chatgpt.com/settings/usage"));
    }
}
