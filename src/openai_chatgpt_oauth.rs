//! OpenAI Responses token sharing through Sign in with ChatGPT.

use crate::oauth::{PkceChallenge, generate_pkce};
use rand::RngCore;

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

pub fn parse_callback_url(
    input: &str,
    expected_state: &str,
) -> Result<ChatGptAuthorization, String> {
    let url = url::Url::parse(input.trim())
        .map_err(|_| "Paste the full callback URL from the browser".to_string())?;
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

async fn request_token(
    token_url: &str,
    fields: &[(&str, &str)],
) -> Result<serde_json::Value, String> {
    let response = reqwest::Client::new()
        .post(token_url)
        .form(fields)
        .send()
        .await
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

pub async fn exchange_authorization_code_at(
    token_url: &str,
    authorization: &ChatGptAuthorization,
    verifier: &str,
) -> Result<ChatGptOAuthCredential, String> {
    let token = request_token(
        token_url,
        &[
            ("grant_type", "authorization_code"),
            ("client_id", &authorization.client_id),
            ("code", &authorization.code),
            ("code_verifier", verifier),
            ("redirect_uri", REDIRECT_URI),
            ("resource", RESOURCE),
        ],
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

pub async fn exchange_authorization_code(
    authorization: &ChatGptAuthorization,
    verifier: &str,
) -> Result<ChatGptOAuthCredential, String> {
    exchange_authorization_code_at(TOKEN_URL, authorization, verifier).await
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
