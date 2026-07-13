//! HTTP retry logic with exponential backoff.

use std::time::Duration;

/// Retry configuration.
#[derive(Debug, Clone)]
pub struct RetryConfig {
    pub max_retries: u32,
    pub initial_delay: Duration,
    pub max_delay: Duration,
    pub backoff_multiplier: f64,
    pub jitter_fraction: f64,
    pub max_retry_delay_ms: u64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(60),
            backoff_multiplier: 2.0,
            jitter_fraction: 0.25,
            max_retry_delay_ms: 60_000,
        }
    }
}

impl RetryConfig {
    /// No retries.
    pub fn none() -> Self {
        Self {
            max_retries: 0,
            ..Default::default()
        }
    }
}

/// Compute exponential backoff delay for an attempt.
pub fn compute_backoff(attempt: u32, config: &RetryConfig) -> Duration {
    let base = config.initial_delay.as_secs_f64() * config.backoff_multiplier.powi(attempt as i32);
    let capped = base.min(config.max_delay.as_secs_f64());
    // Simple jitter: multiply by (1 - jitter/2) for deterministic tests
    let jittered = capped * (1.0 - config.jitter_fraction * 0.5);
    Duration::from_secs_f64(jittered.max(0.0))
}

/// Check if an HTTP status code is retryable. Mirrors the OpenAI/Anthropic SDK
/// retry set (408 request timeout, 409 conflict/lock, 429 rate limit, and all 5xx).
pub fn is_retryable_status(code: u16) -> bool {
    matches!(code, 408 | 409 | 429) || code >= 500
}

/// Parse a `Retry-After` header value (integer/float seconds, or HTTP-date) into a Duration.
pub fn parse_retry_after(value: &str) -> Option<Duration> {
    let trimmed = value.trim();
    if let Ok(seconds) = trimmed.parse::<f64>() {
        if seconds.is_finite() && seconds >= 0.0 {
            return Some(Duration::from_secs_f64(seconds));
        }
        return None;
    }
    // HTTP-date form: delay until that instant.
    if let Ok(when) = httpdate::parse_http_date(trimmed) {
        return Some(
            when.duration_since(std::time::SystemTime::now())
                .unwrap_or(Duration::ZERO),
        );
    }
    None
}

/// Resolve a retry delay from response headers, preferring `retry-after-ms`
/// then `retry-after` (mirrors upstream getRetryAfterDelayMs).
pub fn retry_after_delay(headers: &reqwest::header::HeaderMap) -> Option<Duration> {
    if let Some(ms) = headers.get("retry-after-ms").and_then(|v| v.to_str().ok())
        && let Ok(millis) = ms.trim().parse::<f64>()
        && millis.is_finite()
    {
        return Some(Duration::from_millis(millis.max(0.0) as u64));
    }
    headers
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|v| v.to_str().ok())
        .and_then(parse_retry_after)
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn test_compute_backoff() {
        let config = RetryConfig::default();
        let d0 = compute_backoff(0, &config);
        let d1 = compute_backoff(1, &config);
        assert!(d1 > d0, "backoff should increase");
        assert!(d1.as_secs_f64() <= config.max_delay.as_secs_f64());
    }

    #[test]
    fn test_is_retryable() {
        assert!(is_retryable_status(429));
        assert!(is_retryable_status(500));
        assert!(is_retryable_status(408));
        assert!(is_retryable_status(409));
        assert!(is_retryable_status(501));
        assert!(is_retryable_status(529));
        assert!(!is_retryable_status(200));
        assert!(!is_retryable_status(400));
        assert!(!is_retryable_status(404));
    }

    #[test]
    fn test_parse_retry_after() {
        assert_eq!(parse_retry_after("30"), Some(Duration::from_secs(30)));
        assert_eq!(parse_retry_after("1.5"), Some(Duration::from_secs_f64(1.5)));
        assert_eq!(parse_retry_after("not-a-number"), None);
        // HTTP-date in the past clamps to zero.
        assert_eq!(
            parse_retry_after("Wed, 21 Oct 2015 07:28:00 GMT"),
            Some(Duration::ZERO)
        );
    }

    #[test]
    fn test_retry_after_delay_prefers_ms_header() {
        use reqwest::header::HeaderMap;
        let mut h = HeaderMap::new();
        h.insert("retry-after-ms", "250".parse().unwrap());
        h.insert(reqwest::header::RETRY_AFTER, "5".parse().unwrap());
        // retry-after-ms wins over retry-after.
        assert_eq!(retry_after_delay(&h), Some(Duration::from_millis(250)));
        let mut h2 = HeaderMap::new();
        h2.insert(reqwest::header::RETRY_AFTER, "5".parse().unwrap());
        assert_eq!(retry_after_delay(&h2), Some(Duration::from_secs(5)));
    }

    #[test]
    fn test_retry_config_from_options() {
        let opts = crate::types::StreamOptions {
            max_retries: Some(4),
            max_retry_delay_ms: Some(1500),
            ..Default::default()
        };
        let cfg = retry_config_from_options(&opts);
        assert_eq!(cfg.max_retries, 4);
        assert_eq!(cfg.max_retry_delay_ms, 1500);
        assert_eq!(cfg.max_delay, Duration::from_millis(1500));
    }

    #[tokio::test]
    async fn test_do_with_retry_retries_retryable_status() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/retry"))
            .respond_with(ResponseTemplate::new(503).set_body_string("busy"))
            .up_to_n_times(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/retry"))
            .respond_with(ResponseTemplate::new(200).set_body_string("ok"))
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        let req = client.get(format!("{}/retry", server.uri()));
        let cfg = RetryConfig {
            max_retries: 1,
            initial_delay: Duration::from_millis(1),
            max_delay: Duration::from_millis(5),
            backoff_multiplier: 1.0,
            jitter_fraction: 0.0,
            max_retry_delay_ms: 5,
        };
        let resp = do_with_retry(&client, req, &cfg).await.unwrap();
        assert_eq!(resp.status(), 200);
    }
}

/// No-retry config.
pub fn no_retry_config() -> RetryConfig {
    RetryConfig::none()
}

/// Default retry config.
pub fn default_retry_config() -> RetryConfig {
    RetryConfig::default()
}

/// Build retry config from stream options (mirrors Go's RetryConfigFromOptions).
pub fn retry_config_from_options(opts: &crate::types::StreamOptions) -> RetryConfig {
    if opts.retry_config.is_none()
        && opts.max_retries.is_none()
        && opts.max_retry_delay_ms.is_none()
    {
        return RetryConfig::none();
    }

    let mut cfg = opts.retry_config.clone().unwrap_or_default();
    if let Some(max_retries) = opts.max_retries {
        cfg.max_retries = max_retries;
    }
    if let Some(max_retry_delay_ms) = opts.max_retry_delay_ms {
        cfg.max_retry_delay_ms = max_retry_delay_ms;
        cfg.max_delay = Duration::from_millis(max_retry_delay_ms);
    }
    cfg
}

/// Execute an HTTP request with retry logic (async).
pub async fn do_with_retry(
    _client: &reqwest::Client,
    request_builder: reqwest::RequestBuilder,
    config: &RetryConfig,
) -> Result<reqwest::Response, reqwest::Error> {
    let mut attempt = 0u32;
    let mut builder = request_builder;

    loop {
        let retry_builder = builder.try_clone();
        match builder.send().await {
            Ok(resp) => {
                if !is_retryable_status(resp.status().as_u16()) || attempt >= config.max_retries {
                    return Ok(resp);
                }

                let retry_after = retry_after_delay(resp.headers());
                let mut delay = retry_after.unwrap_or_else(|| compute_backoff(attempt, config));
                delay = delay.min(Duration::from_millis(config.max_retry_delay_ms));
                tokio::time::sleep(delay).await;

                attempt += 1;
                builder = match retry_builder {
                    Some(b) => b,
                    None => return Ok(resp),
                };
            }
            Err(err) => {
                if attempt >= config.max_retries {
                    return Err(err);
                }
                let delay = compute_backoff(attempt, config)
                    .min(Duration::from_millis(config.max_retry_delay_ms));
                tokio::time::sleep(delay).await;
                attempt += 1;
                builder = match retry_builder {
                    Some(b) => b,
                    None => return Err(err),
                };
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Assistant-error classification (port of upstream utils/retry.ts, v0.80.3).
//
// `is_retryable_assistant_error` classifies whether a failed assistant message
// looks like a transient provider/transport error so callers can decide whether
// to restart the last assistant turn. It does NOT implement retry policy.
// ---------------------------------------------------------------------------

/// A single classification pattern. `Plain` is a case-insensitive substring;
/// `Gap` is literal segments separated by an optional single character (regex
/// `.?`) — e.g. `["rate","limit"]` matches "ratelimit", "rate limit", "rate-limit".
enum ErrPat {
    Plain(&'static str),
    Gap(&'static [&'static str]),
}

impl ErrPat {
    /// `haystack` must already be lowercased.
    fn matches(&self, haystack: &str) -> bool {
        match self {
            ErrPat::Plain(needle) => haystack.contains(needle),
            ErrPat::Gap(segs) => contains_with_gaps(haystack, segs),
        }
    }
}

/// Match `segs` in `haystack` where consecutive segments may be separated by an
/// optional single arbitrary character (regex `.?`). Byte-safe: a non-char-boundary
/// gap simply fails to match rather than panicking.
fn contains_with_gaps(haystack: &str, segs: &[&str]) -> bool {
    let first = segs[0];
    let mut start = 0;
    while let Some(idx) = haystack[start..].find(first) {
        let abs = start + idx;
        let mut pos = abs + first.len();
        let mut ok = true;
        for seg in &segs[1..] {
            if haystack.get(pos..).is_some_and(|s| s.starts_with(seg)) {
                pos += seg.len();
            } else if haystack.get(pos + 1..).is_some_and(|s| s.starts_with(seg)) {
                pos += 1 + seg.len();
            } else {
                ok = false;
                break;
            }
        }
        if ok {
            return true;
        }
        start = abs + 1;
    }
    false
}

/// Subscription / quota / billing limits that should NOT be auto-retried.
const NON_RETRYABLE_PROVIDER_LIMIT: &[ErrPat] = &[
    ErrPat::Plain("gousagelimiterror"),
    ErrPat::Plain("freeusagelimiterror"),
    ErrPat::Plain("monthly usage limit reached"),
    ErrPat::Plain("available balance"),
    ErrPat::Plain("insufficient_quota"),
    ErrPat::Plain("out of budget"),
    ErrPat::Plain("quota exceeded"),
    ErrPat::Plain("billing"),
];

/// Transient provider / transport / stream errors that may be retried.
const RETRYABLE_PROVIDER_ERROR: &[ErrPat] = &[
    ErrPat::Plain("overloaded"),
    ErrPat::Gap(&["rate", "limit"]),
    ErrPat::Plain("too many requests"),
    ErrPat::Plain("429"),
    ErrPat::Plain("500"),
    ErrPat::Plain("502"),
    ErrPat::Plain("503"),
    ErrPat::Plain("504"),
    ErrPat::Plain("524"),
    ErrPat::Gap(&["service", "unavailable"]),
    ErrPat::Gap(&["server", "error"]),
    ErrPat::Gap(&["internal", "error"]),
    ErrPat::Gap(&["provider", "returned", "error"]),
    ErrPat::Gap(&["network", "error"]),
    ErrPat::Gap(&["connection", "error"]),
    ErrPat::Gap(&["connection", "refused"]),
    ErrPat::Gap(&["connection", "lost"]),
    ErrPat::Plain("other side closed"),
    ErrPat::Plain("fetch failed"),
    ErrPat::Gap(&["upstream", "connect"]),
    ErrPat::Plain("reset before headers"),
    ErrPat::Plain("socket hang up"),
    ErrPat::Plain("socket connection was closed"),
    // `timed? out` — optional literal `d`.
    ErrPat::Plain("timed out"),
    ErrPat::Plain("time out"),
    ErrPat::Plain("timeout"),
    ErrPat::Plain("terminated"),
    ErrPat::Gap(&["websocket", "closed"]),
    ErrPat::Gap(&["websocket", "error"]),
    ErrPat::Plain("ended without"),
    ErrPat::Plain("stream ended before message_stop"),
    ErrPat::Plain("http2 request did not get a response"),
    ErrPat::Plain("retry delay"),
    ErrPat::Plain("you can retry your request"),
    ErrPat::Plain("try your request again"),
    ErrPat::Plain("please retry your request"),
    // gRPC based providers (e.g. NVIDIA NIM).
    ErrPat::Plain("resourceexhausted"),
];

/// Classify whether a failed assistant message looks like a transient provider
/// or transport error, so callers can decide if the last assistant turn should
/// be restarted. Mirrors upstream `isRetryableAssistantError`.
pub fn is_retryable_assistant_error(message: &crate::types::Message) -> bool {
    if !matches!(message.stop_reason, Some(crate::types::StopReason::Error)) {
        return false;
    }
    let Some(err) = message.error_message.as_deref() else {
        return false;
    };
    let haystack = err.to_lowercase();
    if NON_RETRYABLE_PROVIDER_LIMIT
        .iter()
        .any(|p| p.matches(&haystack))
    {
        return false;
    }
    RETRYABLE_PROVIDER_ERROR
        .iter()
        .any(|p| p.matches(&haystack))
}
