//! OAuth flows for providers that require interactive authentication.
//!
//! Provides PKCE, device-code, and token-refresh helpers for:
//! - GitHub Copilot
//! - Anthropic
//! - OpenAI Codex

/// PKCE challenge/verifier pair.
pub struct PkceChallenge {
    pub verifier: String,
    pub challenge: String,
}

/// Generate a PKCE challenge pair.
pub fn generate_pkce() -> PkceChallenge {
    use rand::RngCore;

    let mut verifier_bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut verifier_bytes);
    let verifier = base64url_encode(&verifier_bytes);
    let challenge = base64url_encode(&sha256_bytes(verifier.as_bytes()));
    PkceChallenge {
        verifier,
        challenge,
    }
}

/// Device code authorization response.
#[derive(Debug, Clone)]
pub struct DeviceCodeResponse {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u32,
    pub interval: u32,
}

/// OAuth token.
#[derive(Debug, Clone)]
pub struct OAuthToken {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: Option<u32>,
    pub refresh_token: Option<String>,
}

fn sha256_bytes(input: &[u8]) -> Vec<u8> {
    use sha2::{Digest, Sha256};
    Sha256::digest(input).to_vec()
}

fn base64url_encode(input: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(input)
}

/// Anthropic OAuth client id (decoded from the upstream base64 constant).
pub const ANTHROPIC_CLIENT_ID: &str = "9d1c250a-e61b-44d9-88ed-5944d1962f5e";
/// Anthropic OAuth token endpoint.
pub const ANTHROPIC_TOKEN_URL: &str = "https://platform.claude.com/v1/oauth/token";

/// A refreshed OAuth token.
#[derive(Debug, Clone)]
pub struct RefreshedToken {
    pub access: String,
    pub refresh: Option<String>,
    /// Absolute expiry in epoch milliseconds, with a 5-minute safety margin.
    pub expires_at_ms: i64,
}

/// Refresh an Anthropic OAuth token (mirrors refreshAnthropicToken).
pub async fn refresh_anthropic_token(refresh_token: &str) -> Result<RefreshedToken, String> {
    refresh_anthropic_token_at(ANTHROPIC_TOKEN_URL, refresh_token).await
}

/// Refresh against an explicit token endpoint (used for testing).
pub async fn refresh_anthropic_token_at(
    token_url: &str,
    refresh_token: &str,
) -> Result<RefreshedToken, String> {
    let client = reqwest::Client::new();
    let resp = client
        .post(token_url)
        .json(&serde_json::json!({
            "grant_type": "refresh_token",
            "client_id": ANTHROPIC_CLIENT_ID,
            "refresh_token": refresh_token,
        }))
        .send()
        .await
        .map_err(|e| {
            format!("Anthropic token refresh request failed. url={token_url}; details={e}")
        })?;
    let body = resp.text().await.map_err(|e| {
        format!("Anthropic token refresh request failed. url={token_url}; details={e}")
    })?;
    let data: serde_json::Value = serde_json::from_str(&body)
        .map_err(|e| format!("Anthropic token refresh returned invalid JSON. url={token_url}; body={body}; details={e}"))?;
    let access = data
        .get("access_token")
        .and_then(|v| v.as_str())
        .ok_or_else(|| format!("Anthropic token refresh missing access_token. body={body}"))?
        .to_string();
    let refresh = data
        .get("refresh_token")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let expires_in = data.get("expires_in").and_then(|v| v.as_i64()).unwrap_or(0);
    let expires_at_ms = crate::utils::now_millis() + expires_in * 1000 - 5 * 60 * 1000;
    Ok(RefreshedToken {
        access,
        refresh,
        expires_at_ms,
    })
}

/// Anthropic OAuth authorize endpoint.
pub const ANTHROPIC_AUTHORIZE_URL: &str = "https://claude.ai/oauth/authorize";
/// Anthropic OAuth scopes.
pub const ANTHROPIC_SCOPES: &str = "org:create_api_key user:profile user:inference user:sessions:claude_code user:mcp_servers user:file_upload";
/// Default Anthropic OAuth loopback redirect URI.
pub const ANTHROPIC_REDIRECT_URI: &str = "http://localhost:53692/callback";

fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Build the Anthropic OAuth authorization URL (mirrors the login authParams).
pub fn build_anthropic_authorize_url(
    challenge: &str,
    verifier: &str,
    redirect_uri: &str,
) -> String {
    let params = [
        ("code", "true"),
        ("client_id", ANTHROPIC_CLIENT_ID),
        ("response_type", "code"),
        ("redirect_uri", redirect_uri),
        ("scope", ANTHROPIC_SCOPES),
        ("code_challenge", challenge),
        ("code_challenge_method", "S256"),
        ("state", verifier),
    ];
    let query = params
        .iter()
        .map(|(k, v)| format!("{}={}", k, url_encode(v)))
        .collect::<Vec<_>>()
        .join("&");
    format!("{ANTHROPIC_AUTHORIZE_URL}?{query}")
}

/// Exchange an Anthropic authorization code for tokens (mirrors exchangeAuthorizationCode).
pub async fn exchange_anthropic_code(
    code: &str,
    state: &str,
    verifier: &str,
    redirect_uri: &str,
) -> Result<RefreshedToken, String> {
    exchange_anthropic_code_at(ANTHROPIC_TOKEN_URL, code, state, verifier, redirect_uri).await
}

/// Exchange against an explicit token endpoint (used for testing).
pub async fn exchange_anthropic_code_at(
    token_url: &str,
    code: &str,
    state: &str,
    verifier: &str,
    redirect_uri: &str,
) -> Result<RefreshedToken, String> {
    let client = reqwest::Client::new();
    let resp = client
        .post(token_url)
        .json(&serde_json::json!({
            "grant_type": "authorization_code",
            "client_id": ANTHROPIC_CLIENT_ID,
            "code": code,
            "state": state,
            "redirect_uri": redirect_uri,
            "code_verifier": verifier,
        }))
        .send()
        .await
        .map_err(|e| format!("Token exchange request failed. url={token_url}; redirect_uri={redirect_uri}; details={e}"))?;
    let status = resp.status();
    let body = resp
        .text()
        .await
        .map_err(|e| format!("Token exchange request failed. url={token_url}; details={e}"))?;
    if !status.is_success() {
        return Err(format!(
            "HTTP request failed. status={status}; url={token_url}; body={body}"
        ));
    }
    let data: serde_json::Value = serde_json::from_str(&body).map_err(|e| {
        format!("Token exchange returned invalid JSON. url={token_url}; body={body}; details={e}")
    })?;
    let access = data
        .get("access_token")
        .and_then(|v| v.as_str())
        .ok_or_else(|| format!("Token exchange missing access_token. body={body}"))?
        .to_string();
    let refresh = data
        .get("refresh_token")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let expires_in = data.get("expires_in").and_then(|v| v.as_i64()).unwrap_or(0);
    Ok(RefreshedToken {
        access,
        refresh,
        expires_at_ms: crate::utils::now_millis() + expires_in * 1000 - 5 * 60 * 1000,
    })
}

/// OpenAI Codex OAuth client id.
pub const CODEX_CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
/// OpenAI Codex OAuth token endpoint.
pub const CODEX_TOKEN_URL: &str = "https://auth.openai.com/oauth/token";
const CODEX_JWT_CLAIM_PATH: &str = "https://api.openai.com/auth";
/// OpenAI Codex OAuth authorize endpoint.
pub const CODEX_AUTHORIZE_URL: &str = "https://auth.openai.com/oauth/authorize";
/// OpenAI Codex OAuth scope.
pub const CODEX_SCOPE: &str = "openid profile email offline_access";
/// Default OpenAI Codex loopback redirect URI.
pub const CODEX_REDIRECT_URI: &str = "http://localhost:1455/auth/callback";

/// Build the OpenAI Codex authorization URL (mirrors createAuthorizationFlow).
pub fn build_codex_authorize_url(
    challenge: &str,
    state: &str,
    redirect_uri: &str,
    originator: &str,
) -> String {
    let params = [
        ("response_type", "code"),
        ("client_id", CODEX_CLIENT_ID),
        ("redirect_uri", redirect_uri),
        ("scope", CODEX_SCOPE),
        ("code_challenge", challenge),
        ("code_challenge_method", "S256"),
        ("state", state),
        ("id_token_add_organizations", "true"),
        ("codex_cli_simplified_flow", "true"),
        ("originator", originator),
    ];
    let query = params
        .iter()
        .map(|(k, v)| format!("{}={}", k, url_encode(v)))
        .collect::<Vec<_>>()
        .join("&");
    format!("{CODEX_AUTHORIZE_URL}?{query}")
}

/// Exchange an OpenAI Codex authorization code for credentials (mirrors
/// exchangeAuthorizationCodeForCredentials).
pub async fn exchange_codex_code(
    code: &str,
    verifier: &str,
    redirect_uri: &str,
) -> Result<CodexCredentials, String> {
    exchange_codex_code_at(CODEX_TOKEN_URL, code, verifier, redirect_uri).await
}

/// Exchange against an explicit token endpoint (used for testing).
pub async fn exchange_codex_code_at(
    token_url: &str,
    code: &str,
    verifier: &str,
    redirect_uri: &str,
) -> Result<CodexCredentials, String> {
    let client = reqwest::Client::new();
    let resp = client
        .post(token_url)
        .form(&[
            ("grant_type", "authorization_code"),
            ("client_id", CODEX_CLIENT_ID),
            ("code", code),
            ("code_verifier", verifier),
            ("redirect_uri", redirect_uri),
        ])
        .send()
        .await
        .map_err(|e| format!("OpenAI Codex token exchange error: {e}"))?;
    let status = resp.status();
    let body = resp
        .text()
        .await
        .map_err(|e| format!("OpenAI Codex token exchange error: {e}"))?;
    if !status.is_success() {
        return Err(format!(
            "OpenAI Codex token exchange failed ({status}): {body}"
        ));
    }
    let data: serde_json::Value = serde_json::from_str(&body).map_err(|e| {
        format!("OpenAI Codex token exchange invalid JSON: body={body}; details={e}")
    })?;
    let access = data
        .get("access_token")
        .and_then(|v| v.as_str())
        .ok_or_else(|| format!("OpenAI Codex token exchange response missing fields: {body}"))?
        .to_string();
    let refresh = data
        .get("refresh_token")
        .and_then(|v| v.as_str())
        .ok_or_else(|| format!("OpenAI Codex token exchange response missing fields: {body}"))?
        .to_string();
    let expires_in = data
        .get("expires_in")
        .and_then(|v| v.as_i64())
        .ok_or_else(|| format!("OpenAI Codex token exchange response missing fields: {body}"))?;
    let account_id = decode_jwt_payload(&access)
        .and_then(|p| {
            p.get(CODEX_JWT_CLAIM_PATH)
                .and_then(|a| a.get("chatgpt_account_id"))
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
        })
        .ok_or_else(|| "Failed to extract accountId from token".to_string())?;
    Ok(CodexCredentials {
        access,
        refresh: Some(refresh),
        expires_at_ms: crate::utils::now_millis() + expires_in * 1000,
        account_id,
    })
}

/// Refreshed Codex credentials (includes the ChatGPT account id from the JWT).
#[derive(Debug, Clone)]
pub struct CodexCredentials {
    pub access: String,
    pub refresh: Option<String>,
    pub expires_at_ms: i64,
    pub account_id: String,
}

/// Decode a JWT payload (middle segment) into JSON.
fn decode_jwt_payload(token: &str) -> Option<serde_json::Value> {
    use base64::Engine;
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return None;
    }
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(parts[1])
        .or_else(|_| base64::engine::general_purpose::STANDARD.decode(parts[1]))
        .ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// Extract the ChatGPT account id from a Codex OAuth access token's JWT claims.
pub fn codex_account_id(token: &str) -> Option<String> {
    decode_jwt_payload(token).and_then(|p| {
        p.get(CODEX_JWT_CLAIM_PATH)
            .and_then(|a| a.get("chatgpt_account_id"))
            .and_then(|v| v.as_str())
            // Upstream getAccountId returns null for an empty account id.
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
    })
}

/// Refresh an OpenAI Codex OAuth token (mirrors refreshOpenAICodexToken).
pub async fn refresh_codex_token(refresh_token: &str) -> Result<CodexCredentials, String> {
    refresh_codex_token_at(CODEX_TOKEN_URL, refresh_token).await
}

/// Refresh against an explicit token endpoint (used for testing).
pub async fn refresh_codex_token_at(
    token_url: &str,
    refresh_token: &str,
) -> Result<CodexCredentials, String> {
    let client = reqwest::Client::new();
    let resp = client
        .post(token_url)
        .form(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
            ("client_id", CODEX_CLIENT_ID),
        ])
        .send()
        .await
        .map_err(|e| format!("OpenAI Codex token refresh error: {e}"))?;
    let status = resp.status();
    let body = resp
        .text()
        .await
        .map_err(|e| format!("OpenAI Codex token refresh error: {e}"))?;
    if !status.is_success() {
        return Err(format!(
            "OpenAI Codex token refresh failed ({}): {body}",
            status.as_u16()
        ));
    }
    let data: serde_json::Value = serde_json::from_str(&body).map_err(|e| {
        format!("OpenAI Codex token refresh invalid JSON: body={body}; details={e}")
    })?;
    let access = data
        .get("access_token")
        .and_then(|v| v.as_str())
        .ok_or_else(|| format!("OpenAI Codex token refresh response missing fields: {body}"))?
        .to_string();
    let refresh = data
        .get("refresh_token")
        .and_then(|v| v.as_str())
        .ok_or_else(|| format!("OpenAI Codex token refresh response missing fields: {body}"))?
        .to_string();
    let expires_in = data
        .get("expires_in")
        .and_then(|v| v.as_i64())
        .ok_or_else(|| format!("OpenAI Codex token refresh response missing fields: {body}"))?;
    let account_id = decode_jwt_payload(&access)
        .and_then(|p| {
            p.get(CODEX_JWT_CLAIM_PATH)
                .and_then(|a| a.get("chatgpt_account_id"))
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
        })
        .ok_or_else(|| "Failed to extract accountId from token".to_string())?;
    Ok(CodexCredentials {
        access,
        refresh: Some(refresh),
        expires_at_ms: crate::utils::now_millis() + expires_in * 1000,
        account_id,
    })
}

/// Refreshed GitHub Copilot credentials.
#[derive(Debug, Clone)]
pub struct CopilotCredentials {
    pub access: String,
    pub refresh: String,
    pub expires_at_ms: i64,
}

/// The Copilot token-exchange URL for a domain (default github.com).
pub fn copilot_token_url(domain: &str) -> String {
    format!("https://api.{domain}/copilot_internal/v2/token")
}

/// GitHub Copilot OAuth client id (decoded from the upstream base64 constant).
pub const COPILOT_CLIENT_ID: &str = "Iv1.b507a08c87ecfe98";

/// A GitHub device-code grant returned by `start_github_device_flow`.
#[derive(Debug, Clone)]
pub struct DeviceCode {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub interval: Option<u64>,
    pub expires_in: u64,
}

/// Start a GitHub device-code flow (mirrors startDeviceFlow).
pub async fn start_github_device_flow(domain: &str) -> Result<DeviceCode, String> {
    start_github_device_flow_at(
        &format!("https://{domain}/login/device/code"),
        COPILOT_CLIENT_ID,
    )
    .await
}

/// Start against an explicit device-code endpoint (used for testing).
pub async fn start_github_device_flow_at(
    device_code_url: &str,
    client_id: &str,
) -> Result<DeviceCode, String> {
    let client = reqwest::Client::new();
    let resp = client
        .post(device_code_url)
        .header("Accept", "application/json")
        .header("User-Agent", "GitHubCopilotChat/0.35.0")
        .form(&[("client_id", client_id), ("scope", "read:user")])
        .send()
        .await
        .map_err(|e| format!("Device code request failed: {e}"))?;
    let body = resp
        .text()
        .await
        .map_err(|e| format!("Device code request failed: {e}"))?;
    let data: serde_json::Value =
        serde_json::from_str(&body).map_err(|_| "Invalid device code response".to_string())?;
    let device_code = data
        .get("device_code")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Invalid device code response fields".to_string())?
        .to_string();
    let user_code = data
        .get("user_code")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Invalid device code response fields".to_string())?
        .to_string();
    let verification_uri = data
        .get("verification_uri")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Invalid device code response fields".to_string())?
        .to_string();
    let expires_in = data
        .get("expires_in")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| "Invalid device code response fields".to_string())?;
    // Reject non-http(s) verification URIs to avoid opening arbitrary handlers.
    if !(verification_uri.starts_with("https://") || verification_uri.starts_with("http://")) {
        return Err("Untrusted verification_uri in device code response".to_string());
    }
    let interval = data.get("interval").and_then(|v| v.as_u64());
    Ok(DeviceCode {
        device_code,
        user_code,
        verification_uri,
        interval,
        expires_in,
    })
}

/// Result of a single device-code token poll (mirrors the poll callback classification).
#[derive(Debug, Clone, PartialEq)]
pub enum DevicePollStatus {
    Complete(String),
    Pending,
    /// `slow_down`; optional server-provided interval (seconds) from the response (v0.80.5).
    SlowDown(Option<u64>),
    Failed(String),
}

/// Outcome of a single device-code poll for the generic polling loop
/// (mirrors upstream `OAuthDeviceCodePollResult`).
pub enum DevicePollOutcome<T> {
    Pending,
    /// `slow_down`; optional server-provided interval (seconds) overriding the
    /// RFC 8628 §3.5 fixed increment (v0.80.5).
    SlowDown(Option<u64>),
    Failed(String),
    Complete(T),
}

// Mirror upstream device-code.ts constants.
const CANCEL_MESSAGE: &str = "Login cancelled";
const TIMEOUT_MESSAGE: &str = "Device flow timed out";
const SLOW_DOWN_TIMEOUT_MESSAGE: &str = "Device flow timed out after one or more slow_down responses. This is often caused by clock drift in WSL or VM environments. Please sync or restart the VM clock and try again.";
const MINIMUM_INTERVAL_MS: u64 = 1000;
const SLOW_DOWN_INTERVAL_INCREMENT_MS: u64 = 5000;

/// Generic OAuth device-code polling loop (mirrors upstream `pollOAuthDeviceCodeFlow`):
/// poll immediately, then every `interval` until the poll reports complete or the
/// `expires_in_seconds` deadline passes. A `SlowDown` outcome increases the interval
/// by 5s (RFC 8628 §3.5, clamped to a 1s minimum); a `Failed` outcome propagates its
/// message. `cancel` resolving aborts the wait with `"Login cancelled"`. Production
/// callers with no cancellation pass `std::future::pending()`.
pub async fn poll_oauth_device_code_flow<T, P, Fut>(
    interval_seconds: u64,
    expires_in_seconds: u64,
    wait_before_first_poll: bool,
    mut poll: P,
    cancel: impl std::future::Future<Output = ()>,
) -> Result<T, String>
where
    P: FnMut() -> Fut,
    Fut: std::future::Future<Output = DevicePollOutcome<T>>,
{
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(expires_in_seconds);
    let mut interval_ms = std::cmp::max(MINIMUM_INTERVAL_MS, interval_seconds.saturating_mul(1000));
    let mut slow_down_responses: u64 = 0;
    tokio::pin!(cancel);
    // v0.80.5: optionally wait one interval before the first poll (GitHub Copilot).
    if wait_before_first_poll {
        let now = tokio::time::Instant::now();
        if now < deadline {
            let remaining = deadline - now;
            let wait = std::cmp::min(std::time::Duration::from_millis(interval_ms), remaining);
            tokio::select! {
                _ = &mut cancel => return Err(CANCEL_MESSAGE.to_string()),
                _ = tokio::time::sleep(wait) => {}
            }
        }
    }
    loop {
        match poll().await {
            DevicePollOutcome::Complete(v) => return Ok(v),
            DevicePollOutcome::Failed(message) => return Err(message),
            DevicePollOutcome::SlowDown(server_interval) => {
                slow_down_responses += 1;
                // v0.80.5: trust the server-provided interval when present (GitHub reports
                // the new required minimum in `interval`); otherwise apply RFC 8628 §3.5
                // (+5s). Both clamp to a 1s minimum.
                interval_ms = match server_interval {
                    Some(s) if s > 0 => std::cmp::max(MINIMUM_INTERVAL_MS, s.saturating_mul(1000)),
                    _ => std::cmp::max(
                        MINIMUM_INTERVAL_MS,
                        interval_ms + SLOW_DOWN_INTERVAL_INCREMENT_MS,
                    ),
                };
            }
            DevicePollOutcome::Pending => {}
        }
        let now = tokio::time::Instant::now();
        if now >= deadline {
            break;
        }
        let remaining = deadline - now;
        let wait = std::cmp::min(std::time::Duration::from_millis(interval_ms), remaining);
        tokio::select! {
            _ = &mut cancel => return Err(CANCEL_MESSAGE.to_string()),
            _ = tokio::time::sleep(wait) => {}
        }
    }
    Err(if slow_down_responses > 0 {
        SLOW_DOWN_TIMEOUT_MESSAGE
    } else {
        TIMEOUT_MESSAGE
    }
    .to_string())
}

pub const COPILOT_API_VERSION: &str = "2026-06-01";
pub const COPILOT_POLICY_CONCURRENCY: usize = 4;

fn copilot_base_url_from_token(token: &str) -> Option<String> {
    token
        .split(';')
        .find_map(|part| part.trim().strip_prefix("proxy-ep="))
        .filter(|host| !host.is_empty())
        .map(|host| format!("https://{}", host.replacen("proxy.", "api.", 1)))
}

/// Resolve the GitHub Copilot API base URL from the Copilot access token's
/// `proxy-ep` claim, enterprise domain, or the Individual-account default.
pub fn github_copilot_base_url(token: Option<&str>, enterprise_domain: Option<&str>) -> String {
    if let Some(token) = token
        && let Some(base_url) = copilot_base_url_from_token(token)
    {
        return base_url;
    }
    if let Some(domain) = enterprise_domain.map(str::trim).filter(|s| !s.is_empty()) {
        return format!("https://copilot-api.{domain}");
    }
    "https://api.individual.githubcopilot.com".to_string()
}

fn copilot_model_supports_tools(model: &serde_json::Value) -> bool {
    model
        .pointer("/capabilities/supports/tool_calls")
        .and_then(|v| v.as_bool())
        != Some(false)
}

/// Whether a GitHub Copilot `/models` entry is selectable for the model picker
/// (mirrors the strict upstream picker filter): `model_picker_enabled` is true,
/// the model is not policy-disabled, and tool calls are not explicitly disabled.
pub fn is_selectable_copilot_model(model: &serde_json::Value) -> bool {
    let picker_enabled = model
        .get("model_picker_enabled")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let policy_disabled =
        model.pointer("/policy/state").and_then(|v| v.as_str()) == Some("disabled");
    picker_enabled && !policy_disabled && copilot_model_supports_tools(model)
}

/// Parse available Copilot model ids from a `/models` response `data` array.
/// Picker-enabled ids win. When the Individual Copilot endpoint returns no
/// picker ids, fall back to explicitly policy-enabled ids (upstream v0.84.2).
pub fn selectable_copilot_model_ids_with_policy_fallback(
    models_data: &[serde_json::Value],
    allow_policy_fallback: bool,
) -> Vec<String> {
    let mut picker_ids = Vec::new();
    let mut policy_enabled_ids = Vec::new();
    for model in models_data {
        let Some(id) = model.get("id").and_then(|v| v.as_str()) else {
            continue;
        };
        if !copilot_model_supports_tools(model) {
            continue;
        }
        let policy_state = model.pointer("/policy/state").and_then(|v| v.as_str());
        if model.get("model_picker_enabled").and_then(|v| v.as_bool()) == Some(true)
            && policy_state != Some("disabled")
        {
            picker_ids.push(id.to_string());
        }
        if policy_state == Some("enabled") {
            policy_enabled_ids.push(id.to_string());
        }
    }
    if !picker_ids.is_empty() || !allow_policy_fallback {
        picker_ids
    } else {
        policy_enabled_ids
    }
}

/// The strict selectable Copilot model ids from a `/models` response `data` array.
pub fn selectable_copilot_model_ids(models_data: &[serde_json::Value]) -> Vec<String> {
    selectable_copilot_model_ids_with_policy_fallback(models_data, false)
}

fn copilot_policy_batches(model_ids: &[String]) -> Vec<Vec<String>> {
    model_ids
        .chunks(COPILOT_POLICY_CONCURRENCY)
        .map(|chunk| chunk.to_vec())
        .collect()
}

/// Fetch available GitHub Copilot model ids from the resolved Copilot API.
pub async fn fetch_available_github_copilot_model_ids_at(
    base_url: &str,
    copilot_token: &str,
) -> Result<Vec<String>, String> {
    let url = format!("{}/models", base_url.trim_end_matches('/'));
    let client = reqwest::Client::new();
    let mut req = client
        .get(&url)
        .header("Accept", "application/json")
        .header("Authorization", format!("Bearer {copilot_token}"))
        .header("X-GitHub-Api-Version", COPILOT_API_VERSION);
    for (k, v) in crate::utils::copilot_headers() {
        req = req.header(k, v);
    }
    let resp = req
        .send()
        .await
        .map_err(|e| format!("Copilot models request failed: {e}"))?;
    let status = resp.status();
    let body = resp
        .text()
        .await
        .map_err(|e| format!("Copilot models request failed: {e}"))?;
    if !status.is_success() {
        return Err(format!(
            "{} {}: {}",
            status.as_u16(),
            status.canonical_reason().unwrap_or(""),
            body
        ));
    }
    let raw: serde_json::Value =
        serde_json::from_str(&body).map_err(|_| "Invalid Copilot models response".to_string())?;
    let data = raw
        .get("data")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "Invalid Copilot models response".to_string())?;
    Ok(selectable_copilot_model_ids_with_policy_fallback(
        data,
        base_url.trim_end_matches('/') == "https://api.individual.githubcopilot.com",
    ))
}

pub async fn fetch_available_github_copilot_model_ids(
    copilot_token: &str,
    enterprise_domain: Option<&str>,
) -> Result<Vec<String>, String> {
    let base_url = github_copilot_base_url(Some(copilot_token), enterprise_domain);
    fetch_available_github_copilot_model_ids_at(&base_url, copilot_token).await
}

async fn enable_github_copilot_model_at(
    client: &reqwest::Client,
    base_url: &str,
    token: &str,
    model_id: &str,
) -> bool {
    let url = format!(
        "{}/models/{}/policy",
        base_url.trim_end_matches('/'),
        model_id
    );
    let mut req = client
        .post(&url)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {token}"))
        .header("openai-intent", "chat-policy")
        .header("x-interaction-type", "chat-policy")
        .json(&serde_json::json!({"state": "enabled"}));
    for (k, v) in crate::utils::copilot_headers() {
        req = req.header(k, v);
    }
    req.send()
        .await
        .is_ok_and(|resp| resp.status().is_success())
}

/// Enable all requested Copilot model policies in bounded batches. Individual
/// failures are ignored to match upstream's best-effort policy acceptance.
pub async fn enable_github_copilot_models_at(
    base_url: &str,
    token: &str,
    model_ids: &[String],
) -> Result<(), String> {
    let client = reqwest::Client::new();
    for batch in copilot_policy_batches(model_ids) {
        let futures = batch
            .iter()
            .map(|id| enable_github_copilot_model_at(&client, base_url, token, id));
        futures::future::join_all(futures).await;
    }
    Ok(())
}

/// Enable all built-in GitHub Copilot model policies for a freshly logged-in account.
pub async fn enable_all_github_copilot_models(
    token: &str,
    enterprise_domain: Option<&str>,
) -> Result<(), String> {
    let ids = crate::registry::list_models(Some(crate::types::provider_id::GITHUB_COPILOT))
        .into_iter()
        .map(|model| model.id)
        .collect::<Vec<_>>();
    let base_url = github_copilot_base_url(Some(token), enterprise_domain);
    enable_github_copilot_models_at(&base_url, token, &ids).await
}

/// Poll once for a GitHub device-code access token (mirrors pollForGitHubAccessToken's poll).
pub async fn poll_github_device_token(domain: &str, device_code: &str) -> DevicePollStatus {
    poll_github_device_token_at(
        &format!("https://{domain}/login/oauth/access_token"),
        COPILOT_CLIENT_ID,
        device_code,
    )
    .await
}

/// Poll against an explicit access-token endpoint (used for testing).
pub async fn poll_github_device_token_at(
    access_token_url: &str,
    client_id: &str,
    device_code: &str,
) -> DevicePollStatus {
    let client = reqwest::Client::new();
    let resp = client
        .post(access_token_url)
        .header("Accept", "application/json")
        .header("User-Agent", "GitHubCopilotChat/0.35.0")
        .form(&[
            ("client_id", client_id),
            ("device_code", device_code),
            ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
        ])
        .send()
        .await;
    let body = match resp {
        Ok(r) => r.text().await.unwrap_or_default(),
        Err(e) => return DevicePollStatus::Failed(format!("Device flow failed: {e}")),
    };
    let data: serde_json::Value = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(_) => return DevicePollStatus::Failed("Invalid device token response".to_string()),
    };
    if let Some(token) = data.get("access_token").and_then(|v| v.as_str()) {
        return DevicePollStatus::Complete(token.to_string());
    }
    if let Some(error) = data.get("error").and_then(|v| v.as_str()) {
        return match error {
            "authorization_pending" => DevicePollStatus::Pending,
            "slow_down" => {
                DevicePollStatus::SlowDown(data.get("interval").and_then(|v| v.as_u64()))
            }
            other => {
                let desc = data
                    .get("error_description")
                    .and_then(|v| v.as_str())
                    .map(|d| format!(": {d}"))
                    .unwrap_or_default();
                DevicePollStatus::Failed(format!("Device flow failed: {other}{desc}"))
            }
        };
    }
    DevicePollStatus::Failed("Invalid device token response".to_string())
}

/// Refresh a GitHub Copilot token (mirrors refreshGitHubCopilotToken).
pub async fn refresh_copilot_token(
    refresh_token: &str,
    enterprise_domain: Option<&str>,
) -> Result<CopilotCredentials, String> {
    let domain = enterprise_domain.unwrap_or("github.com");
    refresh_copilot_token_at(&copilot_token_url(domain), refresh_token).await
}

/// Refresh against an explicit token endpoint (used for testing).
pub async fn refresh_copilot_token_at(
    token_url: &str,
    refresh_token: &str,
) -> Result<CopilotCredentials, String> {
    let client = reqwest::Client::new();
    let mut req = client
        .get(token_url)
        .header("Accept", "application/json")
        .header("Authorization", format!("Bearer {refresh_token}"));
    for (k, v) in crate::utils::copilot_headers() {
        req = req.header(k, v);
    }
    let resp = req
        .send()
        .await
        .map_err(|e| format!("Copilot token refresh error: {e}"))?;
    let body = resp
        .text()
        .await
        .map_err(|e| format!("Copilot token refresh error: {e}"))?;
    let data: serde_json::Value =
        serde_json::from_str(&body).map_err(|_| "Invalid Copilot token response".to_string())?;
    let token = data
        .get("token")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Invalid Copilot token response fields".to_string())?;
    let expires_at = data
        .get("expires_at")
        .and_then(|v| v.as_i64())
        .ok_or_else(|| "Invalid Copilot token response fields".to_string())?;
    Ok(CopilotCredentials {
        access: token.to_string(),
        refresh: refresh_token.to_string(),
        expires_at_ms: expires_at * 1000 - 5 * 60 * 1000,
    })
}

pub const OPENROUTER_AUTH_URL: &str = "https://openrouter.ai/auth";
pub const OPENROUTER_TOKEN_URL: &str = "https://openrouter.ai/api/v1/auth/keys";

pub async fn exchange_openrouter_code_at(
    token_url: &str,
    code: &str,
    verifier: &str,
) -> Result<crate::auth::OAuthCredential, String> {
    let response = crate::http_proxy::client_for_target(token_url, None)
        .post(token_url)
        .header("accept", "application/json")
        .header("content-type", "application/json")
        .json(&serde_json::json!({"code": code, "code_verifier": verifier, "code_challenge_method": "S256"}))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let status = response.status();
    let body = response
        .json::<serde_json::Value>()
        .await
        .unwrap_or_else(|_| serde_json::json!({}));
    if !status.is_success() {
        let detail = body
            .get("error_description")
            .or_else(|| body.get("message"))
            .or_else(|| body.get("error"))
            .and_then(|v| v.as_str())
            .or_else(|| body.pointer("/error/message").and_then(|v| v.as_str()));
        return Err(format!(
            "OpenRouter OAuth key exchange failed (HTTP {}){}",
            status.as_u16(),
            detail.map(|d| format!(": {d}")).unwrap_or_default()
        ));
    }
    let key = body
        .get("key")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "OpenRouter OAuth response carries no \"key\"".to_string())?;
    Ok(crate::auth::OAuthCredential {
        access: key.to_string(),
        refresh: Some(String::new()),
        expires: i64::MAX,
        account_id: None,
    })
}

pub async fn exchange_openrouter_code(
    code: &str,
    verifier: &str,
) -> Result<crate::auth::OAuthCredential, String> {
    exchange_openrouter_code_at(OPENROUTER_TOKEN_URL, code, verifier).await
}

pub const KIMI_CODE_CLIENT_ID: &str = "17e5f671-d194-4dfb-9706-5516cb48c098";
pub const KIMI_CODE_OAUTH_HOST: &str = "https://auth.kimi.com";
const KIMI_CODE_DEFAULT_POLL_INTERVAL_SECONDS: u64 = 5;
const KIMI_CODE_DEVICE_TIMEOUT_SECONDS: u64 = 15 * 60;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KimiDeviceAuthorization {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub verification_uri_complete: String,
    pub interval_seconds: u64,
    pub expires_in_seconds: u64,
}

fn trusted_http_url(value: &str) -> Option<String> {
    reqwest::Url::parse(value)
        .ok()
        .filter(|u| matches!(u.scheme(), "http" | "https"))
        .map(|u| u.to_string())
}

fn kimi_host(host: &str) -> String {
    host.trim_end_matches('/').to_string()
}

pub async fn request_kimi_device_authorization_at(
    host: &str,
) -> Result<KimiDeviceAuthorization, String> {
    let host = kimi_host(host);
    let url = format!("{host}/api/oauth/device_authorization");
    let response = crate::http_proxy::client_for_target(&url, None)
        .post(&url)
        .header("accept", "application/json")
        .form(&[("client_id", KIMI_CODE_CLIENT_ID)])
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!(
            "Kimi Code device authorization failed with status {}",
            status.as_u16()
        ));
    }
    let json = response
        .json::<serde_json::Value>()
        .await
        .map_err(|e| e.to_string())?;
    let s = |k: &str| {
        json.get(k)
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    };
    let device_code = s("device_code")
        .ok_or_else(|| format!("Invalid Kimi Code device authorization response: {json}"))?;
    let user_code = s("user_code")
        .ok_or_else(|| format!("Invalid Kimi Code device authorization response: {json}"))?;
    let verification_uri = s("verification_uri")
        .and_then(|v| trusted_http_url(&v))
        .ok_or_else(|| format!("Invalid Kimi Code device authorization response: {json}"))?;
    let verification_uri_complete = s("verification_uri_complete")
        .and_then(|v| trusted_http_url(&v))
        .ok_or_else(|| format!("Invalid Kimi Code device authorization response: {json}"))?;
    Ok(KimiDeviceAuthorization {
        device_code,
        user_code,
        verification_uri,
        verification_uri_complete,
        interval_seconds: json
            .get("interval")
            .and_then(|v| v.as_u64())
            .filter(|n| *n > 0)
            .unwrap_or(KIMI_CODE_DEFAULT_POLL_INTERVAL_SECONDS),
        expires_in_seconds: json
            .get("expires_in")
            .and_then(|v| v.as_u64())
            .filter(|n| *n > 0)
            .unwrap_or(KIMI_CODE_DEVICE_TIMEOUT_SECONDS),
    })
}

fn kimi_credential_from_json(
    json: &serde_json::Value,
    operation: &str,
) -> Result<crate::auth::OAuthCredential, String> {
    let access = json
        .get("access_token")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!("Kimi Code token {operation} response missing fields: {json}"))?;
    let refresh = json
        .get("refresh_token")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!("Kimi Code token {operation} response missing fields: {json}"))?;
    let expires_in = json
        .get("expires_in")
        .and_then(|v| v.as_i64())
        .filter(|n| *n > 0)
        .ok_or_else(|| format!("Kimi Code token {operation} response missing fields: {json}"))?;
    Ok(crate::auth::OAuthCredential {
        access: access.into(),
        refresh: Some(refresh.into()),
        expires: crate::utils::now_millis() + expires_in * 1000,
        account_id: None,
    })
}

pub async fn refresh_kimi_code_token_at(
    host: &str,
    refresh_token: &str,
) -> Result<crate::auth::OAuthCredential, String> {
    let host = kimi_host(host);
    let url = format!("{host}/api/oauth/token");
    let mut last_err = None;
    for attempt in 0..=3 {
        if attempt > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(
                1000 * 2_u64.pow(attempt - 1),
            ))
            .await;
        }
        let response = crate::http_proxy::client_for_target(&url, None)
            .post(&url)
            .header("accept", "application/json")
            .form(&[
                ("client_id", KIMI_CODE_CLIENT_ID),
                ("grant_type", "refresh_token"),
                ("refresh_token", refresh_token),
            ])
            .send()
            .await
            .map_err(|e| e.to_string())?;
        let status = response.status();
        let json = response
            .json::<serde_json::Value>()
            .await
            .unwrap_or_else(|_| serde_json::json!({}));
        if status.is_success() {
            return kimi_credential_from_json(&json, "refresh");
        }
        last_err = Some(format!(
            "Kimi Code token refresh failed with status {}",
            status.as_u16()
        ));
        if status.as_u16() != 429 && status.as_u16() < 500 {
            break;
        }
    }
    Err(last_err.unwrap_or_else(|| "Kimi Code token refresh failed".into()))
}

pub async fn login_kimi_code_device_at(
    host: &str,
    cancel: impl std::future::Future<Output = ()>,
) -> Result<crate::auth::OAuthCredential, String> {
    let host = kimi_host(host);
    let device = request_kimi_device_authorization_at(&host).await?;
    let url = format!("{host}/api/oauth/token");
    poll_oauth_device_code_flow(
        device.interval_seconds,
        device.expires_in_seconds,
        true,
        || async {
            let response = match crate::http_proxy::client_for_target(&url, None)
                .post(&url)
                .header("accept", "application/json")
                .form(&[
                    ("client_id", KIMI_CODE_CLIENT_ID),
                    ("device_code", device.device_code.as_str()),
                    ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ])
                .send()
                .await
            {
                Ok(r) => r,
                Err(e) => return DevicePollOutcome::Failed(e.to_string()),
            };
            let status = response.status();
            let json = response
                .json::<serde_json::Value>()
                .await
                .unwrap_or_else(|_| serde_json::json!({}));
            if status.is_success() {
                return match kimi_credential_from_json(&json, "poll") {
                    Ok(c) => DevicePollOutcome::Complete(c),
                    Err(e) => DevicePollOutcome::Failed(e),
                };
            }
            match json.get("error").and_then(|v| v.as_str()) {
                Some("authorization_pending") => DevicePollOutcome::Pending,
                Some("slow_down") => {
                    DevicePollOutcome::SlowDown(json.get("interval").and_then(|v| v.as_u64()))
                }
                Some("expired_token") => DevicePollOutcome::Failed(
                    "Kimi Code device authorization expired. Please restart login.".into(),
                ),
                Some("access_denied") => {
                    DevicePollOutcome::Failed("Kimi Code login was denied.".into())
                }
                other => DevicePollOutcome::Failed(format!(
                    "Kimi Code device token request failed (status {}){}",
                    status.as_u16(),
                    other.map(|e| format!(": {e}")).unwrap_or_default()
                )),
            }
        },
        cancel,
    )
    .await
}

pub const XAI_CLIENT_ID: &str = "b1a00492-073a-47ea-816f-4c329264a828";
pub const XAI_SCOPE: &str = "openid profile email offline_access grok-cli:access api:access";
pub const XAI_DEVICE_CODE_URL: &str = "https://auth.x.ai/oauth2/device/code";
pub const XAI_TOKEN_URL: &str = "https://auth.x.ai/oauth2/token";
const XAI_REFRESH_SKEW_MS: i64 = 5 * 60 * 1000;
const XAI_DEFAULT_TOKEN_LIFETIME_SECONDS: i64 = 3600;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XaiDeviceCode {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub verification_uri_complete: Option<String>,
    pub interval_seconds: Option<u64>,
    pub expires_in_seconds: u64,
}

impl XaiDeviceCode {
    pub fn preferred_verification_uri(&self) -> &str {
        self.verification_uri_complete
            .as_deref()
            .unwrap_or(&self.verification_uri)
    }
}

fn xai_required_string(v: &serde_json::Value, field: &str) -> Result<String, String> {
    v.get(field)
        .and_then(|x| x.as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .ok_or_else(|| format!("Invalid xAI OAuth response field: {field}"))
}

fn xai_positive_i64(v: &serde_json::Value, field: &str) -> Result<i64, String> {
    v.get(field)
        .and_then(|x| x.as_i64())
        .filter(|n| *n > 0)
        .ok_or_else(|| format!("Invalid xAI OAuth response field: {field}"))
}

fn validate_xai_verification_uri(raw: &str) -> Result<String, String> {
    let url = reqwest::Url::parse(raw)
        .map_err(|_| "Untrusted verification URI in xAI OAuth response".to_string())?;
    if url.scheme() != "https" {
        return Err("Untrusted verification URI in xAI OAuth response".to_string());
    }
    Ok(url.to_string())
}

async fn post_xai_form(
    url: &str,
    fields: &[(&str, &str)],
) -> Result<(reqwest::StatusCode, serde_json::Value), String> {
    let response = crate::http_proxy::client_for_target(url, None)
        .post(url)
        .header("accept", "application/json")
        .form(fields)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let status = response.status();
    let body = response
        .json::<serde_json::Value>()
        .await
        .map_err(|_| format!("xAI OAuth returned invalid JSON (HTTP {})", status.as_u16()))?;
    Ok((status, body))
}

fn xai_request_failure(
    action: &str,
    status: reqwest::StatusCode,
    body: &serde_json::Value,
) -> String {
    let error = body.get("error").and_then(|x| x.as_str());
    let desc = body.get("error_description").and_then(|x| x.as_str());
    let detail = match (error, desc) {
        (Some(e), Some(d)) => format!(": {e}: {d}"),
        (Some(e), None) => format!(": {e}"),
        (None, Some(d)) => format!(": {d}"),
        _ => String::new(),
    };
    format!(
        "xAI OAuth {action} failed (HTTP {}){detail}",
        status.as_u16()
    )
}

pub async fn request_xai_device_code_at(device_url: &str) -> Result<XaiDeviceCode, String> {
    let (status, body) = post_xai_form(
        device_url,
        &[
            ("client_id", XAI_CLIENT_ID),
            ("scope", XAI_SCOPE),
            ("referrer", "pi"),
        ],
    )
    .await?;
    if !status.is_success() {
        return Err(xai_request_failure("device authorization", status, &body));
    }
    let interval_seconds = body
        .get("interval")
        .and_then(|x| x.as_u64())
        .filter(|n| *n > 0);
    let verification_uri_complete = match body
        .get("verification_uri_complete")
        .and_then(|x| x.as_str())
    {
        Some(raw) if !raw.is_empty() => Some(validate_xai_verification_uri(raw)?),
        _ => None,
    };
    Ok(XaiDeviceCode {
        device_code: xai_required_string(&body, "device_code")?,
        user_code: xai_required_string(&body, "user_code")?,
        verification_uri: validate_xai_verification_uri(&xai_required_string(
            &body,
            "verification_uri",
        )?)?,
        verification_uri_complete,
        interval_seconds,
        expires_in_seconds: xai_positive_i64(&body, "expires_in")? as u64,
    })
}

pub async fn request_xai_device_code() -> Result<XaiDeviceCode, String> {
    request_xai_device_code_at(XAI_DEVICE_CODE_URL).await
}

fn xai_credentials_from_token_response(
    body: &serde_json::Value,
    previous_refresh: Option<&str>,
) -> Result<crate::auth::OAuthCredential, String> {
    let access = xai_required_string(body, "access_token")?;
    let refresh = match body.get("refresh_token").and_then(|x| x.as_str()) {
        Some(r) if !r.is_empty() => r.to_string(),
        _ => previous_refresh
            .ok_or_else(|| "Invalid xAI OAuth response field: refresh_token".to_string())?
            .to_string(),
    };
    let expires = body
        .get("expires_in")
        .and_then(|x| x.as_i64())
        .filter(|n| *n > 0)
        .unwrap_or(XAI_DEFAULT_TOKEN_LIFETIME_SECONDS);
    Ok(crate::auth::OAuthCredential {
        access,
        refresh: Some(refresh),
        expires: crate::utils::now_millis() + expires * 1000 - XAI_REFRESH_SKEW_MS,
        account_id: None,
    })
}

pub async fn refresh_xai_token_at(
    token_url: &str,
    refresh_token: &str,
) -> Result<crate::auth::OAuthCredential, String> {
    let (status, body) = post_xai_form(
        token_url,
        &[
            ("grant_type", "refresh_token"),
            ("client_id", XAI_CLIENT_ID),
            ("refresh_token", refresh_token),
        ],
    )
    .await?;
    if !status.is_success() {
        return Err(xai_request_failure("token refresh", status, &body));
    }
    xai_credentials_from_token_response(&body, Some(refresh_token))
}

pub async fn refresh_xai_token(
    refresh_token: &str,
) -> Result<crate::auth::OAuthCredential, String> {
    refresh_xai_token_at(XAI_TOKEN_URL, refresh_token).await
}

pub async fn login_xai_device_code_at(
    device_url: &str,
    token_url: &str,
    cancel: impl std::future::Future<Output = ()>,
) -> Result<crate::auth::OAuthCredential, String> {
    let device = request_xai_device_code_at(device_url).await?;
    poll_oauth_device_code_flow(
        device.interval_seconds.unwrap_or(5),
        device.expires_in_seconds,
        true,
        || async {
            let (status, body) = match post_xai_form(
                token_url,
                &[
                    ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                    ("client_id", XAI_CLIENT_ID),
                    ("device_code", device.device_code.as_str()),
                ],
            )
            .await
            {
                Ok(v) => v,
                Err(e) => return DevicePollOutcome::Failed(e),
            };
            if status.is_success() {
                return match xai_credentials_from_token_response(&body, None) {
                    Ok(c) => DevicePollOutcome::Complete(c),
                    Err(e) => DevicePollOutcome::Failed(e),
                };
            }
            match body.get("error").and_then(|x| x.as_str()) {
                Some("authorization_pending") => DevicePollOutcome::Pending,
                Some("slow_down") => {
                    DevicePollOutcome::SlowDown(body.get("interval").and_then(|x| x.as_u64()))
                }
                Some("access_denied") | Some("authorization_denied") => {
                    DevicePollOutcome::Failed("xAI device authorization was denied".into())
                }
                Some("expired_token") => {
                    DevicePollOutcome::Failed("xAI device code expired".into())
                }
                _ => DevicePollOutcome::Failed(xai_request_failure(
                    "device token polling",
                    status,
                    &body,
                )),
            }
        },
        cancel,
    )
    .await
}

pub async fn login_xai_device_code() -> Result<crate::auth::OAuthCredential, String> {
    login_xai_device_code_at(
        XAI_DEVICE_CODE_URL,
        XAI_TOKEN_URL,
        std::future::pending::<()>(),
    )
    .await
}

pub const DEFAULT_RADIUS_GATEWAY: &str = "https://radius.pi.dev";
pub const RADIUS_REDIRECT_URI: &str = "http://127.0.0.1:1456/oauth/callback";
const RADIUS_TOKEN_EXPIRY_SKEW_MS: i64 = 60_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadiusOAuthConfig {
    pub issuer: String,
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    pub device_authorization_endpoint: String,
    pub device_authorization_events_endpoint: String,
    pub verification_endpoint: String,
    pub client_id: String,
    pub scope: String,
    pub device_code_grant_type: String,
}

#[derive(Debug, Clone)]
pub struct RadiusGatewayModel {
    pub id: String,
    pub name: String,
    pub reasoning: bool,
    pub thinking_level_map: Option<std::collections::HashMap<String, Option<String>>>,
    pub input: Vec<String>,
    pub cost: crate::types::ModelCost,
    pub context_window: u32,
    pub max_tokens: u32,
}

#[derive(Debug, Clone)]
pub struct RadiusGatewayConfig {
    pub base_url: String,
    pub models: Vec<RadiusGatewayModel>,
}

#[derive(Debug, Clone)]
pub struct RadiusOAuthCredentials {
    pub access: String,
    pub refresh: Option<String>,
    pub expires: i64,
    pub scope: Option<String>,
    pub gateway_config: Option<RadiusGatewayConfig>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadiusDeviceAuthorization {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: Option<String>,
    pub verification_uri_complete: Option<String>,
    pub expires_in: u64,
    pub interval: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadiusAuthorizeRequest {
    pub url: String,
    pub verifier: String,
    pub state: String,
}

pub fn normalize_radius_gateway_url(value: &str) -> String {
    let with_scheme = if value.starts_with("http://") || value.starts_with("https://") {
        value.to_string()
    } else {
        format!("https://{value}")
    };
    with_scheme.trim_end_matches('/').to_string()
}

fn truncate_http_body(body: &str) -> String {
    let trimmed = body.trim();
    if trimmed.chars().count() > 512 {
        format!("{}…", trimmed.chars().take(512).collect::<String>())
    } else {
        trimmed.to_string()
    }
}

fn radius_oauth_error(status: reqwest::StatusCode, body: &str, message: &str) -> String {
    let mut detail = status.as_u16().to_string();
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(body) {
        let oauth_error = v.get("error").and_then(|x| x.as_str());
        let desc = v.get("error_description").and_then(|x| x.as_str());
        detail = match (oauth_error, desc) {
            (Some(e), Some(d)) => format!("{e}: {d}"),
            (Some(e), None) => e.to_string(),
            (None, Some(d)) => d.to_string(),
            _ => detail,
        };
    } else if !body.trim().is_empty() {
        detail = truncate_http_body(body);
    }
    format!("{message}: {detail}")
}

fn parse_radius_oauth_error(body: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|v| v.get("error").and_then(|x| x.as_str()).map(str::to_string))
}

pub async fn load_radius_oauth_config(gateway: &str) -> Result<RadiusOAuthConfig, String> {
    let gateway = normalize_radius_gateway_url(gateway);
    let url = format!("{gateway}/v1/oauth");
    let response = crate::http_proxy::client_for_target(&url, None)
        .get(&url)
        .header("accept", "application/json")
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!(
            "Could not load Radius OAuth config from {gateway}: {} {}",
            status.as_u16(),
            truncate_http_body(&body)
        ));
    }
    let v: serde_json::Value = response.json().await.map_err(|e| e.to_string())?;
    Ok(RadiusOAuthConfig {
        issuer: v
            .get("issuer")
            .and_then(|x| x.as_str())
            .unwrap_or(&gateway)
            .to_string(),
        authorization_endpoint: required_string(&v, "authorizationEndpoint")?,
        token_endpoint: v
            .get("tokenEndpoint")
            .and_then(|x| x.as_str())
            .map(str::to_string)
            .unwrap_or_else(|| format!("{gateway}/v1/oauth/token")),
        device_authorization_endpoint: v
            .get("deviceAuthorizationEndpoint")
            .and_then(|x| x.as_str())
            .map(str::to_string)
            .unwrap_or_else(|| format!("{gateway}/v1/oauth/device_authorization")),
        device_authorization_events_endpoint: v
            .get("deviceAuthorizationEventsEndpoint")
            .and_then(|x| x.as_str())
            .map(str::to_string)
            .unwrap_or_else(|| format!("{gateway}/v1/oauth/device_authorization/events")),
        verification_endpoint: v
            .get("verificationEndpoint")
            .and_then(|x| x.as_str())
            .map(str::to_string)
            .unwrap_or_else(|| gateway.clone()),
        client_id: v
            .get("clientId")
            .and_then(|x| x.as_str())
            .unwrap_or("pi-gateway")
            .to_string(),
        scope: v
            .get("scope")
            .and_then(|x| x.as_str())
            .unwrap_or("gateway offline_access")
            .to_string(),
        device_code_grant_type: v
            .get("deviceCodeGrantType")
            .and_then(|x| x.as_str())
            .unwrap_or("urn:ietf:params:oauth:grant-type:device_code")
            .to_string(),
    })
}

fn required_string(v: &serde_json::Value, key: &str) -> Result<String, String> {
    v.get(key)
        .and_then(|x| x.as_str())
        .map(str::to_string)
        .ok_or_else(|| format!("Radius OAuth config is missing {key}"))
}

pub(crate) fn sanitize_radius_gateway_config(
    v: serde_json::Value,
) -> Result<RadiusGatewayConfig, String> {
    let base_url = v
        .get("baseUrl")
        .and_then(|x| x.as_str())
        .ok_or_else(|| "Invalid Radius config from gateway".to_string())?
        .to_string();
    let models = v
        .get("models")
        .and_then(|x| x.as_array())
        .ok_or_else(|| "Invalid Radius config from gateway".to_string())?;
    let mut out = Vec::new();
    for model in models {
        let Some(id) = model.get("id").and_then(|x| x.as_str()) else {
            continue;
        };
        let Some(name) = model.get("name").and_then(|x| x.as_str()) else {
            continue;
        };
        let Some(reasoning) = model.get("reasoning").and_then(|x| x.as_bool()) else {
            continue;
        };
        let Some(input) = model.get("input").and_then(|x| x.as_array()) else {
            continue;
        };
        let Some(cost_value) = model.get("cost") else {
            continue;
        };
        let Some(context_window) = model.get("contextWindow").and_then(|x| x.as_u64()) else {
            continue;
        };
        let Some(max_tokens) = model.get("maxTokens").and_then(|x| x.as_u64()) else {
            continue;
        };
        let thinking_level_map = model
            .get("thinkingLevelMap")
            .and_then(|x| serde_json::from_value(x.clone()).ok());
        let cost = serde_json::from_value(cost_value.clone()).unwrap_or_default();
        out.push(RadiusGatewayModel {
            id: id.to_string(),
            name: name.to_string(),
            reasoning,
            thinking_level_map,
            input: input
                .iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect(),
            cost,
            context_window: context_window as u32,
            max_tokens: max_tokens as u32,
        });
    }
    Ok(RadiusGatewayConfig {
        base_url,
        models: out,
    })
}

pub async fn load_radius_gateway_config(
    gateway: &str,
    api_key: Option<&str>,
) -> Result<RadiusGatewayConfig, String> {
    let gateway = normalize_radius_gateway_url(gateway);
    let url = format!("{gateway}/v1/config");
    let mut req = crate::http_proxy::client_for_target(&url, None)
        .get(&url)
        .header("accept", "application/json");
    if let Some(api_key) = api_key {
        req = req.bearer_auth(api_key);
    }
    let response = req.send().await.map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!(
            "Could not load Radius config from {gateway}: {}: {}",
            status.as_u16(),
            truncate_http_body(&body)
        ));
    }
    sanitize_radius_gateway_config(response.json().await.map_err(|e| e.to_string())?)
}

pub async fn request_radius_oauth_token(
    oauth: &RadiusOAuthConfig,
    body: &[(impl AsRef<str>, impl AsRef<str>)],
) -> Result<RadiusOAuthCredentials, String> {
    let params: Vec<(String, String)> = body
        .iter()
        .map(|(k, v)| (k.as_ref().to_string(), v.as_ref().to_string()))
        .collect();
    let response = crate::http_proxy::client_for_target(&oauth.token_endpoint, None)
        .post(&oauth.token_endpoint)
        .header("accept", "application/json")
        .form(&params)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(radius_oauth_error(
            status,
            &body,
            "Radius OAuth token request failed",
        ));
    }
    let v: serde_json::Value = response.json().await.map_err(|e| e.to_string())?;
    let access = required_string(&v, "access_token")?;
    let refresh = required_string(&v, "refresh_token")?;
    let expires_in = v
        .get("expires_in")
        .and_then(|x| x.as_i64())
        .ok_or_else(|| "Radius OAuth token response is missing expires_in".to_string())?;
    Ok(RadiusOAuthCredentials {
        access,
        refresh: Some(refresh),
        expires: crate::utils::now_millis() + expires_in * 1000 - RADIUS_TOKEN_EXPIRY_SKEW_MS,
        scope: v.get("scope").and_then(|x| x.as_str()).map(str::to_string),
        gateway_config: None,
    })
}

pub async fn exchange_radius_code(
    oauth: &RadiusOAuthConfig,
    code: &str,
    verifier: &str,
    redirect_uri: &str,
) -> Result<RadiusOAuthCredentials, String> {
    request_radius_oauth_token(
        oauth,
        &[
            ("grant_type", "authorization_code"),
            ("client_id", oauth.client_id.as_str()),
            ("redirect_uri", redirect_uri),
            ("code", code),
            ("code_verifier", verifier),
        ],
    )
    .await
}

pub async fn refresh_radius_token(
    oauth: &RadiusOAuthConfig,
    refresh_token: &str,
) -> Result<RadiusOAuthCredentials, String> {
    request_radius_oauth_token(
        oauth,
        &[
            ("grant_type", "refresh_token"),
            ("client_id", oauth.client_id.as_str()),
            ("refresh_token", refresh_token),
        ],
    )
    .await
}

pub async fn request_radius_device_authorization(
    oauth: &RadiusOAuthConfig,
) -> Result<RadiusDeviceAuthorization, String> {
    let response = crate::http_proxy::client_for_target(&oauth.device_authorization_endpoint, None)
        .post(&oauth.device_authorization_endpoint)
        .header("accept", "application/json")
        .form(&[
            ("client_id", oauth.client_id.as_str()),
            ("scope", oauth.scope.as_str()),
        ])
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(radius_oauth_error(
            status,
            &body,
            "Radius OAuth device authorization failed",
        ));
    }
    let v: serde_json::Value = response.json().await.map_err(|e| e.to_string())?;
    let device_code = required_string(&v, "device_code")?;
    let user_code = required_string(&v, "user_code")?;
    let expires_in = v
        .get("expires_in")
        .and_then(|x| x.as_u64())
        .ok_or_else(|| {
            "Radius OAuth device authorization response is missing required fields".to_string()
        })?;
    Ok(RadiusDeviceAuthorization {
        device_code,
        user_code,
        verification_uri: v
            .get("verification_uri")
            .and_then(|x| x.as_str())
            .map(str::to_string),
        verification_uri_complete: v
            .get("verification_uri_complete")
            .and_then(|x| x.as_str())
            .map(str::to_string),
        expires_in,
        interval: v.get("interval").and_then(|x| x.as_u64()),
    })
}

pub async fn login_radius_device_code(
    oauth: &RadiusOAuthConfig,
) -> Result<RadiusOAuthCredentials, String> {
    login_radius_device_code_with_cancel(oauth, std::future::pending::<()>()).await
}

pub async fn login_radius_device_code_with_cancel(
    oauth: &RadiusOAuthConfig,
    cancel: impl std::future::Future<Output = ()>,
) -> Result<RadiusOAuthCredentials, String> {
    let device = request_radius_device_authorization(oauth).await?;
    poll_oauth_device_code_flow(
        device.interval.unwrap_or(5),
        device.expires_in,
        false,
        || async {
            match request_radius_oauth_token(
                oauth,
                &[
                    ("grant_type", oauth.device_code_grant_type.as_str()),
                    ("client_id", oauth.client_id.as_str()),
                    ("device_code", device.device_code.as_str()),
                ],
            )
            .await
            {
                Ok(credentials) => DevicePollOutcome::Complete(credentials),
                Err(err) => {
                    let detail = err
                        .strip_prefix("Radius OAuth token request failed: ")
                        .unwrap_or(&err);
                    let oauth_error =
                        parse_radius_oauth_error(detail).unwrap_or_else(|| detail.to_string());
                    match oauth_error.as_str() {
                        "authorization_pending" => DevicePollOutcome::Pending,
                        "slow_down" => DevicePollOutcome::SlowDown(None),
                        "expired_token" => {
                            DevicePollOutcome::Failed("Device authorization expired.".to_string())
                        }
                        "access_denied" => DevicePollOutcome::Failed(
                            "Device authorization was denied.".to_string(),
                        ),
                        _ => DevicePollOutcome::Failed(err),
                    }
                }
            }
        },
        cancel,
    )
    .await
}

pub async fn attach_radius_gateway_config(
    gateway: &str,
    mut credentials: RadiusOAuthCredentials,
    previous: Option<&RadiusOAuthCredentials>,
) -> Result<RadiusOAuthCredentials, String> {
    match load_radius_gateway_config(gateway, Some(&credentials.access)).await {
        Ok(config) => {
            credentials.gateway_config = Some(config);
            Ok(credentials)
        }
        Err(err) => {
            if let Some(prev) = previous.and_then(|p| p.gateway_config.clone()) {
                credentials.gateway_config = Some(prev);
                Ok(credentials)
            } else {
                Err(err)
            }
        }
    }
}

pub fn build_radius_authorize_request(
    oauth: &RadiusOAuthConfig,
    state: &str,
    pkce: &PkceChallenge,
) -> RadiusAuthorizeRequest {
    let mut url =
        reqwest::Url::parse(&oauth.authorization_endpoint).expect("valid authorization endpoint");
    url.query_pairs_mut()
        .append_pair("response_type", "code")
        .append_pair("client_id", &oauth.client_id)
        .append_pair("redirect_uri", RADIUS_REDIRECT_URI)
        .append_pair("scope", &oauth.scope)
        .append_pair("code_challenge", &pkce.challenge)
        .append_pair("code_challenge_method", "S256")
        .append_pair("handoff", "url")
        .append_pair("state", state);
    RadiusAuthorizeRequest {
        url: url.to_string(),
        verifier: pkce.verifier.clone(),
        state: state.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_pkce() {
        let pkce = generate_pkce();
        assert!(!pkce.verifier.is_empty());
        assert!(!pkce.challenge.is_empty());
        assert_ne!(pkce.verifier, pkce.challenge);
        assert!(
            pkce.verifier
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        );
        assert!(
            pkce.challenge
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        );
    }

    #[tokio::test]
    async fn test_refresh_anthropic_token() {
        use wiremock::matchers::{body_partial_json, method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/oauth/token"))
            .and(body_partial_json(
                serde_json::json!({"grant_type": "refresh_token", "refresh_token": "old-refresh"}),
            ))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"access_token":"new-access","refresh_token":"new-refresh","expires_in":3600}"#,
            ))
            .mount(&server)
            .await;
        let url = format!("{}/oauth/token", server.uri());
        let before = crate::utils::now_millis();
        let tok = refresh_anthropic_token_at(&url, "old-refresh")
            .await
            .unwrap();
        assert_eq!(tok.access, "new-access");
        assert_eq!(tok.refresh.as_deref(), Some("new-refresh"));
        // expires ~= now + 3600s - 5min safety margin.
        let expected = before + 3600 * 1000 - 5 * 60 * 1000;
        assert!((tok.expires_at_ms - expected).abs() < 5000);
    }

    #[tokio::test]
    async fn test_refresh_anthropic_token_invalid_json() {
        use wiremock::matchers::method;
        use wiremock::{Mock, MockServer, ResponseTemplate};
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_string("<html>nope</html>"))
            .mount(&server)
            .await;
        let err = refresh_anthropic_token_at(&server.uri(), "r")
            .await
            .unwrap_err();
        assert!(err.contains("invalid JSON"));
    }

    #[tokio::test]
    async fn test_refresh_codex_token_extracts_account_id() {
        use base64::Engine;
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};
        // Build a JWT whose payload carries the chatgpt_account_id claim.
        let payload = serde_json::json!({
            "https://api.openai.com/auth": {"chatgpt_account_id": "acc_123"}
        });
        let payload_b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(serde_json::to_vec(&payload).unwrap());
        let jwt = format!("h.{payload_b64}.s");
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/oauth/token"))
            .respond_with(ResponseTemplate::new(200).set_body_string(format!(
                r#"{{"access_token":"{jwt}","refresh_token":"new-refresh","expires_in":3600}}"#
            )))
            .mount(&server)
            .await;
        let url = format!("{}/oauth/token", server.uri());
        let creds = refresh_codex_token_at(&url, "old").await.unwrap();
        assert_eq!(creds.access, jwt);
        assert_eq!(creds.refresh.as_deref(), Some("new-refresh"));
        assert_eq!(creds.account_id, "acc_123");
    }

    #[tokio::test]
    async fn test_refresh_copilot_token() {
        use wiremock::matchers::{header, method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/copilot_internal/v2/token"))
            .and(header("Authorization", "Bearer gho_refresh"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(r#"{"token":"copilot-access","expires_at":1000000}"#),
            )
            .mount(&server)
            .await;
        let url = format!("{}/copilot_internal/v2/token", server.uri());
        let creds = refresh_copilot_token_at(&url, "gho_refresh").await.unwrap();
        assert_eq!(creds.access, "copilot-access");
        assert_eq!(creds.refresh, "gho_refresh");
        // expires_at (seconds) * 1000 - 5min margin.
        assert_eq!(creds.expires_at_ms, 1000000 * 1000 - 5 * 60 * 1000);
    }

    #[tokio::test]
    async fn test_start_github_device_flow() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/login/device/code"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"device_code":"dc","user_code":"WXYZ-1234","verification_uri":"https://github.com/login/device","interval":5,"expires_in":900}"#,
            ))
            .mount(&server)
            .await;
        let url = format!("{}/login/device/code", server.uri());
        let dc = start_github_device_flow_at(&url, COPILOT_CLIENT_ID)
            .await
            .unwrap();
        assert_eq!(dc.device_code, "dc");
        assert_eq!(dc.user_code, "WXYZ-1234");
        assert_eq!(dc.verification_uri, "https://github.com/login/device");
        assert_eq!(dc.interval, Some(5));
        assert_eq!(dc.expires_in, 900);
    }

    #[tokio::test]
    async fn test_start_github_device_flow_rejects_untrusted_uri() {
        use wiremock::matchers::method;
        use wiremock::{Mock, MockServer, ResponseTemplate};
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"device_code":"dc","user_code":"x","verification_uri":"javascript:alert(1)","expires_in":900}"#,
            ))
            .mount(&server)
            .await;
        let err = start_github_device_flow_at(&server.uri(), COPILOT_CLIENT_ID)
            .await
            .unwrap_err();
        assert!(err.contains("Untrusted verification_uri"));
    }

    #[tokio::test]
    async fn test_poll_github_device_token_classifies_responses() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};
        // pending
        let s1 = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/t"))
            .respond_with(
                ResponseTemplate::new(200).set_body_string(r#"{"error":"authorization_pending"}"#),
            )
            .mount(&s1)
            .await;
        assert_eq!(
            poll_github_device_token_at(&format!("{}/t", s1.uri()), COPILOT_CLIENT_ID, "dc").await,
            DevicePollStatus::Pending
        );
        // slow_down
        let s2 = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/t"))
            .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"error":"slow_down"}"#))
            .mount(&s2)
            .await;
        assert_eq!(
            poll_github_device_token_at(&format!("{}/t", s2.uri()), COPILOT_CLIENT_ID, "dc").await,
            DevicePollStatus::SlowDown(None)
        );
        // complete
        let s3 = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/t"))
            .respond_with(
                ResponseTemplate::new(200).set_body_string(r#"{"access_token":"gho_tok"}"#),
            )
            .mount(&s3)
            .await;
        assert_eq!(
            poll_github_device_token_at(&format!("{}/t", s3.uri()), COPILOT_CLIENT_ID, "dc").await,
            DevicePollStatus::Complete("gho_tok".to_string())
        );
        // failed with description
        let s4 = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/t"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(r#"{"error":"access_denied","error_description":"nope"}"#),
            )
            .mount(&s4)
            .await;
        assert_eq!(
            poll_github_device_token_at(&format!("{}/t", s4.uri()), COPILOT_CLIENT_ID, "dc").await,
            DevicePollStatus::Failed("Device flow failed: access_denied: nope".to_string())
        );
    }

    #[test]
    fn test_build_anthropic_authorize_url() {
        let url = build_anthropic_authorize_url("CHAL", "VERIF", ANTHROPIC_REDIRECT_URI);
        assert!(url.starts_with("https://claude.ai/oauth/authorize?"));
        assert!(url.contains("client_id=9d1c250a-e61b-44d9-88ed-5944d1962f5e"));
        assert!(url.contains("response_type=code"));
        assert!(url.contains("code_challenge=CHAL"));
        assert!(url.contains("code_challenge_method=S256"));
        assert!(url.contains("state=VERIF"));
        // redirect_uri and scope are percent-encoded.
        assert!(url.contains("redirect_uri=http%3A%2F%2Flocalhost%3A53692%2Fcallback"));
        assert!(url.contains("scope=org%3Acreate_api_key"));
    }

    #[tokio::test]
    async fn test_exchange_anthropic_code() {
        use wiremock::matchers::{body_partial_json, method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/oauth/token"))
            .and(body_partial_json(serde_json::json!({
                "grant_type": "authorization_code", "code": "the-code", "code_verifier": "v"
            })))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"access_token":"acc","refresh_token":"ref","expires_in":3600}"#,
            ))
            .mount(&server)
            .await;
        let url = format!("{}/oauth/token", server.uri());
        let tok = exchange_anthropic_code_at(
            &url,
            "the-code",
            "st",
            "v",
            "http://localhost:53692/callback",
        )
        .await
        .unwrap();
        assert_eq!(tok.access, "acc");
        assert_eq!(tok.refresh.as_deref(), Some("ref"));
    }

    #[test]
    fn test_build_codex_authorize_url() {
        let url = build_codex_authorize_url("CHAL", "ST", CODEX_REDIRECT_URI, "pi");
        assert!(url.starts_with("https://auth.openai.com/oauth/authorize?"));
        assert!(url.contains("client_id=app_EMoamEEZ73f0CkXaXp7hrann"));
        assert!(url.contains("scope=openid%20profile%20email%20offline_access"));
        assert!(url.contains("code_challenge_method=S256"));
        assert!(url.contains("id_token_add_organizations=true"));
        assert!(url.contains("codex_cli_simplified_flow=true"));
        assert!(url.contains("originator=pi"));
        assert!(url.contains("redirect_uri=http%3A%2F%2Flocalhost%3A1455%2Fauth%2Fcallback"));
    }

    #[tokio::test]
    async fn test_exchange_codex_code() {
        use base64::Engine;
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};
        let payload =
            serde_json::json!({"https://api.openai.com/auth": {"chatgpt_account_id": "acc_9"}});
        let payload_b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(serde_json::to_vec(&payload).unwrap());
        let jwt = format!("h.{payload_b64}.s");
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/oauth/token"))
            .respond_with(ResponseTemplate::new(200).set_body_string(format!(
                r#"{{"access_token":"{jwt}","refresh_token":"r","expires_in":3600}}"#
            )))
            .mount(&server)
            .await;
        let url = format!("{}/oauth/token", server.uri());
        let creds = exchange_codex_code_at(&url, "code", "verifier", CODEX_REDIRECT_URI)
            .await
            .unwrap();
        assert_eq!(creds.account_id, "acc_9");
        assert_eq!(creds.refresh.as_deref(), Some("r"));
    }

    #[test]
    fn test_codex_account_id_empty_is_none() {
        use base64::Engine;
        let mk = |acc: &str| {
            let payload =
                serde_json::json!({"https://api.openai.com/auth": {"chatgpt_account_id": acc}});
            let b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD
                .encode(serde_json::to_vec(&payload).unwrap());
            format!("h.{b64}.s")
        };
        // Upstream getAccountId returns null for an empty account id.
        assert_eq!(codex_account_id(&mk("")), None);
        assert_eq!(codex_account_id(&mk("acc_42")).as_deref(), Some("acc_42"));
        // Malformed (not 3 parts) -> None.
        assert_eq!(codex_account_id("not.a.valid.jwt"), None);
        assert_eq!(codex_account_id("onlyonepart"), None);
    }

    #[tokio::test]
    async fn test_codex_exchange_requires_refresh_token() {
        use base64::Engine;
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};
        let payload =
            serde_json::json!({"https://api.openai.com/auth": {"chatgpt_account_id": "a"}});
        let payload_b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(serde_json::to_vec(&payload).unwrap());
        let jwt = format!("h.{payload_b64}.s");
        let server = MockServer::start().await;
        // Response omits refresh_token -> upstream rejects with "missing fields".
        Mock::given(method("POST"))
            .and(path("/oauth/token"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(format!(r#"{{"access_token":"{jwt}","expires_in":3600}}"#)),
            )
            .mount(&server)
            .await;
        let url = format!("{}/oauth/token", server.uri());
        let err = exchange_codex_code_at(&url, "code", "v", CODEX_REDIRECT_URI)
            .await
            .unwrap_err();
        assert!(err.contains("missing fields"), "{err}");
    }
}
