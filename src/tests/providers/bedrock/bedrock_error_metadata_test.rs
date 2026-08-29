//! v0.84.0 Bedrock structured failure metadata regression.

#[cfg(test)]
mod tests {
    use crate::provider::bedrock::{
        append_bedrock_failure_diagnostic, bedrock_on_response_metadata,
        bedrock_sdk_error_request_id, bedrock_sdk_error_status, normalize_bedrock_error_code,
    };
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
            end_turn: None,
            tool_call_id: None,
            tool_name: None,
            is_error: false,
            details: None,
            added_tool_names: Vec::new(),
        }
    }

    fn details(msg: &Message) -> serde_json::Value {
        let diag = msg.diagnostics.first().expect("bedrock diagnostic");
        assert_eq!(diag.diagnostic_type, "bedrock_response_failure");
        serde_json::to_value(diag.details.as_ref().expect("details")).unwrap()
    }

    #[test]
    fn on_response_metadata_adapts_sdk_exposed_status_and_request_id_boundary() {
        let (status, headers) = bedrock_on_response_metadata(Some("req-123"));
        assert_eq!(status, 200);
        assert_eq!(
            headers.get("x-amzn-requestid").map(String::as_str),
            Some("req-123")
        );
        let (status, headers) = bedrock_on_response_metadata(None);
        assert_eq!(status, 200);
        assert!(headers.is_empty());
    }

    #[test]
    fn send_failure_diagnostic_records_status_code_and_request_id_without_rewriting_error_message()
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
        assert_eq!(
            details(&msg),
            serde_json::json!({
                "status": 400,
                "errorCode": "ValidationException",
                "requestId": "req-123"
            })
        );
        let diag = msg.diagnostics.first().unwrap();
        assert!(diag.error.name.is_none(), "diagnostic carries details only");
        assert!(
            diag.error.message.is_empty(),
            "diagnostic must not duplicate retry-facing message"
        );
    }

    #[test]
    fn modeled_mid_stream_failure_reports_request_id_only_when_sdk_has_no_code() {
        let mut msg = assistant_error();
        append_bedrock_failure_diagnostic(&mut msg, None, None, Some("stream-request"));
        assert_eq!(
            details(&msg),
            serde_json::json!({"requestId":"stream-request"})
        );
    }

    #[test]
    fn unmodeled_mid_stream_failure_captures_exception_code_and_request_id() {
        let mut msg = assistant_error();
        append_bedrock_failure_diagnostic(
            &mut msg,
            None,
            Some("ModelStreamErrorException"),
            Some("stream-request"),
        );
        assert_eq!(
            details(&msg),
            serde_json::json!({
                "errorCode":"ModelStreamErrorException",
                "requestId":"stream-request"
            })
        );
    }

    #[test]
    fn real_sdk_error_extractors_read_raw_status_request_id_and_filter_codes() {
        use aws_sdk_bedrockruntime::operation::converse_stream::ConverseStreamError;
        use aws_smithy_runtime_api::client::orchestrator::HttpResponse;
        use aws_smithy_runtime_api::client::result::SdkError;
        use aws_smithy_types::body::SdkBody;

        let mut raw = HttpResponse::new(
            aws_smithy_runtime_api::http::StatusCode::try_from(429).unwrap(),
            SdkBody::empty(),
        );
        raw.headers_mut().insert("x-amzn-requestid", "real-req");
        let err = SdkError::service_error(
            ConverseStreamError::generic(
                aws_smithy_types::error::ErrorMetadata::builder()
                    .code("ValidationException")
                    .message("invalid")
                    .build(),
            ),
            raw,
        );
        assert_eq!(bedrock_sdk_error_status(&err), Some(429));
        assert_eq!(
            bedrock_sdk_error_request_id(&err).as_deref(),
            Some("real-req")
        );
        assert_eq!(
            normalize_bedrock_error_code(Some("ValidationException")).as_deref(),
            Some("ValidationException")
        );
        assert_eq!(normalize_bedrock_error_code(Some("Unknown")), None);
        assert_eq!(normalize_bedrock_error_code(Some("TimeoutError")), None);
    }

    #[test]
    fn abort_and_no_metadata_suppress_diagnostics() {
        let mut aborted = assistant_error();
        aborted.stop_reason = Some(StopReason::Aborted);
        append_bedrock_failure_diagnostic(&mut aborted, None, None, None);
        assert!(aborted.diagnostics.is_empty());

        let mut no_metadata = assistant_error();
        append_bedrock_failure_diagnostic(&mut no_metadata, None, None, None);
        assert!(no_metadata.diagnostics.is_empty());
    }

    #[test]
    fn unknown_placeholder_is_suppressed_while_status_and_request_id_survive() {
        let mut msg = assistant_error();
        append_bedrock_failure_diagnostic(&mut msg, Some(403), Some("Unknown"), Some("req-403"));
        assert_eq!(
            details(&msg),
            serde_json::json!({"status":403,"requestId":"req-403"})
        );
    }

    #[test]
    fn drops_empty_or_overlong_values_but_keeps_status() {
        let mut msg = assistant_error();
        append_bedrock_failure_diagnostic(&mut msg, Some(400), Some("  "), Some(&"x".repeat(201)));
        assert_eq!(details(&msg), serde_json::json!({"status":400}));
    }
}
