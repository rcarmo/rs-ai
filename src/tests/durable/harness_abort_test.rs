#[cfg(test)]
mod tests {
    use crate::durable::storage::{StorageFuture, WriterClaim};
    use crate::durable::*;
    use crate::types::{Model, ModelCompat, ModelCost, Tool};
    use serde_json::json;
    use std::sync::{Arc, Mutex};
    use tokio::sync::{Notify, oneshot};

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
            name: "block".into(),
            description: "Block".into(),
            parameters: json!({"type":"object","properties":{},"additionalProperties":false}),
            constrained_sampling: None,
        }
    }
    struct ToolModel;
    impl DurableModelRunner for ToolModel {
        fn run<'a>(&'a self, _: ModelIntent) -> crate::durable::model::ModelFuture<'a> {
            Box::pin(async {
                ModelRun::one(ModelTerminal::ToolCalls {
                    calls: vec![DurableToolCall {
                        provider_call_id: "c".into(),
                        name: "block".into(),
                        original_arguments: json!({}),
                    }],
                    usage: DurableUsage::default(),
                    assistant: json!({"role":"assistant","content":[{"type":"toolCall","id":"c","name":"block","arguments":{}}],"timestamp":0,"diagnostics":[],"isError":false,"addedToolNames":[],"toolsAdded":[],"toolsRemoved":[]}),
                })
            })
        }
    }
    struct Blocking {
        started: Mutex<Option<oneshot::Sender<()>>>,
    }
    impl DurableTool for Blocking {
        fn execute<'a>(&'a self, e: ToolExecution) -> crate::durable::tool::ToolFuture<'a> {
            Box::pin(async move {
                if let Some(tx) = self.started.lock().unwrap().take() {
                    let _ = tx.send(());
                }
                let mut cancel = e.cancel.clone();
                while !*cancel.borrow() {
                    if cancel.changed().await.is_err() {
                        break;
                    }
                }
                Err(ToolFailure {
                    code: "tool_aborted".into(),
                    usage: None,
                })
            })
        }
    }

    struct AbortAckStorage {
        inner: MemoryStorage,
        claim: WriterClaim,
        commits: std::sync::atomic::AtomicUsize,
        reached: Arc<Notify>,
        release: Arc<Notify>,
        acked: Arc<Notify>,
        target: Option<usize>,
        abort_batch: bool,
        intercepted: std::sync::atomic::AtomicBool,
    }
    impl AbortAckStorage {
        fn new(target: usize) -> (Self, Arc<Notify>, Arc<Notify>, Arc<Notify>) {
            Self::configured(Some(target), false)
        }
        fn abort_batch() -> (Self, Arc<Notify>, Arc<Notify>, Arc<Notify>) {
            Self::configured(None, true)
        }
        fn configured(
            target: Option<usize>,
            abort_batch: bool,
        ) -> (Self, Arc<Notify>, Arc<Notify>, Arc<Notify>) {
            let inner = MemoryStorage::new();
            let claim = inner.claim_writer().unwrap();
            let reached = Arc::new(Notify::new());
            let release = Arc::new(Notify::new());
            let acked = Arc::new(Notify::new());
            (
                Self {
                    inner,
                    claim,
                    commits: std::sync::atomic::AtomicUsize::new(0),
                    reached: reached.clone(),
                    release: release.clone(),
                    acked: acked.clone(),
                    target,
                    abort_batch,
                    intercepted: std::sync::atomic::AtomicBool::new(false),
                },
                reached,
                release,
                acked,
            )
        }
    }
    impl DurableStorage for AbortAckStorage {
        fn claim_writer(&self) -> Result<WriterClaim, DurableError> {
            Ok(WriterClaim::fresh())
        }
        fn load<'a>(&'a self, _: &'a WriterClaim) -> StorageFuture<'a, StorageSnapshot> {
            self.inner.load(&self.claim)
        }
        fn commit<'a>(&'a self, _: &'a WriterClaim, b: CommitBatch) -> StorageFuture<'a, ()> {
            Box::pin(async move {
                let n = self
                    .commits
                    .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let matches = self.target == Some(n)
                    || (self.abort_batch && b.tasks.iter().any(|task| task.abort_requested));
                let intercept = matches
                    && !self
                        .intercepted
                        .swap(true, std::sync::atomic::Ordering::SeqCst);
                if intercept {
                    self.inner.commit(&self.claim, b).await?;
                    self.reached.notify_one();
                    self.release.notified().await;
                    self.acked.notify_one();
                    Ok(())
                } else {
                    self.inner.commit(&self.claim, b).await
                }
            })
        }
        fn close<'a>(&'a self, _: &'a WriterClaim) -> StorageFuture<'a, ()> {
            self.inner.close(&self.claim)
        }
    }

    #[tokio::test]
    async fn dropped_abort_caller_still_signals_and_settles_bottom_up() {
        let (started_tx, started_rx) = oneshot::channel();
        let registry = Arc::new(DurableToolRegistry::default());
        registry
            .register(
                definition(),
                "block",
                "1",
                ReplayPolicy::Safe,
                Arc::new(Blocking {
                    started: Mutex::new(Some(started_tx)),
                }),
            )
            .unwrap();
        let (storage, reached, release, _acked) = AbortAckStorage::new(4);
        let harness = Arc::new(
            DurableHarness::open_with_tools(
                Box::new(storage),
                Arc::new(ToolModel),
                model(),
                PinnedOptions::default(),
                registry,
            )
            .await
            .unwrap(),
        );
        let handle = harness
            .submit(SubmitRequest {
                request_id: "drop-abort".into(),
                content: "go".into(),
            })
            .await
            .unwrap();
        started_rx.await.unwrap();
        let aborter = harness.clone();
        let h = handle.clone();
        let abort = tokio::spawn(async move { aborter.abort(h).await });
        reached.notified().await;
        abort.abort();
        let _ = abort.await;
        release.notify_one();
        assert_eq!(harness.wait(handle).await.unwrap().status, "aborted");
        harness.close().await.unwrap();
    }

    #[tokio::test]
    async fn abort_selected_but_unadmitted_generation_wakes_executor_and_close_drains() {
        let harness = Arc::new(
            DurableHarness::open(
                Box::new(MemoryStorage::new()),
                Arc::new(ToolModel),
                model(),
                PinnedOptions::default(),
            )
            .await
            .unwrap(),
        );
        let (selected, release) = harness.inject_phase_barrier().await;
        let handle = harness
            .submit(SubmitRequest {
                request_id: "selected-abort".into(),
                content: "go".into(),
            })
            .await
            .unwrap();
        selected.await.unwrap();
        let aborter = harness.clone();
        let h = handle.clone();
        let abort = tokio::spawn(async move { aborter.abort(h).await });
        release.send(()).unwrap();
        abort.await.unwrap().unwrap();
        assert_eq!(harness.wait(handle).await.unwrap().status, "aborted");
        harness.close().await.unwrap();
    }

    #[tokio::test]
    async fn dropped_first_close_caller_still_drains_and_releases_journal_writer() {
        struct ClosingRunner {
            started: Mutex<Option<oneshot::Sender<()>>>,
            release: Arc<Notify>,
        }
        impl DurableModelRunner for ClosingRunner {
            fn run<'a>(&'a self, _: ModelIntent) -> crate::durable::model::ModelFuture<'a> {
                Box::pin(async move {
                    if let Some(tx) = self.started.lock().unwrap().take() {
                        let _ = tx.send(());
                    }
                    self.release.notified().await;
                    ModelRun::one(ModelTerminal::Answer {
                        content: vec![],
                        text: "done".into(),
                        usage: DurableUsage::default(),
                        response_id: None,
                        stop_reason: "stop".into(),
                    })
                })
            }
        }
        let path =
            std::env::temp_dir().join(format!("rs-ai-r1c-drop-close-{}", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let (started_tx, started_rx) = oneshot::channel();
        let release = Arc::new(Notify::new());
        let harness = Arc::new(
            DurableHarness::open(
                Box::new(JournalStorage::open(&path).unwrap()),
                Arc::new(ClosingRunner {
                    started: Mutex::new(Some(started_tx)),
                    release: release.clone(),
                }),
                model(),
                PinnedOptions::default(),
            )
            .await
            .unwrap(),
        );
        let handle = harness
            .submit(SubmitRequest {
                request_id: "drop-close".into(),
                content: "go".into(),
            })
            .await
            .unwrap();
        started_rx.await.unwrap();
        let closer = harness.clone();
        let close = tokio::spawn(async move { closer.close().await });
        while !harness.is_sealed() {
            tokio::task::yield_now().await;
        }
        close.abort();
        let _ = close.await;
        release.notify_one();
        harness.close().await.unwrap();
        drop(harness);
        let reopened = DurableHarness::open(
            Box::new(JournalStorage::open(&path).unwrap()),
            Arc::new(ClosingRunner {
                started: Mutex::new(None),
                release: Arc::new(Notify::new()),
            }),
            model(),
            PinnedOptions::default(),
        )
        .await
        .unwrap();
        assert_eq!(reopened.inspect(handle.id).await.unwrap().status, "done");
        reopened.close().await.unwrap();
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn invalid_abort_handle_is_rejected_without_touching_generation() {
        let (storage, reached, storage_release, acked) = AbortAckStorage::new(1);
        let harness = Arc::new(
            DurableHarness::open(
                Box::new(storage),
                Arc::new(ToolModel),
                model(),
                PinnedOptions::default(),
            )
            .await
            .unwrap(),
        );
        let (selected, release) = harness.inject_phase_barrier().await;
        let handle = harness
            .submit(SubmitRequest {
                request_id: "bad-handle".into(),
                content: "go".into(),
            })
            .await
            .unwrap();
        selected.await.unwrap();
        let bad = SubmissionHandle {
            id: handle.id,
            task_id: TaskId::new(handle.task_id.get() + 1).unwrap(),
        };
        assert!(matches!(
            harness.abort(bad).await,
            Err(DurableError::Rejected(_))
        ));
        let valid = handle.clone();
        let aborter = harness.clone();
        let abort = tokio::spawn(async move { aborter.abort(valid).await });
        reached.notified().await;
        storage_release.notify_one();
        acked.notified().await;
        release.send(()).unwrap();
        abort.await.unwrap().unwrap();
        assert_eq!(harness.wait(handle).await.unwrap().status, "aborted");
        harness.close().await.unwrap();
    }

    struct JournalAbortAckStorage {
        inner: JournalStorage,
        claim: WriterClaim,
        commits: std::sync::atomic::AtomicUsize,
        reached: Arc<Notify>,
        release: Arc<Notify>,
        acked: Arc<Notify>,
    }
    impl JournalAbortAckStorage {
        fn open(path: &std::path::Path) -> (Self, Arc<Notify>, Arc<Notify>, Arc<Notify>) {
            let inner = JournalStorage::open(path).unwrap();
            let claim = inner.claim_writer().unwrap();
            let reached = Arc::new(Notify::new());
            let release = Arc::new(Notify::new());
            let acked = Arc::new(Notify::new());
            (
                Self {
                    inner,
                    claim,
                    commits: std::sync::atomic::AtomicUsize::new(0),
                    reached: reached.clone(),
                    release: release.clone(),
                    acked: acked.clone(),
                },
                reached,
                release,
                acked,
            )
        }
    }
    impl DurableStorage for JournalAbortAckStorage {
        fn claim_writer(&self) -> Result<WriterClaim, DurableError> {
            Ok(WriterClaim::fresh())
        }
        fn load<'a>(&'a self, _: &'a WriterClaim) -> StorageFuture<'a, StorageSnapshot> {
            self.inner.load(&self.claim)
        }
        fn commit<'a>(&'a self, _: &'a WriterClaim, b: CommitBatch) -> StorageFuture<'a, ()> {
            Box::pin(async move {
                let n = self
                    .commits
                    .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                if n == 1 {
                    self.inner.commit(&self.claim, b).await?;
                    self.reached.notify_one();
                    self.release.notified().await;
                    self.acked.notify_one();
                    Ok(())
                } else {
                    self.inner.commit(&self.claim, b).await
                }
            })
        }
        fn close<'a>(&'a self, _: &'a WriterClaim) -> StorageFuture<'a, ()> {
            self.inner.close(&self.claim)
        }
    }

    #[tokio::test]
    async fn selected_unadmitted_abort_and_concurrent_close_settle_and_release_writer() {
        let path =
            std::env::temp_dir().join(format!("rs-ai-r1c-abort-close-{}", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let (storage, reached, storage_release, acked) = JournalAbortAckStorage::open(&path);
        let harness = Arc::new(
            DurableHarness::open(
                Box::new(storage),
                Arc::new(ToolModel),
                model(),
                PinnedOptions::default(),
            )
            .await
            .unwrap(),
        );
        let (selected, release) = harness.inject_phase_barrier().await;
        let handle = harness
            .submit(SubmitRequest {
                request_id: "abort-close".into(),
                content: "go".into(),
            })
            .await
            .unwrap();
        selected.await.unwrap();
        let aborter = harness.clone();
        let h = handle.clone();
        let abort = tokio::spawn(async move { aborter.abort(h).await });
        reached.notified().await;
        storage_release.notify_one();
        acked.notified().await;
        let closer = harness.clone();
        let close = tokio::spawn(async move { closer.close().await });
        while !harness.is_sealed() {
            tokio::task::yield_now().await;
        }
        release.send(()).unwrap();
        abort.await.unwrap().unwrap();
        close.await.unwrap().unwrap();
        drop(harness);
        let reopened = DurableHarness::open(
            Box::new(JournalStorage::open(&path).unwrap()),
            Arc::new(ToolModel),
            model(),
            PinnedOptions::default(),
        )
        .await
        .unwrap();
        assert_eq!(reopened.inspect(handle.id).await.unwrap().status, "aborted");
        reopened.close().await.unwrap();
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn abort_at_successor_checkpoint_fence_prevents_successor_provider_and_new_effect() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        struct Rounds {
            calls: Arc<AtomicUsize>,
        }
        impl DurableModelRunner for Rounds {
            fn run<'a>(&'a self, _: ModelIntent) -> crate::durable::model::ModelFuture<'a> {
                let n = self.calls.fetch_add(1, Ordering::SeqCst);
                Box::pin(async move {
                    if n == 0 {
                        ModelRun::one(ModelTerminal::ToolCalls {
                            calls: vec![DurableToolCall {
                                provider_call_id: "one".into(),
                                name: "block".into(),
                                original_arguments: json!({}),
                            }],
                            usage: DurableUsage::default(),
                            assistant: json!({"role":"assistant","content":[{"type":"toolCall","id":"one","name":"block","arguments":{}}],"timestamp":0,"diagnostics":[],"isError":false,"addedToolNames":[],"toolsAdded":[],"toolsRemoved":[]}),
                        })
                    } else {
                        ModelRun::one(ModelTerminal::Answer {
                            content: vec![],
                            text: "forbidden".into(),
                            usage: DurableUsage::default(),
                            response_id: None,
                            stop_reason: "stop".into(),
                        })
                    }
                })
            }
        }
        struct Immediate(Arc<AtomicUsize>);
        impl DurableTool for Immediate {
            fn execute<'a>(&'a self, _: ToolExecution) -> crate::durable::tool::ToolFuture<'a> {
                self.0.fetch_add(1, Ordering::SeqCst);
                Box::pin(async {
                    Ok(ToolOutput {
                        value: json!({"ok":true}),
                        usage: None,
                    })
                })
            }
        }
        let (storage, reached, storage_release, acked) = AbortAckStorage::abort_batch();
        let calls = Arc::new(AtomicUsize::new(0));
        let effects = Arc::new(AtomicUsize::new(0));
        let registry = Arc::new(DurableToolRegistry::default());
        registry
            .register(
                definition(),
                "immediate",
                "1",
                ReplayPolicy::Safe,
                Arc::new(Immediate(effects.clone())),
            )
            .unwrap();
        let harness = Arc::new(
            DurableHarness::open_with_tools(
                Box::new(storage),
                Arc::new(Rounds {
                    calls: calls.clone(),
                }),
                model(),
                PinnedOptions::default(),
                registry,
            )
            .await
            .unwrap(),
        );
        let (selected, phase_release) = harness.inject_successor_phase_barrier().await;
        let handle = harness
            .submit(SubmitRequest {
                request_id: "successor-abort".into(),
                content: "go".into(),
            })
            .await
            .unwrap();
        selected.await.unwrap();
        let aborter = harness.clone();
        let h = handle.clone();
        let abort = tokio::spawn(async move { aborter.abort(h).await });
        reached.notified().await;
        storage_release.notify_one();
        acked.notified().await;
        phase_release.send(()).unwrap();
        abort.await.unwrap().unwrap();
        assert_eq!(harness.wait(handle).await.unwrap().status, "aborted");
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(effects.load(Ordering::SeqCst), 1);
        harness.close().await.unwrap();
    }

    #[tokio::test]
    async fn abort_marks_then_signals_and_settles_bottom_up() {
        let (started_tx, started_rx) = oneshot::channel();
        let registry = Arc::new(DurableToolRegistry::default());
        registry
            .register(
                definition(),
                "block",
                "1",
                ReplayPolicy::Safe,
                Arc::new(Blocking {
                    started: Mutex::new(Some(started_tx)),
                }),
            )
            .unwrap();
        let harness = Arc::new(
            DurableHarness::open_with_tools(
                Box::new(MemoryStorage::new()),
                Arc::new(ToolModel),
                model(),
                PinnedOptions::default(),
                registry,
            )
            .await
            .unwrap(),
        );
        let handle = harness
            .submit(SubmitRequest {
                request_id: "abort".into(),
                content: "go".into(),
            })
            .await
            .unwrap();
        started_rx.await.unwrap();
        harness.abort(handle.clone()).await.unwrap();
        let view = harness.inspect(handle.id).await.unwrap();
        assert_eq!(view.status, "aborted");
        assert_eq!(view.reason.unwrap()["code"], "aborted");
        harness.close().await.unwrap();
    }

    #[tokio::test]
    async fn abort_during_first_admitted_answer_suppresses_answer_and_preserves_usage() {
        let (started_tx, started_rx) = oneshot::channel();
        let release = Arc::new(tokio::sync::Notify::new());
        struct BlockedAnswer {
            started: Mutex<Option<oneshot::Sender<()>>>,
            release: Arc<tokio::sync::Notify>,
        }
        impl DurableModelRunner for BlockedAnswer {
            fn run<'a>(&'a self, _: ModelIntent) -> crate::durable::model::ModelFuture<'a> {
                Box::pin(async move {
                    if let Some(tx) = self.started.lock().unwrap().take() {
                        let _ = tx.send(());
                    }
                    self.release.notified().await;
                    ModelRun::one(ModelTerminal::Answer {
                        content: vec![],
                        text: "must-not-publish".into(),
                        usage: DurableUsage {
                            input: 3,
                            output: 3,
                            total_tokens: 6,
                            ..Default::default()
                        },
                        response_id: None,
                        stop_reason: "stop".into(),
                    })
                })
            }
        }
        let harness = Arc::new(
            DurableHarness::open(
                Box::new(MemoryStorage::new()),
                Arc::new(BlockedAnswer {
                    started: Mutex::new(Some(started_tx)),
                    release: release.clone(),
                }),
                model(),
                PinnedOptions::default(),
            )
            .await
            .unwrap(),
        );
        let handle = harness
            .submit(SubmitRequest {
                request_id: "first-abort".into(),
                content: "go".into(),
            })
            .await
            .unwrap();
        started_rx.await.unwrap();
        let aborter = harness.clone();
        let h = handle.clone();
        let abort = tokio::spawn(async move { aborter.abort(h).await });
        tokio::task::yield_now().await;
        release.notify_one();
        abort.await.unwrap().unwrap();
        let view = harness.inspect(handle.id).await.unwrap();
        assert_eq!(view.status, "aborted");
        assert!(view.answer.is_none());
        let docs = harness.inspect_documents().await.unwrap();
        assert_eq!(docs["pi.usage"]["aggregate"]["total_tokens"], 6);
        harness.close().await.unwrap();
    }

    #[tokio::test]
    async fn close_drains_admitted_tool_without_abort_or_successor_effect() {
        struct CloseModel(Arc<std::sync::atomic::AtomicUsize>);
        impl DurableModelRunner for CloseModel {
            fn run<'a>(&'a self, _intent: ModelIntent) -> crate::durable::model::ModelFuture<'a> {
                let call = self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Box::pin(async move {
                    if call == 0 {
                        ModelRun::one(ModelTerminal::ToolCalls {
                            calls: vec![DurableToolCall {
                                provider_call_id: "c".into(),
                                name: "block".into(),
                                original_arguments: json!({}),
                            }],
                            usage: DurableUsage::default(),
                            assistant: json!({"role":"assistant","content":[{"type":"toolCall","id":"c","name":"block","arguments":{}}],"timestamp":0,"diagnostics":[],"isError":false,"addedToolNames":[],"toolsAdded":[],"toolsRemoved":[]}),
                        })
                    } else {
                        panic!("sealed close must not start successor")
                    }
                })
            }
        }
        struct NonCooperative {
            started: Mutex<Option<oneshot::Sender<()>>>,
            release: Arc<tokio::sync::Notify>,
        }
        impl DurableTool for NonCooperative {
            fn execute<'a>(&'a self, _: ToolExecution) -> crate::durable::tool::ToolFuture<'a> {
                Box::pin(async move {
                    if let Some(tx) = self.started.lock().unwrap().take() {
                        let _ = tx.send(());
                    }
                    self.release.notified().await;
                    Ok(ToolOutput {
                        value: json!({"done":true}),
                        usage: None,
                    })
                })
            }
        }
        let (started_tx, started_rx) = oneshot::channel();
        let release = Arc::new(tokio::sync::Notify::new());
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let registry = Arc::new(DurableToolRegistry::default());
        registry
            .register(
                definition(),
                "block",
                "1",
                ReplayPolicy::Safe,
                Arc::new(NonCooperative {
                    started: Mutex::new(Some(started_tx)),
                    release: release.clone(),
                }),
            )
            .unwrap();
        let harness = Arc::new(
            DurableHarness::open_with_tools(
                Box::new(MemoryStorage::new()),
                Arc::new(CloseModel(calls.clone())),
                model(),
                PinnedOptions::default(),
                registry,
            )
            .await
            .unwrap(),
        );
        let _handle = harness
            .submit(SubmitRequest {
                request_id: "close".into(),
                content: "go".into(),
            })
            .await
            .unwrap();
        started_rx.await.unwrap();
        let closer = harness.clone();
        let close = tokio::spawn(async move { closer.close().await });
        tokio::task::yield_now().await;
        assert!(!close.is_finished());
        release.notify_one();
        close.await.unwrap().unwrap();
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[test]
    fn tool_registry_seal_is_atomic_and_rejects_late_registration() {
        let registry = DurableToolRegistry::default();
        registry.seal();
        struct Never;
        impl DurableTool for Never {
            fn execute<'a>(&'a self, _: ToolExecution) -> crate::durable::tool::ToolFuture<'a> {
                panic!("never")
            }
        }
        assert!(
            registry
                .register(definition(), "x", "1", ReplayPolicy::Safe, Arc::new(Never))
                .is_err()
        );
    }
}
