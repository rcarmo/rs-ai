#[cfg(test)]
mod tests {
    use crate::durable::*;
    use crate::types::{Model, ModelCompat, ModelCost, Tool};
    use serde_json::json;
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    };
    #[cfg(unix)]
    use std::{
        fs::OpenOptions,
        io::Write,
        path::{Path, PathBuf},
        process::{Command, Stdio},
        time::Duration,
    };

    fn model() -> PinnedModel {
        PinnedModel::from_model(&Model {
            id: "m".into(),
            name: "M".into(),
            api: "test".into(),
            provider: "p".into(),
            base_url: "http://unused".into(),
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
            api_key: None,
            compat: ModelCompat::default(),
        })
        .unwrap()
    }
    fn definition() -> Tool {
        Tool {
            name: "work".into(),
            description: "Work".into(),
            parameters: json!({"type":"object","properties":{},"additionalProperties":false}),
            constrained_sampling: None,
        }
    }
    fn call() -> DurableToolCall {
        DurableToolCall {
            provider_call_id: "provider-1".into(),
            name: "work".into(),
            original_arguments: json!({}),
        }
    }
    fn assistant() -> serde_json::Value {
        json!({"role":"assistant","content":[{"type":"toolCall","id":"provider-1","name":"work","arguments":{}}],"timestamp":0,"diagnostics":[],"isError":false,"addedToolNames":[],"toolsAdded":[],"toolsRemoved":[]})
    }

    struct Counter {
        count: Arc<AtomicUsize>,
        keys: Arc<Mutex<Vec<String>>>,
    }
    impl DurableTool for Counter {
        fn execute<'a>(&'a self, e: ToolExecution) -> crate::durable::tool::ToolFuture<'a> {
            self.count.fetch_add(1, Ordering::SeqCst);
            self.keys.lock().unwrap().push(e.durable_idempotency_key);
            Box::pin(async {
                Ok(ToolOutput {
                    value: json!({"ok":true}),
                    usage: None,
                })
            })
        }
    }
    struct FirstTool;
    impl DurableModelRunner for FirstTool {
        fn run<'a>(&'a self, _: ModelIntent) -> crate::durable::model::ModelFuture<'a> {
            Box::pin(async {
                ModelRun::one(ModelTerminal::ToolCalls {
                    calls: vec![call()],
                    usage: DurableUsage::default(),
                    assistant: assistant(),
                })
            })
        }
    }
    struct FinalAnswer(Arc<AtomicUsize>);
    impl DurableModelRunner for FinalAnswer {
        fn run<'a>(&'a self, intent: ModelIntent) -> crate::durable::model::ModelFuture<'a> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Box::pin(async move {
                assert!(
                    intent
                        .native_messages
                        .unwrap()
                        .iter()
                        .any(|m| m.role == crate::types::Role::ToolResult)
                );
                ModelRun::one(ModelTerminal::Answer {
                    content: vec![],
                    text: "final".into(),
                    usage: DurableUsage::default(),
                    response_id: None,
                    stop_reason: "stop".into(),
                })
            })
        }
    }

    async fn seed_pending_tool(
        path: &std::path::Path,
        registry: Arc<DurableToolRegistry>,
    ) -> SubmissionHandle {
        let harness = Arc::new(
            DurableHarness::open_with_tools(
                Box::new(JournalStorage::open(path).unwrap()),
                Arc::new(FirstTool),
                model(),
                PinnedOptions::default(),
                registry,
            )
            .await
            .unwrap(),
        );
        let (selected, release) = harness.inject_tool_phase_barrier().await;
        let handle = harness
            .submit(SubmitRequest {
                request_id: "recover".into(),
                content: "run".into(),
            })
            .await
            .unwrap();
        selected.await.unwrap();
        let closer = harness.clone();
        let close = tokio::spawn(async move { closer.close().await });
        while !harness.is_sealed() {
            tokio::task::yield_now().await;
        }
        release.send(()).unwrap();
        close.await.unwrap().unwrap();
        handle
    }

    #[tokio::test]
    async fn recovered_tool_receives_current_host_models_without_serializing_service() {
        struct UsesCurrentModels(Arc<dyn DurableModels>, Arc<AtomicUsize>);
        impl DurableTool for UsesCurrentModels {
            fn execute<'a>(
                &'a self,
                execution: ToolExecution,
            ) -> crate::durable::tool::ToolFuture<'a> {
                assert!(Arc::ptr_eq(&self.0, &execution.models));
                assert!(
                    execution
                        .models
                        .get_model("openai", "gpt-4o-mini")
                        .is_some()
                );
                self.1.fetch_add(1, Ordering::SeqCst);
                Box::pin(async {
                    Ok(ToolOutput {
                        value: json!({"ok":true}),
                        usage: None,
                    })
                })
            }
        }
        let path =
            std::env::temp_dir().join(format!("rs-ai-r1c-host-models-{}", std::process::id()));
        assert!(!path.exists());
        let host: Arc<dyn DurableModels> = Arc::new(RegistryModels);
        let count = Arc::new(AtomicUsize::new(0));
        let registry = Arc::new(DurableToolRegistry::default());
        registry
            .register(
                definition(),
                "impl",
                "1",
                ReplayPolicy::Safe,
                Arc::new(UsesCurrentModels(host.clone(), count.clone())),
            )
            .unwrap();
        let handle = seed_pending_tool(&path, registry.clone()).await;
        assert_eq!(count.load(Ordering::SeqCst), 0);
        let reopened = DurableHarness::open_with_tool_models(
            Box::new(JournalStorage::open(&path).unwrap()),
            Arc::new(FinalAnswer(Arc::new(AtomicUsize::new(0)))),
            model(),
            PinnedOptions::default(),
            registry,
            Arc::new(crate::utils::now_millis),
            host,
        )
        .await
        .unwrap();
        assert_eq!(
            count.load(Ordering::SeqCst),
            0,
            "opening never dispatches recovered effects"
        );
        let context_before = reopened
            .message_context(ContextOptions::default())
            .await
            .unwrap();
        assert_eq!(count.load(Ordering::SeqCst), 0);
        assert!(
            context_before
                .iter()
                .any(|message| message.role == crate::types::Role::ToolResult && message.is_error)
        );
        reopened.resume(handle.clone()).await.unwrap();
        assert_eq!(
            reopened.wait(handle).await.unwrap().answer.as_deref(),
            Some("final")
        );
        let context_after = reopened
            .message_context(ContextOptions::default())
            .await
            .unwrap();
        assert_eq!(
            context_after
                .iter()
                .filter(|message| message.role == crate::types::Role::ToolResult)
                .count(),
            1
        );
        assert!(
            !context_after
                .iter()
                .find(|message| message.role == crate::types::Role::ToolResult)
                .unwrap()
                .is_error
        );
        assert_eq!(count.load(Ordering::SeqCst), 1);
        reopened.close().await.unwrap();
        std::fs::remove_file(path).unwrap();
    }

    #[tokio::test]
    async fn unresumed_recovered_abort_settles_without_model_or_tool_effect() {
        let path =
            std::env::temp_dir().join(format!("rs-ai-r1c-unresumed-abort-{}", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let count = Arc::new(AtomicUsize::new(0));
        let keys = Arc::new(Mutex::new(vec![]));
        let registry = Arc::new(DurableToolRegistry::default());
        registry
            .register(
                definition(),
                "impl",
                "1",
                ReplayPolicy::Safe,
                Arc::new(Counter {
                    count: count.clone(),
                    keys,
                }),
            )
            .unwrap();
        let handle = seed_pending_tool(&path, registry.clone()).await;
        let model_calls = Arc::new(AtomicUsize::new(0));
        let reopened = DurableHarness::open_with_tools(
            Box::new(JournalStorage::open(&path).unwrap()),
            Arc::new(FinalAnswer(model_calls.clone())),
            model(),
            PinnedOptions::default(),
            registry,
        )
        .await
        .unwrap();
        reopened.abort(handle.clone()).await.unwrap();
        assert_eq!(
            reopened.wait(handle.clone()).await.unwrap().status,
            "aborted"
        );
        assert_eq!(count.load(Ordering::SeqCst), 0);
        assert_eq!(model_calls.load(Ordering::SeqCst), 0);
        reopened.close().await.unwrap();
        drop(reopened);
        let verify = DurableHarness::open(
            Box::new(JournalStorage::open(&path).unwrap()),
            Arc::new(FinalAnswer(model_calls.clone())),
            model(),
            PinnedOptions::default(),
        )
        .await
        .unwrap();
        assert_eq!(verify.inspect(handle.id).await.unwrap().status, "aborted");
        verify.close().await.unwrap();
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn safe_exact_version_replays_after_reopen_once_with_stable_key() {
        let path = std::env::temp_dir().join(format!("rs-ai-r1c-safe-{}", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let count = Arc::new(AtomicUsize::new(0));
        let keys = Arc::new(Mutex::new(vec![]));
        let registry = Arc::new(DurableToolRegistry::default());
        registry
            .register(
                definition(),
                "impl",
                "1",
                ReplayPolicy::Safe,
                Arc::new(Counter {
                    count: count.clone(),
                    keys: keys.clone(),
                }),
            )
            .unwrap();
        let handle = seed_pending_tool(&path, registry.clone()).await;
        assert_eq!(count.load(Ordering::SeqCst), 0);
        let final_calls = Arc::new(AtomicUsize::new(0));
        let reopened = Arc::new(
            DurableHarness::open_with_tools(
                Box::new(JournalStorage::open(&path).unwrap()),
                Arc::new(FinalAnswer(final_calls.clone())),
                model(),
                PinnedOptions::default(),
                registry,
            )
            .await
            .unwrap(),
        );
        let one = reopened.clone();
        let h1 = handle.clone();
        let resume1 = tokio::spawn(async move { one.resume(h1).await });
        let two = reopened.clone();
        let h2 = handle.clone();
        let resume2 = tokio::spawn(async move { two.resume(h2).await });
        let waiter = reopened.clone();
        let hw = handle.clone();
        let dropped_wait = tokio::spawn(async move { waiter.wait(hw).await });
        dropped_wait.abort();
        let _ = dropped_wait.await;
        resume1.await.unwrap().unwrap();
        resume2.await.unwrap().unwrap();
        assert_eq!(
            reopened.wait(handle).await.unwrap().answer.as_deref(),
            Some("final")
        );
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(final_calls.load(Ordering::SeqCst), 1);
        let key = keys.lock().unwrap()[0].clone();
        assert!(key.contains("tool-"));
        let snapshot = reopened.test_snapshot().await.unwrap();
        let persisted = snapshot
            .tasks
            .values()
            .find(|task| task.kind == "tool")
            .unwrap();
        let intent: ToolIntent = serde_json::from_value(persisted.input.clone()).unwrap();
        assert_eq!(intent.durable_idempotency_key, key);
        reopened.close().await.unwrap();
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn committed_successor_checkpoint_reopens_without_child_list_reconstruction() {
        let path = std::env::temp_dir().join(format!("rs-ai-r1c-successor-{}", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let registry = Arc::new(DurableToolRegistry::default());
        registry
            .register(
                definition(),
                "impl",
                "1",
                ReplayPolicy::Safe,
                Arc::new(Counter {
                    count: Arc::new(AtomicUsize::new(0)),
                    keys: Arc::new(Mutex::new(vec![])),
                }),
            )
            .unwrap();
        let harness = Arc::new(
            DurableHarness::open_with_tools(
                Box::new(JournalStorage::open(&path).unwrap()),
                Arc::new(FirstTool),
                model(),
                PinnedOptions::default(),
                registry.clone(),
            )
            .await
            .unwrap(),
        );
        let (selected, release) = harness.inject_successor_phase_barrier().await;
        let handle = harness
            .submit(SubmitRequest {
                request_id: "successor".into(),
                content: "go".into(),
            })
            .await
            .unwrap();
        selected.await.unwrap();
        let closer = harness.clone();
        let close = tokio::spawn(async move { closer.close().await });
        while !harness.is_sealed() {
            tokio::task::yield_now().await;
        }
        release.send(()).unwrap();
        close.await.unwrap().unwrap();
        let final_calls = Arc::new(AtomicUsize::new(0));
        let reopened = DurableHarness::open_with_tools(
            Box::new(JournalStorage::open(&path).unwrap()),
            Arc::new(FinalAnswer(final_calls.clone())),
            model(),
            PinnedOptions::default(),
            registry,
        )
        .await
        .unwrap();
        reopened.resume(handle.clone()).await.unwrap();
        assert_eq!(
            reopened.wait(handle).await.unwrap().answer.as_deref(),
            Some("final")
        );
        assert_eq!(final_calls.load(Ordering::SeqCst), 1);
        reopened.close().await.unwrap();
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn repeated_round_recovery_does_not_duplicate_prior_tool_results() {
        let calls = Arc::new(Mutex::new(Vec::<Vec<String>>::new()));
        struct Rounds {
            round: Mutex<u8>,
            seen: Arc<Mutex<Vec<Vec<String>>>>,
        }
        impl DurableModelRunner for Rounds {
            fn run<'a>(&'a self, intent: ModelIntent) -> crate::durable::model::ModelFuture<'a> {
                Box::pin(async move {
                    let results = intent
                        .native_messages
                        .as_ref()
                        .map(|messages| {
                            messages
                                .iter()
                                .filter(|m| m.role == crate::types::Role::ToolResult)
                                .filter_map(|m| m.tool_call_id.clone())
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default();
                    self.seen.lock().unwrap().push(results);
                    let mut round = self.round.lock().unwrap();
                    *round += 1;
                    match *round {
                        1 => ModelRun::one(ModelTerminal::ToolCalls {
                            calls: vec![DurableToolCall {
                                provider_call_id: "call1".into(),
                                name: "work".into(),
                                original_arguments: json!({}),
                            }],
                            usage: DurableUsage::default(),
                            assistant: json!({"role":"assistant","content":[{"type":"toolCall","id":"call1","name":"work","arguments":{}}],"timestamp":0,"diagnostics":[],"isError":false,"addedToolNames":[],"toolsAdded":[],"toolsRemoved":[]}),
                        }),
                        2 => ModelRun::one(ModelTerminal::ToolCalls {
                            calls: vec![DurableToolCall {
                                provider_call_id: "call2".into(),
                                name: "work".into(),
                                original_arguments: json!({}),
                            }],
                            usage: DurableUsage::default(),
                            assistant: json!({"role":"assistant","content":[{"type":"toolCall","id":"call2","name":"work","arguments":{}}],"timestamp":0,"diagnostics":[],"isError":false,"addedToolNames":[],"toolsAdded":[],"toolsRemoved":[]}),
                        }),
                        _ => ModelRun::one(ModelTerminal::Answer {
                            content: vec![],
                            text: "final".into(),
                            usage: DurableUsage::default(),
                            response_id: None,
                            stop_reason: "stop".into(),
                        }),
                    }
                })
            }
        }
        let path = std::env::temp_dir().join(format!("rs-ai-r1c-rounds-{}", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let registry = Arc::new(DurableToolRegistry::default());
        registry
            .register(
                definition(),
                "impl",
                "1",
                ReplayPolicy::Safe,
                Arc::new(Counter {
                    count: Arc::new(AtomicUsize::new(0)),
                    keys: Arc::new(Mutex::new(vec![])),
                }),
            )
            .unwrap();
        let harness = DurableHarness::open_with_tools(
            Box::new(JournalStorage::open(&path).unwrap()),
            Arc::new(Rounds {
                round: Mutex::new(0),
                seen: calls.clone(),
            }),
            model(),
            PinnedOptions::default(),
            registry.clone(),
        )
        .await
        .unwrap();
        let handle = harness
            .submit(SubmitRequest {
                request_id: "rounds".into(),
                content: "go".into(),
            })
            .await
            .unwrap();
        assert_eq!(
            harness.wait(handle).await.unwrap().answer.as_deref(),
            Some("final")
        );
        {
            let seen = calls.lock().unwrap();
            assert_eq!(seen[1], vec!["call1"]);
            assert_eq!(seen[2], vec!["call1", "call2"]);
        }
        harness.close().await.unwrap();
        let _ = std::fs::remove_file(path);
    }

    #[cfg(unix)]
    struct FileCounter {
        effects: PathBuf,
        ready: Option<PathBuf>,
        block: bool,
    }
    #[cfg(unix)]
    impl DurableTool for FileCounter {
        fn execute<'a>(&'a self, e: ToolExecution) -> crate::durable::tool::ToolFuture<'a> {
            Box::pin(async move {
                let mut f = OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&self.effects)
                    .unwrap();
                writeln!(f, "{}", e.durable_idempotency_key).unwrap();
                f.sync_all().unwrap();
                if let Some(ready) = &self.ready {
                    std::fs::write(ready, b"ready").unwrap();
                }
                if self.block {
                    std::future::pending::<()>().await;
                }
                Ok(ToolOutput {
                    value: json!({"ok":true}),
                    usage: None,
                })
            })
        }
    }

    #[cfg(unix)]
    fn wait_ready(path: &Path) {
        for _ in 0..500 {
            if path.exists() {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("child phase barrier not reached");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn sigkill_phase_child() {
        let Ok(phase) = std::env::var("RS_AI_R1C_SIGKILL_PHASE") else {
            return;
        };
        let journal = PathBuf::from(std::env::var("RS_AI_R1C_JOURNAL").unwrap());
        let ready = PathBuf::from(std::env::var("RS_AI_R1C_READY").unwrap());
        let effects = PathBuf::from(std::env::var("RS_AI_R1C_EFFECTS").unwrap());
        let registry = Arc::new(DurableToolRegistry::default());
        registry
            .register(
                definition(),
                "impl",
                "1",
                ReplayPolicy::Safe,
                Arc::new(FileCounter {
                    effects,
                    ready: if phase == "effect" {
                        Some(ready.clone())
                    } else {
                        None
                    },
                    block: phase == "effect",
                }),
            )
            .unwrap();
        let harness = Arc::new(
            DurableHarness::open_with_tools(
                Box::new(JournalStorage::open(journal).unwrap()),
                Arc::new(FirstTool),
                model(),
                PinnedOptions::default(),
                registry,
            )
            .await
            .unwrap(),
        );
        let barrier = match phase.as_str() {
            "intent" => Some(harness.inject_tool_phase_barrier().await),
            "successor" => Some(harness.inject_successor_phase_barrier().await),
            _ => None,
        };
        let worker = harness.clone();
        tokio::spawn(async move {
            let _ = worker
                .submit(SubmitRequest {
                    request_id: "sigkill".into(),
                    content: "go".into(),
                })
                .await;
        });
        if let Some((selected, _release)) = barrier {
            selected.await.unwrap();
            std::fs::write(ready, b"ready").unwrap();
            std::future::pending::<()>().await;
        } else {
            std::future::pending::<()>().await;
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn actual_sigkill_intent_effect_successor_reopen_with_stable_keys() {
        for phase in ["intent", "effect", "successor"] {
            let dir =
                std::env::temp_dir().join(format!("rs-ai-r1c-kill-{phase}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            let journal = dir.join("state.durable");
            let ready = dir.join("ready");
            let effects = dir.join("effects");
            let mut child = Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "durable_tool_recovery_test::tests::sigkill_phase_child",
                    "--nocapture",
                    "--test-threads=1",
                ])
                .env("RS_AI_R1C_SIGKILL_PHASE", phase)
                .env("RS_AI_R1C_JOURNAL", &journal)
                .env("RS_AI_R1C_READY", &ready)
                .env("RS_AI_R1C_EFFECTS", &effects)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap();
            wait_ready(&ready);
            child.kill().unwrap();
            let status = child.wait().unwrap();
            assert!(!status.success());
            let calls = Arc::new(AtomicUsize::new(0));
            let registry = Arc::new(DurableToolRegistry::default());
            registry
                .register(
                    definition(),
                    "impl",
                    "1",
                    ReplayPolicy::Safe,
                    Arc::new(FileCounter {
                        effects: effects.clone(),
                        ready: None,
                        block: false,
                    }),
                )
                .unwrap();
            let reopened = DurableHarness::open_with_tools(
                Box::new(JournalStorage::open(&journal).unwrap()),
                Arc::new(FinalAnswer(calls.clone())),
                model(),
                PinnedOptions::default(),
                registry,
            )
            .await
            .unwrap();
            let handle = SubmissionHandle {
                id: SubmissionId::new(3).unwrap(),
                task_id: TaskId::new(2).unwrap(),
            };
            reopened.resume(handle.clone()).await.unwrap();
            assert_eq!(
                reopened.wait(handle).await.unwrap().answer.as_deref(),
                Some("final")
            );
            reopened.close().await.unwrap();
            let keys = std::fs::read_to_string(&effects)
                .unwrap_or_default()
                .lines()
                .map(str::to_string)
                .collect::<Vec<_>>();
            let expected = if phase == "effect" { 2 } else { 1 };
            assert_eq!(keys.len(), expected, "{phase}");
            assert!(keys.windows(2).all(|pair| pair[0] == pair[1]), "{phase}");
            let _ = std::fs::remove_dir_all(dir);
        }
    }

    #[tokio::test]
    async fn stored_unsafe_recovery_settles_without_effect() {
        let path = std::env::temp_dir().join(format!("rs-ai-r1c-unsafe-{}", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let count = Arc::new(AtomicUsize::new(0));
        let registry = Arc::new(DurableToolRegistry::default());
        registry
            .register(
                definition(),
                "impl",
                "1",
                ReplayPolicy::Unsafe,
                Arc::new(Counter {
                    count: count.clone(),
                    keys: Arc::new(Mutex::new(vec![])),
                }),
            )
            .unwrap();
        let handle = seed_pending_tool(&path, registry.clone()).await;
        let reopened = DurableHarness::open_with_tools(
            Box::new(JournalStorage::open(&path).unwrap()),
            Arc::new(FinalAnswer(Arc::new(AtomicUsize::new(0)))),
            model(),
            PinnedOptions::default(),
            registry,
        )
        .await
        .unwrap();
        reopened.resume(handle.clone()).await.unwrap();
        assert_eq!(
            reopened.wait(handle).await.unwrap().answer.as_deref(),
            Some("final")
        );
        assert_eq!(count.load(Ordering::SeqCst), 0);
        reopened.close().await.unwrap();
        let _ = std::fs::remove_file(path);
    }
}
