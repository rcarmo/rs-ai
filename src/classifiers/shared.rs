//! Shared classifier HTTP requests and billed usage parsing.

use super::ClassifierOptions;
use crate::types::{ClassifierModel, Usage};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde_json::Value;
use std::collections::HashMap;
use std::future::Future;

async fn with_cancel<T>(
    operation: impl Future<Output = Result<T, String>>,
    cancel: Option<tokio::sync::watch::Receiver<bool>>,
) -> Result<T, String> {
    let Some(mut cancel) = cancel else {
        return operation.await;
    };
    if *cancel.borrow() {
        return Err("Request aborted".into());
    }
    tokio::select! {
        biased;
        _ = async {
            loop {
                if *cancel.borrow_and_update() { return; }
                if cancel.changed().await.is_err() { std::future::pending::<()>().await; }
            }
        } => Err("Request aborted".into()),
        result = operation => result,
    }
}

fn headers(model: &ClassifierModel, api_key: &str, options: &ClassifierOptions) -> HeaderMap {
    let mut values = HashMap::from([
        ("authorization".to_string(), format!("Bearer {api_key}")),
        ("content-type".to_string(), "application/json".to_string()),
    ]);
    if let Some(model_headers) = &model.headers {
        values.extend(
            model_headers
                .iter()
                .map(|(key, value)| (key.to_ascii_lowercase(), value.clone())),
        );
    }
    if let Some(overrides) = &options.headers {
        for (key, value) in overrides {
            if let Some(value) = value {
                values.insert(key.to_ascii_lowercase(), value.clone());
            } else {
                values.remove(&key.to_ascii_lowercase());
            }
        }
    }
    let mut result = HeaderMap::new();
    for (key, value) in values {
        if let (Ok(key), Ok(value)) = (
            HeaderName::from_bytes(key.as_bytes()),
            HeaderValue::from_str(&value),
        ) {
            result.insert(key, value);
        }
    }
    result
}

pub(super) async fn post(
    label: &str,
    url: &str,
    model: &ClassifierModel,
    mut body: Value,
    options: &ClassifierOptions,
    no_retry_statuses: &[u16],
) -> Result<Value, String> {
    let api_key = options
        .api_key
        .as_deref()
        .or(model.api_key.as_deref())
        .map(str::to_owned)
        .or_else(|| crate::env::get_env_api_key(&model.provider))
        .filter(|key| !key.is_empty())
        .ok_or_else(|| format!("No API key for provider: {}", model.provider))?;
    if let Some(hook) = &options.on_payload {
        body = hook(body, model).map_err(|error| error.to_string())?;
    }
    let client = crate::http_proxy::client_for_target(url, options.env.as_ref());
    let mut request = client
        .post(url)
        .headers(headers(model, &api_key, options))
        .json(&body);
    if let Some(timeout) = options.timeout {
        request = request.timeout(timeout);
    }
    let config = crate::retry::RetryConfig {
        max_retries: options.max_retries.unwrap_or(2),
        max_retry_delay_ms: options.max_retry_delay_ms.unwrap_or(60_000),
        ..Default::default()
    };
    let response = crate::retry::do_with_retry_cancel_statuses(
        &client,
        request,
        &config,
        options.cancel.clone(),
        no_retry_statuses,
    )
    .await
    .map_err(|error| error.to_string())?;
    let status = response.status().as_u16();
    if let Some(hook) = &options.on_response {
        let headers = response
            .headers()
            .iter()
            .filter_map(|(key, value)| {
                value
                    .to_str()
                    .ok()
                    .map(|value| (key.as_str().to_owned(), value.to_owned()))
            })
            .collect();
        // Observe the HTTP response before parsing/reading its body, including
        // non-success and malformed responses, matching upstream fetch hooks.
        hook(status, &headers, model);
    }
    if !response.status().is_success() {
        let body = with_cancel(
            async { response.text().await.map_err(|error| error.to_string()) },
            options.cancel.clone(),
        )
        .await?;
        if label == "OpenAI Decisions" && status == 504 {
            return Err("OpenAI Decisions error (504): the request timed out at the gateway. Very large inputs (above roughly 600K tokens) currently exceed its time limit.".into());
        }
        return Err(crate::error_body::format_provider_http_error(
            status,
            &body,
            Some(&format!("{label} error")),
        ));
    }
    let body = with_cancel(
        async { response.json().await.map_err(|error| error.to_string()) },
        options.cancel.clone(),
    )
    .await?;
    Ok(body)
}

pub(super) fn parse_usage(value: Option<&Value>, model: &ClassifierModel) -> Option<Usage> {
    let object = value?.as_object()?;
    if !object.contains_key("input_tokens") && !object.contains_key("output_tokens") {
        return None;
    }
    let tokens = |name| {
        object
            .get(name)
            .and_then(Value::as_f64)
            .filter(|number| number.is_finite() && *number > 0.0)
            .unwrap_or(0.0) as u32
    };
    let input = tokens("input_tokens");
    let output = tokens("output_tokens");
    let mut usage = Usage {
        input,
        output,
        total_tokens: input.saturating_add(output),
        ..Default::default()
    };
    usage.cost = crate::simple_options::calculate_cost_from_rates(&model.cost, &usage);
    Some(usage)
}

pub(super) fn number(label: &str, value: Option<&Value>, field: &str) -> Result<f64, String> {
    value
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite())
        .ok_or_else(|| format!("{label} returned an invalid {field}"))
}
