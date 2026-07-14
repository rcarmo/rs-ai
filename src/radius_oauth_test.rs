//! Deterministic Radius OAuth helper/adapter coverage for upstream
//! `src/utils/oauth/radius.ts` (v0.80.7). Browser opening itself is a UI side
//! effect; the portable Rust surface is discovery, PKCE URL construction,
//! token exchange/refresh, device authorization, gateway config caching, and
//! catalog-to-model modification.

#[cfg(test)]
mod tests {
    use crate::auth::OAuthAuth;
    use crate::auth_providers::RadiusOAuth;
    use crate::oauth::*;
    use crate::types::{Model, ModelCost};
    use serde_json::json;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn oauth_config(base: &str) -> serde_json::Value {
        json!({
            "issuer": base,
            "authorizationEndpoint": format!("{base}/authorize"),
            "tokenEndpoint": format!("{base}/token"),
            "deviceAuthorizationEndpoint": format!("{base}/device"),
            "deviceAuthorizationEventsEndpoint": format!("{base}/events"),
            "verificationEndpoint": format!("{base}/verify"),
            "clientId": "radius-client",
            "scope": "openid profile",
            "deviceCodeGrantType": "urn:ietf:params:oauth:grant-type:device_code"
        })
    }

    fn base_model() -> Model {
        Model {
            id: "existing".into(),
            name: "Existing".into(),
            api: "pi-messages".into(),
            provider: "radius".into(),
            base_url: "https://old/v1".into(),
            reasoning: false,
            thinking_level_map: None,
            input: vec!["text".into()],
            cost: ModelCost::default(),
            context_window: 1,
            max_tokens: 1,
            headers: None,
            api_key: None,
            compat: Default::default(),
        }
    }

    #[tokio::test]
    async fn discovers_oauth_config_exchanges_code_and_attaches_gateway_config() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/oauth"))
            .respond_with(ResponseTemplate::new(200).set_body_json(oauth_config(&server.uri())))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "access_token":"access-1", "refresh_token":"refresh-1", "expires_in":3600, "scope":"openid profile"
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/v1/config"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "baseUrl": format!("{}/v1", server.uri()),
                "models": [{
                    "id":"auto", "name":"Radius Auto", "reasoning":true,
                    "thinkingLevelMap":{"high":"high"}, "input":["text","image"],
                    "cost":{"input":1,"output":2,"cacheRead":0,"cacheWrite":0},
                    "contextWindow":128000, "maxTokens":16384
                }, {"id":"bad"}]
            })))
            .mount(&server)
            .await;

        let oauth = load_radius_oauth_config(&server.uri()).await.unwrap();
        assert_eq!(oauth.client_id, "radius-client");
        let creds = exchange_radius_code(&oauth, "code-1", "verifier-1", RADIUS_REDIRECT_URI)
            .await
            .unwrap();
        assert_eq!(creds.access, "access-1");
        assert_eq!(creds.refresh.as_deref(), Some("refresh-1"));
        let creds = attach_radius_gateway_config(&server.uri(), creds, None)
            .await
            .unwrap();
        let config = creds.gateway_config.as_ref().unwrap();
        assert_eq!(
            config.models.len(),
            1,
            "malformed catalog entries are dropped"
        );
        assert_eq!(config.models[0].id, "auto");
        assert_eq!(config.models[0].input, vec!["text", "image"]);

        let requests = server.received_requests().await.unwrap();
        let token_body = requests
            .iter()
            .find(|r| r.url.path() == "/token")
            .map(|r| String::from_utf8_lossy(&r.body).to_string())
            .unwrap();
        assert!(token_body.contains("grant_type=authorization_code"));
        assert!(token_body.contains("client_id=radius-client"));
        assert!(token_body.contains("code=code-1"));
        assert!(token_body.contains("code_verifier=verifier-1"));
        let config_req = requests
            .iter()
            .find(|r| r.url.path() == "/v1/config")
            .unwrap();
        assert_eq!(
            config_req
                .headers
                .get("authorization")
                .and_then(|v| v.to_str().ok()),
            Some("Bearer access-1")
        );
    }

    #[tokio::test]
    async fn refresh_adapter_uses_discovery_and_preserves_previous_config_on_transient_config_failure()
     {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/oauth"))
            .respond_with(ResponseTemplate::new(200).set_body_json(oauth_config(&server.uri())))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "access_token":"access-2", "refresh_token":"refresh-2", "expires_in":120
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/v1/config"))
            .respond_with(ResponseTemplate::new(503).set_body_string("try later"))
            .mount(&server)
            .await;

        let previous = RadiusOAuthCredentials {
            access: "old".into(),
            refresh: Some("refresh-old".into()),
            expires: 0,
            scope: None,
            gateway_config: Some(RadiusGatewayConfig {
                base_url: "https://cached/v1".into(),
                models: vec![RadiusGatewayModel {
                    id: "cached".into(),
                    name: "Cached".into(),
                    reasoning: false,
                    thinking_level_map: None,
                    input: vec!["text".into()],
                    cost: ModelCost::default(),
                    context_window: 10,
                    max_tokens: 5,
                }],
            }),
        };
        let radius = RadiusOAuth::new(server.uri());
        let refreshed = radius.refresh_with_gateway_config(&previous).await.unwrap();
        assert_eq!(refreshed.access, "access-2");
        assert_eq!(
            refreshed.gateway_config.unwrap().base_url,
            "https://cached/v1"
        );
    }

    #[tokio::test]
    async fn builds_browser_authorize_url_and_requests_device_authorization() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/device"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "device_code":"dev-1", "user_code":"USER-1", "verification_uri":"https://verify", "expires_in":600, "interval":7
            })))
            .mount(&server)
            .await;
        let oauth = RadiusOAuthConfig {
            issuer: server.uri(),
            authorization_endpoint: format!("{}/authorize", server.uri()),
            token_endpoint: format!("{}/token", server.uri()),
            device_authorization_endpoint: format!("{}/device", server.uri()),
            device_authorization_events_endpoint: format!("{}/events", server.uri()),
            verification_endpoint: format!("{}/verify", server.uri()),
            client_id: "radius-client".into(),
            scope: "openid profile".into(),
            device_code_grant_type: "urn:ietf:params:oauth:grant-type:device_code".into(),
        };
        let pkce = PkceChallenge {
            verifier: "verifier".into(),
            challenge: "challenge".into(),
        };
        let req = build_radius_authorize_request(&oauth, "state-1", &pkce);
        assert!(req.url.contains("response_type=code"));
        assert!(req.url.contains("client_id=radius-client"));
        assert!(
            req.url
                .contains("redirect_uri=http%3A%2F%2F127.0.0.1%3A1456%2Foauth%2Fcallback")
        );
        assert!(req.url.contains("code_challenge=challenge"));
        assert!(req.url.contains("handoff=url"));
        assert!(req.url.contains("state=state-1"));

        let device = request_radius_device_authorization(&oauth).await.unwrap();
        assert_eq!(device.device_code, "dev-1");
        assert_eq!(device.user_code, "USER-1");
        assert_eq!(device.interval, Some(7));
        let body =
            String::from_utf8_lossy(&server.received_requests().await.unwrap()[0].body).to_string();
        assert!(body.contains("client_id=radius-client"));
        assert!(body.contains("scope=openid+profile"));
    }

    #[tokio::test]
    async fn radius_adapter_derives_api_key_and_adds_gateway_catalog_models_without_duplicates() {
        let radius = RadiusOAuth::new("radius.pi.dev/");
        assert_eq!(radius.gateway, "https://radius.pi.dev");
        let auth = radius
            .to_auth(&crate::auth::OAuthCredential {
                access: "access".into(),
                refresh: Some("refresh".into()),
                expires: 999,
                account_id: None,
            })
            .await
            .unwrap();
        assert_eq!(auth.api_key.as_deref(), Some("access"));

        let creds = RadiusOAuthCredentials {
            access: "access".into(),
            refresh: Some("refresh".into()),
            expires: 999,
            scope: None,
            gateway_config: Some(RadiusGatewayConfig {
                base_url: "https://radius/v1".into(),
                models: vec![
                    RadiusGatewayModel {
                        id: "existing".into(),
                        name: "Existing ignored".into(),
                        reasoning: false,
                        thinking_level_map: None,
                        input: vec!["text".into()],
                        cost: ModelCost::default(),
                        context_window: 1,
                        max_tokens: 1,
                    },
                    RadiusGatewayModel {
                        id: "new".into(),
                        name: "New".into(),
                        reasoning: true,
                        thinking_level_map: None,
                        input: vec!["text".into(), "image".into()],
                        cost: ModelCost::default(),
                        context_window: 20,
                        max_tokens: 10,
                    },
                ],
            }),
        };
        let models = radius.modify_models(&[base_model()], "radius", &creds);
        assert_eq!(models.len(), 2);
        let added = models.iter().find(|m| m.id == "new").unwrap();
        assert_eq!(added.api, "pi-messages");
        assert_eq!(added.provider, "radius");
        assert_eq!(added.base_url, "https://radius/v1");
        assert_eq!(added.input, vec!["text", "image"]);
    }
}
