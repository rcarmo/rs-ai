//! OpenAI Chat Completions provider (also serves compatible APIs).

use std::sync::Arc;

use futures::stream::{self, StreamExt};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use serde_json::{json, Value};

use crate::compat::detect_compat;
use crate::env::client_api_key;
use crate::events::Event;
use crate::transports::sse;
use crate::types::*;

/// Start an OpenAI-compatible chat completions stream.
pub fn stream_openai<'a>(
    model: &'a Model,
    context: &'a Context,
    opts: &'a StreamOptions,
) -> std::pin::Pin<Box<dyn futures::Stream<Item = Event> + Send + 'a>> {
    let api_key = client_api_key(model, opts);
    if api_key.is_none() {
        let err = Event::Error {
            reason: StopReason::Error,
            error: Arc::from(Box::<dyn std::error::Error + Send + Sync>::from(
                format!("No API key for provider: {}", model.provider),
            )),
            message: None,
        };
        return Box::pin(stream::once(async { err }));
    }
    let api_key = api_key.unwrap();
    let compat = detect_compat(model);

    // Build request payload
    let mut payload = build_payload(model, context, opts, &compat);
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

    let (url, headers) = match build_openai_request_parts(model, context, opts, &compat, &api_key) {
        Ok(parts) => parts,
        Err(msg) => {
            let err = Event::Error {
                reason: StopReason::Error,
                error: Arc::from(Box::<dyn std::error::Error + Send + Sync>::from(msg)),
                message: None,
            };
            return Box::pin(stream::once(async { err }));
        }
    };

    Box::pin(async_stream::stream! {
        let client = reqwest::Client::new();
        let request = client
            .post(&url)
            .headers(headers)
            .json(&payload);
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
                    error: Arc::from(Box::new(e) as Box<dyn std::error::Error + Send + Sync>),
                    message: None,
                };
                return;
            }
        };

        let status = resp.status().as_u16();

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            yield Event::Error {
                reason: StopReason::Error,
                error: Arc::from(Box::<dyn std::error::Error + Send + Sync>::from(
                    format!("HTTP {}: {}", status, body),
                )),
                message: None,
            };
            return;
        }

        // Upstream invokes onResponse only after a successful response (the SDK's
        // withResponse() rejects on non-2xx before onResponse runs), so fire it after the
        // status check, never for error responses.
        if let Some(ref hook) = opts.on_response {
            let mut hdrs = std::collections::HashMap::new();
            for (k, v) in resp.headers().iter() {
                hdrs.insert(k.to_string(), v.to_str().unwrap_or("").to_string());
            }
            hook(status, &hdrs, model);
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
        };

        yield Event::Start { partial: partial.clone() };

        let mut parser = sse::SseParser::default();
        let mut stream = resp.bytes_stream();

        let mut text_started = false;
        let mut current_text = String::new();
        let mut thinking_started = false;
        // Track whether thinking began before any text in the stream so the
        // assembled content preserves streaming order (mirrors upstream, which
        // creates blocks incrementally: text-first when `content` streams before
        // `reasoning_content`). Defaults to thinking-first for reasoning-first streams.
        let mut thinking_before_text = false;
        let mut current_thinking = String::new();
        let mut current_thinking_signature: Option<String> = None;
        let mut tool_calls: std::collections::BTreeMap<usize, (String, String, String)> = std::collections::BTreeMap::new();
        // Captured encrypted reasoning details keyed by tool-call id (OpenRouter).
        let mut tool_call_signatures: std::collections::HashMap<String, String> = std::collections::HashMap::new();

        let mut got_done = false;
        while let Some(chunk_result) = stream.next().await {
            let chunk_bytes = match chunk_result {
                Ok(c) => c,
                Err(e) => {
                    // Mid-stream network/body error: record it and break so the after-loop
                    // finalization assembles partial.content before the terminal Error event
                    // (mirrors upstream surfacing accumulated blocks on a broken stream).
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
                if evt.data == "[DONE]" {
                    got_done = true;
                    break;
                }
                let chunk: Value = match serde_json::from_str(&evt.data) {
                    Ok(v) => v,
                    Err(_) => continue,
                };

                if let Some(err) = chunk.get("error") {
                    let mut msg = err.get("message").and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| err.to_string());
                    // OpenRouter surfaces the upstream provider's raw error under metadata.raw.
                    if let Some(raw) = err.pointer("/metadata/raw").and_then(|v| v.as_str())
                        && !raw.is_empty() {
                        msg.push('\n');
                        msg.push_str(raw);
                    }
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
                    && let Some(id) = chunk.get("id").and_then(|v| v.as_str()) {
                    partial.response_id = Some(id.to_string());
                }
                if partial.response_model.is_none()
                    && let Some(m) = chunk.get("model").and_then(|v| v.as_str())
                    && !m.is_empty() && m != model.id {
                    partial.response_model = Some(m.to_string());
                }

                if let Some(choices) = chunk.get("choices").and_then(|v| v.as_array()) {
                    for choice in choices {
                        let delta = match choice.get("delta") {
                            Some(d) => d,
                            None => continue,
                        };

                        if let Some(content) = delta.get("content").and_then(|v| v.as_str())
                            && !content.is_empty() {
                                if !text_started {
                                    text_started = true;
                                    yield Event::TextStart;
                                }
                                current_text.push_str(content);
                                yield Event::TextDelta { delta: content.to_string() };
                            }

                        let reasoning_fields = ["reasoning_content", "reasoning", "reasoning_text"];
                        for field in reasoning_fields {
                            if let Some(reasoning) = delta.get(field).and_then(|v| v.as_str())
                                && !reasoning.is_empty() {
                                    if !thinking_started {
                                        thinking_started = true;
                                        if !text_started {
                                            thinking_before_text = true;
                                        }
                                        // opencode-go reports the `reasoning` field but replays it as `reasoning_content`.
                                        let sig = if model.provider == "opencode-go" && field == "reasoning" {
                                            "reasoning_content"
                                        } else {
                                            field
                                        };
                                        current_thinking_signature = Some(sig.to_string());
                                        yield Event::ThinkingStart;
                                    }
                                    current_thinking.push_str(reasoning);
                                    yield Event::ThinkingDelta { delta: reasoning.to_string() };
                                    break;
                                }
                        }

                        if let Some(delta_tool_calls) = delta.get("tool_calls").and_then(|v| v.as_array()) {
                            for tc in delta_tool_calls {
                                // Prefer the streamed index; when absent, correlate by id
                                // (mirrors upstream toolCallBlocksByIndex / toolCallBlocksById).
                                let index = if let Some(i) = tc.get("index").and_then(|v| v.as_u64()) {
                                    i as usize
                                } else if let Some(id) =
                                    tc.get("id").and_then(|v| v.as_str()).filter(|s| !s.is_empty())
                                {
                                    match tool_calls.iter().find(|(_, v)| v.0 == id) {
                                        Some((k, _)) => *k,
                                        None => tool_calls.keys().next_back().map(|k| k + 1).unwrap_or(0),
                                    }
                                } else {
                                    0
                                };
                                let entry = tool_calls.entry(index).or_insert_with(|| (String::new(), String::new(), String::new()));
                                if let Some(id) = tc.get("id").and_then(|v| v.as_str())
                                    && entry.0.is_empty() {
                                        entry.0 = id.to_string();
                                    }
                                if let Some(func) = tc.get("function") {
                                    if let Some(name) = func.get("name").and_then(|v| v.as_str())
                                        && entry.1.is_empty() {
                                            entry.1 = name.to_string();
                                            if !entry.1.is_empty() {
                                                yield Event::ToolCallStart { id: entry.0.clone(), name: entry.1.clone() };
                                            }
                                        }
                                    if let Some(args) = func.get("arguments").and_then(|v| v.as_str()) {
                                            entry.2.push_str(args);
                                            // Upstream emits a toolcall_delta whenever `arguments`
                                            // is present (checks `!== undefined`), including the
                                            // empty-string first fragment — not only when non-empty.
                                            yield Event::ToolCallDelta { delta: args.to_string() };
                                        }
                                }
                            }
                        }

                        // Capture encrypted reasoning details and pair them to tool calls by id.
                        // Signatures are stored by id and applied when tool-call blocks are
                        // built at finish_reason, so this is order-independent (covers the
                        // 0.79.10 pendingReasoningDetailsByToolCallId case). Validation mirrors
                        // isEncryptedReasoningDetail: non-empty string id and data.
                        if let Some(details) = delta.get("reasoning_details").and_then(|v| v.as_array()) {
                            for detail in details {
                                if detail.get("type").and_then(|v| v.as_str()) == Some("reasoning.encrypted")
                                    && let Some(did) = detail.get("id").and_then(|v| v.as_str()).filter(|s| !s.is_empty())
                                    && detail.get("data").and_then(|v| v.as_str()).is_some_and(|s| !s.is_empty()) {
                                    tool_call_signatures.insert(did.to_string(), detail.to_string());
                                }
                            }
                        }

                        // Match upstream's truthy `if (choice.finish_reason)` check:
                        // null/absent/empty-string finish_reason is not a terminal signal.
                        if let Some(reason) = choice.get("finish_reason").and_then(|v| v.as_str()).filter(|s| !s.is_empty()) {
                            if text_started {
                                yield Event::TextEnd;
                                text_started = false;
                            }
                            if thinking_started {
                                yield Event::ThinkingEnd;
                                thinking_started = false;
                            }
                            let (stop, err_msg) = crate::simple_options::map_openai_finish_reason(reason);
                            if let Some(msg) = err_msg {
                                partial.error_message = Some(msg);
                            }
                            partial.stop_reason = Some(stop.clone());
                            assemble_text_thinking(&mut partial.content, &current_thinking, &current_thinking_signature, &current_text, thinking_before_text);
                            if partial.content.iter().all(|b| !matches!(b, ContentBlock::ToolCall { .. })) {
                                for (id, name, args_json) in tool_calls.values() {
                                    let parsed = crate::jsonparse::parse_streaming_json(args_json);
                                    let arguments = match &parsed {
                                        serde_json::Value::Object(map) => map.clone().into_iter().collect(),
                                        _ => std::collections::HashMap::new(),
                                    };
                                    partial.content.push(ContentBlock::ToolCall {
                                        id: id.clone(),
                                        name: name.clone(),
                                        arguments,
                                        thought_signature: tool_call_signatures.get(id).cloned(),
                                    });
                                    yield Event::ToolCallEnd {
                                        id: id.clone(),
                                        name: name.clone(),
                                        arguments: parsed,
                                    };
                                }
                            }
                        }
                    }
                }

                if let Some(usage) = chunk.get("usage") {
                    partial.usage = Some(crate::simple_options::parse_openai_usage(usage, model));
                } else if let Some(choice_usage) = chunk.pointer("/choices/0/usage") {
                    // Some providers report usage on the choice instead of the chunk.
                    partial.usage = Some(crate::simple_options::parse_openai_usage(choice_usage, model));
                }
            }
            if got_done {
                break;
            }
        }

        // Finalize any accumulated blocks so a stream that ends WITHOUT a finish_reason
        // (truncation / error) still carries its partial content, mirroring upstream's
        // finishBlock at stream end. The any()/all() guards make this idempotent with the
        // finish_reason-path assembly above (normal completions already populated content).
        if text_started {
            yield Event::TextEnd;
        }
        if thinking_started {
            yield Event::ThinkingEnd;
        }
        assemble_text_thinking(&mut partial.content, &current_thinking, &current_thinking_signature, &current_text, thinking_before_text);
        if partial.content.iter().all(|b| !matches!(b, ContentBlock::ToolCall { .. })) {
            for (id, name, args_json) in tool_calls.values() {
                let parsed = crate::jsonparse::parse_streaming_json(args_json);
                let arguments = match &parsed {
                    serde_json::Value::Object(map) => map.clone().into_iter().collect(),
                    _ => std::collections::HashMap::new(),
                };
                partial.content.push(ContentBlock::ToolCall {
                    id: id.clone(),
                    name: name.clone(),
                    arguments,
                    thought_signature: tool_call_signatures.get(id).cloned(),
                });
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

        match partial.stop_reason.clone() {
            Some(StopReason::Error) => {
                let msg = partial.error_message.clone().unwrap_or_else(|| "Provider returned an error stop reason".to_string());
                yield Event::Error {
                    reason: StopReason::Error,
                    error: Arc::from(Box::<dyn std::error::Error + Send + Sync>::from(msg)),
                    message: Some(partial),
                };
            }
            None => {
                // Upstream treats a stream that ends without a finish_reason as an error.
                yield Event::Error {
                    reason: StopReason::Error,
                    error: Arc::from(Box::<dyn std::error::Error + Send + Sync>::from(
                        "Stream ended without finish_reason".to_string(),
                    )),
                    message: Some(partial),
                };
            }
            Some(reason) => {
                yield Event::Done { reason, message: partial };
            }
        }
    })
}

/// Append the accumulated text and thinking blocks to `content`, preserving the
/// stream order in which they began (`thinking_first` = thinking started before
/// any text). Mirrors upstream's incremental block creation.
fn assemble_text_thinking(
    content: &mut Vec<ContentBlock>,
    thinking: &str,
    sig: &Option<String>,
    text: &str,
    thinking_first: bool,
) {
    let needs_thinking = !thinking.is_empty()
        && !content.iter().any(|b| matches!(b, ContentBlock::Thinking { .. }));
    let needs_text = !text.is_empty()
        && !content.iter().any(|b| matches!(b, ContentBlock::Text { .. }));
    let thinking_block = || ContentBlock::Thinking {
        thinking: thinking.to_string(),
        thinking_signature: sig.clone(),
        redacted: false,
    };
    let text_block = || ContentBlock::Text { text: text.to_string(), text_signature: None };
    if thinking_first {
        if needs_thinking { content.push(thinking_block()); }
        if needs_text { content.push(text_block()); }
    } else {
        if needs_text { content.push(text_block()); }
        if needs_thinking { content.push(thinking_block()); }
    }
}

/// Build the request URL and headers for an OpenAI-completions request.
/// Extracted from `stream_openai` so the Cloudflare-AI-Gateway base-URL/header
/// resolution and session-affinity headers are unit-testable without a live
/// request (mirrors upstream client-construction assertions).
pub(crate) fn build_openai_request_parts(
    model: &Model,
    context: &Context,
    opts: &StreamOptions,
    compat: &crate::compat::OpenAICompletionsCompat,
    api_key: &str,
) -> Result<(String, HeaderMap), String> {
    let base = crate::utils::resolve_cloudflare_base_url(
        model.base_url.trim_end_matches('/'),
        &model.provider,
    )?;
    let url = format!("{}/chat/completions", base.trim_end_matches('/'));
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    headers.insert("Accept", HeaderValue::from_static("text/event-stream"));

    if model.provider == "cloudflare-ai-gateway" {
        headers.insert("cf-aig-authorization", HeaderValue::from_str(&format!("Bearer {}", api_key)).unwrap());
    } else {
        headers.insert(AUTHORIZATION, HeaderValue::from_str(&format!("Bearer {}", api_key)).unwrap());
    }

    // GitHub Copilot dynamic headers (mirrors upstream buildCopilotDynamicHeaders)
    if model.provider == "github-copilot" {
        for (k, v) in crate::utils::copilot_dynamic_headers(&context.messages) {
            headers.insert(k, HeaderValue::from_static(v));
        }
    }

    // Session affinity headers for providers that require them. The session id is
    // cleared when caching is off (upstream cacheSessionId = retention==="none" ?
    // undefined : sessionId), so these headers are omitted; skip empty session ids.
    let affinity_caching_on =
        crate::prompt_cache::resolve_cache_retention(opts.cache_retention.as_ref()) != crate::types::CacheRetention::None;
    if affinity_caching_on
        && let Some(session_id) = opts.session_id.as_deref().filter(|s| !s.is_empty())
        && compat.supports_session_affinity_headers == Some(true)
            && let Ok(val) = HeaderValue::from_str(session_id) {
                headers.insert("session_id", val.clone());
                headers.insert("x-client-request-id", val.clone());
                headers.insert("x-session-affinity", val);
            }

    // Add model-level headers
    if let Some(ref model_headers) = model.headers {
        for (k, v) in model_headers {
            if let (Ok(name), Ok(val)) = (
                reqwest::header::HeaderName::from_bytes(k.as_bytes()),
                HeaderValue::from_str(v),
            ) {
                headers.insert(name, val);
            }
        }
    }

    if let Some(ref extra_headers) = opts.headers {
        for (k, v) in extra_headers {
            if let (Ok(name), Ok(val)) = (
                reqwest::header::HeaderName::from_bytes(k.as_bytes()),
                HeaderValue::from_str(v),
            ) {
                headers.insert(name, val);
            }
        }
    }

    Ok((url, headers))
}

pub(crate) fn build_payload(
    model: &Model,
    context: &Context,
    opts: &StreamOptions,
    compat: &crate::compat::OpenAICompletionsCompat,
) -> Value {
    let mut messages = Vec::new();

    // System prompt
    if let Some(prompt) = context.system_prompt.as_deref().filter(|p| !p.is_empty()) {
        // Developer role only for reasoning models that support it (mirrors upstream
        // useDeveloperRole = model.reasoning && compat.supportsDeveloperRole).
        let role = if model.reasoning && compat.supports_developer_role == Some(true) {
            "developer"
        } else {
            "system"
        };
        messages.push(json!({ "role": role, "content": prompt }));
    }

    // Conversation messages
    let transformed_messages = crate::transform::transform_messages(&context.messages, model);
    let mut last_role: Option<Role> = None;
    let mut idx = 0usize;
    while idx < transformed_messages.len() {
        let msg = &transformed_messages[idx];

        // Some providers don't allow a user message directly after tool results;
        // insert a synthetic assistant message to bridge (mirrors upstream).
        if compat.requires_assistant_after_tool_result == Some(true)
            && last_role == Some(Role::ToolResult)
            && msg.role == Role::User {
            messages.push(json!({"role": "assistant", "content": "I have processed the tool results."}));
        }

        // Tool results: emit a `tool` message for each consecutive result, then a
        // separate user message carrying any images (the OpenAI `tool` role cannot
        // hold image content) — mirrors upstream convertMessages.
        if msg.role == Role::ToolResult {
            let mut image_blocks: Vec<Value> = Vec::new();
            let mut j = idx;
            while j < transformed_messages.len() && transformed_messages[j].role == Role::ToolResult {
                let tr = &transformed_messages[j];
                let text_result = tr.content.iter().filter_map(|b| match b {
                    ContentBlock::Text { text, .. } => Some(text.as_str()),
                    _ => None,
                }).collect::<Vec<_>>().join("\n");
                let has_images = tr.content.iter().any(|b| matches!(b, ContentBlock::Image { .. }));
                let content = if text_result.is_empty() { "(see attached image)".to_string() } else { text_result };
                let mut tm = json!({
                    "role": "tool",
                    "content": content,
                    "tool_call_id": tr.tool_call_id.as_deref()
                        .map(|id| normalize_tool_call_id(id, &model.provider)).unwrap_or_default(),
                });
                if compat.requires_tool_result_name == Some(true)
                    && let Some(ref name) = tr.tool_name {
                        tm["name"] = json!(name);
                    }
                messages.push(tm);
                if has_images && model.input.iter().any(|i| i == "image") {
                    for b in &tr.content {
                        if let ContentBlock::Image { data, mime_type } = b {
                            image_blocks.push(json!({
                                "type": "image_url",
                                "image_url": {"url": format!("data:{};base64,{}", mime_type, data)}
                            }));
                        }
                    }
                }
                j += 1;
            }
            idx = j;
            if !image_blocks.is_empty() {
                if compat.requires_assistant_after_tool_result == Some(true) {
                    messages.push(json!({"role": "assistant", "content": "I have processed the tool results."}));
                }
                let mut content = vec![json!({"type": "text", "text": "Attached image(s) from tool result:"})];
                content.extend(image_blocks);
                messages.push(json!({"role": "user", "content": content}));
                last_role = Some(Role::User);
            } else {
                last_role = Some(Role::ToolResult);
            }
            continue;
        }

        let role_str = match msg.role {
            Role::User => "user",
            Role::Assistant => "assistant",
            Role::ToolResult => unreachable!(),
        };

        // Assistant text parts mirror upstream `assistantTextParts`: whitespace-only
        // text blocks are filtered out (block.text.trim().length > 0) before joining
        // or spreading. Used only by the assistant branch below.
        let text_blocks: Vec<String> = msg.content.iter().filter_map(|b| match b {
            ContentBlock::Text { text, .. } if !text.trim().is_empty() => Some(text.clone()),
            _ => None,
        }).collect();
        let tool_call_blocks: Vec<Value> = msg.content.iter().filter_map(|b| match b {
            ContentBlock::ToolCall { id, name, arguments, .. } => Some(json!({
                "id": normalize_tool_call_id(id, &model.provider),
                "type": "function",
                "function": {
                    "name": name,
                    "arguments": serde_json::to_string(arguments).unwrap_or_else(|_| "{}".to_string()),
                }
            })),
            _ => None,
        }).collect();

        let content: Value = if msg.role == Role::Assistant {
            // Collect non-empty thinking blocks for replay handling.
            let thinking_blocks: Vec<(&String, &Option<String>)> = msg.content.iter().filter_map(|b| match b {
                ContentBlock::Thinking { thinking, thinking_signature, .. } if !thinking.trim().is_empty() => Some((thinking, thinking_signature)),
                _ => None,
            }).collect();
            let assistant_text = if text_blocks.is_empty() { String::new() } else { text_blocks.join("") };

            if !thinking_blocks.is_empty() && compat.requires_thinking_as_text == Some(true) {
                // Convert thinking blocks into a leading text block (no tags), then
                // spread the assistant text parts as separate items (matches upstream
                // `[{thinkingText}, ...assistantTextParts]`).
                let thinking_text = thinking_blocks.iter().map(|(t, _)| t.as_str()).collect::<Vec<_>>().join("\n\n");
                let mut parts = vec![json!({"type": "text", "text": thinking_text})];
                for t in &text_blocks {
                    parts.push(json!({"type": "text", "text": t}));
                }
                json!(parts)
            } else if assistant_text.is_empty() {
                // Empty assistant content: some providers reject null, use "" when bridging.
                if compat.requires_assistant_after_tool_result == Some(true) { json!("") } else { Value::Null }
            } else {
                json!(assistant_text)
            }
        } else if msg.content.len() == 1 {
            match &msg.content[0] {
                ContentBlock::Text { text, .. } => json!(text),
                _ => json!(format_content_blocks(&msg.content)),
            }
        } else {
            json!(format_content_blocks(&msg.content))
        };

        // Mirror upstream convertMessages: skip a user message whose converted content
        // is an empty array (`if (content.length === 0) continue`). Like upstream, this
        // does not update last_role.
        if msg.role == Role::User
            && content.as_array().map(|a| a.is_empty()).unwrap_or(false) {
            idx += 1;
            continue;
        }

        let mut m = json!({ "role": role_str, "content": content });
        let mut should_push = true;
        if msg.role == Role::Assistant {
            if !tool_call_blocks.is_empty() {
                m["tool_calls"] = json!(tool_call_blocks);
                // Replay per-tool-call reasoning: each thoughtSignature is JSON that
                // upstream collects into `reasoning_details` (e.g. OpenRouter).
                let reasoning_details: Vec<Value> = msg.content.iter().filter_map(|b| match b {
                    ContentBlock::ToolCall { thought_signature: Some(sig), .. } if !sig.is_empty() => {
                        serde_json::from_str::<Value>(sig).ok()
                    }
                    _ => None,
                }).collect();
                if !reasoning_details.is_empty() {
                    m["reasoning_details"] = json!(reasoning_details);
                }
            }
            // When not sending thinking-as-text, replay thinking via its signature field
            // (e.g. reasoning_content for llama.cpp / gpt-oss).
            if compat.requires_thinking_as_text != Some(true) {
                let thinking_blocks: Vec<(&String, &Option<String>)> = msg.content.iter().filter_map(|b| match b {
                    ContentBlock::Thinking { thinking, thinking_signature, .. } if !thinking.trim().is_empty() => Some((thinking, thinking_signature)),
                    _ => None,
                }).collect();
                if let Some((_, Some(sig))) = thinking_blocks.first()
                    && !sig.is_empty() {
                        // opencode-go uses `reasoning_content` as the replay key.
                        let key = if model.provider == "opencode-go" && sig.as_str() == "reasoning" {
                            "reasoning_content"
                        } else {
                            sig.as_str()
                        };
                        let joined = thinking_blocks.iter().map(|(t, _)| t.as_str()).collect::<Vec<_>>().join("\n");
                        m[key] = json!(joined);
                    }
            }
            // DeepSeek-style providers require reasoning_content on assistant messages.
            if compat.requires_reasoning_content_on_assistant_messages == Some(true)
                && model.reasoning
                && m.get("reasoning_content").is_none() {
                m["reasoning_content"] = json!("");
            }
            // Skip empty assistant messages (no content and no tool calls).
            let has_content = !matches!(m["content"], Value::Null)
                && !(m["content"].as_str().map(|s| s.is_empty()).unwrap_or(false));
            if !has_content && tool_call_blocks.is_empty() {
                should_push = false;
            }
        }
        if should_push {
            messages.push(m);
        }
        last_role = Some(msg.role.clone());
        idx += 1;
    }

    let max_tokens_field = compat.max_tokens_field.as_deref().unwrap_or("max_completion_tokens");

    let mut payload = json!({
        "model": model.id,
        "messages": messages,
        "stream": true,
    });

    if compat.supports_usage_in_streaming != Some(false) {
        payload["stream_options"] = json!({ "include_usage": true });
    }
    if compat.supports_store == Some(true) {
        payload["store"] = json!(false);
    }

    // Prompt caching: OpenAI uses prompt_cache_key (from session id) on api.openai.com
    // (unless cache is disabled) or for long retention on supporting providers.
    let retention = crate::prompt_cache::resolve_cache_retention(opts.cache_retention.as_ref());
    let cache_none = retention == CacheRetention::None;
    let cache_long = retention == CacheRetention::Long;
    if let Some(ref session_id) = opts.session_id {
        let on_openai = model.base_url.contains("api.openai.com");
        if (on_openai && !cache_none) || (cache_long && compat.supports_long_cache_retention != Some(false)) {
            payload["prompt_cache_key"] = json!(crate::prompt_cache::clamp_openai_prompt_cache_key(session_id));
        }
    }
    if cache_long && compat.supports_long_cache_retention != Some(false) {
        payload["prompt_cache_retention"] = json!("24h");
    }

    // Upstream gates max tokens on a truthy check, so a 0 is treated as unset.
    if let Some(max) = opts.max_tokens.filter(|m| *m != 0) {
        payload[max_tokens_field] = json!(max);
    }

    if let Some(temp) = opts.temperature {
        payload["temperature"] = json!(temp);
    }

    // Reasoning/thinking (clamped to the model's supported levels).
    // Mirrors upstream buildParams thinking-format handling, gated on model.reasoning.
    let clamped_effort = opts.reasoning.as_ref().and_then(|l| crate::simple_options::clamp_reasoning_for_model(model, l));
    if model.reasoning {
        let map_effort = |level: &ThinkingLevel| -> String {
            let key = format!("{:?}", level).to_lowercase();
            model.thinking_level_map.as_ref()
                .and_then(|m| m.get(&key))
                .and_then(|v| v.clone())
                .unwrap_or(key)
        };
        let off_value = || -> Option<String> {
            match model.thinking_level_map.as_ref().and_then(|m| m.get("off")) {
                Some(Some(s)) => Some(s.clone()),
                Some(None) => None,           // explicitly disabled
                None => Some("none".to_string()),
            }
        };
        match compat.thinking_format.as_deref() {
            Some("zai") => {
                payload["thinking"] = json!({"type": if clamped_effort.is_some() { "enabled" } else { "disabled" }});
                if let Some(ref level) = clamped_effort
                    && compat.supports_reasoning_effort == Some(true) {
                        // effort = thinkingLevelMap[level] (string) else the level; null -> omit.
                        let key = format!("{:?}", level).to_lowercase();
                        match model.thinking_level_map.as_ref().and_then(|m| m.get(&key)) {
                            None => { payload["reasoning_effort"] = json!(key); }
                            Some(Some(s)) => { payload["reasoning_effort"] = json!(s); }
                            Some(None) => {}
                        }
                    }
            }
            Some("qwen") => {
                payload["enable_thinking"] = json!(clamped_effort.is_some());
            }
            Some("qwen-chat-template") => {
                payload["chat_template_kwargs"] = json!({
                    "enable_thinking": clamped_effort.is_some(),
                    "preserve_thinking": true,
                });
            }
            Some("string-thinking") => {
                if let Some(ref level) = clamped_effort {
                    payload["thinking"] = json!(map_effort(level));
                } else if let Some(off) = off_value() {
                    payload["thinking"] = json!(off);
                }
            }
            Some("together") => {
                payload["reasoning"] = json!({"enabled": clamped_effort.is_some()});
                if let Some(ref level) = clamped_effort
                    && compat.supports_reasoning_effort == Some(true) {
                        payload["reasoning_effort"] = json!(map_effort(level));
                    }
            }
            Some("deepseek") => {
                // thinking enabled when effort requested; else disabled UNLESS
                // thinkingLevelMap.off is explicitly null (then omitted).
                if clamped_effort.is_some() {
                    payload["thinking"] = json!({"type": "enabled"});
                } else if !matches!(model.thinking_level_map.as_ref().and_then(|m| m.get("off")), Some(None)) {
                    payload["thinking"] = json!({"type": "disabled"});
                }
                if let Some(ref level) = clamped_effort
                    && compat.supports_reasoning_effort == Some(true) {
                        payload["reasoning_effort"] = json!(map_effort(level));
                    }
            }
            Some("openrouter") => {
                if let Some(ref level) = clamped_effort {
                    payload["reasoning"] = json!({"effort": map_effort(level)});
                } else if let Some(off) = off_value() {
                    payload["reasoning"] = json!({"effort": off});
                }
            }
            Some("ant-ling") => {
                if let Some(ref level) = clamped_effort {
                    let key = format!("{:?}", level).to_lowercase();
                    if let Some(Some(mapped)) = model.thinking_level_map.as_ref().map(|m| m.get(&key).cloned().flatten()) {
                        payload["reasoning"] = json!({"effort": mapped});
                    }
                }
            }
            Some("chat-template") => {
                if let Some(kwargs) = build_chat_template_kwargs(model, &clamped_effort, compat) {
                    payload["chat_template_kwargs"] = kwargs;
                }
            }
            _ => {
                if compat.supports_reasoning_effort == Some(true) {
                    if let Some(ref level) = clamped_effort {
                        payload["reasoning_effort"] = json!(map_effort(level));
                    } else if let Some(Some(off)) = model.thinking_level_map.as_ref().map(|m| m.get("off").cloned().flatten()) {
                        payload["reasoning_effort"] = json!(off);
                    }
                }
            }
        }
    }

    // Tools
    if !context.tools.is_empty() {
        let include_strict = compat.supports_strict_mode != Some(false);
        let tools: Vec<Value> = context.tools.iter().map(|t| {
            let mut function = json!({
                "name": t.name,
                "description": t.description,
                "parameters": t.parameters,
            });
            if include_strict {
                function["strict"] = json!(false);
            }
            json!({ "type": "function", "function": function })
        }).collect();
        payload["tools"] = json!(tools);
        if compat.zai_tool_stream == Some(true) {
            payload["tool_stream"] = json!(true);
        }
    } else if has_tool_history(&context.messages) {
        // Anthropic via LiteLLM/proxy requires a tools param when the conversation
        // already contains tool_calls/tool_results.
        payload["tools"] = json!([]);
    }

    if let Some(ref tool_choice) = opts.tool_choice {
        payload["tool_choice"] = tool_choice.clone();
    }

    // OpenRouter provider-routing preferences (sent verbatim as `provider`).
    if let Some(ref routing) = model.compat.open_router_routing {
        payload["provider"] = routing.clone();
    }
    // Vercel AI Gateway routing preferences -> providerOptions.gateway.
    // 0.80.x: gated solely on compat.vercelGatewayRouting (baseURL check removed).
    if let Some(ref routing) = model.compat.vercel_gateway_routing {
        let only = routing.get("only");
        let order = routing.get("order");
        if only.is_some() || order.is_some() {
            let mut gateway = json!({});
            if let Some(o) = only { gateway["only"] = o.clone(); }
            if let Some(o) = order { gateway["order"] = o.clone(); }
            payload["providerOptions"] = json!({ "gateway": gateway });
        }
    }

    // OpenRouter Anthropic models use Anthropic-style cache_control on system/last-message/last-tool.
    if compat.cache_control_format.as_deref() == Some("anthropic") && retention != CacheRetention::None {
        let ttl_long = retention == CacheRetention::Long && compat.supports_long_cache_retention != Some(false);
        let cc = if ttl_long {
            json!({"type": "ephemeral", "ttl": "1h"})
        } else {
            json!({"type": "ephemeral"})
        };
        apply_anthropic_cache_control(&mut payload, &cc);
    }

    payload
}

/// Apply Anthropic-style cache_control to the system prompt, last tool, and last
/// conversation message of an OpenAI-completions payload (mirrors applyAnthropicCacheControl).
fn apply_anthropic_cache_control(payload: &mut Value, cc: &Value) {
    if let Some(messages) = payload.get_mut("messages").and_then(|m| m.as_array_mut()) {
        // System/developer prompt: first such message.
        if let Some(msg) = messages.iter_mut().find(|m| {
            matches!(m.get("role").and_then(|r| r.as_str()), Some("system") | Some("developer"))
        }) {
            add_cache_control_to_text(msg, cc);
        }
        // Last user/assistant message (from the end) whose text content accepts it.
        for msg in messages.iter_mut().rev() {
            if matches!(msg.get("role").and_then(|r| r.as_str()), Some("user") | Some("assistant"))
                && add_cache_control_to_text(msg, cc) {
                break;
            }
        }
    }
    // Last tool definition.
    if let Some(tools) = payload.get_mut("tools").and_then(|t| t.as_array_mut())
        && let Some(last) = tools.last_mut() {
        last["cache_control"] = cc.clone();
    }
}

/// Stamp cache_control on a message's last text part (converting string content to an
/// array when needed). Returns true when applied (mirrors addCacheControlToTextContent).
fn add_cache_control_to_text(msg: &mut Value, cc: &Value) -> bool {
    match msg.get("content").cloned() {
        Some(Value::String(s)) => {
            if s.is_empty() {
                return false;
            }
            msg["content"] = json!([{ "type": "text", "text": s, "cache_control": cc }]);
            true
        }
        Some(Value::Array(_)) => {
            if let Some(parts) = msg.get_mut("content").and_then(|c| c.as_array_mut()) {
                for part in parts.iter_mut().rev() {
                    if part.get("type").and_then(|t| t.as_str()) == Some("text") {
                        part["cache_control"] = cc.clone();
                        return true;
                    }
                }
            }
            false
        }
        _ => false,
    }
}

/// Whether the conversation already contains tool calls or tool results
/// (mirrors upstream hasToolHistory).
fn has_tool_history(messages: &[Message]) -> bool {
    messages.iter().any(|m| {
        m.role == Role::ToolResult
            || (m.role == Role::Assistant
                && m.content.iter().any(|b| matches!(b, ContentBlock::ToolCall { .. })))
    })
}

/// Normalize a tool-call ID for OpenAI-compatible APIs (mirrors upstream `normalizeToolCallId`).
/// Pipe-separated IDs (from the Responses API) are reduced to the sanitized call_id,
/// and overly-long OpenAI IDs are truncated to 40 chars.
/// Build the `chat_template_kwargs` object for the `chat-template` thinking format
/// (mirrors buildChatTemplateKwargs): resolve each configured value, dropping any
/// that resolve to undefined; returns None when the result is empty.
fn build_chat_template_kwargs(
    model: &Model,
    clamped_effort: &Option<ThinkingLevel>,
    compat: &crate::compat::OpenAICompletionsCompat,
) -> Option<Value> {
    let template = compat.chat_template_kwargs.as_ref()?.as_object()?;
    let mut out = serde_json::Map::new();
    for (key, value) in template {
        if let Some(resolved) = resolve_chat_template_kwarg_value(model, clamped_effort, value) {
            out.insert(key.clone(), resolved);
        }
    }
    if out.is_empty() { None } else { Some(Value::Object(out)) }
}

/// Resolve a single chat-template kwarg value (mirrors resolveChatTemplateKwargValue).
/// Non-object values pass through literally; object values support `omitWhenOff`,
/// `$var: "thinking.enabled"`, and thinkingLevelMap-based effort resolution.
fn resolve_chat_template_kwarg_value(
    model: &Model,
    clamped_effort: &Option<ThinkingLevel>,
    value: &Value,
) -> Option<Value> {
    let obj = match value.as_object() {
        Some(o) => o,
        None => return Some(value.clone()), // literal passthrough
    };
    if clamped_effort.is_none() && obj.get("omitWhenOff").and_then(|v| v.as_bool()) == Some(true) {
        return None;
    }
    if obj.get("$var").and_then(|v| v.as_str()) == Some("thinking.enabled") {
        return Some(Value::Bool(clamped_effort.is_some()));
    }
    // mappedValue = effort ? thinkingLevelMap[effort] : thinkingLevelMap.off;
    // undefined -> reasoningEffort (the level string, or undefined when off).
    let (lookup_key, effort_string): (String, Option<String>) = match clamped_effort {
        Some(level) => {
            let k = format!("{:?}", level).to_lowercase();
            (k.clone(), Some(k))
        }
        None => ("off".to_string(), None),
    };
    match model.thinking_level_map.as_ref().and_then(|m| m.get(&lookup_key)) {
        None => effort_string.map(Value::String),
        Some(Some(s)) => Some(Value::String(s.clone())),
        Some(None) => None,
    }
}

pub(crate) fn normalize_tool_call_id(id: &str, provider: &str) -> String {
    if let Some((call_id, _)) = id.split_once('|') {
        return call_id
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() || c == '_' || c == '-' { c } else { '_' })
            .take(40)
            .collect();
    }
    if provider == "openai" && id.len() > 40 {
        return id.chars().take(40).collect();
    }
    id.to_string()
}

fn format_content_blocks(blocks: &[ContentBlock]) -> Vec<Value> {
    blocks.iter().map(|b| match b {
        ContentBlock::Text { text, .. } => json!({"type": "text", "text": text}),
        ContentBlock::Image { data, mime_type } => json!({
            "type": "image_url",
            "image_url": {"url": format!("data:{};base64,{}", mime_type, data)}
        }),
        ContentBlock::Thinking { thinking, .. } => json!({"type": "text", "text": thinking}),
        ContentBlock::ToolCall { id: _, name, arguments, .. } => json!({
            "type": "text",
            "text": format!("[tool_call: {} {}]", name, serde_json::to_string(arguments).unwrap_or_default())
        }),
    }).collect()
}
