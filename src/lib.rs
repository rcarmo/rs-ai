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
#[path = "tests/providers/anthropic/anthropic_cache_write_1h_cost_test.rs"]
mod anthropic_cache_write_1h_cost_test;
#[cfg(test)]
#[path = "tests/providers/anthropic/anthropic_compat_test.rs"]
mod anthropic_compat_test;
#[cfg(test)]
#[path = "tests/providers/anthropic/anthropic_fallback_test.rs"]
mod anthropic_fallback_test;
#[cfg(test)]
#[path = "tests/providers/anthropic/anthropic_force_adaptive_thinking_test.rs"]
mod anthropic_force_adaptive_thinking_test;
#[cfg(test)]
#[path = "tests/auth/oauth/anthropic_oauth_test.rs"]
mod anthropic_oauth_test;
#[cfg(test)]
#[path = "tests/providers/anthropic/anthropic_sse_parsing_test.rs"]
mod anthropic_sse_parsing_test;
#[cfg(test)]
#[path = "tests/providers/anthropic/anthropic_temperature_compat_test.rs"]
mod anthropic_temperature_compat_test;
#[cfg(test)]
#[path = "tests/providers/anthropic/anthropic_thinking_disable_test.rs"]
mod anthropic_thinking_disable_test;
#[cfg(test)]
#[path = "tests/providers/anthropic/anthropic_tool_name_normalization_test.rs"]
mod anthropic_tool_name_normalization_test;
#[cfg(test)]
#[path = "tests/providers/openai/azure_openai_base_url_test.rs"]
mod azure_openai_base_url_test;
#[cfg(test)]
#[path = "tests/providers/openai/azure_openai_responses_reasoning_replay_test.rs"]
mod azure_openai_responses_reasoning_replay_test;
#[cfg(test)]
#[path = "tests/providers/bedrock/bedrock_coalesce_test.rs"]
mod bedrock_coalesce_test;
#[cfg(test)]
#[path = "tests/providers/bedrock/bedrock_convert_messages_test.rs"]
mod bedrock_convert_messages_test;
#[cfg(test)]
#[path = "tests/providers/bedrock/bedrock_custom_headers_test.rs"]
mod bedrock_custom_headers_test;
#[cfg(test)]
#[path = "tests/providers/bedrock/bedrock_endpoint_test.rs"]
mod bedrock_endpoint_test;
#[cfg(test)]
#[path = "tests/providers/bedrock/bedrock_error_metadata_test.rs"]
mod bedrock_error_metadata_test;
#[cfg(test)]
#[path = "tests/catalogs/bedrock_images_models_test.rs"]
mod bedrock_images_models_test;
#[cfg(test)]
#[path = "tests/providers/bedrock/bedrock_thinking_payload_test.rs"]
mod bedrock_thinking_payload_test;
#[cfg(test)]
#[path = "tests/transports/cloudflare_stream_test.rs"]
mod cloudflare_stream_test;
#[cfg(test)]
#[path = "tests/providers/codex/codex_request_shape_test.rs"]
mod codex_request_shape_test;
#[cfg(test)]
#[path = "tests/providers/codex/codex_ws_account_cache_test.rs"]
mod codex_ws_account_cache_test;
#[cfg(test)]
#[path = "tests/providers/codex/codex_ws_connection_limit_test.rs"]
mod codex_ws_connection_limit_test;
#[cfg(test)]
#[path = "tests/providers/codex/codex_ws_protocol_test.rs"]
mod codex_ws_protocol_test;
#[cfg(test)]
#[path = "tests/core/compat_env_test.rs"]
mod compat_env_test;
#[cfg(test)]
#[path = "tests/core/compat_test.rs"]
mod compat_test;
#[cfg(test)]
#[path = "tests/core/coverage_test.rs"]
mod coverage_test;
#[cfg(test)]
#[path = "tests/core/deferred_tools_test.rs"]
mod deferred_tools_test;
#[cfg(test)]
#[path = "tests/providers/other/edge_case_test.rs"]
mod edge_case_test;
#[cfg(test)]
#[path = "tests/core/env_api_keys_test.rs"]
mod env_api_keys_test;
#[cfg(test)]
#[path = "tests/core/env_test.rs"]
mod env_test;
#[cfg(test)]
#[path = "tests/core/error_body_test.rs"]
mod error_body_test;
#[cfg(test)]
#[path = "tests/core/estimate_test.rs"]
mod estimate_test;
#[cfg(test)]
#[path = "tests/providers/other/extra_coverage_test.rs"]
mod extra_coverage_test;
#[cfg(test)]
#[path = "tests/providers/other/final_coverage_test.rs"]
mod final_coverage_test;
#[cfg(test)]
#[path = "tests/catalogs/fireworks_models_test.rs"]
mod fireworks_models_test;
#[cfg(test)]
#[path = "tests/providers/anthropic/github_copilot_anthropic_test.rs"]
mod github_copilot_anthropic_test;
#[cfg(test)]
#[path = "tests/auth/oauth/github_copilot_oauth_test.rs"]
mod github_copilot_oauth_test;
#[cfg(test)]
#[path = "tests/providers/google/google_gemini3_unsigned_tool_call_test.rs"]
mod google_gemini3_unsigned_tool_call_test;
#[cfg(test)]
#[path = "tests/providers/google/google_image_tool_result_routing_test.rs"]
mod google_image_tool_result_routing_test;
#[cfg(test)]
#[path = "tests/providers/google/google_shared_convert_tools_test.rs"]
mod google_shared_convert_tools_test;
#[cfg(test)]
#[path = "tests/providers/google/google_shared_retry_test.rs"]
mod google_shared_retry_test;
#[cfg(test)]
#[path = "tests/providers/google/google_signed_empty_blocks_test.rs"]
mod google_signed_empty_blocks_test;
#[cfg(test)]
#[path = "tests/providers/google/google_thinking_signature_test.rs"]
mod google_thinking_signature_test;
#[cfg(test)]
#[path = "tests/providers/google/google_vertex_request_path_test.rs"]
mod google_vertex_request_path_test;
#[cfg(test)]
#[path = "tests/core/harness_test.rs"]
mod harness_test;
#[cfg(test)]
#[path = "tests/transports/http_proxy_test.rs"]
mod http_proxy_test;
#[cfg(test)]
#[path = "tests/catalogs/image_model_data_test.rs"]
mod image_model_data_test;
#[cfg(test)]
#[path = "tests/core/integration_test.rs"]
mod integration_test;
#[cfg(test)]
#[path = "tests/core/lax_message_content_test.rs"]
mod lax_message_content_test;
#[cfg(test)]
#[path = "tests/catalogs/max_thinking_test.rs"]
mod max_thinking_test;
#[cfg(test)]
#[path = "tests/providers/mistral/mistral_reasoning_mode_test.rs"]
mod mistral_reasoning_mode_test;
#[cfg(test)]
#[path = "tests/catalogs/model_data_validation_test.rs"]
mod model_data_validation_test;
#[cfg(test)]
#[path = "tests/auth/oauth/models_runtime_auth_test.rs"]
mod models_runtime_auth_test;
#[cfg(test)]
#[path = "tests/catalogs/models_runtime_refresh_test.rs"]
mod models_runtime_refresh_test;
#[cfg(test)]
#[path = "tests/auth/oauth/oauth_auth_test.rs"]
mod oauth_auth_test;
#[cfg(test)]
#[path = "tests/auth/oauth/oauth_device_code_test.rs"]
mod oauth_device_code_test;
#[cfg(test)]
#[path = "tests/auth/oauth/openai_codex_oauth_test.rs"]
mod openai_codex_oauth_test;
#[cfg(test)]
#[path = "tests/providers/codex/openai_codex_stream_test.rs"]
mod openai_codex_stream_test;
#[cfg(test)]
#[path = "tests/providers/openai/openai_completions_cache_control_format_test.rs"]
mod openai_completions_cache_control_format_test;
#[cfg(test)]
#[path = "tests/providers/openai/openai_completions_empty_tools_test.rs"]
mod openai_completions_empty_tools_test;
#[cfg(test)]
#[path = "tests/providers/openai/openai_completions_prompt_cache_test.rs"]
mod openai_completions_prompt_cache_test;
#[cfg(test)]
#[path = "tests/providers/openai/openai_completions_reasoning_details_test.rs"]
mod openai_completions_reasoning_details_test;
#[cfg(test)]
#[path = "tests/catalogs/openai_completions_response_model_test.rs"]
mod openai_completions_response_model_test;
#[cfg(test)]
#[path = "tests/providers/openai/openai_completions_thinking_as_text_test.rs"]
mod openai_completions_thinking_as_text_test;
#[cfg(test)]
#[path = "tests/providers/openai/openai_completions_tool_choice_test.rs"]
mod openai_completions_tool_choice_test;
#[cfg(test)]
#[path = "tests/providers/openai/openai_completions_tool_result_images_test.rs"]
mod openai_completions_tool_result_images_test;
#[cfg(test)]
#[path = "tests/providers/openai/openai_encrypted_reasoning_test.rs"]
mod openai_encrypted_reasoning_test;
#[cfg(test)]
#[path = "tests/providers/openai/openai_responses_copilot_provider_test.rs"]
mod openai_responses_copilot_provider_test;
#[cfg(test)]
#[path = "tests/providers/openai/openai_responses_empty_tool_result_test.rs"]
mod openai_responses_empty_tool_result_test;
#[cfg(test)]
#[path = "tests/providers/openai/openai_responses_partial_json_cleanup_test.rs"]
mod openai_responses_partial_json_cleanup_test;
#[cfg(test)]
#[path = "tests/providers/openai/openai_responses_terminal_event_test.rs"]
mod openai_responses_terminal_event_test;
#[cfg(test)]
#[path = "tests/providers/openai/openai_responses_tool_result_images_test.rs"]
mod openai_responses_tool_result_images_test;
#[cfg(test)]
#[path = "tests/catalogs/openrouter_cache_control_models_test.rs"]
mod openrouter_cache_control_models_test;
#[cfg(test)]
#[path = "tests/providers/openrouter/openrouter_images_test.rs"]
mod openrouter_images_test;
#[cfg(test)]
#[path = "tests/core/overflow_test.rs"]
mod overflow_test;
#[cfg(test)]
#[path = "tests/core/pi_messages_test.rs"]
mod pi_messages_test;
#[cfg(test)]
#[path = "tests/transports/provider_retry_test.rs"]
mod provider_retry_test;
#[cfg(test)]
#[path = "tests/transports/provider_retry_upstream_test.rs"]
mod provider_retry_upstream_test;
#[cfg(test)]
#[path = "tests/providers/other/provider_test.rs"]
mod provider_test;
#[cfg(test)]
#[path = "tests/transports/providers_upstream_test.rs"]
mod providers_upstream_test;
#[cfg(test)]
#[path = "tests/auth/oauth/radius_oauth_test.rs"]
mod radius_oauth_test;
#[cfg(test)]
#[path = "tests/core/reasoning_options_test.rs"]
mod reasoning_options_test;
#[cfg(test)]
#[path = "tests/core/registration_test.rs"]
mod registration_test;
#[cfg(test)]
#[path = "tests/catalogs/registry_test.rs"]
mod registry_test;
#[cfg(test)]
#[path = "tests/release/release_metadata_verification_test.rs"]
mod release_metadata_verification_test;
#[cfg(test)]
#[path = "tests/providers/openai/responses_foreign_toolcall_id_test.rs"]
mod responses_foreign_toolcall_id_test;
#[cfg(test)]
#[path = "tests/providers/openai/responses_message_id_test.rs"]
mod responses_message_id_test;
#[cfg(test)]
#[path = "tests/transports/retry_classify_test.rs"]
mod retry_classify_test;
#[cfg(test)]
#[path = "tests/core/simple_options_test.rs"]
mod simple_options_test;
#[cfg(test)]
#[path = "tests/core/simulated_e2e_fixtures_test.rs"]
mod simulated_e2e_fixtures_test;
#[cfg(test)]
#[path = "tests/transports/stream_e2e_live_test.rs"]
mod stream_e2e_live_test;
#[cfg(test)]
#[path = "tests/catalogs/supports_xhigh_test.rs"]
mod supports_xhigh_test;
#[cfg(test)]
#[path = "tests/catalogs/together_xiaomi_models_test.rs"]
mod together_xiaomi_models_test;
#[cfg(test)]
#[path = "tests/core/tool_call_id_normalization_test.rs"]
mod tool_call_id_normalization_test;
#[cfg(test)]
#[path = "tests/core/uuid_test.rs"]
mod uuid_test;
#[cfg(test)]
#[path = "tests/release/v0830_release_test.rs"]
mod v0830_release_test;
#[cfg(test)]
#[path = "tests/release/v0840_release_test.rs"]
mod v0840_release_test;
#[cfg(test)]
#[path = "tests/release/v0841_release_test.rs"]
mod v0841_release_test;
#[cfg(test)]
#[path = "tests/release/v0842_release_test.rs"]
mod v0842_release_test;
#[cfg(test)]
#[path = "tests/release/v0843_release_test.rs"]
mod v0843_release_test;
#[cfg(test)]
#[path = "tests/release/v0844_release_test.rs"]
mod v0844_release_test;
#[cfg(test)]
#[path = "tests/transports/validation_upstream_test.rs"]
mod validation_upstream_test;
#[cfg(test)]
#[path = "tests/providers/xai/xai_grok45_responses_test.rs"]
mod xai_grok45_responses_test;
#[cfg(test)]
#[path = "tests/auth/oauth/xai_oauth_test.rs"]
mod xai_oauth_test;
