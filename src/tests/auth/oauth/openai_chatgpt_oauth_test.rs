//! v0.99.1 OpenAI ChatGPT direct-token OAuth coverage.

#[cfg(test)]
mod tests {
    use crate::openai_chatgpt_oauth::{
        ChatGptAuthorization, ChatGptOAuthCredential, DIRECT_TOKEN_SCOPE, EXPIRY_MARGIN_MS,
        REDIRECT_URI, RESOURCE, SCOPE, agent_host_id, build_authorize_url,
        exchange_authorization_code_at, parse_callback_url, refresh_access_token_at,
    };
    use base64::Engine;
    use serde_json::json;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    const DEVICE_ID: &str = "e61bbe28-07ef-466d-8e5d-a344f94ab305";

    fn token(scope: &str) -> serde_json::Value {
        json!({
            "access_token":"access-token",
            "refresh_token":"refresh-token",
            "expires_in":3600,
            "id_token":"id-token",
            "scope":scope
        })
    }

    #[test]
    fn authorize_url_uses_dynamic_registration_direct_scope_and_uuid_host() {
        let pkce = crate::oauth::PkceChallenge {
            verifier: "verifier".into(),
            challenge: "challenge".into(),
        };
        let value = build_authorize_url(DEVICE_ID, &pkce, "state", "nonce").unwrap();
        let url = url::Url::parse(&value).unwrap();
        assert_eq!(
            url.query_pairs()
                .find(|(key, _)| key == "client_id")
                .unwrap()
                .1,
            "dynamic_agent_client"
        );
        assert_eq!(
            url.query_pairs()
                .find(|(key, _)| key == "agent_name_hint")
                .unwrap()
                .1,
            "Pi"
        );
        assert_eq!(
            url.query_pairs()
                .find(|(key, _)| key == "ext_agent_host_id")
                .unwrap()
                .1,
            format!("urn:uuid:{DEVICE_ID}")
        );
        assert_eq!(
            url.query_pairs().find(|(key, _)| key == "scope").unwrap().1,
            SCOPE
        );
        assert_eq!(
            url.query_pairs()
                .find(|(key, _)| key == "resource")
                .unwrap()
                .1,
            RESOURCE
        );
        assert_eq!(
            url.query_pairs()
                .find(|(key, _)| key == "redirect_uri")
                .unwrap()
                .1,
            REDIRECT_URI
        );
        assert_eq!(
            url.query_pairs()
                .find(|(key, _)| key == "code_challenge_method")
                .unwrap()
                .1,
            "S256"
        );
        assert_eq!(
            agent_host_id(Some("E61BBE28-07EF-466D-8E5D-A344F94AB305")).unwrap(),
            format!("urn:uuid:{DEVICE_ID}")
        );
        assert!(agent_host_id(None).is_err());
        assert!(agent_host_id(Some("not-a-uuid")).is_err());
    }

    #[test]
    fn callback_requires_matching_state_and_issued_client_id() {
        let good = format!("{REDIRECT_URI}?code=code&state=state&client_id=oaiapp_issued");
        assert_eq!(
            parse_callback_url(&good, "state").unwrap(),
            ChatGptAuthorization {
                code: "code".into(),
                client_id: "oaiapp_issued".into()
            }
        );
        assert!(
            parse_callback_url(&good, "other")
                .unwrap_err()
                .contains("state mismatch")
        );
        assert!(
            parse_callback_url(&format!("{REDIRECT_URI}?code=code&state=state"), "state")
                .unwrap_err()
                .contains("issued client ID")
        );
        assert!(
            parse_callback_url("https://example.test/callback?code=x", "state")
                .unwrap_err()
                .contains("must start")
        );
    }

    #[tokio::test]
    async fn exchange_and_refresh_store_issued_client_scopes_and_early_expiry() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(token(SCOPE)))
            .mount(&server)
            .await;
        let before = crate::utils::now_millis();
        let credential = exchange_authorization_code_at(
            &format!("{}/token", server.uri()),
            &ChatGptAuthorization {
                code: "authorization-code".into(),
                client_id: "oaiapp_issued".into(),
            },
            "verifier",
        )
        .await
        .unwrap();
        assert_eq!(credential.client_id, "oaiapp_issued");
        assert!(
            credential
                .scopes
                .iter()
                .any(|scope| scope == DIRECT_TOKEN_SCOPE)
        );
        assert!(credential.expires >= before + 3_600_000 - EXPIRY_MARGIN_MS);
        let requests = server.received_requests().await.unwrap();
        let body = String::from_utf8(requests[0].body.clone()).unwrap();
        assert!(body.contains("client_id=oaiapp_issued"));
        assert!(body.contains("resource=https%3A%2F%2Fapi.openai.com%2Fv1"));

        let refreshed = refresh_access_token_at(&format!("{}/token", server.uri()), &credential)
            .await
            .unwrap();
        assert_eq!(refreshed.refresh, "refresh-token");
        assert_eq!(refreshed.client_id, "oaiapp_issued");
    }

    #[tokio::test]
    async fn rejects_missing_direct_scope_id_token_and_rotated_refresh_token() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(token("openid offline_access")))
            .mount(&server)
            .await;
        let error = exchange_authorization_code_at(
            &server.uri(),
            &ChatGptAuthorization {
                code: "c".into(),
                client_id: "id".into(),
            },
            "v",
        )
        .await
        .unwrap_err();
        assert!(error.contains(DIRECT_TOKEN_SCOPE));

        let no_refresh = MockServer::start().await;
        let mut response = token(SCOPE);
        response.as_object_mut().unwrap().remove("refresh_token");
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(response))
            .mount(&no_refresh)
            .await;
        let credential = ChatGptOAuthCredential {
            access: "old".into(),
            refresh: "refresh".into(),
            expires: 0,
            client_id: "issued".into(),
            scopes: SCOPE.split_whitespace().map(str::to_string).collect(),
        };
        assert!(
            refresh_access_token_at(&no_refresh.uri(), &credential)
                .await
                .unwrap_err()
                .contains("refresh_token")
        );

        // Keep base64 import used as a compile-time reminder that this flow does not decode the ID token.
        assert_eq!(
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(b"id"),
            "aWQ"
        );
    }
}
