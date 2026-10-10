use crate::durable::types::{DurableError, MAX_ENTRY_BYTES, MAX_TASK_FIELD_BYTES};
use crate::events::Event;
use crate::types::{ContentBlock, Context, Message, Model, Role, StopReason, StreamOptions, Tool};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::future::Future;
use std::io::Write;
use std::pin::Pin;
use tokio_stream::StreamExt;

pub type ModelFuture<'a> = Pin<Box<dyn Future<Output = ModelRun> + Send + 'a>>;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PinnedModel {
    pub behavior: Model,
}

impl PinnedModel {
    pub fn from_model(model: &Model) -> Result<Self, DurableError> {
        let mut behavior = model.clone();
        behavior.base_url.clear();
        behavior.headers = None;
        behavior.api_key = None;
        let pinned = Self { behavior };
        pinned.validate()?;
        Ok(pinned)
    }

    pub fn provider(&self) -> &str {
        &self.behavior.provider
    }
    pub fn id(&self) -> &str {
        &self.behavior.id
    }
    pub fn api(&self) -> &str {
        &self.behavior.api
    }

    pub fn validate(&self) -> Result<(), DurableError> {
        for (field, value) in [
            ("model provider", self.provider()),
            ("model id", self.id()),
            ("model api", self.api()),
            ("model name", self.behavior.name.as_str()),
        ] {
            if value.is_empty() || value.len() > 256 {
                return Err(DurableError::Rejected(format!("invalid {field}")));
            }
        }
        if self.behavior.context_window == 0 || self.behavior.max_tokens == 0 {
            return Err(DurableError::Rejected("invalid model limits".into()));
        }
        if !self.behavior.base_url.is_empty()
            || self.behavior.headers.is_some()
            || self.behavior.api_key.is_some()
        {
            return Err(DurableError::Rejected(
                "pinned model contains process-local transport data".into(),
            ));
        }
        let _ = encode_limited("pinned model", self, MAX_TASK_FIELD_BYTES)?;
        Ok(())
    }

    pub(crate) fn dispatch_model(&self, current: &Model) -> Result<Model, DurableError> {
        if current.provider != self.behavior.provider
            || current.id != self.behavior.id
            || current.api != self.behavior.api
        {
            return Err(DurableError::Rejected(
                "registered model identity changed".into(),
            ));
        }
        let mut model = self.behavior.clone();
        model.base_url = current.base_url.clone();
        model.headers = current.headers.clone();
        model.api_key = current.api_key.clone();
        Ok(model)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PinnedOptions {
    pub temperature: Option<f64>,
    pub max_tokens: Option<u32>,
    pub sampling_params: Option<Value>,
    pub reasoning: Option<String>,
    pub cache_retention: Option<String>,
    pub service_tier: Option<String>,
    pub text_verbosity: Option<String>,
}

impl PinnedOptions {
    pub fn validate(&self) -> Result<(), DurableError> {
        if self.temperature.is_some_and(|value| !value.is_finite()) {
            return Err(DurableError::Rejected("invalid temperature".into()));
        }
        if self.max_tokens == Some(0) {
            return Err(DurableError::Rejected("max_tokens must be positive".into()));
        }
        let _ = encode_limited("pinned options", self, MAX_TASK_FIELD_BYTES)?;
        Ok(())
    }

    fn stream_options(&self) -> Result<StreamOptions, DurableError> {
        self.validate()?;
        let reasoning = match self.reasoning.as_deref() {
            None => None,
            Some("minimal") => Some(crate::types::ThinkingLevel::Minimal),
            Some("low") => Some(crate::types::ThinkingLevel::Low),
            Some("medium") => Some(crate::types::ThinkingLevel::Medium),
            Some("high") => Some(crate::types::ThinkingLevel::High),
            Some("xhigh") => Some(crate::types::ThinkingLevel::XHigh),
            Some("max") => Some(crate::types::ThinkingLevel::Max),
            Some(_) => return Err(DurableError::Rejected("invalid reasoning level".into())),
        };
        let cache_retention = match self.cache_retention.as_deref() {
            None => None,
            Some("none") => Some(crate::types::CacheRetention::None),
            Some("short") => Some(crate::types::CacheRetention::Short),
            Some("long") => Some(crate::types::CacheRetention::Long),
            Some(_) => return Err(DurableError::Rejected("invalid cache retention".into())),
        };
        Ok(StreamOptions {
            temperature: self.temperature,
            max_tokens: self.max_tokens,
            sampling_params: self.sampling_params.clone(),
            reasoning,
            cache_retention,
            service_tier: self.service_tier.clone(),
            text_verbosity: self.text_verbosity.clone(),
            max_retries: Some(0),
            deferred: None,
            ..Default::default()
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DurableMessage {
    pub role: String,
    pub text: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModelIntent {
    pub model: PinnedModel,
    pub options: PinnedOptions,
    /// Persisted conversation identity forwarded for provider session/cache affinity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_session_id: Option<String>,
    #[serde(default)]
    pub offered_tools: Vec<Tool>,
    pub system_prompt: Option<String>,
    pub context: Vec<DurableMessage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native_messages: Option<Vec<Message>>,
    pub context_cutoff: u32,
    pub logical_attempt: u32,
}

impl ModelIntent {
    pub fn validate(&self) -> Result<(), DurableError> {
        self.model.validate()?;
        self.options.validate()?;
        if let Some(session_id) = &self.provider_session_id {
            super::provider::validate_session_id(session_id)?;
        }
        if self.context_cutoff == 0 || self.logical_attempt == 0 {
            return Err(DurableError::Rejected(
                "invalid model intent counters".into(),
            ));
        }
        if self.context.len() > self.context_cutoff as usize {
            return Err(DurableError::Rejected(
                "context exceeds pinned cutoff".into(),
            ));
        }
        if self
            .system_prompt
            .as_ref()
            .is_some_and(|value| value.len() > MAX_ENTRY_BYTES)
        {
            return Err(DurableError::TooLarge {
                field: "system prompt",
                size: self.system_prompt.as_ref().map_or(0, String::len),
                limit: MAX_ENTRY_BYTES,
            });
        }
        for message in &self.context {
            if !matches!(message.role.as_str(), "user" | "assistant") {
                return Err(DurableError::Rejected(
                    "unsupported durable message role".into(),
                ));
            }
            if message.text.len() > MAX_ENTRY_BYTES {
                return Err(DurableError::TooLarge {
                    field: "context message",
                    size: message.text.len(),
                    limit: MAX_ENTRY_BYTES,
                });
            }
        }
        if let Some(messages) = &self.native_messages {
            let _ = encode_limited("native model context", messages, MAX_TASK_FIELD_BYTES)?;
        }
        if self.offered_tools.len() > crate::durable::tool::MAX_OFFERED_TOOLS {
            return Err(DurableError::Rejected("too many offered tools".into()));
        }
        let _ = encode_limited("model intent", self, MAX_TASK_FIELD_BYTES)?;
        Ok(())
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DurableCost {
    pub input: f64,
    pub output: f64,
    pub cache_read: f64,
    pub cache_write: f64,
    pub total: f64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DurableUsage {
    pub input: u32,
    pub output: u32,
    pub cache_read: u32,
    pub cache_write: u32,
    pub cache_write_1h: Option<u32>,
    pub reasoning: Option<u32>,
    pub total_tokens: u32,
    pub cost: DurableCost,
}

impl DurableUsage {
    fn from_usage(usage: &crate::types::Usage) -> Result<Self, DurableError> {
        let costs = [
            usage.cost.input,
            usage.cost.output,
            usage.cost.cache_read,
            usage.cost.cache_write,
            usage.cost.total,
        ];
        if costs.iter().any(|value| !value.is_finite() || *value < 0.0) {
            return Err(DurableError::Rejected("invalid terminal usage cost".into()));
        }
        let minimum = usage
            .input
            .saturating_add(usage.output)
            .saturating_add(usage.cache_read)
            .saturating_add(usage.cache_write);
        if usage.total_tokens < minimum || usage.reasoning.is_some_and(|value| value > usage.output)
        {
            return Err(DurableError::Rejected(
                "invalid terminal token usage".into(),
            ));
        }
        Ok(Self {
            input: usage.input,
            output: usage.output,
            cache_read: usage.cache_read,
            cache_write: usage.cache_write,
            cache_write_1h: usage.cache_write_1h,
            reasoning: usage.reasoning,
            total_tokens: usage.total_tokens,
            cost: DurableCost {
                input: usage.cost.input,
                output: usage.cost.output,
                cache_read: usage.cost.cache_read,
                cache_write: usage.cost.cache_write,
                total: usage.cost.total,
            },
        })
    }

    pub(crate) fn validate(&self) -> Result<(), DurableError> {
        validate_usage(self)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DurableContent {
    Text { text: String },
    Thinking { thinking: String, redacted: bool },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ModelTerminal {
    Answer {
        content: Vec<DurableContent>,
        text: String,
        usage: DurableUsage,
        response_id: Option<String>,
        stop_reason: String,
    },
    BilledError {
        code: String,
        usage: DurableUsage,
    },
    ToolCalls {
        calls: Vec<crate::durable::tool::DurableToolCall>,
        usage: DurableUsage,
        assistant: Value,
    },
    UnsupportedToolCall,
    UnsupportedDeferred,
    Malformed {
        code: String,
    },
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ModelRun {
    pub terminal: Option<ModelTerminal>,
    pub terminal_count: u8,
}

impl ModelRun {
    pub fn one(terminal: ModelTerminal) -> Self {
        Self {
            terminal: Some(terminal),
            terminal_count: 1,
        }
    }

    pub fn validate(&self) -> Result<(), DurableError> {
        if self.terminal_count != 1 || self.terminal.is_none() {
            return Ok(());
        }
        let terminal = self.terminal.as_ref().expect("checked");
        match terminal {
            ModelTerminal::Answer {
                content,
                text,
                usage,
                response_id,
                stop_reason,
            } => {
                if !matches!(stop_reason.as_str(), "stop" | "length") {
                    return Err(DurableError::Rejected("invalid answer stop reason".into()));
                }
                validate_usage(usage)?;
                if text.len() > MAX_ENTRY_BYTES
                    || response_id.as_ref().is_some_and(|value| value.len() > 1024)
                    || content.len() > 4096
                {
                    return Err(DurableError::TooLarge {
                        field: "model terminal",
                        size: text.len(),
                        limit: MAX_ENTRY_BYTES,
                    });
                }
                for block in content {
                    let size = match block {
                        DurableContent::Text { text } => text.len(),
                        DurableContent::Thinking { thinking, .. } => thinking.len(),
                    };
                    if size > MAX_ENTRY_BYTES {
                        return Err(DurableError::TooLarge {
                            field: "terminal content",
                            size,
                            limit: MAX_ENTRY_BYTES,
                        });
                    }
                }
            }
            ModelTerminal::BilledError { code, usage } => {
                validate_code(code)?;
                validate_usage(usage)?;
            }
            ModelTerminal::Malformed { code } => validate_code(code)?,
            ModelTerminal::ToolCalls {
                calls,
                usage,
                assistant,
            } => {
                if calls.is_empty() || calls.len() > crate::durable::tool::MAX_TOOL_CALLS_PER_ROUND
                {
                    return Err(DurableError::Rejected("invalid tool call count".into()));
                }
                usage.validate()?;
                let _ = encode_limited("assistant tool call", assistant, MAX_TASK_FIELD_BYTES)?;
                for call in calls {
                    let _ = encode_limited("tool call", call, MAX_TASK_FIELD_BYTES)?;
                }
            }
            ModelTerminal::UnsupportedToolCall | ModelTerminal::UnsupportedDeferred => {}
        }
        let _ = encode_limited("model terminal", terminal, MAX_TASK_FIELD_BYTES)?;
        Ok(())
    }
}

pub trait DurableModelRunner: Send + Sync + 'static {
    fn run<'a>(&'a self, intent: ModelIntent) -> ModelFuture<'a>;
}

#[derive(Default)]
pub struct RegistryModelRunner;

impl DurableModelRunner for RegistryModelRunner {
    fn run<'a>(&'a self, intent: ModelIntent) -> ModelFuture<'a> {
        Box::pin(async move {
            if intent.validate().is_err() {
                return ModelRun::one(malformed("invalid_intent"));
            }
            let Some(current) =
                crate::registry::get_model(intent.model.provider(), intent.model.id())
            else {
                return ModelRun::one(malformed("missing_model"));
            };
            let Ok(model) = intent.model.dispatch_model(&current) else {
                return ModelRun::one(malformed("model_identity_changed"));
            };
            let Ok(mut options) = intent.options.stream_options() else {
                return ModelRun::one(malformed("invalid_options"));
            };
            options.session_id = intent.provider_session_id.clone();
            let context = Context {
                system_prompt: intent.system_prompt.clone(),
                tools: intent.offered_tools.clone(),
                messages: intent
                    .native_messages
                    .clone()
                    .unwrap_or_else(|| intent.context.iter().map(to_message_for_durable).collect()),
            };
            let mut stream = crate::registry::stream(&model, &context, &options);
            let mut terminal = None;
            let mut terminal_count = 0u8;
            let mut saw_tool = false;
            while let Some(event) = stream.next().await {
                let next = match event {
                    Event::ToolCallStart { .. }
                    | Event::ToolCallDelta { .. }
                    | Event::ToolCallEnd { .. } => {
                        saw_tool = true;
                        None
                    }
                    Event::Done { reason, message } => {
                        Some(done_terminal(&intent.model, reason, message).await)
                    }
                    Event::Error { message, .. } => Some(error_terminal(message)),
                    _ => None,
                };
                if let Some(next) = next {
                    terminal_count = terminal_count.saturating_add(1);
                    if terminal.is_none() {
                        terminal = Some(next);
                    }
                }
            }
            if saw_tool && terminal_count == 0 {
                terminal = Some(ModelTerminal::UnsupportedToolCall);
                terminal_count = 1;
            }
            ModelRun {
                terminal,
                terminal_count,
            }
        })
    }
}

async fn done_terminal(model: &PinnedModel, reason: StopReason, message: Message) -> ModelTerminal {
    if message
        .provider
        .as_deref()
        .is_some_and(|value| value != model.provider())
        || message
            .model
            .as_deref()
            .is_some_and(|value| value != model.id())
        || message
            .api
            .as_deref()
            .is_some_and(|value| value != model.api())
    {
        return malformed("terminal_identity_conflict");
    }
    if reason == StopReason::Deferred || message.deferred.is_some() {
        let deferred = message
            .deferred
            .as_ref()
            .zip(crate::registry::get_model(model.provider(), model.id()));
        if let Some((handle, runtime_model)) = deferred {
            let _ =
                crate::registry::cancel_deferred(&runtime_model, handle, &StreamOptions::default())
                    .await;
        }
        return ModelTerminal::UnsupportedDeferred;
    }
    if reason == StopReason::ToolUse
        || message
            .content
            .iter()
            .any(|block| matches!(block, ContentBlock::ToolCall { .. }))
    {
        let usage = match message.usage.as_ref().map(DurableUsage::from_usage) {
            Some(Ok(usage)) => usage,
            Some(Err(_)) => return malformed("invalid_usage"),
            None => return malformed("missing_usage"),
        };
        let calls = message
            .content
            .iter()
            .filter_map(|block| match block {
                ContentBlock::ToolCall {
                    id,
                    name,
                    arguments,
                    ..
                } => Some(crate::durable::tool::DurableToolCall {
                    provider_call_id: id.clone(),
                    name: name.clone(),
                    original_arguments: Value::Object(arguments.clone().into_iter().collect()),
                }),
                _ => None,
            })
            .collect::<Vec<_>>();
        let assistant = match encode_limited("assistant tool call", &message, MAX_TASK_FIELD_BYTES)
            .and_then(|bytes| {
                serde_json::from_slice(&bytes)
                    .map_err(|error| DurableError::Rejected(error.to_string()))
            }) {
            Ok(value) => value,
            Err(_) => return malformed("answer_too_large"),
        };
        return ModelTerminal::ToolCalls {
            calls,
            usage,
            assistant,
        };
    }
    let usage = match message.usage.as_ref().map(DurableUsage::from_usage) {
        Some(Ok(usage)) => usage,
        Some(Err(_)) => return malformed("invalid_usage"),
        None => return malformed("missing_usage"),
    };
    if !matches!(reason, StopReason::Stop | StopReason::Length) {
        return ModelTerminal::BilledError {
            code: format!("terminal_{}", stop_reason(&reason)),
            usage,
        };
    }
    let mut text = String::new();
    let mut content = Vec::new();
    for block in message.content {
        match block {
            ContentBlock::Text { text: value, .. } => {
                if text.len().saturating_add(value.len()) > MAX_ENTRY_BYTES {
                    return malformed("answer_too_large");
                }
                text.push_str(&value);
                content.push(DurableContent::Text { text: value });
            }
            ContentBlock::Thinking {
                thinking, redacted, ..
            } => {
                if thinking.len() > MAX_ENTRY_BYTES {
                    return malformed("thinking_too_large");
                }
                content.push(DurableContent::Thinking {
                    thinking,
                    redacted: redacted.unwrap_or(false),
                });
            }
            ContentBlock::ToolCall { .. } => return ModelTerminal::UnsupportedToolCall,
            ContentBlock::Image { .. } => return malformed("unsupported_terminal_content"),
        }
        if content.len() > 4096 {
            return malformed("terminal_content_too_many_blocks");
        }
    }
    ModelTerminal::Answer {
        content,
        text,
        usage,
        response_id: message.response_id.filter(|value| value.len() <= 1024),
        stop_reason: stop_reason(&reason).into(),
    }
}

fn validate_usage(usage: &DurableUsage) -> Result<(), DurableError> {
    let costs = [
        usage.cost.input,
        usage.cost.output,
        usage.cost.cache_read,
        usage.cost.cache_write,
        usage.cost.total,
    ];
    if costs.iter().any(|value| !value.is_finite() || *value < 0.0)
        || usage.reasoning.is_some_and(|value| value > usage.output)
    {
        return Err(DurableError::Rejected(
            "invalid durable terminal usage".into(),
        ));
    }
    let minimum = usage
        .input
        .checked_add(usage.output)
        .and_then(|value| value.checked_add(usage.cache_read))
        .and_then(|value| value.checked_add(usage.cache_write))
        .and_then(|value| value.checked_add(usage.cache_write_1h.unwrap_or(0)))
        .ok_or_else(|| DurableError::Range("terminal usage overflow".into()))?;
    if usage.total_tokens < minimum {
        return Err(DurableError::Rejected(
            "invalid durable terminal token total".into(),
        ));
    }
    Ok(())
}

fn validate_code(code: &str) -> Result<(), DurableError> {
    const CODES: &[&str] = &[
        "model_error",
        "rate_limited",
        "invalid_error_usage",
        "invalid_usage",
        "missing_usage",
        "terminal_identity_conflict",
        "answer_too_large",
        "thinking_too_large",
        "terminal_content_too_many_blocks",
        "unsupported_terminal_content",
        "invalid_intent",
        "missing_model",
        "model_identity_changed",
        "invalid_options",
        "terminal_error",
        "terminal_aborted",
        "terminal_pending",
        "durable_tool_rejected",
        "duplicate_terminal",
        "missing_terminal",
    ];
    if !CODES.contains(&code) {
        return Err(DurableError::Rejected(
            "unknown durable terminal code".into(),
        ));
    }
    Ok(())
}

fn error_terminal(message: Option<Message>) -> ModelTerminal {
    let Some(message) = message else {
        return malformed("model_error");
    };
    match message.usage.as_ref().map(DurableUsage::from_usage) {
        Some(Ok(usage)) => ModelTerminal::BilledError {
            code: "model_error".into(),
            usage,
        },
        Some(Err(_)) => malformed("invalid_error_usage"),
        None => malformed("model_error"),
    }
}

pub(crate) fn to_message_for_durable(message: &DurableMessage) -> Message {
    let mut result = crate::types::user_message(&message.text);
    result.role = if message.role == "assistant" {
        Role::Assistant
    } else {
        Role::User
    };
    result
}

fn malformed(code: &str) -> ModelTerminal {
    ModelTerminal::Malformed { code: code.into() }
}

fn stop_reason(reason: &StopReason) -> &'static str {
    match reason {
        StopReason::Stop => "stop",
        StopReason::Length => "length",
        StopReason::Error => "error",
        StopReason::Aborted => "aborted",
        StopReason::Pending => "pending",
        StopReason::ToolUse => "tool_use",
        StopReason::Deferred => "deferred",
    }
}

struct LimitedWriter {
    bytes: Vec<u8>,
    limit: usize,
}
impl Write for LimitedWriter {
    fn write(&mut self, value: &[u8]) -> std::io::Result<usize> {
        if self.bytes.len().saturating_add(value.len()) > self.limit {
            return Err(std::io::Error::new(
                std::io::ErrorKind::FileTooLarge,
                "durable value too large",
            ));
        }
        self.bytes.extend_from_slice(value);
        Ok(value.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
fn encode_limited<T: Serialize>(
    field: &'static str,
    value: &T,
    limit: usize,
) -> Result<Vec<u8>, DurableError> {
    let mut writer = LimitedWriter {
        bytes: Vec::new(),
        limit,
    };
    serde_json::to_writer(&mut writer, value).map_err(|error| {
        if error.io_error_kind() == Some(std::io::ErrorKind::FileTooLarge) {
            DurableError::TooLarge {
                field,
                size: writer.bytes.len().saturating_add(1),
                limit,
            }
        } else {
            DurableError::Rejected(error.to_string())
        }
    })?;
    Ok(writer.bytes)
}
