#[cfg(test)]
mod tests {
    use crate::durable::storage::DurableStorage;
    use crate::durable::*;
    use crate::types::{Model, ModelCompat, ModelCost};
    use serde_json::json;
    use std::sync::Arc;
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

    struct BarrierRunner {
        started: std::sync::Mutex<Option<oneshot::Sender<()>>>,
        release: Arc<Notify>,
    }
    impl DurableModelRunner for BarrierRunner {
        fn run<'a>(&'a self, _intent: ModelIntent) -> crate::durable::model::ModelFuture<'a> {
            Box::pin(async move {
                if let Some(started) = self.started.lock().unwrap().take() {
                    let _ = started.send(());
                }
                self.release.notified().await;
                ModelRun::one(ModelTerminal::Answer {
                    content: vec![DurableContent::Text {
                        text: "done".into(),
                    }],
                    text: "done".into(),
                    usage: DurableUsage::default(),
                    response_id: None,
                    stop_reason: "stop".into(),
                })
            })
        }
    }

    #[tokio::test]
    async fn caller_drop_does_not_abandon_and_close_drains_noncooperative_runner() {
        let (started_tx, started_rx) = oneshot::channel();
        let release = Arc::new(Notify::new());
        let harness = Arc::new(
            DurableHarness::open(
                Box::new(MemoryStorage::new()),
                Arc::new(BarrierRunner {
                    started: std::sync::Mutex::new(Some(started_tx)),
                    release: release.clone(),
                }),
                model(),
                PinnedOptions::default(),
            )
            .await
            .unwrap(),
        );
        let submit_harness = harness.clone();
        let submit = tokio::spawn(async move {
            submit_harness
                .submit(SubmitRequest {
                    request_id: "drop".into(),
                    content: "hello".into(),
                })
                .await
        });
        started_rx.await.unwrap();
        submit.abort();
        let _ = submit.await;
        let close_harness = harness.clone();
        let close = tokio::spawn(async move { close_harness.close().await });
        tokio::task::yield_now().await;
        assert!(!close.is_finished());
        assert!(matches!(
            harness
                .submit(SubmitRequest {
                    request_id: "late".into(),
                    content: "late".into()
                })
                .await,
            Err(DurableError::Closed)
        ));
        close.abort();
        let _ = close.await;
        release.notify_one();
        harness.close().await.unwrap();
    }

    #[tokio::test]
    async fn follow_up_and_passive_write_are_fenced_while_generation_is_pending() {
        let (started_tx, started_rx) = oneshot::channel();
        let release = Arc::new(Notify::new());
        let harness = DurableHarness::open(
            Box::new(MemoryStorage::new()),
            Arc::new(BarrierRunner {
                started: std::sync::Mutex::new(Some(started_tx)),
                release: release.clone(),
            }),
            model(),
            PinnedOptions::default(),
        )
        .await
        .unwrap();
        let first = harness
            .submit(SubmitRequest {
                request_id: "first".into(),
                content: "one".into(),
            })
            .await
            .unwrap();
        started_rx.await.unwrap();
        let second = harness
            .follow_up(SubmitRequest {
                request_id: "second".into(),
                content: "two".into(),
            })
            .await
            .unwrap();
        let documents = harness.inspect_documents().await.unwrap();
        assert_eq!(
            documents["pi.inbox"]["pending"],
            json!([first.id.get(), second.id.get()])
        );
        assert_eq!(documents["pi.live"]["submission_id"], first.id.get());
        assert!(matches!(
            harness.passive_write("note".into()).await,
            Err(DurableError::Rejected(_))
        ));
        assert!(matches!(
            harness.update_context(ContextUpdate::default()).await,
            Err(DurableError::Rejected(_))
        ));
        let before = harness.test_snapshot().await.unwrap();
        assert!(matches!(
            harness.append_entry(EntryDraft::new("note")).await,
            Err(DurableError::Rejected(_))
        ));
        assert_eq!(harness.test_snapshot().await.unwrap(), before);
        release.notify_one();
        assert_eq!(harness.wait(first).await.unwrap().status, "done");
        let documents = harness.inspect_documents().await.unwrap();
        assert_eq!(documents["pi.inbox"]["pending"], json!([second.id.get()]));
        assert_eq!(documents["pi.live"]["submission_id"], second.id.get());
        release.notify_one();
        assert_eq!(harness.wait(second).await.unwrap().status, "done");
        let id = harness.passive_write("note".into()).await.unwrap();
        assert!(id.get() > 0);
        harness.close().await.unwrap();
    }

    #[tokio::test]
    async fn queued_native_context_includes_prior_answer_allocated_after_queued_input() {
        struct Runner {
            calls: std::sync::atomic::AtomicUsize,
            entered: Arc<Notify>,
            release: Arc<Notify>,
            seen: Arc<std::sync::Mutex<Vec<ModelIntent>>>,
        }
        impl DurableModelRunner for Runner {
            fn run<'a>(&'a self, intent: ModelIntent) -> crate::durable::model::ModelFuture<'a> {
                self.seen.lock().unwrap().push(intent);
                let first = self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 0;
                Box::pin(async move {
                    if first {
                        self.entered.notify_one();
                        self.release.notified().await;
                    }
                    ModelRun::one(ModelTerminal::Answer {
                        content: vec![],
                        text: "answer".into(),
                        usage: DurableUsage::default(),
                        response_id: None,
                        stop_reason: "stop".into(),
                    })
                })
            }
        }
        let entered = Arc::new(Notify::new());
        let release = Arc::new(Notify::new());
        let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
        let harness = DurableHarness::open(
            Box::new(MemoryStorage::new()),
            Arc::new(Runner {
                calls: std::sync::atomic::AtomicUsize::new(0),
                entered: entered.clone(),
                release: release.clone(),
                seen: seen.clone(),
            }),
            model(),
            PinnedOptions::default(),
        )
        .await
        .unwrap();
        let first = harness
            .submit(SubmitRequest {
                request_id: "q-first".into(),
                content: "first".into(),
            })
            .await
            .unwrap();
        entered.notified().await;
        let second = harness
            .submit(SubmitRequest {
                request_id: "q-second".into(),
                content: "second".into(),
            })
            .await
            .unwrap();
        release.notify_one();
        harness.wait(first).await.unwrap();
        harness.wait(second).await.unwrap();
        {
            let seen = seen.lock().unwrap();
            let messages = seen[1].native_messages.as_ref().unwrap();
            assert_eq!(
                messages
                    .iter()
                    .map(|message| message.role.clone())
                    .collect::<Vec<_>>(),
                [
                    crate::types::Role::User,
                    crate::types::Role::Assistant,
                    crate::types::Role::User
                ]
            );
            assert!(
                matches!(messages[1].content.first(), Some(crate::types::ContentBlock::Text { text, .. }) if text == "answer")
            );
        }
        harness.close().await.unwrap();
    }

    #[tokio::test]
    async fn more_than_wake_capacity_queued_admissions_do_not_deadlock() {
        let (started_tx, started_rx) = oneshot::channel();
        let release = Arc::new(Notify::new());
        let harness = Arc::new(
            DurableHarness::open(
                Box::new(MemoryStorage::new()),
                Arc::new(BarrierRunner {
                    started: std::sync::Mutex::new(Some(started_tx)),
                    release: release.clone(),
                }),
                model(),
                PinnedOptions::default(),
            )
            .await
            .unwrap(),
        );
        let first = harness
            .submit(SubmitRequest {
                request_id: "cap-0".into(),
                content: "zero".into(),
            })
            .await
            .unwrap();
        started_rx.await.unwrap();
        let mut queued = Vec::new();
        for index in 1..=70 {
            queued.push(
                harness
                    .follow_up(SubmitRequest {
                        request_id: format!("cap-{index}"),
                        content: format!("value-{index}"),
                    })
                    .await
                    .unwrap(),
            );
        }
        assert_eq!(
            harness.inspect_documents().await.unwrap()["pi.inbox"]["pending"]
                .as_array()
                .unwrap()
                .len(),
            71
        );
        let closer = harness.clone();
        let close = tokio::spawn(async move { closer.close().await });
        release.notify_one();
        close.await.unwrap().unwrap();
        let _ = (first, queued);
    }

    #[tokio::test]
    async fn selected_but_not_phase_admitted_task_stays_pending_after_close_and_reopens() {
        let path = std::env::temp_dir().join(format!("rs-ai-r1b-seal-{}", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        struct Immediate(Arc<std::sync::atomic::AtomicUsize>);
        impl DurableModelRunner for Immediate {
            fn run<'a>(&'a self, _intent: ModelIntent) -> crate::durable::model::ModelFuture<'a> {
                self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Box::pin(async {
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
        let harness = Arc::new(
            DurableHarness::open(
                Box::new(JournalStorage::open(&path).unwrap()),
                Arc::new(Immediate(calls.clone())),
                model(),
                PinnedOptions::default(),
            )
            .await
            .unwrap(),
        );
        let (selected, release) = harness.inject_phase_barrier().await;
        let handle = harness
            .submit(SubmitRequest {
                request_id: "sealed".into(),
                content: "hello".into(),
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
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 0);

        let reopened = DurableHarness::open(
            Box::new(JournalStorage::open(&path).unwrap()),
            Arc::new(Immediate(calls.clone())),
            model(),
            PinnedOptions::default(),
        )
        .await
        .unwrap();
        assert_eq!(reopened.inspect(handle.id).await.unwrap().status, "pending");
        reopened.resume(handle.clone()).await.unwrap();
        assert_eq!(reopened.wait(handle).await.unwrap().status, "done");
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
        reopened.close().await.unwrap();
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn close_seal_leaves_queued_follow_up_pending_without_new_effect() {
        let (started_tx, started_rx) = oneshot::channel();
        let release = Arc::new(Notify::new());
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        struct CountingBarrier {
            started: std::sync::Mutex<Option<oneshot::Sender<()>>>,
            release: Arc<Notify>,
            calls: Arc<std::sync::atomic::AtomicUsize>,
        }
        impl DurableModelRunner for CountingBarrier {
            fn run<'a>(&'a self, _intent: ModelIntent) -> crate::durable::model::ModelFuture<'a> {
                self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Box::pin(async move {
                    if let Some(started) = self.started.lock().unwrap().take() {
                        let _ = started.send(());
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
        let harness = Arc::new(
            DurableHarness::open(
                Box::new(MemoryStorage::new()),
                Arc::new(CountingBarrier {
                    started: std::sync::Mutex::new(Some(started_tx)),
                    release: release.clone(),
                    calls: calls.clone(),
                }),
                model(),
                PinnedOptions::default(),
            )
            .await
            .unwrap(),
        );
        let first = harness
            .submit(SubmitRequest {
                request_id: "one".into(),
                content: "one".into(),
            })
            .await
            .unwrap();
        started_rx.await.unwrap();
        let second = harness
            .follow_up(SubmitRequest {
                request_id: "two".into(),
                content: "two".into(),
            })
            .await
            .unwrap();
        let closer = harness.clone();
        let close = tokio::spawn(async move { closer.close().await });
        tokio::task::yield_now().await;
        release.notify_one();
        close.await.unwrap().unwrap();
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
        let _ = (first, second);
    }

    #[tokio::test]
    async fn persistent_running_task_reopens_pending_without_dispatch() {
        let path = std::env::temp_dir().join(format!("rs-ai-r1b-reconcile-{}", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let storage = JournalStorage::open(&path).unwrap();
        let claim = storage.claim_writer().unwrap();
        let seq = CommitSeq::new(1).unwrap();
        let intent = ModelIntent {
            model: model(),
            options: PinnedOptions::default(),
            provider_session_id: None,
            offered_tools: vec![],
            system_prompt: None,
            context: vec![DurableMessage {
                role: "user".into(),
                text: "hello".into(),
            }],
            native_messages: None,
            context_cutoff: 1,
            logical_attempt: 1,
        };
        storage
            .commit(
                &claim,
                CommitBatch {
                    seq,
                    next_id: 4,
                    next_seq: 2,
                    entries: vec![EntryRecord {
                        id: EntryId::new(1).unwrap(),
                        conversation_id: ConversationId::new(1).unwrap(),
                        kind: "user".into(),
                        value: json!({"text":"hello"}),
                        by_task_id: Some(TaskId::new(2).unwrap()),
                        created_seq: seq,
                    }],
                    tasks: vec![TaskRecord {
                        id: TaskId::new(2).unwrap(),
                        conversation_id: ConversationId::new(1).unwrap(),
                        kind: "generation".into(),
                        version: 1,
                        owner_task_id: None,
                        state: TaskState::Running,
                        input: serde_json::to_value(intent).unwrap(),
                        checkpoint: json!({"phase":"running"}),
                        outcome: None,
                        abort_requested: false,
                        started_at: None,
                        ended_at: None,
                        updated_seq: seq,
                    }],
                    submissions: vec![SubmissionRecord {
                        id: SubmissionId::new(3).unwrap(),
                        conversation_id: ConversationId::new(1).unwrap(),
                        request_id: Some("r".into()),
                        status: "pending".into(),
                        entry_id: EntryId::new(1).unwrap(),
                        answer_id: None,
                        reason: None,
                        updated_seq: seq,
                    }],
                    documents: vec![],
                },
            )
            .await
            .unwrap();
        storage.close(&claim).await.unwrap();

        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let contexts = Arc::new(std::sync::Mutex::new(Vec::<Vec<DurableMessage>>::new()));
        struct RecoveryRunner {
            calls: Arc<std::sync::atomic::AtomicUsize>,
            contexts: Arc<std::sync::Mutex<Vec<Vec<DurableMessage>>>>,
        }
        impl DurableModelRunner for RecoveryRunner {
            fn run<'a>(&'a self, intent: ModelIntent) -> crate::durable::model::ModelFuture<'a> {
                assert!(
                    intent
                        .provider_session_id
                        .as_ref()
                        .is_some_and(|id| id.len() == 36)
                );
                self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                self.contexts.lock().unwrap().push(intent.context);
                Box::pin(async {
                    ModelRun::one(ModelTerminal::Answer {
                        content: vec![],
                        text: "resumed".into(),
                        usage: DurableUsage::default(),
                        response_id: None,
                        stop_reason: "stop".into(),
                    })
                })
            }
        }
        let harness = DurableHarness::open(
            Box::new(JournalStorage::open(&path).unwrap()),
            Arc::new(RecoveryRunner {
                calls: calls.clone(),
                contexts: contexts.clone(),
            }),
            model(),
            PinnedOptions::default(),
        )
        .await
        .unwrap();
        let result = harness
            .inspect(SubmissionId::new(3).unwrap())
            .await
            .unwrap();
        assert_eq!(result.status, "pending");
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 0);
        assert!(
            !harness
                .inspect_documents()
                .await
                .unwrap()
                .contains_key("pi.provider")
        );
        let handle = SubmissionHandle {
            id: SubmissionId::new(3).unwrap(),
            task_id: TaskId::new(2).unwrap(),
        };
        harness.resume(handle.clone()).await.unwrap();
        assert_eq!(
            harness.wait(handle).await.unwrap().answer.as_deref(),
            Some("resumed")
        );
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert!(
            harness.inspect_documents().await.unwrap()["pi.provider"]["sessionId"]
                .as_str()
                .is_some()
        );
        assert_eq!(
            contexts.lock().unwrap()[0]
                .iter()
                .map(|value| value.text.as_str())
                .collect::<Vec<_>>(),
            vec!["hello"]
        );
        harness.close().await.unwrap();
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn duplicate_or_missing_terminal_settles_redacted_failure_once() {
        struct DuplicateRunner;
        impl DurableModelRunner for DuplicateRunner {
            fn run<'a>(&'a self, _intent: ModelIntent) -> crate::durable::model::ModelFuture<'a> {
                Box::pin(async {
                    ModelRun {
                        terminal: Some(ModelTerminal::Malformed { code: "one".into() }),
                        terminal_count: 2,
                        duration_ms: None,
                    }
                })
            }
        }
        let harness = DurableHarness::open(
            Box::new(MemoryStorage::new()),
            Arc::new(DuplicateRunner),
            model(),
            PinnedOptions::default(),
        )
        .await
        .unwrap();
        let handle = harness
            .submit(SubmitRequest {
                request_id: "dup".into(),
                content: "hello".into(),
            })
            .await
            .unwrap();
        let result = harness.wait(handle).await.unwrap();
        assert_eq!(result.reason.unwrap()["code"], "duplicate_terminal");
        harness.close().await.unwrap();
    }
}
