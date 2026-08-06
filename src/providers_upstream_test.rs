//! Test-for-test port of the deterministic subset of upstream
//! `test/providers.test.ts` (`@earendil-works/pi-ai` v0.80.2).
//!
//! Upstream exercises an *instance* `Models` collection built from
//! `createModels`/`createProvider`. rs-ai uses a global registry plus
//! `get_env_api_key` for ambient resolution, so the portable cases are mapped
//! onto those surfaces with identical expected values. The remaining upstream
//! cases are architectural / out of scope here:
//!   - cloudflare/vertex *scoped baseUrl + AuthResult.env* shaping is resolved
//!     at request time in `compat`/`env` (no AuthResult.env analogue), and the
//!     vertex ADC file path has no rs-ai credential plumbing;
//!   - `envApiKeyAuth.login` is app-owned (interactive prompt) and omitted;
//!   - dynamic `refreshModels` in-flight dedup is a JS instance-collection
//!     feature; rs-ai links provider catalogs statically.

#[cfg(test)]
mod tests {
    use crate::env::get_env_api_key;
    use crate::provider::faux::FauxProvider;
    use crate::registry::{self, ApiProvider};
    use crate::types::*;
    use std::sync::Arc;
    use std::sync::Mutex;
    use tokio_stream::StreamExt;

    // env-mutating tests share process-global state; serialize them.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    // ---- describe("builtin providers") -------------------------------------

    #[test]
    fn builtin_models_registers_every_builtin_provider_with_models() {
        registry::register_builtin_models();

        let providers = registry::list_providers();
        assert!(providers.contains(&"anthropic".to_string()));

        let anthropic = registry::get_model("anthropic", "claude-haiku-4-5");
        assert_eq!(
            anthropic.map(|m| m.api),
            Some("anthropic-messages".to_string())
        );

        let all = registry::list_models(None);
        assert!(
            all.len() > 500,
            "expected >500 builtin models, got {}",
            all.len()
        );

        // every provider lists at least one model and owns its models
        for provider in &providers {
            let list = registry::list_models(Some(provider));
            assert!(!list.is_empty(), "provider {provider} has no models");
            assert!(
                list.iter().all(|m| &m.provider == provider),
                "provider {provider} owns its models"
            );
        }
    }

    #[test]
    fn uses_official_kimi_k3_pricing_for_moonshot_providers() {
        for provider in ["moonshotai", "moonshotai-cn"] {
            let model = registry::get_model(provider, "kimi-k3").expect("kimi-k3 model");
            assert_eq!(model.cost.input, 3.0);
            assert_eq!(model.cost.output, 15.0);
            assert_eq!(model.cost.cache_read, 0.3);
            assert_eq!(model.cost.cache_write, 0.0);
        }
    }

    #[test]
    fn excludes_retired_xai_models_from_builtin_catalog() {
        for model_id in [
            "grok-3",
            "grok-3-fast",
            "grok-4.20-0309-non-reasoning",
            "grok-4.20-0309-reasoning",
            "grok-code-fast-1",
        ] {
            assert!(
                registry::get_model("xai", model_id).is_none(),
                "retired xai/{model_id} should not be registered"
            );
        }
    }

    #[test]
    fn resolves_anthropic_auth_from_env_with_oauth_token_precedence() {
        let _g = ENV_LOCK.lock().unwrap();
        unsafe {
            std::env::set_var("ANTHROPIC_API_KEY", "key");
            std::env::set_var("ANTHROPIC_OAUTH_TOKEN", "oauth-token");
        }
        let got = get_env_api_key("anthropic");
        unsafe {
            std::env::remove_var("ANTHROPIC_API_KEY");
            std::env::remove_var("ANTHROPIC_OAUTH_TOKEN");
        }
        // ANTHROPIC_OAUTH_TOKEN wins over ANTHROPIC_API_KEY.
        assert_eq!(got.as_deref(), Some("oauth-token"));
    }

    #[test]
    fn reports_bedrock_as_configured_from_ambient_aws_credentials_without_an_api_key() {
        let _g = ENV_LOCK.lock().unwrap();
        // unconfigured: no AWS creds present.
        unsafe {
            std::env::remove_var("AWS_PROFILE");
            std::env::remove_var("AWS_ACCESS_KEY_ID");
            std::env::remove_var("AWS_SECRET_ACCESS_KEY");
            std::env::remove_var("AWS_BEARER_TOKEN_BEDROCK");
        }
        assert!(get_env_api_key("amazon-bedrock").is_none());

        unsafe {
            std::env::set_var("AWS_PROFILE", "dev");
        }
        let configured = get_env_api_key("amazon-bedrock");
        unsafe {
            std::env::remove_var("AWS_PROFILE");
        }
        assert!(
            configured.is_some(),
            "AWS_PROFILE should mark bedrock configured"
        );
    }

    #[test]
    fn requires_cloudflare_workers_ai_account_config_env_key() {
        let _g = ENV_LOCK.lock().unwrap();
        unsafe {
            std::env::remove_var("CLOUDFLARE_API_KEY");
        }
        assert!(get_env_api_key("cloudflare-workers-ai").is_none());

        unsafe {
            std::env::set_var("CLOUDFLARE_API_KEY", "cf-key");
        }
        let got = get_env_api_key("cloudflare-workers-ai");
        unsafe {
            std::env::remove_var("CLOUDFLARE_API_KEY");
        }
        assert_eq!(got.as_deref(), Some("cf-key"));
    }

    // ---- describe("createProvider") ----------------------------------------

    #[tokio::test]
    async fn produces_a_stream_error_for_a_model_whose_api_has_no_implementation() {
        registry::register_builtin_models();
        let model = Model {
            id: "model-x".into(),
            name: "model-x".into(),
            api: "api-ghost".into(),
            provider: "mixed".into(),
            base_url: "https://example.test/v1".into(),
            reasoning: false,
            thinking_level_map: None,
            input: vec!["text".into()],
            cost: ModelCost::default(),
            context_window: 10000,
            max_tokens: 1000,
            sampling_params: None,
            headers: None,
            api_key: Some("k".into()),
            compat: Default::default(),
        };
        let ctx = Context {
            system_prompt: None,
            messages: vec![],
            tools: vec![],
        };
        let opts = StreamOptions::default();
        let mut stream = registry::stream(&model, &ctx, &opts);
        let mut err = None;
        while let Some(evt) = stream.next().await {
            if let crate::events::Event::Error { error, .. } = evt {
                err = Some(error.to_string());
            }
        }
        let err = err.expect("expected an error event for an unimplemented api");
        assert!(err.contains("No API provider registered"), "got: {err}");
    }

    // ---- describe("fauxProvider") ------------------------------------------

    fn assistant_text(text: &str, stop: StopReason) -> Message {
        Message {
            role: Role::Assistant,
            content: vec![ContentBlock::Text {
                text: text.into(),
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
            stop_reason: Some(stop),
            deferred: None,
            error_message: None,
            raw_stop_reason: None,
            tool_call_id: None,
            tool_name: None,
            is_error: false,
            details: None,
            added_tool_names: Vec::new(),
        }
    }

    fn faux_registry_model(api: &str, provider: &str) -> Model {
        Model {
            id: "faux".into(),
            name: "faux".into(),
            api: api.into(),
            provider: provider.into(),
            base_url: "http://localhost:1".into(),
            reasoning: false,
            thinking_level_map: None,
            input: vec!["text".into()],
            cost: ModelCost::default(),
            context_window: 10000,
            max_tokens: 1000,
            sampling_params: None,
            headers: None,
            api_key: None,
            compat: Default::default(),
        }
    }

    async fn terminal_message(
        mut stream: std::pin::Pin<
            Box<dyn futures::Stream<Item = crate::events::Event> + Send + '_>,
        >,
    ) -> Message {
        let mut terminal = None;
        while let Some(evt) = stream.next().await {
            match evt {
                crate::events::Event::Done { message, .. } => terminal = Some(message),
                crate::events::Event::Error { message, error, .. } => {
                    terminal = message
                        .or_else(|| Some(assistant_text(&error.to_string(), StopReason::Error)))
                }
                _ => {}
            }
        }
        terminal.expect("terminal message")
    }

    #[tokio::test]
    async fn faux_provider_streams_queued_responses() {
        let faux = FauxProvider::new("faux", "faux");
        faux.set_responses(vec![Message {
            role: Role::Assistant,
            content: vec![ContentBlock::Text {
                text: "hello from faux".into(),
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
            stop_reason: Some(StopReason::Stop),
            deferred: None,
            error_message: None,
            raw_stop_reason: None,
            tool_call_id: None,
            tool_name: None,
            is_error: false,
            details: None,
            added_tool_names: Vec::new(),
        }]);
        assert_eq!(faux.pending_response_count(), 1);

        let model = faux_registry_model("faux", "faux");
        let ctx = Context {
            system_prompt: None,
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
                deferred: None,
                error_message: None,
                raw_stop_reason: None,
                tool_call_id: None,
                tool_name: None,
                is_error: false,
                details: None,
                added_tool_names: Vec::new(),
            }],
            tools: vec![],
        };
        let opts = StreamOptions::default();
        let mut stream = faux.stream(&model, &ctx, &opts);
        let mut text = String::new();
        let mut reason = None;
        while let Some(evt) = stream.next().await {
            match evt {
                crate::events::Event::TextDelta { delta } => text.push_str(&delta),
                crate::events::Event::Done { reason: r, .. } => reason = Some(r),
                _ => {}
            }
        }
        assert_eq!(text, "hello from faux");
        assert_eq!(reason, Some(StopReason::Stop));
        assert_eq!(faux.pending_response_count(), 0);
    }

    #[tokio::test]
    async fn faux_provider_submits_polls_and_redeems_deferred_responses() {
        let api = "faux-deferred";
        let provider = "faux-deferred";
        let faux = FauxProvider::new_with_deferred(api, provider, 1, Some(25));
        registry::register_api(faux.clone() as Arc<dyn ApiProvider>);
        let model = faux_registry_model(api, provider);
        faux.set_responses(vec![assistant_text("ready", StopReason::Stop)]);
        let ctx = Context {
            system_prompt: None,
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
                deferred: None,
                error_message: None,
                raw_stop_reason: None,
                tool_call_id: None,
                tool_name: None,
                is_error: false,
                details: None,
                added_tool_names: Vec::new(),
            }],
            tools: vec![],
        };
        let opts = StreamOptions {
            deferred: Some(DeferredRequest {
                window: Some("1h".into()),
            }),
            ..Default::default()
        };
        let submitted = terminal_message(registry::stream(&model, &ctx, &opts)).await;
        assert_eq!(submitted.stop_reason, Some(StopReason::Deferred));
        assert!(submitted.content.is_empty());
        let handle = submitted.deferred.clone().expect("deferred handle");
        assert_eq!(handle.provider, provider);
        assert_eq!(handle.model_id, model.id);
        assert_eq!(handle.api, api);
        assert_eq!(handle.poll_after_ms, Some(25));

        let pending = terminal_message(registry::fetch_deferred(
            &model,
            &handle,
            &StreamOptions {
                wait: Some(0),
                ..Default::default()
            },
        ))
        .await;
        assert_eq!(pending.stop_reason, Some(StopReason::Deferred));
        assert_eq!(pending.deferred, Some(handle.clone()));

        let ready = terminal_message(registry::fetch_deferred(
            &model,
            &handle,
            &StreamOptions {
                wait: Some(0),
                ..Default::default()
            },
        ))
        .await;
        assert_eq!(ready.stop_reason, Some(StopReason::Stop));
        assert_eq!(content_to_plain_text(&ready.content), "ready");
        assert!(
            ready
                .usage
                .as_ref()
                .is_some_and(|usage| usage.total_tokens > 0)
        );
        assert_eq!(faux.call_count(), 1);
        assert_eq!(faux.deferred_fetch_count(), 2);
        registry::unregister_api(api);
    }

    #[tokio::test]
    async fn faux_provider_records_cancellation_and_fetches_cancelled_handle_as_error() {
        let api = "faux-cancel";
        let provider = "faux-cancel";
        let faux = FauxProvider::new(api, provider);
        registry::register_api(faux.clone() as Arc<dyn ApiProvider>);
        let model = faux_registry_model(api, provider);
        faux.set_responses(vec![assistant_text("cancelled", StopReason::Stop)]);
        let ctx = Context {
            system_prompt: None,
            messages: vec![],
            tools: vec![],
        };
        let submitted = terminal_message(registry::stream(
            &model,
            &ctx,
            &StreamOptions {
                deferred: Some(DeferredRequest::default()),
                ..Default::default()
            },
        ))
        .await;
        let handle = submitted.deferred.clone().expect("deferred handle");
        registry::cancel_deferred(&model, &handle, &StreamOptions::default())
            .await
            .unwrap();
        assert_eq!(faux.cancelled_deferred(), vec![handle.clone()]);
        let cancelled = terminal_message(registry::fetch_deferred(
            &model,
            &handle,
            &StreamOptions::default(),
        ))
        .await;
        assert_eq!(cancelled.stop_reason, Some(StopReason::Error));
        assert!(
            cancelled
                .error_message
                .as_deref()
                .unwrap_or_default()
                .contains("was cancelled")
        );
        registry::unregister_api(api);
    }

    #[tokio::test]
    async fn telemetry_context_flows_through_stream_and_deferred_fetch_options() {
        let api = "faux-telemetry";
        let provider = "faux-telemetry";
        let faux = FauxProvider::new(api, provider);
        registry::register_api(faux.clone() as Arc<dyn ApiProvider>);
        let model = faux_registry_model(api, provider);
        let ctx = Context {
            system_prompt: None,
            messages: vec![],
            tools: vec![],
        };
        faux.set_responses(vec![assistant_text("ready", StopReason::Stop)]);
        let submitted = terminal_message(registry::stream(
            &model,
            &ctx,
            &StreamOptions {
                telemetry_context: Some(serde_json::json!({"trace":"submit"})),
                deferred: Some(DeferredRequest::default()),
                ..Default::default()
            },
        ))
        .await;
        let handle = submitted.deferred.clone().expect("deferred handle");
        let _ready = terminal_message(registry::fetch_deferred(
            &model,
            &handle,
            &StreamOptions {
                telemetry_context: Some(serde_json::json!({"trace":"fetch"})),
                ..Default::default()
            },
        ))
        .await;
        assert_eq!(
            faux.telemetry_contexts(),
            vec![
                Some(serde_json::json!({"trace":"submit"})),
                Some(serde_json::json!({"trace":"fetch"})),
            ]
        );
        let image_opts = crate::images::openrouter::ImagesOptions {
            telemetry_context: Some(serde_json::json!({"trace":"image"})),
            ..Default::default()
        };
        assert_eq!(
            image_opts.telemetry_context,
            Some(serde_json::json!({"trace":"image"}))
        );
        registry::unregister_api(api);
    }

    #[tokio::test]
    async fn unsupported_deferred_capability_reports_in_band_provider_errors() {
        let faux = FauxProvider::new("faux-no-deferred", "faux-no-deferred");
        let model = faux_registry_model("faux-no-deferred", "faux-no-deferred");
        let handle = DeferredHandle {
            provider: model.provider.clone(),
            model_id: model.id.clone(),
            api: model.api.clone(),
            id: "missing".into(),
            expires_at: None,
            poll_after_ms: None,
            data: None,
        };
        let err =
            terminal_message(faux.fetch_deferred(&model, &handle, &StreamOptions::default())).await;
        assert_eq!(err.stop_reason, Some(StopReason::Error));
        assert!(
            err.error_message
                .as_deref()
                .unwrap_or_default()
                .contains("Unknown faux deferred response")
        );
    }

    fn content_to_plain_text(blocks: &[ContentBlock]) -> String {
        blocks
            .iter()
            .filter_map(|block| match block {
                ContentBlock::Text { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("")
    }
}
