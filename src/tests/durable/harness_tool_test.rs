#[cfg(test)]
mod tests {
    use crate::durable::*;
    use crate::types::{Model, ModelCompat, ModelCost, Tool};
    use serde_json::json;
    use std::sync::{Arc, Mutex};
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn model(base_url: String) -> Model {
        Model {
            id: "tool-model".into(),
            name: "Tool model".into(),
            api: "pi-messages".into(),
            provider: "durable-tool-http".into(),
            base_url,
            reasoning: false,
            thinking_level_map: None,
            input: vec!["text".into()],
            input_limits: None,
            prompt_cache: None,
            enabled: None,
            lab: None,
            providers: None,
            cost: ModelCost::default(),
            context_window: 4096,
            max_tokens: 512,
            sampling_params: None,
            headers: None,
            api_key: Some("local".into()),
            compat: ModelCompat::default(),
        }
    }
    fn definition() -> Tool {
        Tool {
            name: "echo".into(),
            description: "Echo".into(),
            parameters: json!({"type":"object","properties":{"value":{"type":"string"}},"required":["value"],"additionalProperties":false}),
            constrained_sampling: None,
        }
    }
    fn usage() -> serde_json::Value {
        json!({"input":1,"output":1,"cacheRead":0,"cacheWrite":0,"totalTokens":2,"cost":{"input":0.1,"output":0.1,"cacheRead":0.0,"cacheWrite":0.0,"total":0.2}})
    }
    struct Echo {
        seen: Arc<Mutex<Vec<ToolExecution>>>,
    }
    impl DurableTool for Echo {
        fn execute<'a>(&'a self, e: ToolExecution) -> crate::durable::tool::ToolFuture<'a> {
            Box::pin(async move {
                self.seen.lock().unwrap().push(e.clone());
                Ok(ToolOutput {
                    value: json!({"echo":e.arguments["value"]}),
                    usage: None,
                })
            })
        }
    }

    #[tokio::test]
    async fn production_model_tool_answer_uses_two_http_turns_and_one_effect() {
        let server = MockServer::start().await;
        let runtime = model(format!("{}/v1", server.uri()));
        crate::registry::register_model(runtime.clone());
        Mock::given(method("POST")).and(path("/v1/messages")).respond_with(ResponseTemplate::new(200).insert_header("content-type","text/event-stream").set_body_string(format!("data: {{\"type\":\"toolcall_start\",\"contentIndex\":0,\"id\":\"call-1\",\"toolName\":\"echo\"}}\n\ndata: {{\"type\":\"toolcall_end\",\"contentIndex\":0,\"toolCall\":{{\"type\":\"toolCall\",\"id\":\"call-1\",\"name\":\"echo\",\"arguments\":{{\"value\":\"hello\"}}}}}}\n\ndata: {{\"type\":\"done\",\"reason\":\"toolUse\",\"usage\":{}}}\n\n",usage()))).up_to_n_times(1).mount(&server).await;
        Mock::given(method("POST")).and(path("/v1/messages")).respond_with(ResponseTemplate::new(200).insert_header("content-type","text/event-stream").set_body_string(format!("data: {{\"type\":\"text_start\",\"contentIndex\":0}}\n\ndata: {{\"type\":\"text_delta\",\"contentIndex\":0,\"delta\":\"final\"}}\n\ndata: {{\"type\":\"text_end\",\"contentIndex\":0,\"content\":\"final\"}}\n\ndata: {{\"type\":\"done\",\"reason\":\"stop\",\"content\":[{{\"type\":\"text\",\"text\":\"final\"}}],\"usage\":{}}}\n\n",usage()))).mount(&server).await;
        let seen = Arc::new(Mutex::new(vec![]));
        let registry = Arc::new(DurableToolRegistry::default());
        registry
            .register(
                definition(),
                "echo-rust",
                "1",
                ReplayPolicy::Safe,
                Arc::new(Echo { seen: seen.clone() }),
            )
            .unwrap();
        let harness = DurableHarness::open_with_tools(
            Box::new(MemoryStorage::new()),
            Arc::new(RegistryModelRunner),
            PinnedModel::from_model(&runtime).unwrap(),
            PinnedOptions::default(),
            registry,
        )
        .await
        .unwrap();
        let handle = harness
            .submit(SubmitRequest {
                request_id: "tool-http".into(),
                content: "echo".into(),
            })
            .await
            .unwrap();
        assert_eq!(
            harness.wait(handle).await.unwrap().answer.as_deref(),
            Some("final")
        );
        let requests = server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 2);
        let first: serde_json::Value = serde_json::from_slice(&requests[0].body).unwrap();
        let second: serde_json::Value = serde_json::from_slice(&requests[1].body).unwrap();
        assert_eq!(
            first["context"]["messages"][0]["toolsAdded"][0]["name"],
            "echo"
        );
        assert_eq!(
            first["context"]["messages"][0]["toolsAdded"][0]["parameters"]["required"][0],
            "value"
        );
        assert!(
            second["context"]["messages"]
                .as_array()
                .unwrap()
                .iter()
                .any(|message| message["role"] == "toolResult")
        );
        assert_eq!(seen.lock().unwrap().len(), 1);
        harness.close().await.unwrap();
    }

    #[tokio::test]
    async fn unoffered_model_tool_call_terminalises_without_poisoning_fifo() {
        struct Unoffered;
        impl DurableModelRunner for Unoffered {
            fn run<'a>(&'a self, _: ModelIntent) -> crate::durable::model::ModelFuture<'a> {
                Box::pin(async {
                    ModelRun::one(ModelTerminal::ToolCalls {
                        calls: vec![DurableToolCall {
                            provider_call_id: "x".into(),
                            name: "missing".into(),
                            original_arguments: json!({}),
                        }],
                        usage: DurableUsage {
                            input: 2,
                            output: 1,
                            total_tokens: 3,
                            ..Default::default()
                        },
                        assistant: json!({"role":"assistant","content":[],"timestamp":0,"diagnostics":[],"isError":false,"addedToolNames":[],"toolsAdded":[],"toolsRemoved":[]}),
                    })
                })
            }
        }
        let harness = DurableHarness::open(
            Box::new(MemoryStorage::new()),
            Arc::new(Unoffered),
            PinnedModel::from_model(&model("http://unused".into())).unwrap(),
            PinnedOptions::default(),
        )
        .await
        .unwrap();
        let handle = harness
            .submit(SubmitRequest {
                request_id: "unknown".into(),
                content: "go".into(),
            })
            .await
            .unwrap();
        let view = harness.wait(handle).await.unwrap();
        assert_eq!(view.status, "failed");
        assert_eq!(view.reason.unwrap()["code"], "durable_tool_rejected");
        assert_eq!(
            harness.inspect_documents().await.unwrap()["pi.usage"]["aggregate"]["total_tokens"],
            3
        );
        harness.close().await.unwrap();
    }

    #[tokio::test]
    async fn successor_unoffered_call_bills_current_round_and_preserves_prior_usage() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        struct TwoRounds(AtomicUsize);
        impl DurableModelRunner for TwoRounds {
            fn run<'a>(&'a self, _: ModelIntent) -> crate::durable::model::ModelFuture<'a> {
                let n = self.0.fetch_add(1, Ordering::SeqCst);
                Box::pin(async move {
                    let usage = DurableUsage {
                        input: if n == 0 { 1 } else { 3 },
                        output: if n == 0 { 1 } else { 3 },
                        total_tokens: if n == 0 { 2 } else { 6 },
                        ..Default::default()
                    };
                    ModelRun::one(ModelTerminal::ToolCalls {
                        calls: vec![DurableToolCall {
                            provider_call_id: format!("call-{n}"),
                            name: if n == 0 {
                                "echo".into()
                            } else {
                                "missing".into()
                            },
                            original_arguments: json!({"value":"x"}),
                        }],
                        usage,
                        assistant: json!({"role":"assistant","content":[],"timestamp":0,"diagnostics":[],"isError":false,"addedToolNames":[],"toolsAdded":[],"toolsRemoved":[]}),
                    })
                })
            }
        }
        struct BilledTool;
        impl DurableTool for BilledTool {
            fn execute<'a>(&'a self, _: ToolExecution) -> crate::durable::tool::ToolFuture<'a> {
                Box::pin(async {
                    Ok(ToolOutput {
                        value: json!({"ok":true}),
                        usage: Some(DurableUsage {
                            input: 2,
                            output: 2,
                            total_tokens: 4,
                            ..Default::default()
                        }),
                    })
                })
            }
        }
        let registry = Arc::new(DurableToolRegistry::default());
        registry
            .register(
                definition(),
                "bill",
                "1",
                ReplayPolicy::Safe,
                Arc::new(BilledTool),
            )
            .unwrap();
        let harness = DurableHarness::open_with_tools(
            Box::new(MemoryStorage::new()),
            Arc::new(TwoRounds(AtomicUsize::new(0))),
            PinnedModel::from_model(&model("http://unused".into())).unwrap(),
            PinnedOptions::default(),
            registry,
        )
        .await
        .unwrap();
        let handle = harness
            .submit(SubmitRequest {
                request_id: "successor-unknown".into(),
                content: "go".into(),
            })
            .await
            .unwrap();
        let view = harness.wait(handle).await.unwrap();
        assert_eq!(view.status, "failed");
        let usage = &harness.inspect_documents().await.unwrap()["pi.usage"];
        assert_eq!(usage["aggregate"]["total_tokens"], 12, "{usage:#}");
        assert_eq!(usage["final_turn"]["total_tokens"], 6);
        harness.close().await.unwrap();
    }

    #[tokio::test]
    async fn oversized_model_arguments_terminalise_before_effect() {
        struct Oversized;
        impl DurableModelRunner for Oversized {
            fn run<'a>(&'a self, _: ModelIntent) -> crate::durable::model::ModelFuture<'a> {
                Box::pin(async {
                    ModelRun::one(ModelTerminal::ToolCalls {
                        calls: vec![DurableToolCall {
                            provider_call_id: "big".into(),
                            name: "echo".into(),
                            original_arguments: json!({"value":"x".repeat(crate::durable::types::MAX_TASK_FIELD_BYTES+1)}),
                        }],
                        usage: DurableUsage {
                            total_tokens: 7,
                            ..Default::default()
                        },
                        assistant: json!({"role":"assistant","content":[],"timestamp":0,"diagnostics":[],"isError":false,"addedToolNames":[],"toolsAdded":[],"toolsRemoved":[]}),
                    })
                })
            }
        }
        let seen = Arc::new(Mutex::new(vec![]));
        let registry = Arc::new(DurableToolRegistry::default());
        registry
            .register(
                definition(),
                "echo",
                "1",
                ReplayPolicy::Safe,
                Arc::new(Echo { seen: seen.clone() }),
            )
            .unwrap();
        let harness = DurableHarness::open_with_tools(
            Box::new(MemoryStorage::new()),
            Arc::new(Oversized),
            PinnedModel::from_model(&model("http://unused".into())).unwrap(),
            PinnedOptions::default(),
            registry,
        )
        .await
        .unwrap();
        let handle = harness
            .submit(SubmitRequest {
                request_id: "oversized".into(),
                content: "go".into(),
            })
            .await
            .unwrap();
        assert_eq!(harness.wait(handle).await.unwrap().status, "failed");
        assert!(seen.lock().unwrap().is_empty());
        assert_eq!(
            harness.inspect_documents().await.unwrap()["pi.usage"]["final_turn"]["total_tokens"],
            7
        );
        harness.close().await.unwrap();
    }

    #[test]
    fn schema_and_argument_limits_fail_before_effect() {
        let registry = DurableToolRegistry::default();
        struct Never;
        impl DurableTool for Never {
            fn execute<'a>(&'a self, _: ToolExecution) -> crate::durable::tool::ToolFuture<'a> {
                panic!("must not execute")
            }
        }
        let bad = Tool {
            name: "bad".into(),
            description: "bad".into(),
            parameters: json!({"type":"object","oneOf":[]}),
            constrained_sampling: None,
        };
        assert!(
            registry
                .register(bad, "x", "1", ReplayPolicy::Safe, Arc::new(Never))
                .is_err()
        );
    }
}
