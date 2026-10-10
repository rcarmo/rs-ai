use crate::durable::model::DurableUsage;
use crate::durable::types::{DurableError, MAX_TASK_FIELD_BYTES};
use crate::types::Tool;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::io::Write;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use tokio::sync::watch;

pub const MAX_OFFERED_TOOLS: usize = 128;
pub const MAX_TOOL_CALLS_PER_ROUND: usize = 32;
pub const MAX_TOOL_ROUNDS: u32 = 8;
const MAX_SCHEMA_DEPTH: usize = 32;
const MAX_SCHEMA_NODES: usize = 4096;
const MAX_STRING_BYTES: usize = 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplayPolicy {
    Safe,
    Unsafe,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolBinding {
    pub definition: Tool,
    pub implementation_id: String,
    pub implementation_version: String,
    pub replay_policy: ReplayPolicy,
    pub schema_identity: String,
}

impl ToolBinding {
    pub fn validate(&self) -> Result<(), DurableError> {
        if self.definition.name.is_empty()
            || self.definition.name.len() > 128
            || self.implementation_id.is_empty()
            || self.implementation_id.len() > 256
            || self.implementation_version.is_empty()
            || self.implementation_version.len() > 128
        {
            return Err(DurableError::Rejected(
                "invalid tool binding identity".into(),
            ));
        }
        validate_schema(&self.definition.parameters)?;
        if schema_identity(&self.definition.parameters)? != self.schema_identity {
            return Err(DurableError::Rejected(
                "tool schema identity mismatch".into(),
            ));
        }
        Ok(())
    }

    pub fn semantically_matches(&self, other: &Self) -> bool {
        self.definition.name == other.definition.name
            && self.definition.description == other.definition.description
            && self.definition.parameters == other.definition.parameters
            && self.definition.constrained_sampling == other.definition.constrained_sampling
            && self.implementation_id == other.implementation_id
            && self.implementation_version == other.implementation_version
            && self.replay_policy == other.replay_policy
            && self.schema_identity == other.schema_identity
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DurableToolCall {
    pub provider_call_id: String,
    pub name: String,
    pub original_arguments: Value,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ToolIntent {
    pub durable_tool_id: String,
    pub durable_idempotency_key: String,
    pub provider_call_id: String,
    pub name: String,
    pub implementation_id: String,
    pub implementation_version: String,
    pub replay_policy: ReplayPolicy,
    pub schema_identity: String,
    pub original_arguments: Value,
    pub execution_arguments: Value,
    pub logical_attempt: u32,
}

impl ToolIntent {
    pub fn validate(&self) -> Result<(), DurableError> {
        if self.durable_tool_id.is_empty()
            || self.durable_tool_id.len() > 256
            || self.durable_idempotency_key.is_empty()
            || self.durable_idempotency_key.len() > 256
            || self.provider_call_id.len() > 256
            || self.logical_attempt == 0
        {
            return Err(DurableError::Rejected("invalid durable tool intent".into()));
        }
        bounded_json(
            "tool original arguments",
            &self.original_arguments,
            MAX_TASK_FIELD_BYTES,
        )?;
        bounded_json(
            "tool execution arguments",
            &self.execution_arguments,
            MAX_TASK_FIELD_BYTES,
        )?;
        Ok(())
    }
}

#[derive(Clone)]
pub struct ToolExecution {
    pub durable_tool_id: String,
    pub durable_idempotency_key: String,
    pub arguments: Value,
    pub cancel: watch::Receiver<bool>,
    pub models: Arc<dyn crate::durable::models::DurableModels>,
}

impl std::fmt::Debug for ToolExecution {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ToolExecution")
            .field("durable_tool_id", &self.durable_tool_id)
            .field("durable_idempotency_key", &self.durable_idempotency_key)
            .field("arguments", &self.arguments)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ToolOutput {
    pub value: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<DurableUsage>,
}
impl ToolOutput {
    pub fn validate(&self) -> Result<(), DurableError> {
        bounded_json("tool result", &self.value, MAX_TASK_FIELD_BYTES)?;
        if let Some(v) = &self.usage {
            v.validate()?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ToolFailure {
    pub code: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<DurableUsage>,
}
impl ToolFailure {
    pub fn validate(&self) -> Result<(), DurableError> {
        if !matches!(
            self.code.as_str(),
            "tool_error" | "tool_aborted" | "invalid_tool_usage"
        ) {
            return Err(DurableError::Rejected(
                "unknown durable tool failure code".into(),
            ));
        }
        if let Some(v) = &self.usage {
            v.validate()?;
        }
        Ok(())
    }
}

pub type ToolFuture<'a> =
    Pin<Box<dyn Future<Output = Result<ToolOutput, ToolFailure>> + Send + 'a>>;
pub trait DurableTool: Send + Sync + 'static {
    fn execute<'a>(&'a self, execution: ToolExecution) -> ToolFuture<'a>;
}
#[derive(Clone)]
pub struct RegisteredTool {
    pub binding: ToolBinding,
    pub executor: Arc<dyn DurableTool>,
}

#[derive(Default)]
struct RegistryState {
    sealed: bool,
    tools: HashMap<String, RegisteredTool>,
    encoded_bytes: usize,
}
#[derive(Default)]
pub struct DurableToolRegistry {
    state: Mutex<RegistryState>,
}
impl DurableToolRegistry {
    pub fn register(
        &self,
        definition: Tool,
        implementation_id: impl Into<String>,
        implementation_version: impl Into<String>,
        replay_policy: ReplayPolicy,
        executor: Arc<dyn DurableTool>,
    ) -> Result<(), DurableError> {
        validate_schema(&definition.parameters)?;
        let binding = ToolBinding {
            schema_identity: schema_identity(&definition.parameters)?,
            definition,
            implementation_id: implementation_id.into(),
            implementation_version: implementation_version.into(),
            replay_policy,
        };
        binding.validate()?;
        let bytes = encode_limited("tool binding", &binding, MAX_TASK_FIELD_BYTES)?.len();
        let mut state = self.state.lock().unwrap();
        if state.sealed {
            return Err(DurableError::Rejected("tool registry is sealed".into()));
        }
        if state.tools.len() >= MAX_OFFERED_TOOLS {
            return Err(DurableError::Rejected("too many offered tools".into()));
        }
        if state.tools.contains_key(&binding.definition.name) {
            return Err(DurableError::Rejected("duplicate durable tool name".into()));
        }
        state.encoded_bytes = state
            .encoded_bytes
            .checked_add(bytes)
            .filter(|v| *v <= MAX_TASK_FIELD_BYTES)
            .ok_or(DurableError::TooLarge {
                field: "offered tools",
                size: state.encoded_bytes.saturating_add(bytes),
                limit: MAX_TASK_FIELD_BYTES,
            })?;
        state.tools.insert(
            binding.definition.name.clone(),
            RegisteredTool { binding, executor },
        );
        Ok(())
    }
    pub fn seal(&self) {
        self.state.lock().unwrap().sealed = true;
    }
    pub fn bindings(&self) -> Vec<ToolBinding> {
        let mut v = self
            .state
            .lock()
            .unwrap()
            .tools
            .values()
            .map(|v| v.binding.clone())
            .collect::<Vec<_>>();
        v.sort_by(|a, b| a.definition.name.cmp(&b.definition.name));
        v
    }
    pub fn get(&self, name: &str) -> Option<RegisteredTool> {
        self.state.lock().unwrap().tools.get(name).cloned()
    }
}

pub fn prepare_intent(
    binding: &ToolBinding,
    call: &DurableToolCall,
    task_id: u64,
    logical_attempt: u32,
) -> Result<ToolIntent, DurableError> {
    if call.name != binding.definition.name || call.provider_call_id.len() > 256 {
        return Err(DurableError::Rejected("unoffered tool call".into()));
    }
    let execution_arguments =
        validate_arguments(&binding.definition.parameters, &call.original_arguments)?;
    let mut nonce = [0u8; 16];
    rand::rngs::OsRng.fill_bytes(&mut nonce);
    let durable_tool_id = format!("tool-{task_id}");
    let key =
        format!("durable:{}:{:02x?}", durable_tool_id, nonce).replace(['[', ']', ' ', ','], "");
    let intent = ToolIntent {
        durable_idempotency_key: key,
        durable_tool_id,
        provider_call_id: call.provider_call_id.clone(),
        name: call.name.clone(),
        implementation_id: binding.implementation_id.clone(),
        implementation_version: binding.implementation_version.clone(),
        replay_policy: binding.replay_policy,
        schema_identity: binding.schema_identity.clone(),
        original_arguments: call.original_arguments.clone(),
        execution_arguments,
        logical_attempt,
    };
    intent.validate()?;
    Ok(intent)
}

pub fn replay_registration(
    registry: &DurableToolRegistry,
    intent: &ToolIntent,
) -> Result<RegisteredTool, &'static str> {
    let Some(current) = registry.get(&intent.name) else {
        return Err("missing_tool");
    };
    if intent.replay_policy != ReplayPolicy::Safe {
        return Err("stored_unsafe");
    };
    if current.binding.replay_policy != ReplayPolicy::Safe {
        return Err("current_unsafe");
    };
    if current.binding.implementation_id != intent.implementation_id {
        return Err("implementation_changed");
    };
    if current.binding.implementation_version != intent.implementation_version {
        return Err("version_changed");
    };
    if current.binding.schema_identity != intent.schema_identity {
        return Err("schema_changed");
    };
    Ok(current)
}
pub fn interrupted_payload(intent: &ToolIntent, reason: &str) -> Value {
    json!({"code":"durable_tool_interrupted","tool_name":intent.name,"stored_implementation_id":intent.implementation_id,"stored_implementation_version":intent.implementation_version,"reason":reason})
}

fn validate_schema(schema: &Value) -> Result<(), DurableError> {
    preflight(schema, MAX_TASK_FIELD_BYTES)?;
    let o = schema
        .as_object()
        .ok_or_else(|| DurableError::Rejected("tool schema must be an object".into()))?;
    let allowed = HashSet::from(["type", "properties", "required", "additionalProperties"]);
    if o.keys().any(|k| !allowed.contains(k.as_str()))
        || o.get("type").and_then(Value::as_str) != Some("object")
        || o.get("additionalProperties")
            .is_some_and(|v| v != &Value::Bool(false))
    {
        return Err(DurableError::Rejected(
            "unsupported tool schema keyword".into(),
        ));
    }
    let p = o
        .get("properties")
        .and_then(Value::as_object)
        .ok_or_else(|| DurableError::Rejected("tool schema requires properties".into()))?;
    if p.len() > 256 {
        return Err(DurableError::Rejected(
            "too many tool schema properties".into(),
        ));
    }
    for (n, s) in p {
        if n.is_empty() || n.len() > 128 {
            return Err(DurableError::Rejected("invalid tool property name".into()));
        }
        let s = s
            .as_object()
            .ok_or_else(|| DurableError::Rejected("invalid tool property schema".into()))?;
        if s.len() != 1
            || !matches!(
                s.get("type").and_then(Value::as_str),
                Some("string" | "number" | "integer" | "boolean")
            )
        {
            return Err(DurableError::Rejected(
                "unsupported tool property schema".into(),
            ));
        }
    }
    if let Some(r) = o.get("required") {
        for n in r
            .as_array()
            .ok_or_else(|| DurableError::Rejected("required must be array".into()))?
        {
            let n = n
                .as_str()
                .ok_or_else(|| DurableError::Rejected("required name must be string".into()))?;
            if !p.contains_key(n) {
                return Err(DurableError::Rejected(
                    "required property not offered".into(),
                ));
            }
        }
    }
    Ok(())
}
fn validate_arguments(schema: &Value, args: &Value) -> Result<Value, DurableError> {
    validate_schema(schema)?;
    preflight(args, MAX_TASK_FIELD_BYTES)?;
    let a = args
        .as_object()
        .ok_or_else(|| DurableError::Rejected("tool arguments must be object".into()))?;
    let s = schema.as_object().expect("validated");
    let p = s["properties"].as_object().expect("validated");
    if a.keys().any(|n| !p.contains_key(n)) {
        return Err(DurableError::Rejected("unknown tool argument".into()));
    }
    if s.get("required")
        .and_then(Value::as_array)
        .is_some_and(|r| {
            r.iter()
                .filter_map(Value::as_str)
                .any(|n| !a.contains_key(n))
        })
    {
        return Err(DurableError::Rejected(
            "missing required tool argument".into(),
        ));
    }
    let mut repaired = Map::new();
    for (n, v) in a {
        let t = p[n]["type"].as_str().expect("validated");
        let ok = match t {
            "string" => v.is_string(),
            "number" => v.is_number(),
            "integer" => v.as_i64().is_some() || v.as_u64().is_some(),
            "boolean" => v.is_boolean(),
            _ => false,
        };
        if !ok {
            return Err(DurableError::Rejected("tool argument type mismatch".into()));
        }
        repaired.insert(n.clone(), v.clone());
    }
    Ok(Value::Object(repaired))
}
fn schema_identity(schema: &Value) -> Result<String, DurableError> {
    let bytes = encode_limited("tool schema", schema, MAX_TASK_FIELD_BYTES)?;
    Ok(format!("sha256:{:x}", Sha256::digest(bytes)))
}

fn preflight(value: &Value, limit: usize) -> Result<(), DurableError> {
    let mut stack = vec![(value, 1usize)];
    let mut nodes = 0usize;
    while let Some((v, d)) = stack.pop() {
        nodes += 1;
        if d > MAX_SCHEMA_DEPTH || nodes > MAX_SCHEMA_NODES {
            return Err(DurableError::Rejected(
                "durable JSON structure exceeds limits".into(),
            ));
        }
        match v {
            Value::String(s) if s.len() > MAX_STRING_BYTES => {
                return Err(DurableError::TooLarge {
                    field: "durable JSON string",
                    size: s.len(),
                    limit: MAX_STRING_BYTES,
                });
            }
            Value::Array(a) => stack.extend(a.iter().map(|v| (v, d + 1))),
            Value::Object(o) => {
                for (k, v) in o {
                    if k.len() > MAX_STRING_BYTES {
                        return Err(DurableError::TooLarge {
                            field: "durable JSON key",
                            size: k.len(),
                            limit: MAX_STRING_BYTES,
                        });
                    }
                    stack.push((v, d + 1));
                }
            }
            _ => {}
        }
    }
    let _ = encode_limited("durable JSON", value, limit)?;
    Ok(())
}
struct LimitedWriter {
    bytes: Vec<u8>,
    limit: usize,
}
impl Write for LimitedWriter {
    fn write(&mut self, v: &[u8]) -> std::io::Result<usize> {
        if self.bytes.len().saturating_add(v.len()) > self.limit {
            return Err(std::io::Error::new(
                std::io::ErrorKind::FileTooLarge,
                "limit",
            ));
        }
        self.bytes.extend_from_slice(v);
        Ok(v.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
fn encode_limited<T: Serialize>(
    field: &'static str,
    value: &T,
    limit: usize,
) -> Result<Vec<u8>, DurableError> {
    let mut w = LimitedWriter {
        bytes: Vec::new(),
        limit,
    };
    serde_json::to_writer(&mut w, value).map_err(|e| {
        if e.io_error_kind() == Some(std::io::ErrorKind::FileTooLarge) {
            DurableError::TooLarge {
                field,
                size: w.bytes.len().saturating_add(1),
                limit,
            }
        } else {
            DurableError::Rejected(e.to_string())
        }
    })?;
    Ok(w.bytes)
}
fn bounded_json(field: &'static str, value: &Value, limit: usize) -> Result<(), DurableError> {
    preflight(value, limit).map_err(|e| match e {
        DurableError::TooLarge { size, limit, .. } => DurableError::TooLarge { field, size, limit },
        other => other,
    })
}
