//! rs-ai — Unified LLM API with automatic model discovery, streaming,
//! tool calling, and multi-provider support.
//!
//! A Rust port of [@earendil-works/pi-ai](https://www.npmjs.com/package/@earendil-works/pi-ai).
//!
//! # Quick start
//!
//! ```no_run
//! use rs_ai::{registry, provider_id};
//!
//! #[tokio::main]
//! async fn main() {
//!     registry::register_builtin_models();
//!     let model = registry::get_model(provider_id::OPENAI, "gpt-4o-mini").unwrap();
//!     // ... stream or complete
//! }
//! ```

#![allow(clippy::items_after_test_module)]

pub mod auth;
pub mod auth_providers;
pub mod compaction;
pub mod compat;
pub mod context;
pub mod deferred_tools;
pub mod diagnostics;
pub mod env;
pub mod error_body;
pub mod estimate;
pub mod events;
pub mod harness;
pub mod http_proxy;
pub mod images;
pub mod jsonparse;
pub mod logger;
pub mod models_generated;
pub mod models_runtime;
pub mod oauth;
pub mod prompt_cache;
pub mod provider;
pub mod registry;
pub mod retry;
pub mod session_resources;
pub mod simple_options;
pub mod transform;
pub mod transports;
pub mod types;
pub mod utils;
pub mod validation;

// Re-exports for convenience
pub use events::*;
pub use registry::{complete, stream};
pub use types::provider_id;
pub use types::*;

#[cfg(test)]
mod anthropic_cache_write_1h_cost_test;
#[cfg(test)]
mod anthropic_compat_test;
#[cfg(test)]
mod anthropic_force_adaptive_thinking_test;
#[cfg(test)]
mod anthropic_oauth_test;
#[cfg(test)]
mod anthropic_sse_parsing_test;
#[cfg(test)]
mod anthropic_temperature_compat_test;
#[cfg(test)]
mod anthropic_thinking_disable_test;
#[cfg(test)]
mod anthropic_tool_name_normalization_test;
#[cfg(test)]
mod azure_openai_base_url_test;
#[cfg(test)]
mod azure_openai_responses_reasoning_replay_test;
#[cfg(test)]
mod bedrock_coalesce_test;
#[cfg(test)]
mod bedrock_convert_messages_test;
#[cfg(test)]
mod bedrock_custom_headers_test;
#[cfg(test)]
mod bedrock_endpoint_test;
#[cfg(test)]
mod bedrock_images_models_test;
#[cfg(test)]
mod bedrock_thinking_payload_test;
#[cfg(test)]
mod codex_request_shape_test;
#[cfg(test)]
mod codex_ws_connection_limit_test;
#[cfg(test)]
mod codex_ws_protocol_test;
#[cfg(test)]
mod compat_env_test;
#[cfg(test)]
mod compat_test;
#[cfg(test)]
mod coverage_test;
#[cfg(test)]
mod deferred_tools_test;
#[cfg(test)]
mod edge_case_test;
#[cfg(test)]
mod env_api_keys_test;
#[cfg(test)]
mod env_test;
mod error_body_test;
#[cfg(test)]
mod estimate_test;
#[cfg(test)]
mod extra_coverage_test;
#[cfg(test)]
mod final_coverage_test;
#[cfg(test)]
mod fireworks_models_test;
#[cfg(test)]
mod github_copilot_anthropic_test;
#[cfg(test)]
mod github_copilot_oauth_test;
#[cfg(test)]
mod google_gemini3_unsigned_tool_call_test;
#[cfg(test)]
mod google_image_tool_result_routing_test;
#[cfg(test)]
mod google_shared_convert_tools_test;
#[cfg(test)]
mod google_thinking_signature_test;
#[cfg(test)]
mod google_vertex_request_path_test;
#[cfg(test)]
mod harness_test;
#[cfg(test)]
mod http_proxy_test;
#[cfg(test)]
mod integration_test;
mod lax_message_content_test;
mod max_thinking_test;
#[cfg(test)]
mod mistral_reasoning_mode_test;
#[cfg(test)]
mod models_runtime_auth_test;
#[cfg(test)]
mod models_runtime_refresh_test;
#[cfg(test)]
mod oauth_auth_test;
#[cfg(test)]
mod oauth_device_code_test;
#[cfg(test)]
mod openai_codex_oauth_test;
#[cfg(test)]
mod openai_codex_stream_test;
#[cfg(test)]
mod openai_completions_cache_control_format_test;
#[cfg(test)]
mod openai_completions_empty_tools_test;
#[cfg(test)]
mod openai_completions_prompt_cache_test;
#[cfg(test)]
mod openai_completions_reasoning_details_test;
#[cfg(test)]
mod openai_completions_response_model_test;
#[cfg(test)]
mod openai_completions_thinking_as_text_test;
#[cfg(test)]
mod openai_completions_tool_choice_test;
#[cfg(test)]
mod openai_completions_tool_result_images_test;
#[cfg(test)]
mod openai_encrypted_reasoning_test;
#[cfg(test)]
mod openai_responses_copilot_provider_test;
mod openai_responses_empty_tool_result_test;
#[cfg(test)]
mod openai_responses_partial_json_cleanup_test;
#[cfg(test)]
mod openai_responses_terminal_event_test;
#[cfg(test)]
mod openai_responses_tool_result_images_test;
#[cfg(test)]
mod openrouter_images_test;
#[cfg(test)]
mod overflow_test;
#[cfg(test)]
mod pi_messages_test;
#[cfg(test)]
mod provider_retry_test;
#[cfg(test)]
mod provider_test;
#[cfg(test)]
mod providers_upstream_test;
#[cfg(test)]
mod radius_oauth_test;
#[cfg(test)]
mod registration_test;
#[cfg(test)]
mod registry_test;
#[cfg(test)]
mod responses_foreign_toolcall_id_test;
#[cfg(test)]
mod responses_message_id_test;
mod retry_classify_test;
mod simple_options_test;
mod simulated_e2e_fixtures_test;
#[cfg(test)]
mod stream_e2e_live_test;
#[cfg(test)]
mod supports_xhigh_test;
#[cfg(test)]
mod together_xiaomi_models_test;
#[cfg(test)]
mod tool_call_id_normalization_test;
#[cfg(test)]
mod v0820_release_test;
#[cfg(test)]
mod validation_upstream_test;
#[cfg(test)]
mod xai_grok45_responses_test;
#[cfg(test)]
mod xai_oauth_test;
