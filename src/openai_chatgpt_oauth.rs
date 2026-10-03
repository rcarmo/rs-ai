//! OpenAI Responses token sharing through Sign in with ChatGPT.

use crate::oauth::{PkceChallenge, generate_pkce};
use rand::RngCore;
use std::future::Future;
use tokio::sync::watch;

pub const DYNAMIC_CLIENT_ID: &str = "dynamic_agent_client";
pub const AGENT_NAME_HINT: &str = "Pi";
pub const AUTHORIZE_URL: &str = "https://auth.openai.com/api/accounts/authorize";
pub const TOKEN_URL: &str = "https://auth.openai.com/api/accounts/oauth/token";
pub const RESOURCE: &str = "https://api.openai.com/v1";
pub const REDIRECT_URI: &str = "http://127.0.0.1:1455/auth/callback";
pub const DIRECT_TOKEN_SCOPE: &str = "chatgpt.tokens.use.direct";
pub const SCOPE: &str =
    "openid profile email offline_access resource.invoke chatgpt.tokens.use.direct";
pub const EXPIRY_MARGIN_MS: i64 = 3 * 60 * 1000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatGptOAuthCredential {
    pub access: String,
    pub refresh: String,
    pub expires: i64,
    pub client_id: String,
    pub scopes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatGptAuthorization {
    pub code: String,
    pub client_id: String,
}

#[async_trait::async_trait]
pub trait ChatGptLoginHost: Send + Sync {
    /// Return the installation's stable device UUID.
    async fn device_id(&self) -> Result<String, String>;
    /// Present or open the authorization URL. This is called only after port 1455 binds.
    async fn present_authorization_url(&self, url: &str) -> Result<(), String>;
    /// Wait for a pasted full callback URL and honour the cancellation receiver.
    async fn prompt_callback_url(
        &self,
        prompt: &str,
        cancel: Option<watch::Receiver<bool>>,
    ) -> Result<String, String>;
}

struct ChatGptLoginGuard<T> {
    server: crate::oauth_callback::OAuthCallbackServer<T>,
    prompt_cancel: watch::Sender<bool>,
}

impl<T> Drop for ChatGptLoginGuard<T> {
    fn drop(&mut self) {
        let _ = self.prompt_cancel.send(true);
        self.server.cancel();
    }
}

fn random_value() -> String {
    use base64::Engine;
    let mut bytes = [0_u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

fn is_uuid(value: &str) -> bool {
    let groups = [8, 4, 4, 4, 12];
    let mut parts = value.split('-');
    groups.iter().all(|length| {
        parts.next().is_some_and(|part| {
            part.len() == *length && part.bytes().all(|byte| byte.is_ascii_hexdigit())
        })
    }) && parts.next().is_none()
}

pub fn agent_host_id(device_id: Option<&str>) -> Result<String, String> {
    let value = device_id.filter(|value| is_uuid(value)).ok_or_else(|| {
        "Sign in with ChatGPT requires a device ID (UUID) for this installation".to_string()
    })?;
    Ok(format!("urn:uuid:{}", value.to_ascii_lowercase()))
}

pub fn build_authorize_url(
    device_id: &str,
    pkce: &PkceChallenge,
    state: &str,
    nonce: &str,
) -> Result<String, String> {
    let mut url = url::Url::parse(AUTHORIZE_URL).map_err(|error| error.to_string())?;
    url.query_pairs_mut()
        .append_pair("client_id", DYNAMIC_CLIENT_ID)
        .append_pair("agent_name_hint", AGENT_NAME_HINT)
        .append_pair("ext_agent_host_id", &agent_host_id(Some(device_id))?)
        .append_pair("response_type", "code")
        .append_pair("redirect_uri", REDIRECT_URI)
        .append_pair("resource", RESOURCE)
        .append_pair("scope", SCOPE)
        .append_pair("state", state)
        .append_pair("code_challenge", &pkce.challenge)
        .append_pair("code_challenge_method", "S256")
        .append_pair("nonce", nonce);
    Ok(url.into())
}

pub fn new_authorization_url(
    device_id: &str,
) -> Result<(String, PkceChallenge, String, String), String> {
    let pkce = generate_pkce();
    let state = random_value();
    let nonce = random_value();
    let url = build_authorize_url(device_id, &pkce, &state, &nonce)?;
    Ok((url, pkce, state, nonce))
}

fn parse_callback_parts(
    url: &url::Url,
    expected_state: &str,
) -> Result<ChatGptAuthorization, String> {
    let expected = url::Url::parse(REDIRECT_URI).map_err(|error| error.to_string())?;
    if url.origin() != expected.origin() || url.path() != expected.path() {
        return Err(format!(
            "The pasted callback URL must start with {REDIRECT_URI}"
        ));
    }
    if let Some(error) = url.query_pairs().find(|(key, _)| key == "error") {
        return Err(format!("ChatGPT authorization failed: {}", error.1));
    }
    let code = url
        .query_pairs()
        .find(|(key, _)| key == "code")
        .map(|(_, value)| value.into_owned())
        .ok_or_else(|| "Missing authorization code".to_string())?;
    let state = url
        .query_pairs()
        .find(|(key, _)| key == "state")
        .map(|(_, value)| value.into_owned())
        .ok_or_else(|| "Missing OAuth state".to_string())?;
    if state != expected_state {
        return Err("OAuth state mismatch".into());
    }
    let client_id = url
        .query_pairs()
        .find(|(key, _)| key == "client_id")
        .map(|(_, value)| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            "OpenAI OAuth registration callback did not contain an issued client ID".to_string()
        })?;
    Ok(ChatGptAuthorization { code, client_id })
}

pub fn parse_callback_url(
    input: &str,
    expected_state: &str,
) -> Result<ChatGptAuthorization, String> {
    let url = url::Url::parse(input.trim())
        .map_err(|_| "Paste the full callback URL from the browser".to_string())?;
    parse_callback_parts(&url, expected_state)
}

async fn request_token_with_cancel(
    token_url: &str,
    fields: &[(&str, &str)],
    mut cancel: Option<watch::Receiver<bool>>,
) -> Result<serde_json::Value, String> {
    if cancel.as_ref().is_some_and(|rx| *rx.borrow()) {
        return Err("Login cancelled".into());
    }
    let request = reqwest::Client::new().post(token_url).form(fields).send();
    let response = match cancel.as_mut() {
        Some(rx) => {
            tokio::select! {
                response = request => response,
                changed = rx.changed() => {
                    let _ = changed;
                    return Err("Login cancelled".into());
                }
            }
        }
        None => request.await,
    }
    .map_err(|error| format!("OpenAI OAuth token request failed: {error}"))?;
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(format!(
            "OpenAI OAuth token request failed ({}): {}",
            status.as_u16(),
            body
        ));
    }
    serde_json::from_str(&body)
        .map_err(|_| "OpenAI OAuth token response must be an object".to_string())
}

async fn request_token(
    token_url: &str,
    fields: &[(&str, &str)],
) -> Result<serde_json::Value, String> {
    request_token_with_cancel(token_url, fields, None).await
}

fn credential_from_token_response(
    token: &serde_json::Value,
    client_id: &str,
) -> Result<ChatGptOAuthCredential, String> {
    let string = |field: &str| {
        token
            .get(field)
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .ok_or_else(|| format!("OpenAI OAuth token response has invalid {field}"))
    };
    let access = string("access_token")?;
    let refresh = string("refresh_token")?;
    let scope = string("scope")?;
    let expires_in = token
        .get("expires_in")
        .and_then(serde_json::Value::as_f64)
        .filter(|value| value.is_finite() && *value > 0.0)
        .ok_or_else(|| "OpenAI OAuth token response has invalid expires_in".to_string())?;
    let scopes = scope
        .split_whitespace()
        .map(str::to_string)
        .collect::<Vec<_>>();
    if !scopes.iter().any(|scope| scope == DIRECT_TOKEN_SCOPE) {
        return Err(format!(
            "OpenAI OAuth grant did not include {DIRECT_TOKEN_SCOPE}"
        ));
    }
    Ok(ChatGptOAuthCredential {
        access,
        refresh,
        expires: crate::utils::now_millis() + (expires_in * 1000.0) as i64 - EXPIRY_MARGIN_MS,
        client_id: client_id.to_string(),
        scopes,
    })
}

async fn exchange_authorization_code_at_with_cancel(
    token_url: &str,
    authorization: &ChatGptAuthorization,
    verifier: &str,
    cancel: Option<watch::Receiver<bool>>,
) -> Result<ChatGptOAuthCredential, String> {
    let token = request_token_with_cancel(
        token_url,
        &[
            ("grant_type", "authorization_code"),
            ("client_id", &authorization.client_id),
            ("code", &authorization.code),
            ("code_verifier", verifier),
            ("redirect_uri", REDIRECT_URI),
            ("resource", RESOURCE),
        ],
        cancel,
    )
    .await?;
    if token
        .get("id_token")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .is_none_or(str::is_empty)
    {
        return Err("OpenAI OAuth token response did not contain an ID token".into());
    }
    credential_from_token_response(&token, &authorization.client_id)
}

pub async fn exchange_authorization_code_at(
    token_url: &str,
    authorization: &ChatGptAuthorization,
    verifier: &str,
) -> Result<ChatGptOAuthCredential, String> {
    exchange_authorization_code_at_with_cancel(token_url, authorization, verifier, None).await
}

pub async fn exchange_authorization_code(
    authorization: &ChatGptAuthorization,
    verifier: &str,
) -> Result<ChatGptOAuthCredential, String> {
    exchange_authorization_code_at(TOKEN_URL, authorization, verifier).await
}

async fn cancelable_host_call<F, T>(
    future: F,
    mut cancel: Option<watch::Receiver<bool>>,
) -> Result<T, String>
where
    F: Future<Output = Result<T, String>> + Send,
{
    match cancel.as_mut() {
        Some(rx) => {
            if *rx.borrow() {
                return Err("Login cancelled".into());
            }
            tokio::select! {
                result = future => result,
                changed = rx.changed() => {
                    let _ = changed;
                    Err("Login cancelled".into())
                }
            }
        }
        None => future.await,
    }
}

async fn login_chatgpt_with_host_on_port(
    host: &dyn ChatGptLoginHost,
    token_url: &str,
    port: u16,
    cancel: Option<watch::Receiver<bool>>,
) -> Result<ChatGptOAuthCredential, String> {
    use std::sync::atomic::{AtomicBool, Ordering};

    if cancel.as_ref().is_some_and(|rx| *rx.borrow()) {
        return Err("Login cancelled".into());
    }
    // Generate the flow without invoking the host. The callback port must bind before
    // every host-owned side effect, including device-id lookup.
    let pkce = generate_pkce();
    let state = random_value();
    let nonce = random_value();
    let claimed = std::sync::Arc::new(AtomicBool::new(false));
    let callback_state = state.clone();
    let callback_verifier = pkce.verifier.clone();
    let callback_token_url = token_url.to_string();
    let callback_cancel = cancel.clone();
    let callback_claimed = claimed.clone();
    let complete: crate::oauth_callback::OAuthCompletion<ChatGptOAuthCredential> =
        std::sync::Arc::new(move |input| {
            let state = callback_state.clone();
            let verifier = callback_verifier.clone();
            let token_url = callback_token_url.clone();
            let cancel = callback_cancel.clone();
            let claimed = callback_claimed.clone();
            Box::pin(async move {
                if claimed
                    .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                    .is_err()
                {
                    return Err("Authorization was already handled".into());
                }
                let authorization = parse_callback_url(&input, &state)?;
                exchange_authorization_code_at_with_cancel(
                    &token_url,
                    &authorization,
                    &verifier,
                    cancel,
                )
                .await
            })
        });
    let extractor_state = state.clone();
    let extract_input: crate::oauth_callback::OAuthInputExtractor =
        std::sync::Arc::new(move |callback| {
            let mut canonical = url::Url::parse(REDIRECT_URI).map_err(|error| error.to_string())?;
            canonical.set_query(callback.query());
            parse_callback_parts(&canonical, &extractor_state)?;
            Ok(canonical.into())
        });
    let server = match crate::oauth_callback::start_oauth_callback_server_typed(
        crate::oauth_callback::OAuthCallbackOptions {
            provider_name: "OpenAI ChatGPT".into(),
            host: "127.0.0.1".into(),
            port,
            path: "/auth/callback".into(),
            redirect_host: Some("127.0.0.1".into()),
            state: Some(state.clone()),
            complete,
            cancel: cancel.clone(),
            timeout_ms: None,
        },
        extract_input,
    )
    .await
    {
        Ok(server) => server,
        Err(crate::oauth_callback::OAuthCallbackStartError::Bind(error))
            if error.kind() == std::io::ErrorKind::AddrInUse =>
        {
            return Err("Port 1455 is in use, probably by an unfinished login in another pi session or by the Codex CLI. Cancel that login and try again.".into());
        }
        Err(error) => return Err(error.to_string()),
    };
    let (prompt_cancel_tx, prompt_cancel_rx) = watch::channel(false);
    let guard = ChatGptLoginGuard {
        server,
        prompt_cancel: prompt_cancel_tx,
    };

    let result = async {
        let device_id = cancelable_host_call(host.device_id(), cancel.clone()).await?;
        let url = build_authorize_url(&device_id, &pkce, &state, &nonce)?;
        cancelable_host_call(host.present_authorization_url(&url), cancel.clone()).await?;
        let callback = guard.server.wait();
        // Caller cancellation is owned by the callback server. Its terminal result
        // signals this prompt receiver, then we keep polling the host future until it
        // performs cooperative cleanup. Wrapping this future in an outer cancellation
        // select would drop host code before it observes its receiver.
        let prompt = host.prompt_callback_url(
            "Paste the full callback URL from the browser",
            Some(prompt_cancel_rx),
        );
        tokio::pin!(callback);
        tokio::pin!(prompt);
        tokio::select! {
            callback_result = &mut callback => {
                let _ = guard.prompt_cancel.send(true);
                let _prompt_cleanup = (&mut prompt).await;
                // The callback result is authoritative after it claims the exchange.
                callback_result?.ok_or_else(|| "Login cancelled".to_string())
            }
            manual_result = &mut prompt => {
                let input = manual_result?;
                let authorization = parse_callback_url(&input, &state)?;
                if claimed
                    .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                    .is_err()
                {
                    return callback.await?.ok_or_else(|| "Login cancelled".to_string());
                }
                guard.server.cancel();
                let _ = callback.await;
                exchange_authorization_code_at_with_cancel(
                    token_url,
                    &authorization,
                    &pkce.verifier,
                    cancel.clone(),
                ).await
            }
        }
    }
    .await;

    let _ = guard.prompt_cancel.send(true);
    guard.server.close_and_wait().await;
    result
}

pub async fn login_chatgpt_with_host_at(
    host: &dyn ChatGptLoginHost,
    token_url: &str,
    cancel: Option<watch::Receiver<bool>>,
) -> Result<ChatGptOAuthCredential, String> {
    login_chatgpt_with_host_on_port(host, token_url, 1455, cancel).await
}

#[cfg(test)]
pub(crate) async fn login_chatgpt_with_host_test_port(
    host: &dyn ChatGptLoginHost,
    token_url: &str,
    port: u16,
    cancel: Option<watch::Receiver<bool>>,
) -> Result<ChatGptOAuthCredential, String> {
    login_chatgpt_with_host_on_port(host, token_url, port, cancel).await
}

pub async fn login_chatgpt_with_host(
    host: &dyn ChatGptLoginHost,
    cancel: Option<watch::Receiver<bool>>,
) -> Result<ChatGptOAuthCredential, String> {
    login_chatgpt_with_host_at(host, TOKEN_URL, cancel).await
}

pub async fn refresh_access_token_at(
    token_url: &str,
    credential: &ChatGptOAuthCredential,
) -> Result<ChatGptOAuthCredential, String> {
    if credential.client_id.trim().is_empty() {
        return Err(
            "Stored OpenAI OAuth credential does not contain an issued client ID; reconnect ChatGPT"
                .into(),
        );
    }
    let token = request_token(
        token_url,
        &[
            ("grant_type", "refresh_token"),
            ("client_id", &credential.client_id),
            ("refresh_token", &credential.refresh),
            ("resource", RESOURCE),
        ],
    )
    .await?;
    credential_from_token_response(&token, &credential.client_id)
}

pub async fn refresh_access_token(
    credential: &ChatGptOAuthCredential,
) -> Result<ChatGptOAuthCredential, String> {
    refresh_access_token_at(TOKEN_URL, credential).await
}
