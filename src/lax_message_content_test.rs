//! Test-for-test port of upstream `test/lax-message-content.test.ts`
//! (`@earendil-works/pi-ai` v0.80.5).
//!
//! The `Message` type requires `content`, but untyped callers (custom tools,
//! hand-built histories, old session files) can supply `null`/missing content.
//! rs-ai normalizes this at deserialization (`de_null_content_as_empty`), so the
//! choke point before every provider request (`transform_messages`) can rely on
//! the type contract instead of crashing (issues #6259, #6276).

#[cfg(test)]
mod tests {
    use crate::transform::transform_messages;
    use crate::types::{Message, Model};

    // Text-only model so the image downgrade path runs — the primary crash site
    // for null tool-result content upstream.
    fn text_only_model() -> Model {
        serde_json::from_value(serde_json::json!({
            "id": "test-model",
            "name": "Test Model",
            "api": "openai-completions",
            "provider": "openai",
            "baseUrl": "https://example.invalid/v1",
            "reasoning": false,
            "input": ["text"],
            "cost": { "input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0 },
            "contextWindow": 128000,
            "maxTokens": 16000
        }))
        .expect("model deserializes")
    }

    #[test]
    fn normalizes_null_or_missing_content_to_an_empty_array() {
        let messages: Vec<Message> = serde_json::from_value(serde_json::json!([
            { "role": "user", "content": null, "timestamp": 1 },
            {
                "role": "assistant",
                "content": null,
                "api": "openai-completions",
                "provider": "openai",
                "model": "test-model",
                "usage": {
                    "input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0,
                    "totalTokens": 0,
                    "cost": { "input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0, "total": 0 }
                },
                "stopReason": "stop",
                "timestamp": 1
            },
            {
                "role": "toolResult",
                "toolCallId": "call_1",
                "toolName": "web_search",
                "isError": false,
                "timestamp": 1
            }
        ]))
        .expect("messages with null/missing content deserialize");

        let result = transform_messages(&messages, &text_only_model());

        assert_eq!(result.len(), 3);
        for msg in &result {
            assert!(msg.content.is_empty(), "content normalized to empty vec");
        }
    }
}
