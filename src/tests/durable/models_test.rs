#[cfg(test)]
mod tests {
    use crate::durable::{DurableModels, RegistryModels};
    use crate::types::{
        ClassifierContext, ClassifierQuestion, ClassifierStopReason, Context, StreamOptions,
    };
    use serde_json::json;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn default_tool_models_service_dispatches_completion_and_classifier_http() {
        let server = MockServer::start().await;
        Mock::given(method("POST")).and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).insert_header("content-type", "text/event-stream")
                .set_body_string("data: {\"choices\":[{\"delta\":{\"content\":\"nested\"},\"finish_reason\":\"stop\"}]}\n\ndata: [DONE]\n\n"))
            .expect(1).mount(&server).await;
        Mock::given(method("POST"))
            .and(path("/v1/decisions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                json!({"answers":[{"name":"check","type":"predicate","probability":0.8}]}),
            ))
            .expect(1)
            .mount(&server)
            .await;
        let models = RegistryModels;
        let mut model = models.get_model("openai", "gpt-4o-mini").unwrap();
        model.api = "openai-completions".into();
        model.base_url = format!("{}/v1", server.uri());
        let context = Context {
            system_prompt: None,
            messages: vec![crate::user_message("nested")],
            tools: vec![],
        };
        let result = models
            .complete(
                &model,
                &context,
                &StreamOptions {
                    api_key: Some("test-key".into()),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert!(result.duration_ms.is_some());
        assert!(
            matches!(result.content.first(), Some(crate::types::ContentBlock::Text { text, .. }) if text == "nested")
        );
        let mut classifier =
            crate::classifiers::get_classifier_model("openai", "gpt-6-luna").unwrap();
        classifier.base_url = format!("{}/v1", server.uri());
        let result = models
            .classify(
                &classifier,
                &ClassifierContext {
                    state: json!({"nested":true}).as_object().unwrap().clone(),
                    images: vec![],
                    questions: indexmap::IndexMap::from([(
                        "check".into(),
                        ClassifierQuestion::Bool {
                            instructions: "check".into(),
                            criteria: crate::types::ClassifierBoolCriteria {
                                true_value: "yes".into(),
                                false_value: "no".into(),
                            },
                        },
                    )]),
                },
                &crate::classifiers::ClassifierOptions {
                    api_key: Some("test-key".into()),
                    ..Default::default()
                },
            )
            .await;
        assert_eq!(result.stop_reason, ClassifierStopReason::Stop);
        assert!(
            matches!(result.answers.get("check"), Some(crate::types::ClassifierAnswer::Bool { probability }) if *probability == 0.8)
        );
    }
}
