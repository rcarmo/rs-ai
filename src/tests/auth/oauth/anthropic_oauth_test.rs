//! Test-for-test port of upstream `test/anthropic-oauth.test.ts`
//! (`@earendil-works/pi-ai` v0.80.2) — the request-shape cases.
//!
//! The full interactive `loginAnthropic` / `anthropicOAuth.login` orchestration
//! (manual_code prompt, AbortSignal cancellation) is N/A: rs-ai models the token
//! exchange/refresh primitives but not interactive login orchestration (the
//! documented MISSING interactive-OAuth surface). The portable substance is the
//! token endpoint + request body shape, asserted via the received request.

#[cfg(test)]
mod tests {
    use crate::oauth::{
        ANTHROPIC_TOKEN_URL, exchange_anthropic_code_at, refresh_anthropic_token_at,
    };
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    async fn last_request_body(server: &MockServer) -> serde_json::Value {
        let reqs = server.received_requests().await.unwrap();
        let last = reqs.last().expect("a received request");
        serde_json::from_slice(&last.body).expect("json body")
    }

    #[test]
    fn uses_the_platform_claude_com_token_endpoint() {
        // Upstream asserts the production token URL is platform.claude.com.
        assert_eq!(
            ANTHROPIC_TOKEN_URL,
            "https://platform.claude.com/v1/oauth/token"
        );
    }

    #[tokio::test]
    async fn omits_scope_from_refresh_token_requests() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/oauth/token"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"access_token":"new-access-token","refresh_token":"new-refresh-token","expires_in":3600}"#,
            ))
            .mount(&server)
            .await;
        let creds =
            refresh_anthropic_token_at(&format!("{}/oauth/token", server.uri()), "refresh-token")
                .await
                .unwrap();
        assert_eq!(creds.access, "new-access-token");
        assert_eq!(creds.refresh.as_deref(), Some("new-refresh-token"));

        let body = last_request_body(&server).await;
        assert_eq!(body["grant_type"], serde_json::json!("refresh_token"));
        assert_eq!(body["refresh_token"], serde_json::json!("refresh-token"));
        assert!(body["client_id"].as_str().is_some_and(|s| !s.is_empty()));
        assert!(
            body.get("scope").is_none(),
            "refresh must not send `scope`: {body}"
        );
    }

    #[tokio::test]
    async fn keeps_the_localhost_redirect_uri_for_manual_callback_login() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/oauth/token"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"access_token":"access-token","refresh_token":"refresh-token","expires_in":3600}"#,
            ))
            .mount(&server)
            .await;
        let creds = exchange_anthropic_code_at(
            &format!("{}/oauth/token", server.uri()),
            "manual-code",
            "the-state",
            "the-verifier",
            "http://localhost:53692/callback",
        )
        .await
        .unwrap();
        assert_eq!(creds.access, "access-token");
        assert_eq!(creds.refresh.as_deref(), Some("refresh-token"));

        let body = last_request_body(&server).await;
        assert_eq!(body["grant_type"], serde_json::json!("authorization_code"));
        assert_eq!(body["code"], serde_json::json!("manual-code"));
        assert_eq!(
            body["redirect_uri"],
            serde_json::json!("http://localhost:53692/callback")
        );
    }
}
