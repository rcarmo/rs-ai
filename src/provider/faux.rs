//! Faux (test double) provider for unit testing without network calls.

use std::sync::Arc;
use std::sync::Mutex;
use std::collections::{HashMap, VecDeque};

use crate::events::Event;
use crate::types::*;

/// Estimate token count from text length (mirrors upstream estimateTokens = ceil(len/4)).
fn estimate_tokens(text: &str) -> u32 {
    (text.chars().count() as u32).div_ceil(4)
}

fn content_to_text(content: &[ContentBlock]) -> String {
    content.iter().map(|b| match b {
        ContentBlock::Text { text, .. } => text.clone(),
        ContentBlock::Image { data, mime_type } => format!("[image:{}:{}]", mime_type, data.len()),
        ContentBlock::Thinking { thinking, .. } => thinking.clone(),
        ContentBlock::ToolCall { name, arguments, .. } => {
            format!("{}:{}", name, serde_json::to_string(arguments).unwrap_or_default())
        }
    }).collect::<Vec<_>>().join("\n")
}

fn assistant_content_to_text(content: &[ContentBlock]) -> String {
    content.iter().map(|b| match b {
        ContentBlock::Text { text, .. } => text.clone(),
        ContentBlock::Thinking { thinking, .. } => thinking.clone(),
        ContentBlock::ToolCall { name, arguments, .. } =>
            format!("{}:{}", name, serde_json::to_string(arguments).unwrap_or_default()),
        ContentBlock::Image { data, mime_type } => format!("[image:{}:{}]", mime_type, data.len()),
    }).collect::<Vec<_>>().join("\n")
}

fn message_to_text(msg: &Message) -> String {
    match msg.role {
        Role::User => content_to_text(&msg.content),
        Role::Assistant => assistant_content_to_text(&msg.content),
        Role::ToolResult => {
            let name = msg.tool_name.clone().unwrap_or_default();
            let mut parts = vec![name];
            parts.push(content_to_text(&msg.content));
            parts.join("\n")
        }
    }
}

fn serialize_context(context: &Context) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(ref sp) = context.system_prompt {
        parts.push(format!("system:{sp}"));
    }
    for m in &context.messages {
        let role = match m.role { Role::User => "user", Role::Assistant => "assistant", Role::ToolResult => "toolResult" };
        parts.push(format!("{role}:{}", message_to_text(m)));
    }
    if !context.tools.is_empty() {
        parts.push(format!("tools:{}", serde_json::to_string(&context.tools).unwrap_or_default()));
    }
    parts.join("\n\n")
}

fn common_prefix_len(a: &str, b: &str) -> usize {
    a.chars().zip(b.chars()).take_while(|(x, y)| x == y).count()
}

/// Estimate usage for a faux message, simulating prefix caching across a session
/// (mirrors upstream withUsageEstimate).
fn with_usage_estimate(
    content: &[ContentBlock],
    context: &Context,
    session_id: Option<&str>,
    cache_retention: Option<&CacheRetention>,
    prompt_cache: &Mutex<HashMap<String, String>>,
) -> Usage {
    let prompt_text = serialize_context(context);
    let prompt_tokens = estimate_tokens(&prompt_text);
    let output = estimate_tokens(&assistant_content_to_text(content));
    let mut input = prompt_tokens;
    let mut cache_read = 0u32;
    let mut cache_write = 0u32;
    let cache_enabled = !matches!(cache_retention, Some(CacheRetention::None));
    if let Some(sid) = session_id.filter(|_| cache_enabled) {
        let mut cache = prompt_cache.lock().unwrap();
        if let Some(prev) = cache.get(sid).cloned() {
            let cached_chars = common_prefix_len(&prev, &prompt_text);
            let prev_prefix: String = prev.chars().take(cached_chars).collect();
            let new_suffix: String = prompt_text.chars().skip(cached_chars).collect();
            cache_read = estimate_tokens(&prev_prefix);
            cache_write = estimate_tokens(&new_suffix);
            input = prompt_tokens.saturating_sub(cache_read);
        } else {
            cache_write = prompt_tokens;
        }
        cache.insert(sid.to_string(), prompt_text);
    }
    Usage {
        input,
        output,
        cache_read,
        cache_write,
        cache_write_1h: None,
        total_tokens: input + output + cache_read + cache_write,
        cost: CostBreakdown::default(),
    }
}

/// A faux API provider that streams queued canned responses with simulated deltas
/// (mirrors upstream registerFauxProvider).
pub struct FauxProvider {
    api: String,
    provider: String,
    responses: Mutex<VecDeque<Message>>,
    prompt_cache: Mutex<HashMap<String, String>>,
    chunk_chars: usize,
}

impl FauxProvider {
    /// Create a faux provider for the given api/provider name.
    pub fn new(api: impl Into<String>, provider: impl Into<String>) -> Arc<Self> {
        Arc::new(Self {
            api: api.into(),
            provider: provider.into(),
            responses: Mutex::new(VecDeque::new()),
            prompt_cache: Mutex::new(HashMap::new()),
            chunk_chars: 16,
        })
    }

    /// Replace the queued responses.
    pub fn set_responses(&self, responses: Vec<Message>) {
        *self.responses.lock().unwrap() = responses.into();
    }

    /// Append responses to the queue.
    pub fn append_responses(&self, responses: Vec<Message>) {
        self.responses.lock().unwrap().extend(responses);
    }

    /// Number of responses still queued.
    pub fn pending_response_count(&self) -> usize {
        self.responses.lock().unwrap().len()
    }
}

impl crate::registry::ApiProvider for FauxProvider {
    fn api(&self) -> &str { &self.api }

    fn stream<'a>(
        &self,
        model: &'a Model,
        context: &'a Context,
        opts: &'a StreamOptions,
    ) -> std::pin::Pin<Box<dyn futures::Stream<Item = Event> + Send + 'a>> {
        let api = self.api.clone();
        let provider = self.provider.clone();
        let model_id = model.id.clone();
        let step = self.responses.lock().unwrap().pop_front();
        let chunk_chars = self.chunk_chars;
        let usage = step.as_ref().map(|m| with_usage_estimate(
            &m.content, context, opts.session_id.as_deref(), opts.cache_retention.as_ref(), &self.prompt_cache,
        ));
        Box::pin(async_stream::stream! {
            let resolved = match step {
                Some(m) => m,
                None => {
                    yield Event::Error {
                        reason: StopReason::Error,
                        error: Arc::from(Box::<dyn std::error::Error + Send + Sync>::from(
                            "No more faux responses queued".to_string(),
                        )),
                        message: None,
                    };
                    return;
                }
            };
            let usage = usage.unwrap();
            let stop = resolved.stop_reason.clone().unwrap_or(StopReason::Stop);
            let mut partial = Message {
                role: Role::Assistant,
                content: Vec::new(),
                timestamp: crate::utils::now_millis(),
                api: Some(api.clone()),
                provider: Some(provider.clone()),
                model: Some(model_id.clone()),
                response_id: resolved.response_id.clone(),
                response_model: None,
                diagnostics: Vec::new(),
                usage: Some(usage.clone()),
                stop_reason: None,
                error_message: resolved.error_message.clone(),
                tool_call_id: None, tool_name: None, is_error: false, details: None,
            };
            yield Event::Start { partial: partial.clone() };

            for block in &resolved.content {
                match block {
                    ContentBlock::Thinking { thinking, thinking_signature, redacted } => {
                        yield Event::ThinkingStart;
                        for chunk in chunk_str(thinking, chunk_chars) {
                            yield Event::ThinkingDelta { delta: chunk };
                        }
                        yield Event::ThinkingEnd;
                        partial.content.push(ContentBlock::Thinking {
                            thinking: thinking.clone(),
                            thinking_signature: thinking_signature.clone(),
                            redacted: *redacted,
                        });
                    }
                    ContentBlock::Text { text, text_signature } => {
                        yield Event::TextStart;
                        for chunk in chunk_str(text, chunk_chars) {
                            yield Event::TextDelta { delta: chunk };
                        }
                        yield Event::TextEnd;
                        partial.content.push(ContentBlock::Text {
                            text: text.clone(), text_signature: text_signature.clone(),
                        });
                    }
                    ContentBlock::ToolCall { id, name, arguments, thought_signature } => {
                        yield Event::ToolCallStart { id: id.clone(), name: name.clone() };
                        let args_json = serde_json::to_string(arguments).unwrap_or_else(|_| "{}".to_string());
                        for chunk in chunk_str(&args_json, chunk_chars) {
                            yield Event::ToolCallDelta { delta: chunk };
                        }
                        yield Event::ToolCallEnd {
                            id: id.clone(), name: name.clone(),
                            arguments: serde_json::to_value(arguments).unwrap_or_else(|_| serde_json::json!({})),
                        };
                        partial.content.push(ContentBlock::ToolCall {
                            id: id.clone(), name: name.clone(),
                            arguments: arguments.clone(), thought_signature: thought_signature.clone(),
                        });
                    }
                    other => partial.content.push(other.clone()),
                }
            }

            partial.stop_reason = Some(stop.clone());
            if matches!(stop, StopReason::Error | StopReason::Aborted) {
                yield Event::Error {
                    reason: stop,
                    error: Arc::from(Box::<dyn std::error::Error + Send + Sync>::from(
                        partial.error_message.clone().unwrap_or_else(|| "Request was aborted".to_string()),
                    )),
                    message: Some(partial),
                };
            } else {
                yield Event::Done { reason: stop, message: partial };
            }
        })
    }
}

fn chunk_str(text: &str, chunk_chars: usize) -> Vec<String> {
    if text.is_empty() {
        return Vec::new();
    }
    let chars: Vec<char> = text.chars().collect();
    chars.chunks(chunk_chars.max(1)).map(|c| c.iter().collect()).collect()
}

/// Create a faux stream that emits a single text response.
pub fn stream_faux_text<'a>(
    text: &'a str,
    model: &'a Model,
) -> std::pin::Pin<Box<dyn futures::Stream<Item = Event> + Send + 'a>> {
    let text = text.to_string();
    let model_clone = model.clone();
    Box::pin(async_stream::stream! {
        let partial = Message {
            role: Role::Assistant,
            content: Vec::new(),
            timestamp: 0,
            api: Some(model_clone.api.clone()),
            provider: Some(model_clone.provider.clone()),
            model: Some(model_clone.id.clone()),
            response_id: Some("faux-response".into()),
            response_model: None,
            diagnostics: Vec::new(),
            usage: Some(Usage {
                input: 10,
                output: text.len() as u32 / 4,
                total_tokens: 10 + text.len() as u32 / 4,
                ..Default::default()
            }),
            stop_reason: None,
            error_message: None,
            tool_call_id: None,
            tool_name: None,
            is_error: false,
            details: None,
        };
        yield Event::Start { partial: partial.clone() };
        yield Event::TextStart;

        // Emit in chunks
        for chunk in text.as_bytes().chunks(20) {
            let s = String::from_utf8_lossy(chunk).to_string();
            yield Event::TextDelta { delta: s };
        }

        yield Event::TextEnd;

        let final_msg = Message {
            content: vec![ContentBlock::Text { text: text.clone(), text_signature: None }],
            stop_reason: Some(StopReason::Stop),
            usage: partial.usage.clone(),
            ..partial
        };
        yield Event::Done { reason: StopReason::Stop, message: final_msg };
    })
}

/// Create a faux stream that immediately errors.
pub fn stream_faux_error<'a>(
    error_msg: &'a str,
) -> std::pin::Pin<Box<dyn futures::Stream<Item = Event> + Send + 'a>> {
    let msg = error_msg.to_string();
    Box::pin(async_stream::stream! {
        yield Event::Error {
            reason: StopReason::Error,
            error: Arc::from(Box::<dyn std::error::Error + Send + Sync>::from(msg)),
            message: None,
        };
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio_stream::StreamExt;

    fn user_msg(text: &str) -> Message {
        Message {
            role: Role::User,
            content: vec![ContentBlock::Text { text: text.into(), text_signature: None }],
            timestamp: 0, api: None, provider: None, model: None, response_id: None,
            response_model: None, diagnostics: Vec::new(), usage: None, stop_reason: None,
            error_message: None, tool_call_id: None, tool_name: None, is_error: false, details: None,
        }
    }

    fn faux_model() -> Model {
        Model {
            id: "faux-model".into(),
            name: "Faux".into(),
            api: "faux".into(),
            provider: "faux".into(),
            base_url: "".into(),
            reasoning: false,
            thinking_level_map: None,
            input: vec!["text".into()],
            cost: ModelCost::default(),
            context_window: 128000,
            max_tokens: 4096,
            headers: None,
            api_key: None,
            compat: Default::default(),
        }
    }

    #[tokio::test]
    async fn test_faux_text_stream() {
        let model = faux_model();
        let mut stream = stream_faux_text("Hello, world!", &model);
        let mut events = Vec::new();
        while let Some(evt) = stream.next().await {
            events.push(evt);
        }
        // Start, TextStart, TextDelta, TextEnd, Done
        assert!(events.len() >= 4);
        assert!(matches!(&events[0], Event::Start { .. }));
        assert!(matches!(&events[1], Event::TextStart));
        assert!(matches!(events.last().unwrap(), Event::Done { .. }));
    }

    #[tokio::test]
    async fn test_faux_error_stream() {
        let mut stream = stream_faux_error("test failure");
        let evt = stream.next().await.unwrap();
        assert!(matches!(evt, Event::Error { .. }));
    }

    #[tokio::test]
    async fn test_faux_provider_queued_response_and_deltas() {
        use crate::registry::ApiProvider;
        let faux = FauxProvider::new("faux", "faux");
        faux.set_responses(vec![Message {
            role: Role::Assistant,
            content: vec![
                ContentBlock::Thinking { thinking: "pondering hard".into(), thinking_signature: None, redacted: false },
                ContentBlock::Text { text: "the answer is 42".into(), text_signature: None },
                ContentBlock::ToolCall { id: "t1".into(), name: "calc".into(),
                    arguments: std::collections::HashMap::from([("x".to_string(), serde_json::json!(1))]),
                    thought_signature: None },
            ],
            timestamp: 0, api: None, provider: None, model: None, response_id: Some("r1".into()),
            response_model: None, diagnostics: Vec::new(), usage: None,
            stop_reason: Some(StopReason::ToolUse), error_message: None,
            tool_call_id: None, tool_name: None, is_error: false, details: None,
        }]);
        assert_eq!(faux.pending_response_count(), 1);
        let model = faux_model();
        let ctx = Context { system_prompt: Some("sys".into()), messages: vec![user_msg("hi there")], tools: vec![] };
        let opts = StreamOptions::default();
        let mut stream = faux.stream(&model, &ctx, &opts);
        let mut thinking = String::new();
        let mut text = String::new();
        let mut done: Option<Message> = None;
        let mut saw_tool = false;
        while let Some(evt) = stream.next().await {
            match evt {
                Event::ThinkingDelta { delta } => thinking.push_str(&delta),
                Event::TextDelta { delta } => text.push_str(&delta),
                Event::ToolCallStart { name, .. } => { assert_eq!(name, "calc"); saw_tool = true; }
                Event::Done { message, reason } => { assert_eq!(reason, StopReason::ToolUse); done = Some(message); }
                _ => {}
            }
        }
        assert_eq!(thinking, "pondering hard");
        assert_eq!(text, "the answer is 42");
        assert!(saw_tool);
        let msg = done.expect("done");
        assert_eq!(msg.response_id.as_deref(), Some("r1"));
        assert!(msg.usage.as_ref().unwrap().output > 0);
        assert_eq!(faux.pending_response_count(), 0);
    }

    #[tokio::test]
    async fn test_faux_provider_session_cache_estimate() {
        use crate::registry::ApiProvider;
        let faux = FauxProvider::new("faux", "faux");
        let resp = || Message {
            role: Role::Assistant,
            content: vec![ContentBlock::Text { text: "ok".into(), text_signature: None }],
            timestamp: 0, api: None, provider: None, model: None, response_id: None,
            response_model: None, diagnostics: Vec::new(), usage: None,
            stop_reason: Some(StopReason::Stop), error_message: None,
            tool_call_id: None, tool_name: None, is_error: false, details: None,
        };
        faux.set_responses(vec![resp(), resp()]);
        let model = faux_model();
        let opts = StreamOptions { session_id: Some("s1".into()), ..Default::default() };
        let ctx = Context { system_prompt: None, messages: vec![user_msg("the quick brown fox jumps")], tools: vec![] };
        // First call: full prompt is a cache write, no read.
        let mut s1 = faux.stream(&model, &ctx, &opts);
        let mut u1 = None;
        while let Some(e) = s1.next().await { if let Event::Done { message, .. } = e { u1 = message.usage; } }
        let u1 = u1.unwrap();
        assert!(u1.cache_write > 0);
        assert_eq!(u1.cache_read, 0);
        // Second call with the same prompt prefix: now reads from cache.
        let mut s2 = faux.stream(&model, &ctx, &opts);
        let mut u2 = None;
        while let Some(e) = s2.next().await { if let Event::Done { message, .. } = e { u2 = message.usage; } }
        let u2 = u2.unwrap();
        assert!(u2.cache_read > 0);
    }
}
