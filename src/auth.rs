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
use tokio::sync::watch;

/// Provider-scoped environment/config values (e.g. Cloudflare account/gateway ids).
pub type ProviderEnv = HashMap<String, String>;

/// Request auth for a single model request. Anything not expressible as
/// `api_key`/`headers`/`base_url` is provider config, not auth.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ModelAuth {
    pub api_key: Option<String>,
    pub headers: Option<HashMap<String, String>>,
    pub base_url: Option<String>,
    /// Provider-owned header overrides. `Some(value)` sets/replaces a header, while
    /// `None` deletes any matching header case-insensitively before provider request
    /// builders see it (mirrors upstream ProviderHeaders null deletion semantics).
    pub header_overrides: Option<ProviderHeaderOverrides>,
}

pub type ProviderHeaders = HashMap<String, String>;
pub type ProviderHeaderOverrides = HashMap<String, Option<String>>;

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
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn with_cause(
        code: ModelsErrorCode,
        message: impl Into<String>,
        cause: impl std::fmt::Display,
    ) -> Self {
        let message = message.into();
        let detail = cause.to_string();
        let message = if detail.trim().is_empty() || message.contains(&detail) {
            message
        } else {
            format!("{message}: {detail}")
        };
        Self { code, message }
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
#[derive(Clone, Default)]
pub struct InMemoryCredentialStore {
    credentials: Arc<Mutex<HashMap<String, Credential>>>,
    /// Per-provider async locks providing the serialized write path.
    locks: Arc<Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>>,
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
                self.credentials
                    .lock()
                    .unwrap()
                    .insert(provider_id.to_string(), cred.clone());
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
        Self {
            overlay: ProviderEnv::new(),
        }
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
    /// Cancellation-aware refresh seam. Implementations can override this to thread
    /// cancellation into provider network calls; the default checks before/after.
    async fn refresh_with_cancel(
        &self,
        credential: &OAuthCredential,
        cancel: Option<watch::Receiver<bool>>,
    ) -> Result<OAuthCredential, ModelsError> {
        if cancel.as_ref().is_some_and(|rx| *rx.borrow()) {
            return Err(cancelled_oauth_error());
        }
        let refreshed = self.refresh(credential).await?;
        if cancel.as_ref().is_some_and(|rx| *rx.borrow()) {
            return Err(cancelled_oauth_error());
        }
        Ok(refreshed)
    }
    /// Side-effect-free derivation of request auth from a valid credential.
    async fn to_auth(&self, credential: &OAuthCredential) -> Result<ModelAuth, ModelsError>;
}

/// Provider auth descriptor. At least one of `api_key`/`oauth` is present.
#[derive(Default)]
pub struct ProviderAuth {
    pub api_key: Option<Box<dyn ApiKeyAuth>>,
    /// Shared ownership lets admitted token rotations settle after caller cancellation.
    pub oauth: Option<Arc<dyn OAuthAuth>>,
}

/// Request-scoped overrides (mirrors AuthResolutionOverrides).
#[derive(Clone, Debug, Default)]
pub struct AuthResolutionOverrides {
    pub api_key: Option<String>,
    pub env: Option<ProviderEnv>,
    pub min_oauth_validity_ms: Option<i64>,
    pub cancel: Option<watch::Receiver<bool>>,
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
    let cancel = overrides.and_then(|o| o.cancel.clone());
    if cancel.as_ref().is_some_and(|rx| *rx.borrow()) {
        return Err(cancelled_auth_error());
    }
    tokio::select! {
        biased;
        _ = wait_auth_cancel(cancel) => Err(cancelled_auth_error()),
        result = resolve_provider_auth_inner(provider_id, auth, model, credentials, base_ctx, overrides) => result,
    }
}

async fn wait_auth_cancel(cancel: Option<watch::Receiver<bool>>) {
    let Some(mut cancel) = cancel else {
        return std::future::pending().await;
    };
    loop {
        if *cancel.borrow_and_update() {
            return;
        }
        if cancel.changed().await.is_err() {
            // Dropping the sender does not cancel an operation.
            return std::future::pending().await;
        }
    }
}

async fn resolve_provider_auth_inner(
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
        let cred = ApiKeyCredential {
            key: Some(key),
            env: ov.env.clone(),
        };
        return resolve_api_key(request_ctx, api_key.as_ref(), model, Some(&cred)).await;
    }

    // Stored credential owns the provider.
    if let Some(stored) = credentials.read(provider_id) {
        match stored {
            Credential::OAuth(o) => {
                if let Some(oauth) = auth.oauth.as_ref() {
                    return resolve_stored_oauth(
                        credentials,
                        provider_id,
                        oauth.clone(),
                        o,
                        overrides.and_then(|o| o.min_oauth_validity_ms),
                        overrides.and_then(|o| o.cancel.clone()),
                    )
                    .await;
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
        ModelsError::with_cause(
            ModelsErrorCode::Auth,
            format!("API key auth failed for provider {}", model.provider),
            e,
        )
    })
}

/// Refresh under the provider lock. Cancellation affects lock admission only;
/// admitted rotations persist before unlocking even if the caller drops its future.
/// Provider operations are bounded by an independent 15-second timeout.
pub async fn refresh_stored_oauth_credential<F>(
    credentials: &InMemoryCredentialStore,
    provider_id: &str,
    oauth: Arc<dyn OAuthAuth>,
    needs_refresh: F,
    cancel: Option<watch::Receiver<bool>>,
) -> Result<Option<OAuthCredential>, ModelsError>
where
    F: FnOnce(&OAuthCredential) -> bool + Send + 'static,
{
    let lock = credentials.provider_lock(provider_id);
    let guard = tokio::select! {
        biased;
        _ = wait_auth_cancel(cancel.clone()) => return Err(cancelled_auth_error()),
        guard = lock.lock_owned() => guard,
    };
    if cancel.as_ref().is_some_and(|rx| *rx.borrow()) {
        return Err(cancelled_auth_error());
    }
    let credentials = credentials.clone();
    let provider_id = provider_id.to_owned();
    tokio::spawn(async move {
        let _guard = guard;
        let Some(Credential::OAuth(current)) = credentials.read(&provider_id) else {
            return Ok(None);
        };
        if !needs_refresh(&current) {
            return Ok(Some(current));
        }
        // Never forward the caller signal after admission: a rotated refresh
        // token may already have invalidated the credential in storage.
        let (_timeout_owner, timeout_signal) = watch::channel(false);
        let refreshed = tokio::time::timeout(
            std::time::Duration::from_secs(15),
            oauth.refresh_with_cancel(&current, Some(timeout_signal)),
        )
        .await
        .map_err(|_| {
            ModelsError::new(
                ModelsErrorCode::OAuth,
                format!("OAuth refresh failed for {provider_id}: timed out after 15000 ms"),
            )
        })?
        .map_err(|error| {
            ModelsError::with_cause(
                ModelsErrorCode::OAuth,
                format!("OAuth refresh failed for {provider_id}"),
                error,
            )
        })?;
        credentials
            .credentials
            .lock()
            .unwrap()
            .insert(provider_id, Credential::OAuth(refreshed.clone()));
        Ok(Some(refreshed))
    })
    .await
    .map_err(|error| {
        ModelsError::with_cause(ModelsErrorCode::OAuth, "OAuth refresh worker failed", error)
    })?
}

/// OAuth resolution with double-checked locking: valid tokens cost zero locks;
/// expired tokens lock, re-check expiry under the lock, refresh once globally,
/// and persist the rotated credential before release.
async fn resolve_stored_oauth(
    credentials: &InMemoryCredentialStore,
    provider_id: &str,
    oauth: Arc<dyn OAuthAuth>,
    stored: OAuthCredential,
    min_validity_ms: Option<i64>,
    cancel: Option<watch::Receiver<bool>>,
) -> Result<Option<AuthResult>, ModelsError> {
    const DEFAULT_MIN_OAUTH_VALIDITY_MS: i64 = 5 * 60 * 1000;
    let explicit_validity = min_validity_ms.is_some();
    let min_validity_ms = min_validity_ms
        .unwrap_or(0)
        .max(DEFAULT_MIN_OAUTH_VALIDITY_MS);
    let mut credential = stored;
    if now_millis() + min_validity_ms >= credential.expires {
        let post = refresh_stored_oauth_credential(
            credentials,
            provider_id,
            oauth.clone(),
            move |current| now_millis() + min_validity_ms >= current.expires,
            cancel,
        )
        .await?;
        match post {
            Some(c) => {
                if explicit_validity && now_millis() + min_validity_ms >= c.expires {
                    return Err(ModelsError::new(
                        ModelsErrorCode::OAuth,
                        format!(
                            "OAuth refresh failed for {provider_id}: refreshed credential expires too soon"
                        ),
                    ));
                }
                credential = c
            }
            // Logout or replacement with another credential type under the lock.
            None => return Ok(None),
        }
    }
    let auth = oauth.to_auth(&credential).await.map_err(|e| {
        ModelsError::with_cause(
            ModelsErrorCode::OAuth,
            format!("OAuth auth derivation failed for {provider_id}"),
            e,
        )
    })?;
    Ok(Some(AuthResult {
        auth,
        env: None,
        source: Some("OAuth".to_string()),
    }))
}

fn now_millis() -> i64 {
    crate::utils::now_millis()
}

fn cancelled_oauth_error() -> ModelsError {
    ModelsError::new(ModelsErrorCode::OAuth, "AbortError: OAuth refresh aborted")
}

fn cancelled_auth_error() -> ModelsError {
    ModelsError::new(ModelsErrorCode::Auth, "AbortError: Auth resolution aborted")
}

pub fn merge_provider_headers(
    base: Option<&ProviderHeaders>,
    overrides: Option<&ProviderHeaderOverrides>,
) -> Option<ProviderHeaders> {
    if base.is_none() && overrides.is_none() {
        return None;
    }
    let mut merged = base.cloned().unwrap_or_default();
    if let Some(overrides) = overrides {
        for (name, value) in overrides {
            let lower = name.to_ascii_lowercase();
            merged.retain(|existing, _| existing.to_ascii_lowercase() != lower);
            if let Some(value) = value {
                merged.insert(name.clone(), value.clone());
            }
        }
    }
    Some(merged)
}

/// Merge a resolved `AuthResult` into the request: explicit options win per field,
/// the resolved `base_url` is applied to the model, and resolved headers are
/// overlaid under any explicit headers (mirrors the Models collection's auth
/// merge before dispatch). Returns the (model, options) the provider sees.
pub fn merge_auth_into_request(
    auth: &AuthResult,
    mut model: crate::types::Model,
    mut opts: crate::types::StreamOptions,
) -> (crate::types::Model, crate::types::StreamOptions) {
    // api key: explicit option wins, else resolved.
    if opts.api_key.is_none() {
        opts.api_key = auth.auth.api_key.clone();
    }
    // base url: resolved auth overrides the model base url when present.
    if let Some(base) = auth.auth.base_url.as_deref() {
        model.base_url = base.to_string();
    }
    // headers: resolved first, then ProviderHeaders null/deletion overrides, then
    // explicit request headers overlaid per key (explicit wins).
    let provider_headers = merge_provider_headers(
        auth.auth.headers.as_ref(),
        auth.auth.header_overrides.as_ref(),
    );
    if let Some(resolved_headers) = provider_headers {
        let mut merged = resolved_headers;
        if let Some(explicit) = opts.headers.as_ref() {
            for (k, v) in explicit {
                merged.retain(|existing, _| !existing.eq_ignore_ascii_case(k));
                merged.insert(k.clone(), v.clone());
            }
        }
        opts.headers = Some(merged);
    }
    (model, opts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn models_error_displays_message_and_keeps_code() {
        let e = ModelsError::new(
            ModelsErrorCode::Auth,
            "API key auth failed for provider openai",
        );
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
        let cred = Credential::ApiKey(ApiKeyCredential {
            key: Some("sk-1".into()),
            env: None,
        });
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
        let cred = Credential::ApiKey(ApiKeyCredential {
            key: Some("sk-1".into()),
            env: None,
        });
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
                Ok(Some(Credential::ApiKey(ApiKeyCredential {
                    key: Some("k".into()),
                    env: None,
                })))
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
                    access: "0".into(),
                    refresh: None,
                    expires: 0,
                    account_id: None,
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
                        access: (n + 1).to_string(),
                        refresh: None,
                        expires: 0,
                        account_id: None,
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
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    fn test_model(provider: &str) -> Model {
        Model {
            id: "m".into(),
            name: "M".into(),
            api: "openai-completions".into(),
            provider: provider.into(),
            base_url: "http://x".into(),
            reasoning: false,
            thinking_level_map: None,
            input: vec!["text".into()],
            input_limits: None,
            prompt_cache: None,
            enabled: None,
            lab: None,
            providers: None,
            cost: ModelCost::default(),
            context_window: 1000,
            max_tokens: 100,
            sampling_params: None,
            sampling_params_by_thinking_level: None,
            headers: None,
            api_key: None,
            compat: Default::default(),
        }
    }

    /// Resolves `credential.key ?? env(ENV_NAME)` like a typical provider.
    struct KeyOrEnv {
        env_name: &'static str,
    }
    #[async_trait::async_trait]
    impl ApiKeyAuth for KeyOrEnv {
        async fn resolve(
            &self,
            _m: &Model,
            ctx: &dyn AuthContext,
            credential: Option<&ApiKeyCredential>,
        ) -> Result<Option<AuthResult>, ModelsError> {
            let key = match credential.and_then(|c| c.key.clone()) {
                Some(k) => Some(k),
                None => ctx.env(self.env_name).await,
            };
            Ok(key.map(|k| AuthResult {
                auth: ModelAuth {
                    api_key: Some(k),
                    ..Default::default()
                },
                env: None,
                source: Some(self.env_name.to_string()),
            }))
        }
    }

    fn api_key_provider() -> ProviderAuth {
        ProviderAuth {
            api_key: Some(Box::new(KeyOrEnv {
                env_name: "TEST_PROVIDER_KEY_XYZ",
            })),
            oauth: None,
        }
    }

    #[tokio::test]
    async fn resolve_uses_api_key_override_first() {
        let store = InMemoryCredentialStore::new();
        let ctx = EnvAuthContext::new();
        let overrides = AuthResolutionOverrides {
            api_key: Some("ov-key".into()),
            env: None,
            min_oauth_validity_ms: None,
            cancel: None,
        };
        let r = resolve_provider_auth(
            "openai",
            &api_key_provider(),
            &test_model("openai"),
            &store,
            &ctx,
            Some(&overrides),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(r.auth.api_key.as_deref(), Some("ov-key"));
    }

    #[tokio::test]
    async fn resolve_uses_stored_api_key_credential() {
        let store = InMemoryCredentialStore::new();
        store
            .modify::<_, _, std::convert::Infallible>("openai", |_| async {
                Ok(Some(Credential::ApiKey(ApiKeyCredential {
                    key: Some("stored-key".into()),
                    env: None,
                })))
            })
            .await
            .unwrap();
        let ctx = EnvAuthContext::new();
        let r = resolve_provider_auth(
            "openai",
            &api_key_provider(),
            &test_model("openai"),
            &store,
            &ctx,
            None,
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(r.auth.api_key.as_deref(), Some("stored-key"));
    }

    #[tokio::test]
    async fn resolve_falls_back_to_ambient_env_when_nothing_stored() {
        let store = InMemoryCredentialStore::new();
        let mut overlay = ProviderEnv::new();
        overlay.insert("TEST_PROVIDER_KEY_XYZ".into(), "ambient-key".into());
        let ctx = EnvAuthContext::with_overlay(overlay);
        let r = resolve_provider_auth(
            "openai",
            &api_key_provider(),
            &test_model("openai"),
            &store,
            &ctx,
            None,
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(r.auth.api_key.as_deref(), Some("ambient-key"));
        assert_eq!(r.source.as_deref(), Some("TEST_PROVIDER_KEY_XYZ"));
    }

    #[tokio::test]
    async fn resolve_returns_none_when_unconfigured() {
        let store = InMemoryCredentialStore::new();
        let ctx = EnvAuthContext::new(); // empty overlay; env var unset
        let r = resolve_provider_auth(
            "openai",
            &api_key_provider(),
            &test_model("openai"),
            &store,
            &ctx,
            None,
        )
        .await
        .unwrap();
        assert!(r.is_none());
    }

    struct CountingOAuth {
        refreshes: Arc<AtomicUsize>,
    }
    #[async_trait::async_trait]
    impl OAuthAuth for CountingOAuth {
        async fn refresh(&self, _c: &OAuthCredential) -> Result<OAuthCredential, ModelsError> {
            self.refreshes.fetch_add(1, Ordering::SeqCst);
            Ok(OAuthCredential {
                access: "fresh".into(),
                refresh: Some("r2".into()),
                expires: now_millis() + 600_000,
                account_id: None,
            })
        }
        async fn to_auth(&self, c: &OAuthCredential) -> Result<ModelAuth, ModelsError> {
            Ok(ModelAuth {
                api_key: Some(c.access.clone()),
                ..Default::default()
            })
        }
    }

    struct CancelAwareOAuth {
        saw_signal: Arc<AtomicBool>,
    }
    #[async_trait::async_trait]
    impl OAuthAuth for CancelAwareOAuth {
        async fn refresh(&self, _c: &OAuthCredential) -> Result<OAuthCredential, ModelsError> {
            panic!("refresh_with_cancel should be used")
        }
        async fn refresh_with_cancel(
            &self,
            _credential: &OAuthCredential,
            cancel: Option<watch::Receiver<bool>>,
        ) -> Result<OAuthCredential, ModelsError> {
            self.saw_signal.store(cancel.is_some(), Ordering::SeqCst);
            let cancelled = cancel.as_ref().is_some_and(|rx| *rx.borrow());
            if cancelled {
                return Err(ModelsError::new(
                    ModelsErrorCode::OAuth,
                    "cancelled refresh",
                ));
            }
            Ok(OAuthCredential {
                access: "fresh-cancel-aware".into(),
                refresh: Some("r2".into()),
                expires: now_millis() + 600_000,
                account_id: None,
            })
        }
        async fn to_auth(&self, c: &OAuthCredential) -> Result<ModelAuth, ModelsError> {
            Ok(ModelAuth {
                api_key: Some(c.access.clone()),
                ..Default::default()
            })
        }
    }

    struct DeltaOAuth {
        refreshes: Arc<AtomicUsize>,
        delta_ms: i64,
    }
    #[async_trait::async_trait]
    impl OAuthAuth for DeltaOAuth {
        async fn refresh(&self, _c: &OAuthCredential) -> Result<OAuthCredential, ModelsError> {
            self.refreshes.fetch_add(1, Ordering::SeqCst);
            Ok(OAuthCredential {
                access: format!("fresh-{}", self.delta_ms),
                refresh: Some("r2".into()),
                expires: now_millis() + self.delta_ms,
                account_id: None,
            })
        }
        async fn to_auth(&self, c: &OAuthCredential) -> Result<ModelAuth, ModelsError> {
            Ok(ModelAuth {
                api_key: Some(c.access.clone()),
                ..Default::default()
            })
        }
    }

    #[tokio::test]
    async fn resolve_oauth_refreshes_when_inside_min_validity_window_and_honors_override() {
        let store = InMemoryCredentialStore::new();
        store
            .modify::<_, _, std::convert::Infallible>("anthropic", |_| async {
                Ok(Some(Credential::OAuth(OAuthCredential {
                    access: "soon".into(),
                    refresh: Some("r".into()),
                    expires: now_millis() + 60_000,
                    account_id: None,
                })))
            })
            .await
            .unwrap();
        let refreshes = Arc::new(AtomicUsize::new(0));
        let provider = ProviderAuth {
            api_key: None,
            oauth: Some(Arc::new(CountingOAuth {
                refreshes: refreshes.clone(),
            })),
        };
        let ctx = EnvAuthContext::new();
        let r = resolve_provider_auth(
            "anthropic",
            &provider,
            &test_model("anthropic"),
            &store,
            &ctx,
            None,
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(r.auth.api_key.as_deref(), Some("fresh"));
        assert_eq!(refreshes.load(Ordering::SeqCst), 1);

        store
            .modify::<_, _, std::convert::Infallible>("anthropic", |_| async {
                Ok(Some(Credential::OAuth(OAuthCredential {
                    access: "soon2".into(),
                    refresh: Some("r".into()),
                    expires: now_millis() + 60_000,
                    account_id: None,
                })))
            })
            .await
            .unwrap();
        let overrides = AuthResolutionOverrides {
            api_key: None,
            env: None,
            min_oauth_validity_ms: Some(0),
            cancel: None,
        };
        let r = resolve_provider_auth(
            "anthropic",
            &provider,
            &test_model("anthropic"),
            &store,
            &ctx,
            Some(&overrides),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(r.auth.api_key.as_deref(), Some("fresh"));
        assert_eq!(
            refreshes.load(Ordering::SeqCst),
            2,
            "Some(0) is floored to the default 5 minute window"
        );
    }

    #[tokio::test]
    async fn resolve_oauth_post_refresh_must_satisfy_effective_min_validity() {
        let store = InMemoryCredentialStore::new();
        store
            .modify::<_, _, std::convert::Infallible>("anthropic", |_| async {
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
        let provider = ProviderAuth {
            api_key: None,
            oauth: Some(Arc::new(DeltaOAuth {
                refreshes: refreshes.clone(),
                delta_ms: 60_000,
            })),
        };
        let ctx = EnvAuthContext::new();
        let auth = resolve_provider_auth(
            "anthropic",
            &provider,
            &test_model("anthropic"),
            &store,
            &ctx,
            None,
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(auth.auth.api_key.as_deref(), Some("fresh-60000"));

        // Only an explicit minimum imposes a post-refresh validity contract.
        let overrides = AuthResolutionOverrides {
            min_oauth_validity_ms: Some(0),
            ..Default::default()
        };
        let err = resolve_provider_auth(
            "anthropic",
            &provider,
            &test_model("anthropic"),
            &store,
            &ctx,
            Some(&overrides),
        )
        .await
        .unwrap_err();
        assert!(err.to_string().contains("expires too soon"));

        store
            .modify::<_, _, std::convert::Infallible>("anthropic", |_| async {
                Ok(Some(Credential::OAuth(OAuthCredential {
                    access: "old".into(),
                    refresh: Some("r".into()),
                    expires: 0,
                    account_id: None,
                })))
            })
            .await
            .unwrap();
        let provider = ProviderAuth {
            api_key: None,
            oauth: Some(Arc::new(DeltaOAuth {
                refreshes: refreshes.clone(),
                delta_ms: 900_000,
            })),
        };
        let overrides = AuthResolutionOverrides {
            api_key: None,
            env: None,
            min_oauth_validity_ms: Some(600_000),
            cancel: None,
        };
        let ok = resolve_provider_auth(
            "anthropic",
            &provider,
            &test_model("anthropic"),
            &store,
            &ctx,
            Some(&overrides),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(ok.auth.api_key.as_deref(), Some("fresh-900000"));
    }

    #[tokio::test]
    async fn resolve_oauth_refresh_receives_cancellation_signal_and_persists_success() {
        let store = InMemoryCredentialStore::new();
        store
            .modify::<_, _, std::convert::Infallible>("anthropic", |_| async {
                Ok(Some(Credential::OAuth(OAuthCredential {
                    access: "old".into(),
                    refresh: Some("r".into()),
                    expires: 0,
                    account_id: None,
                })))
            })
            .await
            .unwrap();
        let saw_signal = Arc::new(AtomicBool::new(false));
        let provider = ProviderAuth {
            api_key: None,
            oauth: Some(Arc::new(CancelAwareOAuth {
                saw_signal: saw_signal.clone(),
            })),
        };
        let (_tx, rx) = watch::channel(false);
        let overrides = AuthResolutionOverrides {
            api_key: None,
            env: None,
            min_oauth_validity_ms: None,
            cancel: Some(rx),
        };
        let auth = resolve_provider_auth(
            "anthropic",
            &provider,
            &test_model("anthropic"),
            &store,
            &EnvAuthContext::new(),
            Some(&overrides),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(auth.auth.api_key.as_deref(), Some("fresh-cancel-aware"));
        assert!(saw_signal.load(Ordering::SeqCst));
        match store.read("anthropic") {
            Some(Credential::OAuth(o)) => assert_eq!(o.access, "fresh-cancel-aware"),
            other => panic!("unexpected: {other:?}"),
        }
    }

    struct GatedRotation {
        entered: Arc<tokio::sync::Semaphore>,
        release: Arc<tokio::sync::Semaphore>,
        refreshes: Arc<AtomicUsize>,
    }

    #[async_trait::async_trait]
    impl OAuthAuth for GatedRotation {
        async fn refresh(&self, current: &OAuthCredential) -> Result<OAuthCredential, ModelsError> {
            self.refreshes.fetch_add(1, Ordering::SeqCst);
            self.entered.add_permits(1);
            self.release.acquire().await.unwrap().forget();
            assert_eq!(current.refresh.as_deref(), Some("r"));
            Ok(OAuthCredential {
                access: "rotated".into(),
                refresh: Some("rotated-refresh".into()),
                expires: now_millis() + 600_000,
                account_id: None,
            })
        }
        async fn to_auth(&self, current: &OAuthCredential) -> Result<ModelAuth, ModelsError> {
            Ok(ModelAuth {
                api_key: Some(current.access.clone()),
                ..Default::default()
            })
        }
    }

    async fn rotation_fixture() -> (
        InMemoryCredentialStore,
        ProviderAuth,
        Arc<tokio::sync::Semaphore>,
        Arc<tokio::sync::Semaphore>,
        Arc<AtomicUsize>,
    ) {
        let store = InMemoryCredentialStore::new();
        store
            .modify::<_, _, std::convert::Infallible>("p", |_| async {
                Ok(Some(Credential::OAuth(OAuthCredential {
                    access: "old".into(),
                    refresh: Some("r".into()),
                    expires: 0,
                    account_id: None,
                })))
            })
            .await
            .unwrap();
        let entered = Arc::new(tokio::sync::Semaphore::new(0));
        let release = Arc::new(tokio::sync::Semaphore::new(0));
        let count = Arc::new(AtomicUsize::new(0));
        let provider = ProviderAuth {
            api_key: None,
            oauth: Some(Arc::new(GatedRotation {
                entered: entered.clone(),
                release: release.clone(),
                refreshes: count.clone(),
            })),
        };
        (store, provider, entered, release, count)
    }

    #[tokio::test]
    async fn cancelled_auth_returns_promptly_but_admitted_rotation_persists_once() {
        let (store, provider, entered, release, count) = rotation_fixture().await;
        let (tx, rx) = watch::channel(false);
        let overrides = AuthResolutionOverrides {
            cancel: Some(rx),
            ..Default::default()
        };
        let model = test_model("p");
        let ctx = EnvAuthContext::new();
        let mut first = Box::pin(resolve_provider_auth(
            "p",
            &provider,
            &model,
            &store,
            &ctx,
            Some(&overrides),
        ));
        tokio::select! {
            _ = entered.acquire() => {},
            result = &mut first => panic!("refresh returned before release: {result:?}"),
        }
        tx.send(true).unwrap();
        let err = tokio::time::timeout(std::time::Duration::from_secs(1), first)
            .await
            .unwrap()
            .unwrap_err();
        assert!(err.message.contains("aborted"));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        release.add_permits(1);
        // A concurrent resolver waits for persistence, then reuses the rotated token.
        let result = resolve_provider_auth("p", &provider, &model, &store, &ctx, None)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(result.auth.api_key.as_deref(), Some("rotated"));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        let Some(Credential::OAuth(stored)) = store.read("p") else {
            panic!("missing credential")
        };
        assert_eq!(stored.refresh.as_deref(), Some("rotated-refresh"));
    }

    #[tokio::test]
    async fn dropped_auth_future_does_not_discard_rotated_credentials() {
        let (store, provider, entered, release, count) = rotation_fixture().await;
        let model = test_model("p");
        let ctx = EnvAuthContext::new();
        let mut first = Box::pin(resolve_provider_auth(
            "p", &provider, &model, &store, &ctx, None,
        ));
        tokio::select! {
            _ = entered.acquire() => {},
            result = &mut first => panic!("refresh returned before release: {result:?}"),
        }
        drop(first);
        release.add_permits(1);
        let result = resolve_provider_auth("p", &provider, &model, &store, &ctx, None)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(result.auth.api_key.as_deref(), Some("rotated"));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn cancellation_while_waiting_for_lock_never_starts_refresh() {
        let (store, provider, _entered, _release, count) = rotation_fixture().await;
        let lock = store.provider_lock("p");
        let guard = lock.lock().await;
        let (tx, rx) = watch::channel(false);
        let overrides = AuthResolutionOverrides {
            cancel: Some(rx),
            ..Default::default()
        };
        let model = test_model("p");
        let ctx = EnvAuthContext::new();
        let mut pending = Box::pin(resolve_provider_auth(
            "p",
            &provider,
            &model,
            &store,
            &ctx,
            Some(&overrides),
        ));
        assert!(futures::poll!(&mut pending).is_pending());
        tx.send(true).unwrap();
        assert!(pending.await.unwrap_err().message.contains("aborted"));
        drop(guard);
        tokio::task::yield_now().await;
        assert_eq!(count.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn precancelled_oauth_never_calls_provider_even_with_valid_token() {
        let (store, provider, _entered, _release, count) = rotation_fixture().await;
        store
            .modify::<_, _, std::convert::Infallible>("p", |current| async {
                let Some(Credential::OAuth(mut current)) = current else {
                    panic!("missing token")
                };
                current.expires = now_millis() + 600_000;
                Ok(Some(Credential::OAuth(current)))
            })
            .await
            .unwrap();
        let (_tx, rx) = watch::channel(true);
        let overrides = AuthResolutionOverrides {
            cancel: Some(rx),
            ..Default::default()
        };
        assert!(
            resolve_provider_auth(
                "p",
                &provider,
                &test_model("p"),
                &store,
                &EnvAuthContext::new(),
                Some(&overrides)
            )
            .await
            .unwrap_err()
            .message
            .contains("aborted")
        );
        assert_eq!(count.load(Ordering::SeqCst), 0);
    }

    struct NeverReturnsOAuth;
    #[async_trait::async_trait]
    impl OAuthAuth for NeverReturnsOAuth {
        async fn refresh(
            &self,
            _current: &OAuthCredential,
        ) -> Result<OAuthCredential, ModelsError> {
            std::future::pending().await
        }
        async fn to_auth(&self, _current: &OAuthCredential) -> Result<ModelAuth, ModelsError> {
            panic!("timed-out refresh cannot derive auth")
        }
    }

    #[tokio::test]
    async fn admitted_refresh_has_independent_timeout_and_releases_lock() {
        let (store, _, _, _, _) = rotation_fixture().await;
        let error = tokio::time::timeout(
            std::time::Duration::from_secs(17),
            refresh_stored_oauth_credential(
                &store,
                "p",
                Arc::new(NeverReturnsOAuth),
                |_| true,
                None,
            ),
        )
        .await
        .unwrap()
        .unwrap_err();
        assert_eq!(error.code, ModelsErrorCode::OAuth);
        assert!(error.message.contains("15000 ms"));
        // Failure preserves the credential and releases the lock for logout.
        assert!(
            matches!(store.read("p"), Some(Credential::OAuth(current)) if current.access == "old")
        );
        tokio::time::timeout(std::time::Duration::from_secs(1), store.delete("p"))
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn dropped_cancel_sender_does_not_cancel_auth_resolution() {
        let (store, provider, _, release, count) = rotation_fixture().await;
        let (tx, rx) = watch::channel(false);
        drop(tx);
        release.add_permits(1);
        let overrides = AuthResolutionOverrides {
            cancel: Some(rx),
            ..Default::default()
        };
        let result = resolve_provider_auth(
            "p",
            &provider,
            &test_model("p"),
            &store,
            &EnvAuthContext::new(),
            Some(&overrides),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(result.auth.api_key.as_deref(), Some("rotated"));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn merge_provider_headers_supports_null_deletion_case_insensitively() {
        let base = ProviderHeaders::from([
            ("Authorization".to_string(), "Bearer provider".to_string()),
            ("x-api-key".to_string(), "provider-key".to_string()),
            ("X-Shared".to_string(), "provider".to_string()),
        ]);
        let overrides = ProviderHeaderOverrides::from([
            ("authorization".to_string(), None),
            ("X-API-Key".to_string(), None),
            ("x-shared".to_string(), Some("request".to_string())),
        ]);
        let merged = merge_provider_headers(Some(&base), Some(&overrides)).unwrap();
        assert!(
            !merged
                .keys()
                .any(|key| key.eq_ignore_ascii_case("authorization"))
        );
        assert!(
            !merged
                .keys()
                .any(|key| key.eq_ignore_ascii_case("x-api-key"))
        );
        assert_eq!(merged.get("x-shared").map(String::as_str), Some("request"));
    }

    #[tokio::test]
    async fn resolve_oauth_valid_token_skips_refresh() {
        let store = InMemoryCredentialStore::new();
        store
            .modify::<_, _, std::convert::Infallible>("anthropic", |_| async {
                Ok(Some(Credential::OAuth(OAuthCredential {
                    access: "valid".into(),
                    refresh: Some("r".into()),
                    expires: now_millis() + 600_000,
                    account_id: None,
                })))
            })
            .await
            .unwrap();
        let refreshes = Arc::new(AtomicUsize::new(0));
        let provider = ProviderAuth {
            api_key: None,
            oauth: Some(Arc::new(CountingOAuth {
                refreshes: refreshes.clone(),
            })),
        };
        let ctx = EnvAuthContext::new();
        let r = resolve_provider_auth(
            "anthropic",
            &provider,
            &test_model("anthropic"),
            &store,
            &ctx,
            None,
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(r.auth.api_key.as_deref(), Some("valid"));
        assert_eq!(r.source.as_deref(), Some("OAuth"));
        assert_eq!(
            refreshes.load(Ordering::SeqCst),
            0,
            "valid token must not refresh"
        );
    }

    #[tokio::test]
    async fn resolve_oauth_expired_token_refreshes_once_and_persists() {
        let store = InMemoryCredentialStore::new();
        store
            .modify::<_, _, std::convert::Infallible>("anthropic", |_| async {
                Ok(Some(Credential::OAuth(OAuthCredential {
                    access: "old".into(),
                    refresh: Some("r".into()),
                    expires: now_millis() - 1,
                    account_id: None,
                })))
            })
            .await
            .unwrap();
        let refreshes = Arc::new(AtomicUsize::new(0));
        let provider = ProviderAuth {
            api_key: None,
            oauth: Some(Arc::new(CountingOAuth {
                refreshes: refreshes.clone(),
            })),
        };
        let ctx = EnvAuthContext::new();
        let r = resolve_provider_auth(
            "anthropic",
            &provider,
            &test_model("anthropic"),
            &store,
            &ctx,
            None,
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(r.auth.api_key.as_deref(), Some("fresh"));
        assert_eq!(refreshes.load(Ordering::SeqCst), 1);
        // Rotated credential persisted.
        match store.read("anthropic") {
            Some(Credential::OAuth(o)) => assert_eq!(o.access, "fresh"),
            other => panic!("unexpected: {other:?}"),
        }
    }
}
