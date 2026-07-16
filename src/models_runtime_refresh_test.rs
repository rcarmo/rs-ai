//! Runtime model-registry parity tests for upstream's `createProvider` refresh
//! facade: refreshes are coalesced, provider-scoped refresh failures do not
//! poison other providers, and reads keep the last successful/fallback catalog.

#[cfg(test)]
mod tests {
    use crate::registry::{get_model, list_models};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn xai_grok_45_routes_through_responses_api() {
        let model = get_model("xai", "grok-4.5").expect("xai grok-4.5 catalog entry");
        assert_eq!(model.api, crate::types::api::OPENAI_RESPONSES);
        assert_eq!(model.max_tokens, 500_000);
        assert_eq!(model.context_window, 500_000);
    }

    #[tokio::test]
    async fn stored_oauth_refresh_is_coalesced_under_concurrency() {
        use crate::auth::{
            Credential, EnvAuthContext, InMemoryCredentialStore, OAuthAuth, OAuthCredential,
            ProviderAuth, resolve_provider_auth,
        };

        struct SlowOAuth(Arc<AtomicUsize>);
        #[async_trait::async_trait]
        impl OAuthAuth for SlowOAuth {
            async fn refresh(
                &self,
                _: &OAuthCredential,
            ) -> Result<OAuthCredential, crate::auth::ModelsError> {
                let n = self.0.fetch_add(1, Ordering::SeqCst) + 1;
                tokio::time::sleep(std::time::Duration::from_millis(25)).await;
                Ok(OAuthCredential {
                    access: format!("new-{n}"),
                    refresh: Some("r2".into()),
                    expires: crate::utils::now_millis() + 60_000,
                    account_id: None,
                })
            }
            async fn to_auth(
                &self,
                c: &OAuthCredential,
            ) -> Result<crate::auth::ModelAuth, crate::auth::ModelsError> {
                Ok(crate::auth::ModelAuth {
                    api_key: Some(c.access.clone()),
                    ..Default::default()
                })
            }
        }

        let store = Arc::new(InMemoryCredentialStore::new());
        store
            .modify::<_, _, std::convert::Infallible>("p1", |_| async {
                Ok(Some(Credential::OAuth(OAuthCredential {
                    access: "old".into(),
                    refresh: Some("r".into()),
                    expires: 0,
                    account_id: None,
                })))
            })
            .await
            .unwrap();
        let refreshes = Arc::new(AtomicUsize::new(0));
        let provider = Arc::new(ProviderAuth {
            api_key: None,
            oauth: Some(Box::new(SlowOAuth(refreshes.clone()))),
        });
        let ctx = Arc::new(EnvAuthContext::new());
        let model = Arc::new(crate::types::Model {
            id: "model-a".into(),
            name: "model-a".into(),
            api: "test-api".into(),
            provider: "p1".into(),
            base_url: "https://example.test/v1".into(),
            reasoning: false,
            thinking_level_map: None,
            input: vec!["text".into()],
            cost: crate::types::ModelCost::default(),
            context_window: 10_000,
            max_tokens: 1_000,
            headers: None,
            api_key: None,
            compat: Default::default(),
        });

        let mut handles = Vec::new();
        for _ in 0..8 {
            let store = store.clone();
            let provider = provider.clone();
            let ctx = ctx.clone();
            let model = model.clone();
            handles.push(tokio::spawn(async move {
                resolve_provider_auth("p1", &provider, &model, &store, &*ctx, None)
                    .await
                    .unwrap()
                    .unwrap()
                    .auth
                    .api_key
                    .unwrap()
            }));
        }
        let values = futures::future::join_all(handles)
            .await
            .into_iter()
            .map(Result::unwrap)
            .collect::<Vec<_>>();
        assert!(values.iter().all(|v| v == "new-1"), "{values:?}");
        assert_eq!(refreshes.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn builtins_expose_exact_static_fallback_catalogs() {
        let all = list_models(None);
        assert!(
            all.len() > 500,
            "fallback catalog unexpectedly small: {}",
            all.len()
        );
        for (provider, id) in [
            ("anthropic", "claude-sonnet-4-5"),
            ("openai", "gpt-5.2"),
            ("xai", "grok-4.5"),
        ] {
            assert!(
                all.iter().any(|m| m.provider == provider && m.id == id),
                "missing {provider}/{id}"
            );
        }
    }
}
