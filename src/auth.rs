//! Provider auth/credential seam.
//!
//! Idiomatic Rust port of upstream `auth/types.ts` + `auth/credential-store.ts`
//! (`@earendil-works/pi-ai` v0.80.2). Provides the type-tagged `Credential`
//! model, the `ModelAuth`/`AuthResult` request-auth shapes, the `ModelsError`
//! taxonomy, and an in-memory `CredentialStore` with per-provider serialized
//! read-modify-write (the seam `resolveProviderAuth` and the OAuth providers
//! build on). This is the abstraction that, once complete, lets the OAuth
//! provider branches resolve a stored-or-ambient credential through one path
//! instead of ad-hoc per-provider env lookups.

use std::collections::HashMap;
use std::future::Future;
use std::sync::{Arc, Mutex};

/// Provider-scoped environment/config values (e.g. Cloudflare account/gateway ids).
pub type ProviderEnv = HashMap<String, String>;

/// Request auth for a single model request. Anything not expressible as
/// `api_key`/`headers`/`base_url` is provider config, not auth.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ModelAuth {
    pub api_key: Option<String>,
    pub headers: Option<HashMap<String, String>>,
    pub base_url: Option<String>,
}

/// Stored api-key credential. `env` holds provider-scoped config.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ApiKeyCredential {
    pub key: Option<String>,
    pub env: Option<ProviderEnv>,
}

/// Stored OAuth credential (`access`/`refresh`/`expires` + optional account id).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OAuthCredential {
    pub access: String,
    pub refresh: Option<String>,
    /// Absolute expiry, epoch milliseconds.
    pub expires: i64,
    pub account_id: Option<String>,
}

/// One type-tagged credential per provider (mirrors today's auth.json shape).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Credential {
    ApiKey(ApiKeyCredential),
    OAuth(OAuthCredential),
}

impl Credential {
    pub fn is_oauth(&self) -> bool {
        matches!(self, Credential::OAuth(_))
    }
    pub fn is_api_key(&self) -> bool {
        matches!(self, Credential::ApiKey(_))
    }
}

/// Result of resolving auth for a model.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AuthResult {
    pub auth: ModelAuth,
    pub env: Option<ProviderEnv>,
    /// Human-readable label for status UI ("ANTHROPIC_API_KEY", "OAuth", ...).
    pub source: Option<String>,
}

/// Error taxonomy shared by the models/auth layer (mirrors ModelsErrorCode).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModelsErrorCode {
    ModelSource,
    ModelValidation,
    Provider,
    Stream,
    Auth,
    OAuth,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelsError {
    pub code: ModelsErrorCode,
    pub message: String,
}

impl ModelsError {
    pub fn new(code: ModelsErrorCode, message: impl Into<String>) -> Self {
        Self { code, message: message.into() }
    }
}

impl std::fmt::Display for ModelsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for ModelsError {}

/// Default in-memory credential store. Apps inject persistent stores. Keyed by
/// `provider.id`, one credential per provider; writes are serialized per
/// provider so a read-modify-write (OAuth refresh, login-during-refresh) sees a
/// consistent current value and cannot double-refresh a rotated token.
#[derive(Default)]
pub struct InMemoryCredentialStore {
    credentials: Mutex<HashMap<String, Credential>>,
    /// Per-provider async locks providing the serialized write path.
    locks: Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>,
}

impl InMemoryCredentialStore {
    pub fn new() -> Self {
        Self::default()
    }

    fn provider_lock(&self, provider_id: &str) -> Arc<tokio::sync::Mutex<()>> {
        let mut locks = self.locks.lock().unwrap();
        locks.entry(provider_id.to_string()).or_default().clone()
    }

    /// Read the stored credential (possibly expired). `None` for missing entries.
    pub fn read(&self, provider_id: &str) -> Option<Credential> {
        self.credentials.lock().unwrap().get(provider_id).cloned()
    }

    /// Serialized write — the only write path. `f` sees the current credential;
    /// it returns the new credential, or `None` to leave the entry unchanged.
    /// Resolves with the post-write credential (the new one, or the prior one if
    /// `f` returned `None`). Mutual exclusion is per provider id.
    pub async fn modify<F, Fut, E>(&self, provider_id: &str, f: F) -> Result<Option<Credential>, E>
    where
        F: FnOnce(Option<Credential>) -> Fut,
        Fut: Future<Output = Result<Option<Credential>, E>>,
    {
        let lock = self.provider_lock(provider_id);
        let _guard = lock.lock().await;
        let current = self.read(provider_id);
        let next = f(current.clone()).await?;
        match next {
            Some(cred) => {
                self.credentials.lock().unwrap().insert(provider_id.to_string(), cred.clone());
                Ok(Some(cred))
            }
            None => Ok(current),
        }
    }

    /// Remove a credential (logout). Serialized against `modify`.
    pub async fn delete(&self, provider_id: &str) {
        let lock = self.provider_lock(provider_id);
        let _guard = lock.lock().await;
        self.credentials.lock().unwrap().remove(provider_id);
    }
}

/// Environment access for auth resolution (injectable for tests). Mirrors
/// upstream `AuthContext` (the `fileExists` arm is omitted until an ambient
/// file-credential provider needs it — YAGNI).
#[async_trait::async_trait]
pub trait AuthContext: Send + Sync {
    async fn env(&self, name: &str) -> Option<String>;
}

/// Process-environment auth context, optionally overlaid with request-scoped
/// provider env values (mirrors overlayEnvAuthContext: overlay wins).
pub struct EnvAuthContext {
    overlay: ProviderEnv,
}

impl EnvAuthContext {
    pub fn new() -> Self {
        Self { overlay: ProviderEnv::new() }
    }
    pub fn with_overlay(overlay: ProviderEnv) -> Self {
        Self { overlay }
    }
}

impl Default for EnvAuthContext {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl AuthContext for EnvAuthContext {
    async fn env(&self, name: &str) -> Option<String> {
        if let Some(v) = self.overlay.get(name).filter(|v| !v.is_empty()) {
            return Some(v.clone());
        }
        std::env::var(name).ok().filter(|v| !v.is_empty())
    }
}

/// Api-key auth: resolves request auth from a stored credential and/or ambient
/// sources. Mirrors upstream `ApiKeyAuth.resolve` (login is app-owned, omitted).
#[async_trait::async_trait]
pub trait ApiKeyAuth: Send + Sync {
    async fn resolve(
        &self,
        model: &crate::types::Model,
        ctx: &dyn AuthContext,
        credential: Option<&ApiKeyCredential>,
    ) -> Result<Option<AuthResult>, ModelsError>;
}

/// OAuth auth. The `refresh`/`to_auth` split lets the resolver own the locked
/// refresh pattern (mirrors upstream `OAuthAuth`).
#[async_trait::async_trait]
pub trait OAuthAuth: Send + Sync {
    /// Exchange the refresh token. Network call; errors on failure.
    async fn refresh(&self, credential: &OAuthCredential) -> Result<OAuthCredential, ModelsError>;
    /// Side-effect-free derivation of request auth from a valid credential.
    async fn to_auth(&self, credential: &OAuthCredential) -> Result<ModelAuth, ModelsError>;
}

/// Provider auth descriptor. At least one of `api_key`/`oauth` is present.
#[derive(Default)]
pub struct ProviderAuth {
    pub api_key: Option<Box<dyn ApiKeyAuth>>,
    pub oauth: Option<Box<dyn OAuthAuth>>,
}

/// Request-scoped overrides (mirrors AuthResolutionOverrides).
#[derive(Clone, Debug, Default)]
pub struct AuthResolutionOverrides {
    pub api_key: Option<String>,
    pub env: Option<ProviderEnv>,
}

/// Resolve auth for a model (idiomatic port of upstream `resolveProviderAuth`).
///
/// A stored credential owns the provider: ambient/env is consulted only when
/// nothing is stored. No silent env fallback after a failed refresh or for a
/// credential type without a matching handler.
pub async fn resolve_provider_auth(
    provider_id: &str,
    auth: &ProviderAuth,
    model: &crate::types::Model,
    credentials: &InMemoryCredentialStore,
    base_ctx: &dyn AuthContext,
    overrides: Option<&AuthResolutionOverrides>,
) -> Result<Option<AuthResult>, ModelsError> {
    // An env overlay (if any) wins over the ambient context for this request.
    let overlay_ctx = overrides
        .and_then(|o| o.env.clone())
        .map(EnvAuthContext::with_overlay);
    let request_ctx: &dyn AuthContext = match &overlay_ctx {
        Some(c) => c,
        None => base_ctx,
    };

    // Explicit api-key override short-circuits.
    if let Some(ov) = overrides
        && let Some(key) = ov.api_key.clone()
        && let Some(api_key) = auth.api_key.as_ref()
    {
        let cred = ApiKeyCredential { key: Some(key), env: ov.env.clone() };
        return resolve_api_key(request_ctx, api_key.as_ref(), model, Some(&cred)).await;
    }

    // Stored credential owns the provider.
    if let Some(stored) = credentials.read(provider_id) {
        match stored {
            Credential::OAuth(o) => {
                if let Some(oauth) = auth.oauth.as_ref() {
                    return resolve_stored_oauth(credentials, provider_id, oauth.as_ref(), o).await;
                }
                return Ok(None);
            }
            Credential::ApiKey(mut a) => {
                if let Some(api_key) = auth.api_key.as_ref() {
                    if let Some(env) = overrides.and_then(|o| o.env.clone()) {
                        // Overlay env over the stored credential env.
                        let mut merged = a.env.take().unwrap_or_default();
                        merged.extend(env);
                        a.env = Some(merged);
                    }
                    return resolve_api_key(request_ctx, api_key.as_ref(), model, Some(&a)).await;
                }
                return Ok(None);
            }
        }
    }

    // Ambient (env vars, etc.).
    match auth.api_key.as_ref() {
        Some(api_key) => resolve_api_key(request_ctx, api_key.as_ref(), model, None).await,
        None => Ok(None),
    }
}

async fn resolve_api_key(
    ctx: &dyn AuthContext,
    api_key: &dyn ApiKeyAuth,
    model: &crate::types::Model,
    credential: Option<&ApiKeyCredential>,
) -> Result<Option<AuthResult>, ModelsError> {
    api_key.resolve(model, ctx, credential).await.map_err(|e| {
        ModelsError::new(ModelsErrorCode::Auth, format!("API key auth failed for provider {}: {}", model.provider, e.message))
    })
}

/// OAuth resolution with double-checked locking: valid tokens cost zero locks;
/// expired tokens lock, re-check expiry under the lock, refresh once globally,
/// and persist the rotated credential before release.
async fn resolve_stored_oauth(
    credentials: &InMemoryCredentialStore,
    provider_id: &str,
    oauth: &dyn OAuthAuth,
    stored: OAuthCredential,
) -> Result<Option<AuthResult>, ModelsError> {
    let mut credential = stored;
    if now_millis() >= credential.expires {
        let post = credentials
            .modify(provider_id, |current| async move {
                match current {
                    // Logged out meanwhile.
                    Some(Credential::OAuth(cur)) => {
                        // Another request/process refreshed under the lock.
                        if now_millis() < cur.expires {
                            return Ok(None);
                        }
                        let refreshed = oauth.refresh(&cur).await.map_err(|e| {
                            ModelsError::new(ModelsErrorCode::OAuth, format!("OAuth refresh failed for {provider_id}: {}", e.message))
                        })?;
                        Ok(Some(Credential::OAuth(refreshed)))
                    }
                    _ => Ok(None),
                }
            })
            .await?;
        match post {
            Some(Credential::OAuth(c)) => credential = c,
            // Logged out, or the lock re-check returned None (already-valid). Re-read.
            _ => match credentials.read(provider_id) {
                Some(Credential::OAuth(c)) => credential = c,
                _ => return Ok(None),
            },
        }
    }
    let auth = oauth.to_auth(&credential).await.map_err(|e| {
        ModelsError::new(ModelsErrorCode::OAuth, format!("OAuth auth derivation failed for {provider_id}: {}", e.message))
    })?;
    Ok(Some(AuthResult { auth, env: None, source: Some("OAuth".to_string()) }))
}

fn now_millis() -> i64 {
    crate::utils::now_millis()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn models_error_displays_message_and_keeps_code() {
        let e = ModelsError::new(ModelsErrorCode::Auth, "API key auth failed for provider openai");
        assert_eq!(e.to_string(), "API key auth failed for provider openai");
        assert_eq!(e.code, ModelsErrorCode::Auth);
    }

    #[tokio::test]
    async fn read_returns_none_for_missing_entry() {
        let store = InMemoryCredentialStore::new();
        assert_eq!(store.read("openai"), None);
    }

    #[tokio::test]
    async fn modify_writes_and_read_returns_stored() {
        let store = InMemoryCredentialStore::new();
        let cred = Credential::ApiKey(ApiKeyCredential { key: Some("sk-1".into()), env: None });
        let written = store
            .modify::<_, _, std::convert::Infallible>("openai", |cur| {
                assert!(cur.is_none());
                let c = cred.clone();
                async move { Ok(Some(c)) }
            })
            .await
            .unwrap();
        assert_eq!(written, Some(cred.clone()));
        assert_eq!(store.read("openai"), Some(cred));
    }

    #[tokio::test]
    async fn modify_returning_none_leaves_entry_unchanged() {
        let store = InMemoryCredentialStore::new();
        let cred = Credential::ApiKey(ApiKeyCredential { key: Some("sk-1".into()), env: None });
        store
            .modify::<_, _, std::convert::Infallible>("openai", |_| {
                let c = cred.clone();
                async move { Ok(Some(c)) }
            })
            .await
            .unwrap();
        // A no-op modify (fn returns None) preserves the prior credential and
        // resolves with it.
        let result = store
            .modify::<_, _, std::convert::Infallible>("openai", |cur| {
                assert!(cur.is_some());
                async move { Ok(None) }
            })
            .await
            .unwrap();
        assert_eq!(result, Some(cred.clone()));
        assert_eq!(store.read("openai"), Some(cred));
    }

    #[tokio::test]
    async fn delete_removes_credential() {
        let store = InMemoryCredentialStore::new();
        store
            .modify::<_, _, std::convert::Infallible>("openai", |_| async {
                Ok(Some(Credential::ApiKey(ApiKeyCredential { key: Some("k".into()), env: None })))
            })
            .await
            .unwrap();
        store.delete("openai").await;
        assert_eq!(store.read("openai"), None);
    }

    #[tokio::test]
    async fn modify_serializes_concurrent_writes_per_provider() {
        // Two concurrent increments on the same provider must not lose an update:
        // the per-provider lock serializes the read-modify-write.
        let store = Arc::new(InMemoryCredentialStore::new());
        store
            .modify::<_, _, std::convert::Infallible>("p", |_| async {
                Ok(Some(Credential::OAuth(OAuthCredential {
                    access: "0".into(), refresh: None, expires: 0, account_id: None,
                })))
            })
            .await
            .unwrap();

        let bump = |store: Arc<InMemoryCredentialStore>| async move {
            store
                .modify::<_, _, std::convert::Infallible>("p", |cur| async move {
                    let n: i64 = match cur {
                        Some(Credential::OAuth(o)) => o.access.parse().unwrap_or(0),
                        _ => 0,
                    };
                    Ok(Some(Credential::OAuth(OAuthCredential {
                        access: (n + 1).to_string(), refresh: None, expires: 0, account_id: None,
                    })))
                })
                .await
                .unwrap();
        };
        let (a, b) = tokio::join!(bump(store.clone()), bump(store.clone()));
        let _ = (a, b);
        match store.read("p") {
            Some(Credential::OAuth(o)) => assert_eq!(o.access, "2", "both increments must apply"),
            other => panic!("unexpected: {other:?}"),
        }
    }

    // --- resolve_provider_auth ---

    use crate::types::{Model, ModelCost};
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn test_model(provider: &str) -> Model {
        Model {
            id: "m".into(), name: "M".into(), api: "openai-completions".into(),
            provider: provider.into(), base_url: "http://x".into(), reasoning: false,
            thinking_level_map: None, input: vec!["text".into()], cost: ModelCost::default(),
            context_window: 1000, max_tokens: 100, headers: None, api_key: None, compat: Default::default(),
        }
    }

    /// Resolves `credential.key ?? env(ENV_NAME)` like a typical provider.
    struct KeyOrEnv { env_name: &'static str }
    #[async_trait::async_trait]
    impl ApiKeyAuth for KeyOrEnv {
        async fn resolve(&self, _m: &Model, ctx: &dyn AuthContext, credential: Option<&ApiKeyCredential>)
            -> Result<Option<AuthResult>, ModelsError> {
            let key = match credential.and_then(|c| c.key.clone()) {
                Some(k) => Some(k),
                None => ctx.env(self.env_name).await,
            };
            Ok(key.map(|k| AuthResult {
                auth: ModelAuth { api_key: Some(k), ..Default::default() },
                env: None, source: Some(self.env_name.to_string()),
            }))
        }
    }

    fn api_key_provider() -> ProviderAuth {
        ProviderAuth { api_key: Some(Box::new(KeyOrEnv { env_name: "TEST_PROVIDER_KEY_XYZ" })), oauth: None }
    }

    #[tokio::test]
    async fn resolve_uses_api_key_override_first() {
        let store = InMemoryCredentialStore::new();
        let ctx = EnvAuthContext::new();
        let overrides = AuthResolutionOverrides { api_key: Some("ov-key".into()), env: None };
        let r = resolve_provider_auth("openai", &api_key_provider(), &test_model("openai"), &store, &ctx, Some(&overrides))
            .await.unwrap().unwrap();
        assert_eq!(r.auth.api_key.as_deref(), Some("ov-key"));
    }

    #[tokio::test]
    async fn resolve_uses_stored_api_key_credential() {
        let store = InMemoryCredentialStore::new();
        store.modify::<_, _, std::convert::Infallible>("openai", |_| async {
            Ok(Some(Credential::ApiKey(ApiKeyCredential { key: Some("stored-key".into()), env: None })))
        }).await.unwrap();
        let ctx = EnvAuthContext::new();
        let r = resolve_provider_auth("openai", &api_key_provider(), &test_model("openai"), &store, &ctx, None)
            .await.unwrap().unwrap();
        assert_eq!(r.auth.api_key.as_deref(), Some("stored-key"));
    }

    #[tokio::test]
    async fn resolve_falls_back_to_ambient_env_when_nothing_stored() {
        let store = InMemoryCredentialStore::new();
        let mut overlay = ProviderEnv::new();
        overlay.insert("TEST_PROVIDER_KEY_XYZ".into(), "ambient-key".into());
        let ctx = EnvAuthContext::with_overlay(overlay);
        let r = resolve_provider_auth("openai", &api_key_provider(), &test_model("openai"), &store, &ctx, None)
            .await.unwrap().unwrap();
        assert_eq!(r.auth.api_key.as_deref(), Some("ambient-key"));
        assert_eq!(r.source.as_deref(), Some("TEST_PROVIDER_KEY_XYZ"));
    }

    #[tokio::test]
    async fn resolve_returns_none_when_unconfigured() {
        let store = InMemoryCredentialStore::new();
        let ctx = EnvAuthContext::new(); // empty overlay; env var unset
        let r = resolve_provider_auth("openai", &api_key_provider(), &test_model("openai"), &store, &ctx, None)
            .await.unwrap();
        assert!(r.is_none());
    }

    struct CountingOAuth { refreshes: Arc<AtomicUsize> }
    #[async_trait::async_trait]
    impl OAuthAuth for CountingOAuth {
        async fn refresh(&self, _c: &OAuthCredential) -> Result<OAuthCredential, ModelsError> {
            self.refreshes.fetch_add(1, Ordering::SeqCst);
            Ok(OAuthCredential { access: "fresh".into(), refresh: Some("r2".into()), expires: now_millis() + 60_000, account_id: None })
        }
        async fn to_auth(&self, c: &OAuthCredential) -> Result<ModelAuth, ModelsError> {
            Ok(ModelAuth { api_key: Some(c.access.clone()), ..Default::default() })
        }
    }

    #[tokio::test]
    async fn resolve_oauth_valid_token_skips_refresh() {
        let store = InMemoryCredentialStore::new();
        store.modify::<_, _, std::convert::Infallible>("anthropic", |_| async {
            Ok(Some(Credential::OAuth(OAuthCredential { access: "valid".into(), refresh: Some("r".into()), expires: now_millis() + 60_000, account_id: None })))
        }).await.unwrap();
        let refreshes = Arc::new(AtomicUsize::new(0));
        let provider = ProviderAuth { api_key: None, oauth: Some(Box::new(CountingOAuth { refreshes: refreshes.clone() })) };
        let ctx = EnvAuthContext::new();
        let r = resolve_provider_auth("anthropic", &provider, &test_model("anthropic"), &store, &ctx, None)
            .await.unwrap().unwrap();
        assert_eq!(r.auth.api_key.as_deref(), Some("valid"));
        assert_eq!(r.source.as_deref(), Some("OAuth"));
        assert_eq!(refreshes.load(Ordering::SeqCst), 0, "valid token must not refresh");
    }

    #[tokio::test]
    async fn resolve_oauth_expired_token_refreshes_once_and_persists() {
        let store = InMemoryCredentialStore::new();
        store.modify::<_, _, std::convert::Infallible>("anthropic", |_| async {
            Ok(Some(Credential::OAuth(OAuthCredential { access: "old".into(), refresh: Some("r".into()), expires: now_millis() - 1, account_id: None })))
        }).await.unwrap();
        let refreshes = Arc::new(AtomicUsize::new(0));
        let provider = ProviderAuth { api_key: None, oauth: Some(Box::new(CountingOAuth { refreshes: refreshes.clone() })) };
        let ctx = EnvAuthContext::new();
        let r = resolve_provider_auth("anthropic", &provider, &test_model("anthropic"), &store, &ctx, None)
            .await.unwrap().unwrap();
        assert_eq!(r.auth.api_key.as_deref(), Some("fresh"));
        assert_eq!(refreshes.load(Ordering::SeqCst), 1);
        // Rotated credential persisted.
        match store.read("anthropic") {
            Some(Credential::OAuth(o)) => assert_eq!(o.access, "fresh"),
            other => panic!("unexpected: {other:?}"),
        }
    }
}
