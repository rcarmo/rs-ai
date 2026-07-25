//! Runtime provider/model collection with provider-scoped dynamic catalog storage.
//!
//! This mirrors the production shape of upstream `models.ts`/`models-store.ts`:
//! providers own baseline models plus optional dynamic refresh, refreshes are
//! provider-scoped, coalesced, cache-restored, and best-effort across providers.

use crate::auth::{
    Credential, InMemoryCredentialStore, ModelsError, ModelsErrorCode, ProviderAuth,
};
use crate::types::Model;
use futures::future::join_all;
use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use tokio::sync::watch;

#[derive(Clone, Debug)]
pub struct ModelsStoreEntry {
    pub models: Vec<Model>,
    pub last_modified: Option<i64>,
    pub checked_at: Option<i64>,
    pub etag: Option<String>,
}

#[async_trait::async_trait]
pub trait ModelsStore: Send + Sync {
    async fn read(&self, provider_id: &str) -> Result<Option<ModelsStoreEntry>, ModelsError>;
    async fn write(&self, provider_id: &str, entry: ModelsStoreEntry) -> Result<(), ModelsError>;
    async fn delete(&self, provider_id: &str) -> Result<(), ModelsError>;
}

#[derive(Default)]
pub struct InMemoryModelsStore {
    entries: Mutex<HashMap<String, ModelsStoreEntry>>,
}

impl InMemoryModelsStore {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait::async_trait]
impl ModelsStore for InMemoryModelsStore {
    async fn read(&self, provider_id: &str) -> Result<Option<ModelsStoreEntry>, ModelsError> {
        Ok(self.entries.lock().unwrap().get(provider_id).cloned())
    }
    async fn write(&self, provider_id: &str, entry: ModelsStoreEntry) -> Result<(), ModelsError> {
        self.entries
            .lock()
            .unwrap()
            .insert(provider_id.to_string(), entry);
        Ok(())
    }
    async fn delete(&self, provider_id: &str) -> Result<(), ModelsError> {
        self.entries.lock().unwrap().remove(provider_id);
        Ok(())
    }
}

#[derive(Clone)]
pub struct ProviderModelsStore {
    provider_id: String,
    inner: Arc<dyn ModelsStore>,
}

impl ProviderModelsStore {
    pub async fn read(&self) -> Result<Option<ModelsStoreEntry>, ModelsError> {
        self.inner.read(&self.provider_id).await
    }
    pub async fn write(&self, entry: ModelsStoreEntry) -> Result<(), ModelsError> {
        self.inner.write(&self.provider_id, entry).await
    }
    pub async fn delete(&self) -> Result<(), ModelsError> {
        self.inner.delete(&self.provider_id).await
    }
}

#[derive(Clone)]
pub struct RefreshModelsContext {
    pub credential: Option<Credential>,
    pub store: ProviderModelsStore,
    pub allow_network: bool,
    pub force: bool,
    pub cancel: watch::Receiver<bool>,
}

type RefreshFuture = Pin<Box<dyn Future<Output = Result<Vec<Model>, ModelsError>> + Send>>;
type RefreshFn = Arc<dyn Fn(RefreshModelsContext) -> RefreshFuture + Send + Sync>;

pub struct RuntimeProvider {
    pub id: String,
    pub name: String,
    pub auth: Arc<ProviderAuth>,
    baseline: Vec<Model>,
    dynamic: Mutex<Vec<Model>>,
    refresh: Option<RefreshFn>,
    inflight: tokio::sync::Mutex<Option<std::sync::Weak<tokio::sync::Mutex<()>>>>,
}

impl RuntimeProvider {
    pub fn static_provider(
        id: impl Into<String>,
        name: impl Into<String>,
        auth: ProviderAuth,
        models: Vec<Model>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            auth: Arc::new(auth),
            baseline: models,
            dynamic: Mutex::new(Vec::new()),
            refresh: None,
            inflight: tokio::sync::Mutex::new(None),
        }
    }

    pub fn dynamic<F, Fut>(
        id: impl Into<String>,
        name: impl Into<String>,
        auth: ProviderAuth,
        baseline: Vec<Model>,
        refresh: F,
    ) -> Self
    where
        F: Fn(RefreshModelsContext) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Vec<Model>, ModelsError>> + Send + 'static,
    {
        Self {
            id: id.into(),
            name: name.into(),
            auth: Arc::new(auth),
            baseline,
            dynamic: Mutex::new(Vec::new()),
            refresh: Some(Arc::new(move |ctx| Box::pin(refresh(ctx)))),
            inflight: tokio::sync::Mutex::new(None),
        }
    }

    pub fn get_models(&self) -> Vec<Model> {
        let mut merged = self.baseline.clone();
        for model in self.dynamic.lock().unwrap().iter().cloned() {
            if let Some(pos) = merged.iter().position(|m| m.id == model.id) {
                merged[pos] = model;
            } else {
                merged.push(model);
            }
        }
        merged
    }

    pub fn radius(
        id: impl Into<String>,
        name: impl Into<String>,
        gateway: impl Into<String>,
        baseline: Vec<Model>,
    ) -> Self {
        let gateway = crate::oauth::normalize_radius_gateway_url(&gateway.into());
        Self::dynamic(id, name, ProviderAuth::default(), baseline, move |ctx| {
            let gateway = gateway.clone();
            async move {
                let api_key = match ctx.credential.as_ref() {
                    Some(Credential::ApiKey(c)) => c.key.as_deref(),
                    Some(Credential::OAuth(c)) => Some(c.access.as_str()),
                    None => None,
                };
                let stored = ctx.store.read().await?;
                let config = if let Some(etag) = stored.as_ref().and_then(|s| s.etag.clone()) {
                    let url = format!("{gateway}/v1/config");
                    let mut req = crate::http_proxy::client_for_target(&url, None)
                        .get(&url)
                        .header("accept", "application/json")
                        .header("if-none-match", etag);
                    if let Some(api_key) = api_key {
                        req = req.bearer_auth(api_key);
                    }
                    let resp = req.send().await.map_err(|e| {
                        ModelsError::with_cause(
                            ModelsErrorCode::ModelSource,
                            "Could not load Radius config",
                            e,
                        )
                    })?;
                    if resp.status().as_u16() == 304 {
                        return Ok(stored.map(|s| s.models).unwrap_or_default());
                    }
                    if !resp.status().is_success() {
                        return Err(ModelsError::new(
                            ModelsErrorCode::ModelSource,
                            format!(
                                "Could not load Radius config from {gateway}: {}",
                                resp.status().as_u16()
                            ),
                        ));
                    }
                    crate::oauth::sanitize_radius_gateway_config(resp.json().await.map_err(
                        |e| {
                            ModelsError::with_cause(
                                ModelsErrorCode::ModelSource,
                                "Invalid Radius config",
                                e,
                            )
                        },
                    )?)
                    .map_err(|e| ModelsError::new(ModelsErrorCode::ModelSource, e))?
                } else {
                    crate::oauth::load_radius_gateway_config(&gateway, api_key)
                        .await
                        .map_err(|e| ModelsError::new(ModelsErrorCode::ModelSource, e))?
                };
                let creds = crate::oauth::RadiusOAuthCredentials {
                    access: api_key.unwrap_or_default().to_string(),
                    refresh: None,
                    expires: 0,
                    scope: None,
                    gateway_config: Some(config),
                };
                let radius = crate::auth_providers::RadiusOAuth::new(&gateway);
                Ok(radius.modify_models(&[], "radius", &creds))
            }
        })
    }

    async fn refresh_models(&self, ctx: RefreshModelsContext) -> Result<(), ModelsError> {
        let Some(refresh_fn) = self.refresh.clone() else {
            return Ok(());
        };
        // Coalesce concurrent refreshes per provider: late callers wait for the active guard.
        let (guard_arc, owner) = {
            let mut slot = self.inflight.lock().await;
            if let Some(existing) = slot.as_ref().and_then(|w| w.upgrade()) {
                (existing, false)
            } else {
                let arc = Arc::new(tokio::sync::Mutex::new(()));
                *slot = Some(Arc::downgrade(&arc));
                (arc, true)
            }
        };
        let _guard = guard_arc.lock().await;
        if !owner {
            return Ok(());
        }

        if let Some(stored) = ctx.store.read().await? {
            let filtered = stored
                .models
                .into_iter()
                .filter(|m| m.provider == self.id)
                .collect::<Vec<_>>();
            *self.dynamic.lock().unwrap() = filtered;
        }
        if !ctx.allow_network || *ctx.cancel.borrow() {
            return Ok(());
        }
        let refreshed = refresh_fn(ctx.clone()).await?;
        if *ctx.cancel.borrow() {
            return Ok(());
        }
        *self.dynamic.lock().unwrap() = refreshed.clone();
        ctx.store
            .write(ModelsStoreEntry {
                models: refreshed,
                last_modified: None,
                checked_at: Some(crate::utils::now_millis()),
                etag: None,
            })
            .await
    }
}

#[derive(Default, Clone)]
pub struct RefreshOptions {
    pub allow_network: bool,
    pub force: bool,
    pub cancel: Option<watch::Receiver<bool>>,
}

pub struct RefreshResult {
    pub aborted: bool,
    pub errors: HashMap<String, ModelsError>,
}

pub struct ModelsRuntime {
    providers: Mutex<HashMap<String, Arc<RuntimeProvider>>>,
    pub credentials: Arc<InMemoryCredentialStore>,
    pub models_store: Arc<dyn ModelsStore>,
}

impl ModelsRuntime {
    pub fn new() -> Self {
        Self {
            providers: Mutex::new(HashMap::new()),
            credentials: Arc::new(InMemoryCredentialStore::new()),
            models_store: Arc::new(InMemoryModelsStore::new()),
        }
    }
    pub fn with_models_store(models_store: Arc<dyn ModelsStore>) -> Self {
        Self {
            providers: Mutex::new(HashMap::new()),
            credentials: Arc::new(InMemoryCredentialStore::new()),
            models_store,
        }
    }
    pub fn set_provider(&self, provider: RuntimeProvider) {
        self.providers
            .lock()
            .unwrap()
            .insert(provider.id.clone(), Arc::new(provider));
    }
    pub fn delete_provider(&self, id: &str) {
        self.providers.lock().unwrap().remove(id);
    }
    pub fn clear_providers(&self) {
        self.providers.lock().unwrap().clear();
    }
    pub fn get_models(&self, provider: Option<&str>) -> Vec<Model> {
        let providers = self.providers.lock().unwrap();
        match provider {
            Some(id) => providers
                .get(id)
                .map(|p| p.get_models())
                .unwrap_or_default(),
            None => providers.values().flat_map(|p| p.get_models()).collect(),
        }
    }
    pub fn get_model(&self, provider: &str, id: &str) -> Option<Model> {
        self.get_models(Some(provider))
            .into_iter()
            .find(|m| m.id == id)
    }

    pub fn provider_has_oauth(&self, provider: &str) -> bool {
        self.providers
            .lock()
            .unwrap()
            .get(provider)
            .is_some_and(|p| p.auth.oauth.is_some())
    }

    pub fn populate_builtin_fallbacks(&self) {
        let mut by_provider: HashMap<String, Vec<Model>> = HashMap::new();
        for model in crate::models_generated::builtin_models() {
            by_provider
                .entry(model.provider.clone())
                .or_default()
                .push(model);
        }
        for (id, models) in by_provider {
            if !self.providers.lock().unwrap().contains_key(&id) {
                let auth = builtin_provider_auth(&id);
                self.set_provider(RuntimeProvider::static_provider(
                    id.clone(),
                    id,
                    auth,
                    models,
                ));
            }
        }
    }

    pub async fn refresh(&self, mut options: RefreshOptions) -> RefreshResult {
        let (tx, rx) = watch::channel(false);
        let cancel = options.cancel.take().unwrap_or(rx);
        drop(tx);
        let providers = self
            .providers
            .lock()
            .unwrap()
            .values()
            .cloned()
            .collect::<Vec<_>>();
        let futs = providers
            .into_iter()
            .filter(|p| p.refresh.is_some())
            .map(|provider| {
                let store = ProviderModelsStore {
                    provider_id: provider.id.clone(),
                    inner: self.models_store.clone(),
                };
                let creds = self.credentials.clone();
                let cancel = cancel.clone();
                async move {
                    let stored = creds.read(&provider.id);
                    let credential = stored.clone();
                    let ctx = RefreshModelsContext {
                        credential,
                        store: store.clone(),
                        allow_network: options.allow_network,
                        force: options.force,
                        cancel: cancel.clone(),
                    };
                    match provider.refresh_models(ctx).await {
                        Ok(()) => (provider.id.clone(), None),
                        Err(err) => {
                            let restore_ctx = RefreshModelsContext {
                                credential: stored,
                                store,
                                allow_network: false,
                                force: false,
                                cancel,
                            };
                            let _ = provider.refresh_models(restore_ctx).await;
                            (provider.id.clone(), Some(err))
                        }
                    }
                }
            });
        let mut errors = HashMap::new();
        for (id, err) in join_all(futs).await {
            if let Some(err) = err {
                errors.insert(id, err);
            }
        }
        RefreshResult {
            aborted: *cancel.borrow(),
            errors,
        }
    }
}

impl Default for ModelsRuntime {
    fn default() -> Self {
        Self::new()
    }
}

fn builtin_provider_auth(provider_id: &str) -> ProviderAuth {
    match provider_id {
        "openrouter" => ProviderAuth {
            api_key: None,
            oauth: Some(Box::new(crate::auth_providers::OpenRouterOAuth {
                token_url: None,
            })),
        },
        "kimi-coding" => ProviderAuth {
            api_key: None,
            oauth: Some(Box::new(crate::auth_providers::KimiCodeOAuth {
                oauth_host: None,
            })),
        },
        "xai" => ProviderAuth {
            api_key: None,
            oauth: Some(Box::new(crate::auth_providers::XaiOAuth::new())),
        },
        "openai-codex" => ProviderAuth {
            api_key: None,
            oauth: Some(Box::new(crate::auth_providers::CodexOAuth::new())),
        },
        "anthropic" => ProviderAuth {
            api_key: None,
            oauth: Some(Box::new(crate::auth_providers::AnthropicOAuth::new())),
        },
        _ => ProviderAuth::default(),
    }
}

pub fn model_set(models: &[Model]) -> HashSet<(String, String)> {
    models
        .iter()
        .map(|m| (m.provider.clone(), m.id.clone()))
        .collect()
}
