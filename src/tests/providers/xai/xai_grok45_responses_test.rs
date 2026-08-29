//! Verifies upstream `test/xai-responses.test.ts` behavior for xAI Responses routing.

#[cfg(test)]
mod tests {
    use crate::events::Event;
    use crate::provider::responses::stream_responses;
    use crate::registry::get_model;
    use crate::simple_options::get_supported_thinking_levels;
    use crate::types::{
        CacheRetention, ContentBlock, Context, Message, ModelThinkingLevel, Role, StreamOptions,
        ThinkingLevel,
    };
    use serde_json::Value;
    use tokio_stream::StreamExt;
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn ctx() -> Context {
        Context {
            system_prompt: Some("You are a careful coding assistant.".into()),
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

    #[test]
    fn xai_catalog_uses_responses_and_expected_thinking_levels() {
        let grok_45 = get_model("xai", "grok-4.5").expect("xai/grok-4.5");
        assert_eq!(grok_45.api, crate::types::api::OPENAI_RESPONSES);
        assert_eq!(
            get_supported_thinking_levels(&grok_45),
            vec![
                ModelThinkingLevel::Low,
                ModelThinkingLevel::Medium,
                ModelThinkingLevel::High,
            ]
        );
        let grok_46 = get_model("xai", "grok-4.6").expect("xai/grok-4.6");
        assert_eq!(grok_46.api, crate::types::api::OPENAI_RESPONSES);
        assert_eq!(
            get_supported_thinking_levels(&grok_46),
            vec![
                ModelThinkingLevel::Low,
                ModelThinkingLevel::Medium,
                ModelThinkingLevel::High,
                ModelThinkingLevel::XHigh,
            ]
        );
        let grok_43 = get_model("xai", "grok-4.3").expect("xai/grok-4.3");
        assert_eq!(grok_43.api, crate::types::api::OPENAI_RESPONSES);
        assert_eq!(
            get_supported_thinking_levels(&grok_43),
            vec![
                ModelThinkingLevel::Off,
                ModelThinkingLevel::Low,
                ModelThinkingLevel::Medium,
                ModelThinkingLevel::High,
            ]
        );
        let grok_build = get_model("xai", "grok-build-0.1").expect("xai/grok-build-0.1");
        assert_eq!(grok_build.api, crate::types::api::OPENAI_RESPONSES);
        assert_eq!(
            get_supported_thinking_levels(&grok_build),
            vec![
                ModelThinkingLevel::Low,
                ModelThinkingLevel::Medium,
                ModelThinkingLevel::High
            ]
        );
        assert!(grok_45.compat.supports_long_cache_retention == Some(false));
    }

    #[tokio::test]
    async fn xai_grok_45_sends_actual_responses_request_shape() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_string("data: {\"type\":\"response.completed\",\"response\":{\"id\":\"r\",\"status\":\"completed\",\"usage\":{\"input_tokens\":1,\"output_tokens\":1,\"total_tokens\":2}}}\n\ndata: [DONE]\n\n"),
            )
            .mount(&server)
            .await;
        let mut model = get_model("xai", "grok-4.5").expect("xai/grok-4.5");
        model.base_url = server.uri();
        model.api_key = Some("xai-token".into());
        let c = ctx();
        let opts = StreamOptions {
            session_id: Some("pi-session-123".into()),
            cache_retention: Some(CacheRetention::Long),
            reasoning: Some(ThinkingLevel::Medium),
            ..Default::default()
        };
        let mut stream = stream_responses(&model, &c, &opts);
        while let Some(event) = stream.next().await {
            if let Event::Error { error, .. } = event {
                panic!("unexpected error: {error}");
            }
        }
        let reqs = server.received_requests().await.unwrap();
        assert_eq!(reqs.len(), 1);
        assert_eq!(reqs[0].url.path(), "/responses");
        assert_eq!(
            reqs[0]
                .headers
                .get("authorization")
                .unwrap()
                .to_str()
                .unwrap(),
            "Bearer xai-token"
        );
        assert_eq!(
            reqs[0].headers.get("session_id").unwrap().to_str().unwrap(),
            "pi-session-123"
        );
        let body: Value = serde_json::from_slice(&reqs[0].body).unwrap();
        assert_eq!(body["model"], "grok-4.5");
        assert_eq!(body["store"], false);
        assert_eq!(body["stream"], true);
        assert_eq!(body["prompt_cache_key"], "pi-session-123");
        assert!(
            body.get("prompt_cache_retention").is_none(),
            "xAI does not support long retention"
        );
        assert_eq!(
            body["reasoning"],
            serde_json::json!({"effort":"medium","summary":"auto"})
        );
        assert_eq!(
            body["include"],
            serde_json::json!(["reasoning.encrypted_content"])
        );
        let input = body["input"].as_array().expect("input array");
        assert!(input.iter().any(|item| {
            item.get("role") == Some(&serde_json::json!("developer"))
                && item.get("content")
                    == Some(&serde_json::json!("You are a careful coding assistant."))
        }));
    }

    #[tokio::test]
    async fn xai_grok_46_uses_responses_xhigh_encrypted_reasoning_and_user_agent_override() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_string("data: {\"type\":\"response.completed\",\"response\":{\"id\":\"r\",\"status\":\"completed\",\"usage\":{\"input_tokens\":1,\"output_tokens\":1,\"total_tokens\":2}}}\n\ndata: [DONE]\n\n"),
            )
            .mount(&server)
            .await;
        let mut model = get_model("xai", "grok-4.6").expect("xai/grok-4.6");
        model.base_url = server.uri();
        model.api_key = Some("xai-token".into());
        let c = ctx();
        let opts = StreamOptions {
            reasoning: Some(ThinkingLevel::XHigh),
            headers: Some(std::collections::HashMap::from([(
                "User-Agent".into(),
                "custom-agent".into(),
            )])),
            ..Default::default()
        };
        let mut stream = stream_responses(&model, &c, &opts);
        while let Some(event) = stream.next().await {
            if let Event::Error { error, .. } = event {
                panic!("unexpected error: {error}");
            }
        }
        let reqs = server.received_requests().await.unwrap();
        assert_eq!(reqs.len(), 1);
        assert_eq!(reqs[0].url.path(), "/responses");
        assert_eq!(
            reqs[0].headers.get("user-agent").unwrap().to_str().unwrap(),
            "custom-agent"
        );
        let body: Value = serde_json::from_slice(&reqs[0].body).unwrap();
        assert_eq!(body["model"], "grok-4.6");
        assert_eq!(body["store"], false);
        assert_eq!(body["stream"], true);
        assert_eq!(
            body["reasoning"],
            serde_json::json!({"effort":"xhigh","summary":"auto"})
        );
        assert_eq!(
            body["include"],
            serde_json::json!(["reasoning.encrypted_content"])
        );
    }
}
