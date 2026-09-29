//! v0.99.1 shared OAuth callback server coverage.

#[cfg(test)]
mod tests {
    use crate::oauth_callback::{OAuthCallbackOptions, start_oauth_callback_server};
    use std::sync::Arc;

    async fn start(
        state: Option<&str>,
        timeout_ms: Option<u64>,
        cancel: Option<tokio::sync::watch::Receiver<bool>>,
    ) -> crate::oauth_callback::OAuthCallbackServer<String> {
        start_oauth_callback_server(OAuthCallbackOptions {
            provider_name: "Example".into(),
            host: "127.0.0.1".into(),
            port: 0,
            path: "/callback".into(),
            redirect_host: None,
            state: state.map(str::to_string),
            complete: Arc::new(|code| Box::pin(async move { Ok(format!("completed:{code}")) })),
            cancel,
            timeout_ms,
        })
        .await
        .unwrap()
    }

    #[tokio::test]
    async fn ignores_stray_requests_and_completes_matching_callback() {
        let server = start(Some("expected"), None, None).await;
        assert!(server.redirect_uri.starts_with("http://127.0.0.1:"));
        let wrong = reqwest::get(server.redirect_uri.replace("/callback", "/other"))
            .await
            .unwrap();
        assert_eq!(wrong.status(), 404);
        let wrong_state = reqwest::get(format!("{}?code=c&state=other", server.redirect_uri))
            .await
            .unwrap();
        assert_eq!(wrong_state.status(), 400);
        let success = reqwest::get(format!(
            "{}?code=the-code&state=expected",
            server.redirect_uri
        ))
        .await
        .unwrap();
        assert_eq!(success.status(), 200);
        assert!(
            success
                .text()
                .await
                .unwrap()
                .contains("Authentication successful")
        );
        assert_eq!(
            server.wait().await.unwrap(),
            Some("completed:the-code".into())
        );
    }

    #[tokio::test]
    async fn provider_errors_timeout_and_cancellation_settle_the_wait() {
        let denied = start(Some("expected"), None, None).await;
        let response = reqwest::get(format!(
            "{}?error=access_denied&error_description=User%20denied&state=expected",
            denied.redirect_uri
        ))
        .await
        .unwrap();
        assert_eq!(response.status(), 400);
        assert!(denied.wait().await.unwrap_err().contains("User denied"));

        let timed_out = start(None, Some(10), None).await;
        assert!(timed_out.wait().await.unwrap_err().contains("timed out"));

        let (tx, rx) = tokio::sync::watch::channel(false);
        let aborted = start(None, None, Some(rx)).await;
        tx.send(true).unwrap();
        assert_eq!(aborted.wait().await.unwrap_err(), "Login cancelled");

        let cancelled = start(None, None, None).await;
        cancelled.cancel();
        assert_eq!(cancelled.wait().await.unwrap(), None);
    }

    #[tokio::test]
    async fn exact_port_conflicts_fail_and_ipv6_redirects_are_bracketed() {
        let blocker = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .unwrap();
        let port = blocker.local_addr().unwrap().port();
        let conflict = start_oauth_callback_server::<String>(OAuthCallbackOptions {
            provider_name: "Example".into(),
            host: "127.0.0.1".into(),
            port,
            path: "/callback".into(),
            redirect_host: None,
            state: None,
            complete: Arc::new(|code| Box::pin(async move { Ok(code) })),
            cancel: None,
            timeout_ms: None,
        })
        .await;
        assert!(conflict.is_err());

        if let Ok(ipv6) = start_oauth_callback_server::<String>(OAuthCallbackOptions {
            provider_name: "Example".into(),
            host: "::1".into(),
            port: 0,
            path: "/callback".into(),
            redirect_host: None,
            state: None,
            complete: Arc::new(|code| Box::pin(async move { Ok(code) })),
            cancel: None,
            timeout_ms: Some(100),
        })
        .await
        {
            assert!(ipv6.redirect_uri.starts_with("http://[::1]:"));
            ipv6.cancel();
        }
    }
}
