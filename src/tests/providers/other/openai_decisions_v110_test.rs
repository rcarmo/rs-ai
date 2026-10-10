//! Source-bound HTTP contracts from upstream v1.1.0 OpenAI Decisions tests.
use crate::classifiers::{ClassifierOptions, classify};
use crate::types::*;
use indexmap::IndexMap;
use serde_json::{Value, json};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn model(url: &str) -> ClassifierModel {
    ClassifierModel {
        model_type: ModelType::Classifier,
        id: "gpt-6-luna".into(),
        name: "GPT-6 Luna".into(),
        api: api::OPENAI_DECISIONS.into(),
        provider: "openai".into(),
        base_url: url.into(),
        input: vec!["text".into(), "image".into()],
        input_limits: None,
        cost: ModelCost {
            input: 0.1,
            tiers: vec![ModelCostTier {
                input_tokens_above: 272_000,
                input: 0.2,
                output: 0.0,
                cache_read: 0.0,
                cache_write: 0.0,
            }],
            ..Default::default()
        },
        context_window: 922_000,
        headers: None,
        api_key: None,
    }
}
fn context() -> ClassifierContext {
    ClassifierContext {
        state: serde_json::from_value(json!({"text":"The deployment succeeded, thank you."}))
            .unwrap(),
        questions: IndexMap::from([
            (
                "category".into(),
                ClassifierQuestion::Choice {
                    instructions: "Classify the message".into(),
                    criteria: IndexMap::from([
                        ("success".into(), "Successful".into()),
                        ("failure".into(), "".into()),
                    ]),
                },
            ),
            (
                "satisfaction".into(),
                ClassifierQuestion::Score {
                    instructions: "Score satisfaction".into(),
                    criteria: vec!["low".into(), "neutral".into(), "high".into()],
                },
            ),
            (
                "approved".into(),
                ClassifierQuestion::Bool {
                    instructions: "Does the user approve?".into(),
                    criteria: ClassifierBoolCriteria {
                        true_value: "Approval".into(),
                        false_value: "No approval".into(),
                    },
                },
            ),
        ]),
        images: vec![],
    }
}
fn answers() -> Value {
    json!([
        {"type":"predicate","name":"approved","probability":0.95},
        {"type":"score","name":"satisfaction","score":1.8,"confidence":0.7},
        {"type":"choice","name":"category","choice":"success","probabilities":[{"value":"success","probability":0.9},{"value":"failure","probability":0.1}],"confidence":0.8}
    ])
}
fn options() -> ClassifierOptions {
    ClassifierOptions {
        api_key: Some("secret".into()),
        ..Default::default()
    }
}

#[tokio::test]
async fn maps_questions_and_named_answers_and_usage_with_tier_pricing() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/decisions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            json!({"answers":answers(),"usage":{"input_tokens":300000,"output_tokens":0}}),
        ))
        .expect(1)
        .mount(&server)
        .await;
    let result = classify(
        &model(&format!("{}/v1", server.uri())),
        &context(),
        &options(),
    )
    .await;
    assert_eq!(result.stop_reason, ClassifierStopReason::Stop);
    assert!(
        matches!(result.answers["approved"], ClassifierAnswer::Bool { probability } if probability == 0.95)
    );
    assert!(
        matches!(result.answers["satisfaction"], ClassifierAnswer::Score { score, .. } if score == 1.8)
    );
    assert!((result.usage.unwrap().cost.total - 0.06).abs() < 1e-12);
    let requests = server.received_requests().await.unwrap();
    let body: Value = serde_json::from_slice(&requests[0].body).unwrap();
    assert_eq!(
        body["input"],
        serde_json::to_string(&context().state).unwrap()
    );
    assert_eq!(
        body["questions"],
        json!([
            {"type":"choice","name":"category","instructions":"Classify the message","choices":[{"value":"success","description":"Successful"},{"value":"failure"}]},
            {"type":"score","name":"satisfaction","instructions":"Score satisfaction","levels":[{"label":"low"},{"label":"neutral"},{"label":"high"}]},
            {"type":"predicate","name":"approved","instructions":"Does the user approve?\n\nTrue means: Approval\nFalse means: No approval"}
        ])
    );
    assert_eq!(
        requests[0].headers.get("authorization").unwrap(),
        "Bearer secret"
    );
}

#[tokio::test]
async fn image_input_is_ordered_and_over_limit_fails_before_http() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"answers":answers()})))
        .expect(1)
        .mount(&server)
        .await;
    let mut ctx = context();
    ctx.images = vec![ClassifierImage {
        model_type: "image".into(),
        data: "aW1hZ2U=".into(),
        mime_type: "image/png".into(),
    }];
    let m = model(&server.uri());
    assert_eq!(
        classify(&m, &ctx, &options()).await.stop_reason,
        ClassifierStopReason::Stop
    );
    let body: Value =
        serde_json::from_slice(&server.received_requests().await.unwrap()[0].body).unwrap();
    assert_eq!(body["input"][0]["content"][0]["type"], "input_text");
    assert_eq!(
        body["input"][0]["content"][1]["image_url"],
        "data:image/png;base64,aW1hZ2U="
    );
    ctx.images = vec![ctx.images[0].clone(); 129];
    let result = classify(&m, &ctx, &options()).await;
    assert_eq!(result.stop_reason, ClassifierStopReason::Error);
    assert!(
        result
            .error_message
            .unwrap()
            .contains("at most 128 images, got 129")
    );
    assert_eq!(server.received_requests().await.unwrap().len(), 1);
}

#[tokio::test]
async fn refusals_and_malformed_answers_retain_billed_usage_without_partial_answers() {
    for value in [
        json!([{"type":"refusal","name":"approved"}]),
        json!([{"type":"score","name":"approved"}]),
        json!([]),
    ] {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(json!({"answers":value,"usage":{"input_tokens":164}})),
            )
            .mount(&server)
            .await;
        let result = classify(&model(&server.uri()), &context(), &options()).await;
        assert_eq!(result.stop_reason, ClassifierStopReason::Error);
        assert!(result.answers.is_empty());
        assert_eq!(result.usage.unwrap().input, 164);
    }
}

#[tokio::test]
async fn preserves_prototype_sensitive_question_names() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            json!({"answers":[{"type":"predicate","name":"__proto__","probability":0.75}]}),
        ))
        .mount(&server)
        .await;
    let mut ctx = context();
    ctx.questions = IndexMap::from([("__proto__".into(), ctx.questions["approved"].clone())]);
    let result = classify(&model(&server.uri()), &ctx, &options()).await;
    assert_eq!(result.stop_reason, ClassifierStopReason::Stop);
    assert!(
        matches!(result.answers["__proto__"],ClassifierAnswer::Bool{probability} if probability==0.75)
    );
}

#[tokio::test]
async fn gateway_504_is_not_retried_but_503_is() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(504)
                .set_body_string("<html>Gateway time-out</html>")
                .insert_header("retry-after-ms", "0"),
        )
        .expect(1)
        .mount(&server)
        .await;
    let result = classify(&model(&server.uri()), &context(), &options()).await;
    assert!(
        result
            .error_message
            .as_ref()
            .unwrap()
            .contains("timed out at the gateway")
    );
    assert!(!result.error_message.unwrap().contains("<html>"));
    assert_eq!(server.received_requests().await.unwrap().len(), 1);
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(503).insert_header("retry-after-ms", "0"))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"answers":answers()})))
        .mount(&server)
        .await;
    assert_eq!(
        classify(&model(&server.uri()), &context(), &options())
            .await
            .stop_reason,
        ClassifierStopReason::Stop
    );
    assert_eq!(server.received_requests().await.unwrap().len(), 2);
}

#[tokio::test]
async fn cancellation_after_headers_aborts_stalled_response_body() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::sync::{oneshot, watch};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (sent, received) = oneshot::channel();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = [0u8; 8192];
        let _ = socket.read(&mut request).await.unwrap();
        socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 1000\r\nConnection: close\r\n\r\n{").await.unwrap();
        sent.send(()).unwrap();
        std::future::pending::<()>().await;
    });
    let (tx, rx) = watch::channel(false);
    let model = model(&format!("http://{address}"));
    let context = context();
    let options = ClassifierOptions {
        api_key: Some("test-key".into()),
        cancel: Some(rx),
        ..Default::default()
    };
    let mut request = Box::pin(classify(&model, &context, &options));
    tokio::select! { _ = received => {}, result = &mut request => panic!("request settled before body: {result:?}") }
    tokio::select! { _ = tokio::time::sleep(std::time::Duration::from_millis(20)) => {}, result = &mut request => panic!("request should await body: {result:?}") }
    tx.send(true).unwrap();
    let result = tokio::time::timeout(std::time::Duration::from_secs(1), request)
        .await
        .unwrap();
    assert_eq!(result.stop_reason, ClassifierStopReason::Aborted);
    assert!(result.answers.is_empty());
    server.abort();
    let _ = server.await;
}

#[tokio::test]
async fn model_capability_and_cancellation_fail_without_http() {
    let server = MockServer::start().await;
    let mut m = model(&server.uri());
    m.provider = "decision-test-no-env".into();
    let result = classify(&m, &context(), &ClassifierOptions::default()).await;
    assert!(result.error_message.unwrap().contains("No API key"));
    let mut ctx = context();
    ctx.images.push(ClassifierImage {
        model_type: "image".into(),
        data: "abc".into(),
        mime_type: "image/png".into(),
    });
    m.input = vec!["text".into()];
    assert!(
        classify(&m, &ctx, &options())
            .await
            .error_message
            .unwrap()
            .contains("does not support image input")
    );
    let (_, cancel) = tokio::sync::watch::channel(true);
    assert_eq!(
        classify(
            &m,
            &context(),
            &ClassifierOptions {
                cancel: Some(cancel),
                ..options()
            }
        )
        .await
        .stop_reason,
        ClassifierStopReason::Aborted
    );
    assert!(server.received_requests().await.unwrap().is_empty());
}
