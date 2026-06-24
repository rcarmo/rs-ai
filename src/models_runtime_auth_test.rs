//! Test-for-test port of the auth-resolution subset of upstream
//! `test/models-runtime.test.ts` (`@earendil-works/pi-ai` v0.80.2).
//!
//! `Models.getAuth()` maps to rs-ai's `resolve_provider_auth` over an
//! `InMemoryCredentialStore`. The instance-based `Models` collection itself
//! (setProvider/getProviders/refresh/completeSimple auth-merge) is an
//! architectural difference (rs-ai uses a global registry), tracked separately;
//! the credential/OAuth resolution semantics are ported here.

#[cfg(test)]
mod tests {
    use crate::auth::{
        resolve_provider_auth, ApiKeyAuth, ApiKeyCredential, AuthContext, AuthResult, Credential,
        EnvAuthContext, InMemoryCredentialStore, ModelsError, ModelsErrorCode, OAuthAuth,
        OAuthCredential, ProviderAuth,
    };
    use crate::types::{Model, ModelCost};
    use crate::utils::now_millis;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    fn model() -> Model {
        Model {
            id: "model-a".into(), name: "model-a".into(), api: "test-api".into(),
            provider: "p1".into(), base_url: "https://example.test/v1".into(), reasoning: false,
            thinking_level_map: None, input: vec!["text".into()], cost: ModelCost::default(),
            context_window: 10000, max_tokens: 1000, headers: None, api_key: None, compat: Default::default(),
        }
    }

    struct EnvKeyAuth;
    #[async_trait::async_trait]
    impl ApiKeyAuth for EnvKeyAuth {
        async fn resolve(&self, _m: &Model, _c: &dyn AuthContext, cred: Option<&ApiKeyCredential>)
            -> Result<Option<AuthResult>, ModelsError> {
            Ok(cred.and_then(|c| c.key.clone()).map(|k| AuthResult {
                auth: crate::auth::ModelAuth { api_key: Some(k), ..Default::default() },
                env: None, source: Some("stored".into()),
            }))
        }
    }

    /// `a stored credential without a matching handler blocks ambient fallback`:
    /// provider has only api-key auth, but an oauth credential is stored.
    #[tokio::test]
    async fn stored_credential_without_matching_handler_blocks_ambient() {
        let store = InMemoryCredentialStore::new();
        store.modify::<_, _, std::convert::Infallible>("p1", |_| async {
            Ok(Some(Credential::OAuth(OAuthCredential { access: "a".into(), refresh: Some("r".into()), expires: 0, account_id: None })))
        }).await.unwrap();
        let provider = ProviderAuth { api_key: Some(Box::new(EnvKeyAuth)), oauth: None };
        let ctx = EnvAuthContext::new();
        let r = resolve_provider_auth("p1", &provider, &model(), &store, &ctx, None).await.unwrap();
        assert!(r.is_none(), "an unhandled stored credential must block ambient fallback");
    }

    struct FailingRefreshOAuth;
    #[async_trait::async_trait]
    impl OAuthAuth for FailingRefreshOAuth {
        async fn refresh(&self, _c: &OAuthCredential) -> Result<OAuthCredential, ModelsError> {
            Err(ModelsError::new(ModelsErrorCode::OAuth, "invalid_grant"))
        }
        async fn to_auth(&self, c: &OAuthCredential) -> Result<crate::auth::ModelAuth, ModelsError> {
            Ok(crate::auth::ModelAuth { api_key: Some(c.access.clone()), ..Default::default() })
        }
    }

    #[tokio::test]
    async fn rejects_with_code_oauth_when_refresh_fails_preserving_the_credential() {
        let store = InMemoryCredentialStore::new();
        store.modify::<_, _, std::convert::Infallible>("p1", |_| async {
            Ok(Some(Credential::OAuth(OAuthCredential { access: "old".into(), refresh: Some("r".into()), expires: 0, account_id: None })))
        }).await.unwrap();
        let provider = ProviderAuth { api_key: None, oauth: Some(Box::new(FailingRefreshOAuth)) };
        let ctx = EnvAuthContext::new();
        let err = resolve_provider_auth("p1", &provider, &model(), &store, &ctx, None).await.unwrap_err();
        assert_eq!(err.code, ModelsErrorCode::OAuth);
        // Stored credential preserved for retry / re-login.
        match store.read("p1") {
            Some(Credential::OAuth(o)) => assert_eq!(o.access, "old"),
            other => panic!("unexpected: {other:?}"),
        }
    }

    struct CountingRefreshOAuth { refreshes: Arc<AtomicUsize> }
    #[async_trait::async_trait]
    impl OAuthAuth for CountingRefreshOAuth {
        async fn refresh(&self, _c: &OAuthCredential) -> Result<OAuthCredential, ModelsError> {
            let n = self.refreshes.fetch_add(1, Ordering::SeqCst) + 1;
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            Ok(OAuthCredential { access: format!("new-{n}"), refresh: Some("r2".into()), expires: now_millis() + 60_000, account_id: None })
        }
        async fn to_auth(&self, c: &OAuthCredential) -> Result<crate::auth::ModelAuth, ModelsError> {
            Ok(crate::auth::ModelAuth { api_key: Some(c.access.clone()), ..Default::default() })
        }
    }

    #[tokio::test]
    async fn serializes_concurrent_oauth_refreshes_no_double_refresh() {
        let store = Arc::new(InMemoryCredentialStore::new());
        store.modify::<_, _, std::convert::Infallible>("p1", |_| async {
            Ok(Some(Credential::OAuth(OAuthCredential { access: "old".into(), refresh: Some("r1".into()), expires: 0, account_id: None })))
        }).await.unwrap();
        let refreshes = Arc::new(AtomicUsize::new(0));

        let resolve = |store: Arc<InMemoryCredentialStore>, refreshes: Arc<AtomicUsize>| async move {
            let provider = ProviderAuth { api_key: None, oauth: Some(Box::new(CountingRefreshOAuth { refreshes })) };
            let ctx = EnvAuthContext::new();
            resolve_provider_auth("p1", &provider, &model(), &store, &ctx, None).await.unwrap().unwrap()
        };
        let (a, b) = tokio::join!(
            resolve(store.clone(), refreshes.clone()),
            resolve(store.clone(), refreshes.clone()),
        );
        assert_eq!(refreshes.load(Ordering::SeqCst), 1, "the per-provider lock prevents a double refresh");
        assert_eq!(a.auth.api_key.as_deref(), Some("new-1"));
        assert_eq!(b.auth.api_key.as_deref(), Some("new-1"));
    }

    struct FailingApiKey;
    #[async_trait::async_trait]
    impl ApiKeyAuth for FailingApiKey {
        async fn resolve(&self, _m: &Model, _c: &dyn AuthContext, _cred: Option<&ApiKeyCredential>)
            -> Result<Option<AuthResult>, ModelsError> {
            Err(ModelsError::new(ModelsErrorCode::Provider, "nope"))
        }
    }

    #[tokio::test]
    async fn wraps_api_key_auth_failures_in_models_error_auth() {
        let store = InMemoryCredentialStore::new();
        let provider = ProviderAuth { api_key: Some(Box::new(FailingApiKey)), oauth: None };
        let ctx = EnvAuthContext::new();
        let err = resolve_provider_auth("p1", &provider, &model(), &store, &ctx, None).await.unwrap_err();
        assert_eq!(err.code, ModelsErrorCode::Auth);
    }

    // --- merge resolved auth into request (explicit wins per field) ---

    use crate::auth::{merge_auth_into_request, ModelAuth};
    use std::collections::HashMap;

    #[test]
    fn merges_resolved_auth_into_options_explicit_wins_per_field() {
        let auth = AuthResult {
            auth: ModelAuth {
                api_key: Some("resolved-key".into()),
                headers: Some(HashMap::from([("x-a".to_string(), "auth".to_string()), ("x-b".to_string(), "auth".to_string())])),
                base_url: Some("https://auth.test/v1".into()),
            },
            env: None, source: None,
        };
        // With explicit options: explicit apiKey + header x-b win; resolved baseUrl applies.
        let opts = crate::types::StreamOptions {
            api_key: Some("explicit-key".into()),
            headers: Some(HashMap::from([("x-b".to_string(), "explicit".to_string())])),
            ..Default::default()
        };
        let (m, o) = merge_auth_into_request(&auth, model(), opts);
        assert_eq!(o.api_key.as_deref(), Some("explicit-key"));
        let h = o.headers.unwrap();
        assert_eq!(h.get("x-a").map(String::as_str), Some("auth"));
        assert_eq!(h.get("x-b").map(String::as_str), Some("explicit"));
        assert_eq!(m.base_url, "https://auth.test/v1");

        // Without explicit options: resolved auth applies.
        let (m2, o2) = merge_auth_into_request(&auth, model(), crate::types::StreamOptions::default());
        assert_eq!(o2.api_key.as_deref(), Some("resolved-key"));
        assert_eq!(m2.base_url, "https://auth.test/v1");
        assert_eq!(o2.headers.unwrap().get("x-a").map(String::as_str), Some("auth"));
    }
}
