# v1.0.0 bounded 171-file test crosswalk

Source: signed npm `@earendil-works/pi-ai@1.0.0` and official tag `a13d35a742c6ef8462812a28fbe1d8c8b7431c32`. Baseline: accepted v0.99.2 tag `005af57d88ee23b33778f343a9595b32e67ff788`.

The exact release range contains **8 changed paths** with **192 insertions and 14 deletions**. It changes **3 test paths**; the final package test/support corpus contains **171 unique basenames**. The signed npm tarball SHA-256 is `f39b99c29b8598f175b10840e5d2a81983e7c0ce5cae4d7df83a1007447d2c2b`. `scripts/validate_v100_manifests.py` checks the inventories, hashes, matrices and shortstat.

Responses replay resolves grammar capability and the transcript tool name once for declarations, assistant calls and tool results. Foreign function history retains the accepted `fc_<shortHash>` normalisation. Custom grammar calls require `ctc_` item IDs after normalisation; mismatched or same-provider/API different-model item IDs are omitted while `call_id` remains. Missing or null grammar arguments replay as an empty input string, matching `String(value ?? "")` semantics.

Anthropic OAuth extends the registered Rust adapter with host-driven browser/copy-code selection and parsing. Portable URL/code/state/PKCE/token/cancellation behaviour is adapted; browser launching and interactive UI remain host-owned. The shared callback server completes token exchange before success, uses redacted public failures and renders the Pi SVG. Signed schema-v6 catalogs are regenerated only from the exact npm tarball shards.

## Changed-path disposition matrix

| Upstream changed path | Disposition | rs-ai evidence | Notes |
|---|---|---|---|
| `packages/ai/CHANGELOG.md` | N/A | `docs/v100-171-test-crosswalk.md`; signed npm provenance | Upstream release prose records scope but does not change Rust runtime behaviour. |
| `packages/ai/package.json` | ADAPTED | `Cargo.toml`; root `Cargo.lock`; `src/tests/release/v100_release_test.rs` | Native version is 1.0.0. Node export and telemetry dependency mechanics are N/A; exact provider shards drive Rust catalog regeneration. |
| `packages/ai/src/api/openai-responses-shared.ts` | ADAPTED | `src/provider/responses.rs`; `src/tests/providers/openai/openai_responses_v100_grammar_replay_test.rs`; historical foreign-function regression | One capability/transcript-resolved grammar map drives declarations, call replay and result replay with exact ID-prefix and empty-input semantics. |
| `packages/ai/src/auth/oauth/anthropic.ts` | ADAPTED | `src/oauth.rs`; `src/auth_providers.rs`; `src/oauth_callback.rs`; `src/tests/auth/oauth/anthropic_oauth_test.rs` | The existing registered provider gains host-driven browser/copy login, exact parser/state/JSON request/cancellation behaviour without shared trait churn. |
| `packages/ai/src/utils/oauth-page.ts` | ADAPTED | `src/oauth_callback.rs`; `src/tests/auth/oauth/oauth_callback_server_test.rs` | Rust serves the shared callback page; it renders the three-colour Pi SVG, escapes text and redacts public exchange errors. |
| `packages/ai/test/anthropic-oauth.test.ts` | ADAPTED | `src/tests/auth/oauth/anthropic_oauth_test.rs`; production auth paths above | Browser-first/copy selection, cancellation, parser, request and callback settlement are deterministic. |
| `packages/ai/test/constrained-sampling.test.ts` | ADAPTED | `src/tests/providers/openai/openai_responses_v100_grammar_replay_test.rs`; retained strict/stream regressions | Covers custom/function replay, prefix controls, foreign-function preservation, missing/null empty input and custom result pairing. |
| `packages/ai/test/oauth-callback-server.test.ts` | ADAPTED | `src/tests/auth/oauth/oauth_callback_server_test.rs`; `src/oauth_callback.rs` | Covers completion-before-success, redacted failures, SVG colours, escaping and duplicate settlement. |

## Per-file 171-test/support disposition matrix

| Upstream test/support file | Disposition | Source | rs-ai evidence | Notes |
|---|---|---|---|---|
| `abort.test.ts` | ADAPTED | accepted v0.99.1 baseline | `src/tests/providers/other/pre_generation_error_test.rs`; `src/tests/transports/provider_retry_test.rs` | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `anthropic-adaptive-thinking-models.test.ts` | ADAPTED | accepted v0.99.1 baseline | `src/tests/providers/anthropic/anthropic_force_adaptive_thinking_test.rs`; `src/tests/catalogs/supports_xhigh_test.rs` | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `anthropic-auth-token.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `anthropic-cache-write-1h-cost.test.ts` | ADAPTED | accepted v0.99.1 baseline | `src/tests/providers/anthropic/anthropic_cache_write_1h_cost_test.rs` | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `anthropic-eager-tool-input-compat.test.ts` | ADAPTED | v0.99.2 tag `005af57d88ee23b33778f343a9595b32e67ff788` | `src/tests/providers/anthropic/anthropic_compat_test.rs::{sends_per_tool_eager_input_streaming_by_default,uses_legacy_fine_grained_beta_when_eager_disabled,no_legacy_beta_when_there_are_no_tools}`; `src/provider/anthropic.rs` | v0.99.2 retains the three eager predicates while moving strict-schema assertions to a separate test file. |
| `anthropic-eager-tool-input-e2e.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `anthropic-empty-thinking-signature-compat.test.ts` | ADAPTED | accepted v0.99.1 baseline | `src/tests/providers/anthropic/anthropic_compat_test.rs`; `src/tests/catalogs/fireworks_models_test.rs` | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `anthropic-federation-sdk.test.ts` | ADAPTED | v0.99.2 tag `005af57d88ee23b33778f343a9595b32e67ff788` | `anthropic_federation_test.rs::{concurrent_callers_coalesce_one_token_exchange,exchanges_file_identity_once_and_reuses_bearer_token,expiring_token_refreshes_and_cache_reset_isolates_keys}`; `src/provider/anthropic.rs` | Deterministic cache semantics are adapted; JS SDK constructor/default-chain mechanics are N/A; live workload credentials/network are live-only. |
| `anthropic-federation.test.ts` | ADAPTED | v0.99.2 tag `005af57d88ee23b33778f343a9595b32e67ff788` | `src/tests/providers/anthropic/anthropic_federation_test.rs` (9 passing tests); `src/env.rs`; `src/provider/anthropic.rs` | Deterministic env/file/exchange/auth/cache/cancellation/error-redaction production behavior is covered. |
| `anthropic-force-adaptive-thinking.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `anthropic-long-cache-retention-e2e.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `anthropic-mid-conversation-effort.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `anthropic-oauth.test.ts` | ADAPTED | v1.0.0 tag `a13d35a742c6ef8462812a28fbe1d8c8b7431c32` | `src/tests/auth/oauth/anthropic_oauth_test.rs`; `src/oauth.rs`; `src/auth_providers.rs` | Browser/copy selection, parser/state, exact JSON token exchange, timeout/cancellation and refresh shape use existing registered Anthropic auth surfaces. Browser launching and UI are host-owned. |
| `anthropic-opus-4-8-smoke.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `anthropic-sse-parsing.test.ts` | ADAPTED | accepted v0.99.1 baseline | `src/tests/providers/anthropic/anthropic_sse_parsing_test.rs`; `src/provider/anthropic.rs` | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `anthropic-strict-tool-schema.test.ts` | ADAPTED | v0.99.2 tag `005af57d88ee23b33778f343a9595b32e67ff788` | `src/tests/providers/anthropic/anthropic_strict_tool_schema_test.rs`; `src/provider/anthropic.rs`; `src/utils.rs` | Supported constraints remain strict; numeric bounds, minItems 2 and regex format fall back; strict require fails closed. |
| `anthropic-temperature-compat.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `anthropic-thinking-binding-e2e.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `anthropic-thinking-disable.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `anthropic-tool-name-normalization.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `assistant-message-frame.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `azure-openai-base-url.test.ts` | ADAPTED | accepted v0.99.1 baseline | `src/tests/providers/openai/azure_openai_base_url_test.rs`; `src/provider/responses.rs` | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `azure-openai-responses-reasoning-replay.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `azure-openai-tool-choice.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `azure-utils.ts` | N/A | v0.99.2 final package corpus | Upstream support/fixture inventory | Upstream test helper, not an executable test file; Azure behavior retains accepted Rust coverage. |
| `baseten-models.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `bedrock-cache-write-1h-cost.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `bedrock-convert-messages.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `bedrock-credentials.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `bedrock-custom-headers.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `bedrock-endpoint-resolution.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `bedrock-error-metadata.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `bedrock-models.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `bedrock-raw-stop-reason.test.ts` | ADAPTED | accepted v0.99.1 baseline | `src/tests/release/v0830_release_test.rs::bedrock_raw_stop_reason_helper_errors_unknown_and_preserves_raw`; `src/provider/bedrock.rs::tests::provider_event_projection_matches_bedrock_sdk_object_shape`; `provider_error_projection_matches_bedrock_sdk_item_shape` | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `bedrock-redacted-reasoning.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `bedrock-response-headers.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `bedrock-thinking-payload.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `bedrock-utils.ts` | N/A | v0.99.2 final package corpus | Upstream support/fixture inventory | Upstream test helper, not an executable test file; Bedrock behavior retains accepted Rust coverage. |
| `cache-retention.test.ts` | ADAPTED | accepted v0.99.1 baseline | `src/tests/providers/openai/openai_responses_prompt_cache_test.rs`; `src/tests/providers/anthropic/anthropic_compat_test.rs`; `src/prompt_cache.rs` | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `classifier-models.test.ts` | ADAPTED | accepted v0.99.1 baseline | `src/tests/catalogs/model_types_v0991_test.rs`; `src/tests/providers/other/classifier_system_one_test.rs` | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `cloudflare-ai-binding.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `cloudflare-stream.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `cloudflare-utils.ts` | N/A | v0.99.2 final package corpus | Upstream support/fixture inventory | Upstream test helper, not an executable test file; Cloudflare behavior retains accepted Rust coverage. |
| `cloudflare-workers-ai-system-one.test.ts` | ADAPTED | accepted v0.99.1 baseline | `src/tests/providers/other/classifier_system_one_test.rs::cloudflare_uses_account_scoped_run_envelope_and_reports_errors`; `src/classifiers/system_one.rs` | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `codex-websocket-cached-probe.ts` | N/A | v0.99.2 final package corpus | Upstream support/fixture inventory | Upstream test probe/helper, not an executable test file; Codex WebSocket behavior retains accepted Rust coverage. |
| `compat-env.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `constrained-sampling.test.ts` | ADAPTED | v1.0.0 tag `a13d35a742c6ef8462812a28fbe1d8c8b7431c32` | `src/provider/responses.rs`; `src/tests/providers/openai/openai_responses_v100_grammar_replay_test.rs`; retained v0.83 grammar regressions | Capability-resolved custom/function call and result replay preserves call IDs, filters type-specific item IDs and maps missing/null grammar input to an empty string. |
| `context-estimate.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `context-overflow.test.ts` | ADAPTED | accepted v0.99.1 baseline | `src/tests/core/overflow_test.rs`; `src/tests/transports/providers_upstream_test.rs::uses_official_kimi_k3_pricing_for_moonshot_providers` | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `cross-provider-handoff.test.ts` | ADAPTED | accepted v0.99.1 baseline | `src/tests/core/transcript_test.rs`; `src/tests/providers/other/transcript_provider_test.rs`; `src/transform.rs` | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `red-circle.png` | N/A | v0.99.2 final package corpus | Upstream support/fixture inventory | Binary test fixture, not executable behavior. |
| `empty.test.ts` | ADAPTED | accepted v0.99.1 baseline | `src/tests/providers/other/edge_case_test.rs`; `src/tests/core/lax_message_content_test.rs` | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `env-api-keys.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `error-body.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `event-stream.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `faux-provider.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `fetch-option.test.ts` | N/A | accepted v0.99.1 baseline | `reqwest` transport injection is replaced by local `wiremock` endpoints throughout `src/tests/providers/` | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `fireworks-model-generation.test.ts` | ADAPTED | accepted v0.99.1 baseline | `scripts/verify_release_model_metadata.py`; `scripts/verify_v0991_baseline_delta.py`; `src/tests/release/release_metadata_verification_test.rs` | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `fireworks-models.test.ts` | ADAPTED | accepted v0.99.1 baseline | `src/tests/catalogs/fireworks_models_test.rs` | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `generate-models-strict.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `github-copilot-anthropic.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `github-copilot-oauth.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `google-raw-stop-reason.test.ts` | ADAPTED | accepted v0.99.1 baseline | `src/tests/release/v0830_release_test.rs::google_and_mistral_raw_stop_reasons_are_executable`; `src/provider/google.rs` | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `google-shared-convert-tools.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `google-shared-gemini3-unsigned-tool-call.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `google-shared-image-tool-result-routing.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `google-shared-retry.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `google-shared-signed-empty-blocks.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `google-thinking-disable.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `google-thinking-level-map.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `google-thinking-signature.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `google-vertex-api-key-resolution.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `image-model-data.test.ts` | ADAPTED | accepted v0.99.1 baseline | `src/tests/catalogs/image_model_data_test.rs`; `scripts/verify_v0991_baseline_delta.py` | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `image-tool-result.test.ts` | ADAPTED | accepted v0.99.1 baseline | `src/tests/providers/openai/openai_completions_tool_result_images_test.rs`; `src/tests/providers/openai/openai_responses_tool_result_images_test.rs`; `src/tests/providers/google/google_image_tool_result_routing_test.rs` | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `images-models.test.ts` | ADAPTED | accepted v0.99.1 baseline | `src/tests/catalogs/model_types_v0991_test.rs`; `src/tests/providers/openrouter/openrouter_images_test.rs`; `src/model_catalog.rs` | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `images.test.ts` | ADAPTED | accepted v0.99.1 baseline | `src/tests/providers/openrouter/openrouter_images_test.rs`; `src/images/mod.rs` | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `interleaved-thinking.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `kimi-coding-oauth.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `lax-message-content.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `lazy-module-load.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `llama-cpp-classify.test.ts` | ADAPTED | accepted v0.99.1 baseline | `src/tests/providers/other/llama_cpp_classify_test.rs`; `src/classifiers/llama_cpp.rs` | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `max-thinking.test.ts` | ADAPTED | accepted v0.99.1 baseline | `src/tests/catalogs/max_thinking_test.rs`; `src/simple_options.rs` | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `message-types.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `meta-oauth.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `mistral-http-transport.test.ts` | ADAPTED | accepted v0.99.1 baseline | `src/tests/providers/mistral/mistral_reasoning_mode_test.rs`; `src/provider/mistral.rs` | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `mistral-raw-stop-reason.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `mistral-reasoning-mode.test.ts` | ADAPTED | accepted v0.99.1 baseline | `src/tests/providers/mistral/mistral_reasoning_mode_test.rs` | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `mistral-tool-schema.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `model-catalog-types.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `model-data-validation.test.ts` | ADAPTED | accepted v0.99.1 baseline | `src/tests/catalogs/model_data_validation_test.rs`; `scripts/validate_release_model_data.py` | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `model-types.test.ts` | ADAPTED | accepted v0.99.1 baseline | `src/tests/catalogs/model_types_v0991_test.rs`; `src/model_catalog.rs` | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `models-entry.test.ts` | N/A | v0.99.2 tag `005af57d88ee23b33778f343a9595b32e67ff788` | `src/model_catalog.rs`; `src/registry.rs`; faux provider tests | Node subpath/module-loader behavior has no Rust equivalent; Rust registry/faux access is analogous public-surface documentation only. |
| `models-runtime.test.ts` | ADAPTED | accepted v0.99.1 baseline | `src/tests/catalogs/models_runtime_refresh_test.rs`; `src/tests/auth/oauth/models_runtime_auth_test.rs` | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `node-http-proxy.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `oauth-auth.test.ts` | ADAPTED | accepted v0.99.1 baseline | `src/tests/auth/oauth/oauth_auth_test.rs`; `src/tests/auth/oauth/openai_chatgpt_oauth_test.rs` | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `oauth-callback-server.test.ts` | ADAPTED | v1.0.0 tag `a13d35a742c6ef8462812a28fbe1d8c8b7431c32` | `src/tests/auth/oauth/oauth_callback_server_test.rs`; `src/oauth_callback.rs` | Shared pages use the Pi SVG, escape provider text, keep completion-before-success ordering, redact public completion failures and retain at-most-once settlement. |
| `oauth-device-code.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `oauth.ts` | N/A | v0.99.2 final package corpus | Upstream support/fixture inventory | Upstream test helper, not an executable test file; OAuth behavior retains accepted Rust coverage. |
| `openai-chatgpt-oauth.test.ts` | ADAPTED | accepted v0.99.1 baseline | `src/tests/auth/oauth/openai_chatgpt_oauth_test.rs`; `src/openai_chatgpt_oauth.rs` | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `openai-codex-cache-affinity-e2e.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `openai-codex-oauth.test.ts` | ADAPTED | accepted v0.99.1 baseline | `src/tests/auth/oauth/openai_codex_oauth_test.rs`; `src/oauth_callback.rs` | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `openai-codex-stream.test.ts` | ADAPTED | accepted v0.99.1 baseline | `src/tests/providers/codex/openai_codex_stream_test.rs`; `src/tests/providers/codex/codex_ws_protocol_test.rs` | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `openai-completions-cache-control-format.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `openai-completions-empty-tools.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `openai-completions-prompt-cache.test.ts` | ADAPTED | accepted v0.99.1 baseline | `src/tests/providers/openai/openai_completions_prompt_cache_test.rs` | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `openai-completions-provider-stream-event.test.ts` | ADAPTED | accepted v0.99.1 baseline | `src/tests/providers/openai/openai_completions_provider_stream_event_test.rs` | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `openai-completions-raw-stop-reason.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `openai-completions-reasoning-details.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `openai-completions-response-model.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `openai-completions-retry.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `openai-completions-thinking-as-text.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `openai-completions-thinking-token-budget.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `openai-completions-tool-choice.test.ts` | ADAPTED | accepted v0.99.1 baseline | `src/tests/providers/openai/openai_completions_tool_choice_test.rs` | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `openai-completions-tool-result-images.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `openai-completions-vllm-priority.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `openai-responses-cache-affinity-e2e.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `openai-responses-chatgpt-sign-in.test.ts` | ADAPTED | accepted v0.99.1 baseline | `src/tests/providers/openai/openai_responses_chatgpt_sign_in_test.rs` | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `openai-responses-compat.test.ts` | ADAPTED | accepted v0.99.1 baseline | `src/tests/providers/openai/openai_responses_copilot_provider_test.rs`; `src/tests/providers/openai/openai_responses_prompt_cache_test.rs` | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `openai-responses-empty-tool-result.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `openai-responses-foreign-toolcall-id.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `openai-responses-message-id.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `openai-responses-namespace.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `openai-responses-partial-json-cleanup.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `openai-responses-reasoning-replay-e2e.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `openai-responses-terminal-event.test.ts` | ADAPTED | accepted v0.99.1 baseline | `src/tests/providers/openai/openai_responses_terminal_event_test.rs` | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `openai-responses-tool-result-images.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `openai-responses-usage-limit.test.ts` | ADAPTED | accepted v0.99.1 baseline | `src/tests/providers/openai/openai_responses_chatgpt_sign_in_test.rs::usage_limit_errors_link_to_chatgpt_usage_for_http_and_stream` | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `opencode-provider-headers.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `openrouter-cache-control-models.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `openrouter-cache-write-repro.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `openrouter-images.test.ts` | ADAPTED | accepted v0.99.1 baseline | `src/tests/providers/openrouter/openrouter_images_test.rs` | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `openrouter-oauth.test.ts` | ADAPTED | accepted v0.99.1 baseline | `src/tests/auth/oauth/oauth_auth_test.rs`; `src/images/openrouter.rs` | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `openrouter-reasoning-options.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `overflow.test.ts` | ADAPTED | v0.99.2 tag `005af57d88ee23b33778f343a9595b32e67ff788` | `src/tests/core/overflow_test.rs::tests::detects_zai_cn_prompt_exceeds_max_length`; `src/context.rs` | Exact Z.AI CN phrase is covered. Retry-After fallback remains separate production retry evidence. |
| `pi-messages.test.ts` | ADAPTED | accepted v0.99.1 baseline | `src/tests/core/pi_messages_test.rs`; `src/provider/pi_messages.rs` | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `pre-generation-error.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `provider-error-body-passthrough.test.ts` | ADAPTED | accepted v0.99.1 baseline | `src/tests/core/error_body_test.rs`; `src/tests/transports/provider_retry_upstream_test.rs` | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `provider-error-body-regression.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `provider-retry.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `providers.test.ts` | ADAPTED | accepted v0.99.1 baseline | `src/tests/providers/other/provider_test.rs`; `src/tests/core/registration_test.rs` | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `qwen-token-plan-models.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `radius-oauth.test.ts` | ADAPTED | accepted v0.99.1 baseline | `src/tests/auth/oauth/radius_oauth_test.rs`; `src/oauth_callback.rs` | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `radius-provider.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `reasoning-options.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `responseid.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `retry.test.ts` | ADAPTED | accepted v0.99.1 baseline | `src/tests/transports/retry_classify_test.rs`; `src/tests/providers/openai/openai_responses_chatgpt_sign_in_test.rs` | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `sampling-options.test.ts` | ADAPTED | accepted v0.99.1 baseline | `src/tests/release/v0840_release_test.rs::sampling_params_merge_and_override_openai_compatible_payloads`; `src/types.rs` | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `scratch.ts` | N/A | v0.99.2 final package corpus | Upstream support/fixture inventory | Upstream scratch/support file, not an executable release test. |
| `stream.test.ts` | LIVE UNEXECUTED | accepted v0.99.1 baseline | `src/tests/transports/stream_e2e_live_test.rs`; deterministic provider HTTP/SSE/WebSocket suites under `src/tests/providers/` | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `supports-xhigh.test.ts` | ADAPTED | accepted v0.99.1 baseline | `src/tests/catalogs/supports_xhigh_test.rs`; `src/tests/catalogs/max_thinking_test.rs` | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `system-message-replay.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `telemetry-options.test.ts` | N/A | accepted v0.99.1 baseline | `src/images/mod.rs`; `src/model_catalog.rs` typed dispatch tests | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `text.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `together-models.test.ts` | ADAPTED | accepted v0.99.1 baseline | `src/tests/catalogs/together_xiaomi_models_test.rs` | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `tokens.test.ts` | ADAPTED | accepted v0.99.1 baseline | `src/tests/core/estimate_test.rs`; `src/utils.rs` | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `tool-call-id-normalization.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `tool-call-without-result.test.ts` | ADAPTED | accepted v0.99.1 baseline | `src/tests/core/transcript_test.rs`; `src/tests/providers/other/transcript_provider_test.rs`; `src/transform.rs` | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `total-tokens.test.ts` | ADAPTED | accepted v0.99.1 baseline | `src/tests/core/estimate_test.rs`; `src/tests/providers/anthropic/anthropic_cache_write_1h_cost_test.rs`; `src/simple_options.rs` | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `transcript-tool-changes.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `transform-messages-copilot-openai-to-anthropic.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `typesafe-system-one.test.ts` | ADAPTED | accepted v0.99.1 baseline | `src/tests/providers/other/classifier_system_one_test.rs`; `src/classifiers/system_one.rs` | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `unicode-surrogate.test.ts` | ADAPTED | accepted v0.99.1 baseline | `src/tests/providers/other/edge_case_test.rs`; `src/utils.rs` | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `uuid.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `validation.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `xai-oauth.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `xai-responses.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `xhigh.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `xiaomi-models.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `xiaomi-token-plan-ams-anthropic-empty-signature-smoke.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `zai-coding-plan-models.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `zen.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
