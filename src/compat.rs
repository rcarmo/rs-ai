//! OpenAI-compatible provider compatibility detection.

use crate::types::Model;

/// OpenAI Completions compatibility overrides.
#[derive(Debug, Clone, Default)]
pub struct OpenAICompletionsCompat {
    pub supports_store: Option<bool>,
    pub supports_developer_role: Option<bool>,
    pub supports_reasoning_effort: Option<bool>,
    pub supports_usage_in_streaming: Option<bool>,
    pub supports_temperature: Option<bool>,
    pub max_tokens_field: Option<String>,
    pub requires_tool_result_name: Option<bool>,
    pub requires_thinking_as_text: Option<bool>,
    pub requires_assistant_after_tool_result: Option<bool>,
    pub requires_reasoning_content_on_assistant_messages: Option<bool>,
    pub thinking_format: Option<String>,
    pub supports_strict_mode: Option<bool>,
    pub supports_long_cache_retention: Option<bool>,
    pub supports_session_affinity_headers: Option<bool>,
    pub zai_tool_stream: Option<bool>,
    pub cache_control_format: Option<String>,
    /// chat-template thinking format kwargs (object map; see resolve_chat_template_kwarg_value).
    pub chat_template_kwargs: Option<serde_json::Value>,
}

/// Auto-detect compatibility flags from a model's provider/URL, then overlay any
/// explicit per-model `compat` overrides (mirrors upstream `getCompat`).
pub fn detect_compat(model: &Model) -> OpenAICompletionsCompat {
    let overrides = model_compat_overrides(model);
    detect_compat_for_model(model, overrides.as_ref())
}

/// Convert a model's declared `compat` flags into completions-compat overrides.
fn model_compat_overrides(model: &Model) -> Option<OpenAICompletionsCompat> {
    let mc = &model.compat;
    if mc.is_empty() {
        return None;
    }
    Some(OpenAICompletionsCompat {
        supports_store: mc.supports_store,
        supports_developer_role: mc.supports_developer_role,
        supports_reasoning_effort: mc.supports_reasoning_effort,
        supports_usage_in_streaming: mc.supports_usage_in_streaming,
        supports_temperature: mc.supports_temperature,
        max_tokens_field: mc.max_tokens_field.clone(),
        requires_tool_result_name: mc.requires_tool_result_name,
        requires_thinking_as_text: mc.requires_thinking_as_text,
        requires_assistant_after_tool_result: mc.requires_assistant_after_tool_result,
        requires_reasoning_content_on_assistant_messages: mc
            .requires_reasoning_content_on_assistant_messages,
        thinking_format: mc.thinking_format.clone(),
        supports_strict_mode: mc.supports_strict_mode,
        supports_long_cache_retention: mc.supports_long_cache_retention,
        supports_session_affinity_headers: mc.send_session_affinity_headers,
        zai_tool_stream: mc.zai_tool_stream,
        cache_control_format: mc.cache_control_format.clone(),
        chat_template_kwargs: mc.chat_template_kwargs.clone(),
    })
}

/// Detect and merge model-specific compat overrides (mirrors Go's DetectCompatForModel).
pub fn detect_compat_for_model(
    model: &Model,
    overrides: Option<&OpenAICompletionsCompat>,
) -> OpenAICompletionsCompat {
    let mut c = detect_compat_inner(&model.provider, &model.id, &model.base_url);
    if let Some(o) = overrides {
        if o.supports_store.is_some() {
            c.supports_store = o.supports_store;
        }
        if o.supports_developer_role.is_some() {
            c.supports_developer_role = o.supports_developer_role;
        }
        if o.supports_reasoning_effort.is_some() {
            c.supports_reasoning_effort = o.supports_reasoning_effort;
        }
        if o.supports_usage_in_streaming.is_some() {
            c.supports_usage_in_streaming = o.supports_usage_in_streaming;
        }
        if o.supports_temperature.is_some() {
            c.supports_temperature = o.supports_temperature;
        }
        if o.max_tokens_field.is_some() {
            c.max_tokens_field = o.max_tokens_field.clone();
        }
        if o.requires_tool_result_name.is_some() {
            c.requires_tool_result_name = o.requires_tool_result_name;
        }
        if o.requires_thinking_as_text.is_some() {
            c.requires_thinking_as_text = o.requires_thinking_as_text;
        }
        if o.requires_assistant_after_tool_result.is_some() {
            c.requires_assistant_after_tool_result = o.requires_assistant_after_tool_result;
        }
        if o.requires_reasoning_content_on_assistant_messages.is_some() {
            c.requires_reasoning_content_on_assistant_messages =
                o.requires_reasoning_content_on_assistant_messages;
        }
        if o.thinking_format.is_some() {
            c.thinking_format = o.thinking_format.clone();
        }
        if o.supports_strict_mode.is_some() {
            c.supports_strict_mode = o.supports_strict_mode;
        }
        if o.supports_long_cache_retention.is_some() {
            c.supports_long_cache_retention = o.supports_long_cache_retention;
        }
        if o.supports_session_affinity_headers.is_some() {
            c.supports_session_affinity_headers = o.supports_session_affinity_headers;
        }
        if o.zai_tool_stream.is_some() {
            c.zai_tool_stream = o.zai_tool_stream;
        }
        if o.cache_control_format.is_some() {
            c.cache_control_format = o.cache_control_format.clone();
        }
        if o.chat_template_kwargs.is_some() {
            c.chat_template_kwargs = o.chat_template_kwargs.clone();
        }
    }
    c
}

fn detect_compat_inner(provider: &str, model_id: &str, base_url: &str) -> OpenAICompletionsCompat {
    // 0.80.2 restored runtime provider/baseUrl compat detection (mirrors detectCompat).
    // Explicit per-model compat baked into the catalog still overlays these (getCompat).
    let is_zai = provider == "zai"
        || provider == "zai-coding-cn"
        || base_url.contains("api.z.ai")
        || base_url.contains("open.bigmodel.cn");
    let is_together = provider == "together"
        || base_url.contains("api.together.ai")
        || base_url.contains("api.together.xyz");
    let is_moonshot = provider == "moonshotai"
        || provider == "moonshotai-cn"
        || base_url.contains("api.moonshot.");
    let is_openrouter = provider == "openrouter" || base_url.contains("openrouter.ai");
    let is_cloudflare_workers_ai =
        provider == "cloudflare-workers-ai" || base_url.contains("api.cloudflare.com");
    let is_cloudflare_ai_gateway =
        provider == "cloudflare-ai-gateway" || base_url.contains("gateway.ai.cloudflare.com");
    let is_nvidia = provider == "nvidia" || base_url.contains("integrate.api.nvidia.com");
    let is_ant_ling = provider == "ant-ling" || base_url.contains("api.ant-ling.com");
    let is_non_standard = is_nvidia
        || provider == "cerebras"
        || base_url.contains("cerebras.ai")
        || provider == "xai"
        || base_url.contains("api.x.ai")
        || is_together
        || base_url.contains("chutes.ai")
        || base_url.contains("deepseek.com")
        || is_zai
        || is_moonshot
        || provider == "opencode"
        || base_url.contains("opencode.ai")
        || is_cloudflare_workers_ai
        || is_cloudflare_ai_gateway
        || is_ant_ling;
    let use_max_tokens = base_url.contains("chutes.ai")
        || is_moonshot
        || is_cloudflare_ai_gateway
        || is_together
        || is_nvidia
        || is_ant_ling;
    let is_grok = provider == "xai" || base_url.contains("api.x.ai");
    let is_deepseek = provider == "deepseek" || base_url.contains("deepseek.com");
    let is_openrouter_developer_role_model =
        is_openrouter && (model_id.starts_with("anthropic/") || model_id.starts_with("openai/"));
    let cache_control_format = if provider == "openrouter" && model_id.starts_with("anthropic/") {
        Some("anthropic".to_string())
    } else {
        None
    };
    let thinking_format = if is_deepseek {
        "deepseek"
    } else if is_zai {
        "zai"
    } else if is_together {
        "together"
    } else if is_ant_ling {
        "ant-ling"
    } else if is_openrouter {
        "openrouter"
    } else {
        "openai"
    };
    OpenAICompletionsCompat {
        supports_store: Some(!is_non_standard),
        supports_developer_role: Some(
            is_openrouter_developer_role_model || (!is_non_standard && !is_openrouter),
        ),
        supports_reasoning_effort: Some(
            !is_grok
                && !is_zai
                && !is_moonshot
                && !is_together
                && !is_cloudflare_ai_gateway
                && !is_nvidia
                && !is_ant_ling,
        ),
        supports_usage_in_streaming: Some(true),
        // supports_temperature is an rs-ai extension (not in upstream compat); default on.
        supports_temperature: Some(true),
        max_tokens_field: Some(
            if use_max_tokens {
                "max_tokens"
            } else {
                "max_completion_tokens"
            }
            .to_string(),
        ),
        requires_tool_result_name: Some(false),
        requires_thinking_as_text: Some(false),
        requires_assistant_after_tool_result: Some(false),
        requires_reasoning_content_on_assistant_messages: Some(is_deepseek),
        thinking_format: Some(thinking_format.to_string()),
        zai_tool_stream: Some(false),
        supports_strict_mode: Some(
            !is_moonshot && !is_together && !is_cloudflare_ai_gateway && !is_nvidia,
        ),
        cache_control_format,
        supports_session_affinity_headers: Some(false),
        supports_long_cache_retention: Some(
            !(is_together
                || is_cloudflare_workers_ai
                || is_cloudflare_ai_gateway
                || is_nvidia
                || is_ant_ling),
        ),
        ..Default::default()
    }
}
