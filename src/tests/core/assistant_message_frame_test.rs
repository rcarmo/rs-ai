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
        json!([{"type":"text","text":"Hello world","textSignature":"sig-text"}])
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
        redacted: Some(true),
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
        redacted: Some(true),
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
        json!({"type":"thinking","thinking":"[redacted]","thinkingSignature":"encrypted-final","redacted":true})
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
        json!({"type":"toolCall","id":"final-id","name":"write_file","arguments":{"path":"final.md","lines":[3]},"thoughtSignature":"thought","namespace":"files"})
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
                redacted: Some(true),
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
            {"type":"thinking","thinking":"","thinkingSignature":"","redacted":false},
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
                redacted: None,
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
            {"type":"thinking","thinking":"check"}
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
fn frame_golden_json_uses_upstream_camel_case_content_fields_and_round_trips() {
    let mut start_partial = seed();
    start_partial.provider_thinking_level = Some("high".into());
    let frames = vec![
        AssistantMessageFrame::Start {
            partial: Box::new(start_partial),
        },
        AssistantMessageFrame::TextStart {
            content_index: 0,
            content: ContentBlock::Text {
                text: "hi".into(),
                text_signature: Some("text-sig".into()),
            },
        },
        AssistantMessageFrame::TextDelta {
            content_index: 0,
            delta: "!".into(),
        },
        AssistantMessageFrame::TextEnd {
            content_index: 0,
            content: "hi!".into(),
            text_signature: Some("text-final".into()),
        },
        AssistantMessageFrame::ThinkingStart {
            content_index: 0,
            content: ContentBlock::Thinking {
                thinking: "think".into(),
                thinking_signature: Some("thinking-sig".into()),
                redacted: Some(true),
            },
        },
        AssistantMessageFrame::ThinkingDelta {
            content_index: 0,
            delta: " more".into(),
        },
        AssistantMessageFrame::ThinkingEnd {
            content_index: 0,
            content: "think more".into(),
            thinking_signature: Some("thinking-final".into()),
            redacted: Some(false),
        },
        AssistantMessageFrame::ToolCallStart {
            content_index: 0,
            tool_call: ContentBlock::ToolCall {
                id: "call".into(),
                name: "run".into(),
                arguments: args(&[("path", json!("README.md"))]),
                thought_signature: Some("thought-sig".into()),
                namespace: Some("tools".into()),
            },
        },
        AssistantMessageFrame::ToolCallCheckpoint {
            content_index: 0,
            json: "{\"path\":\"README.md\"}".into(),
        },
        AssistantMessageFrame::ToolCallDelta {
            content_index: 0,
            delta: "{}".into(),
        },
        AssistantMessageFrame::ToolCallEnd {
            content_index: 0,
            id: "call-final".into(),
            name: "run_final".into(),
            arguments: args(&[("path", json!("final.md"))]),
            thought_signature: Some("thought-final".into()),
            namespace: Some("files".into()),
        },
    ];
    let golden = json!([
        {"type":"start","partial":{"role":"assistant","content":[],"timestamp":1,"api":"test-api","provider":"test-provider","model":"test-model","providerThinkingLevel":"high","usage":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"totalTokens":0,"cost":{"input":0.0,"output":0.0,"cacheRead":0.0,"cacheWrite":0.0,"total":0.0}},"stopReason":"pending"}},
        {"type":"text_start","contentIndex":0,"content":{"type":"text","text":"hi","textSignature":"text-sig"}},
        {"type":"text_delta","contentIndex":0,"delta":"!"},
        {"type":"text_end","contentIndex":0,"content":"hi!","textSignature":"text-final"},
        {"type":"thinking_start","contentIndex":0,"content":{"type":"thinking","thinking":"think","thinkingSignature":"thinking-sig","redacted":true}},
        {"type":"thinking_delta","contentIndex":0,"delta":" more"},
        {"type":"thinking_end","contentIndex":0,"content":"think more","thinkingSignature":"thinking-final","redacted":false},
        {"type":"toolcall_start","contentIndex":0,"toolCall":{"type":"toolCall","id":"call","name":"run","arguments":{"path":"README.md"},"thoughtSignature":"thought-sig","namespace":"tools"}},
        {"type":"toolcall_checkpoint","contentIndex":0,"json":"{\"path\":\"README.md\"}"},
        {"type":"toolcall_delta","contentIndex":0,"delta":"{}"},
        {"type":"toolcall_end","contentIndex":0,"id":"call-final","name":"run_final","arguments":{"path":"final.md"},"thoughtSignature":"thought-final","namespace":"files"}
    ]);
    let serialized = serde_json::to_value(&frames).unwrap();
    assert_eq!(serialized, golden);
    let decoded: Vec<AssistantMessageFrame> = serde_json::from_value(golden).unwrap();
    assert_eq!(as_json(decoded), serialized);

    let text_reduced = reduce(vec![
        frames[0].clone(),
        frames[1].clone(),
        frames[2].clone(),
        frames[3].clone(),
    ]);
    assert_eq!(
        as_json(text_reduced.content[0].clone())["textSignature"],
        "text-final"
    );
    let tool_reduced = reduce(vec![
        frames[0].clone(),
        frames[7].clone(),
        frames[8].clone(),
        frames[10].clone(),
    ]);
    assert_eq!(
        as_json(tool_reduced.content[0].clone())["thoughtSignature"],
        "thought-final"
    );
}

#[test]
fn frame_golden_json_omits_optional_metadata_and_accepts_legacy_snake_case_content_aliases() {
    let minimal = vec![
        AssistantMessageFrame::TextStart {
            content_index: 0,
            content: ContentBlock::Text {
                text: "hi".into(),
                text_signature: None,
            },
        },
        AssistantMessageFrame::ThinkingEnd {
            content_index: 0,
            content: "done".into(),
            thinking_signature: None,
            redacted: None,
        },
        AssistantMessageFrame::ToolCallEnd {
            content_index: 0,
            id: "call".into(),
            name: "run".into(),
            arguments: HashMap::new(),
            thought_signature: None,
            namespace: None,
        },
    ];
    let encoded = serde_json::to_value(&minimal).unwrap();
    assert!(encoded[0]["content"].get("textSignature").is_none());
    assert!(encoded[1].get("thinkingSignature").is_none());
    assert!(encoded[1].get("redacted").is_none());
    assert!(encoded[2].get("thoughtSignature").is_none());
    assert!(encoded[2].get("namespace").is_none());

    let legacy = json!({"type":"text_start","contentIndex":0,"content":{"type":"text","text":"hi","text_signature":"legacy"}});
    let decoded: AssistantMessageFrame = serde_json::from_value(legacy).unwrap();
    assert_eq!(as_json(decoded)["content"]["textSignature"], "legacy");
}

#[test]
fn frame_json_rejects_malformed_or_unknown_public_shapes_at_decode() {
    for bad in [
        json!({"type":"unknown","contentIndex":0}),
        json!({"type":"text_delta","contentIndex":0,"delta":"x","extra":true}),
        json!({"type":"text_start","contentIndex":0,"content":{"type":"thinking","thinking":"wrong"}}),
        json!({"type":"toolcall_start","contentIndex":0,"toolCall":{"type":"text","text":"wrong"}}),
        json!({"type":"start","partial":{"role":"assistant","content":[],"timestamp":1,"isError":false}}),
    ] {
        assert!(
            serde_json::from_value::<AssistantMessageFrame>(bad.clone()).is_err(),
            "malformed frame should fail decode: {bad}"
        );
    }
}

#[test]
fn content_block_global_wire_uses_camel_case_and_deserializes_legacy_snake_case() {
    let blocks = vec![
        ContentBlock::Text {
            text: "hi".into(),
            text_signature: Some("text-sig".into()),
        },
        ContentBlock::Thinking {
            thinking: "why".into(),
            thinking_signature: Some("thinking-sig".into()),
            redacted: Some(false),
        },
        ContentBlock::Image {
            data: "abc".into(),
            mime_type: "image/png".into(),
        },
        ContentBlock::ToolCall {
            id: "call".into(),
            name: "run".into(),
            arguments: HashMap::new(),
            thought_signature: Some("thought-sig".into()),
            namespace: None,
        },
    ];
    assert_eq!(
        as_json(&blocks),
        json!([
            {"type":"text","text":"hi","textSignature":"text-sig"},
            {"type":"thinking","thinking":"why","thinkingSignature":"thinking-sig","redacted":false},
            {"type":"image","data":"abc","mimeType":"image/png"},
            {"type":"toolCall","id":"call","name":"run","arguments":{},"thoughtSignature":"thought-sig"}
        ])
    );
    let decoded: Vec<ContentBlock> = serde_json::from_value(json!([
        {"type":"text","text":"hi","text_signature":"text-sig"},
        {"type":"thinking","thinking":"why","thinking_signature":"thinking-sig","redacted":false},
        {"type":"image","data":"abc","mime_type":"image/png"},
        {"type":"toolCall","id":"call","name":"run","arguments":{},"thought_signature":"thought-sig"}
    ])).unwrap();
    assert_eq!(as_json(decoded), as_json(blocks));
}

#[test]
fn frame_wire_distinguishes_absent_empty_and_explicit_optional_metadata() {
    let frames = vec![
        AssistantMessageFrame::TextEnd {
            content_index: 0,
            content: "".into(),
            text_signature: None,
        },
        AssistantMessageFrame::TextEnd {
            content_index: 0,
            content: "".into(),
            text_signature: Some(String::new()),
        },
        AssistantMessageFrame::ThinkingStart {
            content_index: 0,
            content: ContentBlock::Thinking {
                thinking: String::new(),
                thinking_signature: None,
                redacted: None,
            },
        },
        AssistantMessageFrame::ThinkingStart {
            content_index: 0,
            content: ContentBlock::Thinking {
                thinking: String::new(),
                thinking_signature: Some(String::new()),
                redacted: Some(false),
            },
        },
        AssistantMessageFrame::ThinkingEnd {
            content_index: 0,
            content: String::new(),
            thinking_signature: None,
            redacted: None,
        },
        AssistantMessageFrame::ThinkingEnd {
            content_index: 0,
            content: String::new(),
            thinking_signature: Some(String::new()),
            redacted: Some(false),
        },
        AssistantMessageFrame::ToolCallEnd {
            content_index: 0,
            id: "call".into(),
            name: "run".into(),
            arguments: HashMap::new(),
            thought_signature: Some(String::new()),
            namespace: Some(String::new()),
        },
        AssistantMessageFrame::ToolCallEnd {
            content_index: 0,
            id: "call".into(),
            name: "run".into(),
            arguments: HashMap::new(),
            thought_signature: None,
            namespace: None,
        },
    ];
    let encoded = serde_json::to_value(&frames).unwrap();
    assert!(encoded[0].get("textSignature").is_none());
    assert_eq!(encoded[1]["textSignature"], "");
    assert!(encoded[2]["content"].get("thinkingSignature").is_none());
    assert!(encoded[2]["content"].get("redacted").is_none());
    assert_eq!(encoded[3]["content"]["thinkingSignature"], "");
    assert_eq!(encoded[3]["content"]["redacted"], false);
    assert!(encoded[4].get("thinkingSignature").is_none());
    assert!(encoded[4].get("redacted").is_none());
    assert_eq!(encoded[5]["thinkingSignature"], "");
    assert_eq!(encoded[5]["redacted"], false);
    assert_eq!(encoded[6]["thoughtSignature"], "");
    assert_eq!(encoded[6]["namespace"], "");
    assert!(encoded[7].get("thoughtSignature").is_none());
    assert!(encoded[7].get("namespace").is_none());

    let absent = reduce(vec![
        AssistantMessageFrame::Start {
            partial: Box::new(seed()),
        },
        frames[2].clone(),
        frames[4].clone(),
    ]);
    assert!(as_json(absent.content[0].clone()).get("redacted").is_none());
    let explicit_false = reduce(vec![
        AssistantMessageFrame::Start {
            partial: Box::new(seed()),
        },
        frames[3].clone(),
        frames[5].clone(),
    ]);
    assert_eq!(
        as_json(explicit_false.content[0].clone())["redacted"],
        false
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
        redacted: None,
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
