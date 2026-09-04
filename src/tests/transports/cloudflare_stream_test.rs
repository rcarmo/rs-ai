//! Test-for-test adaptation of upstream `test/cloudflare-stream.test.ts`.
//!
//! Cloudflare account/gateway placeholders materialize before normal request
//! dispatch when env is available, and remain unresolved when env is absent.

#[cfg(test)]
mod tests {
    use crate::events::Event;
    use crate::provider::openai::{build_openai_request_parts, stream_openai};
    use crate::registry;
    use crate::types::{ContentBlock, Context, Message, Model, ModelCost, Role, StreamOptions};
    use futures::StreamExt;
    use std::sync::LazyLock;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    static CF_ENV_GUARD: LazyLock<tokio::sync::Mutex<()>> =
        LazyLock::new(|| tokio::sync::Mutex::new(()));

    struct CfEnv;
    impl CfEnv {
        fn set(account: &str, gateway: &str) -> Self {
            unsafe {
                std::env::set_var("CLOUDFLARE_ACCOUNT_ID", account);
                std::env::set_var("CLOUDFLARE_GATEWAY_ID", gateway);
            }
            Self
        }
        fn clear() {
            unsafe {
                std::env::remove_var("CLOUDFLARE_ACCOUNT_ID");
                std::env::remove_var("CLOUDFLARE_GATEWAY_ID");
            }
        }
    }
    impl Drop for CfEnv {
        fn drop(&mut self) {
            Self::clear();
        }
    }

    fn model(base_url: &str) -> Model {
        Model {
            id: "model".into(),
            name: "model".into(),
            api: "openai-completions".into(),
            provider: "cloudflare-ai-gateway".into(),
            base_url: base_url.into(),
            reasoning: false,
            thinking_level_map: None,
            input: vec!["text".into()],
            cost: ModelCost::default(),
            context_window: 1000,
            max_tokens: 100,
            sampling_params: None,
            headers: None,
            api_key: Some("cf-key".into()),
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

    fn placeholder_url() -> &'static str {
        "https://gateway.ai.cloudflare.com/v1/{CLOUDFLARE_ACCOUNT_ID}/{CLOUDFLARE_GATEWAY_ID}/openai"
    }

    async fn serve_ok(server: &MockServer) {
        Mock::given(method("POST"))
            .and(path("/v1/account/gateway/openai/chat/completions"))
            .respond_with(ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_string("data: {\"choices\":[{\"delta\":{\"content\":\"cf\"},\"finish_reason\":null,\"index\":0}]}\n\ndata: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\",\"index\":0}]}\n\ndata: [DONE]\n\n"))
            .mount(server)
            .await;
    }

    #[tokio::test]
    async fn keeps_placeholders_when_provider_env_is_unresolved() {
        let _guard = CF_ENV_GUARD.lock().await;
        CfEnv::clear();
        let m = model(placeholder_url());
        let (url, _headers) = build_openai_request_parts(
            &m,
            &ctx(),
            &StreamOptions::default(),
            &crate::compat::detect_compat(&m),
            "cf-key",
        )
        .unwrap();
        assert_eq!(
            url,
            "https://gateway.ai.cloudflare.com/v1/{CLOUDFLARE_ACCOUNT_ID}/{CLOUDFLARE_GATEWAY_ID}/openai/chat/completions"
        );
    }

    #[tokio::test]
    async fn materializes_endpoint_before_normal_stream_dispatch() {
        let _guard = CF_ENV_GUARD.lock().await;
        let server = MockServer::start().await;
        serve_ok(&server).await;
        let _env = CfEnv::set("account", "gateway");
        let mut m = model(&format!(
            "{}/v1/{{CLOUDFLARE_ACCOUNT_ID}}/{{CLOUDFLARE_GATEWAY_ID}}/openai",
            server.uri()
        ));
        m.base_url = format!(
            "{}/v1/{{CLOUDFLARE_ACCOUNT_ID}}/{{CLOUDFLARE_GATEWAY_ID}}/openai",
            server.uri()
        );
        let context = ctx();
        let opts = StreamOptions::default();
        let mut stream = stream_openai(&m, &context, &opts);
        let mut text = String::new();
        while let Some(evt) = stream.next().await {
            if let Event::TextDelta { delta } = evt {
                text.push_str(&delta);
            }
        }
        assert_eq!(text, "cf");
        assert_eq!(server.received_requests().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn materializes_endpoint_before_stream_simple_dispatch() {
        let _guard = CF_ENV_GUARD.lock().await;
        let server = MockServer::start().await;
        serve_ok(&server).await;
        let _env = CfEnv::set("account", "gateway");
        let m = model(&format!(
            "{}/v1/{{CLOUDFLARE_ACCOUNT_ID}}/{{CLOUDFLARE_GATEWAY_ID}}/openai",
            server.uri()
        ));
        let context = ctx();
        let opts = StreamOptions::default();
        let mut stream = registry::stream_simple(&m, &context, &opts);
        let mut text = String::new();
        while let Some(evt) = stream.next().await {
            if let Event::TextDelta { delta } = evt {
                text.push_str(&delta);
            }
        }
        assert_eq!(text, "cf");
        assert_eq!(server.received_requests().await.unwrap().len(), 1);
    }
}
