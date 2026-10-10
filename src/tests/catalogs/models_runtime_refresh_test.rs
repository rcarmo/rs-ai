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
            input_limits: None,
            prompt_cache: None,
            enabled: None,
            lab: None,
            providers: None,
            cost: ModelCost::default(),
            context_window: 10,
            max_tokens: 5,
            sampling_params: None,
            sampling_params_by_thinking_level: None,
            headers: None,
            api_key: None,
            compat: Default::default(),
        }
    }

    struct GatedModelsStore {
        inner: InMemoryModelsStore,
        writes: AtomicUsize,
        entered: tokio::sync::Semaphore,
        release: tokio::sync::Semaphore,
        fail_first: bool,
    }
    impl GatedModelsStore {
        fn new(fail_first: bool) -> Self {
            Self {
                inner: InMemoryModelsStore::new(),
                writes: AtomicUsize::new(0),
                entered: tokio::sync::Semaphore::new(0),
                release: tokio::sync::Semaphore::new(0),
                fail_first,
            }
        }
    }
    #[async_trait::async_trait]
    impl ModelsStore for GatedModelsStore {
        async fn read(&self, id: &str) -> Result<Option<ModelsStoreEntry>, ModelsError> {
            self.inner.read(id).await
        }
        async fn write(&self, id: &str, entry: ModelsStoreEntry) -> Result<(), ModelsError> {
            if self.writes.fetch_add(1, Ordering::SeqCst) == 0 {
                self.entered.add_permits(1);
                self.release.acquire().await.unwrap().forget();
                if self.fail_first {
                    return Err(ModelsError::new(
                        ModelsErrorCode::ModelSource,
                        "write rejected",
                    ));
                }
            }
            self.inner.write(id, entry).await
        }
        async fn delete(&self, id: &str) -> Result<(), ModelsError> {
            self.inner.delete(id).await
        }
    }

    #[tokio::test]
    async fn in_flight_publications_are_serialized_and_publish_memory_only_after_storage() {
        let store = Arc::new(GatedModelsStore::new(false));
        let runtime = ModelsRuntime::with_models_store(store.clone());
        let calls = Arc::new(AtomicUsize::new(0));
        let calls_cb = calls.clone();
        runtime.set_provider(RuntimeProvider::dynamic(
            "dyn",
            "Dynamic",
            ProviderAuth::default(),
            vec![],
            move |_| {
                let calls = calls_cb.clone();
                async move {
                    Ok(vec![model(
                        "dyn",
                        &format!("generation-{}", calls.fetch_add(1, Ordering::SeqCst) + 1),
                    )])
                }
            },
        ));
        let options = RefreshOptions {
            allow_network: true,
            ..Default::default()
        };
        let mut first = Box::pin(runtime.refresh(options.clone()));
        tokio::select! {
            _ = store.entered.acquire() => {},
            _ = &mut first => panic!("publication should wait on storage"),
        }
        assert!(runtime.get_model("dyn", "generation-1").is_none());
        let mut second = Box::pin(runtime.refresh(options));
        assert!(futures::poll!(&mut second).is_pending());
        store.release.add_permits(1);
        let (a, b) = tokio::join!(first, second);
        assert!(a.errors.is_empty() && b.errors.is_empty());
        assert_eq!(
            store.inner.read("dyn").await.unwrap().unwrap().models[0].id,
            "generation-2"
        );
        assert!(runtime.get_model("dyn", "generation-2").is_some());
        assert!(runtime.get_model("dyn", "generation-1").is_none());
    }

    #[tokio::test]
    async fn cancelled_admitted_write_settles_without_publishing_cancelled_memory() {
        let store = Arc::new(GatedModelsStore::new(false));
        let runtime = ModelsRuntime::with_models_store(store.clone());
        runtime.set_provider(RuntimeProvider::dynamic(
            "dyn",
            "Dynamic",
            ProviderAuth::default(),
            vec![],
            |_| async { Ok(vec![model("dyn", "fresh")]) },
        ));
        let (tx, rx) = watch::channel(false);
        let mut first = Box::pin(runtime.refresh(RefreshOptions {
            allow_network: true,
            cancel: Some(rx),
            ..Default::default()
        }));
        tokio::select! { _ = store.entered.acquire() => {}, _ = &mut first => panic!("write not admitted") }
        tx.send(true).unwrap();
        let result = tokio::time::timeout(std::time::Duration::from_secs(1), first)
            .await
            .unwrap();
        assert!(result.aborted && result.errors.is_empty());
        assert!(runtime.get_model("dyn", "fresh").is_none());
        store.release.add_permits(1);
        tokio::time::timeout(std::time::Duration::from_secs(1), async {
            while store.inner.read("dyn").await.unwrap().is_none() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        assert!(runtime.get_model("dyn", "fresh").is_none());
    }

    #[tokio::test]
    async fn failed_publication_preserves_prior_memory_and_cache() {
        let store = Arc::new(GatedModelsStore::new(true));
        store
            .inner
            .write(
                "dyn",
                ModelsStoreEntry {
                    models: vec![model("dyn", "cached")],
                    last_modified: None,
                    checked_at: None,
                    etag: None,
                },
            )
            .await
            .unwrap();
        store.release.add_permits(1);
        let runtime = ModelsRuntime::with_models_store(store.clone());
        runtime.set_provider(RuntimeProvider::dynamic(
            "dyn",
            "Dynamic",
            ProviderAuth::default(),
            vec![],
            |_| async { Ok(vec![model("dyn", "fresh")]) },
        ));
        let result = runtime
            .refresh(RefreshOptions {
                allow_network: true,
                ..Default::default()
            })
            .await;
        assert!(result.errors.contains_key("dyn"));
        assert!(runtime.get_model("dyn", "cached").is_some());
        assert!(runtime.get_model("dyn", "fresh").is_none());
        assert_eq!(
            store.inner.read("dyn").await.unwrap().unwrap().models[0].id,
            "cached"
        );
    }

    #[tokio::test]
    async fn replaced_provider_supersedes_source_and_fences_captured_store_mutations() {
        let runtime = ModelsRuntime::new();
        let captured = Arc::new(Mutex::new(None));
        let capture = captured.clone();
        let entered = Arc::new(tokio::sync::Semaphore::new(0));
        let enter = entered.clone();
        runtime.set_provider(RuntimeProvider::dynamic(
            "dyn",
            "Old",
            ProviderAuth::default(),
            vec![],
            move |ctx| {
                let capture = capture.clone();
                let entered = enter.clone();
                async move {
                    *capture.lock().unwrap() = Some(ctx.store);
                    entered.add_permits(1);
                    std::future::pending().await
                }
            },
        ));
        let mut first = Box::pin(runtime.refresh(RefreshOptions {
            allow_network: true,
            ..Default::default()
        }));
        tokio::select! { _ = entered.acquire() => {}, _ = &mut first => panic!("old source did not start") }
        runtime.set_provider(RuntimeProvider::dynamic(
            "dyn",
            "New",
            ProviderAuth::default(),
            vec![],
            |_| async { Ok(vec![model("dyn", "new")]) },
        ));
        let second = runtime
            .refresh(RefreshOptions {
                allow_network: true,
                ..Default::default()
            })
            .await;
        assert!(second.errors.is_empty());
        assert!(
            tokio::time::timeout(std::time::Duration::from_secs(1), first)
                .await
                .unwrap()
                .errors
                .is_empty()
        );
        let stale: crate::models_runtime::ProviderModelsStore =
            captured.lock().unwrap().take().unwrap();
        stale.delete().await.unwrap();
        stale
            .write(ModelsStoreEntry {
                models: vec![model("dyn", "stale")],
                last_modified: None,
                checked_at: None,
                etag: None,
            })
            .await
            .unwrap();
        assert_eq!(
            runtime
                .models_store
                .read("dyn")
                .await
                .unwrap()
                .unwrap()
                .models[0]
                .id,
            "new"
        );
        assert!(runtime.get_model("dyn", "new").is_some());
    }

    #[tokio::test]
    async fn provider_delete_and_clear_cancel_pending_sources() {
        for clear in [false, true] {
            let runtime = ModelsRuntime::new();
            let entered = Arc::new(tokio::sync::Semaphore::new(0));
            let enter = entered.clone();
            runtime.set_provider(RuntimeProvider::dynamic(
                "dyn",
                "Dynamic",
                ProviderAuth::default(),
                vec![],
                move |_| {
                    let entered = enter.clone();
                    async move {
                        entered.add_permits(1);
                        std::future::pending().await
                    }
                },
            ));
            let mut refresh = Box::pin(runtime.refresh(RefreshOptions {
                allow_network: true,
                ..Default::default()
            }));
            tokio::select! { _ = entered.acquire() => {}, _ = &mut refresh => panic!("source did not start") }
            if clear {
                runtime.clear_providers();
            } else {
                runtime.delete_provider("dyn");
            }
            let result = tokio::time::timeout(std::time::Duration::from_secs(1), refresh)
                .await
                .unwrap();
            assert!(!result.aborted && result.errors.is_empty());
            assert!(runtime.get_models(None).is_empty());
            assert!(runtime.models_store.read("dyn").await.unwrap().is_none());
        }
    }

    #[tokio::test]
    async fn replacement_waits_for_older_admitted_write_before_final_publication() {
        let store = Arc::new(GatedModelsStore::new(false));
        let runtime = ModelsRuntime::with_models_store(store.clone());
        runtime.set_provider(RuntimeProvider::dynamic(
            "dyn",
            "Old",
            ProviderAuth::default(),
            vec![],
            |_| async { Ok(vec![model("dyn", "old")]) },
        ));
        let options = RefreshOptions {
            allow_network: true,
            ..Default::default()
        };
        let mut old = Box::pin(runtime.refresh(options.clone()));
        tokio::select! { _ = store.entered.acquire() => {}, _ = &mut old => panic!("write not admitted") }
        runtime.set_provider(RuntimeProvider::dynamic(
            "dyn",
            "New",
            ProviderAuth::default(),
            vec![],
            |_| async { Ok(vec![model("dyn", "new")]) },
        ));
        let mut new = Box::pin(runtime.refresh(options));
        assert!(futures::poll!(&mut new).is_pending());
        assert!(runtime.get_model("dyn", "new").is_none());
        store.release.add_permits(1);
        let (a, b) = tokio::join!(old, new);
        assert!(a.errors.is_empty() && b.errors.is_empty());
        assert_eq!(
            store.inner.read("dyn").await.unwrap().unwrap().models[0].id,
            "new"
        );
        assert!(runtime.get_model("dyn", "new").is_some());
        assert!(runtime.get_model("dyn", "old").is_none());
    }

    #[tokio::test]
    async fn dropping_refresh_aborts_owned_source_task() {
        struct DropNotice(Arc<tokio::sync::Semaphore>);
        impl Drop for DropNotice {
            fn drop(&mut self) {
                self.0.add_permits(1);
            }
        }
        let runtime = ModelsRuntime::new();
        let entered = Arc::new(tokio::sync::Semaphore::new(0));
        let dropped = Arc::new(tokio::sync::Semaphore::new(0));
        let enter = entered.clone();
        let drop_signal = dropped.clone();
        runtime.set_provider(RuntimeProvider::dynamic(
            "dyn",
            "Dynamic",
            ProviderAuth::default(),
            vec![],
            move |_| {
                let entered = enter.clone();
                let dropped = drop_signal.clone();
                async move {
                    let _guard = DropNotice(dropped);
                    entered.add_permits(1);
                    std::future::pending().await
                }
            },
        ));
        let mut refresh = Box::pin(runtime.refresh(RefreshOptions {
            allow_network: true,
            ..Default::default()
        }));
        tokio::select! { _ = entered.acquire() => {}, _ = &mut refresh => panic!("source did not start") }
        drop(refresh);
        tokio::time::timeout(std::time::Duration::from_secs(1), dropped.acquire())
            .await
            .unwrap()
            .unwrap()
            .forget();
        assert!(runtime.models_store.read("dyn").await.unwrap().is_none());
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

    struct CatalogOAuth {
        entered: Arc<tokio::sync::Semaphore>,
        release: Arc<tokio::sync::Semaphore>,
        count: Arc<AtomicUsize>,
    }
    #[async_trait::async_trait]
    impl crate::auth::OAuthAuth for CatalogOAuth {
        async fn refresh(
            &self,
            _credential: &crate::auth::OAuthCredential,
        ) -> Result<crate::auth::OAuthCredential, ModelsError> {
            self.count.fetch_add(1, Ordering::SeqCst);
            self.entered.add_permits(1);
            self.release.acquire().await.unwrap().forget();
            Ok(crate::auth::OAuthCredential {
                access: "fresh-catalog".into(),
                refresh: Some("rotated".into()),
                expires: crate::utils::now_millis() + 600_000,
                account_id: None,
            })
        }
        async fn to_auth(
            &self,
            _credential: &crate::auth::OAuthCredential,
        ) -> Result<crate::auth::ModelAuth, ModelsError> {
            panic!("catalog refresh uses the credential, not model request auth")
        }
    }

    #[tokio::test]
    async fn cancelled_model_refresh_preserves_rotation_then_reuses_fresh_credential() {
        let runtime = ModelsRuntime::new();
        runtime
            .credentials
            .modify::<_, _, std::convert::Infallible>("dyn", |_| async {
                Ok(Some(Credential::OAuth(crate::auth::OAuthCredential {
                    access: "old".into(),
                    refresh: Some("old-refresh".into()),
                    expires: 0,
                    account_id: None,
                })))
            })
            .await
            .unwrap();
        let entered = Arc::new(tokio::sync::Semaphore::new(0));
        let release = Arc::new(tokio::sync::Semaphore::new(0));
        let rotations = Arc::new(AtomicUsize::new(0));
        let seen = Arc::new(Mutex::new(Vec::new()));
        let seen_cb = seen.clone();
        runtime.set_provider(RuntimeProvider::dynamic(
            "dyn",
            "Dynamic",
            ProviderAuth {
                api_key: None,
                oauth: Some(Arc::new(CatalogOAuth {
                    entered: entered.clone(),
                    release: release.clone(),
                    count: rotations.clone(),
                })),
            },
            vec![],
            move |ctx| {
                let seen = seen_cb.clone();
                async move {
                    let Some(Credential::OAuth(credential)) = ctx.credential else {
                        panic!("missing OAuth credential")
                    };
                    seen.lock().unwrap().push(credential.access);
                    Ok(vec![model("dyn", "fresh")])
                }
            },
        ));
        let (tx, rx) = watch::channel(false);
        let mut first = Box::pin(runtime.refresh(RefreshOptions {
            allow_network: true,
            cancel: Some(rx),
            ..Default::default()
        }));
        tokio::select! {
            _ = entered.acquire() => {},
            _ = &mut first => panic!("refresh settled before OAuth release"),
        }
        tx.send(true).unwrap();
        let result = tokio::time::timeout(std::time::Duration::from_secs(1), first)
            .await
            .unwrap();
        assert!(result.aborted);
        assert!(result.errors.is_empty());
        assert!(
            seen.lock().unwrap().is_empty(),
            "cancelled refresh must not call model source"
        );
        release.add_permits(1);
        let result = runtime
            .refresh(RefreshOptions {
                allow_network: true,
                ..Default::default()
            })
            .await;
        assert!(!result.aborted);
        assert!(result.errors.is_empty());
        assert_eq!(&*seen.lock().unwrap(), &["fresh-catalog"]);
        assert_eq!(rotations.load(Ordering::SeqCst), 1);
        assert!(
            matches!(runtime.credentials.read("dyn"), Some(Credential::OAuth(c)) if c.refresh.as_deref() == Some("rotated"))
        );
        assert!(runtime.get_model("dyn", "fresh").is_some());
    }

    #[tokio::test]
    async fn superseding_catalog_refresh_reuses_admitted_rotation_without_stale_publish() {
        let runtime = ModelsRuntime::new();
        runtime
            .credentials
            .modify::<_, _, std::convert::Infallible>("dyn", |_| async {
                Ok(Some(Credential::OAuth(crate::auth::OAuthCredential {
                    access: "old".into(),
                    refresh: Some("old-refresh".into()),
                    expires: 0,
                    account_id: None,
                })))
            })
            .await
            .unwrap();
        let entered = Arc::new(tokio::sync::Semaphore::new(0));
        let release = Arc::new(tokio::sync::Semaphore::new(0));
        let rotations = Arc::new(AtomicUsize::new(0));
        let sources = Arc::new(AtomicUsize::new(0));
        let source_count = sources.clone();
        runtime.set_provider(RuntimeProvider::dynamic("dyn", "Dynamic", ProviderAuth {
            api_key: None,
            oauth: Some(Arc::new(CatalogOAuth { entered: entered.clone(), release: release.clone(), count: rotations.clone() })),
        }, vec![], move |ctx| {
            let sources = source_count.clone();
            async move {
                assert!(matches!(ctx.credential, Some(Credential::OAuth(c)) if c.access == "fresh-catalog"));
                sources.fetch_add(1, Ordering::SeqCst);
                Ok(vec![model("dyn", "fresh")])
            }
        }));
        let options = RefreshOptions {
            allow_network: true,
            ..Default::default()
        };
        let mut first = Box::pin(runtime.refresh(options.clone()));
        tokio::select! {
            _ = entered.acquire() => {},
            _ = &mut first => panic!("first refresh settled before rotation"),
        }
        let mut second = Box::pin(runtime.refresh(options));
        assert!(futures::poll!(&mut second).is_pending());
        release.add_permits(1);
        let (a, b) = tokio::join!(first, second);
        assert!(a.errors.is_empty() && b.errors.is_empty());
        assert_eq!(rotations.load(Ordering::SeqCst), 1);
        assert_eq!(
            sources.load(Ordering::SeqCst),
            1,
            "superseded attempt never calls the source"
        );
        assert!(runtime.get_model("dyn", "fresh").is_some());
    }

    #[tokio::test]
    async fn offline_model_refresh_never_rotates_expired_credentials() {
        let runtime = ModelsRuntime::new();
        runtime
            .credentials
            .modify::<_, _, std::convert::Infallible>("dyn", |_| async {
                Ok(Some(Credential::OAuth(crate::auth::OAuthCredential {
                    access: "old".into(),
                    refresh: None,
                    expires: 0,
                    account_id: None,
                })))
            })
            .await
            .unwrap();
        let rotations = Arc::new(AtomicUsize::new(0));
        runtime.set_provider(RuntimeProvider::dynamic(
            "dyn",
            "Dynamic",
            ProviderAuth {
                api_key: None,
                oauth: Some(Arc::new(CatalogOAuth {
                    entered: Arc::new(tokio::sync::Semaphore::new(0)),
                    release: Arc::new(tokio::sync::Semaphore::new(0)),
                    count: rotations.clone(),
                })),
            },
            vec![],
            |_| async { panic!("offline refresh must not call source") },
        ));
        let result = runtime.refresh(RefreshOptions::default()).await;
        assert!(result.errors.is_empty());
        assert_eq!(rotations.load(Ordering::SeqCst), 0);
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
                providers: None,
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
                providers: None,
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
    async fn concurrent_refreshes_and_failures_restore_cache_without_poisoning_others() {
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
                    providers: None,
                })
                .await
            },
            async move {
                b.refresh(RefreshOptions {
                    allow_network: true,
                    force: false,
                    cancel: None,
                    providers: None,
                })
                .await
            },
        );
        assert!(ra.errors.contains_key("bad") || rb.errors.contains_key("bad"));
        assert!(
            calls.load(Ordering::SeqCst) >= 1,
            "at least one provider refresh runs"
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
    async fn refresh_provider_filter_skips_unrequested_dynamic_providers() {
        let runtime = ModelsRuntime::new();
        let calls_a = Arc::new(AtomicUsize::new(0));
        let calls_b = Arc::new(AtomicUsize::new(0));
        for (id, calls) in [("a", calls_a.clone()), ("b", calls_b.clone())] {
            runtime.set_provider(RuntimeProvider::dynamic(
                id,
                id,
                ProviderAuth::default(),
                vec![],
                move |_ctx| {
                    let calls = calls.clone();
                    async move {
                        calls.fetch_add(1, Ordering::SeqCst);
                        Ok(vec![model("a", "fresh")])
                    }
                },
            ));
        }
        let result = runtime
            .refresh(RefreshOptions {
                allow_network: true,
                force: false,
                cancel: None,
                providers: Some(vec!["a".into(), "unknown".into()]),
            })
            .await;
        assert!(result.errors.is_empty());
        assert_eq!(calls_a.load(Ordering::SeqCst), 1);
        assert_eq!(calls_b.load(Ordering::SeqCst), 0);
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
                providers: None,
            })
            .await;
        assert!(result.aborted);
        assert!(runtime.get_model("dyn", "cached").is_some());
        assert!(runtime.get_model("dyn", "fresh").is_none());
        drop(tx);
    }

    #[tokio::test]
    async fn radius_remote_and_cached_catalogs_replace_shipped_defaults_even_when_empty() {
        for cached in [false, true] {
            let server = MockServer::start().await;
            if !cached {
                Mock::given(method("GET"))
                    .and(path("/v1/config"))
                    .respond_with(
                        ResponseTemplate::new(200).set_body_json(radius_config(&server.uri(), &[])),
                    )
                    .mount(&server)
                    .await;
            }
            let store = Arc::new(InMemoryModelsStore::new());
            if cached {
                store
                    .write(
                        "radius",
                        ModelsStoreEntry {
                            models: vec![],
                            last_modified: None,
                            checked_at: Some(1),
                            etag: None,
                        },
                    )
                    .await
                    .unwrap();
            }
            let runtime = ModelsRuntime::with_models_store(store.clone());
            runtime.set_provider(RuntimeProvider::radius(
                "radius",
                "Radius",
                server.uri(),
                vec![model("radius", "shipped-default")],
            ));
            assert!(runtime.get_model("radius", "shipped-default").is_some());
            let result = runtime
                .refresh(RefreshOptions {
                    allow_network: !cached,
                    ..Default::default()
                })
                .await;
            assert!(result.errors.is_empty());
            assert!(
                runtime.get_models(Some("radius")).is_empty(),
                "empty organization catalog replaces shipped defaults"
            );
            assert!(runtime.get_model("radius", "shipped-default").is_none());
            assert!(
                store
                    .read("radius")
                    .await
                    .unwrap()
                    .unwrap()
                    .models
                    .is_empty()
            );
            if cached {
                assert!(server.received_requests().await.unwrap().is_empty());
            }
        }
    }

    #[tokio::test]
    async fn radius_cached_organization_catalog_excludes_unconfigured_defaults() {
        let store = Arc::new(InMemoryModelsStore::new());
        store
            .write(
                "radius",
                ModelsStoreEntry {
                    models: vec![model("radius", "organization-only")],
                    last_modified: None,
                    checked_at: Some(1),
                    etag: None,
                },
            )
            .await
            .unwrap();
        let runtime = ModelsRuntime::with_models_store(store);
        runtime.set_provider(RuntimeProvider::radius(
            "radius",
            "Radius",
            "http://127.0.0.1:9",
            vec![model("radius", "shipped-default")],
        ));
        assert!(
            runtime
                .refresh(RefreshOptions::default())
                .await
                .errors
                .is_empty()
        );
        assert_eq!(
            runtime
                .get_models(Some("radius"))
                .iter()
                .map(|model| model.id.as_str())
                .collect::<Vec<_>>(),
            ["organization-only"]
        );
        assert!(runtime.get_model("radius", "shipped-default").is_none());
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
                providers: None,
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
    async fn refresh_abort_stops_waiting_on_non_cooperative_provider() {
        let runtime = Arc::new(ModelsRuntime::new());
        let calls = Arc::new(AtomicUsize::new(0));
        let calls_cb = calls.clone();
        let (started_tx, started_rx) = tokio::sync::oneshot::channel::<()>();
        let started = Arc::new(Mutex::new(Some(started_tx)));
        let started_cb = started.clone();
        runtime.set_provider(RuntimeProvider::dynamic(
            "dynamic",
            "Dynamic",
            ProviderAuth::default(),
            vec![],
            move |_ctx| {
                let calls_cb = calls_cb.clone();
                let started_cb = started_cb.clone();
                async move {
                    calls_cb.fetch_add(1, Ordering::SeqCst);
                    if let Some(tx) = started_cb.lock().unwrap().take() {
                        let _ = tx.send(());
                    }
                    std::future::pending::<Result<Vec<Model>, ModelsError>>().await
                }
            },
        ));
        let (tx, rx) = watch::channel(false);
        let runtime_for_task = runtime.clone();
        let pending = tokio::spawn(async move {
            runtime_for_task
                .refresh(RefreshOptions {
                    allow_network: true,
                    force: false,
                    cancel: Some(rx),
                    providers: Some(vec!["dynamic".into()]),
                })
                .await
        });
        started_rx.await.unwrap();
        tx.send(true).unwrap();
        let result = tokio::time::timeout(std::time::Duration::from_millis(250), pending)
            .await
            .expect("refresh should stop waiting after abort")
            .unwrap();
        assert!(result.aborted);
        assert!(result.errors.is_empty());
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn late_refresh_publication_is_rejected_after_supersession() {
        let store = Arc::new(InMemoryModelsStore::new());
        let runtime = Arc::new(ModelsRuntime::with_models_store(store.clone()));
        let calls = Arc::new(AtomicUsize::new(0));
        let (first_started_tx, first_started_rx) = tokio::sync::oneshot::channel::<()>();
        let first_started = Arc::new(Mutex::new(Some(first_started_tx)));
        let (finish_first_tx, finish_first_rx) = tokio::sync::oneshot::channel::<()>();
        let finish_first = Arc::new(Mutex::new(Some(finish_first_rx)));
        let calls_cb = calls.clone();
        let first_started_cb = first_started.clone();
        let finish_first_cb = finish_first.clone();
        runtime.set_provider(RuntimeProvider::dynamic(
            "dynamic",
            "Dynamic",
            ProviderAuth::default(),
            vec![],
            move |_ctx| {
                let calls_cb = calls_cb.clone();
                let first_started_cb = first_started_cb.clone();
                let finish_first_cb = finish_first_cb.clone();
                async move {
                    let current = calls_cb.fetch_add(1, Ordering::SeqCst) + 1;
                    if current == 1 {
                        if let Some(tx) = first_started_cb.lock().unwrap().take() {
                            let _ = tx.send(());
                        }
                        let rx = finish_first_cb.lock().unwrap().take().unwrap();
                        let _ = rx.await;
                    }
                    Ok(vec![model("dynamic", &format!("generation-{current}"))])
                }
            },
        ));
        let first_runtime = runtime.clone();
        let first = tokio::spawn(async move {
            first_runtime
                .refresh(RefreshOptions {
                    allow_network: true,
                    force: false,
                    cancel: None,
                    providers: Some(vec!["dynamic".into()]),
                })
                .await
        });
        first_started_rx.await.unwrap();
        let second = runtime
            .refresh(RefreshOptions {
                allow_network: true,
                force: false,
                cancel: None,
                providers: Some(vec!["dynamic".into()]),
            })
            .await;
        assert!(second.errors.is_empty());
        assert!(runtime.get_model("dynamic", "generation-2").is_some());
        // Supersession may abort the old source before it consumes the release.
        let _ = finish_first_tx.send(());
        let first_result = first.await.unwrap();
        assert!(first_result.errors.is_empty());
        assert!(runtime.get_model("dynamic", "generation-2").is_some());
        assert!(runtime.get_model("dynamic", "generation-1").is_none());
        assert_eq!(
            store.read("dynamic").await.unwrap().unwrap().models[0].id,
            "generation-2"
        );
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
                providers: None,
            })
            .await;
        assert!(a.errors.is_empty());
        let b = runtime
            .refresh(RefreshOptions {
                allow_network: true,
                force: true,
                cancel: None,
                providers: None,
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
                providers: None,
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
                providers: None,
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
                providers: None,
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
                providers: None,
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

    #[tokio::test]
    async fn direct_lookup_preserves_duplicate_override_order_and_caller_isolation() {
        let runtime = ModelsRuntime::new();
        let mut baseline_first = model("ordered", "same");
        baseline_first.name = "baseline-first".into();
        let mut baseline_second = model("ordered", "same");
        baseline_second.name = "baseline-second".into();
        runtime.set_provider(RuntimeProvider::dynamic(
            "ordered",
            "Ordered",
            ProviderAuth::default(),
            vec![baseline_first, baseline_second],
            |_ctx| async move {
                let mut dynamic_first = model("ordered", "same");
                dynamic_first.name = "dynamic-first".into();
                let mut dynamic_last = model("ordered", "same");
                dynamic_last.name = "dynamic-last".into();
                Ok(vec![dynamic_first, dynamic_last])
            },
        ));

        assert_eq!(
            runtime.get_model("ordered", "same").unwrap().name,
            "baseline-first"
        );
        let refreshed = runtime
            .refresh(RefreshOptions {
                allow_network: true,
                force: false,
                cancel: None,
                providers: Some(vec!["ordered".into()]),
            })
            .await;
        assert!(refreshed.errors.is_empty());
        let mut selected = runtime.get_model("ordered", "same").unwrap();
        assert_eq!(selected.name, "dynamic-last");
        selected.name = "caller-mutated".into();
        assert_eq!(
            runtime.get_model("ordered", "same").unwrap().name,
            "dynamic-last",
            "callers receive an isolated clone"
        );
        assert!(runtime.get_model("ordered", "missing").is_none());
        assert!(runtime.get_model("missing-provider", "same").is_none());
    }

    #[test]
    fn static_direct_lookup_uses_first_baseline_duplicate() {
        let runtime = ModelsRuntime::new();
        let mut first = model("static", "duplicate");
        first.name = "first".into();
        let mut second = model("static", "duplicate");
        second.name = "second".into();
        runtime.set_provider(RuntimeProvider::static_provider(
            "static",
            "Static",
            ProviderAuth::default(),
            vec![first, second],
        ));
        assert_eq!(
            runtime.get_model("static", "duplicate").unwrap().name,
            "first"
        );
    }

    #[test]
    fn direct_lookup_remains_valid_during_provider_replacement() {
        let runtime = Arc::new(ModelsRuntime::new());
        runtime.set_provider(RuntimeProvider::static_provider(
            "replace",
            "Replace",
            ProviderAuth::default(),
            vec![model("replace", "value")],
        ));
        let writer = runtime.clone();
        let replacing = std::thread::spawn(move || {
            for generation in 1..=200 {
                let mut replacement = model("replace", "value");
                replacement.name = format!("generation-{generation}");
                writer.set_provider(RuntimeProvider::static_provider(
                    "replace",
                    "Replace",
                    ProviderAuth::default(),
                    vec![replacement],
                ));
            }
        });
        for _ in 0..2_000 {
            assert!(runtime.get_model("replace", "value").is_some());
        }
        replacing.join().unwrap();
        assert_eq!(
            runtime.get_model("replace", "value").unwrap().name,
            "generation-200"
        );
    }

    #[test]
    fn production_registry_lookup_returns_an_isolated_match() {
        let provider = "lookup-isolation-test-provider";
        let id = "lookup-isolation-test-model";
        let mut registered = model(provider, id);
        registered.name = "registered".into();
        crate::registry::register_model(registered);

        let mut selected = crate::registry::get_model(provider, id).unwrap();
        assert_eq!(selected.name, "registered");
        selected.name = "caller-mutated".into();
        assert_eq!(
            crate::registry::get_model(provider, id).unwrap().name,
            "registered"
        );
        assert!(crate::registry::get_model(provider, "missing").is_none());
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
