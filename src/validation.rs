//! Input validation helpers.

use crate::types::{Context, Tool};

/// Validation error.
#[derive(Debug, Clone)]
pub struct ValidationError {
    pub field: String,
    pub message: String,
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.field, self.message)
    }
}

impl std::error::Error for ValidationError {}

/// Validate a context before sending to a provider.
pub fn validate_context(ctx: &Context) -> Result<(), Vec<ValidationError>> {
    let mut errors = Vec::new();

    if ctx.messages.is_empty() {
        errors.push(ValidationError {
            field: "messages".into(),
            message: "at least one message is required".into(),
        });
    }

    // Check tool definitions have valid JSON Schema parameters
    for (i, tool) in ctx.tools.iter().enumerate() {
        if let Err(e) = validate_tool(tool) {
            errors.push(ValidationError {
                field: format!("tools[{}]", i),
                message: e,
            });
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn validate_tool(tool: &Tool) -> Result<(), String> {
    if tool.name.is_empty() {
        return Err("tool name is required".into());
    }
    if tool.description.is_empty() {
        return Err("tool description is required".into());
    }
    // Parameters must be a valid JSON object (schema)
    if !tool.parameters.is_object() && !tool.parameters.is_null() {
        return Err("parameters must be a JSON object (schema)".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Context, Tool, user_message};
    use serde_json::json;

    #[test]
    fn test_valid_context() {
        let ctx = Context {
            system_prompt: Some("You are helpful.".into()),
            messages: vec![user_message("hi")],
            tools: vec![Tool {
                name: "search".into(),
                description: "Search the web".into(),
                parameters: json!({"type": "object", "properties": {}}),
                constrained_sampling: None,
            }],
        };
        assert!(validate_context(&ctx).is_ok());
    }

    #[test]
    fn test_empty_messages() {
        let ctx = Context {
            system_prompt: None,
            messages: vec![],
            tools: vec![],
        };
        let errs = validate_context(&ctx).unwrap_err();
        assert_eq!(errs.len(), 1);
        assert!(errs[0].message.contains("at least one message"));
    }

    #[test]
    fn test_invalid_tool() {
        let ctx = Context {
            system_prompt: None,
            messages: vec![user_message("hi")],
            tools: vec![Tool {
                name: "".into(),
                description: "desc".into(),
                parameters: json!({}),
                constrained_sampling: None,
            }],
        };
        let errs = validate_context(&ctx).unwrap_err();
        assert!(errs[0].message.contains("name"));
    }
}

/// Validate a tool call's arguments against a schema (basic type check).
pub fn validate_tool_arguments(
    tool: &crate::types::Tool,
    args: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let coerced = coerce_with_json_schema(args.clone(), &tool.parameters);
    if check_schema(&coerced, &tool.parameters) {
        Ok(coerced)
    } else {
        let mut errors = Vec::new();
        collect_schema_errors(&coerced, &tool.parameters, "", &mut errors);
        let detail = if errors.is_empty() {
            "Unknown validation error".to_string()
        } else {
            errors.join("\n")
        };
        Err(format!(
            "Validation failed for tool \"{}\":\n{}\n\nReceived arguments:\n{}",
            tool.name,
            detail,
            serde_json::to_string_pretty(args).unwrap_or_default()
        ))
    }
}

/// Validate a tool call (name exists in context tools, args coerced + validated).
/// Returns the coerced arguments on success (mirrors validateToolCall).
pub fn validate_tool_call(
    ctx: &crate::types::Context,
    name: &str,
    args: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let tool = ctx.tools.iter().find(|t| t.name == name);
    match tool {
        Some(t) => validate_tool_arguments(t, args),
        None => Err(format!("Tool \"{}\" not found", name)),
    }
}

/// Tool call limit configuration.
#[derive(Debug, Clone)]
pub struct ToolCallLimitConfig {
    pub max_parallel_calls: usize,
}

impl Default for ToolCallLimitConfig {
    fn default() -> Self {
        Self {
            max_parallel_calls: 128,
        }
    }
}

/// Default tool call limit config.
pub fn default_tool_call_limit_config() -> ToolCallLimitConfig {
    ToolCallLimitConfig::default()
}

/// Apply tool call limit (truncate excess calls).
pub fn apply_tool_call_limit(
    calls: &[serde_json::Value],
    config: &ToolCallLimitConfig,
) -> Vec<serde_json::Value> {
    if calls.len() <= config.max_parallel_calls {
        calls.to_vec()
    } else {
        calls[..config.max_parallel_calls].to_vec()
    }
}

// ============================================================================
// JSON-schema coercion + validation for tool arguments (mirrors validation.js).
// ============================================================================

use serde_json::{Value, json};

fn schema_types(schema: &Value) -> Vec<String> {
    match schema.get("type") {
        Some(Value::String(s)) => vec![s.clone()],
        Some(Value::Array(arr)) => arr
            .iter()
            .filter_map(|t| t.as_str().map(|s| s.to_string()))
            .collect(),
        _ => Vec::new(),
    }
}

fn matches_json_type(value: &Value, ty: &str) -> bool {
    match ty {
        "number" => value.is_number(),
        "integer" => value.is_number() && value.as_f64().map(|n| n.fract() == 0.0).unwrap_or(false),
        "boolean" => value.is_boolean(),
        "string" => value.is_string(),
        "null" => value.is_null(),
        "array" => value.is_array(),
        "object" => value.is_object(),
        _ => false,
    }
}

fn number_value(n: f64) -> Value {
    if n.fract() == 0.0 && n.abs() < 9.007_199_254_740_992e15 {
        json!(n as i64)
    } else {
        json!(n)
    }
}

/// Mirror JavaScript `Number(string)` for the already-trimmed, non-empty case:
/// hex/octal/binary prefixes (0x/0o/0b) parse as integers; everything else parses
/// as a decimal/scientific float. Rust's `f64::parse` rejects the radix prefixes
/// that JS accepts, so handle them explicitly to coerce the same values upstream does.
fn js_number(t: &str) -> Option<f64> {
    let radix_digits = t
        .strip_prefix("0x")
        .or_else(|| t.strip_prefix("0X"))
        .map(|d| (16u32, d))
        .or_else(|| {
            t.strip_prefix("0o")
                .or_else(|| t.strip_prefix("0O"))
                .map(|d| (8u32, d))
        })
        .or_else(|| {
            t.strip_prefix("0b")
                .or_else(|| t.strip_prefix("0B"))
                .map(|d| (2u32, d))
        });
    if let Some((radix, digits)) = radix_digits {
        // JS requires at least one digit and rejects a sign on radix literals.
        return u64::from_str_radix(digits, radix).ok().map(|n| n as f64);
    }
    t.parse::<f64>().ok()
}

fn coerce_primitive_by_type(value: &Value, ty: &str) -> Value {
    match ty {
        "number" => {
            if value.is_null() {
                return json!(0);
            }
            if let Some(s) = value.as_str()
                && !s.trim().is_empty()
                && let Some(p) = js_number(s.trim())
                && p.is_finite()
            {
                return number_value(p);
            }
            if let Some(b) = value.as_bool() {
                return json!(if b { 1 } else { 0 });
            }
            value.clone()
        }
        "integer" => {
            if value.is_null() {
                return json!(0);
            }
            if let Some(s) = value.as_str()
                && !s.trim().is_empty()
                && let Some(p) = js_number(s.trim())
                && p.fract() == 0.0
            {
                return number_value(p);
            }
            if let Some(b) = value.as_bool() {
                return json!(if b { 1 } else { 0 });
            }
            value.clone()
        }
        "boolean" => {
            if value.is_null() {
                return json!(false);
            }
            if let Some(s) = value.as_str() {
                if s == "true" {
                    return json!(true);
                }
                if s == "false" {
                    return json!(false);
                }
            }
            if let Some(n) = value.as_f64() {
                if n == 1.0 {
                    return json!(true);
                }
                if n == 0.0 {
                    return json!(false);
                }
            }
            value.clone()
        }
        "string" => {
            if value.is_null() {
                return json!("");
            }
            match value {
                Value::Number(n) => json!(n.to_string()),
                Value::Bool(b) => json!(b.to_string()),
                _ => value.clone(),
            }
        }
        "null" => {
            let empty = value.as_str() == Some("")
                || value.as_f64() == Some(0.0)
                || value.as_bool() == Some(false);
            if empty { Value::Null } else { value.clone() }
        }
        _ => value.clone(),
    }
}

fn coerce_with_union(value: &Value, schemas: &[Value]) -> Value {
    // v0.84.0: preserve a value that already matches a nullable anyOf/oneOf arm
    // before trying coercions. Otherwise `null` can be coerced by a preceding
    // number branch to `0`, changing a valid nullable union value.
    if schemas.iter().any(|schema| check_schema(value, schema)) {
        return value.clone();
    }
    for schema in schemas {
        let coerced = coerce_with_json_schema(value.clone(), schema);
        if check_schema(&coerced, schema) {
            return coerced;
        }
    }
    value.clone()
}

/// Coerce a value toward a JSON schema (mirrors coerceWithJsonSchema).
pub fn coerce_with_json_schema(value: Value, schema: &Value) -> Value {
    let mut next = value;
    if let Some(Value::Array(all_of)) = schema.get("allOf") {
        for nested in all_of {
            next = coerce_with_json_schema(next, nested);
        }
    }
    if let Some(Value::Array(any_of)) = schema.get("anyOf") {
        next = coerce_with_union(&next, any_of);
    }
    if let Some(Value::Array(one_of)) = schema.get("oneOf") {
        next = coerce_with_union(&next, one_of);
    }
    let types = schema_types(schema);
    let matches_union = types.len() > 1 && types.iter().any(|t| matches_json_type(&next, t));
    if !types.is_empty() && !matches_union {
        for ty in &types {
            let candidate = coerce_primitive_by_type(&next, ty);
            if candidate != next {
                next = candidate;
                break;
            }
        }
    }
    if types.iter().any(|t| t == "object") && next.is_object() {
        apply_object_coercion(&mut next, schema);
    }
    if types.iter().any(|t| t == "array") && next.is_array() {
        apply_array_coercion(&mut next, schema);
    }
    next
}

fn apply_object_coercion(value: &mut Value, schema: &Value) {
    let obj = match value.as_object_mut() {
        Some(o) => o,
        None => return,
    };
    let mut defined: Vec<String> = Vec::new();
    if let Some(Value::Object(props)) = schema.get("properties") {
        defined = props.keys().cloned().collect();
        for (key, prop_schema) in props {
            if let Some(existing) = obj.get(key).cloned() {
                obj.insert(key.clone(), coerce_with_json_schema(existing, prop_schema));
            }
        }
    }
    if let Some(add) = schema.get("additionalProperties")
        && add.is_object()
    {
        let keys: Vec<String> = obj.keys().cloned().collect();
        for key in keys {
            if defined.contains(&key) {
                continue;
            }
            if let Some(existing) = obj.get(&key).cloned() {
                obj.insert(key, coerce_with_json_schema(existing, add));
            }
        }
    }
}

fn apply_array_coercion(value: &mut Value, schema: &Value) {
    let arr = match value.as_array_mut() {
        Some(a) => a,
        None => return,
    };
    match schema.get("items") {
        Some(Value::Array(item_schemas)) => {
            for (i, item) in arr.iter_mut().enumerate() {
                if let Some(item_schema) = item_schemas.get(i) {
                    *item = coerce_with_json_schema(item.clone(), item_schema);
                }
            }
        }
        Some(item_schema) if item_schema.is_object() => {
            for item in arr.iter_mut() {
                *item = coerce_with_json_schema(item.clone(), item_schema);
            }
        }
        _ => {}
    }
}

/// Lenient JSON-schema check: type match (incl. anyOf/oneOf/allOf), required
/// properties, and nested property/item validation.
pub fn check_schema(value: &Value, schema: &Value) -> bool {
    if let Some(Value::Array(all_of)) = schema.get("allOf")
        && !all_of.iter().all(|s| check_schema(value, s))
    {
        return false;
    }
    if let Some(Value::Array(any_of)) = schema.get("anyOf")
        && !any_of.iter().any(|s| check_schema(value, s))
    {
        return false;
    }
    if let Some(Value::Array(one_of)) = schema.get("oneOf")
        && !one_of.iter().any(|s| check_schema(value, s))
    {
        return false;
    }
    let types = schema_types(schema);
    if !types.is_empty() && !types.iter().any(|t| matches_json_type(value, t)) {
        return false;
    }
    // enum: the value must be one of the allowed values (TypeBox validates this).
    if let Some(Value::Array(allowed)) = schema.get("enum")
        && !allowed.iter().any(|e| e == value)
    {
        return false;
    }
    // const: the value must equal the fixed value.
    if let Some(c) = schema.get("const")
        && c != value
    {
        return false;
    }
    if types.iter().any(|t| t == "object")
        && let Some(obj) = value.as_object()
    {
        if let Some(Value::Array(required)) = schema.get("required") {
            for req in required {
                if let Some(key) = req.as_str()
                    && !obj.contains_key(key)
                {
                    return false;
                }
            }
        }
        if let Some(Value::Object(props)) = schema.get("properties") {
            for (key, prop_schema) in props {
                if let Some(v) = obj.get(key)
                    && !check_schema(v, prop_schema)
                {
                    return false;
                }
            }
        }
    }
    if types.iter().any(|t| t == "array")
        && let Some(arr) = value.as_array()
        && let Some(items) = schema.get("items")
        && items.is_object()
        && !arr.iter().all(|item| check_schema(item, items))
    {
        return false;
    }
    true
}

/// Collect human-readable `  - path: message` validation errors for a value against
/// a JSON schema (mirrors the error list in validateToolArguments). Messages are
/// approximate; the structure (header + error lines + received args) matches upstream.
fn collect_schema_errors(value: &Value, schema: &Value, path: &str, out: &mut Vec<String>) {
    let here = if path.is_empty() {
        "root".to_string()
    } else {
        path.to_string()
    };

    // Composition keywords: only an error when no branch matches.
    if let Some(Value::Array(any_of)) = schema.get("anyOf")
        && !any_of.iter().any(|s| check_schema(value, s))
    {
        out.push(format!("  - {here}: does not match any allowed schema"));
        return;
    }
    if let Some(Value::Array(one_of)) = schema.get("oneOf")
        && one_of.iter().filter(|s| check_schema(value, s)).count() != 1
    {
        out.push(format!("  - {here}: must match exactly one allowed schema"));
        return;
    }
    if let Some(Value::Array(all_of)) = schema.get("allOf") {
        for nested in all_of {
            collect_schema_errors(value, nested, path, out);
        }
    }

    let types = schema_types(schema);
    if !types.is_empty() && !types.iter().any(|t| matches_json_type(value, t)) {
        out.push(format!("  - {}: Expected {}", here, types.join(" or ")));
        return;
    }
    if let Some(Value::Array(allowed)) = schema.get("enum")
        && !allowed.iter().any(|e| e == value)
    {
        out.push(format!(
            "  - {here}: value is not one of the allowed enum values"
        ));
        return;
    }
    if let Some(c) = schema.get("const")
        && c != value
    {
        out.push(format!(
            "  - {here}: value does not equal the required constant"
        ));
        return;
    }
    if types.iter().any(|t| t == "object")
        && let Some(obj) = value.as_object()
    {
        if let Some(Value::Array(required)) = schema.get("required") {
            for req in required {
                if let Some(key) = req.as_str()
                    && !obj.contains_key(key)
                {
                    let p = if path.is_empty() {
                        key.to_string()
                    } else {
                        format!("{path}.{key}")
                    };
                    out.push(format!("  - {p}: Required property missing"));
                }
            }
        }
        if let Some(Value::Object(props)) = schema.get("properties") {
            for (key, prop_schema) in props {
                if let Some(v) = obj.get(key) {
                    let p = if path.is_empty() {
                        key.clone()
                    } else {
                        format!("{path}.{key}")
                    };
                    collect_schema_errors(v, prop_schema, &p, out);
                }
            }
        }
    }
    if types.iter().any(|t| t == "array")
        && let Some(arr) = value.as_array()
        && let Some(items) = schema.get("items")
        && items.is_object()
    {
        for (i, item) in arr.iter().enumerate() {
            let p = format!("{path}.{i}");
            collect_schema_errors(item, items, &p, out);
        }
    }
}

#[cfg(test)]
mod coercion_tests {
    use super::*;
    use crate::types::Tool;
    use serde_json::json;

    #[test]
    fn test_coerce_primitives_and_validate() {
        let t = Tool {
            name: "t".into(),
            description: "d".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "n": {"type": "number"}, "i": {"type": "integer"},
                    "b": {"type": "boolean"}, "s": {"type": "string"},
                },
                "required": ["n", "i", "b", "s"],
            }),
            constrained_sampling: None,
        };
        let out = validate_tool_arguments(&t, &json!({"n": "3.5", "i": "7", "b": "true", "s": 42}))
            .unwrap();
        assert_eq!(out["n"], json!(3.5));
        assert_eq!(out["i"], json!(7));
        assert_eq!(out["b"], json!(true));
        assert_eq!(out["s"], json!("42"));
    }

    #[test]
    fn test_coerce_radix_prefixed_number_strings() {
        // JS Number() parses 0x/0o/0b prefixes; coercion must accept them too so a
        // tool call isn't rejected where upstream would coerce + accept it.
        let t = Tool {
            name: "t".into(),
            description: "d".into(),
            parameters: json!({
                "type": "object",
                "properties": { "n": {"type": "number"}, "i": {"type": "integer"} },
                "required": ["n", "i"],
            }),
            constrained_sampling: None,
        };
        let out = validate_tool_arguments(&t, &json!({"n": "0x1f", "i": "0b101"})).unwrap();
        assert_eq!(out["n"], json!(31));
        assert_eq!(out["i"], json!(5));
        // Decimal/scientific still work; a non-number string still fails coercion.
        let out2 = validate_tool_arguments(&t, &json!({"n": "1e3", "i": "017"})).unwrap();
        assert_eq!(out2["n"], json!(1000));
        assert_eq!(out2["i"], json!(17));
        assert!(validate_tool_arguments(&t, &json!({"n": "abc", "i": 1})).is_err());
    }

    #[test]
    fn test_enum_and_const_validation() {
        // enum: out-of-set value is rejected (TypeBox validates this); in-set passes,
        // and a coercible value that matches after coercion passes.
        let t = Tool {
            name: "t".into(),
            description: "d".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "unit": {"type": "string", "enum": ["celsius", "fahrenheit"]},
                    "level": {"type": "integer", "enum": [1, 2, 3]},
                },
                "required": ["unit", "level"],
            }),
            constrained_sampling: None,
        };
        assert!(validate_tool_arguments(&t, &json!({"unit": "kelvin", "level": 1})).is_err());
        let ok = validate_tool_arguments(&t, &json!({"unit": "celsius", "level": "2"})).unwrap();
        assert_eq!(ok["unit"], "celsius");
        assert_eq!(ok["level"], json!(2)); // "2" coerced then matches enum

        // const.
        let tc = Tool {
            name: "t".into(),
            description: "d".into(),
            parameters: json!({
                "type": "object",
                "properties": { "kind": {"const": "fixed"} },
                "required": ["kind"],
            }),
            constrained_sampling: None,
        };
        assert!(validate_tool_arguments(&tc, &json!({"kind": "other"})).is_err());
        assert!(validate_tool_arguments(&tc, &json!({"kind": "fixed"})).is_ok());
    }

    #[test]
    fn test_missing_required_fails() {
        let t = Tool {
            name: "t".into(),
            description: "d".into(),
            parameters: json!({
                "type": "object", "properties": {"q": {"type": "string"}}, "required": ["q"],
            }),
            constrained_sampling: None,
        };
        assert!(validate_tool_arguments(&t, &json!({})).is_err());
        assert_eq!(
            validate_tool_arguments(&t, &json!({"q": 5})).unwrap()["q"],
            json!("5")
        );
    }

    #[test]
    fn test_validation_error_message_includes_detail() {
        // Missing required + type mismatch should be listed; the message structure
        // mirrors upstream (header + `  - path: message` lines + received args).
        let t = Tool {
            name: "search".into(),
            description: "d".into(),
            parameters: json!({
                "type": "object",
                "properties": {"q": {"type": "string"}, "n": {"type": "object"}},
                "required": ["q"],
            }),
            constrained_sampling: None,
        };
        let err = validate_tool_arguments(&t, &json!({"n": 5})).unwrap_err();
        assert!(
            err.starts_with("Validation failed for tool \"search\":"),
            "{err}"
        );
        assert!(err.contains("  - q: Required property missing"), "{err}");
        assert!(err.contains("  - n: Expected object"), "{err}");
        assert!(err.contains("Received arguments:"), "{err}");
    }

    #[test]
    fn test_nested_array_items_coercion() {
        let t = Tool {
            name: "t".into(),
            description: "d".into(),
            parameters: json!({
                "type": "object", "properties": {"nums": {"type": "array", "items": {"type": "number"}}},
            }),
            constrained_sampling: None,
        };
        let out = validate_tool_arguments(&t, &json!({"nums": ["1", "2", "3"]})).unwrap();
        assert_eq!(out["nums"], json!([1, 2, 3]));
    }
}
