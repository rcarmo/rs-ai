//! TypeSafe and Cloudflare System One classifier transports.

use super::{ClassifierOptions, error_result};
use crate::types::{
    ClassifierAnswer, ClassifierContext, ClassifierModel, ClassifierQuestion, ClassifierResult,
    ClassifierStopReason,
};
use serde_json::{Map, Value, json};

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
            if run.contains_key("answers") {
                return Ok(Value::Object(run.clone()));
            }
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
    if !context.images.is_empty() {
        return error_result(
            model,
            format!("{} does not support image input", transport.label()),
            false,
        );
    }
    let url = match request_url(transport, model, options) {
        Ok(url) => url,
        Err(error) => return error_result(model, error, false),
    };
    let response_body = match super::shared::post(
        transport.label(),
        &url,
        model,
        payload(transport, model, context),
        options,
        &[],
    )
    .await
    {
        Ok(body) => body,
        Err(error) => {
            return error_result(
                model,
                error,
                options
                    .cancel
                    .as_ref()
                    .is_some_and(|cancel| *cancel.borrow()),
            );
        }
    };
    let output = match output_value(transport, response_body) {
        Ok(value) => value,
        Err(error) => return error_result(model, error, false),
    };
    result.usage = super::shared::parse_usage(output.get("usage"), model);
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
    result
}
