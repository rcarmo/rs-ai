//! Test-for-test port of upstream `test/openai-completions-empty-tools.test.ts`
//! (`@earendil-works/pi-ai` v0.80.2) — the payload-shape cases.
//!
//! The Cloudflare-AI-Gateway client-construction cases (baseURL,
//! cf-aig-authorization / session-affinity default headers) are env-coupled and
//! assert the constructed HTTP client options rather than the request body; they
//! are covered separately by rs-ai's cloudflare URL/header resolution and are not
//! re-ported here (env-mutation races). The tools/max-tokens body cases are ported.

#[cfg(test)]
mod tests {
    use crate::compat::detect_compat;
    use crate::provider::openai::build_payload;
    use crate::registry::get_model;
    use crate::types::{Context, ContentBlock, Message, Model, Role, StopReason, StreamOptions};
    use serde_json::Value;
    use std::collections::HashMap;

    fn gpt_4o_mini() -> Model {
        get_model("openai", "gpt-4o-mini").expect("catalog gpt-4o-mini")
    }

    fn user(text: &str) -> Message {
        Message {
            role: Role::User,
            content: vec![ContentBlock::Text { text: text.into(), text_signature: None }],
            timestamp: 0, api: None, provider: None, model: None, response_id: None,
            response_model: None, diagnostics: Vec::new(), usage: None,
            stop_reason: None, error_message: None,
            tool_call_id: None, tool_name: None, is_error: false, details: None,
        }
    }

    fn payload(ctx: &Context, opts: &StreamOptions) -> Value {
        let m = gpt_4o_mini();
        build_payload(&m, ctx, opts, &detect_compat(&m))
    }

    #[test]
    fn omits_tools_field_when_context_tools_is_empty() {
        let ctx = Context { system_prompt: None, tools: Vec::new(), messages: vec![user("hi")] };
        let p = payload(&ctx, &StreamOptions::default());
        assert!(p.get("tools").is_none());
    }

    #[test]
    fn omits_tools_field_when_context_tools_is_undefined() {
        // rs-ai has no separate undefined/empty distinction; an absent tools list is empty.
        let ctx = Context { system_prompt: None, tools: Vec::new(), messages: vec![user("hi")] };
        let p = payload(&ctx, &StreamOptions::default());
        assert!(p.get("tools").is_none());
    }

    #[test]
    fn sends_default_max_tokens_as_model_max_tokens() {
        // v0.80.3: streamSimple defaults maxTokens to model.maxTokens and clamps it;
        // with a large context window "hi" does not clamp, so the model cap is sent.
        let ctx = Context { system_prompt: None, tools: Vec::new(), messages: vec![user("hi")] };
        let p = payload(&ctx, &StreamOptions::default());
        assert!(p.get("max_tokens").is_none());
        assert_eq!(p["max_completion_tokens"], serde_json::json!(gpt_4o_mini().max_tokens));
    }

    /// gpt-4o-mini with an overridden small window so the clamp boundary bites
    /// (mirrors upstream `{ ...baseModel, contextWindow: 10000, maxTokens: 8000 }`).
    fn clamp_payload(ctx: &Context, opts: &StreamOptions) -> Value {
        let mut m = gpt_4o_mini();
        m.context_window = 10000;
        m.max_tokens = 8000;
        build_payload(&m, ctx, opts, &detect_compat(&m))
    }

    #[test]
    fn clamps_default_max_tokens_to_remaining_context() {
        // contextWindow=10000, "x"*8000 (2000 tokens), default maxTokens=8000.
        // used = 2000 + 4096 = 6096; available = 3904; min(8000, 3904) = 3904.
        let ctx = Context { system_prompt: None, tools: Vec::new(), messages: vec![user(&"x".repeat(8000))] };
        let p = clamp_payload(&ctx, &StreamOptions::default());
        assert!(p.get("max_tokens").is_none());
        assert_eq!(p["max_completion_tokens"], serde_json::json!(3904));
    }

    #[test]
    fn clamps_explicit_max_tokens_to_remaining_context() {
        let ctx = Context { system_prompt: None, tools: Vec::new(), messages: vec![user(&"x".repeat(8000))] };
        let opts = StreamOptions { max_tokens: Some(7000), ..Default::default() };
        let p = clamp_payload(&ctx, &opts);
        assert!(p.get("max_tokens").is_none());
        assert_eq!(p["max_completion_tokens"], serde_json::json!(3904));
    }

    #[test]
    fn sends_explicit_max_tokens_as_max_completion_tokens() {
        let ctx = Context { system_prompt: None, tools: Vec::new(), messages: vec![user("hi")] };
        let opts = StreamOptions { max_tokens: Some(1234), ..Default::default() };
        let p = payload(&ctx, &opts);
        assert!(p.get("max_tokens").is_none());
        assert_eq!(p["max_completion_tokens"], serde_json::json!(1234));
    }

    #[test]
    fn still_emits_tools_empty_array_when_conversation_has_tool_history() {
        let assistant = Message {
            role: Role::Assistant,
            content: vec![ContentBlock::ToolCall { id: "t1".into(), name: "noop".into(), arguments: HashMap::new(), thought_signature: None }],
            timestamp: 0,
            api: Some("openai-completions".into()), provider: Some("openai".into()), model: Some("gpt-4o-mini".into()),
            response_id: None, response_model: None, diagnostics: Vec::new(), usage: None,
            stop_reason: Some(StopReason::ToolUse), error_message: None,
            tool_call_id: None, tool_name: None, is_error: false, details: None,
        };
        let tool_result = Message {
            role: Role::ToolResult,
            content: vec![ContentBlock::Text { text: "done".into(), text_signature: None }],
            timestamp: 0, api: None, provider: None, model: None, response_id: None,
            response_model: None, diagnostics: Vec::new(), usage: None,
            stop_reason: None, error_message: None,
            tool_call_id: Some("t1".into()), tool_name: Some("noop".into()), is_error: false, details: None,
        };
        let ctx = Context {
            system_prompt: None, tools: Vec::new(),
            messages: vec![user("use the tool"), assistant, tool_result],
        };
        let p = payload(&ctx, &StreamOptions::default());
        assert_eq!(p["tools"], serde_json::json!([]), "tool history must keep an empty tools array");
    }

    // --- Cloudflare AI Gateway client-construction cases ---
    // Ported from upstream's baseURL / cf-aig-authorization / session-affinity
    // assertions via the extracted `build_openai_request_parts` helper. These set
    // CLOUDFLARE_* env vars (no other test reads them) under a serial guard.
    use crate::provider::openai::build_openai_request_parts;
    use std::sync::Mutex;
    static CF_ENV_GUARD: Mutex<()> = Mutex::new(());

    struct CfEnv;
    impl CfEnv {
        fn set() -> Self {
            unsafe {
                std::env::set_var("CLOUDFLARE_ACCOUNT_ID", "account-id");
                std::env::set_var("CLOUDFLARE_GATEWAY_ID", "gateway-id");
            }
            CfEnv
        }
    }
    impl Drop for CfEnv {
        fn drop(&mut self) {
            unsafe {
                std::env::remove_var("CLOUDFLARE_ACCOUNT_ID");
                std::env::remove_var("CLOUDFLARE_GATEWAY_ID");
            }
        }
    }

    fn cf_model(id: &str) -> Model {
        get_model("cloudflare-ai-gateway", id).expect("cloudflare-ai-gateway model")
    }

    fn parts(model: &Model, opts: &StreamOptions, key: &str) -> (String, reqwest::header::HeaderMap) {
        build_openai_request_parts(model, &Context { system_prompt: None, tools: Vec::new(), messages: vec![user("hi")] }, opts, &detect_compat(model), key).expect("request parts")
    }

    #[test]
    fn cloudflare_gateway_resolves_compat_base_url_and_cf_aig_auth() {
        let _g = CF_ENV_GUARD.lock().unwrap();
        let _env = CfEnv::set();
        let model = cf_model("workers-ai/@cf/moonshotai/kimi-k2.6");
        let (url, headers) = parts(&model, &StreamOptions::default(), "cf-token");
        assert_eq!(url, "https://gateway.ai.cloudflare.com/v1/account-id/gateway-id/compat/chat/completions");
        assert_eq!(headers.get("cf-aig-authorization").unwrap(), "Bearer cf-token");
        assert!(headers.get("authorization").is_none(), "primary Authorization must not be set for the gateway");
    }

    #[test]
    fn cloudflare_gateway_preserves_inline_upstream_authorization_byok() {
        let _g = CF_ENV_GUARD.lock().unwrap();
        let _env = CfEnv::set();
        let model = cf_model("gpt-5.1");
        let opts = StreamOptions {
            headers: Some(HashMap::from([("Authorization".to_string(), "Bearer upstream-token".to_string())])),
            ..Default::default()
        };
        let (_url, headers) = parts(&model, &opts, "cf-token");
        assert_eq!(headers.get("authorization").unwrap(), "Bearer upstream-token");
        assert_eq!(headers.get("cf-aig-authorization").unwrap(), "Bearer cf-token");
    }

    #[test]
    fn cloudflare_gateway_sends_session_affinity_headers() {
        let _g = CF_ENV_GUARD.lock().unwrap();
        let _env = CfEnv::set();
        let model = cf_model("workers-ai/@cf/moonshotai/kimi-k2.6");
        let opts = StreamOptions { session_id: Some("session-1".into()), ..Default::default() };
        let (_url, headers) = parts(&model, &opts, "cf-token");
        assert_eq!(headers.get("session_id").unwrap(), "session-1");
        assert_eq!(headers.get("x-client-request-id").unwrap(), "session-1");
        assert_eq!(headers.get("x-session-affinity").unwrap(), "session-1");
    }
}
