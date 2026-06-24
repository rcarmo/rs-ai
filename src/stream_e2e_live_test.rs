//! Live-gated E2E wrappers mirroring upstream `test/stream.test.ts`
//! ("Generate E2E Tests" → `basicTextGeneration`).
//!
//! Upstream gates these per provider with `describe.skipIf(!process.env.<KEY>)`.
//! These Rust ports mirror that exactly: each test resolves the same env API key
//! and **skips cleanly** (returning early, like vitest's `skipIf`) when the key
//! is absent, and **runs the real provider call with identical assertions** when
//! a key is present. This makes the suite structurally identical to upstream's
//! live bucket without fabricating any expected model output.
//!
//! Status: LIVE-GATED (mirrors upstream skipIf). They assert the deterministic
//! invariants upstream asserts (role=assistant, usage>0, no error, response
//! contains the requested sentinel) — never a fabricated model phrasing.

#[cfg(test)]
mod tests {
    use crate::registry::{complete, get_model};
    use crate::types::{ContentBlock, Context, Message, Role, StreamOptions};

    /// Mirror of upstream `describe.skipIf(!process.env.<KEY>)`: return the key
    /// value if present, else `None` so the caller skips like vitest does.
    fn live_key(name: &str) -> Option<String> {
        std::env::var(name).ok().filter(|v| !v.trim().is_empty())
    }

    fn basic_context() -> Context {
        Context {
            system_prompt: Some("You are a helpful assistant. Be concise.".into()),
            tools: Vec::new(),
            messages: vec![Message {
                role: Role::User,
                content: vec![ContentBlock::Text {
                    text: "Reply with exactly: 'Hello test successful'".into(),
                    text_signature: None,
                }],
                timestamp: 0, api: None, provider: None, model: None, response_id: None,
                response_model: None, diagnostics: Vec::new(), usage: None,
                stop_reason: None, error_message: None, tool_call_id: None,
                tool_name: None, is_error: false, details: None,
            }],
        }
    }

    /// Upstream `basicTextGeneration` assertions (single-turn portion).
    async fn assert_basic_generation(provider: &str, id: &str, api_key: String) {
        let mut model = get_model(provider, id)
            .unwrap_or_else(|| panic!("catalog model {provider}/{id} must exist"));
        model.api_key = Some(api_key);
        let ctx = basic_context();
        let response = complete(&model, &ctx, &StreamOptions::default())
            .await
            .expect("live completion should succeed");

        assert_eq!(response.role, Role::Assistant);
        assert!(!response.content.is_empty(), "response content must be truthy");
        let usage = response.usage.expect("usage present");
        assert!(usage.input + usage.cache_read > 0, "input+cacheRead > 0");
        assert!(usage.output > 0, "output > 0");
        assert!(response.error_message.is_none(), "no errorMessage");
        let text: String = response.content.iter()
            .map(|b| if let ContentBlock::Text { text, .. } = b { text.as_str() } else { "" })
            .collect();
        assert!(text.contains("Hello test successful"), "got: {text:?}");
    }

    macro_rules! live_basic_generation {
        ($test:ident, $env:literal, $provider:literal, $id:literal) => {
            #[tokio::test]
            async fn $test() {
                match live_key($env) {
                    None => { eprintln!("skipped: {} not set", $env); }
                    Some(key) => assert_basic_generation($provider, $id, key).await,
                }
            }
        };
    }

    // Mirror upstream's per-provider gated "should complete basic text generation".
    live_basic_generation!(gemini_basic_text_generation, "GEMINI_API_KEY", "google", "gemini-2.5-flash");
    live_basic_generation!(openai_completions_basic_text_generation, "OPENAI_API_KEY", "openai", "gpt-4o-mini");
    live_basic_generation!(openai_responses_basic_text_generation, "OPENAI_API_KEY", "openai", "gpt-5.4");
    live_basic_generation!(deepseek_basic_text_generation, "DEEPSEEK_API_KEY", "deepseek", "deepseek-v4-flash");
    live_basic_generation!(anthropic_basic_text_generation, "ANTHROPIC_API_KEY", "anthropic", "claude-haiku-4-5");

    /// Deterministic guard: every live-gated model must resolve in the catalog,
    /// so a keyed run can never panic on a missing model id.
    #[test]
    fn live_gated_models_exist_in_catalog() {
        for (provider, id) in [
            ("google", "gemini-2.5-flash"),
            ("openai", "gpt-4o-mini"),
            ("openai", "gpt-5.4"),
            ("deepseek", "deepseek-v4-flash"),
            ("anthropic", "claude-haiku-4-5"),
        ] {
            assert!(get_model(provider, id).is_some(), "catalog missing {provider}/{id}");
        }
    }
}
