//! Azure OpenAI normalization helpers.
//!
//! Handles Azure-specific response format differences (tool_call cleanup).

use serde_json::Value;

/// Strip Azure-specific tool_call cleanup fields.
pub fn strip_azure_tool_call_fields(tool_calls: &mut [Value]) {
    for tc in tool_calls.iter_mut() {
        if let Some(obj) = tc.as_object_mut() {
            obj.remove("content_filter_results");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_strip_azure_tool_call_fields_local() {
        let mut calls = vec![json!({"id": "1", "content_filter_results": {}})];
        strip_azure_tool_call_fields(&mut calls);
        assert!(calls[0].get("content_filter_results").is_none());
    }
}
