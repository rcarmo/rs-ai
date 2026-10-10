//! v1.0.0 OpenAI Responses grammar replay parity.

#[cfg(test)]
mod tests {
    use crate::events::Event;
    use crate::provider::responses::build_responses_payload;
    use crate::provider::responses::stream_responses;
    use crate::registry::get_model;
    use crate::types::{ContentBlock, Context, Message, Role, StopReason, StreamOptions, Tool};
    use futures::StreamExt;
    use std::collections::HashMap;
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn grammar_tool() -> Tool {
        Tool {
            name: "emit".into(),
            description: "emit grammar".into(),
            parameters: serde_json::json!({
                "type":"object",
                "properties":{"payload":{"type":"string"}},
                "required":["payload"]
            }),
            constrained_sampling: Some(serde_json::json!({
                "type":"grammar",
                "variants":{"openai_regex":"[a-z]+"}
            })),
        }
    }

    fn assistant(
        id: &str,
        arguments: HashMap<String, serde_json::Value>,
        provider: &str,
        api: &str,
        model: &str,
    ) -> Message {
        Message {
            role: Role::Assistant,
            content: vec![ContentBlock::ToolCall {
                id: id.into(),
                name: "emit".into(),
                arguments,
                thought_signature: None,
                namespace: None,
            }],
            provider: Some(provider.into()),
            api: Some(api.into()),
            model: Some(model.into()),
            stop_reason: Some(StopReason::ToolUse),
            timestamp: 0,
            duration_ms: None,
            response_id: None,
            response_model: None,
            provider_thinking_level: None,
            thinking_level: None,
            diagnostics: Vec::new(),
            usage: None,
            deferred: None,
            error_message: None,
            raw_stop_reason: None,
            end_turn: None,
            tool_call_id: None,
            tool_name: None,
            is_error: false,
            details: None,
            nested_calls: None,
            added_tool_names: Vec::new(),
            sections: None,
            tools_added: Vec::new(),
            tools_removed: Vec::new(),
        }
    }

    fn result(id: &str) -> Message {
        Message {
            role: Role::ToolResult,
            content: vec![ContentBlock::Text {
                text: "ok".into(),
                text_signature: None,
            }],
            tool_call_id: Some(id.into()),
            tool_name: Some("emit".into()),
            timestamp: 0,
            duration_ms: None,
            provider: None,
            api: None,
            model: None,
            response_id: None,
            response_model: None,
            provider_thinking_level: None,
            thinking_level: None,
            diagnostics: Vec::new(),
            usage: None,
            stop_reason: None,
            deferred: None,
            error_message: None,
            raw_stop_reason: None,
            end_turn: None,
            is_error: false,
            details: None,
            nested_calls: None,
            added_tool_names: Vec::new(),
            sections: None,
            tools_added: Vec::new(),
            tools_removed: Vec::new(),
        }
    }

    fn replay(
        id: &str,
        arguments: HashMap<String, serde_json::Value>,
        source_provider: &str,
        source_api: &str,
        source_model: &str,
        supports_grammar: bool,
    ) -> serde_json::Value {
        let mut model = get_model("openai", "gpt-5-mini").unwrap();
        model.compat.supports_openai_grammar_tools = Some(supports_grammar);
        build_responses_payload(
            &model,
            &Context {
                system_prompt: None,
                messages: vec![
                    assistant(id, arguments, source_provider, source_api, source_model),
                    result(id),
                ],
                tools: vec![grammar_tool()],
            },
            &StreamOptions::default(),
        )
    }

    fn call(payload: &serde_json::Value) -> &serde_json::Value {
        payload["input"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| {
                matches!(
                    item["type"].as_str(),
                    Some("custom_tool_call" | "function_call")
                )
            })
            .unwrap()
    }

    fn output(payload: &serde_json::Value) -> &serde_json::Value {
        payload["input"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| {
                item["type"]
                    .as_str()
                    .is_some_and(|kind| kind.ends_with("_output"))
            })
            .unwrap()
    }

    #[test]
    fn capability_and_transcript_resolve_custom_call_result_and_item_ids() {
        let model = get_model("openai", "gpt-5-mini").unwrap();
        let args = HashMap::from([("payload".into(), serde_json::json!("abc"))]);

        let same = replay(
            "call_1|ctc_1",
            args.clone(),
            &model.provider,
            &model.api,
            &model.id,
            true,
        );
        assert_eq!(call(&same)["type"], "custom_tool_call");
        assert_eq!(call(&same)["id"], "ctc_1");
        assert_eq!(call(&same)["call_id"], "call_1");
        assert_eq!(call(&same)["input"], "abc");
        assert_eq!(output(&same)["type"], "custom_tool_call_output");
        assert_eq!(output(&same)["call_id"], "call_1");

        let mismatched = replay(
            "call_1|fc_1",
            args.clone(),
            &model.provider,
            &model.api,
            &model.id,
            true,
        );
        assert!(call(&mismatched).get("id").is_none());

        let different_model = replay(
            "call_1|ctc_1",
            args.clone(),
            &model.provider,
            &model.api,
            "gpt-other",
            true,
        );
        assert!(call(&different_model).get("id").is_none());

        for foreign_id in ["call_1|raw", "call_1|fc_1", "call_1|ctc_1"] {
            let foreign = replay(
                foreign_id,
                args.clone(),
                "pi-messages",
                "radius",
                "gpt-other",
                true,
            );
            assert_eq!(call(&foreign)["type"], "custom_tool_call");
            assert!(call(&foreign).get("id").is_none(), "{foreign_id}");
            assert_eq!(call(&foreign)["call_id"], "call_1");
            assert_eq!(call(&foreign)["input"], "abc");
        }

        let function = replay(
            "call_1|fc_1",
            args,
            &model.provider,
            &model.api,
            &model.id,
            false,
        );
        assert_eq!(call(&function)["type"], "function_call");
        assert_eq!(call(&function)["id"], "fc_1");
        assert_eq!(output(&function)["type"], "function_call_output");
    }

    #[test]
    fn transcript_tool_addition_drives_declaration_call_and_result_together() {
        let mut model = get_model("openai", "gpt-5-mini").unwrap();
        model.compat.supports_openai_grammar_tools = Some(true);
        model.compat.supports_mid_convo_system_messages = Some(true);
        let id = "call_1|ctc_1";
        let args = HashMap::from([("payload".into(), serde_json::json!("abc"))]);
        let system =
            crate::transcript::system_message("tools", None, vec![grammar_tool()], Vec::new());
        let payload = build_responses_payload(
            &model,
            &Context {
                system_prompt: None,
                messages: vec![
                    system,
                    assistant(id, args, &model.provider, &model.api, &model.id),
                    result(id),
                ],
                tools: Vec::new(),
            },
            &StreamOptions::default(),
        );
        assert_eq!(payload["tools"][0]["type"], "custom");
        assert_eq!(call(&payload)["type"], "custom_tool_call");
        assert_eq!(output(&payload)["type"], "custom_tool_call_output");
    }

    #[test]
    fn default_unsupported_capability_uses_function_declaration_call_and_result() {
        let mut model = get_model("openai", "gpt-5-mini").unwrap();
        model.compat.supports_openai_grammar_tools = None;
        let id = "call_1|fc_1";
        let payload = build_responses_payload(
            &model,
            &Context {
                system_prompt: None,
                messages: vec![
                    assistant(
                        id,
                        HashMap::from([("payload".into(), serde_json::json!("abc"))]),
                        &model.provider,
                        &model.api,
                        &model.id,
                    ),
                    result(id),
                ],
                tools: vec![grammar_tool()],
            },
            &StreamOptions::default(),
        );
        assert_eq!(payload["tools"][0]["type"], "function");
        assert_eq!(call(&payload)["type"], "function_call");
        assert_eq!(output(&payload)["type"], "function_call_output");
    }

    #[tokio::test]
    async fn production_transport_omits_mismatched_id_key_and_sends_agreed_custom_shapes() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).insert_header("content-type", "text/event-stream").set_body_string(concat!(
                "data: {\"type\":\"response.completed\",\"response\":{\"status\":\"completed\",\"usage\":{\"input_tokens\":1,\"output_tokens\":1,\"total_tokens\":2}}}\n\n",
                "data: [DONE]\n\n"
            )))
            .mount(&server)
            .await;
        let mut model = get_model("openai", "gpt-5-mini").unwrap();
        model.base_url = server.uri();
        model.api_key = Some("test-key".into());
        model.compat.supports_openai_grammar_tools = Some(true);
        let id = "call_1|fc_wrong";
        let context = Context {
            system_prompt: None,
            messages: vec![
                assistant(
                    id,
                    HashMap::from([("payload".into(), serde_json::json!("abc"))]),
                    &model.provider,
                    &model.api,
                    &model.id,
                ),
                result(id),
            ],
            tools: vec![grammar_tool()],
        };
        let options = StreamOptions::default();
        let mut stream = stream_responses(&model, &context, &options);
        while let Some(event) = stream.next().await {
            if let Event::Error { error, .. } = event {
                panic!("unexpected error: {error}");
            }
        }
        let requests = server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].url.path(), "/responses");
        let wire: serde_json::Value = serde_json::from_slice(&requests[0].body).unwrap();
        assert_eq!(wire["tools"][0]["type"], "custom");
        let wire_call = call(&wire);
        assert_eq!(wire_call["type"], "custom_tool_call");
        assert!(wire_call.get("id").is_none());
        assert_eq!(wire_call["call_id"], "call_1");
        assert_eq!(wire_call["input"], "abc");
        assert_eq!(output(&wire)["type"], "custom_tool_call_output");
    }

    #[test]
    fn missing_and_null_grammar_arguments_replay_as_empty_input() {
        let model = get_model("openai", "gpt-5-mini").unwrap();
        for arguments in [
            HashMap::new(),
            HashMap::from([("payload".into(), serde_json::Value::Null)]),
        ] {
            let payload = replay(
                "call_1|ctc_1",
                arguments,
                &model.provider,
                &model.api,
                &model.id,
                true,
            );
            assert_eq!(call(&payload)["input"], "");
        }
    }
}
