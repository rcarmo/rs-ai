//! Concrete provider auth implementations that wire the existing OAuth
//! primitives (`src/oauth.rs`) into the credential seam (`src/auth.rs`).
//!
//! These are the `OAuthAuth` impls `resolve_provider_auth` drives: `refresh`
//! exchanges the stored refresh token (under the credential-store lock) and
//! `to_auth` derives request `ModelAuth` from a valid credential. The
//! `token_url` override exists so the refresh network call can be pointed at a
//! mock server in tests (production leaves it `None`).

use crate::auth::{ModelAuth, ModelsError, ModelsErrorCode, OAuthAuth, OAuthCredential};

fn oauth_err(msg: impl Into<String>) -> ModelsError {
    ModelsError::new(ModelsErrorCode::OAuth, msg)
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
