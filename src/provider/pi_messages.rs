//! pi-messages provider: pi's native assistant-message SSE protocol.

use std::collections::HashMap;
use std::sync::Arc;

use futures::{StreamExt, stream};
use reqwest::header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::env::client_api_key;
use crate::events::Event;
use crate::transports::sse::SseParser;
use crate::types::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PiMessagesRewriteImpact {
    pub policy_id: String,
    pub policy_version: u32,
    pub changed: bool,
    pub token_count_change: i64,
    pub message_count_change: i64,
    pub system_prompt_changed: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
enum PiMessagesEvent {
    Start,
    TextStart {
        content_index: usize,
    },
    TextDelta {
        content_index: usize,
        delta: String,
    },
    TextEnd {
        content_index: usize,
        content: String,
        content_signature: Option<String>,
    },
    ThinkingStart {
        content_index: usize,
    },
    ThinkingDelta {
        content_index: usize,
        delta: String,
    },
    ThinkingEnd {
        content_index: usize,
        content: String,
        content_signature: Option<String>,
        #[serde(default)]
        redacted: bool,
    },
    ToolcallStart {
        content_index: usize,
        id: String,
        tool_name: String,
    },
    ToolcallDelta {
        content_index: usize,
        delta: String,
    },
    ToolcallEnd {
        content_index: usize,
        tool_call: ContentBlock,
    },
    Done {
        reason: StopReason,
        usage: Usage,
        response_id: Option<String>,
        rewrite: Option<PiMessagesRewriteImpact>,
    },
    Error {
        reason: StopReason,
        usage: Usage,
        error_message: Option<String>,
        response_id: Option<String>,
        rewrite: Option<PiMessagesRewriteImpact>,
    },
}

fn empty_usage() -> Usage {
    Usage::default()
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn assistant_message(model: &Model) -> Message {
    Message {
        role: Role::Assistant,
        content: Vec::new(),
        timestamp: now_ms(),
        api: Some(model.api.clone()),
        provider: Some(model.provider.clone()),
        model: Some(model.id.clone()),
        response_id: None,
        response_model: None,
        diagnostics: Vec::new(),
        usage: Some(empty_usage()),
        stop_reason: Some(StopReason::Stop),
        error_message: None,
        tool_call_id: None,
        tool_name: None,
        is_error: false,
        details: None,
        added_tool_names: Vec::new(),
    }
}

fn put_content(content: &mut Vec<ContentBlock>, idx: usize, block: ContentBlock) {
    if content.len() <= idx {
        content.resize_with(idx + 1, || ContentBlock::Text {
            text: String::new(),
            text_signature: None,
        });
    }
    content[idx] = block;
}

fn append_rewrite_diagnostic(message: &mut Message, rewrite: Option<PiMessagesRewriteImpact>) {
    let Some(rewrite) = rewrite else {
        return;
    };
    let mut details = HashMap::new();
    details.insert("policyId".into(), json!(rewrite.policy_id));
    details.insert("policyVersion".into(), json!(rewrite.policy_version));
    details.insert("changed".into(), json!(rewrite.changed));
    details.insert("tokenCountChange".into(), json!(rewrite.token_count_change));
    details.insert(
        "messageCountChange".into(),
        json!(rewrite.message_count_change),
    );
    details.insert(
        "systemPromptChanged".into(),
        json!(rewrite.system_prompt_changed),
    );
    message.diagnostics.push(AssistantMessageDiagnostic {
        diagnostic_type: "pi_messages_rewrite".into(),
        timestamp: now_ms(),
        error: DiagnosticError {
            name: None,
            message: "pi-messages rewrite impact".into(),
            stack: None,
            code: None,
        },
        details: Some(details),
    });
}

fn convert_event(
    model: &Model,
    partial: &mut Message,
    event: PiMessagesEvent,
    tool_json: &mut HashMap<usize, String>,
) -> Event {
    match event {
        PiMessagesEvent::Start => Event::Start {
            partial: partial.clone(),
        },
        PiMessagesEvent::TextStart { content_index } => {
            put_content(
                &mut partial.content,
                content_index,
                ContentBlock::Text {
                    text: String::new(),
                    text_signature: None,
                },
            );
            Event::TextStart
        }
        PiMessagesEvent::TextDelta {
            content_index,
            delta,
        } => {
            if let Some(ContentBlock::Text { text, .. }) = partial.content.get_mut(content_index) {
                text.push_str(&delta);
            }
            Event::TextDelta { delta }
        }
        PiMessagesEvent::TextEnd {
            content_index,
            content,
            content_signature,
        } => {
            put_content(
                &mut partial.content,
                content_index,
                ContentBlock::Text {
                    text: content,
                    text_signature: content_signature,
                },
            );
            Event::TextEnd
        }
        PiMessagesEvent::ThinkingStart { content_index } => {
            put_content(
                &mut partial.content,
                content_index,
                ContentBlock::Thinking {
                    thinking: String::new(),
                    thinking_signature: None,
                    redacted: false,
                },
            );
            Event::ThinkingStart
        }
        PiMessagesEvent::ThinkingDelta {
            content_index,
            delta,
        } => {
            if let Some(ContentBlock::Thinking { thinking, .. }) =
                partial.content.get_mut(content_index)
            {
                thinking.push_str(&delta);
            }
            Event::ThinkingDelta { delta }
        }
        PiMessagesEvent::ThinkingEnd {
            content_index,
            content,
            content_signature,
            redacted,
        } => {
            put_content(
                &mut partial.content,
                content_index,
                ContentBlock::Thinking {
                    thinking: content,
                    thinking_signature: content_signature,
                    redacted,
                },
            );
            Event::ThinkingEnd
        }
        PiMessagesEvent::ToolcallStart {
            content_index,
            id,
            tool_name,
        } => {
            put_content(
                &mut partial.content,
                content_index,
                ContentBlock::ToolCall {
                    id: id.clone(),
                    name: tool_name.clone(),
                    arguments: HashMap::new(),
                    thought_signature: None,
                },
            );
            Event::ToolCallStart {
                id,
                name: tool_name,
            }
        }
        PiMessagesEvent::ToolcallDelta {
            content_index,
            delta,
        } => {
            tool_json.entry(content_index).or_default().push_str(&delta);
            Event::ToolCallDelta { delta }
        }
        PiMessagesEvent::ToolcallEnd {
            content_index,
            tool_call,
        } => {
            let (id, name, arguments) = match &tool_call {
                ContentBlock::ToolCall {
                    id,
                    name,
                    arguments,
                    ..
                } => (id.clone(), name.clone(), json!(arguments)),
                _ => (String::new(), String::new(), Value::Null),
            };
            put_content(&mut partial.content, content_index, tool_call);
            tool_json.remove(&content_index);
            Event::ToolCallEnd {
                id,
                name,
                arguments,
            }
        }
        PiMessagesEvent::Done {
            reason,
            usage,
            response_id,
            rewrite,
        } => {
            partial.stop_reason = Some(reason.clone());
            partial.usage = Some(usage);
            partial.response_id = response_id;
            append_rewrite_diagnostic(partial, rewrite);
            Event::Done {
                reason,
                message: partial.clone(),
            }
        }
        PiMessagesEvent::Error {
            reason,
            usage,
            error_message,
            response_id,
            rewrite,
        } => {
            partial.stop_reason = Some(reason.clone());
            partial.usage = Some(usage);
            partial.error_message = error_message.clone();
            partial.response_id = response_id;
            append_rewrite_diagnostic(partial, rewrite);
            Event::Error {
                reason,
                error: Arc::from(Box::<dyn std::error::Error + Send + Sync>::from(
                    error_message.unwrap_or_else(|| format!("{} error", model.provider)),
                )),
                message: Some(partial.clone()),
            }
        }
    }
}

pub fn stream_pi_messages<'a>(
    model: &'a Model,
    context: &'a Context,
    opts: &'a StreamOptions,
) -> std::pin::Pin<Box<dyn futures::Stream<Item = Event> + Send + 'a>> {
    let api_key = match client_api_key(model, opts) {
        Some(k) => k,
        None => {
            return Box::pin(stream::once(async move {
                Event::Error {
                    reason: StopReason::Error,
                    error: Arc::from(Box::<dyn std::error::Error + Send + Sync>::from(format!(
                        "No API key provided for provider \"{}\"",
                        model.provider
                    ))),
                    message: None,
                }
            }));
        }
    };

    let mut base = model.base_url.trim_end_matches('/').to_string();
    base.push_str("/messages");
    let mut url = match reqwest::Url::parse(&base) {
        Ok(u) => u,
        Err(e) => {
            return Box::pin(stream::once(async move {
                Event::Error {
                    reason: StopReason::Error,
                    error: Arc::new(e),
                    message: None,
                }
            }));
        }
    };
    if opts
        .metadata
        .as_ref()
        .and_then(|m| m.get("debug"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        url.query_pairs_mut().append_pair("debug", "1");
    }

    let cache_retention = opts.cache_retention.clone().or_else(|| {
        match std::env::var("PI_CACHE_RETENTION").ok().as_deref() {
            Some("long") => Some(CacheRetention::Long),
            _ => None,
        }
    });
    let mut payload = json!({ "model": model.id, "context": context, "options": { "temperature": opts.temperature, "maxTokens": opts.max_tokens, "reasoning": opts.reasoning, "cacheRetention": cache_retention, "sessionId": opts.session_id, "toolChoice": opts.tool_choice } });
    if let Some(hook) = &opts.on_payload {
        match hook(payload.clone(), model) {
            Ok(v) => payload = v,
            Err(e) => {
                return Box::pin(stream::once(async move {
                    Event::Error {
                        reason: StopReason::Error,
                        error: Arc::from(e),
                        message: None,
                    }
                }));
            }
        }
    }

    let mut headers = HeaderMap::new();
    headers.insert(
        AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {api_key}"))
            .unwrap_or_else(|_| HeaderValue::from_static("Bearer")),
    );
    headers.insert(ACCEPT, HeaderValue::from_static("text/event-stream"));
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    if let Some(h) = &opts.headers {
        for (k, v) in h {
            if let (Ok(name), Ok(value)) = (
                reqwest::header::HeaderName::from_bytes(k.as_bytes()),
                HeaderValue::from_str(v),
            ) {
                headers.insert(name, value);
            }
        }
    }

    Box::pin(async_stream::stream! {
        let client = crate::http_proxy::client_for_target(url.as_str(), None);
        let mut req = client.post(url.clone()).headers(headers).json(&payload);
        if let Some(ms) = opts.timeout_ms { req = req.timeout(std::time::Duration::from_millis(ms)); }
        let resp = match req.send().await { Ok(r) => r, Err(e) => { yield Event::Error { reason: StopReason::Error, error: Arc::new(e), message: None }; return; } };
        let status = resp.status().as_u16();
        let headers_map: HashMap<String, String> = resp.headers().iter().filter_map(|(k,v)| Some((k.as_str().to_string(), v.to_str().ok()?.to_string()))).collect();
        if let Some(cb) = &opts.on_response { cb(status, &headers_map, model); }
        if !resp.status().is_success() { let body = resp.text().await.unwrap_or_default(); yield Event::Error { reason: StopReason::Error, error: Arc::from(Box::<dyn std::error::Error + Send + Sync>::from(crate::error_body::format_provider_http_error(status, &body, None))), message: None }; return; }
        let mut partial = assistant_message(model);
        let mut parser = SseParser::default();
        let mut tool_json = HashMap::new();
        let mut bytes = resp.bytes_stream();
        while let Some(chunk) = bytes.next().await {
            let chunk = match chunk { Ok(c) => c, Err(e) => { yield Event::Error { reason: StopReason::Error, error: Arc::new(e), message: Some(partial.clone()) }; return; } };
            for ev in parser.feed_bytes(&chunk) {
                if ev.data.trim() == "[DONE]" { continue; }
                match serde_json::from_str::<PiMessagesEvent>(&ev.data) {
                    Ok(pi_ev) => { let out = convert_event(model, &mut partial, pi_ev, &mut tool_json); let terminal = matches!(out, Event::Done { .. } | Event::Error { .. }); yield out; if terminal { return; } }
                    Err(e) => { yield Event::Error { reason: StopReason::Error, error: Arc::new(e), message: Some(partial.clone()) }; return; }
                }
            }
        }
        if let Some(ev) = parser.finish()
            && !ev.data.trim().is_empty() && ev.data.trim() != "[DONE]"
        {
            match serde_json::from_str::<PiMessagesEvent>(&ev.data) {
                Ok(pi_ev) => { let out = convert_event(model, &mut partial, pi_ev, &mut tool_json); let terminal = matches!(out, Event::Done { .. } | Event::Error { .. }); yield out; if terminal { return; } }
                Err(e) => { yield Event::Error { reason: StopReason::Error, error: Arc::new(e), message: Some(partial.clone()) }; return; }
            }
        }
        yield Event::Error { reason: StopReason::Error, error: Arc::from(Box::<dyn std::error::Error + Send + Sync>::from(format!("{} stream ended without a terminal event", model.provider))), message: Some(partial) };
    })
}
