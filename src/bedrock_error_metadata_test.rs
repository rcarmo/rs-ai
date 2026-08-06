//! v0.84.0 Bedrock structured failure metadata regression.

#[cfg(test)]
mod tests {
    use crate::provider::bedrock::append_bedrock_failure_diagnostic;
    use crate::types::{ContentBlock, Message, Role, StopReason};

    fn assistant_error() -> Message {
        Message {
            role: Role::Assistant,
            content: vec![ContentBlock::Text {
                text: "partial".into(),
                text_signature: None,
            }],
            timestamp: 0,
            api: Some("bedrock-converse-stream".into()),
            provider: Some("amazon-bedrock".into()),
            model: Some("anthropic.claude".into()),
            response_id: None,
            response_model: None,
            diagnostics: Vec::new(),
            usage: None,
            stop_reason: Some(StopReason::Error),
            deferred: None,
            error_message: Some("Validation error: model rejected request".into()),
            raw_stop_reason: None,
            tool_call_id: None,
            tool_name: None,
            is_error: false,
            details: None,
            added_tool_names: Vec::new(),
        }
    }

    #[test]
    fn bedrock_failure_diagnostic_preserves_status_code_and_request_id_without_rewriting_error_message()
     {
        let mut msg = assistant_error();
        append_bedrock_failure_diagnostic(
            &mut msg,
            Some(400),
            Some("ValidationException"),
            Some("req-123"),
        );
        assert_eq!(
            msg.error_message.as_deref(),
            Some("Validation error: model rejected request")
        );
        let diag = msg.diagnostics.first().expect("bedrock diagnostic");
        assert_eq!(diag.diagnostic_type, "bedrock_response_failure");
        let details = diag.details.as_ref().expect("details");
        assert_eq!(details.get("status"), Some(&serde_json::json!(400)));
        assert_eq!(
            details.get("errorCode"),
            Some(&serde_json::json!("ValidationException"))
        );
        assert_eq!(
            details.get("requestId"),
            Some(&serde_json::json!("req-123"))
        );
    }

    #[test]
    fn bedrock_failure_diagnostic_drops_empty_or_overlong_values() {
        let mut msg = assistant_error();
        append_bedrock_failure_diagnostic(&mut msg, None, Some("  "), Some(&"x".repeat(201)));
        assert!(msg.diagnostics.is_empty());
    }
}
