//! TypeSafe and Cloudflare System One classifier transports.

use super::{ClassifierOptions, error_result};
use crate::types::{
    ClassifierAnswer, ClassifierContext, ClassifierModel, ClassifierQuestion, ClassifierResult,
    ClassifierStopReason, CostBreakdown, Usage,
};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde_json::{Map, Value, json};
use std::collections::HashMap;
use std::time::Duration;

#[derive(Clone, Copy)]
enum Transport {
    TypeSafe,
    Cloudflare,
}

impl Transport {
    fn api(self) -> &'static str {
        match self {
            Self::TypeSafe => crate::types::api::TYPESAFE_SYSTEM_ONE,
            Self::Cloudflare => crate::types::api::CLOUDFLARE_WORKERS_AI_SYSTEM_ONE,
        }
    }
    fn label(self) -> &'static str {
        match self {
            Self::TypeSafe => "System One API",
            Self::Cloudflare => "Cloudflare Workers AI",
        }
    }
}

pub async fn classify_typesafe(
    model: &ClassifierModel,
    context: &ClassifierContext,
    options: &ClassifierOptions,
) -> ClassifierResult {
    classify_system_one(Transport::TypeSafe, model, context, options).await
}

pub async fn classify_cloudflare(
    model: &ClassifierModel,
    context: &ClassifierContext,
    options: &ClassifierOptions,
) -> ClassifierResult {
    classify_system_one(Transport::Cloudflare, model, context, options).await
}

fn wire_questions(context: &ClassifierContext) -> Value {
    let mut questions = Map::new();
    for (id, question) in &context.questions {
        let mut value = serde_json::to_value(question).unwrap_or(Value::Null);
        if matches!(question, ClassifierQuestion::Bool { .. }) {
            value["type"] = json!("noul");
        }
        questions.insert(id.clone(), value);
    }
    Value::Object(questions)
}

fn resolved_cloudflare_url(
    model: &ClassifierModel,
    options: &ClassifierOptions,
) -> Result<String, String> {
    let account = options
        .env
        .as_ref()
        .and_then(|values| values.get("CLOUDFLARE_ACCOUNT_ID"))
        .cloned()
        .or_else(|| std::env::var("CLOUDFLARE_ACCOUNT_ID").ok())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "Missing CLOUDFLARE_ACCOUNT_ID".to_string())?;
    Ok(format!(
        "{}/run",
        model
            .base_url
            .replace("{CLOUDFLARE_ACCOUNT_ID}", &account)
            .trim_end_matches('/')
    ))
}

fn request_url(
    transport: Transport,
    model: &ClassifierModel,
    options: &ClassifierOptions,
) -> Result<String, String> {
    match transport {
        Transport::TypeSafe => Ok(format!(
            "{}/systemone",
            model.base_url.trim_end_matches('/')
        )),
        Transport::Cloudflare => resolved_cloudflare_url(model, options),
    }
}

fn payload(transport: Transport, model: &ClassifierModel, context: &ClassifierContext) -> Value {
    let request = json!({
        "state": context.state,
        "questions": wire_questions(context),
    });
    match transport {
        Transport::TypeSafe => {
            let mut object = request.as_object().cloned().unwrap_or_default();
            object.insert("model".into(), json!(model.id));
            Value::Object(object)
        }
        Transport::Cloudflare => json!({"model": model.id, "input": request}),
    }
}

fn request_headers(
    model: &ClassifierModel,
    api_key: &str,
    options: &ClassifierOptions,
) -> HeaderMap {
    let mut merged = HashMap::<String, String>::new();
    merged.insert("authorization".into(), format!("Bearer {api_key}"));
    merged.insert("content-type".into(), "application/json".into());
    if let Some(headers) = &model.headers {
        for (key, value) in headers {
            merged.insert(key.to_ascii_lowercase(), value.clone());
        }
    }
    if let Some(headers) = &options.headers {
        for (key, value) in headers {
            let key = key.to_ascii_lowercase();
            if let Some(value) = value {
                merged.insert(key, value.clone());
            } else {
                merged.remove(&key);
            }
        }
    }
    let mut result = HeaderMap::new();
    for (key, value) in merged {
        if let (Ok(key), Ok(value)) = (
            HeaderName::from_bytes(key.as_bytes()),
            HeaderValue::from_str(&value),
        ) {
            result.insert(key, value);
        }
    }
    result
}

fn classifier_usage(value: &Value, model: &ClassifierModel) -> Option<Usage> {
    let object = value.as_object()?;
    if !object.contains_key("input_tokens") && !object.contains_key("output_tokens") {
        return None;
    }
    let input = object
        .get("input_tokens")
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite() && *value > 0.0)
        .unwrap_or(0.0) as u32;
    let output = object
        .get("output_tokens")
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite() && *value > 0.0)
        .unwrap_or(0.0) as u32;
    let million = 1_000_000.0;
    let input_cost = f64::from(input) * model.cost.input / million;
    let output_cost = f64::from(output) * model.cost.output / million;
    Some(Usage {
        input,
        output,
        cache_read: 0,
        cache_write: 0,
        cache_write_1h: None,
        reasoning: None,
        total_tokens: input + output,
        cost: CostBreakdown {
            input: input_cost,
            output: output_cost,
            cache_read: 0.0,
            cache_write: 0.0,
            total: input_cost + output_cost,
        },
    })
}

fn number(label: &str, value: Option<&Value>, field: &str) -> Result<f64, String> {
    value
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite())
        .ok_or_else(|| format!("{label} returned an invalid {field}"))
}

fn parse_answers(
    label: &str,
    value: &Value,
    context: &ClassifierContext,
) -> Result<indexmap::IndexMap<String, ClassifierAnswer>, String> {
    let object = value
        .as_object()
        .ok_or_else(|| format!("{label} returned an unexpected response"))?;
    let mut answers = indexmap::IndexMap::new();
    for (id, question) in &context.questions {
        let answer = object
            .get(id)
            .and_then(Value::as_object)
            .ok_or_else(|| format!("{label} did not return an answer for {id}"))?;
        let parsed = match question {
            ClassifierQuestion::Choice { .. } => {
                if answer.get("type").and_then(Value::as_str) != Some("choice") {
                    return Err(format!("{label} did not return a choice answer for {id}"));
                }
                let choice = answer
                    .get("choice")
                    .and_then(Value::as_str)
                    .ok_or_else(|| format!("{label} did not return a choice answer for {id}"))?;
                let probabilities = answer
                    .get("probabilities")
                    .and_then(Value::as_object)
                    .ok_or_else(|| format!("{label} returned invalid probabilities for {id}"))?
                    .iter()
                    .map(|(key, value)| {
                        number(label, Some(value), &format!("probability for {id}.{key}"))
                            .map(|probability| (key.clone(), probability))
                    })
                    .collect::<Result<indexmap::IndexMap<_, _>, _>>()?;
                ClassifierAnswer::Choice {
                    choice: choice.to_string(),
                    probabilities,
                    confidence: number(
                        label,
                        answer.get("confidence"),
                        &format!("confidence for {id}"),
                    )?,
                }
            }
            ClassifierQuestion::Score { .. } => {
                if answer.get("type").and_then(Value::as_str) != Some("score") {
                    return Err(format!("{label} did not return a score answer for {id}"));
                }
                ClassifierAnswer::Score {
                    score: number(label, answer.get("score"), &format!("score for {id}"))?,
                    confidence: number(
                        label,
                        answer.get("confidence"),
                        &format!("confidence for {id}"),
                    )?,
                }
            }
            ClassifierQuestion::Bool { .. } => {
                if answer.get("type").and_then(Value::as_str) != Some("noul") {
                    return Err(format!("{label} did not return a bool answer for {id}"));
                }
                ClassifierAnswer::Bool {
                    probability: number(
                        label,
                        answer.get("noul"),
                        &format!("probability for {id}"),
                    )?,
                }
            }
        };
        answers.insert(id.clone(), parsed);
    }
    Ok(answers)
}

fn output_value(transport: Transport, body: Value) -> Result<Value, String> {
    match transport {
        Transport::TypeSafe => {
            if body.is_object() {
                Ok(body)
            } else {
                Err("System One API returned an unexpected response".into())
            }
        }
        Transport::Cloudflare => {
            let object = body.as_object().ok_or_else(|| {
                "Cloudflare Workers AI returned an unexpected response".to_string()
            })?;
            if object.get("success") == Some(&Value::Bool(false)) {
                let message = object
                    .get("errors")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(|error| error.get("message").and_then(Value::as_str))
                    .collect::<Vec<_>>()
                    .join("; ");
                return Err(if message.is_empty() {
                    "Cloudflare Workers AI request failed".into()
                } else {
                    format!("Cloudflare Workers AI error: {message}")
                });
            }
            let run = object
                .get("result")
                .and_then(Value::as_object)
                .ok_or_else(|| {
                    "Cloudflare Workers AI returned an unexpected response".to_string()
                })?;
            if run.get("state").and_then(Value::as_str) != Some("Completed") {
                return Err(format!(
                    "Cloudflare Workers AI run did not complete (state: {})",
                    run.get("state")
                        .map_or_else(|| "undefined".into(), Value::to_string)
                ));
            }
            run.get("result")
                .filter(|value| value.is_object())
                .cloned()
                .ok_or_else(|| "Cloudflare Workers AI returned an unexpected response".to_string())
        }
    }
}

async fn classify_system_one(
    transport: Transport,
    model: &ClassifierModel,
    context: &ClassifierContext,
    options: &ClassifierOptions,
) -> ClassifierResult {
    let mut result = ClassifierResult {
        api: model.api.clone(),
        provider: model.provider.clone(),
        model: model.id.clone(),
        answers: indexmap::IndexMap::new(),
        usage: None,
        stop_reason: ClassifierStopReason::Stop,
        error_message: None,
        timestamp: crate::utils::now_millis(),
    };
    if model.api != transport.api() {
        return error_result(
            model,
            format!("Unsupported classifier API: {}", model.api),
            false,
        );
    }
    let api_key = options
        .api_key
        .clone()
        .or_else(|| model.api_key.clone())
        .or_else(|| crate::env::get_env_api_key(&model.provider));
    let Some(api_key) = api_key else {
        return error_result(
            model,
            format!("No API key for provider: {}", model.provider),
            false,
        );
    };
    let url = match request_url(transport, model, options) {
        Ok(url) => url,
        Err(error) => return error_result(model, error, false),
    };
    let mut body = payload(transport, model, context);
    if let Some(hook) = &options.on_payload {
        match hook(body.clone(), model) {
            Ok(next) => body = next,
            Err(error) => return error_result(model, error, false),
        }
    }
    let headers = request_headers(model, &api_key, options);
    let client = crate::http_proxy::client_for_target(&url, options.env.as_ref());
    let attempts = options.max_retries.unwrap_or(2);
    let mut last_error = String::new();
    for attempt in 0..=attempts {
        if options
            .cancel
            .as_ref()
            .is_some_and(|cancel| *cancel.borrow())
        {
            return error_result(model, "Request aborted", true);
        }
        let request = client.post(&url).headers(headers.clone()).json(&body);
        let response = if let Some(timeout) = options.timeout {
            match tokio::time::timeout(timeout, request.send()).await {
                Ok(Ok(response)) => response,
                Ok(Err(error)) => {
                    last_error = error.to_string();
                    if attempt < attempts {
                        tokio::time::sleep(Duration::from_millis(10)).await;
                        continue;
                    }
                    return error_result(model, last_error, false);
                }
                Err(_) => {
                    last_error = format!("Request timed out after {}ms", timeout.as_millis());
                    if attempt < attempts {
                        continue;
                    }
                    return error_result(model, last_error, false);
                }
            }
        } else {
            match request.send().await {
                Ok(response) => response,
                Err(error) => {
                    last_error = error.to_string();
                    if attempt < attempts {
                        tokio::time::sleep(Duration::from_millis(10)).await;
                        continue;
                    }
                    return error_result(model, last_error, false);
                }
            }
        };
        let status = response.status().as_u16();
        if crate::retry::is_retryable_status(status) && attempt < attempts {
            let delay = crate::retry::retry_after_delay(response.headers())
                .unwrap_or_else(|| Duration::from_millis(10));
            let cap = options.max_retry_delay_ms.unwrap_or(u64::MAX);
            tokio::time::sleep(delay.min(Duration::from_millis(cap))).await;
            continue;
        }
        if !response.status().is_success() {
            let text = response.text().await.unwrap_or_default();
            return error_result(
                model,
                crate::error_body::format_provider_http_error(status, &text, None),
                false,
            );
        }
        if let Some(hook) = &options.on_response {
            let response_headers = response
                .headers()
                .iter()
                .filter_map(|(key, value)| {
                    value
                        .to_str()
                        .ok()
                        .map(|value| (key.as_str().to_string(), value.to_string()))
                })
                .collect();
            hook(status, &response_headers, model);
        }
        let response_body: Value = match response.json().await {
            Ok(value) => value,
            Err(error) => return error_result(model, error, false),
        };
        let output = match output_value(transport, response_body) {
            Ok(value) => value,
            Err(error) => return error_result(model, error, false),
        };
        result.usage = classifier_usage(output.get("usage").unwrap_or(&Value::Null), model);
        result.answers = match parse_answers(
            transport.label(),
            output.get("answers").unwrap_or(&Value::Null),
            context,
        ) {
            Ok(answers) => answers,
            Err(error) => {
                result.stop_reason = ClassifierStopReason::Error;
                result.error_message = Some(error);
                return result;
            }
        };
        return result;
    }
    error_result(model, last_error, false)
}
