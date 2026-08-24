use crate::events::Event;
use crate::provider::mistral::stream_mistral;
use crate::provider::openai::build_payload;
use crate::provider::responses::build_responses_payload;
use crate::types::{
    ContentBlock, Context, Message, Model, ModelCompat, ModelCost, Role, StopReason, StreamOptions,
    Tool,
};
use futures::StreamExt;
use serde_json::json;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn user_context() -> Context {
    Context {
        system_prompt: None,
        messages: vec![crate::types::user_message("Hi")],
        tools: vec![],
    }
}

fn tool(name: &str) -> Tool {
    Tool {
        name: name.into(),
        description: format!("{name} tool"),
        parameters: json!({"type":"object", "properties": {"q": {"type":"string"}}}),
        constrained_sampling: None,
    }
}

fn tool_result_marker(added: &[&str]) -> Message {
    Message {
        role: Role::ToolResult,
        content: vec![ContentBlock::Text {
            text: "ok".into(),
            text_signature: None,
        }],
        timestamp: 0,
        api: None,
        provider: None,
        model: None,
        response_id: None,
        response_model: None,
        diagnostics: Vec::new(),
        usage: None,
        stop_reason: None,
        deferred: None,
        error_message: None,
        raw_stop_reason: None,
        end_turn: None,
        tool_call_id: Some("call_1".into()),
        tool_name: Some("base_tool".into()),
        is_error: false,
        details: None,
        added_tool_names: added.iter().map(|s| s.to_string()).collect(),
    }
}

fn openai_model(provider: &str, base_url: &str) -> Model {
    Model {
        id: "model".into(),
        name: "model".into(),
        api: crate::types::api::OPENAI_COMPLETIONS.into(),
        provider: provider.into(),
        base_url: base_url.into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec!["text".into()],
        cost: ModelCost::default(),
        context_window: 128000,
        max_tokens: 4096,
        sampling_params: None,
        headers: None,
        api_key: Some("test".into()),
        compat: ModelCompat::default(),
    }
}

fn mistral_model(base_url: &str) -> Model {
    let mut model = openai_model("mistral", base_url);
    model.api = crate::types::api::MISTRAL_CONVERSATIONS.into();
    model.id = "mistral-test".into();
    model
}

fn mistral_sse_event(value: serde_json::Value) -> String {
    format!("data: {value}\n\n")
}

async fn collect_mistral_terminal(
    model: &Model,
    context: &Context,
    opts: &StreamOptions,
) -> (Option<Message>, Option<String>) {
    let mut stream = stream_mistral(model, context, opts);
    let mut done = None;
    let mut error = None;
    while let Some(event) = stream.next().await {
        match event {
            Event::Done { message, .. } => done = Some(message),
            Event::Error {
                message,
                error: err,
                ..
            } => {
                error = Some(
                    message
                        .and_then(|m| m.error_message)
                        .unwrap_or_else(|| err.to_string()),
                )
            }
            _ => {}
        }
    }
    (done, error)
}

fn mistral_success_body() -> String {
    [
        mistral_sse_event(json!({"id":"resp_1","choices":[{"index":0,"finish_reason":null,"delta":{"content":"ok"}}]})),
        mistral_sse_event(json!({"choices":[{"index":0,"finish_reason":"stop","delta":{}}],"usage":{"prompt_tokens":1,"completion_tokens":1,"total_tokens":2}})),
        "data: [DONE]\n\n".to_string(),
    ]
    .join("")
}

async fn spawn_chunked_mistral_server(
    chunks: Vec<Vec<u8>>,
    first_delay: Duration,
    delay_between_chunks: Duration,
    close_signal: Option<tokio::sync::oneshot::Sender<()>>,
) -> String {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let mut close_signal = close_signal;
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut buf = [0u8; 4096];
        let _ = socket.read(&mut buf).await;
        let headers = b"HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ntransfer-encoding: chunked\r\n\r\n";
        if socket.write_all(headers).await.is_err() {
            return;
        }
        for (idx, chunk) in chunks.iter().enumerate() {
            let delay = if idx == 0 {
                first_delay
            } else {
                delay_between_chunks
            };
            if delay > Duration::ZERO {
                tokio::time::sleep(delay).await;
            }
            let prefix = format!("{:x}\r\n", chunk.len());
            if socket.write_all(prefix.as_bytes()).await.is_err()
                || socket.write_all(chunk).await.is_err()
                || socket.write_all(b"\r\n").await.is_err()
            {
                if let Some(tx) = close_signal.take() {
                    let _ = tx.send(());
                }
                return;
            }
        }
        let _ = socket.write_all(b"0\r\n\r\n").await;
        if let Some(tx) = close_signal.take() {
            let _ = tx.send(());
        }
    });
    format!("http://{addr}")
}

#[test]
fn release_pinned_catalog_counts_match_v0842() {
    let all = crate::models_generated::builtin_models();
    let pairs = all
        .iter()
        .map(|model| (model.provider.as_str(), model.id.as_str()))
        .collect::<HashSet<_>>();
    let providers = all
        .iter()
        .map(|model| model.provider.as_str())
        .collect::<HashSet<_>>();
    let apis = all
        .iter()
        .map(|model| model.api.as_str())
        .collect::<HashSet<_>>();
    assert_eq!(pairs.len(), 1312);
    assert_eq!(providers.len(), 39);
    assert_eq!(apis.len(), 9);
    assert_eq!(
        pairs.iter().filter(|(_, id)| id.contains(":batch")).count(),
        60
    );

    let image_pairs = crate::images::list_image_models(None)
        .into_iter()
        .map(|model| (model.provider, model.id))
        .collect::<HashSet<_>>();
    assert_eq!(image_pairs.len(), 45);
}

#[test]
fn strict_json_schema_tools_require_optional_properties_as_nullable() {
    let tool = Tool {
        name: "edit".into(),
        description: "edit".into(),
        parameters: json!({
            "type": "object",
            "properties": {
                "path": {"type":"string"},
                "offset": {"type":"number"},
                "metadata": {"type":"object", "properties": {"enabled": {"type":"boolean"}}},
                "nullable": {"anyOf": [{"type":"string"}, {"type":"null"}]}
            },
            "required": ["path", "metadata"],
            "additionalProperties": false
        }),
        constrained_sampling: Some(json!({"type":"json_schema","strict":"prefer"})),
    };
    let strict = crate::utils::make_strict_json_schema(&tool.parameters).unwrap();
    let required = strict["required"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect::<HashSet<_>>();
    assert_eq!(
        required,
        ["path", "offset", "metadata", "nullable"]
            .into_iter()
            .map(str::to_string)
            .collect::<HashSet<_>>()
    );
    assert_eq!(strict["additionalProperties"], json!(false));
    assert_eq!(
        strict["properties"]["offset"],
        json!({"anyOf":[{"type":"number"},{"type":"null"}]})
    );
    assert_eq!(
        strict["properties"]["nullable"],
        json!({"anyOf":[{"type":"string"},{"type":"null"}]})
    );

    let mut ctx = user_context();
    ctx.tools = vec![tool];
    let model = openai_model("openai", "https://api.openai.com/v1");
    let payload = build_payload(
        &model,
        &ctx,
        &StreamOptions::default(),
        &crate::compat::detect_compat(&model),
    );
    assert_eq!(payload["tools"][0]["function"]["parameters"], strict);
}

#[test]
fn optional_non_nullable_null_is_omitted_but_nullable_null_is_preserved() {
    let tool = Tool {
        name: "edit".into(),
        description: "edit".into(),
        parameters: json!({
            "type":"object",
            "properties": {
                "path": {"type":"string"},
                "offset": {"type":"number"},
                "nullable": {"anyOf":[{"type":"string"},{"type":"null"}]},
                "metadata": {"type":"object", "properties": {"enabled": {"type":"boolean"}}}
            },
            "required": ["path"]
        }),
        constrained_sampling: None,
    };
    let out = crate::validation::validate_tool_arguments(
        &tool,
        &json!({"path":"file.txt", "offset": null, "nullable": null, "metadata": {"enabled": null}}),
    )
    .unwrap();
    assert_eq!(
        out,
        json!({"path":"file.txt", "nullable": null, "metadata": {}})
    );
}

#[test]
fn deepseek_detection_is_case_insensitive_and_uses_max_tokens() {
    let model = openai_model("custom", "https://API.DeepSeek.com/v1");
    let compat = crate::compat::detect_compat(&model);
    assert_eq!(compat.thinking_format.as_deref(), Some("deepseek"));
    assert_eq!(compat.max_tokens_field.as_deref(), Some("max_tokens"));
    let payload = build_payload(&model, &user_context(), &StreamOptions::default(), &compat);
    assert!(payload.get("max_completion_tokens").is_none());
    assert_eq!(payload["max_tokens"], json!(4096));
}

#[test]
fn responses_additional_tools_supersedes_tool_search_for_deferred_tools() {
    let mut ctx = user_context();
    ctx.tools = vec![tool("base_tool"), tool("late_tool")];
    ctx.messages.push(tool_result_marker(&["late_tool"]));
    let model = crate::registry::get_model("openai", "gpt-5.4").unwrap();
    assert_eq!(model.compat.supports_additional_tools, Some(true));
    let payload = build_responses_payload(&model, &ctx, &StreamOptions::default());
    assert_eq!(payload["tools"][0]["function"]["name"], json!("base_tool"));
    let input = payload["input"].as_array().unwrap();
    assert!(input.iter().any(|item| item["type"] == "additional_tools"));
    assert!(
        !input
            .iter()
            .any(|item| item["type"] == "tool_search_output")
    );
    let additional = input
        .iter()
        .find(|item| item["type"] == "additional_tools")
        .unwrap();
    assert_eq!(additional["role"], json!("developer"));
    assert_eq!(
        additional["tools"][0]["function"]["name"],
        json!("late_tool")
    );
}

#[test]
fn responses_replays_namespace_only_when_additional_tools_supported() {
    let mut args = HashMap::new();
    args.insert("q".to_string(), json!("rust"));
    let assistant = Message {
        role: Role::Assistant,
        content: vec![ContentBlock::ToolCall {
            id: "call_1|fc_1".into(),
            name: "late_tool".into(),
            arguments: args,
            thought_signature: None,
            namespace: Some("dynamic_tools".into()),
        }],
        timestamp: 0,
        api: Some(crate::types::api::OPENAI_RESPONSES.into()),
        provider: Some("openai".into()),
        model: Some("gpt-5.4".into()),
        response_id: None,
        response_model: None,
        diagnostics: Vec::new(),
        usage: None,
        stop_reason: Some(StopReason::ToolUse),
        deferred: None,
        error_message: None,
        raw_stop_reason: None,
        end_turn: None,
        tool_call_id: None,
        tool_name: None,
        is_error: false,
        details: None,
        added_tool_names: Vec::new(),
    };
    let ctx = Context {
        system_prompt: None,
        messages: vec![assistant],
        tools: vec![tool("late_tool")],
    };
    let supported = crate::registry::get_model("openai", "gpt-5.4").unwrap();
    let supported_payload = build_responses_payload(&supported, &ctx, &StreamOptions::default());
    let call = supported_payload["input"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["type"] == "function_call")
        .unwrap();
    assert_eq!(call["namespace"], json!("dynamic_tools"));

    let mut unsupported = supported.clone();
    unsupported.compat.supports_additional_tools = Some(false);
    unsupported.compat.supports_tool_search = Some(false);
    let unsupported_payload =
        build_responses_payload(&unsupported, &ctx, &StreamOptions::default());
    let call = unsupported_payload["input"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["type"] == "function_call")
        .unwrap();
    assert!(call.get("namespace").is_none());
}

#[tokio::test]
async fn mistral_http_sse_parses_utf8_usage_and_raw_tool_stop() {
    let server = MockServer::start().await;
    let events = [
        json!({"id":"resp_1","choices":[{"index":0,"finish_reason":null,"delta":{"content":[{"type":"thinking","thinking":[{"type":"text","text":"reason"}]}]}}]}),
        json!({"choices":[{"index":0,"finish_reason":null,"delta":{"content":"héllo 🌍"}}]}),
        json!({"choices":[{"index":0,"finish_reason":"tool_calls","delta":{"tool_calls":[{"index":0,"id":"tool_1","function":{"name":"lookup","arguments":"{\"q\":\"rust\"}"}}]}}],"usage":{"prompt_tokens":7,"completion_tokens":4,"total_tokens":14,"prompt_tokens_details":{"cached_tokens":3}}}),
    ];
    let body = events
        .iter()
        .map(|event| format!("data: {event}\n\n"))
        .collect::<String>();
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_string(body),
        )
        .mount(&server)
        .await;
    let mut model = openai_model("mistral", &server.uri());
    model.api = crate::types::api::MISTRAL_CONVERSATIONS.into();
    model.id = "mistral-test".into();
    let context = user_context();
    let opts = StreamOptions {
        api_key: Some("secret".into()),
        ..Default::default()
    };
    let mut stream = stream_mistral(&model, &context, &opts);
    let mut done = None;
    let mut error = None;
    while let Some(event) = stream.next().await {
        match event {
            Event::Done { message, .. } => done = Some(message),
            Event::Error {
                message,
                error: err,
                ..
            } => {
                error = Some(
                    message
                        .map(|m| format!("{m:?}"))
                        .unwrap_or_else(|| err.to_string()),
                )
            }
            _ => {}
        }
    }
    assert!(error.is_none(), "unexpected error: {error:?}");
    let message = done.expect("done");
    assert_eq!(message.stop_reason, Some(StopReason::ToolUse));
    assert_eq!(message.raw_stop_reason.as_deref(), Some("tool_calls"));
    assert_eq!(message.response_id.as_deref(), Some("resp_1"));
    assert!(message.content.iter().any(
        |block| matches!(block, ContentBlock::Thinking { thinking, .. } if thinking == "reason")
    ));
    assert!(
        message
            .content
            .iter()
            .any(|block| matches!(block, ContentBlock::Text { text, .. } if text == "héllo 🌍"))
    );
    assert!(message.content.iter().any(|block| matches!(block, ContentBlock::ToolCall { id, name, arguments, .. } if id == "tool_1" && name == "lookup" && arguments["q"] == "rust")));
    let usage = message.usage.unwrap();
    assert_eq!(usage.input, 4);
    assert_eq!(usage.output, 4);
    assert_eq!(usage.cache_read, 3);
    assert_eq!(usage.total_tokens, 14);
}

#[tokio::test]
async fn mistral_http_stream_yields_delayed_chunks_incrementally() {
    let first = mistral_sse_event(
        json!({"choices":[{"index":0,"finish_reason":null,"delta":{"content":"first"}}]}),
    );
    let second = mistral_sse_event(
        json!({"choices":[{"index":0,"finish_reason":"stop","delta":{"content":" second"}}]}),
    );
    let url = spawn_chunked_mistral_server(
        vec![
            first.into_bytes(),
            second.into_bytes(),
            b"data: [DONE]\n\n".to_vec(),
        ],
        Duration::ZERO,
        Duration::from_millis(250),
        None,
    )
    .await;
    let model = mistral_model(&url);
    let opts = StreamOptions {
        api_key: Some("secret".into()),
        ..Default::default()
    };
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let handle = tokio::spawn(async move {
        let context = user_context();
        let mut stream = stream_mistral(&model, &context, &opts);
        while let Some(event) = stream.next().await {
            if let Event::TextDelta { delta } = event {
                tx.send(delta).unwrap();
            }
        }
    });

    let first = tokio::time::timeout(Duration::from_millis(500), rx.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(first, "first");
    assert!(
        tokio::time::timeout(Duration::from_millis(80), rx.recv())
            .await
            .is_err()
    );
    let second = tokio::time::timeout(Duration::from_millis(300), rx.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(second, " second");
    handle.await.unwrap();
}

#[tokio::test]
async fn mistral_http_stream_preserves_utf8_split_across_byte_chunks() {
    let body = [
        mistral_sse_event(
            json!({"choices":[{"index":0,"finish_reason":null,"delta":{"content":"héllo 🌍"}}]}),
        ),
        mistral_sse_event(json!({"choices":[{"index":0,"finish_reason":"stop","delta":{}}]})),
        "data: [DONE]\n\n".to_string(),
    ]
    .join("");
    let bytes = body.into_bytes();
    let globe = "🌍".as_bytes();
    let split = bytes
        .windows(globe.len())
        .position(|window| window == globe)
        .unwrap()
        + 1;
    let url = spawn_chunked_mistral_server(
        vec![bytes[..split].to_vec(), bytes[split..].to_vec()],
        Duration::ZERO,
        Duration::from_millis(5),
        None,
    )
    .await;
    let model = mistral_model(&url);
    let context = user_context();
    let opts = StreamOptions {
        api_key: Some("secret".into()),
        ..Default::default()
    };
    let (done, err) = collect_mistral_terminal(&model, &context, &opts).await;
    assert!(err.is_none(), "unexpected error: {err:?}");
    let message = done.unwrap();
    assert!(
        message
            .content
            .iter()
            .any(|block| matches!(block, ContentBlock::Text { text, .. } if text == "héllo 🌍"))
    );
}

#[tokio::test]
async fn mistral_http_stream_cancel_while_waiting_for_chunk_cleans_up() {
    let (closed_tx, closed_rx) = tokio::sync::oneshot::channel();
    let url = spawn_chunked_mistral_server(
        vec![mistral_success_body().into_bytes()],
        Duration::from_millis(250),
        Duration::ZERO,
        Some(closed_tx),
    )
    .await;
    let model = mistral_model(&url);
    let (tx, rx) = tokio::sync::watch::channel(false);
    let opts = StreamOptions {
        api_key: Some("secret".into()),
        cancel: Some(rx),
        ..Default::default()
    };
    let handle = tokio::spawn(async move {
        let context = user_context();
        collect_mistral_terminal(&model, &context, &opts).await
    });
    tokio::time::sleep(Duration::from_millis(30)).await;
    tx.send(true).unwrap();
    let (_done, err) = handle.await.unwrap();
    assert_eq!(err.as_deref(), Some("Request aborted"));
    tokio::time::timeout(Duration::from_millis(500), closed_rx)
        .await
        .expect("server observed client-side stream cleanup")
        .unwrap();
}

#[tokio::test]
async fn mistral_http_stream_timeout_while_awaiting_chunk_reports_error() {
    let url = spawn_chunked_mistral_server(
        vec![mistral_success_body().into_bytes()],
        Duration::from_millis(200),
        Duration::ZERO,
        None,
    )
    .await;
    let model = mistral_model(&url);
    let context = user_context();
    let opts = StreamOptions {
        api_key: Some("secret".into()),
        timeout_ms: Some(40),
        ..Default::default()
    };
    let (_done, err) = collect_mistral_terminal(&model, &context, &opts).await;
    let err = err.expect("timeout error");
    assert!(
        err.to_lowercase().contains("timed out") || err.to_lowercase().contains("timeout"),
        "{err}"
    );
}

#[tokio::test]
async fn mistral_http_uses_bounded_branded_error_body_for_403() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(403).set_body_string("x".repeat(4105)))
        .mount(&server)
        .await;
    let model = mistral_model(&server.uri());
    let context = user_context();
    let opts = StreamOptions {
        api_key: Some("secret".into()),
        ..Default::default()
    };
    let (_done, err) = collect_mistral_terminal(&model, &context, &opts).await;
    let err = err.expect("403 error");
    assert!(err.starts_with("Mistral API error (403): "), "{err}");
    assert!(err.contains("[truncated 105 chars]"), "{err}");
    assert!(err.len() < 4100, "error should be bounded: {}", err.len());
}

#[tokio::test]
async fn mistral_http_retries_with_replayable_json_body() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(503).set_body_string("busy"))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_string(mistral_success_body()),
        )
        .mount(&server)
        .await;
    let model = mistral_model(&server.uri());
    let context = user_context();
    let opts = StreamOptions {
        api_key: Some("secret".into()),
        retry_config: Some(crate::retry::RetryConfig {
            max_retries: 1,
            initial_delay: Duration::from_millis(1),
            max_delay: Duration::from_millis(1),
            backoff_multiplier: 1.0,
            jitter_fraction: 0.0,
            max_retry_delay_ms: 10,
        }),
        ..Default::default()
    };
    let (done, err) = collect_mistral_terminal(&model, &context, &opts).await;
    assert!(err.is_none(), "unexpected error: {err:?}");
    assert_eq!(done.unwrap().stop_reason, Some(StopReason::Stop));
    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].body, requests[1].body);
}

#[tokio::test]
async fn mistral_http_affinity_override_and_suppression_are_honored() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_string(mistral_success_body()),
        )
        .mount(&server)
        .await;
    let mut model = mistral_model(&server.uri());
    model.headers = Some(HashMap::from([(
        "x-affinity".into(),
        "model-affinity".into(),
    )]));
    let context = user_context();
    let opts = StreamOptions {
        api_key: Some("secret".into()),
        session_id: Some("session-affinity".into()),
        ..Default::default()
    };
    let (_done, err) = collect_mistral_terminal(&model, &context, &opts).await;
    assert!(err.is_none(), "unexpected error: {err:?}");

    let mut no_affinity = mistral_model(&server.uri());
    no_affinity.headers = None;
    let opts = StreamOptions {
        api_key: Some("secret".into()),
        session_id: Some("session-affinity".into()),
        cache_retention: Some(crate::types::CacheRetention::None),
        ..Default::default()
    };
    let (_done, err) = collect_mistral_terminal(&no_affinity, &context, &opts).await;
    assert!(err.is_none(), "unexpected error: {err:?}");

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 2);
    assert_eq!(
        requests[0].headers["x-affinity"].to_str().unwrap(),
        "model-affinity"
    );
    assert!(!requests[1].headers.contains_key("x-affinity"));
}

#[tokio::test]
async fn mistral_http_exact_wire_payload_matches_replay_contract() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .insert_header("x-request-id", "req_123")
                .set_body_string(mistral_success_body()),
        )
        .mount(&server)
        .await;
    let mut model = mistral_model(&server.uri());
    model.id = "mistral-large-latest".into();
    let mut context = Context {
        system_prompt: Some("You are exact.".into()),
        messages: vec![crate::types::user_message("Hi")],
        tools: vec![tool("lookup")],
    };
    context.messages.push(Message {
        role: Role::Assistant,
        content: vec![ContentBlock::Thinking {
            thinking: "plan".into(),
            thinking_signature: None,
            redacted: false,
        }],
        timestamp: 0,
        api: Some(crate::types::api::MISTRAL_CONVERSATIONS.into()),
        provider: Some("mistral".into()),
        model: Some("mistral-large-latest".into()),
        response_id: None,
        response_model: None,
        diagnostics: Vec::new(),
        usage: None,
        stop_reason: None,
        deferred: None,
        error_message: None,
        raw_stop_reason: None,
        end_turn: None,
        tool_call_id: None,
        tool_name: None,
        is_error: false,
        details: None,
        added_tool_names: Vec::new(),
    });
    let seen_response = Arc::new(Mutex::new(Vec::new()));
    let seen_response_cb = seen_response.clone();
    let opts = StreamOptions {
        api_key: Some("secret".into()),
        temperature: Some(0.2),
        max_tokens: Some(123),
        tool_choice: Some(json!("auto")),
        session_id: Some("session-1".into()),
        on_response: Some(Arc::new(move |status, headers, _model| {
            seen_response_cb
                .lock()
                .unwrap()
                .push((status, headers.get("x-request-id").cloned()));
        })),
        ..Default::default()
    };
    let (_done, err) = collect_mistral_terminal(&model, &context, &opts).await;
    assert!(err.is_none(), "unexpected error: {err:?}");

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    let request = &requests[0];
    assert_eq!(
        request.headers["authorization"].to_str().unwrap(),
        "Bearer secret"
    );
    assert_eq!(
        request.headers["accept"].to_str().unwrap(),
        "text/event-stream"
    );
    assert_eq!(request.headers["x-affinity"].to_str().unwrap(), "session-1");
    let body = request.body_json::<serde_json::Value>().unwrap();
    assert_eq!(body["model"], json!("mistral-large-latest"));
    assert_eq!(body["stream"], json!(true));
    assert_eq!(body["max_tokens"], json!(123));
    assert_eq!(body["temperature"], json!(0.2));
    assert_eq!(body["tool_choice"], json!("auto"));
    assert_eq!(body["prompt_cache_key"], json!("session-1"));
    assert_eq!(
        body["messages"][0],
        json!({"role":"system","content":"You are exact."})
    );
    assert_eq!(body["messages"][1], json!({"role":"user","content":"Hi"}));
    assert_eq!(body["messages"][2]["role"], json!("assistant"));
    let assistant_parts = body["messages"][2]["content"].as_array().unwrap();
    let thinking = assistant_parts
        .iter()
        .find(|part| part["type"] == json!("thinking"))
        .expect("thinking replay part");
    assert_eq!(thinking["thinking"][0]["text"], json!("plan"));
    assert_eq!(body["tools"][0]["function"]["name"], json!("lookup"));
    assert_eq!(
        *seen_response.lock().unwrap(),
        vec![(200, Some("req_123".to_string()))]
    );
}

#[test]
fn retry_classifier_matches_request_buffer_exhaustion_wording() {
    let message = crate::types::Message {
        role: Role::Assistant,
        content: Vec::new(),
        timestamp: 0,
        api: None,
        provider: None,
        model: None,
        response_id: None,
        response_model: None,
        diagnostics: Vec::new(),
        usage: None,
        stop_reason: Some(StopReason::Error),
        deferred: None,
        error_message: Some("Error: exceeded request buffer limit while retrying upstream".into()),
        raw_stop_reason: None,
        end_turn: None,
        tool_call_id: None,
        tool_name: None,
        is_error: false,
        details: None,
        added_tool_names: Vec::new(),
    };
    assert!(crate::retry::is_retryable_assistant_error(&message));
}

#[test]
fn pi_runtime_user_agent_includes_platform_release_and_arch() {
    let ua = crate::utils::pi_runtime_user_agent();
    assert!(ua.starts_with("pi ("), "{ua}");
    assert!(ua.contains(';'), "{ua}");
    assert!(ua.ends_with(')'), "{ua}");
}
