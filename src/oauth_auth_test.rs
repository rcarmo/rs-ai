//! Test-for-test port of upstream `test/oauth-auth.test.ts`
//! (`@earendil-works/pi-ai` v0.80.2) — the anthropic + openai-codex OAuthAuth
//! adapter cases that rs-ai's credential seam supports.
//!
//! The github-copilot cases (proxy-ep -> baseUrl derivation, enterprise-domain
//! fallback) are N/A: rs-ai's `OAuthCredential` carries no `enterprise_url` and
//! the copilot baseUrl derivation lives in the still-gated Copilot provider gap.

#[cfg(test)]
mod tests {
    use crate::auth::{
        Credential, EnvAuthContext, InMemoryCredentialStore, OAuthAuth, OAuthCredential,
        ProviderAuth, resolve_provider_auth,
    };
    use crate::auth_providers::{AnthropicOAuth, CodexOAuth};
    use crate::types::{Model, ModelCost};
    use crate::utils::now_millis;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn model(provider: &str) -> Model {
        Model {
            id: "m".into(),
            name: "M".into(),
            api: "anthropic-messages".into(),
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

    fn cred(access: &str, expires: i64) -> OAuthCredential {
        OAuthCredential {
            access: access.into(),
            refresh: Some("r".into()),
            expires,
            account_id: None,
        }
    }

    #[tokio::test]
    async fn anthropic_to_auth_derives_the_api_key_from_the_access_token() {
        let auth = AnthropicOAuth::new()
            .to_auth(&cred("token", 0))
            .await
            .unwrap();
        assert_eq!(auth.api_key.as_deref(), Some("token"));
        assert!(auth.base_url.is_none());
        assert!(auth.headers.is_none());
    }

    #[tokio::test]
    async fn openai_codex_to_auth_derives_the_api_key_from_the_access_token() {
        let auth = CodexOAuth::new().to_auth(&cred("token", 0)).await.unwrap();
        assert_eq!(auth.api_key.as_deref(), Some("token"));
    }

    #[tokio::test]
    async fn anthropic_refresh_exchanges_the_refresh_token_and_returns_a_typed_credential() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/oauth/token"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"access_token":"new-access","refresh_token":"new-refresh","expires_in":3600}"#,
            ))
            .mount(&server)
            .await;
        let oauth = AnthropicOAuth {
            token_url: Some(format!("{}/oauth/token", server.uri())),
        };
        let refreshed = oauth
            .refresh(&OAuthCredential {
                access: "old".into(),
                refresh: Some("old-r".into()),
                expires: 0,
                account_id: None,
            })
            .await
            .unwrap();
        assert_eq!(refreshed.access, "new-access");
        assert_eq!(refreshed.refresh.as_deref(), Some("new-refresh"));
        assert!(refreshed.expires > now_millis());
    }

    #[tokio::test]
    async fn resolves_stored_anthropic_oauth_credentials_via_the_resolver() {
        // Mirrors "resolves stored anthropic oauth credentials via the lazy flow import":
        // a valid stored credential resolves to apiKey=<access>, source="OAuth", no refresh.
        let store = InMemoryCredentialStore::new();
        store
            .modify::<_, _, std::convert::Infallible>("anthropic", |_| async {
                Ok(Some(Credential::OAuth(OAuthCredential {
                    access: "oauth-access-token".into(),
                    refresh: Some("r".into()),
                    expires: now_millis() + 60_000,
                    account_id: None,
                })))
            })
            .await
            .unwrap();
        let provider = ProviderAuth {
            api_key: None,
            oauth: Some(Box::new(AnthropicOAuth::new())),
        };
        let ctx = EnvAuthContext::new();
        let result = resolve_provider_auth(
            "anthropic",
            &provider,
            &model("anthropic"),
            &store,
            &ctx,
            None,
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(result.auth.api_key.as_deref(), Some("oauth-access-token"));
        assert_eq!(result.source.as_deref(), Some("OAuth"));
    }
}
