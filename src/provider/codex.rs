//! OpenAI Codex Responses provider (WebSocket + SSE fallback).

use std::sync::Arc;

use futures::stream;
use serde_json::{json, Value};
use tokio_tungstenite::tungstenite;
use crate::env::resolve_api_key;
use crate::events::Event;
use crate::provider::responses;
use crate::transports::sse;
use crate::types::*;

/// Build the Codex User-Agent, mirroring upstream `pi (${os.platform()} ${os.release()}; ${os.arch()})`
/// as closely as std allows. Platform/arch are mapped to Node's naming (darwin/win32, x64/arm64);
/// the OS release is omitted (std exposes no portable release without a libc/uname dependency).
fn codex_user_agent() -> String {
    let platform = match std::env::consts::OS {
        "macos" => "darwin",
        "windows" => "win32",
        other => other,
    };
    let arch = match std::env::consts::ARCH {
        "x86_64" => "x64",
        "aarch64" => "arm64",
        other => other,
    };
    format!("pi ({platform}; {arch})")
}

/// Sessions whose Codex WebSocket transport has failed; subsequent requests for
/// these sessions skip WebSocket and use SSE directly (mirrors upstream
/// websocketSseFallbackSessions).
static WS_FALLBACK_SESSIONS: std::sync::LazyLock<std::sync::Mutex<std::collections::HashSet<String>>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashSet::new()));

/// Codex error code emitted when the server rejects a WebSocket because too many
/// are already open (mirrors upstream WEBSOCKET_CONNECTION_LIMIT_REACHED_CODE).
pub(crate) const WS_CONNECTION_LIMIT_CODE: &str = "websocket_connection_limit_reached";

/// Whether a `try_websocket` error string denotes a connection-limit rejection
/// (mirrors upstream isWebSocketConnectionLimitReachedError), which the caller
/// retries once before falling back to SSE.
pub(crate) fn is_ws_connection_limit_error(err: &str) -> bool {
    err.starts_with(WS_CONNECTION_LIMIT_CODE)
}

fn ws_fallback_active(session_id: Option<&str>) -> bool {
    match session_id {
        Some(s) => WS_FALLBACK_SESSIONS.lock().map(|set| set.contains(s)).unwrap_or(false),
        None => false,
    }
}

fn record_ws_fallback(session_id: Option<&str>) {
    if let Some(s) = session_id
        && let Ok(mut set) = WS_FALLBACK_SESSIONS.lock() {
        set.insert(s.to_string());
    }
}

/// Clear the recorded WebSocket-fallback state for a session (or all sessions).
pub fn clear_ws_fallback(session_id: Option<&str>) {
    if let Ok(mut set) = WS_FALLBACK_SESSIONS.lock() {
        match session_id {
            Some(s) => { set.remove(s); }
            None => set.clear(),
        }
    }
}

/// Start a Codex stream (WebSocket with SSE fallback).
pub fn stream_codex<'a>(
    model: &'a Model,
    context: &'a Context,
    opts: &'a StreamOptions,
) -> std::pin::Pin<Box<dyn futures::Stream<Item = Event> + Send + 'a>> {
    let api_key = resolve_api_key(model, opts);
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

    let mut payload = build_codex_payload(model, context, opts);
    if let Some(ref hook) = opts.on_payload {
        match hook(payload.clone(), model) {
            Ok(next) => payload = next,
            Err(err) => {
                let err = Event::Error { reason: StopReason::Error, error: Arc::from(err), message: None };
                return Box::pin(stream::once(async { err }));
            }
        }
    }

    Box::pin(async_stream::stream! {
        // Try WebSocket first, fall back to SSE
        let ws_url = format!(
            "{}/responses?model={}&stream=true",
            model.base_url.trim_end_matches('/').replace("https://", "wss://").replace("http://", "ws://"),
            &model.id
        );

        // Reuse the SSE transport for the rest of a session once its WebSocket has failed
        // (mirrors upstream's sticky websocketSseFallbackSessions behavior). Also honor an
        // explicit `transport: "sse"` request to skip WebSocket entirely.
        let force_sse = opts.transport == Some(Transport::Sse);
        let skip_ws = force_sse || ws_fallback_active(opts.session_id.as_deref());
        let mut transport_diagnostic: Option<crate::types::AssistantMessageDiagnostic> = None;
        let mut do_sse = skip_ws;
        if !skip_ws {
            // Retry the WebSocket connection once on a pre-start connection-limit
            // rejection before falling back to SSE (mirrors upstream's
            // retriedWebSocketConnectionLimit logic).
            let mut retried_connection_limit = false;
            loop {
            match try_websocket(&ws_url, &api_key, model, opts, &payload).await {
                Ok(events) => {
                    for evt in events {
                        yield evt;
                    }
                    break;
                }
                Err(ws_err) => {
                    if !retried_connection_limit && is_ws_connection_limit_error(&ws_err) {
                        retried_connection_limit = true;
                        continue;
                    }
                    // WebSocket transport failed; remember the fallback for this session and
                    // record a diagnostic (mirrors recordWebSocketFailure + appendAssistantMessageDiagnostic).
                    record_ws_fallback(opts.session_id.as_deref());
                    transport_diagnostic = Some(crate::types::AssistantMessageDiagnostic {
                        diagnostic_type: "provider_transport_failure".to_string(),
                        timestamp: crate::utils::now_millis(),
                        error: crate::types::DiagnosticError {
                            name: Some("TransportError".to_string()),
                            message: ws_err.to_string(),
                            stack: None,
                            code: None,
                        },
                        details: Some(std::collections::HashMap::from([
                            ("configuredTransport".to_string(), serde_json::json!(match opts.transport {
                                Some(Transport::Sse) => "sse",
                                Some(Transport::Websocket) => "websocket",
                                Some(Transport::WebsocketCached) => "websocket-cached",
                                Some(Transport::Auto) | None => "auto",
                            })),
                            ("fallbackTransport".to_string(), serde_json::json!("sse")),
                            ("eventsEmitted".to_string(), serde_json::json!(false)),
                            ("phase".to_string(), serde_json::json!("before_message_stream_start")),
                            ("requestBytes".to_string(), serde_json::json!(
                                serde_json::to_vec(&payload).map(|v| v.len()).unwrap_or(0)
                            )),
                        ])),
                    });
                    do_sse = true;
                    break;
                }
            }
            }
        }
        if do_sse {
                // Fallback to SSE using the Codex request body and headers.
                let url = format!("{}/responses", model.base_url.trim_end_matches('/'));
                let account_id = crate::oauth::codex_account_id(&api_key);
                let user_agent = codex_user_agent();
                let client = crate::http_proxy::client_for_target(&url, None);
                let mut req = client
                    .post(&url)
                    .header("content-type", "application/json")
                    .header("accept", "text/event-stream")
                    .header("OpenAI-Beta", "responses=experimental")
                    .header("authorization", format!("Bearer {}", api_key))
                    .header("originator", "pi")
                    .header("User-Agent", user_agent);
                if let Some(ref aid) = account_id {
                    req = req.header("chatgpt-account-id", aid);
                }
                if let Some(sid) = opts.session_id.as_deref().filter(|s| !s.is_empty()) {
                    req = req.header("session-id", sid).header("x-client-request-id", sid);
                }
                if let Some(ref mh) = model.headers {
                    for (k, v) in mh {
                        req = req.header(k, v);
                    }
                }
                let mut req = req.json(&payload);
                if let Some(ms) = opts.timeout_ms {
                    req = req.timeout(std::time::Duration::from_millis(ms));
                }
                let resp = req.send().await;
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
                // Invoke the on_response hook (mirrors options.onResponse).
                if let Some(ref hook) = opts.on_response {
                    let status = resp.status().as_u16();
                    let mut hdrs = std::collections::HashMap::new();
                    for (k, v) in resp.headers().iter() {
                        hdrs.insert(k.to_string(), v.to_str().unwrap_or("").to_string());
                    }
                    hook(status, &hdrs, model);
                }
                if !resp.status().is_success() {
                    let status = resp.status().as_u16();
                    let body = resp.text().await.unwrap_or_default();
                    let msg = parse_codex_error_response(&body, status);
                    yield Event::Error {
                        reason: StopReason::Error,
                        error: Arc::from(Box::<dyn std::error::Error + Send + Sync>::from(msg)),
                        message: None,
                    };
                    return;
                }

                use futures::StreamExt;
                let mut parser = sse::SseParser::default();
                let mut state = CodexWsState::new(model);
                state.service_tier = opts.service_tier.clone();
                if let Some(d) = transport_diagnostic {
                    state.partial.diagnostics.push(d);
                }
                let mut byte_stream = resp.bytes_stream();
                let mut emitted = 0usize;
                let mut done = false;
                while let Some(chunk_result) = byte_stream.next().await {
                    let chunk = match chunk_result {
                        Ok(c) => c,
                        Err(e) => {
                            yield Event::Error {
                                reason: StopReason::Error,
                                error: Arc::from(Box::new(e) as Box<dyn std::error::Error + Send + Sync>),
                                message: None,
                            };
                            return;
                        }
                    };
                    for evt in parser.feed_bytes(&chunk) {
                        if evt.event == sse::EVENT_ERROR { continue; }
                        if let Ok(data) = serde_json::from_str::<Value>(&evt.data) {
                            done = state.process_event(&data);
                            if done { break; }
                        }
                    }
                    while emitted < state.events.len() {
                        yield state.events[emitted].clone();
                        emitted += 1;
                    }
                    if done { break; }
                }
                if !done {
                    // Codex SSE is consumed via the shared responses-stream decoder
                    // (processStream -> processResponsesStream upstream), which throws this
                    // exact message when the stream ends without a terminal response event.
                    yield Event::Error {
                        reason: StopReason::Error,
                        error: Arc::from(Box::<dyn std::error::Error + Send + Sync>::from(
                            "OpenAI Responses stream ended before a terminal response event".to_string(),
                        )),
                        message: Some(state.partial.clone()),
                    };
                } else {
                    let final_events = state.finish();
                    while emitted < final_events.len() {
                        yield final_events[emitted].clone();
                        emitted += 1;
                    }
                }
        }
    })
}

async fn try_websocket(
    ws_url: &str,
    api_key: &str,
    model: &Model,
    opts: &StreamOptions,
    payload: &Value,
) -> Result<Vec<Event>, String> {
    use tokio_tungstenite::connect_async;
    use futures::SinkExt;

    let account_id = crate::oauth::codex_account_id(api_key);
    let user_agent = codex_user_agent();
    // Upstream: `sessionId || createCodexRequestId()` (truthy), so an empty session id
    // gets a fresh request id rather than being used verbatim.
    let request_id = opts.session_id.clone().filter(|s| !s.is_empty())
        .unwrap_or_else(|| format!("req_{}", crate::utils::now_millis()));
    // A fully-built http::Request bypasses tungstenite's automatic handshake-header
    // generation, so we must supply Host + the RFC6455 upgrade headers (including a
    // fresh Sec-WebSocket-Key) ourselves or the server rejects the handshake.
    let host = url::Url::parse(ws_url).ok()
        .and_then(|u| u.host_str().map(|h| match u.port() {
            Some(p) => format!("{h}:{p}"),
            None => h.to_string(),
        }))
        .unwrap_or_default();
    let mut builder = tungstenite::http::Request::builder()
        .uri(ws_url)
        .header("Host", host)
        .header("Connection", "Upgrade")
        .header("Upgrade", "websocket")
        .header("Sec-WebSocket-Version", "13")
        .header("Sec-WebSocket-Key", tungstenite::handshake::client::generate_key())
        .header("Authorization", format!("Bearer {}", api_key))
        .header("originator", "pi")
        .header("User-Agent", user_agent)
        .header("OpenAI-Beta", "responses_websockets=2026-02-06")
        .header("x-client-request-id", &request_id)
        .header("session-id", &request_id);
    if let Some(ref aid) = account_id {
        builder = builder.header("chatgpt-account-id", aid);
    }
    if let Some(ref mh) = model.headers {
        for (k, v) in mh {
            builder = builder.header(k.as_str(), v.as_str());
        }
    }
    let request = builder
        .body(())
        .map_err(|e| e.to_string())?;

    let (mut ws, _) = connect_async(request)
        .await
        .map_err(|e| e.to_string())?;

    // Send the request as a `response.create` message: upstream sends
    // JSON.stringify({ type: "response.create", ...requestBody }) over the socket.
    let mut ws_msg = payload.clone();
    if let Some(obj) = ws_msg.as_object_mut() {
        obj.insert("type".to_string(), Value::String("response.create".to_string()));
    }
    ws.send(tungstenite::Message::Text(serde_json::to_string(&ws_msg).unwrap().into()))
        .await
        .map_err(|e| e.to_string())?;

    use futures::StreamExt;
    let mut state = CodexWsState::new(model);
    state.service_tier = opts.service_tier.clone();

    let mut saw_terminal = false;
    while let Some(msg) = ws.next().await {
        let msg = msg.map_err(|e| e.to_string())?;
        let text = match msg {
            tungstenite::Message::Text(t) => t.to_string(),
            tungstenite::Message::Close(_) => break,
            _ => continue,
        };

        let data: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
        // A `websocket_connection_limit_reached` error event means the server rejected
        // this connection because too many are open; upstream treats it as a retryable
        // pre-start transport failure (retry the WS once, then fall back to SSE) rather
        // than surfacing it. Signal it to the caller via the Err marker.
        if data.get("type").and_then(|v| v.as_str()) == Some("error") {
            let code = data.get("code").and_then(|v| v.as_str())
                .or_else(|| data.pointer("/error/code").and_then(|v| v.as_str()));
            if code == Some(WS_CONNECTION_LIMIT_CODE) {
                return Err(format!("{WS_CONNECTION_LIMIT_CODE}: codex websocket connection limit reached"));
            }
        }
        let is_done = state.process_event(&data);
        if is_done {
            saw_terminal = true;
            break;
        }
    }

    // Upstream's processWebSocketStream throws this exact error when the socket closes
    // before a terminal response event (response.completed/done/incomplete), which the
    // caller treats as a WS transport failure and falls back to SSE. Mirror that here
    // rather than reporting a clean Done.
    if !saw_terminal {
        return Err("WebSocket stream closed before response.completed".to_string());
    }

    Ok(state.finish())
}

#[derive(Debug, Clone)]
struct CodexWsState {
    partial: Message,
    model_cost: crate::types::ModelCost,
    model_id: String,
    service_tier: Option<String>,
    events: Vec<Event>,
    current_text: String,
    text_started: bool,
    current_thinking: String,
    current_tool_call_id: Option<String>,
    current_tool_item_id: Option<String>,
    current_tool_name: Option<String>,
    current_tool_args: String,
}

impl CodexWsState {
    fn new(model: &Model) -> Self {
        let partial = Message {
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
        let events = vec![Event::Start { partial: partial.clone() }];
        Self {
            partial,
            model_cost: model.cost.clone(),
            model_id: model.id.clone(),
            service_tier: None,
            events,
            current_text: String::new(),
            text_started: false,
            current_thinking: String::new(),
            current_tool_call_id: None,
            current_tool_item_id: None,
            current_tool_name: None,
            current_tool_args: String::new(),
        }
    }

    fn process_event(&mut self, data: &Value) -> bool {
        let event_type = data.get("type").and_then(|v| v.as_str()).unwrap_or("");
        match event_type {
            "response.created" => {
                if let Some(response) = data.get("response")
                    && let Some(id) = response.get("id").and_then(|v| v.as_str()) {
                    self.partial.response_id = Some(id.to_string());
                }
                // Upstream does not capture response.model into responseModel.
            }
            "response.output_item.added" => {
                if let Some(item) = data.get("item") {
                    match item.get("type").and_then(|v| v.as_str()).unwrap_or("") {
                        "function_call" => {
                            self.current_tool_call_id = item.get("call_id").and_then(|v| v.as_str()).map(|s| s.to_string());
                            self.current_tool_item_id = item.get("id").and_then(|v| v.as_str()).map(|s| s.to_string());
                            self.current_tool_name = item.get("name").and_then(|v| v.as_str()).map(|s| s.to_string());
                            self.current_tool_args = item.get("arguments").and_then(|v| v.as_str()).unwrap_or("").to_string();
                            if let (Some(id), Some(name)) = (self.current_tool_call_id.clone(), self.current_tool_name.clone()) {
                                self.events.push(Event::ToolCallStart { id, name });
                            }
                            if !self.current_tool_args.is_empty() {
                                self.events.push(Event::ToolCallDelta { delta: self.current_tool_args.clone() });
                            }
                        }
                        "reasoning" => self.events.push(Event::ThinkingStart),
                        "message"
                            if !self.text_started => {
                                self.text_started = true;
                                self.events.push(Event::TextStart);
                            }
                        _ => {}
                    }
                }
            }
            "response.content_part.added" => {
                if !self.text_started {
                    self.text_started = true;
                    self.events.push(Event::TextStart);
                }
            }
            "response.output_text.delta" | "response.refusal.delta" => {
                if let Some(delta) = data.get("delta").and_then(|v| v.as_str()) {
                    if !self.text_started {
                        self.text_started = true;
                        self.events.push(Event::TextStart);
                    }
                    self.current_text.push_str(delta);
                    self.events.push(Event::TextDelta { delta: delta.to_string() });
                }
            }
            "response.content_part.done" => {
                if self.text_started {
                    self.text_started = false;
                    self.events.push(Event::TextEnd);
                }
            }
            "response.reasoning_text.delta" | "response.reasoning_summary_text.delta" => {
                if let Some(delta) = data.get("delta").and_then(|v| v.as_str()) {
                    self.current_thinking.push_str(delta);
                    self.events.push(Event::ThinkingDelta { delta: delta.to_string() });
                }
            }
            "response.reasoning_summary_part.done" => {
                // Separate consecutive reasoning-summary parts with a blank line (only
                // when a summary is in progress), matching the shared responses decoder.
                if !self.current_thinking.is_empty() {
                    self.current_thinking.push_str("\n\n");
                    self.events.push(Event::ThinkingDelta { delta: "\n\n".to_string() });
                }
            }
            "response.function_call_arguments.delta" => {
                if let Some(delta) = data.get("delta").and_then(|v| v.as_str()) {
                    self.current_tool_args.push_str(delta);
                    self.events.push(Event::ToolCallDelta { delta: delta.to_string() });
                }
            }
            "response.function_call_arguments.done" => {
                if let Some(arguments) = data.get("arguments").and_then(|v| v.as_str()) {
                    if arguments.starts_with(&self.current_tool_args) {
                        let extra = &arguments[self.current_tool_args.len()..];
                        if !extra.is_empty() {
                            self.current_tool_args.push_str(extra);
                            self.events.push(Event::ToolCallDelta { delta: extra.to_string() });
                        }
                    } else {
                        self.current_tool_args = arguments.to_string();
                    }
                }
            }
            "response.output_item.done" => {
                if let Some(item) = data.get("item") {
                    match item.get("type").and_then(|v| v.as_str()) {
                        Some("function_call") => {
                            let id = item.get("call_id").and_then(|v| v.as_str()).map(|s| s.to_string()).or_else(|| self.current_tool_call_id.clone()).unwrap_or_default();
                            let name = item.get("name").and_then(|v| v.as_str()).map(|s| s.to_string()).or_else(|| self.current_tool_name.clone()).unwrap_or_default();
                            let final_args = item.get("arguments").and_then(|v| v.as_str()).unwrap_or(&self.current_tool_args);
                            let parsed: Value = crate::jsonparse::parse_streaming_json(final_args);
                            let parsed_map = match &parsed {
                                Value::Object(map) => map.clone().into_iter().collect(),
                                _ => std::collections::HashMap::new(),
                            };
                            self.partial.content.push(ContentBlock::ToolCall {
                                id: match item.get("id").and_then(|v| v.as_str()).map(|s| s.to_string()).or_else(|| self.current_tool_item_id.clone()) {
                                    Some(item_id) if !id.is_empty() => format!("{}|{}", id, item_id),
                                    _ => id.clone(),
                                },
                                name: name.clone(),
                                arguments: parsed_map,
                                thought_signature: None,
                            });
                            self.events.push(Event::ToolCallEnd { id, name, arguments: parsed });
                            self.current_tool_call_id = None;
                            self.current_tool_item_id = None;
                            self.current_tool_name = None;
                            self.current_tool_args.clear();
                        }
                        Some("reasoning") => {
                            let thinking_text = item.get("summary").and_then(|v| v.as_array())
                                .map(|parts| parts.iter().filter_map(|p| p.get("text").and_then(|v| v.as_str())).collect::<Vec<_>>().join("\n\n"))
                                .filter(|s| !s.is_empty())
                                .or_else(|| item.get("content").and_then(|v| v.as_array()).map(|parts| parts.iter().filter_map(|p| p.get("text").and_then(|v| v.as_str())).collect::<Vec<_>>().join("\n\n")).filter(|s| !s.is_empty()))
                                .unwrap_or_else(|| self.current_thinking.clone());
                            self.partial.content.push(ContentBlock::Thinking {
                                thinking: thinking_text,
                                thinking_signature: Some(item.to_string()),
                                redacted: false,
                            });
                            self.events.push(Event::ThinkingEnd);
                            self.current_thinking.clear();
                        }
                        Some("message") => {
                            // Capture the message item id/phase as a v1 text signature for
                            // correct reasoning-item pairing on replay (mirrors shared processor).
                            let text = item.get("content").and_then(|v| v.as_array())
                                .map(|parts| parts.iter().filter_map(|p| {
                                    p.get("text").and_then(|v| v.as_str())
                                        .or_else(|| p.get("refusal").and_then(|v| v.as_str()))
                                }).collect::<Vec<_>>().join(""))
                                .filter(|s| !s.is_empty())
                                .unwrap_or_else(|| self.current_text.clone());
                            let sig = item.get("id").and_then(|v| v.as_str()).map(|id| {
                                match item.get("phase").and_then(|v| v.as_str()) {
                                    Some(p) => json!({"v": 1, "id": id, "phase": p}).to_string(),
                                    None => json!({"v": 1, "id": id}).to_string(),
                                }
                            });
                            self.partial.content.push(ContentBlock::Text { text, text_signature: sig });
                            self.current_text.clear();
                        }
                        _ => {}
                    }
                }
            }
            // Codex (ChatGPT backend) emits response.done; upstream mapCodexEvents
            // normalizes response.done/response.completed/response.incomplete all into a
            // single terminal event whose response.status drives the stop reason.
            "response.completed" | "response.done" | "response.incomplete" => {
                if self.text_started {
                    self.events.push(Event::TextEnd);
                    self.text_started = false;
                }
                if let Some(response) = data.get("response") {
                    if let Some(id) = response.get("id").and_then(|v| v.as_str()) {
                        self.partial.response_id = Some(id.to_string());
                    }
                    // Upstream does not capture response.model into responseModel.
                    if let Some(usage) = response.get("usage") {
                        let cached = usage.pointer("/input_tokens_details/cached_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                        let input_total = usage.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                        let input = input_total.saturating_sub(cached);
                        let output = usage.get("output_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                        let total = usage.get("total_tokens").and_then(|v| v.as_u64()).unwrap_or((input + output + cached) as u64) as u32;
                        let mut u = Usage {
                            input, output, cache_read: cached, cache_write: 0, cache_write_1h: None, total_tokens: total, cost: Default::default(),
                        };
                        let m = 1_000_000.0;
                        u.cost = crate::types::CostBreakdown {
                            input: f64::from(u.input) * self.model_cost.input / m,
                            output: f64::from(u.output) * self.model_cost.output / m,
                            cache_read: f64::from(u.cache_read) * self.model_cost.cache_read / m,
                            cache_write: f64::from(u.cache_write) * self.model_cost.cache_write / m,
                            total: 0.0,
                        };
                        u.cost.total = u.cost.input + u.cost.output + u.cost.cache_read + u.cost.cache_write;
                        // Apply service-tier cost multiplier (flex 0.5x, priority 2x/2.5x),
                        // resolving the response's tier over the requested one (resolveCodexServiceTier:
                        // a "default" response keeps an explicitly requested flex/priority tier).
                        let request_tier = self.service_tier.as_deref();
                        let response_tier = response.get("service_tier").and_then(|v| v.as_str());
                        let tier = match (response_tier, request_tier) {
                            (Some("default"), Some(rt @ ("flex" | "priority"))) => Some(rt),
                            (Some(rt), _) => Some(rt),
                            (None, rt) => rt,
                        };
                        let multiplier = match tier {
                            Some("flex") => 0.5,
                            Some("priority") => if self.model_id == "gpt-5.5" { 2.5 } else { 2.0 },
                            _ => 1.0,
                        };
                        if multiplier != 1.0 {
                            u.cost.input *= multiplier;
                            u.cost.output *= multiplier;
                            u.cost.cache_read *= multiplier;
                            u.cost.cache_write *= multiplier;
                            u.cost.total = u.cost.input + u.cost.output + u.cost.cache_read + u.cost.cache_write;
                        }
                        self.partial.usage = Some(u);
                    }
                }
                // Map the response status, then override to toolUse only on `stop` when
                // tool calls are present (mirrors the shared processResponsesStream).
                let status = data.pointer("/response/status").and_then(|v| v.as_str()).unwrap_or("completed");
                let mut reason = match status {
                    "incomplete" => StopReason::Length,
                    "failed" | "cancelled" => StopReason::Error,
                    _ => StopReason::Stop,
                };
                if reason == StopReason::Stop
                    && self.partial.content.iter().any(|b| matches!(b, ContentBlock::ToolCall { .. })) {
                    reason = StopReason::ToolUse;
                }
                self.partial.stop_reason = Some(reason);
                if !self.current_text.is_empty() && !self.partial.content.iter().any(|b| matches!(b, ContentBlock::Text { .. })) {
                    self.partial.content.push(ContentBlock::Text { text: self.current_text.clone(), text_signature: None });
                }
                return true;
            }
            "error" => {
                // Codex error event -> `Codex error: <message || code || json>` (mirrors
                // mapCodexEvents + extractCodexEventError, incl. the nested event.error).
                let message = data.get("message").and_then(|v| v.as_str())
                    .or_else(|| data.pointer("/error/message").and_then(|v| v.as_str()));
                let code = data.get("code").and_then(|v| v.as_str())
                    .or_else(|| data.pointer("/error/code").and_then(|v| v.as_str()));
                let detail = message.map(|s| s.to_string())
                    .or_else(|| code.map(|s| s.to_string()))
                    .unwrap_or_else(|| data.to_string());
                let full = format!("Codex error: {detail}");
                self.partial.stop_reason = Some(StopReason::Error);
                self.partial.error_message = Some(full.clone());
                self.events.push(Event::Error {
                    reason: StopReason::Error,
                    error: Arc::from(Box::<dyn std::error::Error + Send + Sync>::from(full)),
                    message: Some(self.partial.clone()),
                });
                return true;
            }
            "response.failed" => {
                // mapCodexEvents: response.error.message, else "Codex response failed".
                let full = data.pointer("/response/error/message").and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "Codex response failed".to_string());
                self.partial.stop_reason = Some(StopReason::Error);
                self.partial.error_message = Some(full.clone());
                self.events.push(Event::Error {
                    reason: StopReason::Error,
                    error: Arc::from(Box::<dyn std::error::Error + Send + Sync>::from(full)),
                    message: Some(self.partial.clone()),
                });
                return true;
            }
            _ => {}
        }
        false
    }

    fn finish(mut self) -> Vec<Event> {
        // If an error event was already emitted, do not also emit Done.
        if matches!(self.partial.stop_reason, Some(StopReason::Error)) {
            return self.events;
        }
        let reason = self.partial.stop_reason.clone().unwrap_or(StopReason::Stop);
        self.events.push(Event::Done { reason, message: self.partial.clone() });
        self.events
    }
}

/// Parse a Codex error response body, extracting a friendly usage-limit message
/// for rate/usage-limit errors (mirrors parseErrorResponse: surfaces
/// friendlyMessage || err.message || raw).
pub(crate) fn parse_codex_error_response(body: &str, status: u16) -> String {
    let mut message = if body.is_empty() {
        // Upstream: raw || response.statusText || "Request failed". With an empty body,
        // fall back to the HTTP status reason phrase before the generic default.
        reqwest::StatusCode::from_u16(status).ok()
            .and_then(|s| s.canonical_reason())
            .unwrap_or("Request failed")
            .to_string()
    } else {
        body.to_string()
    };
    let mut friendly: Option<String> = None;
    if let Ok(parsed) = serde_json::from_str::<Value>(body)
        && let Some(err) = parsed.get("error") {
        let code = err.get("code").and_then(|v| v.as_str())
            .or_else(|| err.get("type").and_then(|v| v.as_str()))
            .unwrap_or("")
            .to_lowercase();
        let is_usage_limit = code.contains("usage_limit_reached")
            || code.contains("usage_not_included")
            || code.contains("rate_limit_exceeded")
            || status == 429;
        if is_usage_limit {
            let plan = err.get("plan_type").and_then(|v| v.as_str())
                .map(|p| format!(" ({} plan)", p.to_lowercase()))
                .unwrap_or_default();
            let when = err.get("resets_at").and_then(|v| v.as_f64())
                .map(|r| {
                    let mins = (((r * 1000.0 - crate::utils::now_millis() as f64) / 60000.0).round()).max(0.0) as i64;
                    format!(" Try again in ~{mins} min.")
                })
                .unwrap_or_default();
            friendly = Some(format!("You have hit your ChatGPT usage limit{plan}.{when}").trim().to_string());
        }
        message = err.get("message").and_then(|v| v.as_str()).map(|s| s.to_string())
            .or_else(|| friendly.clone())
            .unwrap_or(message);
    }
    friendly.unwrap_or(message)
}

pub(crate) fn build_codex_payload(model: &Model, context: &Context, opts: &StreamOptions) -> Value {
    // Reuse the Responses input/tool conversion, then restructure for Codex:
    // the system prompt moves to `instructions` and is removed from `input`.
    let base = responses::build_responses_payload(model, context, opts);
    let mut input = base.get("input").cloned().unwrap_or_else(|| json!([]));
    if let Some(arr) = input.as_array_mut() {
        arr.retain(|m| {
            !matches!(m.get("role").and_then(|r| r.as_str()), Some("system") | Some("developer"))
        });
    }

    let instructions = context.system_prompt.clone().filter(|p| !p.is_empty()).unwrap_or_else(|| "You are a helpful assistant.".to_string());
    let mut body = json!({
        "model": model.id,
        "store": false,
        "stream": true,
        "instructions": instructions,
        "input": input,
        "text": { "verbosity": opts.text_verbosity.clone().filter(|s| !s.is_empty()).unwrap_or_else(|| "low".to_string()) },
        "include": ["reasoning.encrypted_content"],
        "tool_choice": "auto",
        "parallel_tool_calls": true,
    });

    if let Some(ref session_id) = opts.session_id {
        body["prompt_cache_key"] = json!(crate::prompt_cache::clamp_openai_prompt_cache_key(session_id));
    }
    if let Some(temp) = opts.temperature {
        body["temperature"] = json!(temp);
    }
    if let Some(ref service_tier) = opts.service_tier {
        body["service_tier"] = json!(service_tier);
    }
    if !context.tools.is_empty()
        && let Some(tools) = base.get("tools") {
            // Codex uses strict: null (not false) on tool definitions.
            let mut tools = tools.clone();
            if let Some(arr) = tools.as_array_mut() {
                for t in arr.iter_mut() {
                    t["strict"] = Value::Null;
                }
            }
            body["tools"] = tools;
    }
    if let Some(level) = opts.reasoning.as_ref().and_then(|l| crate::simple_options::clamp_reasoning_for_model(model, l)) {
        let key = format!("{:?}", level).to_lowercase();
        let effort = model.thinking_level_map.as_ref()
            .and_then(|m| m.get(&key))
            .and_then(|v| v.clone())
            .unwrap_or(key);
        body["reasoning"] = json!({
            "effort": effort,
            "summary": opts.reasoning_summary.clone().unwrap_or_else(|| "auto".to_string()),
        });
    }
    body
}

#[cfg(test)]
pub(crate) fn replay_codex_ws_events(model: &Model, events: &[Value]) -> Vec<Event> {
    replay_codex_ws_events_with_tier(model, events, None)
}

#[cfg(test)]
pub(crate) fn replay_codex_ws_events_with_tier(model: &Model, events: &[Value], service_tier: Option<&str>) -> Vec<Event> {
    let mut state = CodexWsState::new(model);
    state.service_tier = service_tier.map(|s| s.to_string());
    for event in events {
        if state.process_event(event) {
            break;
        }
    }
    state.finish()
}

#[cfg(test)]
mod ua_tests {
    use super::codex_user_agent;

    #[test]
    fn test_codex_user_agent_uses_node_naming() {
        let ua = codex_user_agent();
        assert!(ua.starts_with("pi (") && ua.ends_with(')'), "{ua}");
        // std names must be mapped to Node's naming, never leaked verbatim.
        assert!(!ua.contains("macos"), "{ua}");
        assert!(!ua.contains("x86_64"), "{ua}");
        assert!(!ua.contains("aarch64"), "{ua}");
        assert!(!ua.contains("windows"), "{ua}");
    }
}
