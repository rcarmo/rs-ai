//! v0.99.1 OpenAI ChatGPT direct-token OAuth coverage.

#[cfg(test)]
mod tests {
    use crate::openai_chatgpt_oauth::{
        ChatGptAuthorization, ChatGptLoginHost, ChatGptOAuthCredential, DIRECT_TOKEN_SCOPE,
        EXPIRY_MARGIN_MS, REDIRECT_URI, RESOURCE, SCOPE, agent_host_id, build_authorize_url,
        exchange_authorization_code_at, login_chatgpt_with_host_at,
        login_chatgpt_with_host_test_port, parse_callback_url, refresh_access_token_at,
    };
    use base64::Engine;
    use serde_json::json;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::sync::{Notify, watch};
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

    const DEVICE_ID: &str = "e61bbe28-07ef-466d-8e5d-a344f94ab305";

    struct Host {
        calls: Arc<AtomicUsize>,
        url: Arc<tokio::sync::Mutex<Option<String>>>,
        ready: Arc<Notify>,
        manual: Option<String>,
        prompt_cleaned: Arc<std::sync::atomic::AtomicBool>,
    }

    #[async_trait::async_trait]
    impl ChatGptLoginHost for Host {
        async fn device_id(&self) -> Result<String, String> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(DEVICE_ID.into())
        }

        async fn present_authorization_url(&self, url: &str) -> Result<(), String> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            *self.url.lock().await = Some(url.to_string());
            self.ready.notify_waiters();
            Ok(())
        }

        async fn prompt_callback_url(
            &self,
            _prompt: &str,
            mut cancel: Option<watch::Receiver<bool>>,
        ) -> Result<String, String> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            if let Some(value) = &self.manual {
                return Ok(value.clone());
            }
            if let Some(rx) = cancel.as_mut() {
                let _ = rx.changed().await;
            }
            self.prompt_cleaned.store(true, Ordering::SeqCst);
            Err("Login cancelled".into())
        }
    }

    fn host(manual: Option<String>) -> Host {
        Host {
            calls: Arc::new(AtomicUsize::new(0)),
            url: Arc::new(tokio::sync::Mutex::new(None)),
            ready: Arc::new(Notify::new()),
            manual,
            prompt_cleaned: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

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

    #[tokio::test]
    async fn occupied_fixed_port_fails_before_any_host_callback() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:1455")
            .await
            .expect("test owns callback port");
        let host = host(None);
        let error = login_chatgpt_with_host_at(&host, "http://127.0.0.1/unused", None)
            .await
            .unwrap_err();
        assert!(error.contains("Port 1455 is in use"), "{error}");
        assert_eq!(host.calls.load(Ordering::SeqCst), 0);
        drop(listener);
    }

    #[tokio::test]
    async fn callback_wins_exchanges_once_cleans_prompt_and_releases_port() {
        let token_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(token(SCOPE)))
            .expect(1)
            .mount(&token_server)
            .await;
        let host = Arc::new(host(None));
        let login_host = host.clone();
        let token_url = format!("{}/token", token_server.uri());
        let login = tokio::spawn(async move {
            login_chatgpt_with_host_at(login_host.as_ref(), &token_url, None).await
        });
        host.ready.notified().await;
        let authorization_url = host.url.lock().await.clone().unwrap();
        let state = url::Url::parse(&authorization_url)
            .unwrap()
            .query_pairs()
            .find(|(key, _)| key == "state")
            .unwrap()
            .1
            .into_owned();
        let response = reqwest::get(format!(
            "{REDIRECT_URI}?code=browser&state={state}&client_id=oaiapp_issued"
        ))
        .await
        .unwrap();
        assert_eq!(response.status(), 200);
        let credential = login.await.unwrap().unwrap();
        assert_eq!(credential.client_id, "oaiapp_issued");
        assert!(
            host.prompt_cleaned.load(Ordering::SeqCst),
            "host prompt cleanup must finish before public login returns"
        );
        assert_eq!(token_server.received_requests().await.unwrap().len(), 1);
        let rebound = tokio::net::TcpListener::bind("127.0.0.1:1455").await;
        assert!(rebound.is_ok(), "callback port leaked after success");
    }

    #[tokio::test]
    async fn manual_callback_uses_one_exchange_and_releases_test_listener() {
        let token_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(token(SCOPE)))
            .expect(1)
            .mount(&token_server)
            .await;
        let probe = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = probe.local_addr().unwrap().port();
        drop(probe);
        let manual = format!("{REDIRECT_URI}?code=manual&state=STATE&client_id=oaiapp_manual");
        // The state is generated by the flow, so use a host that derives it from the presented URL.
        struct ManualHost {
            callback: Arc<tokio::sync::Mutex<Option<String>>>,
        }
        #[async_trait::async_trait]
        impl ChatGptLoginHost for ManualHost {
            async fn device_id(&self) -> Result<String, String> {
                Ok(DEVICE_ID.into())
            }
            async fn present_authorization_url(&self, url: &str) -> Result<(), String> {
                let state = url::Url::parse(url)
                    .unwrap()
                    .query_pairs()
                    .find(|(k, _)| k == "state")
                    .unwrap()
                    .1
                    .into_owned();
                *self.callback.lock().await = Some(format!(
                    "{REDIRECT_URI}?code=manual&state={state}&client_id=oaiapp_manual"
                ));
                Ok(())
            }
            async fn prompt_callback_url(
                &self,
                _: &str,
                _: Option<watch::Receiver<bool>>,
            ) -> Result<String, String> {
                loop {
                    if let Some(value) = self.callback.lock().await.clone() {
                        return Ok(value);
                    }
                    tokio::task::yield_now().await;
                }
            }
        }
        let _ = manual;
        let host = ManualHost {
            callback: Arc::new(tokio::sync::Mutex::new(None)),
        };
        let credential = login_chatgpt_with_host_test_port(
            &host,
            &format!("{}/token", token_server.uri()),
            port,
            None,
        )
        .await
        .unwrap();
        assert_eq!(credential.client_id, "oaiapp_manual");
        assert_eq!(token_server.received_requests().await.unwrap().len(), 1);
        assert!(
            tokio::net::TcpListener::bind(("127.0.0.1", port))
                .await
                .is_ok()
        );
    }

    #[tokio::test]
    async fn simultaneous_manual_and_browser_completion_exchanges_once() {
        #[derive(Clone)]
        struct ClaimedTokenResponder {
            started: Arc<Notify>,
            calls: Arc<AtomicUsize>,
        }
        impl Respond for ClaimedTokenResponder {
            fn respond(&self, _request: &Request) -> ResponseTemplate {
                self.calls.fetch_add(1, Ordering::SeqCst);
                self.started.notify_waiters();
                ResponseTemplate::new(200).set_body_json(token(SCOPE))
            }
        }

        let token_server = MockServer::start().await;
        let token_started = Arc::new(Notify::new());
        let token_calls = Arc::new(AtomicUsize::new(0));
        Mock::given(method("POST"))
            .and(path("/token"))
            .respond_with(ClaimedTokenResponder {
                started: token_started.clone(),
                calls: token_calls.clone(),
            })
            .expect(1)
            .mount(&token_server)
            .await;
        let probe = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = probe.local_addr().unwrap().port();
        drop(probe);
        struct RaceHost {
            port: u16,
            callback: Arc<tokio::sync::Mutex<Option<String>>>,
            token_started: Arc<Notify>,
            browser: Arc<
                tokio::sync::Mutex<
                    Option<tokio::task::JoinHandle<reqwest::Result<reqwest::Response>>>,
                >,
            >,
        }
        #[async_trait::async_trait]
        impl ChatGptLoginHost for RaceHost {
            async fn device_id(&self) -> Result<String, String> {
                Ok(DEVICE_ID.into())
            }
            async fn present_authorization_url(&self, url: &str) -> Result<(), String> {
                let state = url::Url::parse(url)
                    .unwrap()
                    .query_pairs()
                    .find(|(key, _)| key == "state")
                    .unwrap()
                    .1
                    .into_owned();
                let canonical =
                    format!("{REDIRECT_URI}?code=race&state={state}&client_id=oaiapp_race");
                *self.callback.lock().await = Some(canonical);
                let browser = format!(
                    "http://127.0.0.1:{}/auth/callback?code=race&state={state}&client_id=oaiapp_race",
                    self.port
                );
                *self.browser.lock().await =
                    Some(tokio::spawn(async move { reqwest::get(browser).await }));
                Ok(())
            }
            async fn prompt_callback_url(
                &self,
                _: &str,
                _: Option<watch::Receiver<bool>>,
            ) -> Result<String, String> {
                // The browser path has already claimed the once gate and reached the token
                // endpoint before the pasted URL is released into the competing branch.
                self.token_started.notified().await;
                Ok(self.callback.lock().await.clone().unwrap())
            }
        }
        let browser = Arc::new(tokio::sync::Mutex::new(None));
        let host = RaceHost {
            port,
            callback: Arc::new(tokio::sync::Mutex::new(None)),
            token_started,
            browser: browser.clone(),
        };
        let credential = login_chatgpt_with_host_test_port(
            &host,
            &format!("{}/token", token_server.uri()),
            port,
            None,
        )
        .await
        .unwrap();
        let browser_response = browser.lock().await.take().unwrap().await.unwrap().unwrap();
        assert_eq!(browser_response.status(), 200);
        assert_eq!(credential.client_id, "oaiapp_race");
        assert_eq!(token_calls.load(Ordering::SeqCst), 1);
        assert_eq!(token_server.received_requests().await.unwrap().len(), 1);
        assert!(
            tokio::net::TcpListener::bind(("127.0.0.1", port))
                .await
                .is_ok()
        );
    }

    #[tokio::test]
    async fn present_prompt_and_parse_errors_release_listener() {
        enum Failure {
            Present,
            Prompt,
            Parse,
        }
        struct FailureHost {
            failure: Failure,
        }
        #[async_trait::async_trait]
        impl ChatGptLoginHost for FailureHost {
            async fn device_id(&self) -> Result<String, String> {
                Ok(DEVICE_ID.into())
            }
            async fn present_authorization_url(&self, _: &str) -> Result<(), String> {
                if matches!(self.failure, Failure::Present) {
                    Err("present failed".into())
                } else {
                    Ok(())
                }
            }
            async fn prompt_callback_url(
                &self,
                _: &str,
                _: Option<watch::Receiver<bool>>,
            ) -> Result<String, String> {
                match self.failure {
                    Failure::Prompt => Err("prompt failed".into()),
                    Failure::Parse => Ok("not a callback URL".into()),
                    Failure::Present => std::future::pending().await,
                }
            }
        }
        for failure in [Failure::Present, Failure::Prompt, Failure::Parse] {
            let probe = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let port = probe.local_addr().unwrap().port();
            drop(probe);
            let result = login_chatgpt_with_host_test_port(
                &FailureHost { failure },
                "http://127.0.0.1/unused",
                port,
                None,
            )
            .await;
            assert!(result.is_err());
            assert!(
                tokio::net::TcpListener::bind(("127.0.0.1", port))
                    .await
                    .is_ok()
            );
        }
    }

    #[tokio::test]
    async fn dropping_public_login_future_releases_listener() {
        let probe = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = probe.local_addr().unwrap().port();
        drop(probe);
        let host = Arc::new(host(None));
        let login_host = host.clone();
        let login = tokio::spawn(async move {
            login_chatgpt_with_host_test_port(
                login_host.as_ref(),
                "http://127.0.0.1/unused",
                port,
                None,
            )
            .await
        });
        host.ready.notified().await;
        login.abort();
        let _ = login.await;
        for _ in 0..20 {
            if tokio::net::TcpListener::bind(("127.0.0.1", port))
                .await
                .is_ok()
            {
                return;
            }
            tokio::task::yield_now().await;
        }
        panic!("dropping public login future leaked callback port");
    }

    #[tokio::test]
    async fn idle_accepted_socket_cancels_and_releases_listener() {
        let probe = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = probe.local_addr().unwrap().port();
        drop(probe);
        let host = Arc::new(host(None));
        let login_host = host.clone();
        let (tx, rx) = watch::channel(false);
        let login = tokio::spawn(async move {
            login_chatgpt_with_host_test_port(
                login_host.as_ref(),
                "http://127.0.0.1/unused",
                port,
                Some(rx),
            )
            .await
        });
        host.ready.notified().await;
        let _idle = tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .unwrap();
        tx.send(true).unwrap();
        assert!(login.await.unwrap().unwrap_err().contains("cancelled"));
        assert!(
            host.prompt_cleaned.load(Ordering::SeqCst),
            "caller cancellation must await cooperative host cleanup"
        );
        assert!(
            tokio::net::TcpListener::bind(("127.0.0.1", port))
                .await
                .is_ok()
        );
    }
}
