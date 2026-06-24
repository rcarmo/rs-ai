//! Test-for-test port (deterministic substance) of upstream
//! `test/anthropic-tool-name-normalization.test.ts` (`@earendil-works/pi-ai` v0.80.2).
//!
//! Under OAuth (Claude Code compat), user tool names that match a built-in
//! Claude Code tool name (case-insensitively) are normalized to the canonical
//! casing on the way out and mapped back to the registered name on the way in;
//! non-matching names (e.g. `find`, which is not a CC tool) pass through. The
//! live OAuth round-trip is N/A; the pure name-mapping is ported here.

#[cfg(test)]
mod tests {
    use crate::provider::anthropic::{from_claude_code_name, to_claude_code_name};
    use crate::types::Tool;
    use serde_json::json;

    fn tool(name: &str) -> Tool {
        Tool { name: name.into(), description: "t".into(), parameters: json!({"type": "object"}) }
    }

    #[test]
    fn normalizes_user_tool_matching_cc_name_todowrite_to_canonical_casing() {
        assert_eq!(to_claude_code_name("todowrite"), "TodoWrite");
        // ...and maps back to the registered context tool name.
        let tools = vec![tool("todowrite")];
        assert_eq!(from_claude_code_name("TodoWrite", &tools), "todowrite");
    }

    #[test]
    fn handles_pi_builtin_tools_read_write_edit_bash() {
        assert_eq!(to_claude_code_name("read"), "Read");
        assert_eq!(to_claude_code_name("write"), "Write");
        assert_eq!(to_claude_code_name("edit"), "Edit");
        assert_eq!(to_claude_code_name("bash"), "Bash");
    }

    #[test]
    fn does_not_map_find_to_glob_find_is_not_a_cc_tool() {
        assert_eq!(to_claude_code_name("find"), "find");
    }

    #[test]
    fn handles_custom_tools_that_dont_match_any_cc_tool_names() {
        assert_eq!(to_claude_code_name("my_custom_tool"), "my_custom_tool");
        let tools = vec![tool("my_custom_tool")];
        assert_eq!(from_claude_code_name("my_custom_tool", &tools), "my_custom_tool");
        // An unknown incoming name with no matching context tool passes through.
        assert_eq!(from_claude_code_name("Unknown", &tools), "Unknown");
    }
}
