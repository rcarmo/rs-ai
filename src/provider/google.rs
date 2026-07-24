//! Google Generative AI (Gemini) provider.

use std::sync::Arc;

use futures::{StreamExt, stream};
use reqwest::header::{CONTENT_TYPE, HeaderMap, HeaderValue};
use serde_json::{Value, json};

use crate::env::resolve_api_key;
use crate::events::Event;
use crate::transports::sse;
use crate::types::*;

/// Start a Google Generative AI stream.
pub fn stream_google<'a>(
    model: &'a Model,
    context: &'a Context,
    opts: &'a StreamOptions,
) -> std::pin::Pin<Box<dyn futures::Stream<Item = Event> + Send + 'a>> {
    let api_key = resolve_api_key(model, opts);
    if api_key.is_none() {
        let err = Event::Error {
            reason: StopReason::Error,
            error: Arc::from(Box::<dyn std::error::Error + Send + Sync>::from(format!(
                "No API key for provider: {}",
                model.provider
            ))),
            message: None,
        };
        return Box::pin(stream::once(async { err }));
    }
    let api_key = api_key.unwrap();

    let mut payload = build_google_payload(model, context, opts);
    if let Some(ref hook) = opts.on_payload {
        match hook(payload.clone(), model) {
            Ok(next) => payload = next,
            Err(err) => {
                let err = Event::Error {
                    reason: StopReason::Error,
                    error: Arc::from(err),
                    message: None,
                };
                return Box::pin(stream::once(async { err }));
            }
        }
    }
    let url = match build_stream_url(model, &api_key, opts) {
        Ok(u) => u,
        Err(e) => {
            let err = Event::Error {
                reason: StopReason::Error,
                error: Arc::from(Box::<dyn std::error::Error + Send + Sync>::from(e)),
                message: None,
            };
            return Box::pin(stream::once(async { err }));
        }
    };

    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    headers.insert("accept", HeaderValue::from_static("text/event-stream"));
    // Merge model-level and option headers (mirrors `{ ...model.headers, ...optionsHeaders }`).
    for source in [model.headers.as_ref(), opts.headers.as_ref()]
        .into_iter()
        .flatten()
    {
        for (k, v) in source {
            if let (Ok(name), Ok(val)) = (
                reqwest::header::HeaderName::from_bytes(k.as_bytes()),
                HeaderValue::from_str(v),
            ) {
                headers.insert(name, val);
            }
        }
    }

    Box::pin(async_stream::stream! {
        let client = crate::http_proxy::client_for_target(&url, None);
        let request = client.post(&url).headers(headers).json(&payload);
        let request = if let Some(ms) = opts.timeout_ms {
            request.timeout(std::time::Duration::from_millis(ms))
        } else { request };
        let retry_cfg = crate::retry::retry_config_from_options(opts);
        let resp = crate::retry::do_with_retry(&client, request, &retry_cfg).await;

        let resp = match resp {
            Ok(r) => r,
            Err(e) => {
                yield Event::Error {
                    reason: StopReason::Error,
                    error: Arc::from(e),
                    message: None,
                };
                return;
            }
        };

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            yield Event::Error {
                reason: StopReason::Error,
                error: Arc::from(Box::<dyn std::error::Error + Send + Sync>::from(
                    crate::error_body::format_provider_http_error(status, &body, None),
                )),
                message: None,
            };
            return;
        }

        let mut partial = Message {
            role: Role::Assistant,
            content: Vec::new(),
            timestamp: crate::utils::now_millis(),
            api: Some(model.api.clone()),
            provider: Some(model.provider.clone()),
            model: Some(model.id.clone()),
            response_id: None,
            response_model: None,
            diagnostics: Vec::new(),
            usage: None,
            stop_reason: None,
            error_message: None,
            tool_call_id: None,
            tool_name: None,
            is_error: false,
            details: None,
            added_tool_names: Vec::new(),
        };

        yield Event::Start { partial: partial.clone() };

        let mut parser = sse::SseParser::default();
        let mut byte_stream = resp.bytes_stream();

        let mut current_text = String::new();
        let mut current_text_signature: Option<String> = None;
        let mut current_thinking = String::new();
        let mut current_thinking_signature: Option<String> = None;
        // Streaming block state: 0 = none, 1 = text, 2 = thinking. Blocks are finalized
        // (pushed to content) in streaming order on type transitions / tool calls / end,
        // mirroring upstream's per-block assembly (preserves interleaving).
        let mut block_kind: u8 = 0;
        // Tool-call ids seen so far (for uniqueness, count, and stop-reason).
        let mut tool_call_ids: Vec<String> = Vec::new();

        while let Some(chunk_result) = byte_stream.next().await {
            let chunk_bytes = match chunk_result {
                Ok(c) => c,
                Err(e) => {
                    // Mid-stream network/body error: record it and break so the after-loop
                    // block finalization assembles the in-progress block before the terminal
                    // Error event.
                    partial.stop_reason = Some(StopReason::Error);
                    partial.error_message = Some(e.to_string());
                    break;
                }
            };

            for evt in parser.feed_bytes(&chunk_bytes) {
                if evt.event == sse::EVENT_ERROR {
                    yield Event::Error {
                        reason: StopReason::Error,
                        error: Arc::from(Box::<dyn std::error::Error + Send + Sync>::from(
                            format!("SSE error: {}", evt.data),
                        )),
                        message: Some(partial.clone()),
                    };
                    return;
                }

                let chunk: Value = match serde_json::from_str(&evt.data) {
                    Ok(v) => v,
                    Err(_) => continue,
                };

                if let Some(err) = chunk.get("error") {
                    let msg = err.get("message").and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| err.to_string());
                    partial.stop_reason = Some(StopReason::Error);
                    partial.error_message = Some(msg.clone());
                    yield Event::Error {
                        reason: StopReason::Error,
                        error: Arc::from(Box::<dyn std::error::Error + Send + Sync>::from(msg)),
                        message: Some(partial.clone()),
                    };
                    return;
                }

                if partial.response_id.is_none()
                    && let Some(rid) = chunk.get("responseId").and_then(|v| v.as_str()) {
                    partial.response_id = Some(rid.to_string());
                }
                if let Some(candidates) = chunk.get("candidates").and_then(|v| v.as_array()) {
                    for candidate in candidates {
                        if let Some(parts) = candidate.pointer("/content/parts").and_then(|v| v.as_array()) {
                            for part in parts {
                                let is_thought = is_thinking_part(part);
                                let part_sig = part.get("thoughtSignature").and_then(|v| v.as_str()).filter(|s| !s.is_empty());
                                // Upstream processes any part whose `text` field is defined,
                                // including empty strings, so a trailing empty-text delta still
                                // contributes its thoughtSignature to the current block.
                                if let Some(text) = part.get("text").and_then(|v| v.as_str()) {
                                    let want: u8 = if is_thought { 2 } else { 1 };
                                    if block_kind != want {
                                        // Finalize the previous block in order.
                                        match block_kind {
                                            1 => {
                                                yield Event::TextEnd;
                                                partial.content.push(ContentBlock::Text {
                                                    text: std::mem::take(&mut current_text),
                                                    text_signature: current_text_signature.take(),
                                                });
                                            }
                                            2 => {
                                                yield Event::ThinkingEnd;
                                                partial.content.push(ContentBlock::Thinking {
                                                    thinking: std::mem::take(&mut current_thinking),
                                                    thinking_signature: current_thinking_signature.take(),
                                                    redacted: false,
                                                });
                                            }
                                            _ => {}
                                        }
                                        if want == 2 { yield Event::ThinkingStart; } else { yield Event::TextStart; }
                                        block_kind = want;
                                    }
                                    if is_thought {
                                        current_thinking.push_str(text);
                                        current_thinking_signature = retain_thought_signature(current_thinking_signature.as_deref(), part_sig);
                                        yield Event::ThinkingDelta { delta: text.to_string() };
                                    } else {
                                        current_text.push_str(text);
                                        current_text_signature = retain_thought_signature(current_text_signature.as_deref(), part_sig);
                                        yield Event::TextDelta { delta: text.to_string() };
                                    }
                                }
                                if let Some(fc) = part.get("functionCall") {
                                    // Finalize any open text/thinking block before the tool call.
                                    match block_kind {
                                        1 => {
                                            yield Event::TextEnd;
                                            partial.content.push(ContentBlock::Text {
                                                text: std::mem::take(&mut current_text),
                                                text_signature: current_text_signature.take(),
                                            });
                                        }
                                        2 => {
                                            yield Event::ThinkingEnd;
                                            partial.content.push(ContentBlock::Thinking {
                                                thinking: std::mem::take(&mut current_thinking),
                                                thinking_signature: current_thinking_signature.take(),
                                                redacted: false,
                                            });
                                        }
                                        _ => {}
                                    }
                                    block_kind = 0;
                                    let name = fc.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                    let args = fc.get("args").cloned().unwrap_or_else(|| serde_json::json!({}));
                                    // Preserve the provider-supplied id when present and unique;
                                    // otherwise synthesize a unique one (mirrors upstream).
                                    let provided = fc.get("id").and_then(|v| v.as_str()).map(|s| s.to_string());
                                    let needs_new = match &provided {
                                        None => true,
                                        Some(pid) => tool_call_ids.iter().any(|eid| eid == pid),
                                    };
                                    let id = if needs_new {
                                        format!("{}_{}_{}", name, crate::utils::now_millis(), tool_call_ids.len() + 1)
                                    } else {
                                        provided.unwrap()
                                    };
                                    let sig = part_sig.map(|s| s.to_string());
                                    yield Event::ToolCallStart { id: id.clone(), name: name.clone() };
                                    yield Event::ToolCallDelta { delta: serde_json::to_string(&args).unwrap_or_default() };
                                    yield Event::ToolCallEnd { id: id.clone(), name: name.clone(), arguments: args.clone() };
                                    let arguments = match &args {
                                        serde_json::Value::Object(map) => map.clone().into_iter().collect(),
                                        _ => std::collections::HashMap::new(),
                                    };
                                    partial.content.push(ContentBlock::ToolCall {
                                        id: id.clone(), name, arguments, thought_signature: sig,
                                    });
                                    tool_call_ids.push(id);
                                }
                            }
                        }
                        if let Some(reason) = candidate.get("finishReason").and_then(|v| v.as_str()).filter(|s| !s.is_empty()) {
                            // Finalize any open block before recording the stop reason.
                            match block_kind {
                                1 => {
                                    yield Event::TextEnd;
                                    partial.content.push(ContentBlock::Text {
                                        text: std::mem::take(&mut current_text),
                                        text_signature: current_text_signature.take(),
                                    });
                                }
                                2 => {
                                    yield Event::ThinkingEnd;
                                    partial.content.push(ContentBlock::Thinking {
                                        thinking: std::mem::take(&mut current_thinking),
                                        thinking_signature: current_thinking_signature.take(),
                                        redacted: false,
                                    });
                                }
                                _ => {}
                            }
                            block_kind = 0;
                            partial.stop_reason = Some(if !tool_call_ids.is_empty() {
                                // Any tool call present -> toolUse, regardless of finishReason
                                // (mirrors upstream's content.some(toolCall) override).
                                StopReason::ToolUse
                            } else {
                                match reason {
                                    "STOP" => StopReason::Stop,
                                    "MAX_TOKENS" => StopReason::Length,
                                    other => {
                                        // Safety/recitation/malformed/etc. finish reasons are errors.
                                        partial.error_message = Some(format!("Gemini stopped with finish reason: {other}"));
                                        StopReason::Error
                                    }
                                }
                            });
                        }
                    }
                }

                if let Some(usage) = chunk.get("usageMetadata") {
                    let prompt = usage.get("promptTokenCount").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                    let cached = usage.get("cachedContentTokenCount").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                    let candidates = usage.get("candidatesTokenCount").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                    let thoughts = usage.get("thoughtsTokenCount").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                    partial.usage = Some(Usage {
                        // promptTokenCount includes cached tokens; subtract to get non-cached input.
                        input: prompt.saturating_sub(cached),
                        // candidatesTokenCount excludes reasoning tokens; add thoughtsTokenCount.
                        output: candidates + thoughts,
                        cache_read: cached,
                        // Google reports reasoning tokens via thoughtsTokenCount (subset of output).
                        reasoning: Some(thoughts),
                        total_tokens: usage.get("totalTokenCount").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
                        ..Default::default()
                    });
                }
            }
        }

        if let Some(evt) = parser.finish()
            && evt.event == sse::EVENT_ERROR {
                yield Event::Error {
                    reason: StopReason::Error,
                    error: Arc::from(Box::<dyn std::error::Error + Send + Sync>::from(
                        format!("SSE error: {}", evt.data),
                    )),
                    message: Some(partial.clone()),
                };
                return;
            }

        // Finalize any block still open when the stream ends without a finishReason.
        match block_kind {
            1 if !current_text.is_empty() => {
                partial.content.push(ContentBlock::Text {
                    text: std::mem::take(&mut current_text),
                    text_signature: current_text_signature.take(),
                });
            }
            2 if !current_thinking.is_empty() => {
                partial.content.push(ContentBlock::Thinking {
                    thinking: std::mem::take(&mut current_thinking),
                    thinking_signature: current_thinking_signature.take(),
                    redacted: false,
                });
            }
            _ => {}
        }
        if let Some(ref mut u) = partial.usage {
            crate::simple_options::finalize_usage(model, u);
        }
        let reason = partial.stop_reason.clone().unwrap_or(StopReason::Stop);
        if matches!(reason, StopReason::Error | StopReason::Aborted) {
            let msg = partial.error_message.clone().unwrap_or_else(|| "An unknown error occurred".to_string());
            yield Event::Error {
                reason,
                error: Arc::from(Box::<dyn std::error::Error + Send + Sync>::from(msg)),
                message: Some(partial),
            };
        } else {
            yield Event::Done { reason, message: partial };
        }
    })
}

/// Build Google Generative AI request payload (public for Gemini CLI reuse).
pub fn build_google_payload_public(
    model: &Model,
    context: &Context,
    opts: &StreamOptions,
) -> Value {
    build_google_payload(model, context, opts)
}

/// Gemini models that require explicit tool-call ids on functionCall/functionResponse
/// (mirrors requiresToolCallId).
fn google_requires_tool_call_id(model_id: &str) -> bool {
    model_id.starts_with("claude-") || model_id.starts_with("gpt-oss-")
}

/// Normalize a tool-call id for Gemini when required (alnum/_/- only, max 64 chars).
/// Recursively strip JSON Schema meta keys ($schema, $id, $comment, $defs,
/// definitions) from a schema value, preserving `$ref` and everything else
/// (mirrors the upstream convertTools `useParameters` stripper). Non-mutating.
fn strip_json_schema_meta_keys(value: &Value) -> Value {
    match value {
        Value::Array(arr) => Value::Array(arr.iter().map(strip_json_schema_meta_keys).collect()),
        Value::Object(map) => {
            let mut out = serde_json::Map::new();
            for (k, v) in map {
                if matches!(
                    k.as_str(),
                    "$schema" | "$id" | "$comment" | "$defs" | "definitions"
                ) {
                    continue;
                }
                out.insert(k.clone(), strip_json_schema_meta_keys(v));
            }
            Value::Object(out)
        }
        other => other.clone(),
    }
}

/// Convert tools to the Google `functionDeclarations` form (mirrors convertTools).
/// `use_parameters=true` emits the legacy OpenAPI-3 `parameters` field with JSON
/// Schema meta keys stripped; otherwise the full-JSON-Schema `parametersJsonSchema`
/// is preserved. Returns `None` for an empty tool list.
pub(crate) fn convert_google_tools(
    tools: &[crate::types::Tool],
    use_parameters: bool,
) -> Option<Value> {
    if tools.is_empty() {
        return None;
    }
    let decls: Vec<Value> = tools.iter().map(|t| {
        if use_parameters {
            json!({"name": t.name, "description": t.description, "parameters": strip_json_schema_meta_keys(&t.parameters)})
        } else {
            json!({"name": t.name, "description": t.description, "parametersJsonSchema": t.parameters})
        }
    }).collect();
    Some(json!([{"functionDeclarations": decls}]))
}

pub(crate) fn google_normalize_tool_call_id(model_id: &str, id: &str) -> String {
    if !google_requires_tool_call_id(model_id) {
        return id.to_string();
    }
    let sanitized: String = id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if sanitized.len() > 64 {
        sanitized[..64].to_string()
    } else {
        sanitized
    }
}

/// Parse a leading `gemini-N` / `gemini-live-N` major version.
fn gemini_major_version(model_id: &str) -> Option<u32> {
    let lower = model_id.to_lowercase();
    let rest = lower
        .strip_prefix("gemini-live-")
        .or_else(|| lower.strip_prefix("gemini-"))?;
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        None
    } else {
        digits.parse().ok()
    }
}

/// Gemini 3+ (and non-Gemini) models support image parts nested in functionResponse.
fn google_supports_multimodal_function_response(model_id: &str) -> bool {
    match gemini_major_version(model_id) {
        Some(v) => v >= 3,
        None => true,
    }
}

/// Whether a Gemini stream part is thinking content: only `thought === true`
/// indicates thinking (a `thoughtSignature` alone does not — it can appear on any
/// part type for context replay). Mirrors upstream `isThinkingPart`.
pub(crate) fn is_thinking_part(part: &Value) -> bool {
    part.get("thought")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

/// Keep the existing thought signature when a subsequent delta omits/empties it;
/// update when a new non-empty signature arrives (mirrors upstream
/// `retainThoughtSignature`).
pub(crate) fn retain_thought_signature(prev: Option<&str>, new: Option<&str>) -> Option<String> {
    new.filter(|s| !s.is_empty())
        .map(String::from)
        .or_else(|| prev.map(String::from))
}

/// Validate a Gemini thought signature: base64-ish and length a multiple of 4.
fn is_valid_thought_signature(sig: &str) -> bool {
    if sig.is_empty() || !sig.len().is_multiple_of(4) {
        return false;
    }
    let mut seen_pad = false;
    let mut pad = 0;
    for c in sig.chars() {
        if c == '=' {
            seen_pad = true;
            pad += 1;
            if pad > 2 {
                return false;
            }
        } else {
            if seen_pad {
                return false;
            }
            if !(c.is_ascii_alphanumeric() || c == '+' || c == '/') {
                return false;
            }
        }
    }
    true
}

/// Only replay a thought signature when the message is from the same provider+model
/// and the signature is valid (mirrors resolveThoughtSignature).
fn resolve_thought_signature(is_same: bool, sig: Option<&str>) -> Option<&str> {
    match sig {
        Some(s) if is_same && is_valid_thought_signature(s) => Some(s),
        _ => None,
    }
}

fn build_google_payload(model: &Model, context: &Context, opts: &StreamOptions) -> Value {
    let mut contents: Vec<Value> = Vec::new();

    let transformed_messages = crate::transform::transform_messages(&context.messages, model);

    for msg in &transformed_messages {
        match msg.role {
            Role::ToolResult => {
                // Tool results must be sent as functionResponse parts, and consecutive
                // tool results must be merged into a single user turn (Cloud Code Assist).
                let text_result = msg
                    .content
                    .iter()
                    .filter_map(|b| match b {
                        ContentBlock::Text { text, .. } => Some(text.clone()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                let has_images = model.input.iter().any(|i| i == "image")
                    && msg
                        .content
                        .iter()
                        .any(|b| matches!(b, ContentBlock::Image { .. }));
                let response_value = if !text_result.is_empty() {
                    text_result
                } else if has_images {
                    "(see attached image)".to_string()
                } else {
                    String::new()
                };
                let response = if msg.is_error {
                    json!({"error": response_value})
                } else {
                    json!({"output": response_value})
                };
                let image_parts: Vec<Value> = msg
                    .content
                    .iter()
                    .filter_map(|b| match b {
                        ContentBlock::Image { data, mime_type } => Some(json!({
                            "inlineData": {"mimeType": mime_type, "data": data}
                        })),
                        _ => None,
                    })
                    .collect();
                let supports_multimodal = google_supports_multimodal_function_response(&model.id);
                let mut function_response = json!({
                    "name": msg.tool_name.clone().unwrap_or_default(),
                    "response": response,
                });
                // Gemini 3+ supports image parts nested in the functionResponse.
                if has_images && supports_multimodal {
                    function_response["parts"] = json!(image_parts.clone());
                }
                if google_requires_tool_call_id(&model.id)
                    && let Some(ref id) = msg.tool_call_id
                {
                    function_response["id"] = json!(google_normalize_tool_call_id(&model.id, id));
                }
                let function_response_part = json!({
                    "functionResponse": function_response
                });

                let merge = contents
                    .last()
                    .and_then(|c| {
                        c.get("role")
                            .and_then(|r| r.as_str())
                            .map(|r| r == "user")
                            .map(|is_user| {
                                is_user
                                    && c.get("parts")
                                        .and_then(|p| p.as_array())
                                        .map(|parts| {
                                            parts
                                                .iter()
                                                .any(|p| p.get("functionResponse").is_some())
                                        })
                                        .unwrap_or(false)
                            })
                    })
                    .unwrap_or(false);
                if merge {
                    if let Some(parts) = contents
                        .last_mut()
                        .and_then(|c| c.get_mut("parts"))
                        .and_then(|p| p.as_array_mut())
                    {
                        parts.push(function_response_part);
                    }
                } else {
                    contents.push(json!({"role": "user", "parts": [function_response_part]}));
                }
                // For models without multimodal functionResponse support (Gemini < 3),
                // attach images in a separate user turn (mirrors upstream).
                if has_images && !supports_multimodal {
                    let mut parts = vec![json!({"text": "Tool result image:"})];
                    parts.extend(image_parts);
                    contents.push(json!({"role": "user", "parts": parts}));
                }
            }
            Role::User => {
                let parts: Vec<Value> = msg
                    .content
                    .iter()
                    .map(|b| match b {
                        ContentBlock::Text { text, .. } => json!({"text": text}),
                        ContentBlock::Image { data, mime_type } => json!({
                            "inlineData": {"mimeType": mime_type, "data": data}
                        }),
                        ContentBlock::Thinking { thinking, .. } => json!({"text": thinking}),
                        ContentBlock::ToolCall {
                            name, arguments, ..
                        } => json!({
                            "functionCall": {"name": name, "args": arguments}
                        }),
                    })
                    .collect();
                if parts.is_empty() {
                    continue;
                }
                contents.push(json!({"role": "user", "parts": parts}));
            }
            Role::Assistant => {
                // Thought signatures and thinking blocks only replay when the message
                // came from the same provider+model (mirrors isSameProviderAndModel).
                let is_same = msg.provider.as_deref() == Some(model.provider.as_str())
                    && msg.model.as_deref() == Some(model.id.as_str());
                let parts: Vec<Value> = msg
                    .content
                    .iter()
                    .filter_map(|b| match b {
                        ContentBlock::Text {
                            text,
                            text_signature,
                        } if !text.trim().is_empty() => {
                            let mut p = json!({"text": text});
                            if let Some(sig) =
                                resolve_thought_signature(is_same, text_signature.as_deref())
                            {
                                p["thoughtSignature"] = json!(sig);
                            }
                            Some(p)
                        }
                        ContentBlock::Image { data, mime_type } => Some(json!({
                            "inlineData": {"mimeType": mime_type, "data": data}
                        })),
                        ContentBlock::Thinking {
                            thinking,
                            thinking_signature,
                            ..
                        } if !thinking.trim().is_empty() => {
                            if is_same {
                                let mut p = json!({"thought": true, "text": thinking});
                                if let Some(sig) = resolve_thought_signature(
                                    is_same,
                                    thinking_signature.as_deref(),
                                ) {
                                    p["thoughtSignature"] = json!(sig);
                                }
                                Some(p)
                            } else {
                                // Different model: downgrade thinking to plain text.
                                Some(json!({"text": thinking}))
                            }
                        }
                        ContentBlock::ToolCall {
                            id,
                            name,
                            arguments,
                            thought_signature,
                        } => {
                            let mut fc = json!({"name": name, "args": arguments});
                            if google_requires_tool_call_id(&model.id) {
                                fc["id"] = json!(google_normalize_tool_call_id(&model.id, id));
                            }
                            let mut p = json!({"functionCall": fc});
                            if let Some(sig) =
                                resolve_thought_signature(is_same, thought_signature.as_deref())
                            {
                                p["thoughtSignature"] = json!(sig);
                            }
                            Some(p)
                        }
                        _ => None,
                    })
                    .collect();
                if parts.is_empty() {
                    continue;
                }
                contents.push(json!({"role": "model", "parts": parts}));
            }
        }
    }

    let mut payload = json!({"contents": contents});

    if let Some(prompt) = context.system_prompt.as_deref().filter(|p| !p.is_empty()) {
        payload["systemInstruction"] = json!({"parts": [{"text": prompt}]});
    }

    let mut config = json!({});
    // Fold streamSimple's buildBaseOptions clamped/defaulted cap. google's inner gate
    // is `maxTokens !== undefined`, and base.maxTokens is always defined, so the field
    // is always emitted (including a clamped 0).
    let max_output_tokens = crate::simple_options::clamp_max_tokens_to_context(
        model,
        context,
        opts.max_tokens.unwrap_or(model.max_tokens),
    );
    config["maxOutputTokens"] = json!(max_output_tokens);
    if let Some(temp) = opts.temperature {
        config["temperature"] = json!(temp);
    }
    // Thinking config for reasoning models.
    if model.reasoning {
        let id = model.id.to_lowercase();
        let is_gemini3_pro = id.contains("gemini-3") && id.contains("-pro");
        let is_gemini3_flash = (id.contains("gemini-3") && id.contains("-flash"))
            || id == "gemini-flash-latest"
            || id == "gemini-flash-lite-latest";
        let is_gemma4 = id.contains("gemma-4") || id.contains("gemma4");
        if let Some(reasoning) = opts.reasoning.as_ref() {
            let mut thinking_config = json!({"includeThoughts": true});
            // Clamp to a supported level; a level that clamps to off becomes "high"
            // (mirrors streamSimpleGoogle effort = clamped==="off" ? "high" : clamped).
            let effort = match crate::simple_options::clamp_reasoning_for_model(model, reasoning) {
                Some(clamped) => format!("{:?}", clamped).to_lowercase(),
                None => "high".to_string(),
            };
            if is_gemini3_pro || is_gemini3_flash || is_gemma4 {
                // Gemini 3 / Gemma 4 use a thinkingLevel string (omitted if effort has no mapping).
                let tl: Option<&str> = if is_gemini3_pro {
                    match effort.as_str() {
                        "minimal" | "low" => Some("LOW"),
                        "medium" | "high" => Some("HIGH"),
                        _ => None,
                    }
                } else if is_gemma4 {
                    match effort.as_str() {
                        "minimal" | "low" => Some("MINIMAL"),
                        "medium" | "high" => Some("HIGH"),
                        _ => None,
                    }
                } else {
                    match effort.as_str() {
                        "minimal" => Some("MINIMAL"),
                        "low" => Some("LOW"),
                        "medium" => Some("MEDIUM"),
                        "high" => Some("HIGH"),
                        _ => None,
                    }
                };
                if let Some(tl) = tl {
                    thinking_config["thinkingLevel"] = json!(tl);
                }
            } else {
                // Budget-based models: per-effort custom budget, else model-specific
                // defaults, else -1 (dynamic). Omitted when getGoogleBudget has no value.
                let custom = opts
                    .thinking_budgets
                    .as_ref()
                    .and_then(|b| match effort.as_str() {
                        "minimal" => b.minimal,
                        "low" => b.low,
                        "medium" => b.medium,
                        "high" => b.high,
                        _ => None,
                    })
                    .map(|v| v as i64);
                let budget: Option<i64> = custom.or_else(|| {
                    if id.contains("2.5-pro") {
                        match effort.as_str() {
                            "minimal" => Some(128),
                            "low" => Some(2048),
                            "medium" => Some(8192),
                            "high" => Some(32768),
                            _ => None,
                        }
                    } else if id.contains("2.5-flash-lite") {
                        match effort.as_str() {
                            "minimal" => Some(512),
                            "low" => Some(2048),
                            "medium" => Some(8192),
                            "high" => Some(24576),
                            _ => None,
                        }
                    } else if id.contains("2.5-flash") {
                        match effort.as_str() {
                            "minimal" => Some(128),
                            "low" => Some(2048),
                            "medium" => Some(8192),
                            "high" => Some(24576),
                            _ => None,
                        }
                    } else {
                        Some(-1)
                    }
                });
                if let Some(budget) = budget {
                    thinking_config["thinkingBudget"] = json!(budget);
                }
            }
            config["thinkingConfig"] = thinking_config;
        } else {
            // Reasoning not requested: explicitly disable thinking (mirrors getDisabledThinkingConfig).
            let disabled = if is_gemini3_pro {
                json!({"thinkingLevel": "LOW"})
            } else if is_gemini3_flash || is_gemma4 {
                json!({"thinkingLevel": "MINIMAL"})
            } else {
                json!({"thinkingBudget": 0})
            };
            config["thinkingConfig"] = disabled;
        }
    }
    if config != json!({}) {
        payload["generationConfig"] = config;
    }

    if !context.tools.is_empty()
        && let Some(tools) = convert_google_tools(&context.tools, false)
    {
        payload["tools"] = tools;

        // Tool choice -> functionCallingConfig mode.
        if let Some(ref tc) = opts.tool_choice {
            let mode = match tc.as_str() {
                Some("auto") => "AUTO",
                Some("any") => "ANY",
                Some("none") => "NONE",
                _ => "AUTO",
            };
            payload["toolConfig"] = json!({"functionCallingConfig": {"mode": mode}});
        }
    }

    payload
}

fn url_encode(s: &str) -> String {
    s.replace('%', "%25")
        .replace(' ', "%20")
        .replace('+', "%2B")
        .replace('/', "%2F")
}

/// Build the streaming REST endpoint for Gemini or Vertex AI.
/// Mirrors upstream go-ai `buildStreamURL` / `resolveVertexProjectLocation`.
pub(crate) fn build_stream_url(
    model: &Model,
    api_key: &str,
    opts: &StreamOptions,
) -> Result<String, String> {
    if model.api == crate::types::api::GOOGLE_VERTEX {
        let (project, location) = resolve_vertex_project_location(opts)?;
        let mut base_url = model.base_url.clone();
        if base_url.is_empty() {
            base_url = "https://{location}-aiplatform.googleapis.com".to_string();
        }
        base_url = base_url.replace("{location}", &location);
        let mut endpoint = format!(
            "{}/v1/projects/{}/locations/{}/publishers/google/models/{}:streamGenerateContent?alt=sse",
            base_url.trim_end_matches('/'),
            url_encode(&project),
            url_encode(&location),
            url_encode(&model.id),
        );
        if !api_key.is_empty() && !api_key.starts_with('<') {
            endpoint.push_str("&key=");
            endpoint.push_str(&url_encode(api_key));
        }
        return Ok(endpoint);
    }
    let mut base_url = model.base_url.clone();
    if base_url.is_empty() {
        base_url = "https://generativelanguage.googleapis.com/v1beta".to_string();
    }
    Ok(format!(
        "{}/models/{}:streamGenerateContent?alt=sse&key={}",
        base_url.trim_end_matches('/'),
        url_encode(&model.id),
        url_encode(api_key),
    ))
}

/// Resolve the Vertex AI project and location from options or environment.
/// Mirrors upstream go-ai `resolveVertexProjectLocation`.
pub(crate) fn resolve_vertex_project_location(
    opts: &StreamOptions,
) -> Result<(String, String), String> {
    let env_value = |name: &str| std::env::var(name).ok().filter(|v| !v.is_empty());
    let mut project = opts.project.clone().filter(|v| !v.is_empty());
    let mut location = opts.location.clone().filter(|v| !v.is_empty());
    if project.is_none() {
        project = env_value("GOOGLE_CLOUD_PROJECT");
    }
    if project.is_none() {
        project = env_value("GCLOUD_PROJECT");
    }
    if location.is_none() {
        location = env_value("GOOGLE_CLOUD_LOCATION");
    }
    let project = project.ok_or_else(|| {
        "vertex AI requires a project ID; set GOOGLE_CLOUD_PROJECT/GCLOUD_PROJECT or pass Project in options".to_string()
    })?;
    let location = location.ok_or_else(|| {
        "vertex AI requires a location; set GOOGLE_CLOUD_LOCATION or pass Location in options"
            .to_string()
    })?;
    Ok((project, location))
}
