//! Test-for-test port of upstream `test/github-copilot-anthropic.test.ts`
//! (`@earendil-works/pi-ai` v0.80.2): Copilot Claude via Anthropic Messages —
//! adaptive-thinking effort overrides, Bearer auth + Copilot headers + valid
//! payload, and interleaved-thinking beta omission for adaptive models.

#[cfg(test)]
mod tests {
    use crate::provider::anthropic::stream_anthropic;
    use crate::registry::get_model;
    use crate::simple_options::get_supported_thinking_levels;
    use crate::types::{Context, ContentBlock, Message, Model, Role, StreamOptions};
    use crate::events::Event;
    use serde_json::Value;
    use tokio_stream::StreamExt;
    use wiremock::{Mock, MockServer, ResponseTemplate};
    use wiremock::matchers::method;

    fn ctx() -> Context {
        Context {
            system_prompt: Some("You are a helpful assistant.".into()), tools: Vec::new(),
            messages: vec![Message {
                role: Role::User, content: vec![ContentBlock::Text { text: "Hello".into(), text_signature: None }],
                timestamp: 0, api: None, provider: None, model: None, response_id: None,
                response_model: None, diagnostics: Vec::new(), usage: None,
                stop_reason: None, error_message: None,
                tool_call_id: None, tool_name: None, is_error: false, details: None,
            }],
        }
    }

    fn levels(m: &Model) -> Vec<String> {
        get_supported_thinking_levels(m).into_iter()
            .map(|l| serde_json::to_value(l).unwrap().as_str().unwrap().to_string()).collect()
    }

    #[test]
    fn applies_copilot_specific_adaptive_thinking_effort_overrides() {
        let opus47 = get_model("github-copilot", "claude-opus-4.7").unwrap();
        let map = opus47.thinking_level_map.as_ref().unwrap();
        assert_eq!(map.get("minimal"), Some(&Some("low".to_string())));
        assert_eq!(map.get("xhigh"), Some(&Some("xhigh".to_string())));
        assert!(levels(&opus47).iter().any(|l| l == "xhigh"));

        let sonnet46 = get_model("github-copilot", "claude-sonnet-4.6").unwrap();
        let map = sonnet46.thinking_level_map.as_ref().unwrap();
        assert_eq!(map.get("minimal"), Some(&Some("low".to_string())));
        assert_eq!(map.get("xhigh"), Some(&Some("max".to_string())));
        assert!(levels(&sonnet46).iter().any(|l| l == "xhigh"));
    }

    async fn run(opts: StreamOptions) -> (Value, std::collections::HashMap<String, String>) {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).insert_header("content-type", "text/event-stream").set_body_string(concat!(
                "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_test\",\"usage\":{\"input_tokens\":10,\"output_tokens\":0}}}\n\n",
                "event: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":5}}\n\n",
                "event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n",
            )))
            .mount(&server).await;
        let mut model = get_model("github-copilot", "claude-sonnet-4.6").unwrap();
        model.base_url = server.uri();
        let c = ctx();
        let mut stream = stream_anthropic(&model, &c, &opts);
        while let Some(evt) = stream.next().await {
            if matches!(evt, Event::Done { .. } | Event::Error { .. }) { break; }
        }
        let reqs = server.received_requests().await.unwrap();
        let req = reqs.last().unwrap();
        let body: Value = serde_json::from_slice(&req.body).unwrap();
        let headers = req.headers.iter()
            .map(|(k, v)| (k.as_str().to_string(), v.to_str().unwrap_or("").to_string()))
            .collect();
        (body, headers)
    }

    #[tokio::test]
    async fn uses_bearer_auth_copilot_headers_and_valid_anthropic_payload() {
        let model = get_model("github-copilot", "claude-sonnet-4.6").unwrap();
        assert_eq!(model.api, "anthropic-messages");

        let opts = StreamOptions { api_key: Some("tid_copilot_session_test_token".into()), ..Default::default() };
        let (body, headers) = run(opts).await;

        // Bearer auth (not x-api-key).
        assert_eq!(headers.get("authorization").map(String::as_str), Some("Bearer tid_copilot_session_test_token"));
        assert!(!headers.contains_key("x-api-key"), "copilot must not send x-api-key");

        // Static copilot headers (from catalog) + dynamic headers.
        assert!(headers.get("user-agent").is_some_and(|v| v.contains("GitHubCopilotChat")));
        assert_eq!(headers.get("copilot-integration-id").map(String::as_str), Some("vscode-chat"));
        assert_eq!(headers.get("x-initiator").map(String::as_str), Some("user"));
        assert_eq!(headers.get("openai-intent").map(String::as_str), Some("conversation-edits"));

        // No fine-grained-tool-streaming beta for Copilot.
        let beta = headers.get("anthropic-beta").cloned().unwrap_or_default();
        assert!(!beta.contains("fine-grained-tool-streaming"));

        // Valid Anthropic Messages payload.
        assert_eq!(body["model"], serde_json::json!("claude-sonnet-4.6"));
        assert_eq!(body["stream"], serde_json::json!(true));
        assert_eq!(body["max_tokens"], serde_json::json!(model.max_tokens));
        assert!(body["messages"].is_array());
    }

    #[tokio::test]
    async fn omits_interleaved_thinking_beta_for_adaptive_thinking_models() {
        let opts = StreamOptions {
            api_key: Some("tid_copilot_session_test_token".into()),
            interleaved_thinking: Some(true),
            ..Default::default()
        };
        let (_body, headers) = run(opts).await;
        let beta = headers.get("anthropic-beta").cloned().unwrap_or_default();
        assert!(!beta.contains("interleaved-thinking-2025-05-14"), "adaptive models omit interleaved-thinking beta: {beta}");
    }
}
