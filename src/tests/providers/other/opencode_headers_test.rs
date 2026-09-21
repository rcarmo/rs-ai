use std::collections::HashMap;

use reqwest::header::HeaderMap;

use crate::types::{Model, ModelCompat, ModelCost, StreamOptions};
use crate::utils::add_opencode_session_header;

fn model(provider: &str) -> Model {
    Model {
        id: "test".into(),
        name: "Test".into(),
        api: "openai-responses".into(),
        provider: provider.into(),
        base_url: "http://127.0.0.1:9".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec!["text".into()],
        input_limits: None,
        prompt_cache: None,
        enabled: None,
        lab: None,
        providers: None,
        cost: ModelCost::default(),
        context_window: 10_000,
        max_tokens: 1_000,
        sampling_params: None,
        headers: None,
        api_key: None,
        compat: ModelCompat::default(),
    }
}

fn apply(model: &Model, options: StreamOptions) -> HeaderMap {
    let mut headers = HeaderMap::new();
    if let Some(extra) = &options.headers {
        for (name, value) in extra {
            headers.insert(
                reqwest::header::HeaderName::from_bytes(name.as_bytes()).unwrap(),
                value.parse().unwrap(),
            );
        }
    }
    add_opencode_session_header(&mut headers, model, &options);
    headers
}

#[test]
fn opencode_session_header_is_independent_of_cache_retention() {
    let headers = apply(
        &model("opencode"),
        StreamOptions {
            session_id: Some("session-123".into()),
            cache_retention: Some(crate::types::CacheRetention::None),
            ..Default::default()
        },
    );
    assert_eq!(headers["x-opencode-session"], "session-123");
}

#[test]
fn opencode_session_header_preserves_case_insensitive_override() {
    let headers = apply(
        &model("opencode"),
        StreamOptions {
            session_id: Some("session-123".into()),
            headers: Some(HashMap::from([(
                "X-OpenCode-Session".into(),
                "caller-value".into(),
            )])),
            ..Default::default()
        },
    );
    assert_eq!(headers["x-opencode-session"], "caller-value");
}

#[test]
fn opencode_session_header_is_absent_without_session_or_other_provider() {
    assert!(
        apply(&model("opencode"), StreamOptions::default())
            .get("x-opencode-session")
            .is_none()
    );
    assert!(
        apply(
            &model("openai"),
            StreamOptions {
                session_id: Some("session-123".into()),
                ..Default::default()
            },
        )
        .get("x-opencode-session")
        .is_none()
    );
}
