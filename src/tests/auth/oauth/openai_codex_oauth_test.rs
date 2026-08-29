//! Test-for-test port of upstream `test/openai-codex-oauth.test.ts`
//! (`@earendil-works/pi-ai` v0.80.2) — the portable `refreshOpenAICodexToken`
//! case.
//!
//! Most of this file exercises the interactive `loginOpenAICodexDeviceCode` /
//! `openaiCodexOAuthProvider.login` device-code orchestration (browser/device
//! method select, timed polling, 15-minute timeout). That interactive login
//! orchestration is N/A — rs-ai models the codex token refresh/exchange
//! primitives but not the login flow (the documented MISSING interactive-OAuth
//! surface). The refresh-failure-error case is portable.

#[cfg(test)]
mod tests {
    use crate::oauth::refresh_codex_token_at;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn token_refresh_failure_includes_status_and_body() {
        // Mirrors "does not write token refresh failures to stderr": a 401 surfaces
        // as `OpenAI Codex token refresh failed (401): <body>` carrying the message.
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/oauth/token"))
            .respond_with(ResponseTemplate::new(401).set_body_string(
                r#"{"error":{"message":"Could not validate your token. Please try signing in again.","type":"invalid_request_error"}}"#,
            ))
            .mount(&server)
            .await;
        let err = refresh_codex_token_at(
            &format!("{}/oauth/token", server.uri()),
            "invalid-refresh-token",
        )
        .await
        .unwrap_err();
        assert!(
            err.contains("OpenAI Codex token refresh failed (401)"),
            "got: {err}"
        );
        assert!(err.contains("Could not validate your token"), "got: {err}");
    }
}
