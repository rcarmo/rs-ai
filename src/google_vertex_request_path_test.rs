//! Ports upstream `google-vertex-api-key-resolution.test.ts` to the REST request
//! path used by this port (mirrors go-ai `buildStreamURL` / `resolveVertexProjectLocation`).
//! Upstream asserts on the `@google/genai` SDK constructor; this port builds the
//! project/location-scoped streaming URL directly, so we assert on URL shape and
//! project/location resolution instead.

use crate::provider::google::{build_stream_url, resolve_vertex_project_location};
use crate::types::{api, Model, ModelCost, StreamOptions};

fn make_model(id: &str, api: &str, provider: &str, base_url: &str) -> Model {
    Model {
        id: id.into(),
        name: "Test".into(),
        api: api.into(),
        provider: provider.into(),
        base_url: base_url.into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec!["text".into()],
        cost: ModelCost::default(),
        context_window: 128000,
        max_tokens: 4096,
        headers: None,
        api_key: None,
        compat: Default::default(),
    }
}

fn vertex_model(base_url: &str) -> Model {
    make_model("gemini-2.5-pro", api::GOOGLE_VERTEX, "google-vertex", base_url)
}

#[test]
fn vertex_url_uses_project_and_location_options() {
    let model = vertex_model("https://{location}-aiplatform.googleapis.com");
    let opts = StreamOptions {
        project: Some("proj-1".to_string()),
        location: Some("europe-west4".to_string()),
        ..Default::default()
    };
    let got = build_stream_url(&model, "vertex-key", &opts).expect("url");
    let want = "https://europe-west4-aiplatform.googleapis.com/v1/projects/proj-1/locations/europe-west4/publishers/google/models/gemini-2.5-pro:streamGenerateContent?alt=sse&key=vertex-key";
    assert_eq!(got, want);
}

#[test]
fn vertex_url_defaults_base_url_when_empty() {
    let model = vertex_model("");
    let opts = StreamOptions {
        project: Some("p".to_string()),
        location: Some("us-central1".to_string()),
        ..Default::default()
    };
    let got = build_stream_url(&model, "", &opts).expect("url");
    assert!(got.starts_with("https://us-central1-aiplatform.googleapis.com/v1/projects/p/locations/us-central1/"));
    // No API key supplied -> no key query parameter (ADC path).
    assert!(!got.contains("&key="));
}

#[test]
fn vertex_url_omits_placeholder_api_key() {
    let model = vertex_model("https://{location}-aiplatform.googleapis.com");
    let opts = StreamOptions {
        project: Some("p".to_string()),
        location: Some("us-central1".to_string()),
        ..Default::default()
    };
    // Placeholder markers (e.g. "<authenticated>") must not be appended as a key.
    let got = build_stream_url(&model, "<authenticated>", &opts).expect("url");
    assert!(!got.contains("key="));
}

#[test]
fn vertex_url_appends_real_api_key() {
    let model = vertex_model("https://{location}-aiplatform.googleapis.com");
    let opts = StreamOptions {
        project: Some("p".to_string()),
        location: Some("us-central1".to_string()),
        ..Default::default()
    };
    let got = build_stream_url(&model, "AIzaSyRealKey123", &opts).expect("url");
    assert!(got.ends_with("&key=AIzaSyRealKey123"));
}

#[test]
fn resolve_vertex_requires_project() {
    // Ensure env does not leak into the assertion.
    let opts = StreamOptions {
        location: Some("us-central1".to_string()),
        ..Default::default()
    };
    // Without project (and assuming no env), resolution must error.
    if std::env::var("GOOGLE_CLOUD_PROJECT").is_err() && std::env::var("GCLOUD_PROJECT").is_err() {
        let err = resolve_vertex_project_location(&opts).unwrap_err();
        assert!(err.contains("project ID"));
    }
}

#[test]
fn resolve_vertex_requires_location() {
    let opts = StreamOptions {
        project: Some("p".to_string()),
        ..Default::default()
    };
    if std::env::var("GOOGLE_CLOUD_LOCATION").is_err() {
        let err = resolve_vertex_project_location(&opts).unwrap_err();
        assert!(err.contains("location"));
    }
}

#[test]
fn non_vertex_gemini_url_unchanged() {
    let model = make_model(
        "gemini-2.0-flash",
        api::GOOGLE_GENERATIVE_AI,
        "google",
        "https://generativelanguage.googleapis.com/v1beta",
    );
    let got = build_stream_url(&model, "k", &StreamOptions::default()).expect("url");
    assert_eq!(
        got,
        "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.0-flash:streamGenerateContent?alt=sse&key=k"
    );
}
