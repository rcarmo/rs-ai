# v1.0.1 bounded 171-file test crosswalk

Source: verified pinned npm artifact `@earendil-works/pi-ai@1.0.1` and fixed official tag `a7229ddc21810d6245105978033b7df645ecc2f7`. Its computed SHA-512 matches npm integrity; advertised SLSA provenance was not independently verified. Baseline: accepted v1.0.0 tag `a13d35a742c6ef8462812a28fbe1d8c8b7431c32`.

The exact release range contains **19 changed paths** with **546 insertions and 153 deletions**. It changes **6 test paths**; the final package test/support corpus contains **171 unique basenames** (**164 executable tests and 7 support files**). The verified pinned npm tarball SHA-256 is `8a9e69b1309cf93405d87729fa123c8b11c6be7c646b16f34f8bef7b792f9138`. `scripts/validate_v101_manifests.py` checks the inventories, hashes, matrices and shortstat.

Anthropic inline mode starts on the first request when both compatibility flags and at least one initial tool are present. The top-level initial definitions, placeholder and array order form a stable cache prefix; later additions carry full inline definitions with same-name removal suppression. Bedrock binding controls apply only to eligible non-GovCloud adaptive models. Cloudflare classifiers accept direct `answers` and nested completed-run envelopes. ChatGPT login binds port 1455 before host effects, preserves typed bind causes and drains owned callback/prompt tasks. Capacity errors are retryable without weakening quota precedence.

Verified schema-v6 catalogs are regenerated only from the exact pinned npm tarball shards: 1536 chat, 59 image and 20 classifier records (1615 total), across 42 provider files. Native staged generation validates all inputs and all three outputs before replacing accepted files.

## Changed-path disposition matrix

| Upstream changed path | Disposition | rs-ai evidence | Notes |
|---|---|---|---|
| `packages/ai/CHANGELOG.md` | N/A | `docs/v101-171-test-crosswalk.md`; verified artifact/tag receipts | Release prose only. |
| `packages/ai/README.md` | ADAPTED | `README.md`; `RELEASE.md` | User-facing classifier and inline-tool semantics are recorded in native docs. |
| `packages/ai/package.json` | ADAPTED | `Cargo.toml`; root `Cargo.lock`; v1.0.1 release test | Root version changes only; JS SDK/telemetry dependencies are N/A and no Rust dependency/AWS graph changes. |
| `packages/ai/scripts/generate-models.ts` | ADAPTED | three native generators; reproducibility and baseline scripts | All three exact catalogs are generated from pinned shards. |
| `packages/ai/scripts/hydrate-model-catalog.ts` | ADAPTED | staged native catalog driver and corruption tests | JS CLI/layout is N/A; validate-only and staged all-output zero-mutation contract is native. |
| `packages/ai/scripts/model-data.ts` | ADAPTED | `scripts/validate_release_model_data.py`; metadata tests | Schema, API/provider/cost, typed identity, grouping and manifest invariants fail closed. |
| `packages/ai/src/api/anthropic-messages.ts` | ADAPTED | `src/provider/anthropic.rs`; inline-tool tests | Full inline definitions, stable initial prefix, OAuth names, strict/eager flags and replacement semantics. |
| `packages/ai/src/api/bedrock-converse-stream.ts` | ADAPTED | `src/provider/bedrock.rs`; Bedrock thinking tests | Exact binding beta/field eligibility and GovCloud/4.6 exclusions. |
| `packages/ai/src/api/cloudflare-workers-ai-system-one.ts` | ADAPTED | `src/classifiers/system_one.rs`; classifier mocks | Direct answers and nested completed run parsing preserve usage. |
| `packages/ai/src/auth/oauth/openai-chatgpt.ts` | ADAPTED | `src/openai_chatgpt_oauth.rs`; `src/oauth_callback.rs`; OAuth tests | Fixed-port bind-before-effects, typed cause, callback/manual race, cancellation and owned cleanup. |
| `packages/ai/src/types.ts` | ADAPTED | `src/types.rs` | Inline tool-definition contract documented on existing compatibility flag. |
| `packages/ai/src/utils/retry.ts` | ADAPTED | `src/retry.rs`; retry classification tests | `model is at capacity` is transient; quota/billing precedence remains non-retryable. |
| `packages/ai/src/utils/transcript.ts` | ADAPTED | `src/transcript.rs`; Anthropic provider conversion | Existing state deltas drive provider-specific same-name replacement. |
| `packages/ai/test/bedrock-thinking-payload.test.ts` | ADAPTED | `src/tests/providers/bedrock/bedrock_thinking_payload_test.rs` | Deterministic full-payload eligibility matrix. |
| `packages/ai/test/cloudflare-workers-ai-system-one.test.ts` | ADAPTED | `src/tests/providers/other/classifier_system_one_test.rs` | Public HTTP mocks for direct/nested/error/usage paths. |
| `packages/ai/test/model-data-validation.test.ts` | ADAPTED | native staged generation and catalog validation tests | Malformed and late-failure mutation faults reject. |
| `packages/ai/test/stream.test.ts` | ADAPTED | direct provider tests; generated catalog pair tests | Live credential matrix is replaced by deterministic public transport mocks and exact IDs. |
| `packages/ai/test/together-models.test.ts` | ADAPTED | v1.0.1 catalog/release tests | Correct suffixed DeepSeek ID and stale-ID absence. |
| `packages/ai/test/transcript-tool-changes.test.ts` | ADAPTED | Anthropic inline-tool and transcript provider tests | Initial prefix, additions, removals, replacement and fallback branches. |

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
| `bedrock-thinking-payload.test.ts` | ADAPTED | v1.0.1 tag `a7229ddc21810d6245105978033b7df645ecc2f7` | `src/tests/providers/bedrock/bedrock_thinking_payload_test.rs`; `src/provider/bedrock.rs` | Changed in v1.0.1; deterministic native payload coverage includes binding eligibility and exclusions. |
| `bedrock-utils.ts` | N/A | v0.99.2 final package corpus | Upstream support/fixture inventory | Upstream test helper, not an executable test file; Bedrock behavior retains accepted Rust coverage. |
| `cache-retention.test.ts` | ADAPTED | accepted v0.99.1 baseline | `src/tests/providers/openai/openai_responses_prompt_cache_test.rs`; `src/tests/providers/anthropic/anthropic_compat_test.rs`; `src/prompt_cache.rs` | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `classifier-models.test.ts` | ADAPTED | accepted v0.99.1 baseline | `src/tests/catalogs/model_types_v0991_test.rs`; `src/tests/providers/other/classifier_system_one_test.rs` | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `cloudflare-ai-binding.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `cloudflare-stream.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `cloudflare-utils.ts` | N/A | v0.99.2 final package corpus | Upstream support/fixture inventory | Upstream test helper, not an executable test file; Cloudflare behavior retains accepted Rust coverage. |
| `cloudflare-workers-ai-system-one.test.ts` | ADAPTED | v1.0.1 tag `a7229ddc21810d6245105978033b7df645ecc2f7` | `src/tests/providers/other/classifier_system_one_test.rs::cloudflare_uses_account_scoped_run_envelope_and_reports_errors`; `src/classifiers/system_one.rs` | Changed in v1.0.1; bounded native evidence is listed above and in the focused production tests. |
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
| `model-data-validation.test.ts` | ADAPTED | v1.0.1 tag `a7229ddc21810d6245105978033b7df645ecc2f7` | `src/tests/catalogs/model_data_validation_test.rs`; `scripts/validate_release_model_data.py` | Changed in v1.0.1; bounded native evidence is listed above and in the focused production tests. |
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
| `stream.test.ts` | LIVE UNEXECUTED | v1.0.1 tag `a7229ddc21810d6245105978033b7df645ecc2f7` | `src/tests/transports/stream_e2e_live_test.rs`; deterministic provider HTTP/SSE/WebSocket suites under `src/tests/providers/` | Changed in v1.0.1; bounded native evidence is listed above and in the focused production tests. |
| `supports-xhigh.test.ts` | ADAPTED | accepted v0.99.1 baseline | `src/tests/catalogs/supports_xhigh_test.rs`; `src/tests/catalogs/max_thinking_test.rs` | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `system-message-replay.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `telemetry-options.test.ts` | N/A | accepted v0.99.1 baseline | `src/images/mod.rs`; `src/model_catalog.rs` typed dispatch tests | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `text.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `together-models.test.ts` | ADAPTED | v1.0.1 tag `a7229ddc21810d6245105978033b7df645ecc2f7` | `src/tests/catalogs/together_xiaomi_models_test.rs` | Changed in v1.0.1; bounded native evidence is listed above and in the focused production tests. |
| `tokens.test.ts` | ADAPTED | accepted v0.99.1 baseline | `src/tests/core/estimate_test.rs`; `src/utils.rs` | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `tool-call-id-normalization.test.ts` | COVERED | accepted v0.99.1 baseline | Existing accepted Rust parity suite | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `tool-call-without-result.test.ts` | ADAPTED | accepted v0.99.1 baseline | `src/tests/core/transcript_test.rs`; `src/tests/providers/other/transcript_provider_test.rs`; `src/transform.rs` | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `total-tokens.test.ts` | ADAPTED | accepted v0.99.1 baseline | `src/tests/core/estimate_test.rs`; `src/tests/providers/anthropic/anthropic_cache_write_1h_cost_test.rs`; `src/simple_options.rs` | Unchanged in v0.99.1→v0.99.2; accepted disposition retained. |
| `transcript-tool-changes.test.ts` | ADAPTED | v1.0.1 tag `a7229ddc21810d6245105978033b7df645ecc2f7` | `src/tests/providers/other/transcript_provider_test.rs`; Anthropic inline-tool tests; `src/provider/anthropic.rs` | Changed in v1.0.1; native coverage locks stable prefix, inline definitions, replacements, removals and fallback. |
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
