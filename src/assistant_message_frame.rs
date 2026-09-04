//! Portable assistant-message frame encoder/reducer.
//!
//! This mirrors upstream `utils/assistant-message-frame.ts`: compact frames are
//! replayable assistant progress snapshots, while terminal settlement remains
//! persisted separately.

use crate::jsonparse::parse_streaming_json;
use crate::types::{
    Api, AssistantMessageDiagnostic, ContentBlock, Message, Provider, Role, StopReason, Usage,
};
use serde::de::Error as DeError;
use serde::ser::SerializeStruct;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};

#[derive(Debug, Clone)]
pub enum AssistantMessageFrame {
    Start {
        partial: Box<Message>,
    },
    TextStart {
        content_index: usize,
        content: ContentBlock,
    },
    TextDelta {
        content_index: usize,
        delta: String,
    },
    TextEnd {
        content_index: usize,
        content: String,
        text_signature: Option<String>,
    },
    ThinkingStart {
        content_index: usize,
        content: ContentBlock,
    },
    ThinkingDelta {
        content_index: usize,
        delta: String,
    },
    ThinkingEnd {
        content_index: usize,
        content: String,
        thinking_signature: Option<String>,
        redacted: Option<bool>,
    },
    ToolCallStart {
        content_index: usize,
        tool_call: ContentBlock,
    },
    ToolCallCheckpoint {
        content_index: usize,
        json: String,
    },
    ToolCallDelta {
        content_index: usize,
        delta: String,
    },
    ToolCallEnd {
        content_index: usize,
        id: String,
        name: String,
        arguments: HashMap<String, Value>,
        thought_signature: Option<String>,
        namespace: Option<String>,
    },
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AssistantStartPartialWire<'a> {
    role: &'a Role,
    content: &'a [ContentBlock],
    timestamp: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    api: &'a Option<Api>,
    #[serde(skip_serializing_if = "Option::is_none")]
    provider: &'a Option<Provider>,
    #[serde(skip_serializing_if = "Option::is_none")]
    model: &'a Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    response_id: &'a Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    response_model: &'a Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    provider_thinking_level: &'a Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    diagnostics: &'a Vec<AssistantMessageDiagnostic>,
    #[serde(skip_serializing_if = "Option::is_none")]
    usage: &'a Option<Usage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stop_reason: &'a Option<StopReason>,
}

impl<'a> From<&'a Message> for AssistantStartPartialWire<'a> {
    fn from(message: &'a Message) -> Self {
        Self {
            role: &message.role,
            content: &message.content,
            timestamp: message.timestamp,
            api: &message.api,
            provider: &message.provider,
            model: &message.model,
            response_id: &message.response_id,
            response_model: &message.response_model,
            provider_thinking_level: &message.provider_thinking_level,
            diagnostics: &message.diagnostics,
            usage: &message.usage,
            stop_reason: &message.stop_reason,
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AssistantStartPartialOwned {
    role: Role,
    #[serde(default)]
    content: Vec<ContentBlock>,
    #[serde(default)]
    timestamp: i64,
    api: Option<Api>,
    provider: Option<Provider>,
    model: Option<String>,
    #[serde(default)]
    response_id: Option<String>,
    #[serde(default)]
    response_model: Option<String>,
    #[serde(default)]
    provider_thinking_level: Option<String>,
    #[serde(default)]
    diagnostics: Vec<AssistantMessageDiagnostic>,
    usage: Option<Usage>,
    stop_reason: Option<StopReason>,
}

impl From<AssistantStartPartialOwned> for Message {
    fn from(value: AssistantStartPartialOwned) -> Self {
        Message {
            role: value.role,
            content: value.content,
            timestamp: value.timestamp,
            api: value.api,
            provider: value.provider,
            model: value.model,
            response_id: value.response_id,
            response_model: value.response_model,
            provider_thinking_level: value.provider_thinking_level,
            diagnostics: value.diagnostics,
            usage: value.usage,
            stop_reason: value.stop_reason,
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
}

impl Serialize for AssistantMessageFrame {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            AssistantMessageFrame::Start { partial } => {
                let mut s = serializer.serialize_struct("AssistantMessageFrame", 2)?;
                s.serialize_field("type", "start")?;
                s.serialize_field(
                    "partial",
                    &AssistantStartPartialWire::from(partial.as_ref()),
                )?;
                s.end()
            }
            AssistantMessageFrame::TextStart {
                content_index,
                content,
            } => {
                let mut s = serializer.serialize_struct("AssistantMessageFrame", 3)?;
                s.serialize_field("type", "text_start")?;
                s.serialize_field("contentIndex", content_index)?;
                s.serialize_field("content", content)?;
                s.end()
            }
            AssistantMessageFrame::TextDelta {
                content_index,
                delta,
            } => {
                let mut s = serializer.serialize_struct("AssistantMessageFrame", 3)?;
                s.serialize_field("type", "text_delta")?;
                s.serialize_field("contentIndex", content_index)?;
                s.serialize_field("delta", delta)?;
                s.end()
            }
            AssistantMessageFrame::TextEnd {
                content_index,
                content,
                text_signature,
            } => {
                let mut s = serializer.serialize_struct(
                    "AssistantMessageFrame",
                    if text_signature.is_some() { 4 } else { 3 },
                )?;
                s.serialize_field("type", "text_end")?;
                s.serialize_field("contentIndex", content_index)?;
                s.serialize_field("content", content)?;
                if let Some(sig) = text_signature {
                    s.serialize_field("textSignature", sig)?;
                }
                s.end()
            }
            AssistantMessageFrame::ThinkingStart {
                content_index,
                content,
            } => {
                let mut s = serializer.serialize_struct("AssistantMessageFrame", 3)?;
                s.serialize_field("type", "thinking_start")?;
                s.serialize_field("contentIndex", content_index)?;
                s.serialize_field("content", content)?;
                s.end()
            }
            AssistantMessageFrame::ThinkingDelta {
                content_index,
                delta,
            } => {
                let mut s = serializer.serialize_struct("AssistantMessageFrame", 3)?;
                s.serialize_field("type", "thinking_delta")?;
                s.serialize_field("contentIndex", content_index)?;
                s.serialize_field("delta", delta)?;
                s.end()
            }
            AssistantMessageFrame::ThinkingEnd {
                content_index,
                content,
                thinking_signature,
                redacted,
            } => {
                let mut s = serializer.serialize_struct(
                    "AssistantMessageFrame",
                    3 + usize::from(thinking_signature.is_some()) + usize::from(redacted.is_some()),
                )?;
                s.serialize_field("type", "thinking_end")?;
                s.serialize_field("contentIndex", content_index)?;
                s.serialize_field("content", content)?;
                if let Some(sig) = thinking_signature {
                    s.serialize_field("thinkingSignature", sig)?;
                }
                if let Some(value) = redacted {
                    s.serialize_field("redacted", value)?;
                }
                s.end()
            }
            AssistantMessageFrame::ToolCallStart {
                content_index,
                tool_call,
            } => {
                let mut s = serializer.serialize_struct("AssistantMessageFrame", 3)?;
                s.serialize_field("type", "toolcall_start")?;
                s.serialize_field("contentIndex", content_index)?;
                s.serialize_field("toolCall", tool_call)?;
                s.end()
            }
            AssistantMessageFrame::ToolCallCheckpoint {
                content_index,
                json,
            } => {
                let mut s = serializer.serialize_struct("AssistantMessageFrame", 3)?;
                s.serialize_field("type", "toolcall_checkpoint")?;
                s.serialize_field("contentIndex", content_index)?;
                s.serialize_field("json", json)?;
                s.end()
            }
            AssistantMessageFrame::ToolCallDelta {
                content_index,
                delta,
            } => {
                let mut s = serializer.serialize_struct("AssistantMessageFrame", 3)?;
                s.serialize_field("type", "toolcall_delta")?;
                s.serialize_field("contentIndex", content_index)?;
                s.serialize_field("delta", delta)?;
                s.end()
            }
            AssistantMessageFrame::ToolCallEnd {
                content_index,
                id,
                name,
                arguments,
                thought_signature,
                namespace,
            } => {
                let mut s = serializer.serialize_struct(
                    "AssistantMessageFrame",
                    5 + usize::from(thought_signature.is_some()) + usize::from(namespace.is_some()),
                )?;
                s.serialize_field("type", "toolcall_end")?;
                s.serialize_field("contentIndex", content_index)?;
                s.serialize_field("id", id)?;
                s.serialize_field("name", name)?;
                s.serialize_field("arguments", arguments)?;
                if let Some(sig) = thought_signature {
                    s.serialize_field("thoughtSignature", sig)?;
                }
                if let Some(ns) = namespace {
                    s.serialize_field("namespace", ns)?;
                }
                s.end()
            }
        }
    }
}

impl<'de> Deserialize<'de> for AssistantMessageFrame {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        decode_frame_value(value).map_err(D::Error::custom)
    }
}

fn decode_frame_value(value: Value) -> Result<AssistantMessageFrame, String> {
    let mut object = match value {
        Value::Object(map) => map,
        _ => return Err("assistant message frame must be an object".into()),
    };
    let frame_type = take_string(&mut object, "type")?;
    match frame_type.as_str() {
        "start" => {
            reject_unknown(&object, &["partial"])?;
            let partial = take_value(&mut object, "partial")?;
            let partial = serde_json::from_value::<AssistantStartPartialOwned>(partial)
                .map_err(|e| e.to_string())?;
            Ok(AssistantMessageFrame::Start {
                partial: Box::new(partial.into()),
            })
        }
        "text_start" => {
            reject_unknown(&object, &["contentIndex", "content"])?;
            let content_index = take_usize(&mut object, "contentIndex")?;
            let content = take_content(&mut object, "content")?;
            if !matches!(content, ContentBlock::Text { .. }) {
                return Err(format!(
                    "text_start frame contains {} content",
                    content_block_type(&content)
                ));
            }
            Ok(AssistantMessageFrame::TextStart {
                content_index,
                content,
            })
        }
        "text_delta" => {
            reject_unknown(&object, &["contentIndex", "delta"])?;
            Ok(AssistantMessageFrame::TextDelta {
                content_index: take_usize(&mut object, "contentIndex")?,
                delta: take_string(&mut object, "delta")?,
            })
        }
        "text_end" => {
            reject_unknown(&object, &["contentIndex", "content", "textSignature"])?;
            Ok(AssistantMessageFrame::TextEnd {
                content_index: take_usize(&mut object, "contentIndex")?,
                content: take_string(&mut object, "content")?,
                text_signature: take_optional_string(&mut object, "textSignature")?,
            })
        }
        "thinking_start" => {
            reject_unknown(&object, &["contentIndex", "content"])?;
            let content_index = take_usize(&mut object, "contentIndex")?;
            let content = take_content(&mut object, "content")?;
            if !matches!(content, ContentBlock::Thinking { .. }) {
                return Err(format!(
                    "thinking_start frame contains {} content",
                    content_block_type(&content)
                ));
            }
            Ok(AssistantMessageFrame::ThinkingStart {
                content_index,
                content,
            })
        }
        "thinking_delta" => {
            reject_unknown(&object, &["contentIndex", "delta"])?;
            Ok(AssistantMessageFrame::ThinkingDelta {
                content_index: take_usize(&mut object, "contentIndex")?,
                delta: take_string(&mut object, "delta")?,
            })
        }
        "thinking_end" => {
            reject_unknown(
                &object,
                &["contentIndex", "content", "thinkingSignature", "redacted"],
            )?;
            Ok(AssistantMessageFrame::ThinkingEnd {
                content_index: take_usize(&mut object, "contentIndex")?,
                content: take_string(&mut object, "content")?,
                thinking_signature: take_optional_string(&mut object, "thinkingSignature")?,
                redacted: take_optional_bool(&mut object, "redacted")?,
            })
        }
        "toolcall_start" => {
            reject_unknown(&object, &["contentIndex", "toolCall"])?;
            let content_index = take_usize(&mut object, "contentIndex")?;
            let tool_call = take_content(&mut object, "toolCall")?;
            if !matches!(tool_call, ContentBlock::ToolCall { .. }) {
                return Err(format!(
                    "toolcall_start frame contains {} content",
                    content_block_type(&tool_call)
                ));
            }
            Ok(AssistantMessageFrame::ToolCallStart {
                content_index,
                tool_call,
            })
        }
        "toolcall_checkpoint" => {
            reject_unknown(&object, &["contentIndex", "json"])?;
            Ok(AssistantMessageFrame::ToolCallCheckpoint {
                content_index: take_usize(&mut object, "contentIndex")?,
                json: take_string(&mut object, "json")?,
            })
        }
        "toolcall_delta" => {
            reject_unknown(&object, &["contentIndex", "delta"])?;
            Ok(AssistantMessageFrame::ToolCallDelta {
                content_index: take_usize(&mut object, "contentIndex")?,
                delta: take_string(&mut object, "delta")?,
            })
        }
        "toolcall_end" => {
            reject_unknown(
                &object,
                &[
                    "contentIndex",
                    "id",
                    "name",
                    "arguments",
                    "thoughtSignature",
                    "namespace",
                ],
            )?;
            let arguments = take_value(&mut object, "arguments")?;
            let arguments = match arguments {
                Value::Object(map) => map.into_iter().collect(),
                _ => return Err("toolcall_end arguments must be an object".into()),
            };
            Ok(AssistantMessageFrame::ToolCallEnd {
                content_index: take_usize(&mut object, "contentIndex")?,
                id: take_string(&mut object, "id")?,
                name: take_string(&mut object, "name")?,
                arguments,
                thought_signature: take_optional_string(&mut object, "thoughtSignature")?,
                namespace: take_optional_string(&mut object, "namespace")?,
            })
        }
        other => Err(format!("unknown assistant message frame type: {other}")),
    }
}

fn reject_unknown(
    object: &serde_json::Map<String, Value>,
    allowed_without_type: &[&str],
) -> Result<(), String> {
    for key in object.keys() {
        if !allowed_without_type.contains(&key.as_str()) {
            return Err(format!("unknown assistant message frame field: {key}"));
        }
    }
    Ok(())
}

fn take_value(object: &mut serde_json::Map<String, Value>, key: &str) -> Result<Value, String> {
    object
        .remove(key)
        .ok_or_else(|| format!("assistant message frame missing field: {key}"))
}

fn take_string(object: &mut serde_json::Map<String, Value>, key: &str) -> Result<String, String> {
    take_value(object, key)?
        .as_str()
        .map(ToString::to_string)
        .ok_or_else(|| format!("assistant message frame field {key} must be a string"))
}

fn take_optional_string(
    object: &mut serde_json::Map<String, Value>,
    key: &str,
) -> Result<Option<String>, String> {
    match object.remove(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Ok(Some(value)),
        Some(_) => Err(format!(
            "assistant message frame field {key} must be a string"
        )),
    }
}

fn take_optional_bool(
    object: &mut serde_json::Map<String, Value>,
    key: &str,
) -> Result<Option<bool>, String> {
    match object.remove(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Bool(value)) => Ok(Some(value)),
        Some(_) => Err(format!(
            "assistant message frame field {key} must be a boolean"
        )),
    }
}

fn take_usize(object: &mut serde_json::Map<String, Value>, key: &str) -> Result<usize, String> {
    let value = take_value(object, key)?.as_u64().ok_or_else(|| {
        format!("assistant message frame field {key} must be a non-negative integer")
    })?;
    usize::try_from(value).map_err(|_| format!("assistant message frame field {key} is too large"))
}

fn take_content(
    object: &mut serde_json::Map<String, Value>,
    key: &str,
) -> Result<ContentBlock, String> {
    serde_json::from_value(take_value(object, key)?).map_err(|e| e.to_string())
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
                    redacted: *redacted,
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
                *block_redacted = redacted;
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
