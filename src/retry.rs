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

fn is_retryable_response(resp: &reqwest::Response) -> bool {
    match resp
        .headers()
        .get("x-should-retry")
        .and_then(|v| v.to_str().ok())
    {
        Some("true") => return true,
        Some("false") => return false,
        _ => {}
    }
    is_retryable_status(resp.status().as_u16())
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

    fn assistant_msg(stop: crate::types::StopReason, err: Option<&str>) -> crate::types::Message {
        crate::types::Message {
            role: crate::types::Role::Assistant,
            content: Vec::new(),
            timestamp: 0,
            api: None,
            provider: None,
            model: None,
            response_id: None,
            response_model: None,
            diagnostics: Vec::new(),
            usage: None,
            stop_reason: Some(stop),
            deferred: None,
            error_message: err.map(str::to_string),
            raw_stop_reason: None,
            tool_call_id: None,
            tool_name: None,
            is_error: err.is_some(),
            details: None,
            added_tool_names: Vec::new(),
        }
    }

    #[tokio::test(start_paused = true)]
    async fn retry_assistant_reports_abort_after_scheduled_retry_as_unsuccessful() {
        use std::sync::{Arc, Mutex};
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let calls_cb = calls.clone();
        let (tx, rx) = tokio::sync::watch::channel(false);
        let finished = Arc::new(Mutex::new(Vec::<(bool, u32, Option<String>)>::new()));
        let finished_cb = finished.clone();
        let handle = tokio::spawn(async move {
            retry_assistant_call(
                move || {
                    let calls_cb = calls_cb.clone();
                    async move {
                        let n = calls_cb.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                        if n == 0 {
                            assistant_msg(
                                crate::types::StopReason::Error,
                                Some("503 stream ended before a terminal response event"),
                            )
                        } else {
                            assistant_msg(crate::types::StopReason::Stop, None)
                        }
                    }
                },
                Some(AssistantRetryPolicy {
                    enabled: true,
                    max_retries: 1,
                    base_delay_ms: 5000,
                }),
                Some(rx),
                Some(AssistantRetryCallbacks {
                    on_retry_finished: Some(Box::new(move |success, attempt, err| {
                        finished_cb.lock().unwrap().push((success, attempt, err))
                    })),
                }),
            )
            .await
        });
        tokio::task::yield_now().await;
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
        tx.send(true).unwrap();
        let msg = handle.await.unwrap();
        assert_eq!(msg.stop_reason, Some(crate::types::StopReason::Aborted));
        assert_eq!(
            &*finished.lock().unwrap(),
            &[(
                false,
                1,
                Some("503 stream ended before a terminal response event".into())
            )]
        );
    }

    #[tokio::test]
    async fn provider_retry_honors_x_should_retry_and_excessive_delay() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/force"))
            .respond_with(ResponseTemplate::new(400).insert_header("x-should-retry", "true"))
            .up_to_n_times(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/force"))
            .respond_with(ResponseTemplate::new(200).set_body_string("ok"))
            .mount(&server)
            .await;
        let client = reqwest::Client::new();
        let cfg = RetryConfig {
            max_retries: 1,
            initial_delay: Duration::from_millis(1),
            max_delay: Duration::from_millis(1),
            backoff_multiplier: 1.0,
            jitter_fraction: 0.0,
            max_retry_delay_ms: 1000,
        };
        let resp = do_with_retry(&client, client.get(format!("{}/force", server.uri())), &cfg)
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);

        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/no"))
            .respond_with(ResponseTemplate::new(503).insert_header("x-should-retry", "false"))
            .mount(&server)
            .await;
        let resp = do_with_retry(&client, client.get(format!("{}/no", server.uri())), &cfg)
            .await
            .unwrap();
        assert_eq!(resp.status(), 503);

        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/delay"))
            .respond_with(ResponseTemplate::new(503).insert_header("retry-after", "5"))
            .mount(&server)
            .await;
        let err = do_with_retry(&client, client.get(format!("{}/delay", server.uri())), &cfg)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("Server requested 5s retry delay"));
    }

    #[tokio::test(start_paused = true)]
    async fn provider_retry_abort_during_backoff_is_interruptible() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/abort"))
            .respond_with(ResponseTemplate::new(503))
            .mount(&server)
            .await;
        let client = reqwest::Client::new();
        let cfg = RetryConfig {
            max_retries: 1,
            initial_delay: Duration::from_secs(60),
            max_delay: Duration::from_secs(60),
            backoff_multiplier: 1.0,
            jitter_fraction: 0.0,
            max_retry_delay_ms: 0,
        };
        let (tx, rx) = tokio::sync::watch::channel(false);
        let handle = tokio::spawn({
            let client = client.clone();
            let url = format!("{}/abort", server.uri());
            async move { do_with_retry_cancel(&client, client.get(url), &cfg, Some(rx)).await }
        });
        tokio::task::yield_now().await;
        tx.send(true).unwrap();
        let err = handle.await.unwrap().unwrap_err();
        assert_eq!(err.to_string(), "Request aborted");
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
pub type RetryError = Box<dyn std::error::Error + Send + Sync>;

fn boxed_error(message: impl Into<String>) -> RetryError {
    Box::new(std::io::Error::other(message.into()))
}

async fn provider_retry_sleep(
    delay: Duration,
    cancel: Option<&mut tokio::sync::watch::Receiver<bool>>,
) -> Result<(), RetryError> {
    match cancel {
        Some(rx) => {
            if *rx.borrow() {
                return Err(boxed_error("Request aborted"));
            }
            tokio::select! {
                _ = tokio::time::sleep(delay) => Ok(()),
                changed = rx.changed() => {
                    if changed.is_ok() && *rx.borrow() { Err(boxed_error("Request aborted")) } else { Ok(()) }
                }
            }
        }
        None => {
            tokio::time::sleep(delay).await;
            Ok(())
        }
    }
}

fn provider_retry_delay(
    resp: &reqwest::Response,
    attempt: u32,
    config: &RetryConfig,
) -> Result<Duration, RetryError> {
    let delay =
        retry_after_delay(resp.headers()).unwrap_or_else(|| compute_backoff(attempt, config));
    let max = Duration::from_millis(config.max_retry_delay_ms);
    if config.max_retry_delay_ms > 0 && delay > max {
        return Err(boxed_error(format!(
            "Server requested {}s retry delay (max: {}s). HTTP {}",
            delay.as_secs().max(1),
            max.as_secs().max(1),
            resp.status().as_u16()
        )));
    }
    Ok(delay)
}

pub async fn do_with_retry(
    client: &reqwest::Client,
    request_builder: reqwest::RequestBuilder,
    config: &RetryConfig,
) -> Result<reqwest::Response, RetryError> {
    do_with_retry_cancel(client, request_builder, config, None).await
}

pub async fn do_with_retry_cancel(
    _client: &reqwest::Client,
    request_builder: reqwest::RequestBuilder,
    config: &RetryConfig,
    mut cancel: Option<tokio::sync::watch::Receiver<bool>>,
) -> Result<reqwest::Response, RetryError> {
    let mut attempt = 0u32;
    let mut builder = request_builder;

    loop {
        let retry_builder = builder.try_clone();
        match builder.send().await {
            Ok(resp) => {
                if !is_retryable_response(&resp) || attempt >= config.max_retries {
                    return Ok(resp);
                }
                let delay = provider_retry_delay(&resp, attempt, config)?;
                provider_retry_sleep(delay, cancel.as_mut()).await?;
                attempt += 1;
                builder = match retry_builder {
                    Some(b) => b,
                    None => return Ok(resp),
                };
            }
            Err(err) => {
                if attempt >= config.max_retries {
                    return Err(Box::new(err));
                }
                let delay = compute_backoff(attempt, config);
                provider_retry_sleep(delay, cancel.as_mut()).await?;
                attempt += 1;
                builder = match retry_builder {
                    Some(b) => b,
                    None => return Err(Box::new(err)),
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
    ErrPat::Plain("stream ended before a terminal response event"),
    ErrPat::Plain("http2 request did not get a response"),
    ErrPat::Plain("retry delay"),
    ErrPat::Plain("you can retry your request"),
    ErrPat::Plain("try your request again"),
    ErrPat::Plain("please retry your request"),
    // gRPC based providers (e.g. NVIDIA NIM).
    ErrPat::Plain("resourceexhausted"),
];

#[derive(Debug, Clone)]
pub struct AssistantRetryPolicy {
    pub enabled: bool,
    pub max_retries: u32,
    pub base_delay_ms: u64,
}

type RetryFinishedCallback<'a> = Box<dyn FnMut(bool, u32, Option<String>) + Send + 'a>;

pub struct AssistantRetryCallbacks<'a> {
    pub on_retry_finished: Option<RetryFinishedCallback<'a>>,
}

fn emit_retry_finished(
    callbacks: &mut Option<AssistantRetryCallbacks<'_>>,
    success: bool,
    attempt: u32,
    final_error: Option<String>,
) {
    if let Some(cb) = callbacks
        .as_mut()
        .and_then(|c| c.on_retry_finished.as_mut())
    {
        cb(success, attempt, final_error);
    }
}

async fn retry_sleep(
    ms: u64,
    cancel: Option<&mut tokio::sync::watch::Receiver<bool>>,
) -> Result<(), ()> {
    match cancel {
        Some(rx) => {
            if *rx.borrow() {
                return Err(());
            }
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_millis(ms)) => Ok(()),
                changed = rx.changed() => {
                    if changed.is_ok() && *rx.borrow() { Err(()) } else { Ok(()) }
                }
            }
        }
        None => {
            tokio::time::sleep(Duration::from_millis(ms)).await;
            Ok(())
        }
    }
}

pub async fn retry_assistant_call<F, Fut>(
    mut produce: F,
    policy: Option<AssistantRetryPolicy>,
    mut cancel: Option<tokio::sync::watch::Receiver<bool>>,
    mut callbacks: Option<AssistantRetryCallbacks<'_>>,
) -> crate::types::Message
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = crate::types::Message>,
{
    let max_attempts = policy
        .as_ref()
        .filter(|p| p.enabled)
        .map(|p| p.max_retries)
        .unwrap_or(0);
    let base_delay_ms = policy.as_ref().map(|p| p.base_delay_ms).unwrap_or(0);
    let mut attempt = 0u32;
    let mut last_retry: Option<(u32, String)> = None;
    loop {
        let response = produce().await;
        if matches!(
            response.stop_reason,
            Some(crate::types::StopReason::Aborted)
        ) {
            if let Some((last_attempt, _)) = last_retry {
                emit_retry_finished(&mut callbacks, false, last_attempt, None);
            }
            return response;
        }
        if !matches!(response.stop_reason, Some(crate::types::StopReason::Error)) {
            if let Some((last_attempt, _)) = last_retry {
                emit_retry_finished(&mut callbacks, true, last_attempt, None);
            }
            return response;
        }
        if attempt >= max_attempts || !is_retryable_assistant_error(&response) {
            if let Some((last_attempt, _)) = last_retry {
                emit_retry_finished(
                    &mut callbacks,
                    false,
                    last_attempt,
                    response.error_message.clone(),
                );
            }
            return response;
        }
        attempt += 1;
        let error = response
            .error_message
            .clone()
            .unwrap_or_else(|| "Unknown error".into());
        last_retry = Some((attempt, error.clone()));
        let delay = base_delay_ms.saturating_mul(2_u64.saturating_pow(attempt.saturating_sub(1)));
        let sleep_result = retry_sleep(delay, cancel.as_mut()).await;
        if sleep_result.is_err() {
            emit_retry_finished(&mut callbacks, false, attempt, Some(error));
            let mut aborted = response;
            aborted.stop_reason = Some(crate::types::StopReason::Aborted);
            aborted.error_message = None;
            return aborted;
        }
    }
}

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
