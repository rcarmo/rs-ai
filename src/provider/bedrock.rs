//! Amazon Bedrock ConverseStream provider.

use std::sync::Arc;

use aws_config::BehaviorVersion;
use aws_sdk_bedrockruntime::types::{
    AnyToolChoice, AutoToolChoice, CachePointBlock, CachePointType, CacheTtl,
    ContentBlock as BedrockContent, ConversationRole, ImageBlock, ImageFormat, ImageSource,
    Message as BedrockMessage, ReasoningContentBlock, ReasoningTextBlock, SpecificToolChoice,
    SystemContentBlock, Tool, ToolChoice, ToolConfiguration, ToolInputSchema, ToolResultBlock,
    ToolResultContentBlock, ToolResultStatus, ToolSpecification, ToolUseBlock,
};
use aws_sdk_bedrockruntime::Client as BedrockClient;
use aws_smithy_types::{Document, Number};

use crate::events::Event;
use crate::types::*;

const EMPTY_TEXT_PLACEHOLDER: &str = "<empty>";

/// Whether the Bedrock model supports prompt caching (mirrors supportsPromptCaching).
fn supports_bedrock_prompt_caching(model: &Model) -> bool {
    let candidates = bedrock_model_match_candidates(model);
    let any = |needle: &str| candidates.iter().any(|c| c.contains(needle));
    if !any("claude") {
        return std::env::var("AWS_BEDROCK_FORCE_CACHE").ok().as_deref() == Some("1");
    }
    any("-4-") || any("claude-3-7-sonnet") || any("claude-3-5-haiku")
        // v0.80.5: Claude 5 models (fable-5, sonnet-5) support prompt caching.
        || any("fable-5") || any("sonnet-5")
}

/// Build a Bedrock cache-point block with an optional 1h TTL for long retention.
fn bedrock_cache_point(long: bool) -> CachePointBlock {
    let mut b = CachePointBlock::builder().r#type(CachePointType::Default);
    if long {
        b = b.ttl(CacheTtl::OneHour);
    }
    b.build().unwrap()
}

/// True for Anthropic Claude models on Bedrock (id or name), which support the
/// reasoningContent signature field (mirrors isAnthropicClaudeModel).
fn is_anthropic_claude_model(model: &Model) -> bool {
    let id = model.id.to_lowercase();
    let name = model.name.to_lowercase();
    id.contains("anthropic.claude")
        || id.contains("anthropic/claude")
        || name.contains("anthropic.claude")
        || name.contains("anthropic/claude")
        || name.contains("claude")
}

/// Sanitize a tool-call id for Bedrock (alnum/_/- only, max 64 chars).
pub(crate) fn normalize_bedrock_tool_call_id(id: &str) -> String {
    let sanitized: String = id
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '_' || c == '-' { c } else { '_' })
        .collect();
    if sanitized.len() > 64 { sanitized[..64].to_string() } else { sanitized }
}

/// Build the Bedrock Converse message list from the conversation (transform +
/// per-role conversion + consecutive-tool-result coalescing + last-user cache
/// point). Extracted so the coalescing/cache-point behaviour is unit-testable
/// without an AWS endpoint. Returns `Err("Unknown image type: ...")` for an
/// unsupported image mime (mirrors createImageBlock's throw).
pub(crate) fn build_bedrock_messages(
    messages: &[Message],
    model: &Model,
    opts: &StreamOptions,
) -> Result<Vec<BedrockMessage>, String> {
    let supports_signature = is_anthropic_claude_model(model);
    let transformed = crate::transform::transform_messages(messages, model);
    if let Some(bad) = bedrock_unsupported_image_mime(&transformed) {
        return Err(format!("Unknown image type: {bad}"));
    }
    let mut out: Vec<BedrockMessage> = Vec::new();
    let mut i = 0;
    while i < transformed.len() {
        let msg = &transformed[i];
        match msg.role {
            Role::User => {
                let mut content: Vec<BedrockContent> = Vec::new();
                for b in &msg.content {
                    match b {
                        ContentBlock::Text { text, .. } => {
                            if let Some(tb) = non_blank_text(text) {
                                content.push(tb);
                            }
                        }
                        ContentBlock::Image { data, mime_type } => {
                            if let Some(img) = bedrock_image_block(mime_type, data) {
                                content.push(BedrockContent::Image(img));
                            }
                        }
                        _ => {}
                    }
                }
                if content.is_empty() {
                    content.push(BedrockContent::Text(EMPTY_TEXT_PLACEHOLDER.to_string()));
                }
                out.push(BedrockMessage::builder().role(ConversationRole::User).set_content(Some(content)).build().unwrap());
                i += 1;
            }
            Role::Assistant => {
                if msg.content.is_empty() { i += 1; continue; }
                let mut content: Vec<BedrockContent> = Vec::new();
                for b in &msg.content {
                    match b {
                        ContentBlock::Text { text, .. } => {
                            if let Some(tb) = non_blank_text(text) {
                                content.push(tb);
                            }
                        }
                        ContentBlock::ToolCall { id, name, arguments, .. } => {
                            let args_value = serde_json::to_value(arguments).unwrap_or_else(|_| serde_json::json!({}));
                            if let Ok(tub) = ToolUseBlock::builder()
                                .tool_use_id(normalize_bedrock_tool_call_id(id))
                                .name(name.clone())
                                .input(json_to_document(&args_value))
                                .build()
                            {
                                content.push(BedrockContent::ToolUse(tub));
                            }
                        }
                        ContentBlock::Thinking { thinking, thinking_signature, redacted } if !redacted && !thinking.trim().is_empty() => {
                            // Only Anthropic Claude models accept the reasoning signature.
                            // For Claude with a missing signature, fall back to plain text
                            // (Bedrock rejects a replayed reasoning block without a signature).
                            if supports_signature {
                                match thinking_signature.as_ref().filter(|s| !s.trim().is_empty()) {
                                    Some(sig) => {
                                        if let Ok(rt) = ReasoningTextBlock::builder().text(thinking.clone()).signature(sig.clone()).build() {
                                            content.push(BedrockContent::ReasoningContent(ReasoningContentBlock::ReasoningText(rt)));
                                        }
                                    }
                                    None => content.push(BedrockContent::Text(thinking.clone())),
                                }
                            } else if let Ok(rt) = ReasoningTextBlock::builder().text(thinking.clone()).build() {
                                content.push(BedrockContent::ReasoningContent(ReasoningContentBlock::ReasoningText(rt)));
                            }
                        }
                        _ => {}
                    }
                }
                if content.is_empty() { i += 1; continue; }
                out.push(BedrockMessage::builder().role(ConversationRole::Assistant).set_content(Some(content)).build().unwrap());
                i += 1;
            }
            Role::ToolResult => {
                // Merge consecutive tool results into a single user message.
                let mut content: Vec<BedrockContent> = Vec::new();
                while i < transformed.len() && transformed[i].role == Role::ToolResult {
                    let tr = &transformed[i];
                    let status = if tr.is_error { ToolResultStatus::Error } else { ToolResultStatus::Success };
                    if let Ok(trb) = ToolResultBlock::builder()
                        .tool_use_id(normalize_bedrock_tool_call_id(tr.tool_call_id.as_deref().unwrap_or_default()))
                        .set_content(Some(convert_tool_result_content(&tr.content)))
                        .status(status)
                        .build()
                    {
                        content.push(BedrockContent::ToolResult(trb));
                    }
                    i += 1;
                }
                out.push(BedrockMessage::builder().role(ConversationRole::User).set_content(Some(content)).build().unwrap());
            }
        }
    }

    // Prompt caching: add a cache point to the last user message for supported
    // Claude models. Retention is resolved (defaults to short caching on).
    let retention = crate::prompt_cache::resolve_cache_retention(opts.cache_retention.as_ref());
    let cache_long = matches!(retention, CacheRetention::Long);
    let cache_enabled = retention != CacheRetention::None && supports_bedrock_prompt_caching(model);
    if cache_enabled
        && let Some(last) = out.pop() {
        if last.role() == &ConversationRole::User {
            let mut content = last.content().to_vec();
            content.push(BedrockContent::CachePoint(bedrock_cache_point(cache_long)));
            out.push(BedrockMessage::builder().role(ConversationRole::User).set_content(Some(content)).build().unwrap());
        } else {
            out.push(last);
        }
    }
    Ok(out)
}

/// Build a non-blank text content block, or None when blank (mirrors createNonBlankTextBlock).
fn non_blank_text(text: &str) -> Option<BedrockContent> {
    if text.trim().is_empty() {
        None
    } else {
        Some(BedrockContent::Text(text.to_string()))
    }
}

/// Whether a caller-supplied header is a reserved SigV4/auth header that must not
/// be overridden (mirrors upstream isReservedHeader): any `x-amz-*` header, or
/// `authorization`/`host` (all case-insensitive).
pub(crate) fn is_reserved_bedrock_header(key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    lower.starts_with("x-amz-") || lower == "authorization" || lower == "host"
}

/// First image with a mime type Bedrock doesn't support (jpeg/png/gif/webp), if any.
pub(crate) fn bedrock_unsupported_image_mime(messages: &[crate::types::Message]) -> Option<String> {
    messages.iter().flat_map(|m| m.content.iter()).find_map(|b| match b {
        ContentBlock::Image { mime_type, .. } if !matches!(mime_type.as_str(),
            "image/jpeg" | "image/jpg" | "image/png" | "image/gif" | "image/webp") => Some(mime_type.clone()),
        _ => None,
    })
}

/// Build a Bedrock image block from a base64 data string.
fn bedrock_image_block(mime_type: &str, data: &str) -> Option<ImageBlock> {
    use base64::Engine;
    let format = match mime_type {
        "image/jpeg" | "image/jpg" => ImageFormat::Jpeg,
        "image/png" => ImageFormat::Png,
        "image/gif" => ImageFormat::Gif,
        "image/webp" => ImageFormat::Webp,
        _ => return None,
    };
    let bytes = base64::engine::general_purpose::STANDARD.decode(data).ok()?;
    ImageBlock::builder()
        .format(format)
        .source(ImageSource::Bytes(aws_smithy_types::Blob::new(bytes)))
        .build()
        .ok()
}

/// Convert tool-result content into Bedrock tool-result content blocks, mirroring
/// convertToolResultContent (images + non-blank text, with an empty placeholder fallback).
fn convert_tool_result_content(content: &[ContentBlock]) -> Vec<ToolResultContentBlock> {
    let mut result: Vec<ToolResultContentBlock> = Vec::new();
    for c in content {
        match c {
            ContentBlock::Image { data, mime_type } => {
                if let Some(img) = bedrock_image_block(mime_type, data) {
                    result.push(ToolResultContentBlock::Image(img));
                }
            }
            ContentBlock::Text { text, .. } if !text.trim().is_empty() => {
                result.push(ToolResultContentBlock::Text(text.clone()));
            }
            _ => {}
        }
    }
    if result.is_empty() {
        result.push(ToolResultContentBlock::Text(EMPTY_TEXT_PLACEHOLDER.to_string()));
    }
    result
}

/// Extract the AWS region from a Bedrock inference-profile ARN model id
/// (arn:partition:bedrock:REGION:...).
pub(crate) fn bedrock_arn_region(model_id: &str) -> Option<String> {
    if !model_id.starts_with("arn:") {
        return None;
    }
    let parts: Vec<&str> = model_id.split(':').collect();
    if parts.len() >= 4 && parts[2] == "bedrock" && !parts[3].is_empty() {
        Some(parts[3].to_string())
    } else {
        None
    }
}

/// Parse the region from a standard Bedrock runtime endpoint host
/// (bedrock-runtime[-fips].REGION.amazonaws.com[.cn]); None for custom URLs.
pub(crate) fn bedrock_standard_endpoint_region(base_url: &str) -> Option<String> {
    let parsed = url::Url::parse(base_url).ok()?;
    let host = parsed.host_str()?.to_lowercase();
    let rest = host.strip_prefix("bedrock-runtime")?;
    let rest = rest.strip_prefix("-fips").unwrap_or(rest);
    let rest = rest.strip_prefix('.')?;
    let region = rest.split('.').next()?;
    let suffix = &rest[region.len()..];
    if (suffix == ".amazonaws.com" || suffix == ".amazonaws.com.cn") && !region.is_empty() {
        Some(region.to_string())
    } else {
        None
    }
}

fn bedrock_configured_region() -> Option<String> {
    std::env::var("AWS_REGION").or_else(|_| std::env::var("AWS_DEFAULT_REGION")).ok().filter(|s| !s.is_empty())
}

fn bedrock_has_profile() -> bool {
    std::env::var("AWS_PROFILE").map(|v| !v.is_empty()).unwrap_or(false)
}

/// Whether to pin the model base URL as an explicit endpoint (mirrors
/// shouldUseExplicitBedrockEndpoint): always for custom URLs, and for standard
/// runtime endpoints only when no region/profile is configured.
pub(crate) fn bedrock_use_explicit_endpoint(model: &Model) -> bool {
    match bedrock_standard_endpoint_region(&model.base_url) {
        None => true,
        Some(_) => bedrock_configured_region().is_none() && !bedrock_has_profile(),
    }
}

/// Resolve the Bedrock region (mirrors the upstream priority: ARN-embedded >
/// configured env > standard-endpoint region (when explicit) > us-east-1 unless a
/// profile is configured, in which case the SDK chain resolves it).
pub(crate) fn resolve_bedrock_region(model: &Model) -> Option<String> {
    if let Some(r) = bedrock_arn_region(&model.id) {
        return Some(r);
    }
    if let Some(r) = bedrock_configured_region() {
        return Some(r);
    }
    if bedrock_use_explicit_endpoint(model)
        && let Some(r) = bedrock_standard_endpoint_region(&model.base_url) {
        return Some(r);
    }
    if !bedrock_has_profile() {
        return Some("us-east-1".to_string());
    }
    None
}

/// Whether the Bedrock target is GovCloud, where the Claude `thinking.display`
/// field must be omitted (mirrors isGovCloudBedrockTarget).
pub(crate) fn is_govcloud_bedrock_target(model: &Model) -> bool {
    if let Ok(region) = std::env::var("AWS_REGION").or_else(|_| std::env::var("AWS_DEFAULT_REGION"))
        && region.to_lowercase().starts_with("us-gov-") {
        return true;
    }
    let id = model.id.to_lowercase();
    id.starts_with("us-gov.") || id.starts_with("arn:aws-us-gov:")
}

/// Whether a Bedrock Claude model supports adaptive thinking, detected by id/name
/// pattern (mirrors supportsAdaptiveThinking) rather than a compat flag.
/// Build the lowercased + separator-collapsed model match candidates from id and
/// name (mirrors upstream getModelMatchCandidates).
fn bedrock_model_match_candidates(model: &Model) -> Vec<String> {
    let collapse = |s: &str| -> String {
        let mut out = String::new();
        let mut prev_sep = false;
        for c in s.chars() {
            if c.is_whitespace() || c == '_' || c == '.' || c == ':' {
                if !prev_sep { out.push('-'); prev_sep = true; }
            } else {
                out.push(c);
                prev_sep = false;
            }
        }
        out
    };
    let mut candidates: Vec<String> = Vec::new();
    for value in [model.id.as_str(), model.name.as_str()] {
        let lower = value.to_lowercase();
        candidates.push(collapse(&lower));
        candidates.push(lower);
    }
    candidates
}

pub(crate) fn bedrock_supports_adaptive_thinking(model: &Model) -> bool {
    bedrock_model_match_candidates(model).iter().any(|s| {
        s.contains("opus-4-6") || s.contains("opus-4-7") || s.contains("opus-4-8")
            || s.contains("sonnet-4-6") || s.contains("sonnet-5") || s.contains("fable-5")
    })
}

/// Claude models that accept a native `xhigh` effort value (mirrors
/// supportsNativeXhighEffort).
fn bedrock_supports_native_xhigh_effort(model: &Model) -> bool {
    bedrock_model_match_candidates(model).iter().any(|s| {
        s.contains("opus-4-7") || s.contains("opus-4-8") || s.contains("fable-5")
    })
}

/// Resolve the Bedrock inference-config `max_tokens` (mirrors stream's inferenceConfig):
/// thinking-adjusted cap, else caller cap, else the model cap for Claude; clamped to the
/// context window via `clampMaxTokensToContext`. `thinking_max` is the optional thinking-
/// adjusted cap from `bedrock_thinking_fields`.
pub(crate) fn bedrock_inference_max_tokens(
    model: &Model,
    context: &Context,
    opts: &StreamOptions,
    thinking_max: Option<u32>,
) -> Option<u32> {
    thinking_max
        .or(opts.max_tokens)
        .or_else(|| {
            if is_anthropic_claude_model(model) && model.max_tokens > 0 { Some(model.max_tokens) } else { None }
        })
        .map(|mt| crate::simple_options::clamp_max_tokens_to_context(model, context, mt))
}

/// Build the Bedrock `additionalModelRequestFields` thinking config for Anthropic
/// Claude models (mirrors buildAdditionalModelRequestFields).
pub(crate) fn bedrock_thinking_fields(model: &Model, opts: &StreamOptions) -> Option<(serde_json::Value, Option<u32>)> {
    if !model.reasoning || !is_anthropic_claude_model(model) {
        return None;
    }
    let level = opts.reasoning.as_ref()?;
    let key = format!("{level:?}").to_lowercase();
    // GovCloud Bedrock rejects the thinking.display field, so omit it there.
    let display: Option<&str> = if is_govcloud_bedrock_target(model) {
        None
    } else {
        Some(opts.thinking_display.as_deref().unwrap_or("summarized"))
    };
    if bedrock_supports_adaptive_thinking(model) {
        // Adaptive-thinking models: effort-based config, no interleaved beta, no max adjustment.
        let default_effort = match key.as_str() {
            "minimal" | "low" => "low",
            "medium" => "medium",
            _ => "high",
        };
        // xhigh on a native-xhigh model takes precedence over the map/switch
        // (mirrors mapThinkingLevelToEffort).
        let effort = if key == "xhigh" && bedrock_supports_native_xhigh_effort(model) {
            "xhigh".to_string()
        } else {
            model.thinking_level_map.as_ref()
                .and_then(|m| m.get(&key)).and_then(|v| v.clone())
                .unwrap_or_else(|| default_effort.to_string())
        };
        let mut thinking = serde_json::json!({ "type": "adaptive" });
        if let Some(d) = display { thinking["display"] = serde_json::json!(d); }
        Some((serde_json::json!({
            "thinking": thinking,
            "output_config": { "effort": effort },
        }), None))
    } else {
        // Budget-based: select budget by level and adjust max_tokens (adjustMaxTokensForThinking).
        let mut budgets_map = std::collections::HashMap::new();
        if let Some(b) = opts.thinking_budgets.as_ref() {
            if let Some(v) = b.minimal { budgets_map.insert(ThinkingLevel::Minimal, v); }
            if let Some(v) = b.low { budgets_map.insert(ThinkingLevel::Low, v); }
            if let Some(v) = b.medium { budgets_map.insert(ThinkingLevel::Medium, v); }
            if let Some(v) = b.high { budgets_map.insert(ThinkingLevel::High, v); }
        }
        let (adj_max, budget) = crate::simple_options::adjust_max_tokens_for_thinking(
            opts.max_tokens, model.max_tokens, level, &budgets_map,
        );
        let mut fields = serde_json::json!({
            "thinking": { "type": "enabled", "budget_tokens": budget },
        });
        if let Some(d) = display { fields["thinking"]["display"] = serde_json::json!(d); }
        // Interleaved-thinking beta (budget path), gated on the interleaved_thinking option.
        if opts.interleaved_thinking != Some(false) {
            fields["anthropic_beta"] = serde_json::json!(["interleaved-thinking-2025-05-14"]);
        }
        Some((fields, Some(adj_max)))
    }
}

/// Convert a serde_json::Value into an AWS smithy Document.
fn json_to_document(v: &serde_json::Value) -> Document {
    match v {
        serde_json::Value::Null => Document::Null,
        serde_json::Value::Bool(b) => Document::Bool(*b),
        serde_json::Value::Number(n) => {
            if let Some(u) = n.as_u64() {
                Document::Number(Number::PosInt(u))
            } else if let Some(i) = n.as_i64() {
                Document::Number(Number::NegInt(i))
            } else {
                Document::Number(Number::Float(n.as_f64().unwrap_or(0.0)))
            }
        }
        serde_json::Value::String(s) => Document::String(s.clone()),
        serde_json::Value::Array(a) => Document::Array(a.iter().map(json_to_document).collect()),
        serde_json::Value::Object(o) => {
            Document::Object(o.iter().map(|(k, v)| (k.clone(), json_to_document(v))).collect())
        }
    }
}

/// Start a Bedrock ConverseStream.
pub fn stream_bedrock<'a>(
    model: &'a Model,
    context: &'a Context,
    opts: &'a StreamOptions,
) -> std::pin::Pin<Box<dyn futures::Stream<Item = Event> + Send + 'a>> {
    Box::pin(async_stream::stream! {
        let mut loader = aws_config::defaults(BehaviorVersion::latest());
        if let Some(region) = resolve_bedrock_region(model) {
            loader = loader.region(aws_config::Region::new(region));
        }
        if bedrock_use_explicit_endpoint(model) && !model.base_url.is_empty() {
            loader = loader.endpoint_url(model.base_url.clone());
        }
        let config = loader.load().await;
        let client = BedrockClient::new(&config);

        let messages = match build_bedrock_messages(&context.messages, model, opts) {
            Ok(m) => m,
            Err(msg) => {
                yield Event::Error {
                    reason: StopReason::Error,
                    error: Arc::from(Box::<dyn std::error::Error + Send + Sync>::from(msg)),
                    message: None,
                };
                return;
            }
        };
        let retention = crate::prompt_cache::resolve_cache_retention(opts.cache_retention.as_ref());
        let cache_long = matches!(retention, CacheRetention::Long);
        let cache_enabled = retention != CacheRetention::None
            && supports_bedrock_prompt_caching(model);

        let mut req = client
            .converse_stream()
            .model_id(&model.id)
            .set_messages(Some(messages));

        if let Some(prompt) = context.system_prompt.as_deref().filter(|p| !p.is_empty()) {
            req = req.system(SystemContentBlock::Text(prompt.to_string()));
            if cache_enabled {
                req = req.system(SystemContentBlock::CachePoint(bedrock_cache_point(cache_long)));
            }
        }

        // Compute thinking config (and its adjusted max output tokens for the budget path).
        let thinking = bedrock_thinking_fields(model, opts);
        // Inference config: max output tokens (adjusted for thinking when applicable, else the
        // caller cap, else the model cap for Claude) and temperature (mirrors inferenceConfig).
        let inference_max_tokens = bedrock_inference_max_tokens(
            model,
            context,
            opts,
            thinking.as_ref().and_then(|(_, m)| *m),
        );
        if inference_max_tokens.is_some() || opts.temperature.is_some() {
            let mut ic = aws_sdk_bedrockruntime::types::InferenceConfiguration::builder();
            if let Some(mt) = inference_max_tokens {
                ic = ic.max_tokens(mt as i32);
            }
            if let Some(temp) = opts.temperature {
                ic = ic.temperature(temp as f32);
            }
            req = req.inference_config(ic.build());
        }

        // Tool config: skip entirely when toolChoice is "none" (mirrors convertToolConfig).
        let tool_choice_none = opts.tool_choice.as_ref().and_then(|v| v.as_str()) == Some("none");
        if !context.tools.is_empty() && !tool_choice_none {
            let mut tool_list = Vec::new();
            for t in &context.tools {
                if let Ok(spec) = ToolSpecification::builder()
                    .name(t.name.clone())
                    .description(t.description.clone())
                    .input_schema(ToolInputSchema::Json(json_to_document(&t.parameters)))
                    .build()
                {
                    tool_list.push(Tool::ToolSpec(spec));
                }
            }
            let mut tc_builder = ToolConfiguration::builder().set_tools(Some(tool_list));
            // Map tool choice: auto/any/{type:tool,name}.
            if let Some(choice) = opts.tool_choice.as_ref() {
                if let Some(s) = choice.as_str() {
                    match s {
                        "auto" => tc_builder = tc_builder.tool_choice(ToolChoice::Auto(AutoToolChoice::builder().build())),
                        "any" => tc_builder = tc_builder.tool_choice(ToolChoice::Any(AnyToolChoice::builder().build())),
                        _ => {}
                    }
                } else if choice.get("type").and_then(|v| v.as_str()) == Some("tool")
                    && let Some(name) = choice.get("name").and_then(|v| v.as_str())
                    && let Ok(spec) = SpecificToolChoice::builder().name(name).build() {
                    tc_builder = tc_builder.tool_choice(ToolChoice::Tool(spec));
                }
            }
            if let Ok(tc) = tc_builder.build() {
                req = req.tool_config(tc);
            }
        }

        // Enable thinking for Anthropic Claude models on Bedrock (additionalModelRequestFields).
        if let Some((mut fields, _)) = thinking {
            // Re-fit the thinking budget under the context-clamped cap (mirrors upstream
            // `min(thinkingBudget, max(0, maxTokens - 1024))`).
            if let (Some(cap), Some(budget)) = (inference_max_tokens, fields["thinking"]["budget_tokens"].as_u64()) {
                let refit = (budget as u32).min(cap.saturating_sub(1024));
                fields["thinking"]["budget_tokens"] = serde_json::json!(refit);
            }
            req = req.additional_model_request_fields(json_to_document(&fields));
        }

        // Apply caller-supplied custom headers to the request (mirrors addCustomHeadersMiddleware):
        // skip reserved SigV4/auth headers (x-amz-*, authorization, host); other caller headers
        // override any existing same-named header.
        let result = if let Some(custom) = opts.headers.as_ref().filter(|h| !h.is_empty()) {
            let custom = custom.clone();
            req.customize()
                .mutate_request(move |http_req| {
                    let headers = http_req.headers_mut();
                    for (k, v) in &custom {
                        if !is_reserved_bedrock_header(k) {
                            headers.insert(k.clone(), v.clone());
                        }
                    }
                })
                .send()
                .await
        } else {
            req.send().await
        };

        let output = match result {
            Ok(o) => o,
            Err(e) => {
                yield Event::Error {
                    reason: StopReason::Error,
                    error: Arc::from(Box::<dyn std::error::Error + Send + Sync>::from(format_bedrock_sdk_error(&e))),
                    message: None,
                };
                return;
            }
        };

        // Invoke the on_response hook on a successful response, mirroring upstream's
        // onResponse({ status: $metadata.httpStatusCode, headers }). A successful
        // converse_stream is HTTP 200. (The AWS request id is not readily exposed on the
        // streaming output type, so the header map is left empty.)
        if let Some(ref hook) = opts.on_response {
            let hdrs = std::collections::HashMap::new();
            hook(200, &hdrs, model);
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

        let mut current_text = String::new();
        let mut text_started = false;
        let mut current_tool_id = String::new();
        let mut current_tool_name = String::new();
        let mut current_tool_args = String::new();
        let mut in_tool_block = false;
        let mut current_thinking = String::new();
        let mut current_thinking_signature: Option<String> = None;
        let mut thinking_started = false;

        let mut recv = output.stream;
        loop {
            match recv.recv().await {
                Ok(Some(event)) => {
                    use aws_sdk_bedrockruntime::types::ConverseStreamOutput;
                    match event {
                        ConverseStreamOutput::MessageStart(ev) => {
                            // Mirror upstream's defensive role check: a converse stream must
                            // begin an assistant message. (Start is emitted eagerly above.)
                            if ev.role() != &ConversationRole::Assistant {
                                yield Event::Error {
                                    reason: StopReason::Error,
                                    error: Arc::from(Box::<dyn std::error::Error + Send + Sync>::from(
                                        "Unexpected assistant message start but got user message start instead".to_string(),
                                    )),
                                    message: Some(partial.clone()),
                                };
                                return;
                            }
                        }
                        ConverseStreamOutput::ContentBlockStart(start) => {
                            if let Some(aws_sdk_bedrockruntime::types::ContentBlockStart::ToolUse(tu)) = start.start() {
                                in_tool_block = true;
                                current_tool_id = tu.tool_use_id().to_string();
                                current_tool_name = tu.name().to_string();
                                current_tool_args.clear();
                                yield Event::ToolCallStart { id: current_tool_id.clone(), name: current_tool_name.clone() };
                            } else if !text_started {
                                text_started = true;
                                yield Event::TextStart;
                            }
                        }
                        ConverseStreamOutput::ContentBlockDelta(delta) => {
                            if let Some(d) = delta.delta() {
                                match d {
                                    aws_sdk_bedrockruntime::types::ContentBlockDelta::Text(t) => {
                                        current_text.push_str(t);
                                        yield Event::TextDelta { delta: t.to_string() };
                                    }
                                    aws_sdk_bedrockruntime::types::ContentBlockDelta::ToolUse(tu) => {
                                        let input = tu.input();
                                        current_tool_args.push_str(input);
                                        yield Event::ToolCallDelta { delta: input.to_string() };
                                    }
                                    aws_sdk_bedrockruntime::types::ContentBlockDelta::ReasoningContent(rc) => {
                                        use aws_sdk_bedrockruntime::types::ReasoningContentBlockDelta;
                                        match rc {
                                            ReasoningContentBlockDelta::Text(t) => {
                                                if !thinking_started {
                                                    thinking_started = true;
                                                    yield Event::ThinkingStart;
                                                }
                                                current_thinking.push_str(t);
                                                yield Event::ThinkingDelta { delta: t.to_string() };
                                            }
                                            ReasoningContentBlockDelta::Signature(s) => {
                                                // Concatenate multi-chunk signatures (mirrors upstream).
                                                match &mut current_thinking_signature {
                                                    Some(existing) => existing.push_str(s),
                                                    None => current_thinking_signature = Some(s.to_string()),
                                                }
                                            }
                                            _ => {}
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                        ConverseStreamOutput::ContentBlockStop(_) => {
                            if in_tool_block {
                                in_tool_block = false;
                                let parsed = crate::jsonparse::parse_streaming_json(&current_tool_args);
                                let arguments = match &parsed {
                                    serde_json::Value::Object(map) => map.clone().into_iter().collect(),
                                    _ => std::collections::HashMap::new(),
                                };
                                partial.content.push(ContentBlock::ToolCall {
                                    id: current_tool_id.clone(),
                                    name: current_tool_name.clone(),
                                    arguments,
                                    thought_signature: None,
                                });
                                yield Event::ToolCallEnd {
                                    id: std::mem::take(&mut current_tool_id),
                                    name: std::mem::take(&mut current_tool_name),
                                    arguments: parsed,
                                };
                                current_tool_args.clear();
                            } else if thinking_started {
                                thinking_started = false;
                                partial.content.push(ContentBlock::Thinking {
                                    thinking: std::mem::take(&mut current_thinking),
                                    thinking_signature: current_thinking_signature.take(),
                                    redacted: false,
                                });
                                yield Event::ThinkingEnd;
                            } else if text_started {
                                text_started = false;
                                // Finalize this text block in order (Bedrock can interleave
                                // text/tool/text blocks); don't merge them at stream end.
                                if !current_text.is_empty() {
                                    partial.content.push(ContentBlock::Text {
                                        text: std::mem::take(&mut current_text),
                                        text_signature: None,
                                    });
                                }
                                yield Event::TextEnd;
                            }
                        }
                        ConverseStreamOutput::MessageStop(stop) => {
                            use aws_sdk_bedrockruntime::types::StopReason as BedrockStop;
                            let reason = stop.stop_reason();
                            partial.stop_reason = Some(match reason {
                                BedrockStop::EndTurn | BedrockStop::StopSequence => StopReason::Stop,
                                BedrockStop::MaxTokens | BedrockStop::ModelContextWindowExceeded => StopReason::Length,
                                BedrockStop::ToolUse => StopReason::ToolUse,
                                // content_filtered, guardrail_intervened, malformed_tool_use, etc.
                                other => {
                                    partial.error_message = Some(format!("Bedrock stop reason: {}", other));
                                    StopReason::Error
                                }
                            });
                        }
                        ConverseStreamOutput::Metadata(meta) => {
                            if let Some(u) = meta.usage() {
                                let input = u.input_tokens() as u32;
                                let output = u.output_tokens() as u32;
                                // Upstream: totalTokens || (input + output) — fall back when
                                // the API reports a zero/absent total.
                                let total = u.total_tokens() as u32;
                                partial.usage = Some(Usage {
                                    input,
                                    output,
                                    cache_read: u.cache_read_input_tokens().unwrap_or(0) as u32,
                                    cache_write: u.cache_write_input_tokens().unwrap_or(0) as u32,
                                    total_tokens: if total == 0 { input + output } else { total },
                                    ..Default::default()
                                });
                            }
                        }
                        _ => {}
                    }
                }
                Ok(None) => break,
                Err(e) => {
                    yield Event::Error {
                        reason: StopReason::Error,
                        error: Arc::from(Box::<dyn std::error::Error + Send + Sync>::from(format_bedrock_stream_error(&e))),
                        message: Some(partial.clone()),
                    };
                    return;
                }
            }
        }

        if !current_text.is_empty() {
            partial.content.push(ContentBlock::Text { text: current_text, text_signature: None });
        }
        if let Some(ref mut u) = partial.usage {
            crate::simple_options::finalize_usage(model, u);
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
            Some(reason) => {
                yield Event::Done { reason, message: partial };
            }
            None => {
                yield Event::Done { reason: StopReason::Stop, message: partial };
            }
        }
    })
}

/// AWS docs explaining how to configure a supported Bedrock data retention mode.
const BEDROCK_DATA_RETENTION_DOCS_URL: &str =
    "https://docs.aws.amazon.com/bedrock/latest/userguide/data-retention.html";

/// Append a data-retention docs hint when the error references retention mode
/// (mirrors upstream pi-ai formatBedrockError).
pub(crate) fn format_bedrock_error(message: &str) -> String {
    if message.to_lowercase().contains("data retention mode") {
        format!("{} See {} for supported data retention modes.", message, BEDROCK_DATA_RETENTION_DOCS_URL)
    } else {
        message.to_string()
    }
}

/// Format a Bedrock converse-stream SdkError, prepending a human-readable prefix for
/// known service exceptions (mirrors formatBedrockError + BEDROCK_ERROR_PREFIXES).
fn format_bedrock_sdk_error<R>(
    e: &aws_sdk_bedrockruntime::error::SdkError<
        aws_sdk_bedrockruntime::operation::converse_stream::ConverseStreamError,
        R,
    >,
) -> String {
    use aws_sdk_bedrockruntime::operation::converse_stream::ConverseStreamError as Cse;
    use aws_smithy_types::error::metadata::ProvideErrorMetadata;
    let base = format_bedrock_error(&e.to_string());
    let prefix: Option<String> = match e.as_service_error() {
        Some(Cse::InternalServerException(_)) => Some("Internal server error".to_string()),
        Some(Cse::ModelStreamErrorException(_)) => Some("Model stream error".to_string()),
        Some(Cse::ValidationException(_)) => Some("Validation error".to_string()),
        Some(Cse::ThrottlingException(_)) => Some("Throttling error".to_string()),
        Some(Cse::ServiceUnavailableException(_)) => Some("Service unavailable".to_string()),
        Some(other) => other.code().map(|c| c.to_string()),
        None => None,
    };
    match prefix {
        Some(p) => format!("{p}: {base}"),
        None => base,
    }
}

/// Format a mid-stream Bedrock error (ConverseStreamOutputError), prepending a prefix
/// for known exceptions (internalServer/modelStream/validation/throttling/serviceUnavailable).
fn format_bedrock_stream_error<R>(
    e: &aws_sdk_bedrockruntime::error::SdkError<
        aws_sdk_bedrockruntime::types::error::ConverseStreamOutputError,
        R,
    >,
) -> String {
    use aws_sdk_bedrockruntime::types::error::ConverseStreamOutputError as Cse;
    use aws_smithy_types::error::metadata::ProvideErrorMetadata;
    let base = format_bedrock_error(&e.to_string());
    let prefix: Option<String> = match e.as_service_error() {
        Some(Cse::InternalServerException(_)) => Some("Internal server error".to_string()),
        Some(Cse::ModelStreamErrorException(_)) => Some("Model stream error".to_string()),
        Some(Cse::ValidationException(_)) => Some("Validation error".to_string()),
        Some(Cse::ThrottlingException(_)) => Some("Throttling error".to_string()),
        Some(Cse::ServiceUnavailableException(_)) => Some("Service unavailable".to_string()),
        Some(other) => other.code().map(|c| c.to_string()),
        None => None,
    };
    match prefix {
        Some(p) => format!("{p}: {base}"),
        None => base,
    }
}

#[cfg(test)]
mod tests {
    use super::{format_bedrock_error, json_to_document};
    use aws_smithy_types::{Document, Number};

    #[test]
    fn test_json_to_document_roundtrip_shapes() {
        let v = serde_json::json!({"q": "rust", "n": 3, "f": 1.5, "b": true, "arr": [1, 2], "nil": null});
        let doc = json_to_document(&v);
        let obj = doc.as_object().expect("object");
        assert!(matches!(obj.get("q"), Some(Document::String(s)) if s == "rust"));
        assert!(matches!(obj.get("n"), Some(Document::Number(Number::PosInt(3)))));
        assert!(matches!(obj.get("b"), Some(Document::Bool(true))));
        assert!(matches!(obj.get("nil"), Some(Document::Null)));
        assert!(matches!(obj.get("arr"), Some(Document::Array(a)) if a.len() == 2));
    }

    #[test]
    fn test_format_bedrock_error_adds_retention_hint() {
        let msg = "data retention mode 'default' is not available for this model";
        let out = format_bedrock_error(msg);
        assert!(out.contains("data-retention.html"));
    }

    #[test]
    fn test_format_bedrock_error_passthrough() {
        assert_eq!(format_bedrock_error("some other error"), "some other error");
    }

    #[test]
    fn test_is_anthropic_claude_model() {
        use super::is_anthropic_claude_model;
        use crate::types::{Model, ModelCost};
        let mk = |id: &str, name: &str| Model {
            id: id.into(), name: name.into(), api: "bedrock-converse-stream".into(),
            provider: "bedrock".into(), base_url: String::new(), reasoning: true,
            thinking_level_map: None, input: vec!["text".into()], cost: ModelCost::default(),
            context_window: 0, max_tokens: 0, headers: None, api_key: None, compat: Default::default(),
        };
        assert!(is_anthropic_claude_model(&mk("anthropic.claude-sonnet-4", "")));
        assert!(is_anthropic_claude_model(&mk("some-profile", "Anthropic Claude Sonnet")));
        assert!(!is_anthropic_claude_model(&mk("meta.llama3", "Llama 3")));
    }

    #[test]
    fn test_convert_tool_result_content_empty_and_text() {
        use super::{convert_tool_result_content, EMPTY_TEXT_PLACEHOLDER};
        use crate::types::ContentBlock;
        use aws_sdk_bedrockruntime::types::ToolResultContentBlock;
        // Empty content -> placeholder.
        let out = convert_tool_result_content(&[]);
        assert_eq!(out.len(), 1);
        assert!(matches!(&out[0], ToolResultContentBlock::Text(t) if t == EMPTY_TEXT_PLACEHOLDER));
        // Blank text is skipped, real text kept.
        let out = convert_tool_result_content(&[
            ContentBlock::Text { text: "   ".into(), text_signature: None },
            ContentBlock::Text { text: "done".into(), text_signature: None },
        ]);
        assert_eq!(out.len(), 1);
        assert!(matches!(&out[0], ToolResultContentBlock::Text(t) if t == "done"));
    }

    #[test]
    fn test_bedrock_region_parsers() {
        use super::{bedrock_arn_region, bedrock_standard_endpoint_region};
        // ARN-embedded region.
        assert_eq!(bedrock_arn_region("arn:aws:bedrock:us-west-2:123:inference-profile/x").as_deref(), Some("us-west-2"));
        assert_eq!(bedrock_arn_region("arn:aws-us-gov:bedrock:us-gov-west-1:123:foundation-model/y").as_deref(), Some("us-gov-west-1"));
        assert_eq!(bedrock_arn_region("anthropic.claude-opus-4-6"), None);
        // Standard runtime endpoint region.
        assert_eq!(bedrock_standard_endpoint_region("https://bedrock-runtime.us-east-1.amazonaws.com").as_deref(), Some("us-east-1"));
        assert_eq!(bedrock_standard_endpoint_region("https://bedrock-runtime-fips.us-east-2.amazonaws.com").as_deref(), Some("us-east-2"));
        assert_eq!(bedrock_standard_endpoint_region("https://bedrock-runtime.cn-north-1.amazonaws.com.cn").as_deref(), Some("cn-north-1"));
        // Custom/VPC URL -> no standard region.
        assert_eq!(bedrock_standard_endpoint_region("https://my-vpc-proxy.internal/bedrock"), None);
    }

    #[test]
    fn test_is_govcloud_bedrock_target() {
        use super::is_govcloud_bedrock_target;
        use crate::types::{Model, ModelCost};
        fn m(id: &str) -> Model {
            Model {
                id: id.into(), name: "n".into(), api: "bedrock-converse-stream".into(),
                provider: "amazon-bedrock".into(), base_url: "".into(), reasoning: true,
                thinking_level_map: None, input: vec!["text".into()], cost: ModelCost::default(),
                context_window: 200000, max_tokens: 8192, headers: None, api_key: None, compat: Default::default(),
            }
        }
        assert!(is_govcloud_bedrock_target(&m("us-gov.anthropic.claude-opus-4-6")));
        assert!(is_govcloud_bedrock_target(&m("arn:aws-us-gov:bedrock:us-gov-west-1::foundation-model/x")));
        assert!(!is_govcloud_bedrock_target(&m("anthropic.claude-opus-4-6-v1")));
    }

    #[test]
    fn test_bedrock_supports_adaptive_thinking() {
        use super::bedrock_supports_adaptive_thinking;
        use crate::types::{Model, ModelCost};
        fn m(id: &str, name: &str) -> Model {
            Model {
                id: id.into(), name: name.into(), api: "bedrock-converse-stream".into(),
                provider: "amazon-bedrock".into(), base_url: "".into(), reasoning: true,
                thinking_level_map: None, input: vec!["text".into()], cost: ModelCost::default(),
                context_window: 200000, max_tokens: 8192, headers: None, api_key: None, compat: Default::default(),
            }
        }
        // Detected by id pattern (incl. region prefixes and separator normalization).
        assert!(bedrock_supports_adaptive_thinking(&m("anthropic.claude-opus-4-6-v1", "Claude Opus 4.6")));
        assert!(bedrock_supports_adaptive_thinking(&m("au.anthropic.claude-sonnet-4-6", "")));
        assert!(bedrock_supports_adaptive_thinking(&m("x", "Claude Opus 4.8")));
        assert!(bedrock_supports_adaptive_thinking(&m("eu.anthropic.claude-fable-5", "")));
        // Older models -> budget path.
        assert!(!bedrock_supports_adaptive_thinking(&m("anthropic.claude-opus-4-5-20251101-v1:0", "Claude Opus 4.5")));
        assert!(!bedrock_supports_adaptive_thinking(&m("anthropic.claude-sonnet-4-20250514-v1:0", "Claude Sonnet 4")));
    }

    #[test]
    fn test_normalize_bedrock_tool_call_id() {
        use super::normalize_bedrock_tool_call_id;
        assert_eq!(normalize_bedrock_tool_call_id("call:1|x"), "call_1_x");
        assert_eq!(normalize_bedrock_tool_call_id("abc-123_OK"), "abc-123_OK");
        assert_eq!(normalize_bedrock_tool_call_id(&"a".repeat(80)).len(), 64);
    }

    #[test]
    fn test_bedrock_unsupported_image_mime() {
        use super::bedrock_unsupported_image_mime;
        use crate::types::{Message, Role, ContentBlock};
        fn img_msg(mime: &str) -> Message {
            Message {
                role: Role::User,
                content: vec![ContentBlock::Image { data: "x".into(), mime_type: mime.into() }],
                timestamp: 0, api: None, provider: None, model: None, response_id: None,
                response_model: None, diagnostics: Vec::new(), usage: None, stop_reason: None,
                error_message: None, tool_call_id: None, tool_name: None, is_error: false, details: None,
            }
        }
        // Supported formats -> None.
        for ok in ["image/jpeg", "image/jpg", "image/png", "image/gif", "image/webp"] {
            assert_eq!(bedrock_unsupported_image_mime(&[img_msg(ok)]), None, "{ok}");
        }
        // Unknown format -> Some(mime) (would become "Unknown image type" error).
        assert_eq!(bedrock_unsupported_image_mime(&[img_msg("image/bmp")]), Some("image/bmp".to_string()));
    }

    #[test]
    fn test_bedrock_thinking_fields() {
        use super::bedrock_thinking_fields;
        use crate::types::{Model, ModelCost, StreamOptions, ThinkingLevel};
        let mk = |id: &str, adaptive: bool| {
            let mut m = Model {
                id: id.into(), name: String::new(), api: "bedrock-converse-stream".into(),
                provider: "bedrock".into(), base_url: String::new(), reasoning: true,
                thinking_level_map: None, input: vec!["text".into()], cost: ModelCost::default(),
                context_window: 0, max_tokens: 64000, headers: None, api_key: None, compat: Default::default(),
            };
            if adaptive { m.compat.force_adaptive_thinking = Some(true); }
            m
        };
        // Budget-based (non-adaptive) Claude: enabled + interleaved beta; budget by level.
        let opts = StreamOptions { reasoning: Some(ThinkingLevel::High), ..Default::default() };
        let (f, adj_max) = bedrock_thinking_fields(&mk("anthropic.claude-3", false), &opts).unwrap();
        assert_eq!(f["thinking"]["type"], "enabled");
        assert_eq!(f["thinking"]["budget_tokens"], 16384);
        assert!(f["anthropic_beta"].is_array());
        // No caller cap -> adjusted max is the model cap.
        assert_eq!(adj_max, Some(64000));
        // Adaptive Claude: adaptive + output_config, no interleaved beta, no max adjustment.
        let (f, adj_max) = bedrock_thinking_fields(&mk("anthropic.claude-opus-4-6", true), &opts).unwrap();
        assert_eq!(f["thinking"]["type"], "adaptive");
        assert_eq!(f["output_config"]["effort"], "high");
        assert!(f.get("anthropic_beta").is_none());
        assert_eq!(adj_max, None);
        // Native-xhigh adaptive Claude (opus-4-7): xhigh effort is preserved.
        let xopts = StreamOptions { reasoning: Some(ThinkingLevel::XHigh), ..Default::default() };
        let (f, _) = bedrock_thinking_fields(&mk("anthropic.claude-opus-4-7", false), &xopts).unwrap();
        assert_eq!(f["thinking"]["type"], "adaptive");
        assert_eq!(f["output_config"]["effort"], "xhigh");
        // Non-native adaptive (opus-4-6) with xhigh -> clamps to high.
        let (f2, _) = bedrock_thinking_fields(&mk("anthropic.claude-opus-4-6", false), &xopts).unwrap();
        assert_eq!(f2["output_config"]["effort"], "high");
        // Non-Claude model: no thinking fields.
        let f = bedrock_thinking_fields(&mk("meta.llama3", false), &opts);
        assert!(f.is_none());
        // No reasoning requested: none.
        let f = bedrock_thinking_fields(&mk("anthropic.claude-3", false), &StreamOptions::default());
        assert!(f.is_none());
    }

    #[test]
    fn test_supports_bedrock_prompt_caching() {
        use super::supports_bedrock_prompt_caching;
        use crate::types::{Model, ModelCost};
        let mk = |id: &str, name: &str| Model {
            id: id.into(), name: name.into(), api: "bedrock-converse-stream".into(),
            provider: "bedrock".into(), base_url: String::new(), reasoning: false,
            thinking_level_map: None, input: vec!["text".into()], cost: ModelCost::default(),
            context_window: 0, max_tokens: 0, headers: None, api_key: None, compat: Default::default(),
        };
        assert!(supports_bedrock_prompt_caching(&mk("anthropic.claude-sonnet-4-5", "")));
        assert!(supports_bedrock_prompt_caching(&mk("anthropic.claude-3-7-sonnet", "")));
        assert!(supports_bedrock_prompt_caching(&mk("anthropic.claude-3-5-haiku", "")));
        assert!(!supports_bedrock_prompt_caching(&mk("anthropic.claude-3-sonnet", "")));
        assert!(!supports_bedrock_prompt_caching(&mk("meta.llama3", "Llama")));
    }
}
