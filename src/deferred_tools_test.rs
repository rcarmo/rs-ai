//! Test-for-test behavioral port of upstream `deferred-tools.test.ts`.

#[cfg(test)]
mod tests {
    use crate::estimate::estimate_context_tokens;
    use crate::provider::anthropic::build_anthropic_payload;
    use crate::provider::codex::build_codex_payload;
    use crate::provider::responses::build_responses_payload;
    use crate::registry::get_model;
    use crate::types::*;
    use serde_json::{Value, json};

    fn tool(name: &str) -> Tool {
        Tool {
            name: name.into(),
            description: format!("The {name} tool"),
            parameters: json!({"type":"object","properties":{"value":{"type":"string"}},"required":["value"]}),
            constrained_sampling: None,
        }
    }

    fn user(ts: i64) -> Message {
        Message {
            role: Role::User,
            content: vec![ContentBlock::Text {
                text: "Hello".into(),
                text_signature: None,
            }],
            timestamp: ts,
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
            tool_call_id: None,
            tool_name: None,
            is_error: false,
            details: None,
            added_tool_names: Vec::new(),
        }
    }

    fn assistant_tool_call() -> Message {
        Message {
            role: Role::Assistant,
            content: vec![ContentBlock::ToolCall {
                id: "call_1".into(),
                name: "base_tool".into(),
                arguments: Default::default(),
                thought_signature: None,
            }],
            timestamp: 2,
            api: Some("anthropic-messages".into()),
            provider: Some("anthropic".into()),
            model: Some("claude-opus-4-6".into()),
            response_id: None,
            response_model: None,
            diagnostics: Vec::new(),
            usage: Some(Usage::default()),
            stop_reason: Some(StopReason::ToolUse),
            deferred: None,
            error_message: None,
            raw_stop_reason: None,
            tool_call_id: None,
            tool_name: None,
            is_error: false,
            details: None,
            added_tool_names: Vec::new(),
        }
    }

    fn result(added: &[&str]) -> Message {
        Message {
            role: Role::ToolResult,
            content: vec![ContentBlock::Text {
                text: "done".into(),
                text_signature: None,
            }],
            timestamp: 3,
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
            tool_call_id: Some("call_1".into()),
            tool_name: Some("base_tool".into()),
            is_error: false,
            details: None,
            added_tool_names: added.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn context(tools: Vec<Tool>, added: &[&str]) -> Context {
        Context {
            system_prompt: None,
            messages: vec![user(1), assistant_tool_call(), result(added), user(4)],
            tools,
        }
    }

    fn anthropic_tool_result_content(payload: &Value) -> &Vec<Value> {
        payload["messages"]
            .as_array()
            .unwrap()
            .iter()
            .find_map(|m| {
                m["content"]
                    .as_array()
                    .filter(|a| a.iter().any(|b| b["type"] == "tool_result"))
            })
            .expect("tool_result content")
    }

    fn tool_names(payload: &Value) -> Vec<String> {
        payload["tools"]
            .as_array()
            .unwrap_or(&Vec::new())
            .iter()
            .map(|t| {
                t["name"]
                    .as_str()
                    .or_else(|| t["function"]["name"].as_str())
                    .unwrap_or("")
                    .to_string()
            })
            .collect()
    }

    fn response_tool_search_output(payload: &Value) -> Option<&Value> {
        payload["input"]
            .as_array()?
            .iter()
            .find(|i| i["type"] == "tool_search_output")
    }

    #[test]
    fn anthropic_loads_tool_at_tool_result_marker() {
        let ctx = context(vec![tool("base_tool"), tool("late_tool")], &["late_tool"]);
        let payload = build_anthropic_payload(
            &get_model("anthropic", "claude-opus-4-6").unwrap(),
            &ctx,
            &StreamOptions::default(),
        );
        assert_eq!(tool_names(&payload), vec!["base_tool", "late_tool"]);
        assert_eq!(payload["tools"][1]["defer_loading"], true);
        assert_eq!(
            anthropic_tool_result_content(&payload)[0]["content"],
            json!([{ "type": "tool_reference", "tool_name": "late_tool" }])
        );
    }

    #[test]
    fn anthropic_preserves_tool_output_as_sibling_content_after_references() {
        let mut ctx = context(vec![tool("base_tool"), tool("late_tool")], &["late_tool"]);
        ctx.messages[1].content = vec![
            ContentBlock::ToolCall {
                id: "call_1".into(),
                name: "base_tool".into(),
                arguments: Default::default(),
                thought_signature: None,
            },
            ContentBlock::ToolCall {
                id: "call_2".into(),
                name: "base_tool".into(),
                arguments: Default::default(),
                thought_signature: None,
            },
        ];
        ctx.messages[2].content = vec![
            ContentBlock::Text {
                text: "work completed".into(),
                text_signature: None,
            },
            ContentBlock::Image {
                mime_type: "image/png".into(),
                data: "aW1hZ2U=".into(),
            },
        ];
        let mut second = result(&[]);
        second.tool_call_id = Some("call_2".into());
        second.content = vec![ContentBlock::Text {
            text: "second result".into(),
            text_signature: None,
        }];
        ctx.messages.insert(3, second);
        let payload = build_anthropic_payload(
            &get_model("anthropic", "claude-opus-4-6").unwrap(),
            &ctx,
            &StreamOptions::default(),
        );
        let content = anthropic_tool_result_content(&payload);
        assert_eq!(
            content[0]["content"],
            json!([{ "type": "tool_reference", "tool_name": "late_tool" }])
        );
        assert_eq!(content[1], json!({"type":"text","text":"work completed"}));
        assert_eq!(
            content[2]["source"],
            json!({"type":"base64","media_type":"image/png","data":"aW1hZ2U="})
        );
        assert_eq!(content[3]["content"], "second result");
    }

    #[test]
    fn anthropic_loads_tool_introduced_by_openai_history() {
        let mut ctx = context(vec![tool("base_tool"), tool("late_tool")], &["late_tool"]);
        ctx.messages[1].api = Some("openai-responses".into());
        ctx.messages[1].provider = Some("openai".into());
        ctx.messages[1].model = Some("gpt-5.4".into());
        let payload = build_anthropic_payload(
            &get_model("anthropic", "claude-opus-4-8").unwrap(),
            &ctx,
            &StreamOptions::default(),
        );
        assert_eq!(payload["tools"][1]["defer_loading"], true);
        assert_eq!(
            anthropic_tool_result_content(&payload)[0]["content"][0]["tool_name"],
            "late_tool"
        );
    }

    #[test]
    fn missing_marked_tool_is_not_resurrected() {
        let ctx = context(vec![tool("base_tool")], &["late_tool"]);
        let payload = build_anthropic_payload(
            &get_model("anthropic", "claude-opus-4-6").unwrap(),
            &ctx,
            &StreamOptions::default(),
        );
        assert_eq!(tool_names(&payload), vec!["base_tool"]);
        assert!(
            !anthropic_tool_result_content(&payload)[0]["content"]
                .to_string()
                .contains("tool_reference")
        );
    }

    #[test]
    fn tool_used_before_marker_stays_immediate() {
        let mut ctx = context(vec![tool("base_tool"), tool("late_tool")], &["late_tool"]);
        ctx.messages[1].content = vec![ContentBlock::ToolCall {
            id: "call_1".into(),
            name: "late_tool".into(),
            arguments: Default::default(),
            thought_signature: None,
        }];
        let payload = build_anthropic_payload(
            &get_model("anthropic", "claude-opus-4-6").unwrap(),
            &ctx,
            &StreamOptions::default(),
        );
        assert_eq!(tool_names(&payload), vec!["base_tool", "late_tool"]);
        assert!(
            payload["tools"]
                .as_array()
                .unwrap()
                .iter()
                .all(|t| t.get("defer_loading").is_none())
        );
    }

    #[test]
    fn oauth_names_are_canonicalized_for_prior_usage_markers_and_dedup() {
        let mut ctx = context(vec![tool("base_tool"), tool("read")], &["read"]);
        ctx.messages[1].content = vec![ContentBlock::ToolCall {
            id: "call_1".into(),
            name: "Read".into(),
            arguments: Default::default(),
            thought_signature: None,
        }];
        let opts = StreamOptions {
            api_key: Some("sk-ant-oat-fake".into()),
            ..Default::default()
        };
        let payload = build_anthropic_payload(
            &get_model("anthropic", "claude-opus-4-6").unwrap(),
            &ctx,
            &opts,
        );
        assert_eq!(tool_names(&payload), vec!["base_tool", "Read"]);
        assert!(
            payload["tools"]
                .as_array()
                .unwrap()
                .iter()
                .all(|t| t.get("defer_loading").is_none())
        );

        let ctx = context(vec![tool("base_tool"), tool("read")], &["Read"]);
        let payload = build_anthropic_payload(
            &get_model("anthropic", "claude-opus-4-6").unwrap(),
            &ctx,
            &opts,
        );
        assert_eq!(payload["tools"][1]["defer_loading"], true);
        assert_eq!(
            anthropic_tool_result_content(&payload)[0]["content"][0]["tool_name"],
            "Read"
        );

        let ctx = Context {
            system_prompt: None,
            messages: vec![user(1)],
            tools: vec![
                tool("read"),
                Tool {
                    description: "Canonical definition".into(),
                    ..tool("Read")
                },
            ],
        };
        let payload = build_anthropic_payload(
            &get_model("anthropic", "claude-opus-4-6").unwrap(),
            &ctx,
            &opts,
        );
        assert_eq!(tool_names(&payload), vec!["Read"]);
        assert_eq!(payload["tools"][0]["description"], "Canonical definition");
    }

    #[test]
    fn anthropic_unsupported_or_all_marked_or_override_branches() {
        let ctx = context(vec![tool("base_tool"), tool("late_tool")], &["late_tool"]);
        for model in [get_model("anthropic", "claude-haiku-4-5").unwrap(), {
            let mut m = get_model("anthropic", "claude-opus-4-6").unwrap();
            m.id = "claude-sonnet-4-20250514".into();
            m
        }] {
            let payload = build_anthropic_payload(&model, &ctx, &StreamOptions::default());
            assert_eq!(tool_names(&payload), vec!["base_tool", "late_tool"]);
            assert!(
                payload["tools"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .all(|t| t.get("defer_loading").is_none())
            );
        }
        let ctx_all = context(vec![tool("late_tool")], &["late_tool"]);
        let payload = build_anthropic_payload(
            &get_model("anthropic", "claude-opus-4-6").unwrap(),
            &ctx_all,
            &StreamOptions::default(),
        );
        assert_eq!(tool_names(&payload), vec!["late_tool"]);
        assert!(payload["tools"][0].get("defer_loading").is_none());

        let mut proxy = get_model("anthropic", "claude-opus-4-6").unwrap();
        proxy.provider = "anthropic-proxy".into();
        proxy.compat.supports_tool_references = Some(true);
        let payload = build_anthropic_payload(&proxy, &ctx, &StreamOptions::default());
        assert_eq!(payload["tools"][1]["defer_loading"], true);
    }

    #[test]
    fn openai_responses_tool_search_supported_and_fallbacks() {
        let ctx = context(vec![tool("base_tool"), tool("late_tool")], &["late_tool"]);
        let payload = build_responses_payload(
            &get_model("openai", "gpt-5.4").unwrap(),
            &ctx,
            &StreamOptions::default(),
        );
        assert_eq!(tool_names(&payload), vec!["base_tool"]);
        let out = response_tool_search_output(&payload).unwrap();
        assert_eq!(out["tools"][0]["name"], "late_tool");
        assert_eq!(out["tools"][0]["defer_loading"], true);

        for id in ["gpt-5.2", "gpt-5.4-nano", "gpt-5.5-pro"] {
            let payload = build_responses_payload(
                &get_model("openai", id).unwrap(),
                &ctx,
                &StreamOptions::default(),
            );
            assert_eq!(tool_names(&payload), vec!["base_tool", "late_tool"]);
            assert!(response_tool_search_output(&payload).is_none());
        }
        let mut disabled = get_model("openai", "gpt-5.4").unwrap();
        disabled.provider = "openai-proxy".into();
        disabled.compat.supports_tool_search = Some(false);
        let payload = build_responses_payload(&disabled, &ctx, &StreamOptions::default());
        assert_eq!(tool_names(&payload), vec!["base_tool", "late_tool"]);
        assert!(response_tool_search_output(&payload).is_none());
    }

    #[test]
    fn codex_tool_search_only_for_supported_models_and_other_providers_unchanged() {
        let ctx = context(vec![tool("base_tool"), tool("late_tool")], &["late_tool"]);
        let supported = build_codex_payload(
            &get_model("openai-codex", "gpt-5.4").unwrap(),
            &ctx,
            &StreamOptions::default(),
        );
        assert_eq!(tool_names(&supported), vec!["base_tool"]);
        assert!(response_tool_search_output(&supported).is_some());
        let unsupported = build_codex_payload(
            &get_model("openai-codex", "gpt-5.3-codex-spark").unwrap(),
            &ctx,
            &StreamOptions::default(),
        );
        assert_eq!(tool_names(&unsupported), vec!["base_tool", "late_tool"]);
        assert!(response_tool_search_output(&unsupported).is_none());

        let groq = get_model("groq", "llama-3.3-70b-versatile").unwrap();
        assert!(!crate::deferred_tools::openai_supports_tool_search(&groq));
        let (tools, deferred) =
            crate::deferred_tools::immediate_and_deferred_tools(&ctx, false, false);
        assert_eq!(
            tools.iter().map(|t| t.name.as_str()).collect::<Vec<_>>(),
            vec!["base_tool", "late_tool"]
        );
        assert!(deferred.is_empty());
    }

    #[test]
    fn estimate_counts_definitions_marked_after_latest_usage_checkpoint() {
        let mut assistant = assistant_tool_call();
        assistant.content = vec![ContentBlock::Text {
            text: "done".into(),
            text_signature: None,
        }];
        assistant.usage = Some(Usage {
            input: 50,
            output: 50,
            total_tokens: 100,
            ..Default::default()
        });
        assistant.stop_reason = Some(StopReason::Stop);
        let plain = estimate_context_tokens(&Context {
            system_prompt: None,
            messages: vec![assistant.clone(), user(4)],
            tools: vec![],
        });
        let late = Tool {
            description: "x".repeat(4000),
            ..tool("late_tool")
        };
        let marked = estimate_context_tokens(&Context {
            system_prompt: None,
            messages: vec![assistant, result(&["late_tool"])],
            tools: vec![late],
        });
        assert!(
            marked.tokens > plain.tokens + 500,
            "{marked:?} <= {plain:?}"
        );
        assert!(
            marked.trailing_tokens > plain.trailing_tokens + 500,
            "{marked:?} <= {plain:?}"
        );
    }
}
