use indexmap::IndexMap;
use serde_json::json;

use crate::transcript::{
    collapse_system_messages, declarations_equal, get_current_system_message,
    get_current_system_prompt, get_tool_state_changes, has_non_additive_tool_changes,
    has_tool_redefinitions, normalize_context, render_system_message_update,
    resolve_transcript_tools, system_message,
};
use crate::types::{Context, Role, Tool, ToolReference, user_message};

fn tool(name: &str, description: &str) -> Tool {
    Tool {
        name: name.into(),
        description: description.into(),
        parameters: json!({"type": "object", "properties": {}}),
        constrained_sampling: None,
    }
}

fn replay_context() -> Context {
    let mut initial_sections = IndexMap::new();
    initial_sections.insert("a".into(), Some("<a>1</a>".into()));
    initial_sections.insert("b".into(), Some("<b>1</b>".into()));
    let mut updates = IndexMap::new();
    updates.insert("a".into(), Some("<a>2</a>".into()));
    updates.insert("b".into(), None);
    updates.insert("c".into(), Some("<c>1</c>".into()));

    let mut first = system_message(
        "base",
        Some(initial_sections),
        vec![tool("first", "first tool")],
        vec![],
    );
    first.timestamp = 10;
    let mut later = system_message(
        "also do this",
        Some(updates),
        vec![tool("second", "second tool")],
        vec![ToolReference {
            name: "first".into(),
        }],
    );
    later.timestamp = 14;
    Context {
        system_prompt: None,
        messages: vec![first, user_message("hello"), later],
        tools: vec![],
    }
}

#[test]
fn replays_content_ordered_sections_tools_and_first_timestamp() {
    let context = replay_context();
    let current = get_current_system_message(&context.messages).unwrap();
    assert_eq!(current.timestamp, 10);
    assert_eq!(
        current
            .tools_added
            .iter()
            .map(|tool| tool.name.as_str())
            .collect::<Vec<_>>(),
        ["second"]
    );
    assert_eq!(
        current
            .sections
            .as_ref()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["a", "c"]
    );
    assert_eq!(
        get_current_system_prompt(&context.messages),
        "base\n\nalso do this\n\n<a>2</a>\n\n<c>1</c>"
    );
}

#[test]
fn collapse_is_idempotent_and_keeps_one_leading_system_message() {
    let collapsed = collapse_system_messages(&replay_context());
    assert_eq!(
        collapsed
            .messages
            .iter()
            .map(|message| message.role.clone())
            .collect::<Vec<_>>(),
        [Role::System, Role::User]
    );
    let twice = collapse_system_messages(&collapsed);
    assert_eq!(
        serde_json::to_value(twice).unwrap(),
        serde_json::to_value(collapsed).unwrap()
    );
}

#[test]
fn normalizes_legacy_prompt_and_tools_into_a_leading_system_message() {
    let context = Context {
        system_prompt: Some("be brief".into()),
        messages: vec![user_message("hi")],
        tools: vec![tool("a", "a tool")],
    };
    let normalized = normalize_context(&context);
    assert!(normalized.system_prompt.is_none());
    assert!(normalized.tools.is_empty());
    assert_eq!(normalized.messages[0].role, Role::System);
    assert_eq!(get_current_system_prompt(&normalized.messages), "be brief");
    assert_eq!(normalized.messages[0].tools_added[0].name, "a");
}

#[test]
fn renders_framed_system_updates_in_source_order() {
    let update = &replay_context().messages[2];
    assert_eq!(
        render_system_message_update(update),
        "also do this\n\nUpdated system prompt section \"a\":\n\n<a>2</a>\n\nRemoved system prompt section \"b\".\n\nUpdated system prompt section \"c\":\n\n<c>1</c>"
    );
}

#[test]
fn tool_state_changes_and_history_detection_match_upstream() {
    let a = tool("a", "a tool");
    let b = tool("b", "b tool");
    let changed_b = tool("b", "changed");
    let c = tool("c", "c tool");
    assert!(declarations_equal(&a, &a));
    assert!(!declarations_equal(&b, &changed_b));
    let changes = get_tool_state_changes(&[a.clone(), b], &[changed_b.clone(), c]);
    assert_eq!(
        changes
            .tools_added
            .iter()
            .map(|tool| tool.name.as_str())
            .collect::<Vec<_>>(),
        ["b", "c"]
    );
    assert_eq!(
        changes
            .tools_removed
            .iter()
            .map(|tool| tool.name.as_str())
            .collect::<Vec<_>>(),
        ["a", "b"]
    );

    let additive = vec![
        system_message("", None, vec![a.clone()], vec![]),
        system_message("", None, vec![tool("late", "late")], vec![]),
    ];
    assert!(!has_non_additive_tool_changes(&additive));
    assert!(!has_tool_redefinitions(&additive));
    let split = resolve_transcript_tools(&additive, true);
    assert!(split.anchors_additions);
    assert_eq!(
        split
            .request_tools
            .iter()
            .map(|tool| tool.name.as_str())
            .collect::<Vec<_>>(),
        ["a"]
    );

    let redefined = vec![
        system_message("", None, vec![a], vec![]),
        system_message(
            "",
            None,
            vec![changed_b],
            vec![ToolReference { name: "b".into() }],
        ),
    ];
    assert!(has_non_additive_tool_changes(&redefined));
}
