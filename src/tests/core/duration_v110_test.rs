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
