//! OpenAI Decisions classifier transport, including image input and named answers.

use super::{ClassifierOptions, error_result, shared};
use crate::types::{
    ClassifierAnswer, ClassifierContext, ClassifierModel, ClassifierQuestion, ClassifierResult,
    ClassifierStopReason,
};
use serde_json::{Value, json};

const LABEL: &str = "OpenAI Decisions";
const MAX_IMAGES: usize = 128;

fn question(name: &str, question: &ClassifierQuestion) -> Value {
    match question {
        ClassifierQuestion::Choice {
            instructions,
            criteria,
        } => {
            let choices = criteria
                .iter()
                .map(|(value, description)| {
                    if description.is_empty() {
                        json!({"value":value})
                    } else {
                        json!({"value":value,"description":description})
                    }
                })
                .collect::<Vec<_>>();
            json!({"type":"choice","name":name,"instructions":instructions,"choices":choices})
        }
        ClassifierQuestion::Score {
            instructions,
            criteria,
        } => json!({
            "type":"score","name":name,"instructions":instructions,
            "levels":criteria.iter().map(|label| json!({"label":label})).collect::<Vec<_>>()
        }),
        ClassifierQuestion::Bool {
            instructions,
            criteria,
        } => {
            let mut meanings = Vec::new();
            if !criteria.true_value.is_empty() {
                meanings.push(format!("True means: {}", criteria.true_value));
            }
            if !criteria.false_value.is_empty() {
                meanings.push(format!("False means: {}", criteria.false_value));
            }
            let instructions = if meanings.is_empty() {
                instructions.clone()
            } else {
                format!("{instructions}\n\n{}", meanings.join("\n"))
            };
            json!({"type":"predicate","name":name,"instructions":instructions})
        }
    }
}

fn input(context: &ClassifierContext) -> Result<Value, String> {
    let state = serde_json::to_string(&context.state).map_err(|error| error.to_string())?;
    if context.images.is_empty() {
        return Ok(Value::String(state));
    }
    if context.images.len() > MAX_IMAGES {
        return Err(format!(
            "{LABEL} accepts at most {MAX_IMAGES} images, got {}",
            context.images.len()
        ));
    }
    let mut content = vec![json!({"type":"input_text","text":state})];
    content.extend(context.images.iter().map(|image| json!({
        "type":"input_image","image_url":format!("data:{};base64,{}", image.mime_type, image.data)
    })));
    Ok(json!([{"role":"user","content":content}]))
}

fn answers(
    value: Option<&Value>,
    context: &ClassifierContext,
) -> Result<indexmap::IndexMap<String, ClassifierAnswer>, String> {
    let values = value
        .and_then(Value::as_array)
        .ok_or_else(|| format!("{LABEL} returned an unexpected response"))?;
    let by_name = values
        .iter()
        .filter_map(|answer| {
            answer
                .get("name")
                .and_then(Value::as_str)
                .map(|name| (name, answer))
        })
        .collect::<std::collections::HashMap<_, _>>();
    let mut result = indexmap::IndexMap::new();
    for (id, question) in &context.questions {
        let answer = by_name
            .get(id.as_str())
            .ok_or_else(|| format!("{LABEL} did not return an answer for {id}"))?;
        let kind = answer.get("type").and_then(Value::as_str);
        if kind == Some("refusal") {
            return Err(format!("{LABEL} refused to answer {id}"));
        }
        let parsed = match question {
            ClassifierQuestion::Choice { .. } => {
                let choice = answer
                    .get("choice")
                    .and_then(Value::as_str)
                    .filter(|_| kind == Some("choice"))
                    .ok_or_else(|| format!("{LABEL} did not return a choice answer for {id}"))?;
                let values = answer
                    .get("probabilities")
                    .and_then(Value::as_array)
                    .ok_or_else(|| format!("{LABEL} returned invalid probabilities for {id}"))?;
                let mut probabilities = indexmap::IndexMap::new();
                for entry in values {
                    let key = entry.get("value").and_then(Value::as_str).ok_or_else(|| {
                        format!("{LABEL} returned invalid probabilities for {id}")
                    })?;
                    probabilities.insert(
                        key.to_owned(),
                        shared::number(
                            LABEL,
                            entry.get("probability"),
                            &format!("probability for {id}.{key}"),
                        )?,
                    );
                }
                ClassifierAnswer::Choice {
                    choice: choice.into(),
                    probabilities,
                    confidence: shared::number(
                        LABEL,
                        answer.get("confidence"),
                        &format!("confidence for {id}"),
                    )?,
                }
            }
            ClassifierQuestion::Score { .. } => {
                if kind != Some("score") {
                    return Err(format!("{LABEL} did not return a score answer for {id}"));
                }
                ClassifierAnswer::Score {
                    score: shared::number(LABEL, answer.get("score"), &format!("score for {id}"))?,
                    confidence: shared::number(
                        LABEL,
                        answer.get("confidence"),
                        &format!("confidence for {id}"),
                    )?,
                }
            }
            ClassifierQuestion::Bool { .. } => {
                if kind != Some("predicate") {
                    return Err(format!(
                        "{LABEL} did not return a predicate answer for {id}"
                    ));
                }
                ClassifierAnswer::Bool {
                    probability: shared::number(
                        LABEL,
                        answer.get("probability"),
                        &format!("probability for {id}"),
                    )?,
                }
            }
        };
        result.insert(id.clone(), parsed);
    }
    Ok(result)
}

pub async fn classify_openai_decisions(
    model: &ClassifierModel,
    context: &ClassifierContext,
    options: &ClassifierOptions,
) -> ClassifierResult {
    if model.api != crate::types::api::OPENAI_DECISIONS {
        return error_result(
            model,
            format!("Unsupported classifier API: {}", model.api),
            false,
        );
    }
    let input = match input(context) {
        Ok(value) => value,
        Err(error) => return error_result(model, error, false),
    };
    let url = format!("{}/decisions", model.base_url.trim_end_matches('/'));
    let body = json!({"model":model.id,"input":input,"questions":context.questions.iter().map(|(name,q)| question(name,q)).collect::<Vec<_>>()});
    let body = match shared::post(LABEL, &url, model, body, options, &[504]).await {
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
    let mut result = ClassifierResult {
        api: model.api.clone(),
        provider: model.provider.clone(),
        model: model.id.clone(),
        answers: indexmap::IndexMap::new(),
        usage: shared::parse_usage(body.get("usage"), model),
        stop_reason: ClassifierStopReason::Stop,
        error_message: None,
        timestamp: crate::utils::now_millis(),
    };
    match answers(body.get("answers"), context) {
        Ok(answers) => result.answers = answers,
        Err(error) => {
            result.stop_reason = ClassifierStopReason::Error;
            result.error_message = Some(error);
        }
    }
    result
}
