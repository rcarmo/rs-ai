//! Release-pinned v0.87.1 catalog and provider behavior coverage.

#[cfg(test)]
mod tests {
    use crate::provider::anthropic::stream_anthropic;
    use crate::provider::codex::build_codex_payload;
    use crate::provider::responses::build_responses_payload;
    use crate::registry::get_model;
    use crate::simple_options::get_supported_thinking_levels;
    use crate::types::{
        CacheRetention, Context, ModelThinkingLevel, StreamOptions, ThinkingLevel, user_message,
    };
    use serde_json::json;
    use std::collections::HashSet;
    use tokio_stream::StreamExt;
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn context() -> Context {
        Context {
            system_prompt: Some("You are a helpful assistant.".into()),
            messages: vec![user_message("Hello")],
            tools: Vec::new(),
        }
    }

    #[test]
    fn release_pinned_catalog_counts_match_v0871() {
        let models = crate::models_generated::builtin_models();
        let providers = models
            .iter()
            .map(|model| model.provider.as_str())
            .collect::<HashSet<_>>();
        let apis = models
            .iter()
            .map(|model| model.api.as_str())
            .collect::<HashSet<_>>();
        assert_eq!(models.len(), 1495);
        assert_eq!(providers.len(), 41);
        assert_eq!(apis.len(), 10);
        assert_eq!(
            models
                .iter()
                .filter(|model| model.id.contains(":batch"))
                .count(),
            71
        );

        let images = crate::images::models_generated::builtin_image_models();
        assert_eq!(images.len(), 55);
        assert!(images.iter().any(|model| {
            model.provider == "openrouter" && model.id == "inclusionai/ming-image-0.1-design"
        }));
    }

    #[test]
    fn grok_4_7_catalog_matches_upstream() {
        let model = get_model("xai", "grok-4.7").expect("xai/grok-4.7");
        assert_eq!(model.api, crate::types::api::OPENAI_RESPONSES);
        assert_eq!(model.input, vec!["text".to_string(), "image".to_string()]);
        assert_eq!(model.context_window, 500_000);
        assert_eq!(model.max_tokens, 500_000);
        assert_eq!(model.cost.input, 2.0);
        assert_eq!(model.cost.output, 6.0);
        assert_eq!(model.cost.cache_read, 0.5);
        assert_eq!(model.cost.tiers.len(), 1);
        assert_eq!(model.cost.tiers[0].input_tokens_above, 200_000);
        assert_eq!(model.cost.tiers[0].input, 4.0);
        assert_eq!(model.cost.tiers[0].output, 12.0);
        assert_eq!(
            get_supported_thinking_levels(&model),
            vec![
                ModelThinkingLevel::Low,
                ModelThinkingLevel::Medium,
                ModelThinkingLevel::High,
                ModelThinkingLevel::XHigh,
            ]
        );

        let payload = build_responses_payload(
            &model,
            &context(),
            &StreamOptions {
                reasoning: Some(ThinkingLevel::XHigh),
                ..Default::default()
            },
        );
        assert_eq!(
            payload["reasoning"],
            json!({"effort": "xhigh", "summary": "auto"})
        );
        assert_eq!(payload["include"], json!(["reasoning.encrypted_content"]));
    }

    #[test]
    fn claude_opus_5_5_catalog_matches_upstream() {
        let model = get_model("anthropic", "claude-opus-5-5").expect("anthropic/claude-opus-5-5");
        assert_eq!(model.api, crate::types::api::ANTHROPIC_MESSAGES);
        assert_eq!(model.input, vec!["text".to_string(), "image".to_string()]);
        assert_eq!(model.context_window, 1_000_000);
        assert_eq!(model.max_tokens, 128_000);
        assert_eq!(model.cost.input, 4.0);
        assert_eq!(model.cost.output, 20.0);
        assert_eq!(model.cost.cache_read, 0.2);
        assert_eq!(model.cost.cache_write, 5.0);
        assert_eq!(
            get_supported_thinking_levels(&model),
            vec![
                ModelThinkingLevel::Low,
                ModelThinkingLevel::Medium,
                ModelThinkingLevel::High,
                ModelThinkingLevel::XHigh,
                ModelThinkingLevel::Max,
            ]
        );
        assert_eq!(model.compat.force_adaptive_thinking, Some(true));
        assert_eq!(model.compat.supports_mid_convo_effort, Some(true));
        assert_eq!(model.compat.supports_temperature, Some(false));

        let copilot =
            get_model("github-copilot", "claude-opus-5.5").expect("github-copilot/claude-opus-5.5");
        assert_eq!(copilot.api, crate::types::api::ANTHROPIC_MESSAGES);
        assert_eq!(copilot.context_window, 1_000_000);
        assert_eq!(copilot.max_tokens, 128_000);
        assert_eq!(
            get_supported_thinking_levels(&copilot),
            vec![
                ModelThinkingLevel::Low,
                ModelThinkingLevel::Medium,
                ModelThinkingLevel::High,
                ModelThinkingLevel::XHigh,
                ModelThinkingLevel::Max,
            ]
        );
    }

    #[test]
    fn gpt_6_sol_and_luna_catalog_cache_and_default_reasoning_match_upstream() {
        for (id, input, output) in [("gpt-6-sol", 2.0, 10.0), ("gpt-6-luna", 0.1, 0.5)] {
            let model = get_model("openai", id).unwrap_or_else(|| panic!("openai/{id}"));
            assert_eq!(model.api, crate::types::api::OPENAI_RESPONSES);
            assert_eq!(model.context_window, 272_000);
            assert_eq!(model.max_tokens, 128_000);
            assert_eq!(model.cost.input, input);
            assert_eq!(model.cost.output, output);
            let levels = get_supported_thinking_levels(&model);
            assert!(
                levels.contains(&ModelThinkingLevel::XHigh),
                "{id}: {levels:?}"
            );
            assert!(
                levels.contains(&ModelThinkingLevel::Max),
                "{id}: {levels:?}"
            );

            let payload = build_responses_payload(
                &model,
                &context(),
                &StreamOptions {
                    cache_retention: Some(CacheRetention::Long),
                    session_id: Some("session-2".into()),
                    ..Default::default()
                },
            );
            assert_eq!(payload["reasoning"], json!({"effort": "none"}), "{id}");
            assert_eq!(payload["prompt_cache_key"], "session-2", "{id}");
            assert_eq!(
                payload["prompt_cache_options"],
                json!({"ttl": "30m"}),
                "{id}"
            );
            assert!(payload.get("prompt_cache_retention").is_none(), "{id}");

            let codex =
                get_model("openai-codex", id).unwrap_or_else(|| panic!("openai-codex/{id}"));
            let codex_payload = build_codex_payload(
                &codex,
                &context(),
                &StreamOptions {
                    reasoning: Some(ThinkingLevel::Max),
                    ..Default::default()
                },
            );
            assert_eq!(
                codex_payload["reasoning"],
                json!({"effort": "max", "summary": "auto"}),
                "{id}"
            );

            let copilot =
                get_model("github-copilot", id).unwrap_or_else(|| panic!("github-copilot/{id}"));
            assert_eq!(copilot.api, crate::types::api::OPENAI_RESPONSES);
            assert_eq!(copilot.context_window, 1_000_000);
            assert_eq!(copilot.max_tokens, 128_000);
        }
    }

    #[tokio::test]
    async fn anthropic_oauth_uses_claude_code_2_1_280_user_agent() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_string(""),
            )
            .mount(&server)
            .await;
        let mut model = get_model("anthropic", "claude-opus-5-5").unwrap();
        model.base_url = server.uri();
        model.api_key = Some("sk-ant-oat01-test".into());
        let context = context();
        let options = StreamOptions::default();
        let mut stream = stream_anthropic(&model, &context, &options);
        while stream.next().await.is_some() {}
        let requests = server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(
            requests[0].headers["user-agent"].to_str().unwrap(),
            "claude-cli/2.1.280"
        );
        assert_eq!(requests[0].headers["x-app"].to_str().unwrap(), "cli");
        assert_eq!(
            requests[0].headers["authorization"].to_str().unwrap(),
            "Bearer sk-ant-oat01-test"
        );
        assert!(
            !requests[0].headers.contains_key("x-api-key"),
            "OAuth requests must not send x-api-key"
        );
    }
}
