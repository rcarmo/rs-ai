#[cfg(test)]
mod tests {
    use crate::durable::*;
    use crate::types::{Model, ModelCompat, ModelCost};
    use serde_json::json;
    use std::sync::{Arc, Mutex};

    fn model() -> PinnedModel {
        PinnedModel::from_model(&Model {
            id: "m".into(),
            name: "M".into(),
            api: "test".into(),
            provider: "p".into(),
            base_url: "http://secret.local".into(),
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
            headers: Some(std::collections::HashMap::from([(
                "authorization".into(),
                "secret".into(),
            )])),
            api_key: Some("secret".into()),
            compat: ModelCompat::default(),
        })
        .unwrap()
    }

    struct ImmediateRunner {
        calls: Arc<Mutex<usize>>,
    }
    impl DurableModelRunner for ImmediateRunner {
        fn run<'a>(&'a self, _intent: ModelIntent) -> crate::durable::model::ModelFuture<'a> {
            Box::pin(async move {
                *self.calls.lock().unwrap() += 1;
                ModelRun::one(ModelTerminal::Answer {
                    content: vec![DurableContent::Text {
                        text: "answer".into(),
                    }],
                    text: "answer".into(),
                    usage: DurableUsage::default(),
                    response_id: None,
                    stop_reason: "stop".into(),
                })
            })
        }
    }

    #[tokio::test]
    async fn request_id_reacquires_same_winner_and_conflict_is_rejected() {
        let calls = Arc::new(Mutex::new(0));
        let harness = DurableHarness::open(
            Box::new(MemoryStorage::new()),
            Arc::new(ImmediateRunner {
                calls: calls.clone(),
            }),
            model(),
            PinnedOptions::default(),
        )
        .await
        .unwrap();
        let request = SubmitRequest {
            request_id: "r1".into(),
            content: "hello".into(),
        };
        let first = harness.submit(request.clone()).await.unwrap();
        let done = harness.wait(first.clone()).await.unwrap();
        assert_eq!(done.answer.as_deref(), Some("answer"));
        let second = harness.submit(request).await.unwrap();
        assert_eq!(first, second);
        assert_eq!(*calls.lock().unwrap(), 1);
        assert!(matches!(
            harness
                .submit(SubmitRequest {
                    request_id: "r1".into(),
                    content: "different".into()
                })
                .await,
            Err(DurableError::Rejected(_))
        ));
        harness.close().await.unwrap();
    }

    #[tokio::test]
    async fn pinned_intent_strips_process_local_transport_and_bounds_submission() {
        let pinned = model();
        assert!(pinned.behavior.base_url.is_empty());
        assert!(pinned.behavior.headers.is_none());
        assert!(pinned.behavior.api_key.is_none());
        let runner = Arc::new(ImmediateRunner {
            calls: Arc::new(Mutex::new(0)),
        });
        let harness = DurableHarness::open(
            Box::new(MemoryStorage::new()),
            runner,
            pinned,
            PinnedOptions::default(),
        )
        .await
        .unwrap();
        assert!(matches!(
            harness
                .submit(SubmitRequest {
                    request_id: "large".into(),
                    content: "x".repeat(MAX_SUBMISSION_BYTES + 1)
                })
                .await,
            Err(DurableError::TooLarge { .. })
        ));
        let id = harness.passive_write("note".into()).await.unwrap();
        assert_eq!(id.get(), 1);
        assert_eq!(harness.context().await.unwrap()[0].text, "note");
        harness.close().await.unwrap();
    }

    #[tokio::test]
    async fn historical_context_options_cut_inclusively_without_mutation_or_dispatch() {
        let calls = Arc::new(Mutex::new(0));
        let harness = DurableHarness::open(
            Box::new(MemoryStorage::new()),
            Arc::new(ImmediateRunner {
                calls: calls.clone(),
            }),
            model(),
            PinnedOptions::default(),
        )
        .await
        .unwrap();
        let first = harness.passive_write("before".into()).await.unwrap();
        let handle = harness
            .submit(SubmitRequest {
                request_id: "cut".into(),
                content: "question".into(),
            })
            .await
            .unwrap();
        let view = harness.wait(handle.clone()).await.unwrap();
        assert_eq!(view.answer.as_deref(), Some("answer"));
        let tail = harness.passive_write("after".into()).await.unwrap();
        let snapshot = harness.test_snapshot().await.unwrap();
        let submission = &snapshot.submissions[&handle.id];
        let cut = harness
            .context_with_options(ContextOptions { at: Some(first) })
            .await
            .unwrap();
        assert_eq!(
            cut.iter()
                .map(|message| message.text.as_str())
                .collect::<Vec<_>>(),
            ["before"]
        );
        let cut = harness
            .context_with_options(ContextOptions {
                at: Some(submission.entry_id),
            })
            .await
            .unwrap();
        assert_eq!(
            cut.iter()
                .map(|message| message.text.as_str())
                .collect::<Vec<_>>(),
            ["before", "question"]
        );
        let cut = harness
            .context_with_options(ContextOptions {
                at: submission.answer_id,
            })
            .await
            .unwrap();
        assert_eq!(
            cut.iter()
                .map(|message| message.text.as_str())
                .collect::<Vec<_>>(),
            ["before", "question", "answer"]
        );
        assert_eq!(
            harness
                .context_with_options(ContextOptions { at: Some(tail) })
                .await
                .unwrap(),
            harness.context().await.unwrap()
        );
        assert!(matches!(
            harness
                .context_with_options(ContextOptions {
                    at: Some(EntryId::new(99999).unwrap())
                })
                .await,
            Err(DurableError::Rejected(_))
        ));
        assert_eq!(harness.test_snapshot().await.unwrap(), snapshot);
        assert_eq!(*calls.lock().unwrap(), 1);
        harness.close().await.unwrap();
    }

    #[tokio::test]
    async fn passive_entry_and_prior_answer_are_in_follow_up_context_order() {
        struct CaptureRunner {
            contexts: Arc<Mutex<Vec<Vec<DurableMessage>>>>,
        }
        impl DurableModelRunner for CaptureRunner {
            fn run<'a>(&'a self, intent: ModelIntent) -> crate::durable::model::ModelFuture<'a> {
                self.contexts.lock().unwrap().push(intent.context.clone());
                Box::pin(async {
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
        let contexts = Arc::new(Mutex::new(Vec::new()));
        let harness = DurableHarness::open(
            Box::new(MemoryStorage::new()),
            Arc::new(CaptureRunner {
                contexts: contexts.clone(),
            }),
            model(),
            PinnedOptions::default(),
        )
        .await
        .unwrap();
        harness.passive_write("passive".into()).await.unwrap();
        let first = harness
            .submit(SubmitRequest {
                request_id: "ctx-1".into(),
                content: "first".into(),
            })
            .await
            .unwrap();
        harness.wait(first).await.unwrap();
        let second = harness
            .follow_up(SubmitRequest {
                request_id: "ctx-2".into(),
                content: "second".into(),
            })
            .await
            .unwrap();
        harness.wait(second).await.unwrap();
        {
            let seen = contexts.lock().unwrap();
            assert_eq!(
                seen[0]
                    .iter()
                    .map(|value| value.text.as_str())
                    .collect::<Vec<_>>(),
                vec!["passive", "first"]
            );
            assert_eq!(
                seen[1]
                    .iter()
                    .map(|value| value.text.as_str())
                    .collect::<Vec<_>>(),
                vec!["passive", "first", "answer", "second"]
            );
        }
        harness.close().await.unwrap();
    }

    #[tokio::test]
    async fn invalid_custom_terminal_usage_is_rejected_and_surfaces_to_wait_and_close() {
        struct InvalidRunner;
        impl DurableModelRunner for InvalidRunner {
            fn run<'a>(&'a self, _intent: ModelIntent) -> crate::durable::model::ModelFuture<'a> {
                Box::pin(async {
                    ModelRun::one(ModelTerminal::Answer {
                        content: vec![],
                        text: "bad".into(),
                        usage: DurableUsage {
                            cost: crate::durable::model::DurableCost {
                                total: f64::NAN,
                                ..Default::default()
                            },
                            ..Default::default()
                        },
                        response_id: None,
                        stop_reason: "stop".into(),
                    })
                })
            }
        }
        let harness = DurableHarness::open(
            Box::new(MemoryStorage::new()),
            Arc::new(InvalidRunner),
            model(),
            PinnedOptions::default(),
        )
        .await
        .unwrap();
        let handle = harness
            .submit(SubmitRequest {
                request_id: "invalid".into(),
                content: "hello".into(),
            })
            .await
            .unwrap();
        assert!(matches!(
            harness.wait(handle).await,
            Err(DurableError::Rejected(_))
        ));
        assert!(matches!(
            harness.close().await,
            Err(DurableError::Rejected(_))
        ));
    }

    #[tokio::test]
    async fn provider_panic_immediately_poison_waits_and_close_releases_writer() {
        struct PanicRunner;
        impl DurableModelRunner for PanicRunner {
            fn run<'a>(&'a self, _intent: ModelIntent) -> crate::durable::model::ModelFuture<'a> {
                Box::pin(async { panic!("provider panic") })
            }
        }
        let path =
            std::env::temp_dir().join(format!("rs-ai-r1b-panic-close-{}", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let harness = DurableHarness::open(
            Box::new(JournalStorage::open(&path).unwrap()),
            Arc::new(PanicRunner),
            model(),
            PinnedOptions::default(),
        )
        .await
        .unwrap();
        let handle = harness
            .submit(SubmitRequest {
                request_id: "panic".into(),
                content: "hello".into(),
            })
            .await
            .unwrap();
        assert!(matches!(
            harness.wait(handle).await,
            Err(DurableError::Poisoned)
        ));
        assert!(matches!(
            harness
                .submit(SubmitRequest {
                    request_id: "after".into(),
                    content: "after".into()
                })
                .await,
            Err(DurableError::Poisoned)
        ));
        assert!(matches!(harness.close().await, Err(DurableError::Poisoned)));
        let reopened = JournalStorage::open(&path).unwrap();
        let claim = reopened.claim_writer().unwrap();
        reopened.close(&claim).await.unwrap();
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn executor_failure_close_releases_persistent_writer() {
        struct InvalidRunner;
        impl DurableModelRunner for InvalidRunner {
            fn run<'a>(&'a self, _intent: ModelIntent) -> crate::durable::model::ModelFuture<'a> {
                Box::pin(async {
                    ModelRun::one(ModelTerminal::Malformed {
                        code: "secret raw error".into(),
                    })
                })
            }
        }
        let path =
            std::env::temp_dir().join(format!("rs-ai-r1b-error-close-{}", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let harness = DurableHarness::open(
            Box::new(JournalStorage::open(&path).unwrap()),
            Arc::new(InvalidRunner),
            model(),
            PinnedOptions::default(),
        )
        .await
        .unwrap();
        let handle = harness
            .submit(SubmitRequest {
                request_id: "bad-code".into(),
                content: "hello".into(),
            })
            .await
            .unwrap();
        assert!(matches!(
            harness.wait(handle).await,
            Err(DurableError::Rejected(_))
        ));
        assert!(matches!(
            harness.close().await,
            Err(DurableError::Rejected(_))
        ));
        let reopened = JournalStorage::open(&path).unwrap();
        let claim = reopened.claim_writer().unwrap();
        reopened.close(&claim).await.unwrap();
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn two_turn_usage_aggregate_survives_journal_reopen() {
        struct UsageRunner(Arc<Mutex<u32>>);
        impl DurableModelRunner for UsageRunner {
            fn run<'a>(&'a self, _intent: ModelIntent) -> crate::durable::model::ModelFuture<'a> {
                let n = {
                    let mut value = self.0.lock().unwrap();
                    *value += 1;
                    *value
                };
                Box::pin(async move {
                    ModelRun::one(ModelTerminal::Answer {
                        content: vec![],
                        text: format!("a{n}"),
                        usage: DurableUsage {
                            input: n,
                            output: n,
                            total_tokens: n * 2,
                            cost: crate::durable::model::DurableCost {
                                total: f64::from(n),
                                ..Default::default()
                            },
                            ..Default::default()
                        },
                        response_id: None,
                        stop_reason: "stop".into(),
                    })
                })
            }
        }
        let path = std::env::temp_dir().join(format!("rs-ai-r1b-usage-{}", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let calls = Arc::new(Mutex::new(0));
        {
            let harness = DurableHarness::open(
                Box::new(JournalStorage::open(&path).unwrap()),
                Arc::new(UsageRunner(calls.clone())),
                model(),
                PinnedOptions::default(),
            )
            .await
            .unwrap();
            let first = harness
                .submit(SubmitRequest {
                    request_id: "u1".into(),
                    content: "one".into(),
                })
                .await
                .unwrap();
            harness.wait(first).await.unwrap();
            harness.close().await.unwrap();
        }
        let harness = DurableHarness::open(
            Box::new(JournalStorage::open(&path).unwrap()),
            Arc::new(UsageRunner(calls)),
            model(),
            PinnedOptions::default(),
        )
        .await
        .unwrap();
        let second = harness
            .submit(SubmitRequest {
                request_id: "u2".into(),
                content: "two".into(),
            })
            .await
            .unwrap();
        harness.wait(second).await.unwrap();
        let usage = harness
            .inspect_documents()
            .await
            .unwrap()
            .remove("pi.usage")
            .unwrap();
        assert_eq!(usage["aggregate"]["input"], 3);
        assert_eq!(usage["aggregate"]["output"], 3);
        assert_eq!(usage["aggregate"]["total_tokens"], 6);
        assert_eq!(usage["aggregate"]["cost"]["total"], 3.0);
        harness.close().await.unwrap();
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn billed_error_usage_is_preserved_atomically() {
        struct ErrorRunner;
        impl DurableModelRunner for ErrorRunner {
            fn run<'a>(&'a self, _intent: ModelIntent) -> crate::durable::model::ModelFuture<'a> {
                Box::pin(async {
                    ModelRun::one(ModelTerminal::BilledError {
                        code: "rate_limited".into(),
                        usage: DurableUsage {
                            input: 4,
                            total_tokens: 4,
                            ..Default::default()
                        },
                    })
                })
            }
        }
        let harness = DurableHarness::open(
            Box::new(MemoryStorage::new()),
            Arc::new(ErrorRunner),
            model(),
            PinnedOptions::default(),
        )
        .await
        .unwrap();
        let handle = harness
            .submit(SubmitRequest {
                request_id: "err".into(),
                content: "hello".into(),
            })
            .await
            .unwrap();
        let failed = harness.wait(handle).await.unwrap();
        assert_eq!(failed.status, "failed");
        assert_eq!(failed.reason.unwrap()["code"], json!("rate_limited"));
        assert_eq!(failed.usage.unwrap()["input"], json!(4));
        harness.close().await.unwrap();
    }
}
