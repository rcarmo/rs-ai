//! Monotonic assistant response timing and forwarded-duration compatibility.
use crate::events::Event;
use crate::registry::{EventStream, time_assistant_stream};
use crate::types::{Message, Role, StopReason};
use futures::StreamExt;

fn message(timestamp: i64, duration: Option<u64>) -> Message {
    let mut message = crate::user_message("answer");
    message.role = Role::Assistant;
    message.timestamp = timestamp;
    message.duration_ms = duration;
    message.stop_reason = Some(StopReason::Stop);
    message
}
fn stream(message: Message, error: bool) -> EventStream<'static> {
    let event = if error {
        Event::Error {
            reason: StopReason::Error,
            error: std::sync::Arc::from(Box::<dyn std::error::Error + Send + Sync>::from(
                "fixture",
            )),
            message: Some(message),
        }
    } else {
        Event::Done {
            reason: StopReason::Stop,
            message,
        }
    };
    Box::pin(tokio_stream::once(event))
}
#[tokio::test]
async fn adds_duration_to_done_and_error_terminal_messages() {
    for error in [false, true] {
        // Future timestamp makes the upstream wall-clock admission condition deterministic.
        let mut source = time_assistant_stream(stream(
            message(crate::utils::now_millis() + 1000, None),
            error,
        ));
        let event = source.next().await.unwrap();
        let msg = match event {
            Event::Done { message, .. } => message,
            Event::Error {
                message: Some(message),
                ..
            } => message,
            _ => panic!("terminal"),
        };
        assert!(msg.duration_ms.is_some());
        assert!(
            serde_json::to_value(msg)
                .unwrap()
                .get("durationMs")
                .is_some()
        );
    }
}
#[test]
fn elapsed_duration_rounds_to_nearest_millisecond() {
    use std::time::Duration;
    assert_eq!(
        crate::registry::round_duration_ms(Duration::from_micros(499)),
        0
    );
    assert_eq!(
        crate::registry::round_duration_ms(Duration::from_micros(500)),
        1
    );
    assert_eq!(
        crate::registry::round_duration_ms(Duration::from_micros(1499)),
        1
    );
    assert_eq!(
        crate::registry::round_duration_ms(Duration::from_micros(1500)),
        2
    );
    assert_eq!(crate::registry::round_duration_ms(Duration::MAX), u64::MAX);
}

#[tokio::test]
async fn direct_provider_entry_points_time_terminal_messages() {
    use crate::types::{Context, StreamOptions};
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};
    let server = MockServer::start().await;
    Mock::given(method("POST")).respond_with(ResponseTemplate::new(200)
        .insert_header("content-type", "text/event-stream")
        .set_body_string("data: {\"choices\":[{\"delta\":{\"content\":\"ok\"},\"finish_reason\":\"stop\"}]}\n\ndata: [DONE]\n\n"))
        .mount(&server).await;
    let mut model = crate::registry::get_model("openai", "gpt-4o-mini").unwrap();
    model.base_url = server.uri();
    model.api_key = Some("test-key".into());
    let context = Context {
        system_prompt: None,
        messages: vec![],
        tools: vec![],
    };
    let options = StreamOptions::default();
    let events = crate::provider::openai::stream_openai(&model, &context, &options)
        .collect::<Vec<_>>()
        .await;
    let Some(Event::Done { message, .. }) = events.last() else {
        panic!("expected Done: {events:?}")
    };
    assert!(message.duration_ms.is_some());
    let events = crate::provider::faux::stream_faux_text("hello", &model)
        .collect::<Vec<_>>()
        .await;
    let Some(Event::Done { message, .. }) = events.last() else {
        panic!("expected Done: {events:?}")
    };
    assert!(message.duration_ms.is_some());
}

#[tokio::test]
async fn preserves_existing_duration_and_leaves_older_forwarded_response_untimed() {
    for (timestamp, duration) in [(0, None), (crate::utils::now_millis() + 1000, Some(1234))] {
        let event = time_assistant_stream(stream(message(timestamp, duration), false))
            .next()
            .await
            .unwrap();
        let Event::Done { message, .. } = event else {
            panic!("done")
        };
        assert_eq!(message.duration_ms, duration);
    }
}
