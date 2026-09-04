//! Portable assistant-message frame encoder/reducer.
//!
//! This mirrors upstream `utils/assistant-message-frame.ts`: compact frames are
//! replayable assistant progress snapshots, while terminal settlement remains
//! persisted separately.

use crate::jsonparse::parse_streaming_json;
use crate::types::{ContentBlock, Message, Role, StopReason};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum AssistantMessageFrame {
    Start {
        partial: Box<Message>,
    },
    TextStart {
        #[serde(rename = "contentIndex")]
        content_index: usize,
        content: ContentBlock,
    },
    TextDelta {
        #[serde(rename = "contentIndex")]
        content_index: usize,
        delta: String,
    },
    TextEnd {
        #[serde(rename = "contentIndex")]
        content_index: usize,
        content: String,
        #[serde(rename = "textSignature", skip_serializing_if = "Option::is_none")]
        text_signature: Option<String>,
    },
    ThinkingStart {
        #[serde(rename = "contentIndex")]
        content_index: usize,
        content: ContentBlock,
    },
    ThinkingDelta {
        #[serde(rename = "contentIndex")]
        content_index: usize,
        delta: String,
    },
    ThinkingEnd {
        #[serde(rename = "contentIndex")]
        content_index: usize,
        content: String,
        #[serde(rename = "thinkingSignature", skip_serializing_if = "Option::is_none")]
        thinking_signature: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        redacted: Option<bool>,
    },
    #[serde(rename = "toolcall_start")]
    ToolCallStart {
        #[serde(rename = "contentIndex")]
        content_index: usize,
        #[serde(rename = "toolCall")]
        tool_call: ContentBlock,
    },
    #[serde(rename = "toolcall_checkpoint")]
    ToolCallCheckpoint {
        #[serde(rename = "contentIndex")]
        content_index: usize,
        json: String,
    },
    #[serde(rename = "toolcall_delta")]
    ToolCallDelta {
        #[serde(rename = "contentIndex")]
        content_index: usize,
        delta: String,
    },
    #[serde(rename = "toolcall_end")]
    ToolCallEnd {
        #[serde(rename = "contentIndex")]
        content_index: usize,
        id: String,
        name: String,
        arguments: HashMap<String, Value>,
        #[serde(rename = "thoughtSignature", skip_serializing_if = "Option::is_none")]
        thought_signature: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        namespace: Option<String>,
    },
}

#[derive(Debug, Clone)]
pub enum AssistantMessageEvent {
    Start {
        partial: Message,
    },
    TextStart {
        content_index: usize,
        partial: Message,
    },
    TextDelta {
        content_index: usize,
        delta: String,
        partial: Message,
    },
    TextEnd {
        content_index: usize,
        content: String,
        partial: Message,
    },
    ThinkingStart {
        content_index: usize,
        partial: Message,
    },
    ThinkingDelta {
        content_index: usize,
        delta: String,
        partial: Message,
    },
    ThinkingEnd {
        content_index: usize,
        content: String,
        partial: Message,
    },
    ToolCallStart {
        content_index: usize,
        partial: Message,
    },
    ToolCallDelta {
        content_index: usize,
        delta: String,
        partial: Message,
    },
    ToolCallEnd {
        content_index: usize,
        tool_call: ContentBlock,
        partial: Message,
    },
    Done {
        reason: StopReason,
        message: Message,
    },
    Error {
        reason: StopReason,
        error: Message,
    },
}

#[derive(Debug, Clone)]
enum EncoderBlockState {
    Text {
        covered_chars: usize,
        delta_chars: usize,
    },
    Thinking {
        covered_chars: usize,
        delta_chars: usize,
    },
    ToolCall {
        caught_up: bool,
        catchup_json: String,
        snapshot_arguments: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BlockKind {
    Text,
    Thinking,
    ToolCall,
}

impl EncoderBlockState {
    fn kind(&self) -> BlockKind {
        match self {
            EncoderBlockState::Text { .. } => BlockKind::Text,
            EncoderBlockState::Thinking { .. } => BlockKind::Thinking,
            EncoderBlockState::ToolCall { .. } => BlockKind::ToolCall,
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct AssistantMessageFrameEncoder {
    started: bool,
    terminal: bool,
    blocks: HashMap<usize, EncoderBlockState>,
}

impl AssistantMessageFrameEncoder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn encode(
        &mut self,
        event: AssistantMessageEvent,
    ) -> Result<Option<AssistantMessageFrame>, String> {
        let event_type = event_type(&event);
        if self.terminal {
            return Err(format!(
                "Assistant message event {event_type} follows a terminal event"
            ));
        }

        match event {
            AssistantMessageEvent::Start { partial } => {
                if self.started {
                    return Err(
                        "Assistant message stream contains more than one start event".into(),
                    );
                }
                self.started = true;
                Ok(Some(AssistantMessageFrame::Start {
                    partial: Box::new(clone_start_message(&partial)),
                }))
            }
            AssistantMessageEvent::Done { .. } => {
                if !self.started {
                    return Err("Assistant message done event appears before start".into());
                }
                self.terminal = true;
                Ok(None)
            }
            AssistantMessageEvent::Error { .. } => {
                self.terminal = true;
                Ok(None)
            }
            AssistantMessageEvent::TextStart {
                content_index,
                partial,
            } => {
                self.require_started("text_start")?;
                let content = event_block(&partial, content_index, "text_start")?;
                let ContentBlock::Text { text, .. } = content else {
                    return Err(format!(
                        "text_start event points to {} block at index {content_index}",
                        content_block_type(content)
                    ));
                };
                self.start_block(
                    content_index,
                    EncoderBlockState::Text {
                        covered_chars: text.chars().count(),
                        delta_chars: 0,
                    },
                )?;
                Ok(Some(AssistantMessageFrame::TextStart {
                    content_index,
                    content: clone_public_block(content),
                }))
            }
            AssistantMessageEvent::TextDelta {
                content_index,
                delta,
                ..
            } => {
                self.require_started("text_delta")?;
                self.encode_text_delta(content_index, &delta, BlockKind::Text)
            }
            AssistantMessageEvent::TextEnd {
                content_index,
                content,
                partial,
            } => {
                self.require_started("text_end")?;
                let block = event_block(&partial, content_index, "text_end")?;
                let ContentBlock::Text { text_signature, .. } = block else {
                    return Err(format!(
                        "text_end event points to {} block at index {content_index}",
                        content_block_type(block)
                    ));
                };
                self.end_block(content_index, BlockKind::Text)?;
                Ok(Some(AssistantMessageFrame::TextEnd {
                    content_index,
                    content,
                    text_signature: text_signature.clone(),
                }))
            }
            AssistantMessageEvent::ThinkingStart {
                content_index,
                partial,
            } => {
                self.require_started("thinking_start")?;
                let content = event_block(&partial, content_index, "thinking_start")?;
                let ContentBlock::Thinking { thinking, .. } = content else {
                    return Err(format!(
                        "thinking_start event points to {} block at index {content_index}",
                        content_block_type(content)
                    ));
                };
                self.start_block(
                    content_index,
                    EncoderBlockState::Thinking {
                        covered_chars: thinking.chars().count(),
                        delta_chars: 0,
                    },
                )?;
                Ok(Some(AssistantMessageFrame::ThinkingStart {
                    content_index,
                    content: clone_public_block(content),
                }))
            }
            AssistantMessageEvent::ThinkingDelta {
                content_index,
                delta,
                ..
            } => {
                self.require_started("thinking_delta")?;
                self.encode_text_delta(content_index, &delta, BlockKind::Thinking)
            }
            AssistantMessageEvent::ThinkingEnd {
                content_index,
                content,
                partial,
            } => {
                self.require_started("thinking_end")?;
                let block = event_block(&partial, content_index, "thinking_end")?;
                let ContentBlock::Thinking {
                    thinking_signature,
                    redacted,
                    ..
                } = block
                else {
                    return Err(format!(
                        "thinking_end event points to {} block at index {content_index}",
                        content_block_type(block)
                    ));
                };
                self.end_block(content_index, BlockKind::Thinking)?;
                Ok(Some(AssistantMessageFrame::ThinkingEnd {
                    content_index,
                    content,
                    thinking_signature: thinking_signature.clone(),
                    redacted: Some(*redacted),
                }))
            }
            AssistantMessageEvent::ToolCallStart {
                content_index,
                partial,
            } => {
                self.require_started("toolcall_start")?;
                let content = event_block(&partial, content_index, "toolcall_start")?;
                let ContentBlock::ToolCall { arguments, .. } = content else {
                    return Err(format!(
                        "toolcall_start event points to {} block at index {content_index}",
                        content_block_type(content)
                    ));
                };
                let snapshot_arguments = serialized_arguments(arguments)?;
                let caught_up = snapshot_arguments == "{}";
                self.start_block(
                    content_index,
                    EncoderBlockState::ToolCall {
                        caught_up,
                        catchup_json: String::new(),
                        snapshot_arguments: if caught_up {
                            String::new()
                        } else {
                            snapshot_arguments
                        },
                    },
                )?;
                Ok(Some(AssistantMessageFrame::ToolCallStart {
                    content_index,
                    tool_call: clone_public_block(content),
                }))
            }
            AssistantMessageEvent::ToolCallDelta {
                content_index,
                delta,
                ..
            } => {
                self.require_started("toolcall_delta")?;
                let state = self.block_mut(content_index, BlockKind::ToolCall)?;
                let EncoderBlockState::ToolCall {
                    caught_up,
                    catchup_json,
                    snapshot_arguments,
                } = state
                else {
                    unreachable!("validated tool-call encoder state")
                };
                if *caught_up {
                    return if delta.is_empty() {
                        Ok(None)
                    } else {
                        Ok(Some(AssistantMessageFrame::ToolCallDelta {
                            content_index,
                            delta,
                        }))
                    };
                }
                catchup_json.push_str(&delta);
                let arguments_value = parse_streaming_json(catchup_json);
                if canonical_json(&arguments_value)? != *snapshot_arguments {
                    let snapshot_value = parse_streaming_json(snapshot_arguments);
                    if !is_json_prefix(&snapshot_value, &arguments_value) {
                        return Ok(None);
                    }
                }
                *caught_up = true;
                snapshot_arguments.clear();
                let json = std::mem::take(catchup_json);
                if json.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(AssistantMessageFrame::ToolCallCheckpoint {
                        content_index,
                        json,
                    }))
                }
            }
            AssistantMessageEvent::ToolCallEnd {
                content_index,
                tool_call,
                partial,
            } => {
                self.require_started("toolcall_end")?;
                let content = event_block(&partial, content_index, "toolcall_end")?;
                if !matches!(content, ContentBlock::ToolCall { .. }) {
                    return Err(format!(
                        "toolcall_end event points to {} block at index {content_index}",
                        content_block_type(content)
                    ));
                }
                let ContentBlock::ToolCall {
                    id,
                    name,
                    arguments,
                    thought_signature,
                    namespace,
                } = tool_call
                else {
                    return Err(format!(
                        "toolcall_end event has invalid tool call at index {content_index}"
                    ));
                };
                self.end_block(content_index, BlockKind::ToolCall)?;
                Ok(Some(AssistantMessageFrame::ToolCallEnd {
                    content_index,
                    id,
                    name,
                    arguments,
                    thought_signature,
                    namespace,
                }))
            }
        }
    }

    fn require_started(&self, event_type: &str) -> Result<(), String> {
        if self.started {
            Ok(())
        } else {
            Err(format!(
                "Assistant message {event_type} event appears before start"
            ))
        }
    }

    fn start_block(
        &mut self,
        content_index: usize,
        state: EncoderBlockState,
    ) -> Result<(), String> {
        if self.blocks.contains_key(&content_index) {
            return Err(format!(
                "Assistant message block {content_index} starts more than once"
            ));
        }
        self.blocks.insert(content_index, state);
        Ok(())
    }

    fn block_mut(
        &mut self,
        content_index: usize,
        kind: BlockKind,
    ) -> Result<&mut EncoderBlockState, String> {
        let state = self.blocks.get_mut(&content_index).ok_or_else(|| {
            format!(
                "Assistant message {} block {content_index} has not started",
                kind_name(kind)
            )
        })?;
        if state.kind() != kind {
            return Err(format!(
                "Assistant message block {content_index} is {}, not {}",
                kind_name(state.kind()),
                kind_name(kind)
            ));
        }
        Ok(state)
    }

    fn end_block(&mut self, content_index: usize, kind: BlockKind) -> Result<(), String> {
        self.block_mut(content_index, kind)?;
        self.blocks.remove(&content_index);
        Ok(())
    }

    fn encode_text_delta(
        &mut self,
        content_index: usize,
        delta: &str,
        kind: BlockKind,
    ) -> Result<Option<AssistantMessageFrame>, String> {
        let state = self.block_mut(content_index, kind)?;
        let (covered_chars, delta_chars) = match state {
            EncoderBlockState::Text {
                covered_chars,
                delta_chars,
            }
            | EncoderBlockState::Thinking {
                covered_chars,
                delta_chars,
            } => (covered_chars, delta_chars),
            EncoderBlockState::ToolCall { .. } => unreachable!("validated text encoder state"),
        };
        let delta_start = *delta_chars;
        let delta_len = delta.chars().count();
        *delta_chars += delta_len;
        let covered = covered_chars.saturating_sub(delta_start);
        if covered >= delta_len {
            return Ok(None);
        }
        let uncovered = delta.chars().skip(covered).collect::<String>();
        Ok(Some(match kind {
            BlockKind::Text => AssistantMessageFrame::TextDelta {
                content_index,
                delta: uncovered,
            },
            BlockKind::Thinking => AssistantMessageFrame::ThinkingDelta {
                content_index,
                delta: uncovered,
            },
            BlockKind::ToolCall => unreachable!("validated text delta kind"),
        }))
    }
}

#[derive(Debug, Clone)]
struct ReducerBlockState {
    kind: BlockKind,
    ended: bool,
    json: String,
}

pub fn reduce_assistant_message_frames<I>(frames: I) -> Result<Option<Message>, String>
where
    I: IntoIterator<Item = AssistantMessageFrame>,
{
    let mut message: Option<Message> = None;
    let mut frame_before_start: Option<&'static str> = None;
    let mut states: HashMap<usize, ReducerBlockState> = HashMap::new();

    for frame in frames {
        let frame_type = frame_type(&frame);
        if let AssistantMessageFrame::Start { partial } = frame {
            if message.is_some() {
                return Err(
                    "Assistant message frame sequence contains more than one start frame".into(),
                );
            }
            if let Some(before) = frame_before_start {
                return Err(format!("{before} frame appears before the start frame"));
            }
            message = Some((*partial).clone());
            continue;
        }
        let Some(msg) = message.as_mut() else {
            frame_before_start.get_or_insert(frame_type);
            continue;
        };

        match frame {
            AssistantMessageFrame::TextStart {
                content_index,
                content,
            } => {
                if !matches!(content, ContentBlock::Text { .. }) {
                    return Err(format!(
                        "text_start frame contains {} content",
                        content_block_type(&content)
                    ));
                }
                append_block(msg, &mut states, content_index, content, BlockKind::Text)?;
            }
            AssistantMessageFrame::TextDelta {
                content_index,
                delta,
            } => {
                let block =
                    active_block(msg, &mut states, content_index, BlockKind::Text, frame_type)?;
                let ContentBlock::Text { text, .. } = block else {
                    unreachable!("validated text frame state")
                };
                text.push_str(&delta);
            }
            AssistantMessageFrame::TextEnd {
                content_index,
                content,
                text_signature,
            } => {
                let block =
                    active_block(msg, &mut states, content_index, BlockKind::Text, frame_type)?;
                let ContentBlock::Text {
                    text,
                    text_signature: sig,
                } = block
                else {
                    unreachable!("validated text frame state")
                };
                *text = content;
                *sig = text_signature;
                if let Some(state) = states.get_mut(&content_index) {
                    state.ended = true;
                }
            }
            AssistantMessageFrame::ThinkingStart {
                content_index,
                content,
            } => {
                if !matches!(content, ContentBlock::Thinking { .. }) {
                    return Err(format!(
                        "thinking_start frame contains {} content",
                        content_block_type(&content)
                    ));
                }
                append_block(
                    msg,
                    &mut states,
                    content_index,
                    content,
                    BlockKind::Thinking,
                )?;
            }
            AssistantMessageFrame::ThinkingDelta {
                content_index,
                delta,
            } => {
                let block = active_block(
                    msg,
                    &mut states,
                    content_index,
                    BlockKind::Thinking,
                    frame_type,
                )?;
                let ContentBlock::Thinking { thinking, .. } = block else {
                    unreachable!("validated thinking frame state")
                };
                thinking.push_str(&delta);
            }
            AssistantMessageFrame::ThinkingEnd {
                content_index,
                content,
                thinking_signature,
                redacted,
            } => {
                let block = active_block(
                    msg,
                    &mut states,
                    content_index,
                    BlockKind::Thinking,
                    frame_type,
                )?;
                let ContentBlock::Thinking {
                    thinking,
                    thinking_signature: sig,
                    redacted: block_redacted,
                } = block
                else {
                    unreachable!("validated thinking frame state")
                };
                *thinking = content;
                *sig = thinking_signature;
                if let Some(value) = redacted {
                    *block_redacted = value;
                }
                if let Some(state) = states.get_mut(&content_index) {
                    state.ended = true;
                }
            }
            AssistantMessageFrame::ToolCallStart {
                content_index,
                tool_call,
            } => {
                if !matches!(tool_call, ContentBlock::ToolCall { .. }) {
                    return Err(format!(
                        "toolcall_start frame contains {} content",
                        content_block_type(&tool_call)
                    ));
                }
                append_block(
                    msg,
                    &mut states,
                    content_index,
                    tool_call,
                    BlockKind::ToolCall,
                )?;
            }
            AssistantMessageFrame::ToolCallCheckpoint {
                content_index,
                json,
            } => {
                let block = active_block(
                    msg,
                    &mut states,
                    content_index,
                    BlockKind::ToolCall,
                    frame_type,
                )?;
                let ContentBlock::ToolCall { arguments, .. } = block else {
                    unreachable!("validated tool-call checkpoint state")
                };
                *arguments = value_to_arguments(parse_streaming_json(&json));
                if let Some(state) = states.get_mut(&content_index) {
                    state.json = json;
                }
            }
            AssistantMessageFrame::ToolCallDelta {
                content_index,
                delta,
            } => {
                active_block(
                    msg,
                    &mut states,
                    content_index,
                    BlockKind::ToolCall,
                    frame_type,
                )?;
                if let Some(state) = states.get_mut(&content_index) {
                    state.json.push_str(&delta);
                }
            }
            AssistantMessageFrame::ToolCallEnd {
                content_index,
                id,
                name,
                arguments,
                thought_signature,
                namespace,
            } => {
                let block = active_block(
                    msg,
                    &mut states,
                    content_index,
                    BlockKind::ToolCall,
                    frame_type,
                )?;
                let ContentBlock::ToolCall {
                    id: block_id,
                    name: block_name,
                    arguments: block_arguments,
                    thought_signature: block_thought_signature,
                    namespace: block_namespace,
                } = block
                else {
                    unreachable!("validated tool-call frame state")
                };
                *block_id = id;
                *block_name = name;
                *block_arguments = arguments;
                *block_thought_signature = thought_signature;
                *block_namespace = namespace;
                if let Some(state) = states.get_mut(&content_index) {
                    state.ended = true;
                }
            }
            AssistantMessageFrame::Start { .. } => unreachable!("start handled above"),
        }
    }

    let Some(mut msg) = message else {
        return Ok(None);
    };
    for (content_index, state) in states {
        if state.kind != BlockKind::ToolCall || state.ended || state.json.is_empty() {
            continue;
        }
        let Some(ContentBlock::ToolCall { arguments, .. }) = msg.content.get_mut(content_index)
        else {
            return Err("Unreachable tool-call frame state".into());
        };
        *arguments = value_to_arguments(parse_streaming_json(&state.json));
    }
    Ok(Some(msg))
}

fn event_type(event: &AssistantMessageEvent) -> &'static str {
    match event {
        AssistantMessageEvent::Start { .. } => "start",
        AssistantMessageEvent::TextStart { .. } => "text_start",
        AssistantMessageEvent::TextDelta { .. } => "text_delta",
        AssistantMessageEvent::TextEnd { .. } => "text_end",
        AssistantMessageEvent::ThinkingStart { .. } => "thinking_start",
        AssistantMessageEvent::ThinkingDelta { .. } => "thinking_delta",
        AssistantMessageEvent::ThinkingEnd { .. } => "thinking_end",
        AssistantMessageEvent::ToolCallStart { .. } => "toolcall_start",
        AssistantMessageEvent::ToolCallDelta { .. } => "toolcall_delta",
        AssistantMessageEvent::ToolCallEnd { .. } => "toolcall_end",
        AssistantMessageEvent::Done { .. } => "done",
        AssistantMessageEvent::Error { .. } => "error",
    }
}

fn frame_type(frame: &AssistantMessageFrame) -> &'static str {
    match frame {
        AssistantMessageFrame::Start { .. } => "start",
        AssistantMessageFrame::TextStart { .. } => "text_start",
        AssistantMessageFrame::TextDelta { .. } => "text_delta",
        AssistantMessageFrame::TextEnd { .. } => "text_end",
        AssistantMessageFrame::ThinkingStart { .. } => "thinking_start",
        AssistantMessageFrame::ThinkingDelta { .. } => "thinking_delta",
        AssistantMessageFrame::ThinkingEnd { .. } => "thinking_end",
        AssistantMessageFrame::ToolCallStart { .. } => "toolcall_start",
        AssistantMessageFrame::ToolCallCheckpoint { .. } => "toolcall_checkpoint",
        AssistantMessageFrame::ToolCallDelta { .. } => "toolcall_delta",
        AssistantMessageFrame::ToolCallEnd { .. } => "toolcall_end",
    }
}

fn content_block_type(block: &ContentBlock) -> &'static str {
    match block {
        ContentBlock::Text { .. } => "text",
        ContentBlock::Thinking { .. } => "thinking",
        ContentBlock::Image { .. } => "image",
        ContentBlock::ToolCall { .. } => "toolCall",
    }
}

fn kind_name(kind: BlockKind) -> &'static str {
    match kind {
        BlockKind::Text => "text",
        BlockKind::Thinking => "thinking",
        BlockKind::ToolCall => "toolCall",
    }
}

fn event_block<'a>(
    partial: &'a Message,
    content_index: usize,
    event_type: &str,
) -> Result<&'a ContentBlock, String> {
    partial
        .content
        .get(content_index)
        .ok_or_else(|| format!("{event_type} event has no content block at index {content_index}"))
}

fn clone_start_message(message: &Message) -> Message {
    Message {
        role: Role::Assistant,
        content: Vec::new(),
        timestamp: message.timestamp,
        api: message.api.clone(),
        provider: message.provider.clone(),
        model: message.model.clone(),
        response_id: message.response_id.clone(),
        response_model: message.response_model.clone(),
        provider_thinking_level: message.provider_thinking_level.clone(),
        diagnostics: message.diagnostics.clone(),
        usage: message.usage.clone(),
        stop_reason: Some(StopReason::Pending),
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

fn clone_public_block(block: &ContentBlock) -> ContentBlock {
    block.clone()
}

fn serialized_arguments(arguments: &HashMap<String, Value>) -> Result<String, String> {
    canonical_json(&Value::Object(
        arguments
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect(),
    ))
}

fn canonical_json(value: &Value) -> Result<String, String> {
    fn normalize(value: &Value) -> Value {
        match value {
            Value::Object(map) => {
                let sorted = map
                    .iter()
                    .map(|(key, value)| (key.clone(), normalize(value)))
                    .collect::<BTreeMap<_, _>>();
                Value::Object(sorted.into_iter().collect())
            }
            Value::Array(values) => Value::Array(values.iter().map(normalize).collect()),
            other => other.clone(),
        }
    }
    serde_json::to_string(&normalize(value))
        .map_err(|_| "Tool-call arguments are not JSON-serializable".to_string())
}

fn value_to_arguments(value: Value) -> HashMap<String, Value> {
    match value {
        Value::Object(map) => map.into_iter().collect(),
        _ => HashMap::new(),
    }
}

fn is_json_prefix(snapshot: &Value, current: &Value) -> bool {
    match (snapshot, current) {
        (Value::String(left), Value::String(right)) => right.starts_with(left),
        (Value::Array(left), Value::Array(right)) => {
            left.len() <= right.len()
                && left
                    .iter()
                    .zip(right.iter())
                    .all(|(l, r)| is_json_prefix(l, r))
        }
        (Value::Object(left), Value::Object(right)) => left
            .iter()
            .all(|(key, value)| right.get(key).is_some_and(|v| is_json_prefix(value, v))),
        (Value::Null, Value::Null) => true,
        (Value::Bool(left), Value::Bool(right)) => left == right,
        (Value::Number(left), Value::Number(right)) => left == right,
        _ => false,
    }
}

fn append_block(
    message: &mut Message,
    states: &mut HashMap<usize, ReducerBlockState>,
    content_index: usize,
    block: ContentBlock,
    kind: BlockKind,
) -> Result<(), String> {
    if content_index != message.content.len() {
        let reason = if content_index < message.content.len() {
            "already exists"
        } else {
            "would leave a gap"
        };
        return Err(format!(
            "Cannot start assistant message block at index {content_index}: {reason}"
        ));
    }
    message.content.push(block);
    states.insert(
        content_index,
        ReducerBlockState {
            kind,
            ended: false,
            json: String::new(),
        },
    );
    Ok(())
}

fn active_block<'a>(
    message: &'a mut Message,
    states: &mut HashMap<usize, ReducerBlockState>,
    content_index: usize,
    expected_kind: BlockKind,
    frame_type: &str,
) -> Result<&'a mut ContentBlock, String> {
    let state = states.get(&content_index).ok_or_else(|| {
        format!("{frame_type} frame has no started block at index {content_index}")
    })?;
    let found_type = message
        .content
        .get(content_index)
        .map(content_block_type)
        .unwrap_or("missing");
    if state.kind != expected_kind || found_type != kind_name(expected_kind) {
        return Err(format!(
            "{frame_type} frame expected {} block at index {content_index}, found {found_type}",
            kind_name(expected_kind)
        ));
    }
    if state.ended {
        return Err(format!(
            "{frame_type} frame follows the end of block at index {content_index}"
        ));
    }
    message
        .content
        .get_mut(content_index)
        .ok_or_else(|| format!("{frame_type} frame has no started block at index {content_index}"))
}
