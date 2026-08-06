//! OpenRouter image generation provider.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue};
use serde_json::{Value, json};

use super::types::*;
use crate::env::get_env_api_key;
use crate::types::{CostBreakdown, StopReason, TelemetryContext, Usage};

/// Hook to inspect/modify the image request payload before sending (mirrors onPayload).
pub type ImagesPayloadHook = Arc<
    dyn Fn(Value, &ImagesModel) -> Result<Value, Box<dyn std::error::Error + Send + Sync>>
        + Send
        + Sync,
>;
/// Hook invoked with the HTTP status and headers of the image response (mirrors onResponse).
pub type ImagesResponseHook =
    Arc<dyn Fn(u16, &HashMap<String, String>, &ImagesModel) + Send + Sync>;

/// Options for image generation.
#[derive(Clone, Default)]
pub struct ImagesOptions {
    pub api_key: Option<String>,
    pub headers: Option<HashMap<String, String>>,
    pub timeout: Option<Duration>,
    pub max_retries: u32,
    pub max_retry_delay_ms: u64,
    pub telemetry_context: Option<TelemetryContext>,
    pub on_payload: Option<ImagesPayloadHook>,
    pub on_response: Option<ImagesResponseHook>,
}

impl std::fmt::Debug for ImagesOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImagesOptions")
            .field("api_key", &self.api_key.as_ref().map(|_| "<redacted>"))
            .field("headers", &self.headers)
            .field("timeout", &self.timeout)
            .field("max_retries", &self.max_retries)
            .field("max_retry_delay_ms", &self.max_retry_delay_ms)
            .field("telemetry_context", &self.telemetry_context)
            .field("on_payload", &self.on_payload.as_ref().map(|_| "<hook>"))
            .field("on_response", &self.on_response.as_ref().map(|_| "<hook>"))
            .finish()
    }
}

/// Generate images via OpenRouter.
pub async fn generate_openrouter(
    model: &ImagesModel,
    context: &ImagesContext,
    opts: &ImagesOptions,
) -> AssistantImages {
    let mut out = AssistantImages {
        api: model.api.clone(),
        provider: model.provider.clone(),
        model: model.id.clone(),
        output: Vec::new(),
        stop_reason: StopReason::Stop,
        timestamp: chrono_timestamp(),
        response_id: None,
        usage: None,
        error_message: None,
    };

    let api_key = opts
        .api_key
        .clone()
        .or_else(|| get_env_api_key(&model.provider));
    let api_key = match api_key {
        Some(k) => k,
        None => {
            out.stop_reason = StopReason::Error;
            out.error_message = Some(format!("No API key for provider: {}", model.provider));
            return out;
        }
    };

    if context.input.is_empty() {
        out.stop_reason = StopReason::Error;
        out.error_message = Some("image context has no inputs".into());
        return out;
    }

    let mut payload = build_payload(model, context);
    // onPayload hook: allow the caller to inspect/replace the request body.
    if let Some(ref hook) = opts.on_payload {
        match hook(payload.clone(), model) {
            Ok(next) => payload = next,
            Err(e) => {
                out.stop_reason = StopReason::Error;
                out.error_message = Some(e.to_string());
                return out;
            }
        }
    }
    let body = match serde_json::to_vec(&payload) {
        Ok(b) => b,
        Err(e) => {
            out.stop_reason = StopReason::Error;
            out.error_message = Some(e.to_string());
            return out;
        }
    };

    let url = format!("{}/chat/completions", model.base_url.trim_end_matches('/'));
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    headers.insert(
        AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {}", api_key)).unwrap(),
    );

    if let Some(ref extra) = opts.headers {
        for (k, v) in extra {
            if let (Ok(name), Ok(val)) = (
                reqwest::header::HeaderName::from_bytes(k.as_bytes()),
                HeaderValue::from_str(v),
            ) {
                headers.insert(name, val);
            }
        }
    }

    let client = reqwest::Client::new();
    let mut last_err = String::new();

    for attempt in 0..=opts.max_retries {
        let resp = client
            .post(&url)
            .headers(headers.clone())
            .body(body.clone())
            .send()
            .await;

        let resp = match resp {
            Ok(r) => r,
            Err(e) => {
                last_err = e.to_string();
                if attempt < opts.max_retries {
                    tokio::time::sleep(Duration::from_millis(250 * (attempt as u64 + 1))).await;
                    continue;
                }
                out.stop_reason = StopReason::Error;
                out.error_message = Some(last_err);
                return out;
            }
        };

        let status = resp.status().as_u16();
        if crate::retry::is_retryable_status(status) {
            last_err = format!("HTTP {}", status);
            if attempt < opts.max_retries {
                tokio::time::sleep(Duration::from_millis(250 * (attempt as u64 + 1))).await;
                continue;
            }
            out.stop_reason = StopReason::Error;
            out.error_message = Some(last_err);
            return out;
        }

        if status >= 300 {
            let body_text = resp.text().await.unwrap_or_default();
            out.stop_reason = StopReason::Error;
            out.error_message = Some(crate::error_body::format_provider_http_error(
                status, &body_text, None,
            ));
            return out;
        }

        // Upstream fires onResponse once, after a successful response (the SDK's
        // withResponse() resolves only on success after its internal retries), so call
        // it here — not on retryable/error attempts.
        if let Some(ref hook) = opts.on_response {
            let hdrs: HashMap<String, String> = resp
                .headers()
                .iter()
                .filter_map(|(k, v)| {
                    v.to_str()
                        .ok()
                        .map(|s| (k.as_str().to_string(), s.to_string()))
                })
                .collect();
            hook(status, &hdrs, model);
        }

        let raw: Value = match resp.json().await {
            Ok(v) => v,
            Err(e) => {
                out.stop_reason = StopReason::Error;
                out.error_message = Some(e.to_string());
                return out;
            }
        };

        parse_response(&raw, model, &mut out);
        return out;
    }

    out.stop_reason = StopReason::Error;
    out.error_message = Some(last_err);
    out
}

fn build_payload(model: &ImagesModel, context: &ImagesContext) -> Value {
    let content: Vec<Value> = context
        .input
        .iter()
        .map(|input| match input {
            ImageInput::Text { text } => json!({"type": "text", "text": text}),
            ImageInput::Image { data, mime_type } => json!({
                "type": "image_url",
                "image_url": {"url": format!("data:{};base64,{}", mime_type, data)}
            }),
        })
        .collect();

    let modalities: Vec<&str> = if model.output.iter().any(|o| o == "text") {
        vec!["image", "text"]
    } else {
        vec!["image"]
    };

    json!({
        "model": model.id,
        "messages": [{"role": "user", "content": content}],
        "stream": false,
        "modalities": modalities,
    })
}

fn parse_response(raw: &Value, model: &ImagesModel, out: &mut AssistantImages) {
    // Surface in-band error objects (some providers return 200 with an error body).
    if let Some(err) = raw.get("error") {
        let msg = err
            .get("message")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| err.to_string());
        out.stop_reason = StopReason::Error;
        out.error_message = Some(msg);
        return;
    }
    if let Some(id) = raw.get("id").and_then(|v| v.as_str()) {
        out.response_id = Some(id.to_string());
    }

    if let Some(usage) = raw.get("usage") {
        out.usage = Some(parse_usage(usage, model));
    }

    if let Some(choices) = raw.get("choices").and_then(|v| v.as_array())
        && let Some(choice) = choices.first()
        && let Some(msg) = choice.get("message")
    {
        if let Some(text) = msg.get("content").and_then(|v| v.as_str())
            && !text.is_empty()
        {
            out.output.push(ImageOutput::Text {
                text: text.to_string(),
            });
        }
        if let Some(images) = msg.get("images").and_then(|v| v.as_array()) {
            for img in images {
                let url = img
                    .get("image_url")
                    .and_then(|v| v.as_object())
                    .and_then(|o| o.get("url"))
                    .and_then(|v| v.as_str())
                    .or_else(|| img.get("image_url").and_then(|v| v.as_str()));
                if let Some(u) = url
                    && let Some(rest) = u.strip_prefix("data:")
                    && let Some((mime, data)) = rest.split_once(";base64,")
                {
                    out.output.push(ImageOutput::Image {
                        data: data.to_string(),
                        mime_type: mime.to_string(),
                    });
                }
            }
        }
    }
}

fn parse_usage(raw: &Value, model: &ImagesModel) -> Usage {
    let prompt = raw
        .get("prompt_tokens")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;
    let completion = raw
        .get("completion_tokens")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;
    let cached = raw
        .pointer("/prompt_tokens_details/cached_tokens")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;
    let cache_write = raw
        .pointer("/prompt_tokens_details/cache_write_tokens")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;
    let cache_read = if cache_write > 0 {
        cached.saturating_sub(cache_write)
    } else {
        cached
    };
    let input = prompt.saturating_sub(cache_read + cache_write);

    let m = 1_000_000.0;
    let cost = CostBreakdown {
        input: f64::from(input) * model.cost.input / m,
        output: f64::from(completion) * model.cost.output / m,
        cache_read: f64::from(cache_read) * model.cost.cache_read / m,
        cache_write: f64::from(cache_write) * model.cost.cache_write / m,
        total: 0.0,
    };
    let total = cost.input + cost.output + cost.cache_read + cost.cache_write;
    Usage {
        input,
        output: completion,
        cache_read,
        cache_write,
        cache_write_1h: None,
        reasoning: None,
        total_tokens: input + completion + cache_read + cache_write,
        cost: CostBreakdown { total, ..cost },
    }
}

fn chrono_timestamp() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ModelCost;

    fn img_model() -> ImagesModel {
        ImagesModel {
            id: "m".into(),
            name: "M".into(),
            api: "openrouter-images".into(),
            provider: "openrouter".into(),
            base_url: "https://example.com".into(),
            input: vec!["text".into()],
            output: vec!["image".into()],
            cost: ModelCost {
                input: 3.0,
                output: 15.0,
                cache_read: 0.3,
                cache_write: 0.0,
                tiers: vec![],
            },
        }
    }

    #[test]
    fn test_parse_usage_computes_total_cost() {
        let raw = serde_json::json!({
            "prompt_tokens": 1000, "completion_tokens": 200,
            "prompt_tokens_details": { "cached_tokens": 400 }
        });
        let usage = parse_usage(&raw, &img_model());
        assert_eq!(usage.cache_read, 400);
        assert_eq!(usage.input, 600);
        // total must be non-zero and equal the sum of components
        let expected =
            usage.cost.input + usage.cost.output + usage.cost.cache_read + usage.cost.cache_write;
        assert!(usage.cost.total > 0.0);
        assert!((usage.cost.total - expected).abs() < 1e-9);
    }

    #[test]
    fn test_parse_response_surfaces_error() {
        let raw = serde_json::json!({ "error": { "message": "content policy" } });
        let mut out = AssistantImages {
            api: "openrouter-images".into(),
            provider: "openrouter".into(),
            model: "m".into(),
            output: Vec::new(),
            stop_reason: StopReason::Stop,
            timestamp: 0,
            response_id: None,
            usage: None,
            error_message: None,
        };
        parse_response(&raw, &img_model(), &mut out);
        assert_eq!(out.stop_reason, StopReason::Error);
        assert_eq!(out.error_message.as_deref(), Some("content policy"));
    }

    #[tokio::test]
    async fn test_on_payload_hook_modifies_request() {
        use wiremock::matchers::{body_string_contains, method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .and(body_string_contains("\"injected\":true"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "img-1",
                "choices": [{"message": {"content": "ok", "images": []}}]
            })))
            .mount(&server)
            .await;
        let mut model = img_model();
        model.base_url = server.uri();
        let ctx = ImagesContext {
            input: vec![ImageInput::Text {
                text: "draw".into(),
            }],
        };
        let hook: ImagesPayloadHook = Arc::new(|mut p: Value, _m: &ImagesModel| {
            p["injected"] = serde_json::json!(true);
            Ok(p)
        });
        let opts = ImagesOptions {
            api_key: Some("k".into()),
            on_payload: Some(hook),
            ..Default::default()
        };
        let out = generate_openrouter(&model, &ctx, &opts).await;
        // The mock only matches when the injected field is present, so a non-error
        // result confirms the hook's modified payload was actually sent.
        assert_eq!(out.stop_reason, StopReason::Stop);
        assert_eq!(out.response_id.as_deref(), Some("img-1"));
    }
}
