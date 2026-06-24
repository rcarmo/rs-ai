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

pub mod types;
pub mod events;
pub mod registry;
pub mod env;
pub mod compat;
pub mod provider;
pub mod transports;
pub mod images;
pub mod models_generated;
pub mod transform;
pub mod simple_options;
pub mod retry;
pub mod logger;
pub mod jsonparse;
pub mod harness;
pub mod utils;
pub mod context;
pub mod diagnostics;
pub mod session_resources;
pub mod prompt_cache;
pub mod validation;
pub mod oauth;
pub mod auth;
pub mod auth_providers;
pub mod compaction;

// Re-exports for convenience
pub use types::*;
pub use types::provider_id;
pub use events::*;
pub use registry::{stream, complete};

#[cfg(test)]
mod registry_test;
#[cfg(test)]
mod env_test;
#[cfg(test)]
mod compat_test;
#[cfg(test)]
mod simple_options_test;
#[cfg(test)]
mod harness_test;
#[cfg(test)]
mod integration_test;
#[cfg(test)]
mod provider_test;
#[cfg(test)]
mod coverage_test;
#[cfg(test)]
mod provider_retry_test;
#[cfg(test)]
mod extra_coverage_test;
#[cfg(test)]
mod final_coverage_test;
#[cfg(test)]
mod edge_case_test;
#[cfg(test)]
mod registration_test;
#[cfg(test)]
mod mistral_reasoning_mode_test;
#[cfg(test)]
mod azure_openai_base_url_test;
#[cfg(test)]
mod bedrock_endpoint_test;
#[cfg(test)]
mod bedrock_coalesce_test;
#[cfg(test)]
mod openai_encrypted_reasoning_test;
#[cfg(test)]
mod openai_completions_tool_choice_test;
#[cfg(test)]
mod oauth_auth_test;
#[cfg(test)]
mod anthropic_oauth_test;
#[cfg(test)]
mod oauth_device_code_test;
#[cfg(test)]
mod openai_codex_oauth_test;
#[cfg(test)]
mod openai_completions_empty_tools_test;
#[cfg(test)]
mod openai_completions_thinking_as_text_test;
#[cfg(test)]
mod openai_completions_response_model_test;
#[cfg(test)]
mod openai_completions_cache_control_format_test;
#[cfg(test)]
mod openai_completions_reasoning_details_test;
#[cfg(test)]
mod openai_completions_tool_result_images_test;
#[cfg(test)]
mod openai_responses_terminal_event_test;
#[cfg(test)]
mod openai_responses_partial_json_cleanup_test;
#[cfg(test)]
mod openai_responses_tool_result_images_test;
#[cfg(test)]
mod codex_request_shape_test;
#[cfg(test)]
mod codex_ws_connection_limit_test;
#[cfg(test)]
mod codex_ws_protocol_test;
#[cfg(test)]
mod validation_upstream_test;
#[cfg(test)]
mod responses_foreign_toolcall_id_test;
#[cfg(test)]
mod responses_message_id_test;
