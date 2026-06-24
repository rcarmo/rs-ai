//! Test-for-test port of upstream `test/google-shared-convert-tools.test.ts`
//! (`@earendil-works/pi-ai` v0.80.2).

#[cfg(test)]
mod tests {
    use crate::provider::google::convert_google_tools;
    use crate::types::Tool;
    use serde_json::{json, Value};

    fn make_tool(parameters: Value) -> Tool {
        Tool { name: "test_tool".into(), description: "A test tool".into(), parameters }
    }

    fn decl_params(tools: &[Tool], use_parameters: bool) -> Value {
        let result = convert_google_tools(tools, use_parameters).expect("tools");
        let decl = &result[0]["functionDeclarations"][0];
        if use_parameters { decl["parameters"].clone() } else { decl["parametersJsonSchema"].clone() }
    }

    #[test]
    fn strips_json_schema_meta_keys_when_use_parameters_true() {
        let tools = vec![make_tool(json!({
            "$schema": "http://json-schema.org/draft-07/schema#",
            "$id": "urn:bash-tool",
            "$comment": "A bash tool for demonstration",
            "$defs": {"commandDef": {"type": "string"}},
            "definitions": {"legacyDef": {"type": "number"}},
            "type": "object",
            "properties": {"command": {"type": "string"}},
            "required": ["command"],
        }))];
        let p = decl_params(&tools, true);
        assert_eq!(p, json!({
            "type": "object",
            "properties": {"command": {"type": "string"}},
            "required": ["command"],
        }));
    }

    #[test]
    fn recursively_strips_nested_json_schema_meta_keys() {
        let tools = vec![make_tool(json!({
            "$schema": "http://json-schema.org/draft-07/schema#",
            "type": "object",
            "properties": {"deep": {"$schema": "x", "$id": "urn:nested", "type": "string"}},
        }))];
        let p = decl_params(&tools, true);
        assert_eq!(p, json!({"type": "object", "properties": {"deep": {"type": "string"}}}));
    }

    #[test]
    fn preserves_ref_while_stripping_meta_keys() {
        let tools = vec![make_tool(json!({
            "$schema": "x",
            "type": "object",
            "properties": {"refProp": {"$ref": "#/$defs/someDef", "type": "string"}},
        }))];
        let p = decl_params(&tools, true);
        assert_eq!(p, json!({
            "type": "object",
            "properties": {"refProp": {"$ref": "#/$defs/someDef", "type": "string"}},
        }));
    }

    #[test]
    fn does_not_mutate_the_original_tool_parameters() {
        let original = json!({
            "$schema": "http://json-schema.org/draft-07/schema#",
            "type": "object",
            "properties": {"command": {"type": "string"}},
            "required": ["command"],
        });
        let tools = vec![make_tool(original.clone())];
        let _ = convert_google_tools(&tools, true);
        assert_eq!(tools[0].parameters, original, "original parameters must be unchanged");
    }

    #[test]
    fn preserves_schema_in_parameters_json_schema_when_use_parameters_false() {
        let tools = vec![make_tool(json!({
            "$schema": "http://json-schema.org/draft-07/schema#",
            "type": "object",
            "properties": {"command": {"type": "string"}},
            "required": ["command"],
        }))];
        let p = decl_params(&tools, false);
        assert_eq!(p, json!({
            "$schema": "http://json-schema.org/draft-07/schema#",
            "type": "object",
            "properties": {"command": {"type": "string"}},
            "required": ["command"],
        }));
    }

    #[test]
    fn handles_tools_without_schema_gracefully() {
        let tools = vec![make_tool(json!({
            "type": "object",
            "properties": {"path": {"type": "string"}},
            "required": ["path"],
        }))];
        let p = decl_params(&tools, true);
        assert_eq!(p, json!({"type": "object", "properties": {"path": {"type": "string"}}, "required": ["path"]}));
    }

    #[test]
    fn returns_none_for_empty_tool_list() {
        assert!(convert_google_tools(&[], false).is_none());
        assert!(convert_google_tools(&[], true).is_none());
    }
}
