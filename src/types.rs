//! Core types mirroring the upstream pi-ai type system.
//!
//! JSON-serialization compatible with the TypeScript and Go implementations.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// Wire protocol identifier.
pub type Api = String;

/// Known API wire protocols.
pub mod api {
    pub const OPENAI_COMPLETIONS: &str = "openai-completions";
    pub const OPENAI_RESPONSES: &str = "openai-responses";
    pub const AZURE_OPENAI_RESPONSES: &str = "azure-openai-responses";
    pub const OPENAI_CODEX_RESPONSES: &str = "openai-codex-responses";
    pub const ANTHROPIC_MESSAGES: &str = "anthropic-messages";
    pub const BEDROCK_CONVERSE_STREAM: &str = "bedrock-converse-stream";
    pub const GOOGLE_GENERATIVE_AI: &str = "google-generative-ai";
    pub const GOOGLE_VERTEX: &str = "google-vertex";
    pub const MISTRAL_CONVERSATIONS: &str = "mistral-conversations";
    pub const PI_MESSAGES: &str = "pi-messages";
}

/// Provider identifier.
pub type Provider = String;

/// Opaque telemetry parent context carried through request options.
pub type TelemetryContext = serde_json::Value;

/// Known providers.
pub mod provider_id {
    pub const OPENAI: &str = "openai";
    pub const ANTHROPIC: &str = "anthropic";
    pub const GOOGLE: &str = "google";
    pub const GOOGLE_VERTEX: &str = "google-vertex";
    pub const AZURE_OPENAI: &str = "azure-openai-responses";
    pub const OPENAI_CODEX: &str = "openai-codex";
    pub const RADIUS: &str = "radius";
    pub const GITHUB_COPILOT: &str = "github-copilot";
    pub const AMAZON_BEDROCK: &str = "amazon-bedrock";
    pub const MISTRAL: &str = "mistral";
    pub const XAI: &str = "xai";
    pub const GROQ: &str = "groq";
    pub const CEREBRAS: &str = "cerebras";
    pub const OPENROUTER: &str = "openrouter";
    pub const DEEPSEEK: &str = "deepseek";
    pub const ANT_LING: &str = "ant-ling";
    pub const NVIDIA: &str = "nvidia";
}

/// Thinking/reasoning level.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThinkingLevel {
    Minimal,
    Low,
    Medium,
    High,
    #[serde(rename = "xhigh")]
    XHigh,
    Max,
}

/// Extended thinking level (includes "off").
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModelThinkingLevel {
    Off,
    Minimal,
    Low,
    Medium,
    High,
    #[serde(rename = "xhigh")]
    XHigh,
    Max,
}

/// Message sender role.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Role {
    User,
    Assistant,
    ToolResult,
}

/// Why the model stopped generating.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StopReason {
    Pending,
    Stop,
    Length,
    ToolUse,
    Error,
    Aborted,
    Deferred,
}

/// Cache retention preference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CacheRetention {
    None,
    Short,
    Long,
}

/// Wire transport selection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Transport {
    Sse,
    Websocket,
    WebsocketCached,
    Auto,
}

/// Content block in a message.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ContentBlock {
    #[serde(rename = "text")]
    Text {
        text: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        text_signature: Option<String>,
    },
    #[serde(rename = "thinking")]
    Thinking {
        thinking: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        thinking_signature: Option<String>,
        #[serde(default)]
        redacted: bool,
    },
    #[serde(rename = "image")]
    Image { data: String, mime_type: String },
    #[serde(rename = "toolCall")]
    ToolCall {
        id: String,
        name: String,
        arguments: HashMap<String, serde_json::Value>,
        #[serde(skip_serializing_if = "Option::is_none")]
        thought_signature: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        namespace: Option<String>,
    },
}

/// Token cost breakdown in USD.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CostBreakdown {
    pub input: f64,
    pub output: f64,
    pub cache_read: f64,
    pub cache_write: f64,
    pub total: f64,
}

/// Token usage for a single request.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DeferredHandle {
    pub provider: String,
    pub model_id: String,
    pub api: String,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub poll_after_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DeferredRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window: Option<String>,
}

/// Token usage for a single request.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Usage {
    pub input: u32,
    pub output: u32,
    pub cache_read: u32,
    pub cache_write: u32,
    /// 1-hour cache-write tokens (Anthropic `ephemeral_1h_input_tokens`), charged at 2x base input.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_write_1h: Option<u32>,
    /// Reasoning/thinking tokens when the provider reports them. Subset of `output`
    /// (output already includes these). `None` when the provider exposes no breakdown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<u32>,
    pub total_tokens: u32,
    pub cost: CostBreakdown,
}

/// Error captured as a diagnostic without failing the overall request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticError {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stack: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<serde_json::Value>,
}

/// A diagnostic record attached to an assistant message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistantMessageDiagnostic {
    #[serde(rename = "type")]
    pub diagnostic_type: String,
    pub timestamp: i64,
    pub error: DiagnosticError,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub details: Option<HashMap<String, serde_json::Value>>,
}

/// Per-million-token cost rates (base tier or a request-wide pricing tier).
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelCostRates {
    pub input: f64,
    pub output: f64,
    pub cache_read: f64,
    pub cache_write: f64,
}

/// A request-wide pricing tier. The highest matching `input_tokens_above`
/// threshold applies to the full request (v0.80.6 tiered pricing).
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelCostTier {
    /// Use this tier for requests whose total input usage exceeds this token count.
    pub input_tokens_above: u64,
    pub input: f64,
    pub output: f64,
    pub cache_read: f64,
    pub cache_write: f64,
}

/// Per-million-token costs for a model, with optional request-wide tiers.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelCost {
    pub input: f64,
    pub output: f64,
    pub cache_read: f64,
    pub cache_write: f64,
    /// Request-wide pricing tiers. The highest matching input threshold applies
    /// to the full request.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tiers: Vec<ModelCostTier>,
}

/// Deserialize `content`, treating `null`/missing as an empty vec (v0.80.5
/// lax-message-content normalization).
fn de_null_content_as_empty<'de, D>(deserializer: D) -> Result<Vec<ContentBlock>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(Option::<Vec<ContentBlock>>::deserialize(deserializer)?.unwrap_or_default())
}

/// A conversation message.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    pub role: Role,
    // v0.80.5: normalize null/missing content from untyped callers (custom tools,
    // hand-built histories, old session files) to an empty vec instead of crashing.
    #[serde(default, deserialize_with = "de_null_content_as_empty")]
    pub content: Vec<ContentBlock>,
    #[serde(default)]
    pub timestamp: i64,

    // Assistant-only fields
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api: Option<Api>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<Provider>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_model: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<AssistantMessageDiagnostic>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<Usage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<StopReason>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deferred: Option<DeferredHandle>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_stop_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_turn: Option<bool>,

    // Tool result fields
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
    #[serde(default)]
    pub is_error: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub added_tool_names: Vec<String>,
}

/// Tool definition with JSON Schema parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tool {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub constrained_sampling: Option<serde_json::Value>,
}

/// Conversation context passed to stream/complete.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Context {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
    pub messages: Vec<Message>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<Tool>,
}

/// Model definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Model {
    pub id: String,
    pub name: String,
    pub api: Api,
    pub provider: Provider,
    pub base_url: String,
    #[serde(default)]
    pub reasoning: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thinking_level_map: Option<HashMap<String, Option<String>>>,
    #[serde(default)]
    pub input: Vec<String>,
    pub cost: ModelCost,
    pub context_window: u32,
    pub max_tokens: u32,
    /// Default arbitrary sampling parameters merged into OpenAI-compatible request bodies.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sampling_params: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub headers: Option<HashMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    #[serde(default, skip_serializing_if = "ModelCompat::is_empty")]
    pub compat: ModelCompat,
}

/// Per-model compatibility flags that drive provider request behavior.
/// Mirrors upstream OpenAICompletionsCompat / OpenAIResponsesCompat / AnthropicMessagesCompat.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelCompat {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allow_empty_signature: Option<bool>,
    /// OpenRouter provider-routing preferences, sent verbatim as `provider`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub open_router_routing: Option<serde_json::Value>,
    /// Vercel AI Gateway routing (`{only?, order?}`), sent as `providerOptions.gateway`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vercel_gateway_routing: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub force_adaptive_thinking: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tokens_field: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requires_reasoning_content_on_assistant_messages: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requires_tool_result_name: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requires_thinking_as_text: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_control_format: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deferred_tools_mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requires_assistant_after_tool_result: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub send_session_affinity_headers: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub send_session_id_header: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_cache_control_on_tools: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_developer_role: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_eager_tool_input_streaming: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_long_cache_retention: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_reasoning_effort: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_store: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_usage_in_streaming: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_finish_reason: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_strict_mode: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_openai_grammar_tools: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_additional_tools: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_temperature: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_thinking_token_budget: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_tool_references: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_tool_search: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thinking_format: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zai_tool_stream: Option<bool>,
    /// chat-template thinking format kwargs (object map; mirrors compat.chatTemplateKwargs).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chat_template_kwargs: Option<serde_json::Value>,
    /// chat-template thinking format args (object map; mirrors compat.chatTemplateArgs for Baseten).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chat_template_args: Option<serde_json::Value>,
}

impl ModelCompat {
    /// True when no compat flags are set (used to skip serialization).
    pub fn is_empty(&self) -> bool {
        *self == ModelCompat::default()
    }
}

/// Stream options for a single request.
#[derive(Clone, Default)]
pub struct ThinkingBudgets {
    pub minimal: Option<u32>,
    pub low: Option<u32>,
    pub medium: Option<u32>,
    pub high: Option<u32>,
}

pub type PayloadHook = Arc<
    dyn Fn(
            serde_json::Value,
            &Model,
        ) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>>
        + Send
        + Sync,
>;
pub type ResponseHook = Arc<dyn Fn(u16, &HashMap<String, String>, &Model) + Send + Sync>;

#[derive(Clone, Default)]
pub struct StreamOptions {
    pub temperature: Option<f64>,
    /// Arbitrary sampling parameters merged last into OpenAI-compatible request bodies.
    pub sampling_params: Option<serde_json::Value>,
    pub telemetry_context: Option<TelemetryContext>,
    pub max_tokens: Option<u32>,
    pub api_key: Option<String>,
    pub transport: Option<Transport>,
    pub cache_retention: Option<CacheRetention>,
    pub session_id: Option<String>,
    pub previous_response_id: Option<String>,
    pub headers: Option<HashMap<String, String>>,
    pub max_retry_delay_ms: Option<u64>,
    pub retry_config: Option<crate::retry::RetryConfig>,
    pub metadata: Option<HashMap<String, serde_json::Value>>,
    pub timeout_ms: Option<u64>,
    pub max_retries: Option<u32>,
    /// Optional caller-owned cancellation signal for retry backoff and provider request waits.
    pub cancel: Option<tokio::sync::watch::Receiver<bool>>,
    pub wait: Option<u64>,
    pub deferred: Option<DeferredRequest>,
    pub reasoning: Option<ThinkingLevel>,
    pub reasoning_summary: Option<String>,
    pub thinking_budgets: Option<ThinkingBudgets>,
    pub tool_choice: Option<serde_json::Value>,
    pub service_tier: Option<String>,
    /// OpenAI Codex `text.verbosity` (defaults to "low" when unset).
    pub text_verbosity: Option<String>,
    /// Anthropic interleaved-thinking beta (defaults to enabled when unset).
    pub interleaved_thinking: Option<bool>,
    /// Thinking display mode for Anthropic/Bedrock Claude (defaults to "summarized").
    pub thinking_display: Option<String>,
    pub on_payload: Option<PayloadHook>,
    pub on_response: Option<ResponseHook>,
    /// Google Vertex AI project ID (overrides GOOGLE_CLOUD_PROJECT/GCLOUD_PROJECT).
    pub project: Option<String>,
    /// Google Vertex AI location (overrides GOOGLE_CLOUD_LOCATION).
    pub location: Option<String>,
    /// Provider profile (currently AWS_PROFILE override for Bedrock).
    pub profile: Option<String>,
}

impl std::fmt::Debug for StreamOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StreamOptions")
            .field("temperature", &self.temperature)
            .field("sampling_params", &self.sampling_params)
            .field("telemetry_context", &self.telemetry_context)
            .field("max_tokens", &self.max_tokens)
            .field("api_key", &self.api_key.as_ref().map(|_| "***"))
            .field("transport", &self.transport)
            .field("cache_retention", &self.cache_retention)
            .field("session_id", &self.session_id)
            .field("previous_response_id", &self.previous_response_id)
            .field("headers", &self.headers)
            .field("max_retry_delay_ms", &self.max_retry_delay_ms)
            .field("retry_config", &self.retry_config)
            .field("metadata", &self.metadata)
            .field("timeout_ms", &self.timeout_ms)
            .field("max_retries", &self.max_retries)
            .field("cancel", &self.cancel.as_ref().map(|_| "..."))
            .field("wait", &self.wait)
            .field("deferred", &self.deferred)
            .field("reasoning", &self.reasoning)
            .field("reasoning_summary", &self.reasoning_summary)
            .field("tool_choice", &self.tool_choice)
            .field("service_tier", &self.service_tier)
            .field("text_verbosity", &self.text_verbosity)
            .field("interleaved_thinking", &self.interleaved_thinking)
            .field("thinking_display", &self.thinking_display)
            .field(
                "thinking_budgets",
                &self.thinking_budgets.as_ref().map(|_| "..."),
            )
            .field("project", &self.project)
            .field("location", &self.location)
            .field("profile", &self.profile)
            .finish()
    }
}

/// Helper to create a user message.
pub fn user_message(text: &str) -> Message {
    Message {
        role: Role::User,
        content: vec![ContentBlock::Text {
            text: text.to_string(),
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
        tool_call_id: None,
        tool_name: None,
        is_error: false,
        details: None,
        added_tool_names: Vec::new(),
    }
}
