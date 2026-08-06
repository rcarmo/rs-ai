# rs-ai upstream release parity

## In-progress v0.84.0 release port — committed slice 1 (Baseten / sampling / vLLM budget)

- Previous accepted upstream: `v0.83.0` / `845d6ff1f6643aba440341cce877ce1c43ebbc39`
- Target upstream release: `v0.84.0` / `a5f43bf8aff3c55752432655f7334e3dafd1e256`
- Audited range for manifests: `845d6ff1f6643aba440341cce877ce1c43ebbc39..a5f43bf8aff3c55752432655f7334e3dafd1e256`
- Scope status: **in progress**. This is the first evidence-driven committed slice requested by the auditor, not the final v0.84.0 completion report.

### Exact manifests generated

- `docs/v0840-manifests.md` records the exact **101** changed `packages/ai` paths and **46** changed `packages/ai/test` files, with extracted upstream case/assertion/gate lines from the authoritative tag.

### Ported/adapted in this slice

- Regenerated text and image catalogs from the v0.84.0 tag JSON output.
- Added Baseten provider catalog/auth parity:
  - provider id `baseten`
  - `BASETEN_API_KEY`
  - `openai-completions` runtime path
  - `zai-org/GLM-5.2`, `zai-org/GLM-5.2-Fast`, `moonshotai/Kimi-K2.6`, and related Baseten metadata from `scripts/generate-models.ts`.
- Added `Model.sampling_params` / `StreamOptions.sampling_params` and OpenAI-compatible request merging:
  - model defaults merge with request params
  - request keys override model keys
  - merged params are applied last in OpenAI Completions and OpenAI/Azure Responses payloads so arbitrary sampling keys can override named request fields.
- Added Baseten `thinkingFormat: "baseten"` handling with configurable `chat_template_args` and optional `reasoning_effort`.
- Added vLLM `thinking_token_budget` support for OpenAI-compatible Completions when `supportsThinkingTokenBudget` is set, including the upstream `MIN_ANSWER_TOKENS = 1024` edge behavior.
- Added `supportsFinishReason: false` OpenAI-compatible stream inference so streams without provider finish reasons infer `stop` vs `toolUse` instead of failing.
- Fixed validation union coercion to preserve values that already match nullable `anyOf`/`oneOf` arms before coercing through earlier primitive arms.
- Changed generated text registry construction to append in small chunks, avoiding test-stack overflow with the larger v0.84.0 catalog.

### Named Rust evidence in this slice

`src/v0840_release_test.rs`:

- `sampling_params_merge_and_override_openai_compatible_payloads`
- `baseten_catalog_and_reasoning_payload_match_v0840`
- `vllm_thinking_token_budget_edge_matrix`
- `nullable_anyof_oneof_preserves_matching_null_before_coercion`
- `supports_finish_reason_false_infers_terminal_stop_or_tool_use`

### Slice verification

Executed from `/workspace/projects/rs-ai`:

```bash
PI_AI_MODEL_DATA_DIR=/workspace/tmp/pi-v0840-release-json   python3 scripts/compare_upstream_registry_pairs.py   /workspace/tmp/pi-v0840   a5f43bf8aff3c55752432655f7334e3dafd1e256
cargo build
cargo test v0840_release_test -- --nocapture
cargo clippy --all-targets -- -D warnings
```

Results:

- Comparator: text `1153/1153`, image `42/42`, missing `0`, extra `0`
- `cargo build`: passed
- `cargo test v0840_release_test -- --nocapture`: `5 passed; 0 failed`
- `cargo clippy --all-targets -- -D warnings`: passed


### Catalog correction: release-pinned provider shards supersede dynamic 1212 evidence

Auditor correction accepted: the earlier `1212` text-pair evidence was generated from a fresh dynamic `models.dev`/OpenRouter aggregate and included 59 OpenRouter `:batch` aliases that are not present in either authoritative official-release artifact. It is superseded and must not be used as v0.84.0 parity evidence.

Authoritative catalog source for this release audit is offline and release-pinned:

- upstream tag worktree: `/workspace/tmp/pi-v0840` at `a5f43bf8aff3c55752432655f7334e3dafd1e256`
- unpacked npm package: `/workspace/tmp/pi-ai-0.84.0-package/package`
- provider shards: `/workspace/tmp/pi-ai-0.84.0-package/package/dist/providers/data/*.json`
- provider manifest `schemaVersion=3`, `generatedAt=2026-08-06T11:03:30.465Z`, `structureHash=3ed4153a80db1d4458c5d275334d995f90f9a9c3d77970a0262c715df0e43e79`
- extracted release JSON: `/workspace/tmp/pi-v0840-release-json/models.json`
- extractor metadata: `/workspace/tmp/pi-v0840-release-json/source-metadata.json`

Hardened extractor: `scripts/extract_release_model_shards.py` reads only local npm `dist/providers/data` shards, records package/shard/manifest hashes, verifies the tag worktree SHA when provided, and fails if any `:batch` aliases appear in the release shards. Current release-pinned text catalog is **1153 provider/id pairs across 38 providers and 9 APIs** with **0** `:batch` aliases; image catalog remains **42** pairs. Regression evidence is `release_pinned_catalog_has_no_unpinned_batch_aliases` in `src/v0840_release_test.rs`.

### Committed slice 2: public deferred/background response lifecycle

Auditor priority correction: upstream commit `382aa641cc4c197dfa95ed684f187b5a39bc30ce` (`DRAFT: add openai background mode responses`) is public required behavior, not N/A and not deferred-tool-only.

Ported/adapted in this slice:

- Added public `DeferredHandle` and `DeferredRequest` Rust types.
- Added `StopReason::Deferred` and `Message.deferred` serde-compatible state.
- Added `StreamOptions.deferred` and `StreamOptions.wait` request fields.
- Extended `ApiProvider` with public `fetch_deferred` and `cancel_deferred` capability methods.
- Added top-level `registry::fetch_deferred` / `registry::cancel_deferred` dispatch.
- Extended `FauxProvider` with deterministic deferred/background lifecycle state:
  - submit with deferred option returns a handle and `StopReason::Deferred`
  - first N polls can return pending/deferred
  - ready poll streams the stored final assistant message
  - cancellation records handles and turns later fetches into in-band error messages
  - unknown handle fetches return in-band assistant errors.
- Added type aliases for boxed event streams and async cancellation futures to keep the public trait Send/pin-safe and Clippy-clean.

Named Rust evidence:

- `providers_upstream_test::tests::faux_provider_submits_polls_and_redeems_deferred_responses`
- `providers_upstream_test::tests::faux_provider_records_cancellation_and_fetches_cancelled_handle_as_error`
- `providers_upstream_test::tests::unsupported_deferred_capability_reports_in_band_provider_errors`
- Existing `provider::faux` stream tests remain active against the expanded provider.

Slice 2 verification:

```bash
cargo test providers_upstream_test -- --nocapture
cargo test provider::faux -- --nocapture
cargo test v0840_release_test -- --nocapture
cargo test
cargo fmt --check
PI_AI_MODEL_DATA_DIR=/workspace/tmp/pi-v0840-release-json   python3 scripts/compare_upstream_registry_pairs.py /workspace/tmp/pi-v0840 a5f43bf8aff3c55752432655f7334e3dafd1e256
cargo build
cargo clippy --all-targets -- -D warnings
```

Results: provider/deferred targeted tests passed; full `cargo test` passed with `836 passed; 0 failed`; doctest `1 passed`; comparator remains text `1153/1153`, image `42/42`, missing `0`, extra `0`; build/fmt/clippy passed.

### Committed slice 4: provider stream/error regressions

Ported/adapted executable provider-specific v0.84.0 regressions:

- Anthropic Messages preserves initial `content_block_start` text, thinking text, and thinking signature before later deltas.
- OpenAI/Azure Responses terminal handling now records `incomplete_details.reason` in `raw_stop_reason` as `status.reason`, maps only `incomplete.max_output_tokens` to `length`, and surfaces other incomplete reasons as errors.
- Google history conversion now requires tool-call IDs for Gemini 3+ models and preserves same-model signed empty text/thinking blocks instead of dropping the reasoning signatures.
- Bedrock failures now attach structured `bedrock_response_failure` diagnostics with status/errorCode/requestId where available while preserving the retry-facing error message.

Named Rust evidence:

- `anthropic_sse_parsing_test::tests::preserves_initial_text_and_thinking_from_content_block_start`
- `openai_responses_terminal_event_test::tests::finalizes_incomplete_max_output_terminal_events_as_length_stops`
- `openai_responses_terminal_event_test::tests::incomplete_non_max_output_reason_is_error_with_raw_reason`
- `google_gemini3_unsigned_tool_call_test::tests::no_skip_validator_for_unsigned_google_gen_ai_tool_calls` (updated to assert Gemini 3 IDs)
- `google_signed_empty_blocks_test::tests::preserves_same_model_empty_text_and_thinking_blocks_when_signed`
- `google_signed_empty_blocks_test::tests::drops_cross_model_empty_signed_thinking_because_signature_is_unusable`
- `bedrock_error_metadata_test::tests::bedrock_failure_diagnostic_preserves_status_code_and_request_id_without_rewriting_error_message`
- `bedrock_error_metadata_test::tests::bedrock_failure_diagnostic_drops_empty_or_overlong_values`

Slice verification:

```bash
cargo test bedrock_error_metadata_test -- --nocapture
cargo test anthropic_sse_parsing_test -- --nocapture
cargo test openai_responses_terminal_event_test -- --nocapture
cargo test google_gemini3_unsigned_tool_call_test -- --nocapture
cargo test google_signed_empty_blocks_test -- --nocapture
cargo test google_shared_convert_tools_test -- --nocapture
cargo test google_thinking_signature_test -- --nocapture
cargo fmt --check
PI_AI_MODEL_DATA_DIR=/workspace/tmp/pi-v0840-release-json   python3 scripts/compare_upstream_registry_pairs.py /workspace/tmp/pi-v0840 a5f43bf8aff3c55752432655f7334e3dafd1e256
cargo build
cargo test
cargo clippy --all-targets -- -D warnings
```

Results: targeted provider tests passed; full `cargo test` passed with `843 passed; 0 failed`; doctest `1 passed`; comparator remains text `1153/1153`, image `42/42`, missing `0`, extra `0`; build/fmt/clippy passed.

### Committed slice 5: runtime auth/options/telemetry semantics

Ported/adapted runtime/OAuth/telemetry v0.84.0 behavior:

- Added `merge_provider_headers` with case-insensitive null/deletion semantics for provider-resolved headers, preserving Rust request headers as concrete strings while still testing the upstream `ProviderHeaders` deletion model.
- Added cancellation-aware OAuth refresh seam (`refresh_with_cancel`) and `AuthResolutionOverrides.cancel`; refresh receives a cancellation receiver and preserves the rotated credential only after successful refresh.
- Added `RefreshOptions.providers` filtering so selected refreshes skip unrequested dynamic providers and ignore unknown provider IDs.
- Added `TelemetryContext` on `StreamOptions` and `ImagesOptions`, plus FauxProvider capture to prove telemetry metadata flows through normal stream and deferred fetch options; image options carry the same opaque telemetry context structurally.

Named Rust evidence:

- `auth::tests::merge_provider_headers_supports_null_deletion_case_insensitively`
- `auth::tests::resolve_oauth_refresh_receives_cancellation_signal_and_persists_success`
- `models_runtime_refresh_test::tests::refresh_provider_filter_skips_unrequested_dynamic_providers`
- `providers_upstream_test::tests::telemetry_context_flows_through_stream_and_deferred_fetch_options`

Slice verification:

```bash
cargo test providers_upstream_test -- --nocapture
cargo test auth::tests::merge_provider_headers_supports_null_deletion_case_insensitively -- --nocapture
cargo test auth::tests::resolve_oauth_refresh_receives_cancellation_signal_and_persists_success -- --nocapture
cargo test models_runtime_refresh_test::tests::refresh_provider_filter_skips_unrequested_dynamic_providers -- --nocapture
cargo fmt --check
PI_AI_MODEL_DATA_DIR=/workspace/tmp/pi-v0840-release-json   python3 scripts/compare_upstream_registry_pairs.py /workspace/tmp/pi-v0840 a5f43bf8aff3c55752432655f7334e3dafd1e256
cargo build
cargo test
cargo clippy --all-targets -- -D warnings
```

Results: targeted runtime/OAuth/telemetry tests passed; full `cargo test` passed with `847 passed; 0 failed`; doctest `1 passed`; comparator remains text `1153/1153`, image `42/42`, missing `0`, extra `0`; build/fmt/clippy passed.

### Closure slice 6: runtime/OAuth/telemetry dispatch, Codex WS account cache, Bedrock metadata

This slice closes the auditor-requested runtime/auth/telemetry cluster with executable provider/path coverage rather than helper-only assertions:

- Caller-owned OAuth refresh cancellation is wired through concrete OAuth providers (`AnthropicOAuth`, `CodexOAuth`, `KimiCodeOAuth`, `XaiOAuth`, `RadiusOAuth`, `OpenRouterOAuth`) with pre-cancel and mid-refresh tests. Aborted refreshes return typed `AbortError`-bearing OAuth errors and do not persist rotated credentials.
- `TelemetryContext` is captured through `registry::stream_simple`, deferred submit/fetch/cancel, and image provider dispatch.
- `ProviderHeaders` null/deletion semantics now flow through `merge_auth_into_request` into the real OpenAI-compatible request builder; explicit request headers still win afterward.
- `RefreshOptions.providers`, cancellation and supersession are covered through `ModelsRuntime::refresh`: aborted callers stop waiting on non-cooperative providers, and late first-generation refreshes cannot overwrite newer dynamic catalog state.
- Codex sticky WebSocket fallback is keyed by ChatGPT account id plus session id; one account’s WS failure no longer poisons another account using the same session id, while the original account reuses SSE.
- Bedrock failure diagnostics now extract SDK raw-response status and request id where available, suppress `Unknown`/transport names, keep retry-facing error messages untouched, and cover modeled/unmodeled send and stream diagnostic shapes.

Named Rust evidence:

- `auth_providers::tests::real_oauth_providers_pre_cancel_without_network_or_rotation`
- `auth_providers::tests::real_oauth_providers_mid_refresh_cancel_without_rotation`
- `auth_providers::tests::openrouter_oauth_honors_pre_cancel_without_mutation`
- `providers_upstream_test::tests::telemetry_context_flows_through_stream_simple_deferred_cancel_and_images`
- `models_runtime_auth_test::tests::provider_header_null_deletion_reaches_openai_request_builder`
- `models_runtime_refresh_test::tests::refresh_abort_stops_waiting_on_non_cooperative_provider`
- `models_runtime_refresh_test::tests::late_refresh_publication_is_rejected_after_supersession`
- `codex_ws_account_cache_test::tests::websocket_fallback_is_scoped_by_account_and_session`
- `bedrock_error_metadata_test::tests::*` (7-case status/requestId/code/suppression matrix)

### Still pending for final v0.84.0 completion

The remaining changed assertions/provider clusters from `docs/v0840-manifests.md` still need final disposition and executable evidence before declaring the v0.84.0 release complete, including Google retry changes and remaining provider-specific stream/tool/error/usage fixes not already closed by slices 4–6.

This file is the release-audit ledger for `rs-ai`. It must be updated in the same commit as every future upstream `@earendil-works/pi-ai` release audit.

## Current upstream baseline

- Upstream package: `@earendil-works/pi-ai`
- Current accepted release: `v0.83.0`
- Upstream tag/commit: `845d6ff1f6643aba440341cce877ce1c43ebbc39`
- Previous accepted upstream baseline: `v0.82.1` / `b4f293684bba718d59cc1157679bcf6157b3a7f5`
- Audited range: `b4f293684bba718d59cc1157679bcf6157b3a7f5..845d6ff1f6643aba440341cce877ce1c43ebbc39`
- Scope: `packages/ai` only; no newer-main chase.

## Exact upstream change set

The v0.83.0 release changes 41 `packages/ai` paths:

- Release/docs/package metadata:
  - `CHANGELOG.md`
  - `README.md`
  - `package.json`
- Catalog generation:
  - `scripts/generate-models.ts`
- API/runtime:
  - `src/api/anthropic-messages.ts`
  - `src/api/azure-openai-responses.ts`
  - `src/api/bedrock-converse-stream.ts`
  - `src/api/google-generative-ai.ts`
  - `src/api/google-vertex.ts`
  - `src/api/mistral-conversations.ts`
  - `src/api/openai-codex-responses.ts`
  - `src/api/openai-completions.ts`
  - `src/api/openai-responses-shared.ts`
  - `src/api/openai-responses.ts`
  - `src/api/openrouter-images.ts`
  - `src/api/pi-messages.ts`
  - `src/api/simple-options.ts`
- Auth/provider/runtime support:
  - `src/auth/oauth/openrouter.ts`
  - `src/auth/resolve.ts`
  - `src/providers/faux.ts`
  - `src/types.ts`
- Tests:
  - `test/anthropic-sse-parsing.test.ts`
  - `test/azure-openai-responses-reasoning-replay.test.ts`
  - `test/bedrock-credentials.test.ts`
  - `test/bedrock-raw-stop-reason.test.ts`
  - `test/constrained-sampling.test.ts`
  - `test/faux-provider.test.ts`
  - `test/fetch-option.test.ts`
  - `test/github-copilot-anthropic.test.ts`
  - `test/google-raw-stop-reason.test.ts`
  - `test/mistral-raw-stop-reason.test.ts`
  - `test/models-runtime.test.ts`
  - `test/oauth-auth.test.ts`
  - `test/openai-completions-raw-stop-reason.test.ts`
  - `test/openai-completions-tool-choice.test.ts`
  - `test/openai-responses-partial-json-cleanup.test.ts`
  - `test/openai-responses-terminal-event.test.ts`
  - `test/openrouter-oauth.test.ts`
  - `test/pi-messages.test.ts`
  - `test/qwen-token-plan-models.test.ts`
  - `test/validation.test.ts`

## Rust implementation and disposition

### Catalog and metadata

- Regenerated `src/models_generated.rs` from hydrated v0.83.0 JSON shards.
- Text provider/id comparator:
  - `upstream=1153 local=1153 missing=0 extra=0`
- Image provider/id comparator:
  - `upstream=40 local=40 missing=0 extra=0`
- Repro command:

```bash
PI_AI_MODEL_DATA_DIR=/workspace/tmp/pi-v0830-json \
  scripts/compare_upstream_registry_pairs.py \
  /workspace/tmp/pi-v0830 \
  845d6ff1f6643aba440341cce877ce1c43ebbc39
```

### Stop reason and raw stop reason

- Added `StopReason::Pending` to mirror the v0.83 public type surface.
- Added `Message.raw_stop_reason` with serde-compatible camelCase field handling.
- Streaming assistant partials now start as `Pending` instead of absent/successful stop state.
- `Pending` must not emit a successful `Done`; streams that end pending are surfaced as errors.
- Raw stop reason capture is wired for:
  - OpenAI Completions
  - OpenAI Responses / Azure OpenAI Responses
  - OpenAI Codex Responses
  - Anthropic Messages
  - Google Generative AI / Vertex shared paths
  - Mistral Conversations
  - Bedrock Converse Stream
- Tests include raw-stop and pending/no-terminal behavior in provider fixtures and `src/v0830_release_test.rs`.

### Constrained sampling

Already completed as part of the v0.82.0 corrective work and retained for v0.83.0:

- `Tool.constrained_sampling` with camelCase serde.
- JSON-schema strict constrained sampling.
- Grammar constrained sampling:
  - lark/regex variant resolution
  - one-required-string schema validation
  - custom-tool request shape
  - monotonic streamed JSON delta reconstruction
- Integrated request-shape handling for OpenAI Completions, Responses/Azure, and Codex payloads.
- Tests are in `src/v0830_release_test.rs` and provider request fixtures.

### Retry behavior

Already completed as part of v0.82.0 corrective work and retained:

- `do_with_retry` honors `x-should-retry`.
- Excessive provider-requested retry delays fail immediately instead of silently clamping.
- Backoff sleep can be aborted via `do_with_retry_cancel`.
- Transport/no-status request errors are retryable.
- Tests cover direct retry helper behavior and actual OpenAI provider request path.

### OAuth/auth/model runtime

Existing v0.82.1 work remains active:

- `ModelsError::with_cause` preserves underlying cause detail in surfaced messages.
- Production auth resolution uses cause-preserving errors for API-key and OAuth failure paths.
- Radius OAuth discovery-only gateway routing is supported.
- Radius dynamic catalog refresh supports ETag/`If-None-Match` and 304 cached reuse.
- Anthropic `ANTHROPIC_AUTH_TOKEN` participates in env discovery and is sent as `Authorization: Bearer`, not `x-api-key`.

### v0.83 corrective runtime details

- Malformed OpenAI tool delta regression `malformed_openai_delta_preserves_function_when_custom_is_empty` verifies valid `function` payload plus empty `custom` preserves function name and arguments.
- Added focused raw/pending parser tests for `anthropic_raw_stop_and_missing_stop_are_executable`, `responses_pending_status_is_error_and_preserves_raw_stop_reason`, `azure_responses_pending_terminal_status_is_error_with_raw_reason`, `codex_pending_status_is_error_and_raw`, `google_and_mistral_raw_stop_reasons_are_executable`, and `bedrock_raw_stop_reason_helper_errors_unknown_and_preserves_raw`.

- OAuth resolution now refreshes tokens early by default when less than 5 minutes of validity remain. `min_oauth_validity_ms` is floored to that upstream default 5-minute window, and refreshed credentials are rejected when they still do not satisfy the effective minimum validity.
- OpenAI malformed/empty custom and grammar constrained-sampling request/stream behavior remains covered by v0.82/v0.83 release fixtures.
- Provider raw stop reason tests cover OpenAI plus provider-specific raw/pending behavior through release and provider tests.
- Bedrock explicit profile precedence is tested: an explicit `StreamOptions.profile`/profile seam suppresses standard-endpoint region pinning even when ambient access keys exist, while ARN regions and custom endpoints retain priority.

### Fetch option changes

- Upstream v0.83.0 adds optional custom fetch plumbing in TypeScript APIs.
- Rust uses `reqwest` clients and proxy-aware builders instead of JavaScript `fetch`; there is no direct `fetch` injection point.
- Disposition: **N/A for Rust runtime**, with existing request/proxy/client tests covering the equivalent Rust transport surface.

### TypeScript-only/model-catalog tests

- Type-only/export/build-surface changes are not applicable to Rust.
- Disposition: **N/A**, with Rust model-data structural validation and serde model shape tests covering runtime-valid metadata.

### Live/credential tests

- Upstream live credential/e2e matrix updates remain N/A when they require external provider credentials or nondeterministic live service behavior.
- Deterministic request, stream, retry, error, and metadata behavior is covered by local mock-server tests.

## Verification for current release

Executed from `/workspace/projects/rs-ai`:

```bash
cargo fmt --check
PI_AI_MODEL_DATA_DIR=/workspace/tmp/pi-v0830-json \
  scripts/compare_upstream_registry_pairs.py \
  /workspace/tmp/pi-v0830 \
  845d6ff1f6643aba440341cce877ce1c43ebbc39
cargo build
cargo test   # run 3 times
cargo clippy --all-targets -- -D warnings
```

Results:

- Comparator: text `1153/1153`, image `40/40`, missing `0`, extra `0`
- `cargo build`: passed
- `cargo test` x3: each run `822 passed; 0 failed; 0 ignored`; doctest `1 passed; 0 failed`
- `cargo clippy --all-targets -- -D warnings`: passed

## Release-audit policy

For every future upstream `@earendil-works/pi-ai` release audit:

1. Pin the exact previous accepted upstream tag/SHA and new upstream tag/SHA.
2. Do not chase upstream main beyond the requested release tag.
3. List the exact changed `packages/ai` paths or a grouped matrix that accounts for all paths by count.
4. Regenerate and compare model/image catalogs with the JSON-shard-aware comparator when applicable.
5. Implement all applicable runtime/API/provider/model/tool/stream/error/usage behavior in production Rust paths.
6. Document every N/A decision with the concrete reason.
7. Run the required Rust gates.
8. Update this `RELEASE.md` in the same release commit before reporting completion.
