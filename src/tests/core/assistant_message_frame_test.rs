//! Deterministic Rust port of upstream `test/assistant-message-frame.test.ts`.

use crate::assistant_message_frame::{
    AssistantMessageEvent, AssistantMessageFrame, AssistantMessageFrameEncoder,
    reduce_assistant_message_frames,
};
use crate::types::{
    AssistantMessageDiagnostic, ContentBlock, CostBreakdown, DiagnosticError, Message, Role,
    StopReason, Usage,
};
use serde_json::json;
use std::collections::HashMap;

fn seed() -> Message {
    Message {
        role: Role::Assistant,
        content: Vec::new(),
        api: Some("test-api".into()),
        provider: Some("test-provider".into()),
        model: Some("test-model".into()),
        response_id: None,
        response_model: None,
        provider_thinking_level: None,
        diagnostics: Vec::new(),
        usage: Some(Usage {
            input: 0,
            output: 0,
            cache_read: 0,
            cache_write: 0,
            cache_write_1h: None,
            reasoning: None,
            total_tokens: 0,
            cost: CostBreakdown::default(),
        }),
        stop_reason: Some(StopReason::Pending),
        timestamp: 1,
        deferred: None,
        error_message: None,
        raw_stop_reason: None,
        end_turn: None,
        tool_call_id: None,
        tool_name: None,
        is_error: false,
        details: None,
        added_tool_names: Vec::new(),
    }
}

fn encode(
    encoder: &mut AssistantMessageFrameEncoder,
    event: AssistantMessageEvent,
) -> AssistantMessageFrame {
    encoder
        .encode(event)
        .unwrap()
        .expect("event should produce a frame")
}

fn reduce(frames: Vec<AssistantMessageFrame>) -> Message {
    reduce_assistant_message_frames(frames)
        .unwrap()
        .expect("start frame")
}

fn as_json(value: impl serde::Serialize) -> serde_json::Value {
    serde_json::to_value(value).unwrap()
}

fn args(entries: &[(&str, serde_json::Value)]) -> HashMap<String, serde_json::Value> {
    entries
        .iter()
        .map(|(key, value)| ((*key).to_string(), value.clone()))
        .collect()
}

#[test]
fn uses_authoritative_text_end_content_and_signature() {
    let mut partial = seed();
    let mut encoder = AssistantMessageFrameEncoder::new();
    let mut frames = vec![encode(
        &mut encoder,
        AssistantMessageEvent::Start {
            partial: partial.clone(),
        },
    )];
    partial.content.push(ContentBlock::Text {
        text: "Hello ".into(),
        text_signature: None,
    });
    frames.push(encode(
        &mut encoder,
        AssistantMessageEvent::TextStart {
            content_index: 0,
            partial: partial.clone(),
        },
    ));
    partial.content[0] = ContentBlock::Text {
        text: "Hello world".into(),
        text_signature: Some("sig-text".into()),
    };
    frames.push(encode(
        &mut encoder,
        AssistantMessageEvent::TextDelta {
            content_index: 0,
            delta: "incorrect".into(),
            partial: partial.clone(),
        },
    ));
    frames.push(encode(
        &mut encoder,
        AssistantMessageEvent::TextEnd {
            content_index: 0,
            content: "Hello world".into(),
            partial: partial.clone(),
        },
    ));

    assert_eq!(
        as_json(frames.last().unwrap()),
        json!({"type":"text_end","contentIndex":0,"content":"Hello world","textSignature":"sig-text"})
    );
    assert_eq!(
        as_json(reduce(frames).content),
        json!([{"type":"text","text":"Hello world","text_signature":"sig-text"}])
    );
}

#[test]
fn preserves_provider_thinking_level_and_start_metadata() {
    let mut partial = seed();
    partial.provider_thinking_level = Some("high".into());
    let mut encoder = AssistantMessageFrameEncoder::new();
    let start = encode(&mut encoder, AssistantMessageEvent::Start { partial });
    assert_eq!(
        as_json(&start)["partial"]["providerThinkingLevel"],
        json!("high")
    );
    assert_eq!(
        reduce_assistant_message_frames(vec![start])
            .unwrap()
            .unwrap()
            .provider_thinking_level
            .as_deref(),
        Some("high")
    );
}

#[test]
fn preserves_initial_and_final_thinking_metadata_including_redaction() {
    let mut partial = seed();
    let mut encoder = AssistantMessageFrameEncoder::new();
    let mut frames = vec![encode(
        &mut encoder,
        AssistantMessageEvent::Start {
            partial: partial.clone(),
        },
    )];
    partial.content.push(ContentBlock::Thinking {
        thinking: "[redacted]".into(),
        thinking_signature: Some("encrypted-start".into()),
        redacted: true,
    });
    frames.push(encode(
        &mut encoder,
        AssistantMessageEvent::ThinkingStart {
            content_index: 0,
            partial: partial.clone(),
        },
    ));
    partial.content[0] = ContentBlock::Thinking {
        thinking: "[redacted]".into(),
        thinking_signature: Some("encrypted-final".into()),
        redacted: true,
    };
    frames.push(encode(
        &mut encoder,
        AssistantMessageEvent::ThinkingEnd {
            content_index: 0,
            content: "[redacted]".into(),
            partial,
        },
    ));

    assert_eq!(
        as_json(frames.last().unwrap()),
        json!({"type":"thinking_end","contentIndex":0,"content":"[redacted]","thinkingSignature":"encrypted-final","redacted":true})
    );
    assert_eq!(
        as_json(reduce(frames).content[0].clone()),
        json!({"type":"thinking","thinking":"[redacted]","thinking_signature":"encrypted-final","redacted":true})
    );
}

#[test]
fn parses_unfinished_tool_json_and_uses_authoritative_completed_arguments() {
    let initial_frames = vec![
        AssistantMessageFrame::Start {
            partial: Box::new(seed()),
        },
        AssistantMessageFrame::ToolCallStart {
            content_index: 0,
            tool_call: ContentBlock::ToolCall {
                id: "initial-id".into(),
                name: "write".into(),
                arguments: HashMap::new(),
                thought_signature: None,
                namespace: None,
            },
        },
        AssistantMessageFrame::ToolCallDelta {
            content_index: 0,
            delta: "{\"path\":\"READ".into(),
        },
    ];
    assert_eq!(
        as_json(reduce(initial_frames.clone()).content[0].clone())["arguments"]["path"],
        json!("READ")
    );

    let mut complete_frames = initial_frames;
    complete_frames.extend([
        AssistantMessageFrame::ToolCallDelta {
            content_index: 0,
            delta: "ME.md\",\"lines\":[1,2]}".into(),
        },
        AssistantMessageFrame::ToolCallEnd {
            content_index: 0,
            id: "final-id".into(),
            name: "write_file".into(),
            arguments: args(&[("path", json!("final.md")), ("lines", json!([3]))]),
            thought_signature: Some("thought".into()),
            namespace: Some("files".into()),
        },
    ]);
    assert_eq!(
        as_json(reduce(complete_frames).content[0].clone()),
        json!({"type":"toolCall","id":"final-id","name":"write_file","arguments":{"path":"final.md","lines":[3]},"thought_signature":"thought","namespace":"files"})
    );
}

#[test]
fn reconciles_queued_text_events_against_advanced_live_partial() {
    let mut live_partial = seed();
    live_partial.content.push(ContentBlock::Text {
        text: "Hello world".into(),
        text_signature: None,
    });
    let events = vec![
        AssistantMessageEvent::Start { partial: seed() },
        AssistantMessageEvent::TextStart {
            content_index: 0,
            partial: live_partial.clone(),
        },
        AssistantMessageEvent::TextDelta {
            content_index: 0,
            delta: "Hel".into(),
            partial: live_partial.clone(),
        },
        AssistantMessageEvent::TextDelta {
            content_index: 0,
            delta: "lo".into(),
            partial: live_partial.clone(),
        },
        AssistantMessageEvent::TextDelta {
            content_index: 0,
            delta: " ".into(),
            partial: live_partial.clone(),
        },
        AssistantMessageEvent::TextDelta {
            content_index: 0,
            delta: "world".into(),
            partial: live_partial,
        },
    ];

    let mut encoder = AssistantMessageFrameEncoder::new();
    let frames = events
        .into_iter()
        .filter_map(|event| encoder.encode(event).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        frames
            .iter()
            .map(|frame| as_json(frame)["type"].clone())
            .collect::<Vec<_>>(),
        vec![json!("start"), json!("text_start")]
    );
    assert_eq!(
        as_json(reduce(frames).content),
        json!([{"type":"text","text":"Hello world"}])
    );
}

#[test]
fn trims_only_covered_prefix_when_snapshot_lands_inside_delta() {
    let mut partial = seed();
    let mut encoder = AssistantMessageFrameEncoder::new();
    let mut frames = vec![encode(
        &mut encoder,
        AssistantMessageEvent::Start {
            partial: partial.clone(),
        },
    )];
    partial.content.push(ContentBlock::Text {
        text: "Hel".into(),
        text_signature: None,
    });
    frames.push(encode(
        &mut encoder,
        AssistantMessageEvent::TextStart {
            content_index: 0,
            partial: partial.clone(),
        },
    ));
    assert!(
        encoder
            .encode(AssistantMessageEvent::TextDelta {
                content_index: 0,
                delta: "He".into(),
                partial: partial.clone(),
            })
            .unwrap()
            .is_none()
    );
    let remainder = encode(
        &mut encoder,
        AssistantMessageEvent::TextDelta {
            content_index: 0,
            delta: "llo".into(),
            partial,
        },
    );
    assert_eq!(
        as_json(&remainder),
        json!({"type":"text_delta","contentIndex":0,"delta":"lo"})
    );
    frames.push(remainder);
    assert_eq!(
        as_json(reduce(frames).content),
        json!([{"type":"text","text":"Hello"}])
    );
}

#[test]
fn checkpoints_queued_tool_json_and_resumes_legacy_grammar() {
    let mut partial = seed();
    partial.content.push(ContentBlock::ToolCall {
        id: "call".into(),
        name: "write".into(),
        arguments: args(&[("path", json!("README.md"))]),
        thought_signature: None,
        namespace: None,
    });
    let events = vec![
        AssistantMessageEvent::Start { partial: seed() },
        AssistantMessageEvent::ToolCallStart {
            content_index: 0,
            partial: partial.clone(),
        },
        AssistantMessageEvent::ToolCallDelta {
            content_index: 0,
            delta: "{\"path\":\"READ".into(),
            partial: partial.clone(),
        },
        AssistantMessageEvent::ToolCallDelta {
            content_index: 0,
            delta: "ME.md\"}".into(),
            partial,
        },
    ];

    let mut encoder = AssistantMessageFrameEncoder::new();
    let frames = events
        .into_iter()
        .filter_map(|event| encoder.encode(event).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        as_json(frames.last().unwrap()),
        json!({"type":"toolcall_checkpoint","contentIndex":0,"json":"{\"path\":\"README.md\"}"})
    );
    assert_eq!(
        as_json(reduce(frames).content[0].clone())["arguments"],
        json!({"path":"README.md"})
    );

    let mut partial = seed();
    let mut encoder = AssistantMessageFrameEncoder::new();
    let mut frames = vec![encode(
        &mut encoder,
        AssistantMessageEvent::Start {
            partial: partial.clone(),
        },
    )];
    partial.content.push(ContentBlock::ToolCall {
        id: "call".into(),
        name: "bash".into(),
        arguments: args(&[("input", json!("a"))]),
        thought_signature: None,
        namespace: None,
    });
    frames.push(encode(
        &mut encoder,
        AssistantMessageEvent::ToolCallStart {
            content_index: 0,
            partial: partial.clone(),
        },
    ));
    partial.content[0] = ContentBlock::ToolCall {
        id: "call".into(),
        name: "bash".into(),
        arguments: args(&[("input", json!("ab"))]),
        thought_signature: None,
        namespace: None,
    };
    frames.push(encode(
        &mut encoder,
        AssistantMessageEvent::ToolCallDelta {
            content_index: 0,
            delta: "{\"input\":\"ab".into(),
            partial: partial.clone(),
        },
    ));
    partial.content[0] = ContentBlock::ToolCall {
        id: "call".into(),
        name: "bash".into(),
        arguments: args(&[("input", json!("abc"))]),
        thought_signature: None,
        namespace: None,
    };
    frames.push(encode(
        &mut encoder,
        AssistantMessageEvent::ToolCallDelta {
            content_index: 0,
            delta: "c\"}".into(),
            partial,
        },
    ));
    assert_eq!(
        as_json(frames[2].clone()),
        json!({"type":"toolcall_checkpoint","contentIndex":0,"json":"{\"input\":\"ab"})
    );
    assert_eq!(
        as_json(frames[3].clone()),
        json!({"type":"toolcall_delta","contentIndex":0,"delta":"c\"}"})
    );
    assert_eq!(
        as_json(reduce(frames).content[0].clone())["arguments"],
        json!({"input":"abc"})
    );
}

#[test]
fn streams_tool_json_compactly_from_empty_argument_start() {
    let mut partial = seed();
    let mut encoder = AssistantMessageFrameEncoder::new();
    let mut frames = vec![encode(
        &mut encoder,
        AssistantMessageEvent::Start {
            partial: partial.clone(),
        },
    )];
    partial.content.push(ContentBlock::ToolCall {
        id: "call".into(),
        name: "bash".into(),
        arguments: HashMap::new(),
        thought_signature: None,
        namespace: None,
    });
    frames.push(encode(
        &mut encoder,
        AssistantMessageEvent::ToolCallStart {
            content_index: 0,
            partial: partial.clone(),
        },
    ));
    partial.content[0] = ContentBlock::ToolCall {
        id: "call".into(),
        name: "bash".into(),
        arguments: args(&[("command", json!("ls -la /tmp"))]),
        thought_signature: None,
        namespace: None,
    };
    frames.push(encode(
        &mut encoder,
        AssistantMessageEvent::ToolCallDelta {
            content_index: 0,
            delta: "{\"command\":\"ls -la /tmp\"}".into(),
            partial,
        },
    ));
    assert_eq!(
        as_json(frames.last().unwrap()),
        json!({"type":"toolcall_delta","contentIndex":0,"delta":"{\"command\":\"ls -la /tmp\"}"})
    );
    assert_eq!(
        as_json(reduce(frames).content[0].clone())["arguments"],
        json!({"command":"ls -la /tmp"})
    );
}

#[test]
fn accepts_pre_generation_error_but_rejects_success_or_updates_before_start() {
    let mut failed = seed();
    failed.stop_reason = Some(StopReason::Error);
    failed.error_message = Some("setup failed".into());
    assert!(
        AssistantMessageFrameEncoder::new()
            .encode(AssistantMessageEvent::Error {
                reason: StopReason::Error,
                error: failed,
            })
            .unwrap()
            .is_none()
    );

    let mut completed = seed();
    completed.stop_reason = Some(StopReason::Stop);
    assert!(
        AssistantMessageFrameEncoder::new()
            .encode(AssistantMessageEvent::Done {
                reason: StopReason::Stop,
                message: completed,
            })
            .unwrap_err()
            .contains("done event appears before start")
    );
    assert!(
        AssistantMessageFrameEncoder::new()
            .encode(AssistantMessageEvent::TextDelta {
                content_index: 0,
                delta: "x".into(),
                partial: seed(),
            })
            .unwrap_err()
            .contains("text_delta event appears before start")
    );
}

#[test]
fn end_signature_metadata_and_tool_arguments_are_authoritative() {
    let frames = vec![
        AssistantMessageFrame::Start {
            partial: Box::new(seed()),
        },
        AssistantMessageFrame::TextStart {
            content_index: 0,
            content: ContentBlock::Text {
                text: String::new(),
                text_signature: Some("stale-text".into()),
            },
        },
        AssistantMessageFrame::TextEnd {
            content_index: 0,
            content: String::new(),
            text_signature: None,
        },
        AssistantMessageFrame::ThinkingStart {
            content_index: 1,
            content: ContentBlock::Thinking {
                thinking: String::new(),
                thinking_signature: Some("stale-thinking".into()),
                redacted: true,
            },
        },
        AssistantMessageFrame::ThinkingEnd {
            content_index: 1,
            content: String::new(),
            thinking_signature: Some(String::new()),
            redacted: Some(false),
        },
        AssistantMessageFrame::ToolCallStart {
            content_index: 2,
            tool_call: ContentBlock::ToolCall {
                id: "call".into(),
                name: "read".into(),
                arguments: HashMap::new(),
                thought_signature: Some("stale-tool".into()),
                namespace: Some("stale-namespace".into()),
            },
        },
        AssistantMessageFrame::ToolCallEnd {
            content_index: 2,
            id: "final".into(),
            name: "read_file".into(),
            arguments: args(&[("path", json!("final.md"))]),
            thought_signature: None,
            namespace: None,
        },
    ];
    assert_eq!(
        as_json(reduce(frames).content),
        json!([
            {"type":"text","text":""},
            {"type":"thinking","thinking":"","thinking_signature":"","redacted":false},
            {"type":"toolCall","id":"final","name":"read_file","arguments":{"path":"final.md"}}
        ])
    );
}

#[test]
fn supports_interleaved_streams_by_content_index() {
    let frames = vec![
        AssistantMessageFrame::Start {
            partial: Box::new(seed()),
        },
        AssistantMessageFrame::TextStart {
            content_index: 0,
            content: ContentBlock::Text {
                text: String::new(),
                text_signature: None,
            },
        },
        AssistantMessageFrame::ToolCallStart {
            content_index: 1,
            tool_call: ContentBlock::ToolCall {
                id: "call".into(),
                name: "lookup".into(),
                arguments: HashMap::new(),
                thought_signature: None,
                namespace: None,
            },
        },
        AssistantMessageFrame::ThinkingStart {
            content_index: 2,
            content: ContentBlock::Thinking {
                thinking: String::new(),
                thinking_signature: None,
                redacted: false,
            },
        },
        AssistantMessageFrame::TextDelta {
            content_index: 0,
            delta: "answer".into(),
        },
        AssistantMessageFrame::ToolCallDelta {
            content_index: 1,
            delta: "{\"query\":\"pi\"}".into(),
        },
        AssistantMessageFrame::ThinkingDelta {
            content_index: 2,
            delta: "check".into(),
        },
        AssistantMessageFrame::ToolCallEnd {
            content_index: 1,
            id: "call".into(),
            name: "lookup".into(),
            arguments: args(&[("query", json!("pi"))]),
            thought_signature: None,
            namespace: None,
        },
        AssistantMessageFrame::TextEnd {
            content_index: 0,
            content: "answer".into(),
            text_signature: None,
        },
        AssistantMessageFrame::ThinkingEnd {
            content_index: 2,
            content: "check".into(),
            thinking_signature: None,
            redacted: None,
        },
    ];
    assert_eq!(
        as_json(reduce(frames).content),
        json!([
            {"type":"text","text":"answer"},
            {"type":"toolCall","id":"call","name":"lookup","arguments":{"query":"pi"}},
            {"type":"thinking","thinking":"check","redacted":false}
        ])
    );
}

#[test]
fn snapshots_mutable_event_data_and_keeps_reduction_pure() {
    let mut partial = seed();
    partial.diagnostics = vec![AssistantMessageDiagnostic {
        diagnostic_type: "test".into(),
        timestamp: 2,
        error: DiagnosticError {
            name: None,
            message: "ok".into(),
            stack: None,
            code: None,
        },
        details: Some(args(&[("value", json!("original"))])),
    }];
    let mut encoder = AssistantMessageFrameEncoder::new();
    let start = encode(
        &mut encoder,
        AssistantMessageEvent::Start {
            partial: partial.clone(),
        },
    );
    partial.diagnostics[0]
        .details
        .as_mut()
        .unwrap()
        .insert("value".into(), json!("mutated"));
    partial.usage.as_mut().unwrap().cost.total = 99.0;

    partial.content.push(ContentBlock::ToolCall {
        id: "call".into(),
        name: "run".into(),
        arguments: args(&[("nested", json!({"value":"original"}))]),
        thought_signature: None,
        namespace: None,
    });
    let tool_start = encode(
        &mut encoder,
        AssistantMessageEvent::ToolCallStart {
            content_index: 0,
            partial: partial.clone(),
        },
    );
    if let ContentBlock::ToolCall { arguments, .. } = &mut partial.content[0] {
        arguments.insert("nested".into(), json!({"value":"mutated"}));
    }

    let mut reduced = reduce(vec![start, tool_start.clone()]);
    assert_eq!(
        reduced.diagnostics[0].details.as_ref().unwrap()["value"],
        json!("original")
    );
    assert_eq!(reduced.usage.as_ref().unwrap().cost.total, 0.0);
    assert_eq!(
        as_json(reduced.content[0].clone())["arguments"]["nested"],
        json!({"value":"original"})
    );
    if let ContentBlock::ToolCall { arguments, .. } = &mut reduced.content[0] {
        arguments.insert("nested".into(), json!("changed-output"));
    }
    assert_eq!(
        as_json(tool_start)["toolCall"]["arguments"]["nested"],
        json!({"value":"original"})
    );
}

#[test]
fn omits_terminal_events_and_tracks_terminal_state() {
    let mut message = seed();
    let mut encoder = AssistantMessageFrameEncoder::new();
    assert!(
        encoder
            .encode(AssistantMessageEvent::Start {
                partial: message.clone()
            })
            .unwrap()
            .is_some()
    );
    message.stop_reason = Some(StopReason::Stop);
    assert!(
        encoder
            .encode(AssistantMessageEvent::Done {
                reason: StopReason::Stop,
                message
            })
            .unwrap()
            .is_none()
    );
    assert!(
        encoder
            .encode(AssistantMessageEvent::TextDelta {
                content_index: 0,
                delta: "late".into(),
                partial: seed()
            })
            .unwrap_err()
            .contains("follows a terminal event")
    );
}

#[test]
fn returns_none_without_start_and_rejects_bad_frame_sequences() {
    assert!(
        reduce_assistant_message_frames(Vec::<AssistantMessageFrame>::new())
            .unwrap()
            .is_none()
    );
    assert!(
        reduce_assistant_message_frames(vec![AssistantMessageFrame::TextDelta {
            content_index: 0,
            delta: "x".into(),
        }])
        .unwrap()
        .is_none()
    );
    assert!(
        reduce_assistant_message_frames(vec![
            AssistantMessageFrame::TextDelta {
                content_index: 0,
                delta: "x".into(),
            },
            AssistantMessageFrame::Start {
                partial: Box::new(seed())
            },
        ])
        .unwrap_err()
        .contains("before the start frame")
    );
    assert!(
        reduce_assistant_message_frames(vec![
            AssistantMessageFrame::Start {
                partial: Box::new(seed())
            },
            AssistantMessageFrame::ToolCallStart {
                content_index: 0,
                tool_call: ContentBlock::ToolCall {
                    id: "call".into(),
                    name: "run".into(),
                    arguments: HashMap::new(),
                    thought_signature: None,
                    namespace: None,
                },
            },
            AssistantMessageFrame::TextDelta {
                content_index: 0,
                delta: "wrong".into(),
            },
        ])
        .unwrap_err()
        .contains("expected text block")
    );
    assert!(
        reduce_assistant_message_frames(vec![
            AssistantMessageFrame::Start {
                partial: Box::new(seed())
            },
            AssistantMessageFrame::TextStart {
                content_index: 0,
                content: ContentBlock::Text {
                    text: String::new(),
                    text_signature: None,
                },
            },
            AssistantMessageFrame::TextEnd {
                content_index: 0,
                content: String::new(),
                text_signature: None,
            },
            AssistantMessageFrame::TextEnd {
                content_index: 0,
                content: String::new(),
                text_signature: None,
            },
        ])
        .unwrap_err()
        .contains("follows the end")
    );
    assert!(
        reduce_assistant_message_frames(vec![
            AssistantMessageFrame::Start {
                partial: Box::new(seed())
            },
            AssistantMessageFrame::TextStart {
                content_index: 1,
                content: ContentBlock::Text {
                    text: String::new(),
                    text_signature: None,
                },
            },
        ])
        .unwrap_err()
        .contains("would leave a gap")
    );
}

#[test]
fn rejects_conversion_events_with_wrong_block_kind_or_duplicate_start() {
    let mut partial = seed();
    let mut encoder = AssistantMessageFrameEncoder::new();
    encoder
        .encode(AssistantMessageEvent::Start {
            partial: partial.clone(),
        })
        .unwrap();
    partial.content.push(ContentBlock::Thinking {
        thinking: String::new(),
        thinking_signature: None,
        redacted: false,
    });
    assert!(
        encoder
            .encode(AssistantMessageEvent::TextStart {
                content_index: 0,
                partial: partial.clone(),
            })
            .unwrap_err()
            .contains("text_start event points to thinking block")
    );

    let mut partial = seed();
    let mut encoder = AssistantMessageFrameEncoder::new();
    encoder
        .encode(AssistantMessageEvent::Start {
            partial: partial.clone(),
        })
        .unwrap();
    partial.content.push(ContentBlock::Text {
        text: String::new(),
        text_signature: None,
    });
    encoder
        .encode(AssistantMessageEvent::TextStart {
            content_index: 0,
            partial: partial.clone(),
        })
        .unwrap();
    assert!(
        encoder
            .encode(AssistantMessageEvent::TextStart {
                content_index: 0,
                partial,
            })
            .unwrap_err()
            .contains("starts more than once")
    );
}
