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
}
