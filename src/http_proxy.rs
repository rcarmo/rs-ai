//! Test-for-test port of upstream `src/utils/node-http-proxy.ts`
//! (`@earendil-works/pi-ai`).
//!
//! Resolves the HTTP/HTTPS proxy URL that applies to a given target URL using
//! the standard `*_proxy` / `no_proxy` environment-variable conventions, with an
//! optional caller-supplied env map taking precedence over process env. SOCKS and
//! PAC proxies are rejected explicitly, mirroring upstream.

use std::collections::HashMap;

pub const UNSUPPORTED_PROXY_PROTOCOL_MESSAGE: &str =
    "Unsupported proxy protocol. SOCKS and PAC proxy URLs are not supported; use an HTTP or HTTPS proxy URL.";

fn default_proxy_port(protocol: &str) -> u32 {
    match protocol {
        "ftp" => 21,
        "gopher" => 70,
        "http" => 80,
        "https" => 443,
        "ws" => 80,
        "wss" => 443,
        _ => 0,
    }
}

/// Mirror of upstream `getProxyEnv`: prefer the caller env map (lower- then
/// upper-case), then fall back to process env (lower- then upper-case).
fn get_proxy_env(key: &str, env: Option<&HashMap<String, String>>) -> String {
    let lower = key.to_lowercase();
    let upper = key.to_uppercase();
    if let Some(map) = env {
        if let Some(v) = map.get(&lower).filter(|v| !v.is_empty()) {
            return v.clone();
        }
        if let Some(v) = map.get(&upper).filter(|v| !v.is_empty()) {
            return v.clone();
        }
    }
    if let Ok(v) = std::env::var(&lower)
        && !v.is_empty()
    {
        return v;
    }
    if let Ok(v) = std::env::var(&upper)
        && !v.is_empty()
    {
        return v;
    }
    String::new()
}

/// Mirror of upstream `shouldProxyHostname`.
fn should_proxy_hostname(hostname: &str, port: u32, env: Option<&HashMap<String, String>>) -> bool {
    let no_proxy = get_proxy_env("no_proxy", env).to_lowercase();
    if no_proxy.is_empty() {
        return true;
    }
    if no_proxy == "*" {
        return false;
    }

    no_proxy.split(|c: char| c == ',' || c.is_whitespace()).all(|proxy| {
        if proxy.is_empty() {
            return true;
        }

        // Match `^(.+):(\d+)$`.
        let (mut proxy_hostname, proxy_port) = match proxy.rsplit_once(':') {
            Some((host, port_str)) if !host.is_empty() && port_str.parse::<u32>().is_ok() => {
                (host.to_string(), port_str.parse::<u32>().unwrap_or(0))
            }
            _ => (proxy.to_string(), 0),
        };

        if proxy_port != 0 && proxy_port != port {
            return true;
        }

        let starts_with_dot_or_star =
            proxy_hostname.starts_with('.') || proxy_hostname.starts_with('*');
        if !starts_with_dot_or_star {
            return hostname != proxy_hostname;
        }

        if proxy_hostname.starts_with('*') {
            proxy_hostname = proxy_hostname[1..].to_string();
        }
        !hostname.ends_with(&proxy_hostname)
    })
}

/// Mirror of upstream `getProxyForUrl`.
fn get_proxy_for_url(target_url: &str, env: Option<&HashMap<String, String>>) -> String {
    let parsed = match url::Url::parse(target_url) {
        Ok(u) => u,
        Err(_) => return String::new(),
    };
    let protocol = parsed.scheme();
    let hostname = match parsed.host_str() {
        Some(h) => h,
        None => return String::new(),
    };
    if protocol.is_empty() || hostname.is_empty() {
        return String::new();
    }
    let port = parsed.port().map(|p| p as u32).unwrap_or_else(|| default_proxy_port(protocol));

    if !should_proxy_hostname(hostname, port, env) {
        return String::new();
    }

    let mut proxy = get_proxy_env(&format!("{protocol}_proxy"), env);
    if proxy.is_empty() {
        proxy = get_proxy_env("all_proxy", env);
    }
    if !proxy.is_empty() && !proxy.contains("://") {
        proxy = format!("{protocol}://{proxy}");
    }
    proxy
}

/// Resolve the HTTP/HTTPS proxy URL for `target_url`, or `None` when no proxy
/// applies. Returns `Err` for invalid proxy URLs and unsupported (SOCKS/PAC)
/// protocols, mirroring upstream `resolveHttpProxyUrlForTarget`.
pub fn resolve_http_proxy_url_for_target(
    target_url: &str,
    env: Option<&HashMap<String, String>>,
) -> Result<Option<url::Url>, String> {
    let proxy = get_proxy_for_url(target_url, env);
    if proxy.is_empty() {
        return Ok(None);
    }

    let proxy_url = url::Url::parse(&proxy)
        .map_err(|e| format!("Invalid proxy URL {:?}: {}", proxy, e))?;

    if proxy_url.scheme() != "http" && proxy_url.scheme() != "https" {
        return Err(format!(
            "{UNSUPPORTED_PROXY_PROTOCOL_MESSAGE} Got {}:",
            proxy_url.scheme()
        ));
    }

    Ok(Some(proxy_url))
}

/// Build a `reqwest::Client` that routes requests to `target_url` through the
/// resolved HTTP/HTTPS proxy (per `*_proxy`/`no_proxy` env), falling back to a
/// direct client when no proxy applies or proxy resolution fails.
pub fn client_for_target(target_url: &str, env: Option<&HashMap<String, String>>) -> reqwest::Client {
    if let Ok(Some(proxy_url)) = resolve_http_proxy_url_for_target(target_url, env)
        && let Ok(proxy) = reqwest::Proxy::all(proxy_url.as_str())
        && let Ok(client) = reqwest::Client::builder().proxy(proxy).build()
    {
        return client;
    }
    reqwest::Client::new()
}
