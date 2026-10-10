//! v0.99.1 llama.cpp classifier production transport coverage.

#[cfg(test)]
mod tests {
    use crate::classifiers::llama_cpp::{
        answer_from_probabilities, label_probabilities, llama_server_root, peak_confidence,
        render_question,
    };
    use crate::classifiers::{ClassifierOptions, classify};
    use crate::types::{
        ClassifierAnswer, ClassifierContext, ClassifierQuestion, ClassifierStopReason,
    };
    use indexmap::IndexMap;
    use serde_json::{Value, json};
    use std::sync::{Arc, Mutex};
    use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

    struct LlamaResponder {
        depths: Arc<Mutex<Vec<u64>>>,
        omit_second_until: u64,
    }

    impl Respond for LlamaResponder {
        fn respond(&self, request: &Request) -> ResponseTemplate {
            let body: Value = request.body_json().unwrap();
            match request.url.path() {
                "/tokenize" => {
                    let content = body["content"].as_str().unwrap();
                    let tokens = content
                        .chars()
                        .map(|value| u32::from(value) as u64)
                        .collect::<Vec<_>>();
                    ResponseTemplate::new(200).set_body_json(json!({"tokens":tokens}))
                }
                "/apply-template" => {
                    ResponseTemplate::new(200).set_body_json(json!({"prompt":"<|assistant|>\n"}))
                }
                "/completion" => {
                    let depth = body["n_probs"].as_u64().unwrap();
                    self.depths.lock().unwrap().push(depth);
                    let mut top = vec![json!({"id":65,"logprob":-1.5})];
                    if depth >= self.omit_second_until {
                        top.push(json!({"id":66,"logprob":-0.3}));
                    }
                    ResponseTemplate::new(200).set_body_json(json!({
                        "completion_probabilities":[{"top_logprobs":top}]
                    }))
                }
                _ => ResponseTemplate::new(404),
            }
        }
    }

    fn context() -> ClassifierContext {
        ClassifierContext {
            images: vec![],
            state: serde_json::from_value(json!({"message":"failing payouts"})).unwrap(),
            questions: IndexMap::from([(
                "team".into(),
                ClassifierQuestion::Choice {
                    instructions: "Which team?".into(),
                    criteria: IndexMap::from([
                        ("billing".into(), "Payments".into()),
                        ("technical".into(), "Bugs".into()),
                    ]),
                },
            )]),
        }
    }

    #[tokio::test]
    async fn classifies_from_next_token_probabilities_and_escalates_depth() {
        let server = MockServer::start().await;
        let depths = Arc::new(Mutex::new(Vec::new()));
        Mock::given(wiremock::matchers::method("POST"))
            .respond_with(LlamaResponder {
                depths: depths.clone(),
                omit_second_until: 4096,
            })
            .mount(&server)
            .await;
        let mut model = crate::types::ClassifierModel {
            model_type: crate::types::ModelType::Classifier,
            id: "qwen".into(),
            name: "Qwen".into(),
            api: crate::types::api::LLAMA_CPP_CLASSIFY.into(),
            provider: "llama.cpp".into(),
            base_url: format!("{}/v1", server.uri()),
            input: vec!["text".into()],
            input_limits: None,
            cost: Default::default(),
            context_window: 32768,
            headers: None,
            api_key: None,
        };
        model.api_key = Some("local".into());
        let payload_count = Arc::new(Mutex::new(0usize));
        let seen = payload_count.clone();
        let result = classify(
            &model,
            &context(),
            &ClassifierOptions {
                on_payload: Some(Arc::new(move |payload, _| {
                    *seen.lock().unwrap() += 1;
                    Ok(payload)
                })),
                ..Default::default()
            },
        )
        .await;
        assert_eq!(result.stop_reason, ClassifierStopReason::Stop);
        assert!(matches!(
            &result.answers["team"],
            ClassifierAnswer::Choice { choice, .. } if choice == "technical"
        ));
        assert_eq!(&*depths.lock().unwrap(), &[256, 4096]);
        assert_eq!(*payload_count.lock().unwrap(), 2);
        let requests = server.received_requests().await.unwrap();
        assert!(requests.iter().all(|request| {
            request.headers["authorization"].to_str().unwrap() == "Bearer local"
        }));
        let completion = requests
            .iter()
            .find(|request| request.url.path() == "/completion")
            .unwrap();
        let body: Value = completion.body_json().unwrap();
        assert_eq!(body["n_predict"], 1);
        assert_eq!(body["post_sampling_probs"], false);
        assert_eq!(body["cache_prompt"], true);
    }

    #[tokio::test]
    async fn rejects_invalid_temperature_before_network_access() {
        let server = MockServer::start().await;
        let mut model = crate::classifiers::get_classifier_model("typesafe", "jev-latest").unwrap();
        model.api = crate::types::api::LLAMA_CPP_CLASSIFY.into();
        model.base_url = server.uri();
        let result = classify(
            &model,
            &context(),
            &ClassifierOptions {
                temperature: Some(0.0),
                ..Default::default()
            },
        )
        .await;
        assert_eq!(result.stop_reason, ClassifierStopReason::Error);
        assert!(result.error_message.unwrap().contains("positive number"));
        assert!(server.received_requests().await.unwrap().is_empty());
    }

    #[test]
    fn renders_question_and_computes_probabilities_confidence_and_scores() {
        let rendered = render_question(&context(), "team").unwrap();
        assert_eq!(rendered.labels, vec!["A", "B"]);
        assert_eq!(rendered.keys, vec!["billing", "technical"]);
        assert!(rendered.content.ends_with("Answer with one letter."));
        let probabilities = label_probabilities(&[-1.5, -0.3], 1.0);
        assert!(probabilities[1] > probabilities[0]);
        assert!(peak_confidence(&probabilities) > 0.0);
        let score_question = ClassifierQuestion::Score {
            instructions: String::new(),
            criteria: vec!["a".into(), "b".into(), "c".into()],
        };
        assert!(matches!(
            answer_from_probabilities(
                &score_question,
                &["0".into(), "1".into(), "2".into()],
                &[0.2, 0.3, 0.5]
            ),
            ClassifierAnswer::Score { score, .. } if (score - 1.3).abs() < 1e-12
        ));
        assert_eq!(
            llama_server_root("http://127.0.0.1:8080/v1/"),
            "http://127.0.0.1:8080"
        );
    }
}
