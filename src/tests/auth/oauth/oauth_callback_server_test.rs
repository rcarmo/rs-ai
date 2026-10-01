//! v1.0.0 shared OAuth callback server coverage.

#[cfg(test)]
mod tests {
    use crate::oauth_callback::{OAuthCallbackOptions, start_oauth_callback_server};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

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
    async fn branded_pages_escape_text_redact_completion_errors_and_settle_once() {
        let calls = Arc::new(AtomicUsize::new(0));
        let counted = calls.clone();
        let server = start_oauth_callback_server::<String>(OAuthCallbackOptions {
            provider_name: "Example <unsafe>".into(),
            host: "127.0.0.1".into(),
            port: 0,
            path: "/callback".into(),
            redirect_host: None,
            state: None,
            complete: Arc::new(move |_code| {
                let counted = counted.clone();
                Box::pin(async move {
                    counted.fetch_add(1, Ordering::SeqCst);
                    Err("secret-code token-body".into())
                })
            }),
            cancel: None,
            timeout_ms: None,
        })
        .await
        .unwrap();
        let empty = reqwest::get(format!("{}?code=", server.redirect_uri))
            .await
            .unwrap();
        assert_eq!(empty.status(), 400);
        let failure = reqwest::get(format!("{}?code=one", server.redirect_uri))
            .await
            .unwrap();
        assert_eq!(failure.status(), 502);
        let body = failure.text().await.unwrap();
        for expected in [
            "viewBox=\"0 0 800 800\"",
            "#F09082",
            "#4D9ABF",
            "#F1BE58",
            "Example &lt;unsafe&gt;",
        ] {
            assert!(body.contains(expected), "missing {expected}: {body}");
        }
        assert!(!body.contains("secret-code"));
        assert!(!body.contains("token-body"));
        assert_eq!(server.wait().await.unwrap_err(), "secret-code token-body");
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn success_waits_for_completion_and_response_is_delivered_before_settlement() {
        let (release_tx, release_rx) = tokio::sync::oneshot::channel::<()>();
        let release = Arc::new(tokio::sync::Mutex::new(Some(release_rx)));
        let server = Arc::new(
            start_oauth_callback_server::<String>(OAuthCallbackOptions {
                provider_name: "Example".into(),
                host: "127.0.0.1".into(),
                port: 0,
                path: "/callback".into(),
                redirect_host: None,
                state: None,
                complete: Arc::new(move |code| {
                    let release = release.clone();
                    Box::pin(async move {
                        if let Some(rx) = release.lock().await.take() {
                            let _ = rx.await;
                        }
                        Ok(code)
                    })
                }),
                cancel: None,
                timeout_ms: None,
            })
            .await
            .unwrap(),
        );
        let url = format!("{}?code=immediate", server.redirect_uri);
        let request = tokio::spawn(async move { reqwest::get(url).await.unwrap() });
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        assert!(!request.is_finished(), "success returned before completion");
        release_tx.send(()).unwrap();
        let response = request.await.unwrap();
        assert_eq!(response.status(), 200);
        let body = response.text().await.unwrap();
        assert!(body.contains("Authentication successful"));
        assert!(body.contains("viewBox=\"0 0 800 800\""));
        assert_eq!(server.wait().await.unwrap(), Some("immediate".into()));
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
