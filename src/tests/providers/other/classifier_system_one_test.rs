//! v0.99.1 TypeSafe and Cloudflare System One production transport coverage.

#[cfg(test)]
mod tests {
    use crate::classifiers::{ClassifierOptions, classify, system_one};
    use crate::types::{
        ClassifierAnswer, ClassifierBoolCriteria, ClassifierContext, ClassifierQuestion,
        ClassifierStopReason,
    };
    use indexmap::IndexMap;
    use serde_json::json;
    use std::collections::HashMap;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn context() -> ClassifierContext {
        ClassifierContext {
            images: vec![],
            state: serde_json::from_value(json!({"text":"Deployment succeeded"})).unwrap(),
            questions: IndexMap::from([
                (
                    "category".into(),
                    ClassifierQuestion::Choice {
                        instructions: "Classify".into(),
                        criteria: IndexMap::from([
                            ("success".into(), "Successful".into()),
                            ("failure".into(), "Failed".into()),
                        ]),
                    },
                ),
                (
                    "score".into(),
                    ClassifierQuestion::Score {
                        instructions: "Score".into(),
                        criteria: vec!["low".into(), "high".into()],
                    },
                ),
                (
                    "approved".into(),
                    ClassifierQuestion::Bool {
                        instructions: "Approved?".into(),
                        criteria: ClassifierBoolCriteria {
                            true_value: "Yes".into(),
                            false_value: "No".into(),
                        },
                    },
                ),
            ]),
        }
    }

    fn answers() -> serde_json::Value {
        json!({
            "category": {"type":"choice","choice":"success","probabilities":{"success":0.9,"failure":0.1},"confidence":0.8},
            "score": {"type":"score","score":1,"confidence":0.7},
            "approved": {"type":"noul","noul":0.95}
        })
    }

    #[tokio::test]
    async fn typesafe_maps_bool_questions_parses_answers_and_prices_usage() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/systemone"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "application/json")
                    .set_body_json(json!({"answers":answers(),"usage":{"input_tokens":308,"output_tokens":23}})),
            )
            .mount(&server)
            .await;
        let mut model = crate::classifiers::get_classifier_model("typesafe", "jev-latest").unwrap();
        model.base_url = format!("{}/v1/", server.uri());
        model.cost.input = 0.042;
        let result = classify(
            &model,
            &context(),
            &ClassifierOptions {
                api_key: Some("secret".into()),
                temperature: Some(1.5),
                ..Default::default()
            },
        )
        .await;
        assert_eq!(result.stop_reason, ClassifierStopReason::Stop);
        assert!(matches!(
            result.answers["approved"],
            ClassifierAnswer::Bool { probability } if (probability - 0.95).abs() < 1e-12
        ));
        let ClassifierAnswer::Choice {
            choice,
            probabilities,
            confidence,
        } = &result.answers["category"]
        else {
            panic!("choice answer")
        };
        assert_eq!(choice, "success");
        assert!((probabilities["success"] - 0.9).abs() < 1e-12);
        assert!((probabilities["failure"] - 0.1).abs() < 1e-12);
        assert!((*confidence - 0.8).abs() < 1e-12);
        assert!(
            matches!(result.answers["score"], ClassifierAnswer::Score { score, confidence } if (score - 1.0).abs() < 1e-12 && (confidence - 0.7).abs() < 1e-12)
        );
        assert_eq!(
            serde_json::to_value(&result.answers["approved"]).unwrap(),
            json!({"type":"bool","probability":0.95})
        );
        assert_eq!(result.usage.as_ref().unwrap().total_tokens, 331);
        assert!((result.usage.unwrap().cost.total - 0.000012936).abs() < 1e-12);
        let requests = server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(
            requests[0].headers["authorization"].to_str().unwrap(),
            "Bearer secret"
        );
        let body: serde_json::Value = requests[0].body_json().unwrap();
        assert_eq!(body["model"], "jev-latest");
        assert_eq!(body["state"], json!({"text":"Deployment succeeded"}));
        assert_eq!(body["questions"]["approved"]["type"], "noul");
        assert_eq!(body["questions"]["category"]["instructions"], "Classify");
        assert_eq!(
            body["questions"]["category"]["criteria"]["success"],
            "Successful"
        );
        assert_eq!(
            body["questions"]["category"]["criteria"]["failure"],
            "Failed"
        );
        assert_eq!(
            body["questions"]["score"]["criteria"],
            json!(["low", "high"])
        );
        assert!(body.get("temperature").is_none());
    }

    #[tokio::test]
    async fn system_one_cancellation_covers_stalled_success_body() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (sent, received) = tokio::sync::oneshot::channel();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = [0; 8192];
            let read = socket.read(&mut request).await.unwrap();
            assert!(read > 0, "HTTP request arrived before response headers");
            socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 1000\r\n\r\n{").await.unwrap();
            sent.send(()).unwrap();
            std::future::pending::<()>().await;
        });
        let mut model = crate::classifiers::get_classifier_model("typesafe", "jev-latest").unwrap();
        model.base_url = format!("http://{address}/v1");
        let context = context();
        let (tx, rx) = tokio::sync::watch::channel(false);
        let options = ClassifierOptions {
            api_key: Some("test-key".into()),
            cancel: Some(rx),
            ..Default::default()
        };
        let mut operation = Box::pin(classify(&model, &context, &options));
        tokio::select! { _ = received => {}, result = &mut operation => panic!("settled early: {result:?}") }
        tokio::select! { _ = tokio::time::sleep(std::time::Duration::from_millis(20)) => {}, result = &mut operation => panic!("stalled body settled: {result:?}") }
        tx.send(true).unwrap();
        assert_eq!(
            tokio::time::timeout(std::time::Duration::from_secs(1), operation)
                .await
                .unwrap()
                .stop_reason,
            ClassifierStopReason::Aborted
        );
        server.abort();
        let _ = server.await;
    }

    #[tokio::test]
    async fn system_one_usage_uses_prompt_length_pricing_tier() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/systemone"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "answers": answers(),
                "usage":{"input_tokens":200,"output_tokens":10}
            })))
            .mount(&server)
            .await;
        let mut model = crate::classifiers::get_classifier_model("typesafe", "jev-latest").unwrap();
        model.base_url = format!("{}/v1", server.uri());
        model.cost.output = 2.0;
        model.cost.tiers = vec![crate::types::ModelCostTier {
            input_tokens_above: 100,
            input: 1.0,
            output: 3.0,
            cache_read: 0.0,
            cache_write: 0.0,
        }];
        let result = classify(
            &model,
            &context(),
            &ClassifierOptions {
                api_key: Some("test-key".into()),
                ..Default::default()
            },
        )
        .await;
        assert_eq!(result.stop_reason, ClassifierStopReason::Stop);
        let usage = result.usage.unwrap();
        assert!((usage.cost.input - 0.0002).abs() < 1e-12);
        assert!((usage.cost.output - 0.00003).abs() < 1e-12);
        assert!((usage.cost.total - 0.00023).abs() < 1e-12);
    }

    #[tokio::test]
    async fn malformed_answers_keep_billed_usage() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "application/json")
                    .set_body_json(
                        json!({"answers":{},"usage":{"input_tokens":10,"output_tokens":2}}),
                    ),
            )
            .mount(&server)
            .await;
        let mut model = crate::classifiers::get_classifier_model("typesafe", "jev-latest").unwrap();
        model.base_url = server.uri();
        let result = system_one::classify_typesafe(
            &model,
            &context(),
            &ClassifierOptions {
                api_key: Some("secret".into()),
                ..Default::default()
            },
        )
        .await;
        assert_eq!(result.stop_reason, ClassifierStopReason::Error);
        assert_eq!(result.usage.unwrap().total_tokens, 12);
        assert!(result.error_message.unwrap().contains("category"));
    }

    #[tokio::test]
    async fn cloudflare_uses_account_scoped_run_envelope_and_reports_errors() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/client/v4/accounts/account-id/ai/run"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "application/json")
                    .set_body_json(json!({
                        "success":true,
                        "result":{"state":"Completed","result":{"answers":answers(),"usage":{"input_tokens":426,"output_tokens":73}}},
                        "errors":[],"messages":[]
                    })),
            )
            .mount(&server)
            .await;
        let mut model =
            crate::classifiers::get_classifier_model("cloudflare-workers-ai", "typesafe/jev")
                .unwrap();
        model.base_url = format!(
            "{}/client/v4/accounts/{{CLOUDFLARE_ACCOUNT_ID}}/ai",
            server.uri()
        );
        let result = classify(
            &model,
            &context(),
            &ClassifierOptions {
                api_key: Some("cf-key".into()),
                env: Some(HashMap::from([(
                    "CLOUDFLARE_ACCOUNT_ID".into(),
                    "account-id".into(),
                )])),
                ..Default::default()
            },
        )
        .await;
        assert_eq!(result.stop_reason, ClassifierStopReason::Stop);
        assert_eq!(result.usage.unwrap().total_tokens, 499);

        Mock::given(method("POST"))
            .and(path("/client/v4/accounts/direct/ai/run"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "application/json")
                    .set_body_json(json!({
                        "success": true,
                        "result": {"answers": answers(), "usage": {"input_tokens": 12, "output_tokens": 3}},
                        "errors": [], "messages": []
                    })),
            )
            .mount(&server)
            .await;
        let mut direct = crate::classifiers::get_classifier_model(
            "cloudflare-workers-ai",
            "@cf/cloudflare/clef",
        )
        .unwrap();
        direct.base_url = format!(
            "{}/client/v4/accounts/{{CLOUDFLARE_ACCOUNT_ID}}/ai",
            server.uri()
        );
        let direct_result = system_one::classify_cloudflare(
            &direct,
            &context(),
            &ClassifierOptions {
                api_key: Some("cf-key".into()),
                env: Some(HashMap::from([(
                    "CLOUDFLARE_ACCOUNT_ID".into(),
                    "direct".into(),
                )])),
                ..Default::default()
            },
        )
        .await;
        assert_eq!(direct_result.stop_reason, ClassifierStopReason::Stop);
        assert_eq!(direct_result.usage.unwrap().total_tokens, 15);

        let queued = system_one::classify_cloudflare(
            &model,
            &context(),
            &ClassifierOptions {
                api_key: Some("cf-key".into()),
                env: Some(HashMap::from([(
                    "CLOUDFLARE_ACCOUNT_ID".into(),
                    "missing".into(),
                )])),
                max_retries: Some(0),
                ..Default::default()
            },
        )
        .await;
        assert_eq!(queued.stop_reason, ClassifierStopReason::Error);
    }
}
