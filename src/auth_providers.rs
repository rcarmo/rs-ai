//! Concrete provider auth implementations that wire the existing OAuth
//! primitives (`src/oauth.rs`) into the credential seam (`src/auth.rs`).
//!
//! These are the `OAuthAuth` impls `resolve_provider_auth` drives: `refresh`
//! exchanges the stored refresh token (under the credential-store lock) and
//! `to_auth` derives request `ModelAuth` from a valid credential. The
//! `token_url` override exists so the refresh network call can be pointed at a
//! mock server in tests (production leaves it `None`).

use crate::auth::{ModelAuth, ModelsError, ModelsErrorCode, OAuthAuth, OAuthCredential};
use tokio::sync::watch;

fn oauth_err(msg: impl Into<String>) -> ModelsError {
    ModelsError::new(ModelsErrorCode::OAuth, msg)
}

fn oauth_abort_err() -> ModelsError {
    ModelsError::new(ModelsErrorCode::OAuth, "AbortError: OAuth refresh aborted")
}

async fn wait_cancelled(mut cancel: watch::Receiver<bool>) {
    loop {
        if *cancel.borrow() {
            return;
        }
        if cancel.changed().await.is_err() {
            std::future::pending::<()>().await;
        }
    }
}

async fn cancelable_oauth_call<T, Fut>(
    future: Fut,
    cancel: Option<watch::Receiver<bool>>,
) -> Result<T, ModelsError>
where
    Fut: std::future::Future<Output = Result<T, String>> + Send,
{
    if cancel.as_ref().is_some_and(|rx| *rx.borrow()) {
        return Err(oauth_abort_err());
    }
    match cancel {
        Some(rx) => {
            tokio::select! {
                biased;
                _ = wait_cancelled(rx) => Err(oauth_abort_err()),
                result = future => result.map_err(oauth_err),
            }
        }
        None => future.await.map_err(oauth_err),
    }
}

/// OpenAI Codex (ChatGPT) OAuth. `to_auth` exposes the access token as the
/// request api key; the provider derives the `chatgpt-account-id` header from
/// the token claims at request time.
pub struct CodexOAuth {
    pub token_url: Option<String>,
}

impl CodexOAuth {
    pub fn new() -> Self {
        Self { token_url: None }
    }
}

impl Default for CodexOAuth {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl OAuthAuth for CodexOAuth {
    async fn refresh(&self, credential: &OAuthCredential) -> Result<OAuthCredential, ModelsError> {
        let refresh = credential
            .refresh
            .as_deref()
            .ok_or_else(|| oauth_err("codex credential is missing a refresh token"))?;
        let creds = match self.token_url.as_deref() {
            Some(url) => crate::oauth::refresh_codex_token_at(url, refresh).await,
            None => crate::oauth::refresh_codex_token(refresh).await,
        }
        .map_err(oauth_err)?;
        Ok(OAuthCredential {
            access: creds.access,
            refresh: creds.refresh,
            expires: creds.expires_at_ms,
            account_id: Some(creds.account_id),
        })
    }

    async fn refresh_with_cancel(
        &self,
        credential: &OAuthCredential,
        cancel: Option<watch::Receiver<bool>>,
    ) -> Result<OAuthCredential, ModelsError> {
        let refresh = credential
            .refresh
            .as_deref()
            .ok_or_else(|| oauth_err("codex credential is missing a refresh token"))?;
        let call = async move {
            match self.token_url.as_deref() {
                Some(url) => crate::oauth::refresh_codex_token_at(url, refresh).await,
                None => crate::oauth::refresh_codex_token(refresh).await,
            }
            .map(|creds| OAuthCredential {
                access: creds.access,
                refresh: creds.refresh,
                expires: creds.expires_at_ms,
                account_id: Some(creds.account_id),
            })
        };
        cancelable_oauth_call(call, cancel).await
    }

    async fn to_auth(&self, credential: &OAuthCredential) -> Result<ModelAuth, ModelsError> {
        Ok(ModelAuth {
            api_key: Some(credential.access.clone()),
            ..Default::default()
        })
    }
}

/// Anthropic (Claude Pro/Max) OAuth.
pub struct AnthropicOAuth {
    pub token_url: Option<String>,
}

impl AnthropicOAuth {
    pub fn new() -> Self {
        Self { token_url: None }
    }
}

impl Default for AnthropicOAuth {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl OAuthAuth for AnthropicOAuth {
    async fn refresh(&self, credential: &OAuthCredential) -> Result<OAuthCredential, ModelsError> {
        let refresh = credential
            .refresh
            .as_deref()
            .ok_or_else(|| oauth_err("anthropic credential is missing a refresh token"))?;
        let tok = match self.token_url.as_deref() {
            Some(url) => crate::oauth::refresh_anthropic_token_at(url, refresh).await,
            None => crate::oauth::refresh_anthropic_token(refresh).await,
        }
        .map_err(oauth_err)?;
        Ok(OAuthCredential {
            access: tok.access,
            refresh: tok.refresh,
            expires: tok.expires_at_ms,
            account_id: None,
        })
    }

    async fn refresh_with_cancel(
        &self,
        credential: &OAuthCredential,
        cancel: Option<watch::Receiver<bool>>,
    ) -> Result<OAuthCredential, ModelsError> {
        let refresh = credential
            .refresh
            .as_deref()
            .ok_or_else(|| oauth_err("anthropic credential is missing a refresh token"))?;
        let call = async move {
            match self.token_url.as_deref() {
                Some(url) => crate::oauth::refresh_anthropic_token_at(url, refresh).await,
                None => crate::oauth::refresh_anthropic_token(refresh).await,
            }
            .map(|tok| OAuthCredential {
                access: tok.access,
                refresh: tok.refresh,
                expires: tok.expires_at_ms,
                account_id: None,
            })
        };
        cancelable_oauth_call(call, cancel).await
    }

    async fn to_auth(&self, credential: &OAuthCredential) -> Result<ModelAuth, ModelsError> {
        Ok(ModelAuth {
            api_key: Some(credential.access.clone()),
            ..Default::default()
        })
    }
}

/// OpenRouter OAuth (permanent API key exchange).
pub struct OpenRouterOAuth {
    pub token_url: Option<String>,
}

#[async_trait::async_trait]
impl OAuthAuth for OpenRouterOAuth {
    async fn refresh(&self, credential: &OAuthCredential) -> Result<OAuthCredential, ModelsError> {
        Ok(credential.clone())
    }
    async fn refresh_with_cancel(
        &self,
        credential: &OAuthCredential,
        cancel: Option<watch::Receiver<bool>>,
    ) -> Result<OAuthCredential, ModelsError> {
        if cancel.as_ref().is_some_and(|rx| *rx.borrow()) {
            return Err(oauth_abort_err());
        }
        Ok(credential.clone())
    }
    async fn to_auth(&self, credential: &OAuthCredential) -> Result<ModelAuth, ModelsError> {
        Ok(ModelAuth {
            api_key: Some(credential.access.clone()),
            ..Default::default()
        })
    }
}

/// Kimi Code subscription OAuth.
pub struct KimiCodeOAuth {
    pub oauth_host: Option<String>,
}

#[async_trait::async_trait]
impl OAuthAuth for KimiCodeOAuth {
    async fn refresh(&self, credential: &OAuthCredential) -> Result<OAuthCredential, ModelsError> {
        let refresh = credential
            .refresh
            .as_deref()
            .ok_or_else(|| oauth_err("kimi credential is missing a refresh token"))?;
        crate::oauth::refresh_kimi_code_token_at(
            self.oauth_host
                .as_deref()
                .unwrap_or(crate::oauth::KIMI_CODE_OAUTH_HOST),
            refresh,
        )
        .await
        .map_err(oauth_err)
    }
    async fn refresh_with_cancel(
        &self,
        credential: &OAuthCredential,
        cancel: Option<watch::Receiver<bool>>,
    ) -> Result<OAuthCredential, ModelsError> {
        let refresh = credential
            .refresh
            .as_deref()
            .ok_or_else(|| oauth_err("kimi credential is missing a refresh token"))?;
        let host = self
            .oauth_host
            .as_deref()
            .unwrap_or(crate::oauth::KIMI_CODE_OAUTH_HOST);
        cancelable_oauth_call(
            crate::oauth::refresh_kimi_code_token_at(host, refresh),
            cancel,
        )
        .await
    }

    async fn to_auth(&self, credential: &OAuthCredential) -> Result<ModelAuth, ModelsError> {
        Ok(ModelAuth {
            api_key: Some(credential.access.clone()),
            ..Default::default()
        })
    }
}

/// xAI (Grok/X subscription) OAuth.
pub struct XaiOAuth {
    pub token_url: Option<String>,
}

impl XaiOAuth {
    pub fn new() -> Self {
        Self { token_url: None }
    }
}

impl Default for XaiOAuth {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl OAuthAuth for XaiOAuth {
    async fn refresh(&self, credential: &OAuthCredential) -> Result<OAuthCredential, ModelsError> {
        let refresh = credential
            .refresh
            .as_deref()
            .ok_or_else(|| oauth_err("xai credential is missing a refresh token"))?;
        match self.token_url.as_deref() {
            Some(url) => crate::oauth::refresh_xai_token_at(url, refresh).await,
            None => crate::oauth::refresh_xai_token(refresh).await,
        }
        .map_err(oauth_err)
    }

    async fn refresh_with_cancel(
        &self,
        credential: &OAuthCredential,
        cancel: Option<watch::Receiver<bool>>,
    ) -> Result<OAuthCredential, ModelsError> {
        let refresh = credential
            .refresh
            .as_deref()
            .ok_or_else(|| oauth_err("xai credential is missing a refresh token"))?;
        let call = async move {
            match self.token_url.as_deref() {
                Some(url) => crate::oauth::refresh_xai_token_at(url, refresh).await,
                None => crate::oauth::refresh_xai_token(refresh).await,
            }
        };
        cancelable_oauth_call(call, cancel).await
    }

    async fn to_auth(&self, credential: &OAuthCredential) -> Result<ModelAuth, ModelsError> {
        Ok(ModelAuth {
            api_key: Some(credential.access.clone()),
            ..Default::default()
        })
    }
}

/// Radius gateway OAuth. The refresh path discovers the gateway OAuth metadata,
/// refreshes the stored token, and exposes the access token as the request API key.
pub struct RadiusOAuth {
    pub gateway: String,
}

impl RadiusOAuth {
    pub fn new(gateway: impl Into<String>) -> Self {
        Self {
            gateway: crate::oauth::normalize_radius_gateway_url(&gateway.into()),
        }
    }

    pub async fn refresh_with_gateway_config(
        &self,
        credential: &crate::oauth::RadiusOAuthCredentials,
    ) -> Result<crate::oauth::RadiusOAuthCredentials, ModelsError> {
        let refresh = credential
            .refresh
            .as_deref()
            .ok_or_else(|| oauth_err("radius credential is missing a refresh token"))?;
        let oauth = crate::oauth::load_radius_oauth_config(&self.gateway)
            .await
            .map_err(oauth_err)?;
        let refreshed = crate::oauth::refresh_radius_token(&oauth, refresh)
            .await
            .map_err(oauth_err)?;
        crate::oauth::attach_radius_gateway_config(&self.gateway, refreshed, Some(credential))
            .await
            .map_err(oauth_err)
    }

    pub fn modify_models(
        &self,
        models: &[crate::types::Model],
        provider_id: &str,
        credentials: &crate::oauth::RadiusOAuthCredentials,
    ) -> Vec<crate::types::Model> {
        let Some(config) = credentials.gateway_config.as_ref() else {
            return models.to_vec();
        };
        let mut out = models.to_vec();
        let existing: std::collections::HashSet<String> = out
            .iter()
            .filter(|m| m.provider == provider_id)
            .map(|m| m.id.clone())
            .collect();
        for model in &config.models {
            if existing.contains(&model.id) {
                continue;
            }
            out.push(crate::types::Model {
                id: model.id.clone(),
                name: model.name.clone(),
                api: crate::types::api::PI_MESSAGES.to_string(),
                provider: provider_id.to_string(),
                base_url: config.base_url.clone(),
                reasoning: model.reasoning,
                thinking_level_map: model.thinking_level_map.clone(),
                input: model.input.clone(),
                cost: model.cost.clone(),
                context_window: model.context_window,
                max_tokens: model.max_tokens,
                sampling_params: None,
                headers: None,
                api_key: None,
                compat: Default::default(),
            });
        }
        out
    }
}

#[async_trait::async_trait]
impl OAuthAuth for RadiusOAuth {
    async fn refresh(&self, credential: &OAuthCredential) -> Result<OAuthCredential, ModelsError> {
        let refresh = credential
            .refresh
            .as_deref()
            .ok_or_else(|| oauth_err("radius credential is missing a refresh token"))?;
        let oauth = crate::oauth::load_radius_oauth_config(&self.gateway)
            .await
            .map_err(oauth_err)?;
        let refreshed = crate::oauth::refresh_radius_token(&oauth, refresh)
            .await
            .map_err(oauth_err)?;
        Ok(OAuthCredential {
            access: refreshed.access,
            refresh: refreshed.refresh,
            expires: refreshed.expires,
            account_id: None,
        })
    }

    async fn refresh_with_cancel(
        &self,
        credential: &OAuthCredential,
        cancel: Option<watch::Receiver<bool>>,
    ) -> Result<OAuthCredential, ModelsError> {
        let refresh = credential
            .refresh
            .as_deref()
            .ok_or_else(|| oauth_err("radius credential is missing a refresh token"))?;
        let call = async move {
            let oauth = crate::oauth::load_radius_oauth_config(&self.gateway).await?;
            let refreshed = crate::oauth::refresh_radius_token(&oauth, refresh).await?;
            Ok(OAuthCredential {
                access: refreshed.access,
                refresh: refreshed.refresh,
                expires: refreshed.expires,
                account_id: None,
            })
        };
        cancelable_oauth_call(call, cancel).await
    }

    async fn to_auth(&self, credential: &OAuthCredential) -> Result<ModelAuth, ModelsError> {
        Ok(ModelAuth {
            api_key: Some(credential.access.clone()),
            ..Default::default()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::{
        ApiKeyCredential, Credential, EnvAuthContext, InMemoryCredentialStore, ProviderAuth,
        resolve_provider_auth,
    };
    use crate::types::{Model, ModelCost};
    use crate::utils::now_millis;
    use tokio::sync::watch;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn model(provider: &str) -> Model {
        Model {
            id: "m".into(),
            name: "M".into(),
            api: "openai-codex-responses".into(),
            provider: provider.into(),
            base_url: "http://x".into(),
            reasoning: false,
            thinking_level_map: None,
            input: vec!["text".into()],
            cost: ModelCost::default(),
            context_window: 1000,
            max_tokens: 100,
            sampling_params: None,
            headers: None,
            api_key: None,
            compat: Default::default(),
        }
    }

    #[tokio::test]
    async fn codex_oauth_refreshes_expired_credential_through_resolver() {
        use base64::Engine;
        // A JWT carrying the chatgpt_account_id claim (refresh returns this access token).
        let payload =
            serde_json::json!({ "https://api.openai.com/auth": {"chatgpt_account_id": "acc_123"} });
        let payload_b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(serde_json::to_vec(&payload).unwrap());
        let jwt = format!("h.{payload_b64}.s");
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/oauth/token"))
            .respond_with(ResponseTemplate::new(200).set_body_string(format!(
                r#"{{"access_token":"{jwt}","refresh_token":"new-refresh","expires_in":3600}}"#
            )))
            .mount(&server)
            .await;

        let store = InMemoryCredentialStore::new();
        store
            .modify::<_, _, std::convert::Infallible>("openai-codex", |_| async {
                Ok(Some(Credential::OAuth(OAuthCredential {
                    access: "stale".into(),
                    refresh: Some("old".into()),
                    expires: now_millis() - 1,
                    account_id: None,
                })))
            })
            .await
            .unwrap();

        let provider = ProviderAuth {
            api_key: None,
            oauth: Some(Box::new(CodexOAuth {
                token_url: Some(format!("{}/oauth/token", server.uri())),
            })),
        };
        let ctx = EnvAuthContext::new();
        let result = resolve_provider_auth(
            "openai-codex",
            &provider,
            &model("openai-codex"),
            &store,
            &ctx,
            None,
        )
        .await
        .unwrap()
        .unwrap();

        // to_auth surfaces the refreshed access token; source is "OAuth".
        assert_eq!(result.auth.api_key.as_deref(), Some(jwt.as_str()));
        assert_eq!(result.source.as_deref(), Some("OAuth"));
        // The rotated credential (incl. extracted account id) is persisted.
        match store.read("openai-codex") {
            Some(Credential::OAuth(o)) => {
                assert_eq!(o.access, jwt);
                assert_eq!(o.refresh.as_deref(), Some("new-refresh"));
                assert_eq!(o.account_id.as_deref(), Some("acc_123"));
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[derive(Clone, Copy, Debug)]
    enum RealOAuthCase {
        Anthropic,
        Codex,
        Kimi,
        Xai,
        Radius,
    }

    fn jwt(account_id: &str) -> String {
        use base64::Engine;
        let payload = serde_json::json!({
            "https://api.openai.com/auth": {"chatgpt_account_id": account_id}
        });
        let payload_b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(serde_json::to_vec(&payload).unwrap());
        format!("h.{payload_b64}.s")
    }

    fn oauth_provider(case: RealOAuthCase, server: &MockServer) -> (String, ProviderAuth) {
        let base = server.uri();
        match case {
            RealOAuthCase::Anthropic => (
                "anthropic".into(),
                ProviderAuth {
                    api_key: None,
                    oauth: Some(Box::new(AnthropicOAuth {
                        token_url: Some(format!("{base}/oauth/token")),
                    })),
                },
            ),
            RealOAuthCase::Codex => (
                "openai-codex".into(),
                ProviderAuth {
                    api_key: None,
                    oauth: Some(Box::new(CodexOAuth {
                        token_url: Some(format!("{base}/oauth/token")),
                    })),
                },
            ),
            RealOAuthCase::Kimi => (
                "kimi-coding".into(),
                ProviderAuth {
                    api_key: None,
                    oauth: Some(Box::new(KimiCodeOAuth {
                        oauth_host: Some(base),
                    })),
                },
            ),
            RealOAuthCase::Xai => (
                "xai".into(),
                ProviderAuth {
                    api_key: None,
                    oauth: Some(Box::new(XaiOAuth {
                        token_url: Some(format!("{base}/token")),
                    })),
                },
            ),
            RealOAuthCase::Radius => (
                "radius".into(),
                ProviderAuth {
                    api_key: None,
                    oauth: Some(Box::new(RadiusOAuth::new(base))),
                },
            ),
        }
    }

    async fn mount_oauth_success(case: RealOAuthCase, server: &MockServer, delay_ms: u64) {
        let delay = std::time::Duration::from_millis(delay_ms);
        match case {
            RealOAuthCase::Anthropic => {
                Mock::given(method("POST"))
                    .and(path("/oauth/token"))
                    .respond_with(ResponseTemplate::new(200).set_delay(delay).set_body_json(
                        serde_json::json!({
                            "access_token":"fresh-anthropic",
                            "refresh_token":"fresh-refresh",
                            "expires_in":3600
                        }),
                    ))
                    .mount(server)
                    .await;
            }
            RealOAuthCase::Codex => {
                Mock::given(method("POST"))
                    .and(path("/oauth/token"))
                    .respond_with(ResponseTemplate::new(200).set_delay(delay).set_body_json(
                        serde_json::json!({
                            "access_token": jwt("acc-real"),
                            "refresh_token":"fresh-refresh",
                            "expires_in":3600
                        }),
                    ))
                    .mount(server)
                    .await;
            }
            RealOAuthCase::Kimi => {
                Mock::given(method("POST"))
                    .and(path("/api/oauth/token"))
                    .respond_with(ResponseTemplate::new(200).set_delay(delay).set_body_json(
                        serde_json::json!({
                            "access_token":"fresh-kimi",
                            "refresh_token":"fresh-refresh",
                            "expires_in":3600
                        }),
                    ))
                    .mount(server)
                    .await;
            }
            RealOAuthCase::Xai => {
                Mock::given(method("POST"))
                    .and(path("/token"))
                    .respond_with(ResponseTemplate::new(200).set_delay(delay).set_body_json(
                        serde_json::json!({
                            "access_token":"fresh-xai",
                            "refresh_token":"fresh-refresh",
                            "expires_in":3600
                        }),
                    ))
                    .mount(server)
                    .await;
            }
            RealOAuthCase::Radius => {
                Mock::given(method("GET"))
                    .and(path("/v1/oauth"))
                    .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                        "authorizationEndpoint": format!("{}/authorize", server.uri()),
                        "tokenEndpoint": format!("{}/token", server.uri())
                    })))
                    .mount(server)
                    .await;
                Mock::given(method("POST"))
                    .and(path("/token"))
                    .respond_with(ResponseTemplate::new(200).set_delay(delay).set_body_json(
                        serde_json::json!({
                            "access_token":"fresh-radius",
                            "refresh_token":"fresh-refresh",
                            "expires_in":3600
                        }),
                    ))
                    .mount(server)
                    .await;
            }
        }
    }

    async fn seed_expired(store: &InMemoryCredentialStore, provider_id: &str) {
        store
            .modify::<_, _, std::convert::Infallible>(provider_id, |_| async {
                Ok(Some(Credential::OAuth(OAuthCredential {
                    access: "old-access".into(),
                    refresh: Some("old-refresh".into()),
                    expires: now_millis() - 1,
                    account_id: None,
                })))
            })
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn real_oauth_providers_pre_cancel_without_network_or_rotation() {
        for case in [
            RealOAuthCase::Anthropic,
            RealOAuthCase::Codex,
            RealOAuthCase::Kimi,
            RealOAuthCase::Xai,
            RealOAuthCase::Radius,
        ] {
            let server = MockServer::start().await;
            mount_oauth_success(case, &server, 1).await;
            let (provider_id, provider) = oauth_provider(case, &server);
            let store = InMemoryCredentialStore::new();
            seed_expired(&store, &provider_id).await;
            let (_tx, rx) = watch::channel(true);
            let err = resolve_provider_auth(
                &provider_id,
                &provider,
                &model(&provider_id),
                &store,
                &EnvAuthContext::new(),
                Some(&crate::auth::AuthResolutionOverrides {
                    cancel: Some(rx),
                    ..Default::default()
                }),
            )
            .await
            .unwrap_err();
            assert_eq!(err.code, ModelsErrorCode::OAuth, "case {case:?}: {err}");
            assert!(err.message.contains("AbortError"), "case {case:?}: {err}");
            assert!(
                server.received_requests().await.unwrap().is_empty(),
                "case {case:?}"
            );
            match store.read(&provider_id) {
                Some(Credential::OAuth(o)) => assert_eq!(o.access, "old-access", "case {case:?}"),
                other => panic!("case {case:?}: unexpected {other:?}"),
            }
        }
    }

    #[tokio::test]
    async fn real_oauth_providers_mid_refresh_cancel_without_rotation() {
        for case in [
            RealOAuthCase::Anthropic,
            RealOAuthCase::Codex,
            RealOAuthCase::Kimi,
            RealOAuthCase::Xai,
            RealOAuthCase::Radius,
        ] {
            let server = MockServer::start().await;
            mount_oauth_success(case, &server, 250).await;
            let (provider_id, provider) = oauth_provider(case, &server);
            let store = InMemoryCredentialStore::new();
            seed_expired(&store, &provider_id).await;
            let (tx, rx) = watch::channel(false);
            let request_model = model(&provider_id);
            let env_ctx = EnvAuthContext::new();
            let overrides = crate::auth::AuthResolutionOverrides {
                cancel: Some(rx),
                ..Default::default()
            };
            let fut = resolve_provider_auth(
                &provider_id,
                &provider,
                &request_model,
                &store,
                &env_ctx,
                Some(&overrides),
            );
            let err = tokio::time::timeout(std::time::Duration::from_millis(500), async {
                tokio::pin!(fut);
                tokio::time::sleep(std::time::Duration::from_millis(30)).await;
                tx.send(true).unwrap();
                fut.await.unwrap_err()
            })
            .await
            .expect("cancelled OAuth refresh must finish promptly");
            assert_eq!(err.code, ModelsErrorCode::OAuth, "case {case:?}: {err}");
            assert!(err.message.contains("AbortError"), "case {case:?}: {err}");
            match store.read(&provider_id) {
                Some(Credential::OAuth(o)) => assert_eq!(o.access, "old-access", "case {case:?}"),
                other => panic!("case {case:?}: unexpected {other:?}"),
            }
        }
    }

    #[tokio::test]
    async fn openrouter_oauth_honors_pre_cancel_without_mutation() {
        let provider = ProviderAuth {
            api_key: None,
            oauth: Some(Box::new(OpenRouterOAuth { token_url: None })),
        };
        let store = InMemoryCredentialStore::new();
        seed_expired(&store, "openrouter").await;
        let (_tx, rx) = watch::channel(true);
        let err = resolve_provider_auth(
            "openrouter",
            &provider,
            &model("openrouter"),
            &store,
            &EnvAuthContext::new(),
            Some(&crate::auth::AuthResolutionOverrides {
                cancel: Some(rx),
                ..Default::default()
            }),
        )
        .await
        .unwrap_err();
        assert_eq!(err.code, ModelsErrorCode::OAuth);
        assert!(err.message.contains("AbortError"));
    }

    #[tokio::test]
    async fn codex_oauth_missing_refresh_token_errors() {
        let oauth = CodexOAuth::new();
        let err = oauth
            .refresh(&OAuthCredential {
                access: "a".into(),
                refresh: None,
                expires: 0,
                account_id: None,
            })
            .await
            .unwrap_err();
        assert_eq!(err.code, ModelsErrorCode::OAuth);
        assert!(err.message.contains("missing a refresh token"));
    }

    #[tokio::test]
    async fn stored_api_key_credential_still_resolves_when_provider_has_oauth_only_for_other_path()
    {
        // Sanity: an api-key provider path is unaffected by the OAuth impls.
        let store = InMemoryCredentialStore::new();
        store
            .modify::<_, _, std::convert::Infallible>("openai", |_| async {
                Ok(Some(Credential::ApiKey(ApiKeyCredential {
                    key: Some("sk".into()),
                    env: None,
                })))
            })
            .await
            .unwrap();
        struct PassThrough;
        #[async_trait::async_trait]
        impl crate::auth::ApiKeyAuth for PassThrough {
            async fn resolve(
                &self,
                _m: &Model,
                _c: &dyn crate::auth::AuthContext,
                cred: Option<&ApiKeyCredential>,
            ) -> Result<Option<crate::auth::AuthResult>, ModelsError> {
                Ok(cred
                    .and_then(|c| c.key.clone())
                    .map(|k| crate::auth::AuthResult {
                        auth: ModelAuth {
                            api_key: Some(k),
                            ..Default::default()
                        },
                        env: None,
                        source: None,
                    }))
            }
        }
        let provider = ProviderAuth {
            api_key: Some(Box::new(PassThrough)),
            oauth: None,
        };
        let ctx = EnvAuthContext::new();
        let r = resolve_provider_auth("openai", &provider, &model("openai"), &store, &ctx, None)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(r.auth.api_key.as_deref(), Some("sk"));
    }
}
