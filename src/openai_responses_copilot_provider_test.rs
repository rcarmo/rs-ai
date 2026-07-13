//! Test-for-test port of upstream `test/openai-responses-copilot-provider.test.ts`
//! (`@earendil-works/pi-ai` v0.80.2) — openai-responses provider defaults
//! (reasoning-effort defaults, cache-affinity headers, prompt_cache_key clamp,
//! service-tier cost). All deterministic; driven via build_responses_payload and
//! stream_responses + wiremock.

#[cfg(test)]
mod tests {
    use crate::events::Event;
    use crate::provider::responses::{build_responses_payload, stream_responses};
    use crate::registry::get_model;
    use crate::types::{
        CacheRetention, ContentBlock, Context, Message, Model, Role, StreamOptions,
    };
    use serde_json::Value;
    use tokio_stream::StreamExt;
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

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
            }],
        }
    }

    fn payload(model: &Model, opts: &StreamOptions) -> Value {
        build_responses_payload(model, &ctx(), opts)
    }

    fn openai(id: &str) -> Model {
        get_model("openai", id).unwrap()
    }

    #[test]
    fn omits_reasoning_for_github_copilot_when_none_requested() {
        let m = get_model("github-copilot", "gpt-5-mini").unwrap();
        let p = payload(&m, &StreamOptions::default());
        assert!(p.get("reasoning").is_none());
    }

    #[test]
    fn sends_none_reasoning_effort_for_openai_models_when_none_requested() {
        for id in [
            "gpt-5.1",
            "gpt-5.2",
            "gpt-5.3-codex",
            "gpt-5.4",
            "gpt-5.4-mini",
            "gpt-5.4-nano",
            "gpt-5.5",
        ] {
            let p = payload(&openai(id), &StreamOptions::default());
            assert_eq!(
                p["reasoning"],
                serde_json::json!({"effort": "none"}),
                "model {id}"
            );
        }
    }

    #[test]
    fn omits_reasoning_effort_for_openai_models_when_off_unsupported() {
        for id in [
            "gpt-5",
            "gpt-5-mini",
            "gpt-5-nano",
            "gpt-5-pro",
            "gpt-5.2-pro",
            "gpt-5.4-pro",
            "gpt-5.5-pro",
        ] {
            let p = payload(&openai(id), &StreamOptions::default());
            assert!(
                p.get("reasoning").is_none(),
                "model {id} must omit reasoning"
            );
        }
    }

    #[test]
    fn clamps_prompt_cache_key_to_64_chars() {
        let opts = StreamOptions {
            session_id: Some("x".repeat(67)),
            ..Default::default()
        };
        let p = payload(&openai("gpt-5.4"), &opts);
        assert_eq!(p["prompt_cache_key"], serde_json::json!("x".repeat(64)));
    }

    // --- cache-affinity headers (via wiremock received_requests) ---

    async fn capture_headers(
        model: Model,
        opts: StreamOptions,
    ) -> (Option<String>, Option<String>) {
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
        let mut stream = stream_responses(&model, &c, &opts);
        while let Some(evt) = stream.next().await {
            if matches!(evt, Event::Done { .. } | Event::Error { .. }) {
                break;
            }
        }
        let reqs = server.received_requests().await.unwrap();
        let h = &reqs.last().unwrap().headers;
        (
            h.get("session_id").map(|v| v.to_str().unwrap().to_string()),
            h.get("x-client-request-id")
                .map(|v| v.to_str().unwrap().to_string()),
        )
    }

    fn opts_session(sid: &str) -> StreamOptions {
        StreamOptions {
            session_id: Some(sid.into()),
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn sets_cache_affinity_headers_for_official_openai_with_session() {
        let (sid, crid) = capture_headers(openai("gpt-5.4"), opts_session("session-123")).await;
        assert_eq!(sid.as_deref(), Some("session-123"));
        assert_eq!(crid.as_deref(), Some("session-123"));
    }

    #[tokio::test]
    async fn can_omit_session_id_header_while_preserving_other_affinity_headers() {
        let mut model = openai("gpt-5.4");
        model.provider = "opencode".into();
        model.compat.send_session_id_header = Some(false);
        let (sid, crid) = capture_headers(model, opts_session("session-123")).await;
        assert!(sid.is_none());
        assert_eq!(crid.as_deref(), Some("session-123"));
    }

    #[tokio::test]
    async fn explicit_headers_override_default_affinity_headers() {
        let opts = StreamOptions {
            session_id: Some("session-123".into()),
            headers: Some(std::collections::HashMap::from([
                ("session_id".to_string(), "override-session".to_string()),
                (
                    "x-client-request-id".to_string(),
                    "override-request".to_string(),
                ),
            ])),
            ..Default::default()
        };
        let (sid, crid) = capture_headers(openai("gpt-5.4"), opts).await;
        assert_eq!(sid.as_deref(), Some("override-session"));
        assert_eq!(crid.as_deref(), Some("override-request"));
    }

    #[tokio::test]
    async fn omits_affinity_headers_when_cache_retention_none() {
        let opts = StreamOptions {
            session_id: Some("session-123".into()),
            cache_retention: Some(CacheRetention::None),
            ..Default::default()
        };
        let (sid, crid) = capture_headers(openai("gpt-5.4"), opts).await;
        assert!(sid.is_none());
        assert!(crid.is_none());
    }

    // --- service-tier cost multipliers ---

    async fn cost_for(model_id: &str, tier: &str) -> (f64, f64, f64, Model) {
        let model = openai(model_id);
        let sse = format!(
            "data: {{\"type\":\"response.completed\",\"response\":{{\"status\":\"completed\",\"service_tier\":\"{tier}\",\"usage\":{{\"input_tokens\":1000000,\"output_tokens\":1000000,\"total_tokens\":2000000,\"input_tokens_details\":{{\"cached_tokens\":0}}}}}}}}\n\n"
        );
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_string(sse),
            )
            .mount(&server)
            .await;
        let mut m = model.clone();
        m.base_url = server.uri();
        m.api_key = Some("test-key".into());
        let opts = StreamOptions {
            service_tier: Some(tier.into()),
            ..Default::default()
        };
        let c = ctx();
        let mut stream = stream_responses(&m, &c, &opts);
        let mut usage = None;
        while let Some(evt) = stream.next().await {
            if let Event::Done { message, .. } = evt {
                usage = message.usage;
            }
        }
        let u = usage.unwrap();
        (u.cost.input, u.cost.output, u.cost.total, model)
    }

    #[tokio::test]
    async fn applies_service_tier_cost_multipliers() {
        for (id, tier, mult) in [
            ("gpt-5.4", "priority", 2.0),
            ("gpt-5.5", "priority", 2.5),
            ("gpt-5.5", "flex", 0.5),
        ] {
            let (ci, co, ct, model) = cost_for(id, tier).await;
            // v0.80.6: the fixture sends 1,000,000 input tokens, which exceeds the
            // request-wide pricing tier threshold, so the tier rate (not the base
            // rate) applies before the service-tier multiplier.
            let input_tokens = 1_000_000u64;
            let (rate_in, rate_out) = model
                .cost
                .tiers
                .iter()
                .filter(|t| input_tokens > t.input_tokens_above)
                .max_by_key(|t| t.input_tokens_above)
                .map(|t| (t.input, t.output))
                .unwrap_or((model.cost.input, model.cost.output));
            assert!((ci - rate_in * mult).abs() < 1e-9, "{id}/{tier} input cost");
            assert!(
                (co - rate_out * mult).abs() < 1e-9,
                "{id}/{tier} output cost"
            );
            assert!(
                (ct - (rate_in + rate_out) * mult).abs() < 1e-9,
                "{id}/{tier} total cost"
            );
        }
    }
}
