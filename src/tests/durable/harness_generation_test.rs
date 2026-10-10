#[cfg(test)]
mod tests {
    use crate::durable::*;
    use crate::types::{Model, ModelCompat, ModelCost};
    use std::sync::Arc;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn model(base_url: String, id: &str, provider: &str) -> Model {
        Model {
            id: id.into(),
            name: "Durable test".into(),
            api: "pi-messages".into(),
            provider: provider.into(),
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
            sampling_params_by_thinking_level: None,
            headers: None,
            api_key: Some("local-key".into()),
            compat: ModelCompat::default(),
        }
    }

    #[tokio::test]
    async fn production_registry_stream_commits_real_http_answer_and_usage() {
        let server = MockServer::start().await;
        let runtime = model(format!("{}/v1", server.uri()), "http-model", "durable-http");
        crate::registry::register_model(runtime.clone());
        Mock::given(method("POST")).and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(200).insert_header("content-type", "text/event-stream").set_body_string(concat!(
                "data: {\"type\":\"start\"}\n\n",
                "data: {\"type\":\"text_start\",\"contentIndex\":0}\n\n",
                "data: {\"type\":\"text_delta\",\"contentIndex\":0,\"delta\":\"real\"}\n\n",
                "data: {\"type\":\"text_end\",\"contentIndex\":0,\"content\":\"real\"}\n\n",
                "data: {\"type\":\"done\",\"reason\":\"stop\",\"content\":[{\"type\":\"text\",\"text\":\"real\"}],\"usage\":{\"input\":2,\"output\":1,\"cacheRead\":0,\"cacheWrite\":0,\"totalTokens\":3,\"cost\":{\"input\":0.1,\"output\":0.2,\"cacheRead\":0.0,\"cacheWrite\":0.0,\"total\":0.3}}}\n\n"
            ))).expect(1).mount(&server).await;

        let harness = DurableHarness::open(
            Box::new(MemoryStorage::new()),
            Arc::new(RegistryModelRunner),
            PinnedModel::from_model(&runtime).unwrap(),
            PinnedOptions {
                max_tokens: Some(333),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        let handle = harness
            .submit(SubmitRequest {
                request_id: "http-1".into(),
                content: "hello".into(),
            })
            .await
            .unwrap();
        let result = harness.wait(handle).await.unwrap();
        assert_eq!(result.answer.as_deref(), Some("real"), "{result:?}");
        let entries = harness
            .entries(EntryQuery {
                kind: Some("assistant".into()),
                ..Default::default()
            })
            .await
            .unwrap();
        let duration = entries.items[0].value["durationMs"]
            .as_u64()
            .expect("stream duration persisted");
        let context = harness
            .message_context(ContextOptions::default())
            .await
            .unwrap();
        assert_eq!(context.last().unwrap().duration_ms, Some(duration));
        assert_eq!(
            result.usage.unwrap()["cost"]["total"],
            serde_json::json!(0.3)
        );
        let documents = harness.inspect_documents().await.unwrap();
        assert_eq!(documents["pi.usage"]["turn"]["cost"]["total"], 0.3);
        assert_eq!(documents["pi.usage"]["aggregate"]["total_tokens"], 3);
        assert_eq!(documents["pi.inbox"]["pending"], serde_json::json!([]));
        assert_eq!(documents["pi.live"]["status"], "done");
        assert_eq!(
            documents["pi.agent"]["model"]["behavior"]["id"],
            "http-model"
        );
        let requests = server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 1);
        let body: serde_json::Value = serde_json::from_slice(&requests[0].body).unwrap();
        assert_eq!(body["model"], "http-model");
        assert_eq!(body["options"]["maxTokens"], 333);
        let provider_session = documents["pi.provider"]["sessionId"].as_str().unwrap();
        assert_eq!(provider_session.len(), 36);
        assert_eq!(body["options"]["sessionId"], provider_session);
        assert_eq!(
            body["context"]["messages"][0]["content"][0]["text"],
            "hello"
        );
        harness.close().await.unwrap();
    }

    #[tokio::test]
    async fn supplied_answer_duration_survives_journal_reopen_without_dispatch() {
        struct Timed;
        impl DurableModelRunner for Timed {
            fn run<'a>(&'a self, _: ModelIntent) -> crate::durable::model::ModelFuture<'a> {
                Box::pin(async {
                    let mut run = ModelRun::one(ModelTerminal::Answer {
                        content: vec![],
                        text: "timed".into(),
                        usage: DurableUsage::default(),
                        response_id: None,
                        stop_reason: "stop".into(),
                    });
                    run.duration_ms = Some(1234);
                    run
                })
            }
        }
        struct Never;
        impl DurableModelRunner for Never {
            fn run<'a>(&'a self, _: ModelIntent) -> crate::durable::model::ModelFuture<'a> {
                panic!("context read must not dispatch")
            }
        }
        let root = std::env::temp_dir().join(format!("rs-ai-duration-{}", crate::utils::uuidv7()));
        std::fs::create_dir(&root).unwrap();
        let path = root.join("journal");
        let pinned =
            PinnedModel::from_model(&model("http://unused".into(), "timed", "test")).unwrap();
        let harness = DurableHarness::open(
            Box::new(JournalStorage::open(&path).unwrap()),
            Arc::new(Timed),
            pinned.clone(),
            PinnedOptions::default(),
        )
        .await
        .unwrap();
        let handle = harness
            .submit(SubmitRequest {
                request_id: "timed".into(),
                content: "go".into(),
            })
            .await
            .unwrap();
        harness.wait(handle).await.unwrap();
        assert_eq!(
            harness
                .message_context(ContextOptions::default())
                .await
                .unwrap()
                .last()
                .unwrap()
                .duration_ms,
            Some(1234)
        );
        harness.close().await.unwrap();
        let reopened = DurableHarness::open(
            Box::new(JournalStorage::open(&path).unwrap()),
            Arc::new(Never),
            pinned,
            PinnedOptions::default(),
        )
        .await
        .unwrap();
        assert_eq!(
            reopened
                .message_context(ContextOptions::default())
                .await
                .unwrap()
                .last()
                .unwrap()
                .duration_ms,
            Some(1234)
        );
        reopened.close().await.unwrap();
        std::fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn provider_session_is_stable_across_turns_and_journal_reopen() {
        struct Capture(std::sync::Mutex<Vec<String>>);
        impl DurableModelRunner for Capture {
            fn run<'a>(&'a self, intent: ModelIntent) -> crate::durable::model::ModelFuture<'a> {
                self.0
                    .lock()
                    .unwrap()
                    .push(intent.provider_session_id.expect("persisted identity"));
                Box::pin(async {
                    ModelRun::one(ModelTerminal::Answer {
                        content: vec![],
                        text: "ok".into(),
                        usage: DurableUsage::default(),
                        response_id: None,
                        stop_reason: "stop".into(),
                    })
                })
            }
        }
        let root = std::env::temp_dir().join(format!("rs-ai-provider-{}", crate::utils::uuidv7()));
        std::fs::create_dir(&root).unwrap();
        let path = root.join("journal");
        let runner = Arc::new(Capture(std::sync::Mutex::new(Vec::new())));
        let runtime = model(
            "http://unused".into(),
            "provider-session",
            "durable-provider-session",
        );
        let harness = DurableHarness::open(
            Box::new(JournalStorage::open(&path).unwrap()),
            runner.clone(),
            PinnedModel::from_model(&runtime).unwrap(),
            PinnedOptions::default(),
        )
        .await
        .unwrap();
        for number in 0..2 {
            let handle = harness
                .submit(SubmitRequest {
                    request_id: format!("turn-{number}"),
                    content: "hello".into(),
                })
                .await
                .unwrap();
            harness.wait(handle).await.unwrap();
        }
        let identity = harness.inspect_documents().await.unwrap()["pi.provider"]["sessionId"]
            .as_str()
            .unwrap()
            .to_owned();
        harness.close().await.unwrap();
        {
            let storage = JournalStorage::open(&path).unwrap();
            let claim = storage.claim_writer().unwrap();
            let snapshot = storage.load(&claim).await.unwrap();
            assert!(
                snapshot
                    .tasks
                    .values()
                    .all(|task| task.started_at.is_some() && task.ended_at.is_some())
            );
            for task in snapshot.tasks.values() {
                let mut value = serde_json::to_value(task).unwrap();
                assert!(value.get("startedAt").is_some());
                assert!(value.get("endedAt").is_some());
                value.as_object_mut().unwrap().remove("startedAt");
                value.as_object_mut().unwrap().remove("endedAt");
                let legacy: TaskRecord = serde_json::from_value(value).unwrap();
                assert!(legacy.started_at.is_none() && legacy.ended_at.is_none());
            }
            storage.close(&claim).await.unwrap();
        }
        let reopened = DurableHarness::open(
            Box::new(JournalStorage::open(&path).unwrap()),
            runner.clone(),
            PinnedModel::from_model(&runtime).unwrap(),
            PinnedOptions::default(),
        )
        .await
        .unwrap();
        assert_eq!(
            runner.0.lock().unwrap().len(),
            2,
            "open performs no provider effect"
        );
        assert_eq!(
            reopened.inspect_documents().await.unwrap()["pi.provider"]["sessionId"],
            identity
        );
        let handle = reopened
            .submit(SubmitRequest {
                request_id: "after-reopen".into(),
                content: "hello".into(),
            })
            .await
            .unwrap();
        reopened.wait(handle).await.unwrap();
        reopened.close().await.unwrap();
        assert_eq!(
            &*runner.0.lock().unwrap(),
            &[identity.clone(), identity.clone(), identity.clone()]
        );
        let other = DurableHarness::open(
            Box::new(MemoryStorage::new()),
            runner.clone(),
            PinnedModel::from_model(&runtime).unwrap(),
            PinnedOptions::default(),
        )
        .await
        .unwrap();
        let handle = other
            .submit(SubmitRequest {
                request_id: "other".into(),
                content: "hello".into(),
            })
            .await
            .unwrap();
        other.wait(handle).await.unwrap();
        let other_identity = other.inspect_documents().await.unwrap()["pi.provider"]["sessionId"]
            .as_str()
            .unwrap()
            .to_owned();
        assert_ne!(identity, other_identity);
        other.close().await.unwrap();
        std::fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn second_idle_submission_refreshes_live_before_provider_settles() {
        let runtime = model("http://unused".into(), "live-model", "durable-live");
        struct BarrierSecond {
            calls: std::sync::atomic::AtomicUsize,
            release: Arc<tokio::sync::Notify>,
        }
        impl DurableModelRunner for BarrierSecond {
            fn run<'a>(&'a self, _intent: ModelIntent) -> crate::durable::model::ModelFuture<'a> {
                let call = self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Box::pin(async move {
                    if call == 1 {
                        self.release.notified().await;
                    }
                    ModelRun::one(ModelTerminal::Answer {
                        content: vec![],
                        text: "ok".into(),
                        usage: DurableUsage::default(),
                        response_id: None,
                        stop_reason: "stop".into(),
                    })
                })
            }
        }
        let release = Arc::new(tokio::sync::Notify::new());
        let path = std::env::temp_dir().join(format!("rs-ai-r1b-live-{}", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let harness = DurableHarness::open(
            Box::new(JournalStorage::open(&path).unwrap()),
            Arc::new(BarrierSecond {
                calls: std::sync::atomic::AtomicUsize::new(0),
                release: release.clone(),
            }),
            PinnedModel::from_model(&runtime).unwrap(),
            PinnedOptions::default(),
        )
        .await
        .unwrap();
        let first = harness
            .submit(SubmitRequest {
                request_id: "idle-1".into(),
                content: "one".into(),
            })
            .await
            .unwrap();
        harness.wait(first).await.unwrap();
        let second = harness
            .submit(SubmitRequest {
                request_id: "idle-2".into(),
                content: "two".into(),
            })
            .await
            .unwrap();
        let documents = harness.inspect_documents().await.unwrap();
        assert_eq!(documents["pi.live"]["submission_id"], second.id.get());
        assert_eq!(documents["pi.live"]["status"], "pending");
        release.notify_one();
        harness.wait(second.clone()).await.unwrap();
        harness.close().await.unwrap();
        struct NeverRunner;
        impl DurableModelRunner for NeverRunner {
            fn run<'a>(&'a self, _intent: ModelIntent) -> crate::durable::model::ModelFuture<'a> {
                panic!("terminal reopen must not dispatch")
            }
        }
        let reopened = DurableHarness::open(
            Box::new(JournalStorage::open(&path).unwrap()),
            Arc::new(NeverRunner),
            PinnedModel::from_model(&runtime).unwrap(),
            PinnedOptions::default(),
        )
        .await
        .unwrap();
        let documents = reopened.inspect_documents().await.unwrap();
        assert_eq!(documents["pi.live"]["submission_id"], second.id.get());
        assert_eq!(documents["pi.live"]["status"], "done");
        assert_eq!(reopened.inspect(second.id).await.unwrap().status, "done");
        reopened.close().await.unwrap();
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn pinned_behavior_survives_registry_behavior_change() {
        let server = MockServer::start().await;
        let original = model(format!("{}/v1", server.uri()), "pin-model", "durable-pin");
        let pinned = PinnedModel::from_model(&original).unwrap();
        let mut changed = original.clone();
        changed.max_tokens = 7;
        changed.name = "Changed later".into();
        crate::registry::register_model(changed.clone());
        assert_eq!(pinned.dispatch_model(&changed).unwrap().max_tokens, 512);
        Mock::given(method("POST")).and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(200).insert_header("content-type", "text/event-stream").set_body_string(
                "data: {\"type\":\"done\",\"reason\":\"stop\",\"usage\":{\"input\":0,\"output\":0,\"cacheRead\":0,\"cacheWrite\":0,\"totalTokens\":0,\"cost\":{\"input\":0.0,\"output\":0.0,\"cacheRead\":0.0,\"cacheWrite\":0.0,\"total\":0.0}}}\n\n"
            )).expect(1).mount(&server).await;
        let harness = DurableHarness::open(
            Box::new(MemoryStorage::new()),
            Arc::new(RegistryModelRunner),
            pinned,
            PinnedOptions::default(),
        )
        .await
        .unwrap();
        let handle = harness
            .submit(SubmitRequest {
                request_id: "pin".into(),
                content: "hello".into(),
            })
            .await
            .unwrap();
        assert_eq!(harness.wait(handle).await.unwrap().status, "done");
        let requests = server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 1);
        harness.close().await.unwrap();
    }

    #[tokio::test]
    async fn tool_terminal_is_typed_unsupported_and_never_faked() {
        struct ToolRunner;
        impl DurableModelRunner for ToolRunner {
            fn run<'a>(&'a self, _intent: ModelIntent) -> crate::durable::model::ModelFuture<'a> {
                Box::pin(async { ModelRun::one(ModelTerminal::UnsupportedToolCall) })
            }
        }
        let runtime = model("http://unused".into(), "tool-model", "durable-tool");
        let harness = DurableHarness::open(
            Box::new(MemoryStorage::new()),
            Arc::new(ToolRunner),
            PinnedModel::from_model(&runtime).unwrap(),
            PinnedOptions::default(),
        )
        .await
        .unwrap();
        let handle = harness
            .submit(SubmitRequest {
                request_id: "tool".into(),
                content: "call".into(),
            })
            .await
            .unwrap();
        let result = harness.wait(handle).await.unwrap();
        assert_eq!(
            result.reason.unwrap()["code"],
            "durable_tool_unsupported_r1b"
        );
        harness.close().await.unwrap();
    }
}
