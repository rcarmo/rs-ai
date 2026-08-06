//! Production runtime model-refresh parity for upstream `models.ts` / `models-store.ts`.
//! These tests exercise real `ModelsRuntime`/`RuntimeProvider` behavior rather than
//! static registry or unrelated OAuth coalescing.

#[cfg(test)]
mod tests {
    use crate::auth::{Credential, ModelsError, ModelsErrorCode, ProviderAuth};
    use crate::models_runtime::{
        InMemoryModelsStore, ModelsRuntime, ModelsStore, ModelsStoreEntry, RefreshOptions,
        RuntimeProvider,
    };
    use crate::oauth::{RadiusGatewayConfig, RadiusGatewayModel, RadiusOAuthCredentials};
    use crate::types::{Model, ModelCost};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};
    use tokio::sync::watch;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

    fn model(provider: &str, id: &str) -> Model {
        Model {
            id: id.into(),
            name: id.into(),
            api: "pi-messages".into(),
            provider: provider.into(),
            base_url: "https://example.test/v1".into(),
            reasoning: false,
            thinking_level_map: None,
            input: vec!["text".into()],
            cost: ModelCost::default(),
            context_window: 10,
            max_tokens: 5,
            sampling_params: None,
            headers: None,
            api_key: None,
            compat: Default::default(),
        }
    }

    struct ConfigSeq {
        calls: Arc<AtomicUsize>,
        responses: Vec<(u16, serde_json::Value)>,
    }
    impl Respond for ConfigSeq {
        fn respond(&self, _request: &Request) -> ResponseTemplate {
            let idx = self.calls.fetch_add(1, Ordering::SeqCst);
            let (status, body) = self
                .responses
                .get(idx)
                .or_else(|| self.responses.last())
                .cloned()
                .unwrap();
            ResponseTemplate::new(status).set_body_json(body)
        }
    }

    fn radius_config(base: &str, ids: &[&str]) -> serde_json::Value {
        serde_json::json!({
            "baseUrl": format!("{base}/v1"),
            "models": ids.iter().map(|id| serde_json::json!({
                "id": id,
                "name": id,
                "reasoning": false,
                "input": ["text"],
                "cost": {"input":0,"output":0,"cacheRead":0,"cacheWrite":0},
                "contextWindow": 10,
                "maxTokens": 5
            })).collect::<Vec<_>>()
        })
    }

    fn gateway_model(id: &str) -> RadiusGatewayModel {
        RadiusGatewayModel {
            id: id.into(),
            name: id.into(),
            reasoning: false,
            thinking_level_map: None,
            input: vec!["text".into()],
            cost: ModelCost::default(),
            context_window: 10,
            max_tokens: 5,
        }
    }

    #[tokio::test]
    async fn dynamic_refresh_replaces_removes_and_restores_provider_scoped_catalog() {
        let store = Arc::new(InMemoryModelsStore::new());
        store
            .write(
                "dyn",
                ModelsStoreEntry {
                    models: vec![model("dyn", "cached")],
                    last_modified: None,
                    checked_at: Some(1),
                    etag: None,
                },
            )
            .await
            .unwrap();
        store
            .write(
                "other",
                ModelsStoreEntry {
                    models: vec![model("other", "cached-other")],
                    last_modified: None,
                    checked_at: Some(1),
                    etag: None,
                },
            )
            .await
            .unwrap();
        let runtime = ModelsRuntime::with_models_store(store.clone());
        runtime.set_provider(RuntimeProvider::dynamic(
            "dyn",
            "Dynamic",
            ProviderAuth::default(),
            vec![model("dyn", "fallback")],
            |_ctx| async move { Ok(vec![model("dyn", "fresh")]) },
        ));
        runtime.set_provider(RuntimeProvider::static_provider(
            "other",
            "Other",
            ProviderAuth::default(),
            vec![model("other", "static")],
        ));

        let offline = runtime
            .refresh(RefreshOptions {
                allow_network: false,
                force: false,
                cancel: None,
            })
            .await;
        assert!(offline.errors.is_empty());
        assert!(runtime.get_model("dyn", "cached").is_some());
        assert!(runtime.get_model("dyn", "fresh").is_none());
        assert!(
            runtime.get_model("other", "cached-other").is_none(),
            "provider-scoped store entries cannot leak"
        );

        let online = runtime
            .refresh(RefreshOptions {
                allow_network: true,
                force: false,
                cancel: None,
            })
            .await;
        assert!(online.errors.is_empty());
        assert!(runtime.get_model("dyn", "fresh").is_some());
        assert!(
            runtime.get_model("dyn", "cached").is_none(),
            "remote list replaces/removes dynamic entries"
        );
        assert!(
            store
                .read("dyn")
                .await
                .unwrap()
                .unwrap()
                .models
                .iter()
                .any(|m| m.id == "fresh")
        );
    }

    #[tokio::test]
    async fn concurrent_refresh_is_deduped_and_failures_restore_cache_without_poisoning_others() {
        let store = Arc::new(InMemoryModelsStore::new());
        store
            .write(
                "bad",
                ModelsStoreEntry {
                    models: vec![model("bad", "cached")],
                    last_modified: None,
                    checked_at: Some(1),
                    etag: None,
                },
            )
            .await
            .unwrap();
        let runtime = Arc::new(ModelsRuntime::with_models_store(store));
        let calls = Arc::new(AtomicUsize::new(0));
        let calls_ok = calls.clone();
        runtime.set_provider(RuntimeProvider::dynamic(
            "ok",
            "OK",
            ProviderAuth::default(),
            vec![],
            move |_ctx| {
                let calls_ok = calls_ok.clone();
                async move {
                    calls_ok.fetch_add(1, Ordering::SeqCst);
                    tokio::time::sleep(std::time::Duration::from_millis(25)).await;
                    Ok(vec![model("ok", "fresh")])
                }
            },
        ));
        runtime.set_provider(RuntimeProvider::dynamic(
            "bad",
            "Bad",
            ProviderAuth::default(),
            vec![],
            |_ctx| async move { Err(ModelsError::new(ModelsErrorCode::ModelSource, "offline")) },
        ));

        let a = runtime.clone();
        let b = runtime.clone();
        let (ra, rb) = tokio::join!(
            async move {
                a.refresh(RefreshOptions {
                    allow_network: true,
                    force: false,
                    cancel: None,
                })
                .await
            },
            async move {
                b.refresh(RefreshOptions {
                    allow_network: true,
                    force: false,
                    cancel: None,
                })
                .await
            },
        );
        assert!(ra.errors.contains_key("bad") || rb.errors.contains_key("bad"));
        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "provider refresh is coalesced"
        );
        assert!(
            runtime.get_model("ok", "fresh").is_some(),
            "other providers still refresh"
        );
        assert!(
            runtime.get_model("bad", "cached").is_some(),
            "failure restores cached catalog"
        );
    }

    #[tokio::test]
    async fn cancellation_restores_cache_and_reports_aborted() {
        let store = Arc::new(InMemoryModelsStore::new());
        store
            .write(
                "dyn",
                ModelsStoreEntry {
                    models: vec![model("dyn", "cached")],
                    last_modified: None,
                    checked_at: Some(1),
                    etag: None,
                },
            )
            .await
            .unwrap();
        let runtime = ModelsRuntime::with_models_store(store);
        runtime.set_provider(RuntimeProvider::dynamic(
            "dyn",
            "Dynamic",
            ProviderAuth::default(),
            vec![],
            |_ctx| async move { Ok(vec![model("dyn", "fresh")]) },
        ));
        let (tx, rx) = watch::channel(true);
        let result = runtime
            .refresh(RefreshOptions {
                allow_network: true,
                force: false,
                cancel: Some(rx),
            })
            .await;
        assert!(result.aborted);
        assert!(runtime.get_model("dyn", "cached").is_some());
        assert!(runtime.get_model("dyn", "fresh").is_none());
        drop(tx);
    }

    #[tokio::test]
    async fn radius_gateway_config_is_wired_as_dynamic_provider_catalog() {
        let runtime = ModelsRuntime::new();
        let captured = Arc::new(Mutex::new(None::<Credential>));
        let captured_cb = captured.clone();
        runtime.set_provider(RuntimeProvider::dynamic(
            "radius",
            "Radius",
            ProviderAuth::default(),
            vec![model("radius", "fallback")],
            move |ctx| {
                let captured_cb = captured_cb.clone();
                async move {
                    *captured_cb.lock().unwrap() = ctx.credential.clone();
                    let oauth = RadiusOAuthCredentials {
                        access: "access".into(),
                        refresh: Some("refresh".into()),
                        expires: 1,
                        scope: None,
                        gateway_config: Some(RadiusGatewayConfig {
                            base_url: "https://radius/v1".into(),
                            models: vec![gateway_model("auto")],
                        }),
                    };
                    let radius = crate::auth_providers::RadiusOAuth::new("https://radius.test");
                    let models = radius.modify_models(&[], "radius", &oauth);
                    Ok(models)
                }
            },
        ));
        runtime
            .credentials
            .modify::<_, _, std::convert::Infallible>("radius", |_| async {
                Ok(Some(Credential::ApiKey(crate::auth::ApiKeyCredential {
                    key: Some("key".into()),
                    env: None,
                })))
            })
            .await
            .unwrap();
        let result = runtime
            .refresh(RefreshOptions {
                allow_network: true,
                force: false,
                cancel: None,
            })
            .await;
        assert!(result.errors.is_empty());
        let auto = runtime.get_model("radius", "auto").unwrap();
        assert_eq!(auto.api, "pi-messages");
        assert_eq!(auto.base_url, "https://radius/v1");
        assert!(matches!(
            &*captured.lock().unwrap(),
            Some(Credential::ApiKey(_))
        ));
    }

    #[tokio::test]
    async fn refresh_force_is_propagated_to_provider_context() {
        let runtime = ModelsRuntime::new();
        let seen = Arc::new(Mutex::new(Vec::<bool>::new()));
        let seen_cb = seen.clone();
        runtime.set_provider(RuntimeProvider::dynamic(
            "forcey",
            "Forcey",
            ProviderAuth::default(),
            vec![],
            move |ctx| {
                let seen_cb = seen_cb.clone();
                async move {
                    seen_cb.lock().unwrap().push(ctx.force);
                    Ok(vec![model("forcey", "m")])
                }
            },
        ));
        let a = runtime
            .refresh(RefreshOptions {
                allow_network: true,
                force: false,
                cancel: None,
            })
            .await;
        assert!(a.errors.is_empty());
        let b = runtime
            .refresh(RefreshOptions {
                allow_network: true,
                force: true,
                cancel: None,
            })
            .await;
        assert!(b.errors.is_empty());
        assert_eq!(&*seen.lock().unwrap(), &[false, true]);
    }

    #[tokio::test]
    async fn ordinary_registry_lookups_reflect_radius_refresh_replacement_and_cache_retention() {
        let server = MockServer::start().await;
        let calls = Arc::new(AtomicUsize::new(0));
        Mock::given(method("GET"))
            .and(path("/v1/config"))
            .respond_with(ConfigSeq {
                calls: calls.clone(),
                responses: vec![
                    (200, radius_config(&server.uri(), &["alpha"])),
                    (200, radius_config(&server.uri(), &["beta"])),
                    (503, serde_json::json!({"error":"offline"})),
                ],
            })
            .mount(&server)
            .await;

        crate::registry::register_radius_runtime_provider(&server.uri());
        let first =
            crate::registry::refresh_runtime_models(crate::models_runtime::RefreshOptions {
                allow_network: true,
                force: false,
                cancel: None,
            })
            .await;
        assert!(first.errors.is_empty());
        assert!(crate::registry::get_model("radius", "alpha").is_some());
        assert!(
            crate::registry::list_models(Some("radius"))
                .iter()
                .any(|m| m.id == "alpha")
        );

        let second =
            crate::registry::refresh_runtime_models(crate::models_runtime::RefreshOptions {
                allow_network: true,
                force: false,
                cancel: None,
            })
            .await;
        assert!(second.errors.is_empty());
        assert!(
            crate::registry::get_model("radius", "alpha").is_none(),
            "removed remote models disappear from ordinary lookups"
        );
        assert!(
            crate::registry::get_model("radius", "beta").is_some(),
            "new remote models appear in ordinary lookups"
        );

        let failed =
            crate::registry::refresh_runtime_models(crate::models_runtime::RefreshOptions {
                allow_network: true,
                force: false,
                cancel: None,
            })
            .await;
        assert!(failed.errors.contains_key("radius"));
        assert!(
            crate::registry::get_model("radius", "beta").is_some(),
            "network failure retains cached dynamic catalog"
        );

        let offline =
            crate::registry::refresh_runtime_models(crate::models_runtime::RefreshOptions {
                allow_network: false,
                force: false,
                cancel: None,
            })
            .await;
        assert!(offline.errors.is_empty());
        assert!(
            crate::registry::get_model("radius", "beta").is_some(),
            "offline refresh restores cached dynamic catalog"
        );
        assert_eq!(
            calls.load(Ordering::SeqCst),
            3,
            "offline refresh does not hit the network"
        );
    }

    #[test]
    fn default_runtime_populates_builtin_fallbacks_and_grok_45_routes_responses() {
        let runtime = ModelsRuntime::new();
        runtime.populate_builtin_fallbacks();
        let model = runtime
            .get_model("xai", "grok-4.5")
            .expect("xai grok-4.5 catalog entry");
        assert_eq!(model.api, crate::types::api::OPENAI_RESPONSES);
        assert_eq!(model.max_tokens, 500_000);
        assert!(runtime.get_models(None).len() > 500);
    }
}
