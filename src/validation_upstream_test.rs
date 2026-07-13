//! Test-for-test port of upstream `test/validation.test.ts`
//! (`@earendil-works/pi-ai` v0.80.2) `validateToolArguments` coercion cases.
//!
//! The upstream "still validates when Function constructor is unavailable" case
//! is JS-runtime-specific (rs-ai has no `Function` constructor / codegen path),
//! so only the AJV-compatible coercion cases are portable; they are ported
//! verbatim with identical schema/input/expected values.

#[cfg(test)]
mod tests {
    use crate::types::Tool;
    use crate::validation::validate_tool_arguments;
    use serde_json::{Value, json};

    fn tool_with_value_schema(schema: Value) -> Tool {
        Tool {
            name: "echo".into(),
            description: "Echo tool".into(),
            parameters: json!({
                "type": "object",
                "properties": { "value": schema },
                "required": ["value"],
            }),
        }
    }

    #[test]
    fn coerces_serialized_plain_json_schemas_with_ajv_primitive_rules() {
        let passing: &[(Value, Value, Value)] = &[
            (json!({"type": "number"}), json!("42"), json!(42)),
            (json!({"type": "number"}), json!(true), json!(1)),
            (json!({"type": "number"}), json!(null), json!(0)),
            (json!({"type": "integer"}), json!("42"), json!(42)),
            (json!({"type": "boolean"}), json!("true"), json!(true)),
            (json!({"type": "boolean"}), json!("false"), json!(false)),
            (json!({"type": "boolean"}), json!(1), json!(true)),
            (json!({"type": "boolean"}), json!(0), json!(false)),
            (json!({"type": "string"}), json!(null), json!("")),
            (json!({"type": "string"}), json!(true), json!("true")),
            (json!({"type": "null"}), json!(""), json!(null)),
            (json!({"type": "null"}), json!(0), json!(null)),
            (json!({"type": "null"}), json!(false), json!(null)),
            (
                json!({"type": ["number", "string"]}),
                json!("1"),
                json!("1"),
            ),
            (json!({"type": ["boolean", "number"]}), json!("1"), json!(1)),
        ];
        for (schema, input, expected) in passing {
            let tool = tool_with_value_schema(schema.clone());
            let args = json!({ "value": input });
            let got = validate_tool_arguments(&tool, &args)
                .unwrap_or_else(|e| panic!("schema {schema} input {input}: {e}"));
            assert_eq!(
                got,
                json!({ "value": expected }),
                "schema {schema} input {input}"
            );
        }
    }

    #[test]
    fn rejects_invalid_coercions_for_serialized_plain_json_schemas() {
        let failing: &[(Value, Value)] = &[
            (json!({"type": "boolean"}), json!("1")),
            (json!({"type": "boolean"}), json!("0")),
            (json!({"type": "null"}), json!("null")),
            (json!({"type": "integer"}), json!("42.1")),
        ];
        for (schema, input) in failing {
            let tool = tool_with_value_schema(schema.clone());
            let args = json!({ "value": input });
            let err = validate_tool_arguments(&tool, &args)
                .expect_err(&format!("schema {schema} input {input} must fail"));
            assert!(
                err.contains("Validation failed"),
                "schema {schema} input {input}: {err}"
            );
        }
    }
}
