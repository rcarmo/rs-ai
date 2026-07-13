//! Test-for-test port of upstream `test/openai-completions-prompt-cache.test.ts`
//! (`@earendil-works/pi-ai` v0.80.2): prompt_cache_key / prompt_cache_retention
//! payload fields and session-affinity headers.

#[cfg(test)]
mod tests {
    use crate::compat::detect_compat;
    use crate::events::Event;
    use crate::provider::openai::{build_payload, stream_openai};
    use crate::registry::get_model;
    use crate::types::{
        CacheRetention, ContentBlock, Context, Message, Model, ModelCompat, Role, StreamOptions,
    };
    use serde_json::Value;
    use std::collections::HashMap;
    use std::sync::Mutex;
    use tokio_stream::StreamExt;
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn openai_model() -> Model {
        get_model("openai", "gpt-4o-mini").unwrap()
    }

    fn proxy_model(compat: ModelCompat) -> Model {
        let mut m = openai_model();
        m.provider = "custom".into();
        m.base_url = "https://proxy.example.com/v1".into();
        m.compat = compat;
        m
    }

    fn ctx() -> Context {
        Context {
            system_prompt: Some("sys".into()),
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

    fn payload(model: &Model, opts: &StreamOptions) -> Value {
        build_payload(model, &ctx(), opts, &detect_compat(model))
    }

    fn session(sid: &str, retention: Option<CacheRetention>) -> StreamOptions {
        StreamOptions {
            session_id: Some(sid.into()),
            cache_retention: retention,
            ..Default::default()
        }
    }

    #[test]
    fn sets_prompt_cache_key_for_direct_openai_when_caching_enabled() {
        let p = payload(&openai_model(), &session("session-123", None));
        assert_eq!(p["prompt_cache_key"], serde_json::json!("session-123"));
        assert!(p.get("prompt_cache_retention").is_none());
    }

    #[test]
    fn sets_prompt_cache_retention_24h_when_long() {
        let p = payload(
            &openai_model(),
            &session("session-456", Some(CacheRetention::Long)),
        );
        assert_eq!(p["prompt_cache_key"], serde_json::json!("session-456"));
        assert_eq!(p["prompt_cache_retention"], serde_json::json!("24h"));
    }

    #[test]
    fn clamps_prompt_cache_key_to_64_chars() {
        let p = payload(&openai_model(), &session(&"x".repeat(67), None));
        assert_eq!(p["prompt_cache_key"], serde_json::json!("x".repeat(64)));
    }

    #[test]
    fn omits_prompt_cache_fields_when_none() {
        let p = payload(
            &openai_model(),
            &session("session-789", Some(CacheRetention::None)),
        );
        assert!(p.get("prompt_cache_key").is_none());
        assert!(p.get("prompt_cache_retention").is_none());
    }

    #[test]
    fn omits_prompt_cache_fields_for_non_openai_without_long_retention() {
        let model = proxy_model(ModelCompat {
            supports_long_cache_retention: Some(false),
            ..Default::default()
        });
        let p = payload(
            &model,
            &session("session-proxy", Some(CacheRetention::Long)),
        );
        assert!(p.get("prompt_cache_key").is_none());
        assert!(p.get("prompt_cache_retention").is_none());
    }

    #[test]
    fn uses_pi_cache_retention_env_for_direct_openai() {
        let _g = ENV_LOCK.lock().unwrap();
        unsafe {
            std::env::set_var("PI_CACHE_RETENTION", "long");
        }
        let p = payload(&openai_model(), &session("session-env", None));
        unsafe {
            std::env::remove_var("PI_CACHE_RETENTION");
        }
        assert_eq!(p["prompt_cache_key"], serde_json::json!("session-env"));
        assert_eq!(p["prompt_cache_retention"], serde_json::json!("24h"));
    }

    // --- session-affinity headers ---

    async fn capture_headers(model: Model, opts: StreamOptions) -> HashMap<String, String> {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_string("data: [DONE]\n\n"),
            )
            .mount(&server)
            .await;
        let mut model = model;
        model.base_url = server.uri();
        model.api_key = Some("test-key".into());
        let c = ctx();
        let mut stream = stream_openai(&model, &c, &opts);
        while let Some(evt) = stream.next().await {
            if matches!(evt, Event::Done { .. } | Event::Error { .. }) {
                break;
            }
        }
        let reqs = server.received_requests().await.unwrap();
        reqs.last()
            .unwrap()
            .headers
            .iter()
            .map(|(k, v)| (k.as_str().to_string(), v.to_str().unwrap_or("").to_string()))
            .collect()
    }

    fn affinity_model() -> Model {
        proxy_model(ModelCompat {
            send_session_affinity_headers: Some(true),
            ..Default::default()
        })
    }

    #[tokio::test]
    async fn sends_session_affinity_headers_when_enabled() {
        let h = capture_headers(affinity_model(), session("session-affinity", None)).await;
        assert_eq!(
            h.get("session_id").map(String::as_str),
            Some("session-affinity")
        );
        assert_eq!(
            h.get("x-client-request-id").map(String::as_str),
            Some("session-affinity")
        );
        assert_eq!(
            h.get("x-session-affinity").map(String::as_str),
            Some("session-affinity")
        );
    }

    #[tokio::test]
    async fn omits_session_affinity_headers_when_cache_retention_none() {
        let h = capture_headers(
            affinity_model(),
            session("session-affinity", Some(CacheRetention::None)),
        )
        .await;
        assert!(!h.contains_key("session_id"));
        assert!(!h.contains_key("x-client-request-id"));
        assert!(!h.contains_key("x-session-affinity"));
    }

    #[tokio::test]
    async fn explicit_headers_override_generated_session_affinity_headers() {
        let opts = StreamOptions {
            session_id: Some("session-affinity".into()),
            headers: Some(HashMap::from([
                ("session_id".to_string(), "override-session".to_string()),
                (
                    "x-client-request-id".to_string(),
                    "override-request".to_string(),
                ),
                (
                    "x-session-affinity".to_string(),
                    "override-affinity".to_string(),
                ),
            ])),
            ..Default::default()
        };
        let h = capture_headers(affinity_model(), opts).await;
        assert_eq!(
            h.get("session_id").map(String::as_str),
            Some("override-session")
        );
        assert_eq!(
            h.get("x-client-request-id").map(String::as_str),
            Some("override-request")
        );
        assert_eq!(
            h.get("x-session-affinity").map(String::as_str),
            Some("override-affinity")
        );
    }
}
