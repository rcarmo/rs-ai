//! v1.0.0 Anthropic OAuth token and host-driven login coverage.
//!
//! Rust already registers the Anthropic refresh/to-auth adapter and shared
//! callback server. These tests cover the portable browser/copy-code
//! orchestration; browser launching and interactive UI remain host-owned.

#[cfg(test)]
mod tests {
    use crate::auth_providers::{AnthropicLoginHost, AnthropicLoginMethod, AnthropicOAuth};
    use crate::oauth::{
        ANTHROPIC_COPY_CODE_REDIRECT_URI, ANTHROPIC_TOKEN_URL, exchange_anthropic_code_at,
        exchange_anthropic_code_at_with_cancel, parse_anthropic_authorization_input,
        refresh_anthropic_token_at,
    };
    use std::sync::{Arc, LazyLock, Mutex};

    static CALLBACK_PORT_LOCK: LazyLock<tokio::sync::Mutex<()>> =
        LazyLock::new(|| tokio::sync::Mutex::new(()));
    use tokio::sync::watch;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    async fn last_request_body(server: &MockServer) -> serde_json::Value {
        let reqs = server.received_requests().await.unwrap();
        let last = reqs.last().expect("a received request");
        serde_json::from_slice(&last.body).expect("json body")
    }

    #[derive(Clone)]
    struct Host {
        method: Option<AnthropicLoginMethod>,
        input: String,
        urls: Arc<Mutex<Vec<String>>>,
    }

    #[async_trait::async_trait]
    impl AnthropicLoginHost for Host {
        async fn select_method(&self) -> Result<Option<AnthropicLoginMethod>, String> {
            Ok(self.method)
        }
        async fn present_authorization_url(&self, url: &str) -> Result<(), String> {
            self.urls.lock().unwrap().push(url.to_string());
            Ok(())
        }
        async fn prompt_authorization_input(
            &self,
            _prompt: &str,
            _cancel: Option<watch::Receiver<bool>>,
        ) -> Result<String, String> {
            Ok(self.input.clone())
        }
    }

    #[test]
    fn exact_endpoints_and_parser_forms_match_v100() {
        assert_eq!(
            ANTHROPIC_TOKEN_URL,
            "https://platform.claude.com/v1/oauth/token"
        );
        assert_eq!(
            ANTHROPIC_COPY_CODE_REDIRECT_URI,
            "https://platform.claude.com/oauth/code/callback"
        );
        for (input, code, state) in [
            ("raw-code", "raw-code", None),
            ("copied-code#the-state", "copied-code", Some("the-state")),
            (
                "?code=query-code&state=query-state",
                "query-code",
                Some("query-state"),
            ),
            (
                "https://localhost/callback?code=url-code&state=url-state",
                "url-code",
                Some("url-state"),
            ),
        ] {
            let parsed = parse_anthropic_authorization_input(input).unwrap();
            assert_eq!(parsed.code, code);
            assert_eq!(parsed.state.as_deref(), state);
        }
        for empty in [
            "https://platform.claude.com/oauth/code/callback?code=",
            "?code=",
            "code=",
            "#state",
            "   ",
        ] {
            let error = parse_anthropic_authorization_input(empty).unwrap_err();
            assert_eq!(error, "Missing authorization code", "{empty:?}");
        }
        assert!(parse_anthropic_authorization_input("https://localhost/callback?state=x").is_err());
    }

    #[tokio::test]
    async fn omits_scope_from_refresh_token_requests() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/oauth/token"))
            .and(header("content-type", "application/json"))
            .and(header("accept", "application/json"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token":"new-access-token","refresh_token":"new-refresh-token","expires_in":3600
            })))
            .mount(&server)
            .await;
        let creds =
            refresh_anthropic_token_at(&format!("{}/oauth/token", server.uri()), "refresh-token")
                .await
                .unwrap();
        assert_eq!(creds.access, "new-access-token");
        let body = last_request_body(&server).await;
        assert_eq!(body["grant_type"], "refresh_token");
        assert_eq!(body["refresh_token"], "refresh-token");
        assert!(body.get("scope").is_none());
    }

    #[tokio::test]
    async fn exact_json_exchange_and_local_redirect_are_preserved() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/oauth/token"))
            .and(header("content-type", "application/json"))
            .and(header("accept", "application/json"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token":"access-token","refresh_token":"refresh-token","expires_in":3600
            })))
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
        let body = last_request_body(&server).await;
        assert_eq!(
            body,
            serde_json::json!({
                "grant_type":"authorization_code",
                "client_id":crate::oauth::ANTHROPIC_CLIENT_ID,
                "code":"manual-code",
                "state":"the-state",
                "redirect_uri":"http://localhost:53692/callback",
                "code_verifier":"the-verifier"
            })
        );
    }

    #[tokio::test]
    async fn copy_code_login_uses_verifier_state_and_copy_redirect() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/oauth/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token":"copy-access","refresh_token":"copy-refresh","expires_in":3600
            })))
            .mount(&server)
            .await;
        let urls = Arc::new(Mutex::new(Vec::new()));
        let host = Host {
            method: Some(AnthropicLoginMethod::CopyCode),
            input: "copied-code".into(),
            urls: urls.clone(),
        };
        let oauth = AnthropicOAuth {
            token_url: Some(format!("{}/oauth/token", server.uri())),
        };
        let credential = oauth.login(&host, None).await.unwrap();
        assert_eq!(credential.access, "copy-access");
        let url = url::Url::parse(&urls.lock().unwrap()[0]).unwrap();
        assert_eq!(url.host_str(), Some("claude.ai"));
        assert_eq!(url.path(), "/oauth/authorize");
        assert_eq!(
            url.query_pairs().find(|(k, _)| k == "code").unwrap().1,
            "true"
        );
        assert_eq!(
            url.query_pairs()
                .find(|(k, _)| k == "redirect_uri")
                .unwrap()
                .1,
            ANTHROPIC_COPY_CODE_REDIRECT_URI
        );
        let verifier = url
            .query_pairs()
            .find(|(k, _)| k == "state")
            .unwrap()
            .1
            .into_owned();
        let body = last_request_body(&server).await;
        assert_eq!(body["state"], verifier);
        assert_eq!(body["code_verifier"], body["state"]);
        assert_eq!(body["redirect_uri"], ANTHROPIC_COPY_CODE_REDIRECT_URI);
    }

    #[tokio::test]
    async fn copy_code_empty_url_and_query_inputs_make_no_request_and_do_not_leak() {
        for input in [
            "https://platform.claude.com/oauth/code/callback?code=",
            "?code=",
        ] {
            let server = MockServer::start().await;
            let oauth = AnthropicOAuth {
                token_url: Some(format!("{}/oauth/token", server.uri())),
            };
            let host = Host {
                method: Some(AnthropicLoginMethod::CopyCode),
                input: input.into(),
                urls: Arc::new(Mutex::new(Vec::new())),
            };
            let error = oauth.login(&host, None).await.unwrap_err();
            assert_eq!(error.message, "Missing authorization code");
            assert!(!error.message.contains(input));
            assert!(!error.message.contains("state"));
            assert!(server.received_requests().await.unwrap().is_empty());
        }
    }

    #[tokio::test]
    async fn selection_cancel_and_state_mismatch_make_no_token_request() {
        let server = MockServer::start().await;
        let oauth = AnthropicOAuth {
            token_url: Some(format!("{}/oauth/token", server.uri())),
        };
        let cancelled = Host {
            method: None,
            input: String::new(),
            urls: Arc::new(Mutex::new(Vec::new())),
        };
        assert!(
            oauth
                .login(&cancelled, None)
                .await
                .unwrap_err()
                .message
                .contains("Login cancelled")
        );
        let mismatch = Host {
            method: Some(AnthropicLoginMethod::CopyCode),
            input: "code#wrong-state".into(),
            urls: Arc::new(Mutex::new(Vec::new())),
        };
        assert!(
            oauth
                .login(&mismatch, None)
                .await
                .unwrap_err()
                .message
                .contains("state mismatch")
        );
        assert_eq!(server.received_requests().await.unwrap().len(), 0);
    }

    struct PendingSelectHost;

    #[async_trait::async_trait]
    impl AnthropicLoginHost for PendingSelectHost {
        async fn select_method(&self) -> Result<Option<AnthropicLoginMethod>, String> {
            std::future::pending().await
        }
        async fn present_authorization_url(&self, _url: &str) -> Result<(), String> {
            Ok(())
        }
        async fn prompt_authorization_input(
            &self,
            _prompt: &str,
            _cancel: Option<watch::Receiver<bool>>,
        ) -> Result<String, String> {
            std::future::pending().await
        }
    }

    #[tokio::test]
    async fn caller_cancellation_interrupts_pending_method_selection() {
        let oauth = Arc::new(AnthropicOAuth::new());
        let host = Arc::new(PendingSelectHost);
        let (tx, rx) = watch::channel(false);
        let task = tokio::spawn(async move { oauth.login(host.as_ref(), Some(rx)).await });
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        tx.send(true).unwrap();
        assert!(
            tokio::time::timeout(std::time::Duration::from_secs(1), task)
                .await
                .unwrap()
                .unwrap()
                .unwrap_err()
                .message
                .contains("aborted")
        );
    }

    struct BrowserHost {
        prompt_cancelled: Arc<tokio::sync::Notify>,
        response: Arc<tokio::sync::Mutex<Option<(u16, String)>>>,
    }

    #[async_trait::async_trait]
    impl AnthropicLoginHost for BrowserHost {
        async fn select_method(&self) -> Result<Option<AnthropicLoginMethod>, String> {
            Ok(Some(AnthropicLoginMethod::Browser))
        }
        async fn present_authorization_url(&self, url: &str) -> Result<(), String> {
            let state = url::Url::parse(url)
                .unwrap()
                .query_pairs()
                .find(|(key, _)| key == "state")
                .unwrap()
                .1
                .into_owned();
            let response = self.response.clone();
            tokio::spawn(async move {
                let result = reqwest::get(format!(
                    "http://127.0.0.1:53692/callback?code=browser-code&state={state}"
                ))
                .await
                .unwrap();
                let status = result.status().as_u16();
                let body = result.text().await.unwrap();
                *response.lock().await = Some((status, body));
            });
            Ok(())
        }
        async fn prompt_authorization_input(
            &self,
            _prompt: &str,
            mut cancel: Option<watch::Receiver<bool>>,
        ) -> Result<String, String> {
            let cancel = cancel.as_mut().expect("per-login prompt cancellation");
            loop {
                if *cancel.borrow() {
                    self.prompt_cancelled.notify_one();
                    return Err("prompt cancelled".into());
                }
                cancel
                    .changed()
                    .await
                    .map_err(|_| "prompt cancel sender dropped".to_string())?;
            }
        }
    }

    struct HostUrlError;

    #[async_trait::async_trait]
    impl AnthropicLoginHost for HostUrlError {
        async fn select_method(&self) -> Result<Option<AnthropicLoginMethod>, String> {
            Ok(Some(AnthropicLoginMethod::Browser))
        }
        async fn present_authorization_url(&self, _url: &str) -> Result<(), String> {
            Err("host launch failed".into())
        }
        async fn prompt_authorization_input(
            &self,
            _prompt: &str,
            _cancel: Option<watch::Receiver<bool>>,
        ) -> Result<String, String> {
            std::future::pending().await
        }
    }

    struct PendingBrowserHost {
        ready: Arc<tokio::sync::Notify>,
    }

    #[async_trait::async_trait]
    impl AnthropicLoginHost for PendingBrowserHost {
        async fn select_method(&self) -> Result<Option<AnthropicLoginMethod>, String> {
            Ok(Some(AnthropicLoginMethod::Browser))
        }
        async fn present_authorization_url(&self, _url: &str) -> Result<(), String> {
            self.ready.notify_one();
            Ok(())
        }
        async fn prompt_authorization_input(
            &self,
            _prompt: &str,
            _cancel: Option<watch::Receiver<bool>>,
        ) -> Result<String, String> {
            std::future::pending().await
        }
    }

    #[tokio::test]
    async fn browser_listener_is_released_on_host_error_and_public_future_drop() {
        let _guard = CALLBACK_PORT_LOCK.lock().await;
        let before = tokio::net::TcpListener::bind("127.0.0.1:53692")
            .await
            .expect("callback port free");
        drop(before);
        let error = AnthropicOAuth::new()
            .login(&HostUrlError, None)
            .await
            .unwrap_err();
        assert!(error.message.contains("host launch failed"));
        tokio::time::sleep(std::time::Duration::from_millis(30)).await;
        drop(
            tokio::net::TcpListener::bind("127.0.0.1:53692")
                .await
                .expect("host error released listener"),
        );

        let ready = Arc::new(tokio::sync::Notify::new());
        let notified = ready.clone();
        let task = tokio::spawn(async move {
            AnthropicOAuth::new()
                .login(&PendingBrowserHost { ready: notified }, None)
                .await
        });
        ready.notified().await;
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        task.abort();
        let _ = task.await;
        tokio::time::sleep(std::time::Duration::from_millis(30)).await;
        drop(
            tokio::net::TcpListener::bind("127.0.0.1:53692")
                .await
                .expect("future drop released listener"),
        );
    }

    #[tokio::test]
    async fn caller_cancel_releases_browser_listener() {
        let _guard = CALLBACK_PORT_LOCK.lock().await;
        let ready = Arc::new(tokio::sync::Notify::new());
        let notified = ready.clone();
        let (tx, rx) = watch::channel(false);
        let task = tokio::spawn(async move {
            AnthropicOAuth::new()
                .login(&PendingBrowserHost { ready: notified }, Some(rx))
                .await
        });
        ready.notified().await;
        tx.send(true).unwrap();
        let message = task
            .await
            .unwrap()
            .unwrap_err()
            .message
            .to_ascii_lowercase();
        assert!(
            message.contains("cancel") || message.contains("aborted"),
            "{message}"
        );
        tokio::time::sleep(std::time::Duration::from_millis(30)).await;
        drop(
            tokio::net::TcpListener::bind("127.0.0.1:53692")
                .await
                .expect("caller cancel released listener"),
        );
    }

    #[tokio::test]
    async fn browser_callback_delivers_page_cancels_prompt_and_exchanges_once() {
        let _guard = CALLBACK_PORT_LOCK.lock().await;
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/oauth/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token":"browser-access","refresh_token":"browser-refresh","expires_in":3600
            })))
            .expect(1)
            .mount(&server)
            .await;
        let prompt_cancelled = Arc::new(tokio::sync::Notify::new());
        let response = Arc::new(tokio::sync::Mutex::new(None));
        let host = BrowserHost {
            prompt_cancelled: prompt_cancelled.clone(),
            response: response.clone(),
        };
        let oauth = AnthropicOAuth {
            token_url: Some(format!("{}/oauth/token", server.uri())),
        };
        let credential = oauth.login(&host, None).await.unwrap();
        assert_eq!(credential.access, "browser-access");
        tokio::time::timeout(
            std::time::Duration::from_secs(1),
            prompt_cancelled.notified(),
        )
        .await
        .unwrap();
        for _ in 0..50 {
            if response.lock().await.is_some() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        let (status, body) = response
            .lock()
            .await
            .clone()
            .expect("delivered callback page");
        assert_eq!(status, 200);
        assert!(body.contains("Authentication successful"));
        assert!(body.contains("viewBox=\"0 0 800 800\""));
        assert_eq!(server.received_requests().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn caller_cancellation_interrupts_delayed_token_post() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_delay(std::time::Duration::from_secs(10))
                    .set_body_json(serde_json::json!({
                        "access_token":"late","refresh_token":"late","expires_in":3600
                    })),
            )
            .mount(&server)
            .await;
        let (tx, rx) = watch::channel(false);
        let url = format!("{}/oauth/token", server.uri());
        let task = tokio::spawn(async move {
            exchange_anthropic_code_at_with_cancel(
                &url,
                "secret-code",
                "state",
                "verifier",
                ANTHROPIC_COPY_CODE_REDIRECT_URI,
                Some(rx),
            )
            .await
        });
        tokio::time::sleep(std::time::Duration::from_millis(30)).await;
        tx.send(true).unwrap();
        assert_eq!(task.await.unwrap().unwrap_err(), "Login cancelled");
    }
}
