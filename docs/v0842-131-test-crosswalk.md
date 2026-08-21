# v0.84.2 cumulative 131-file test crosswalk

Source: official upstream `@earendil-works/pi-ai` tag `914cf1472e715297caa30db4b9535d534a9eb718` (`v0.84.2`). Baseline: accepted `v0.84.1` tag `53fa77ccd8a279eb87e92294ef3687b03ff80112`.

Total upstream `packages/ai/test/*.test.ts` files: **131**.

## Release-delta test accounting

The bounded v0.84.2 range changes **21 test paths total**:

- **18 existing tests modified**: `anthropic-auth-token`, `anthropic-eager-tool-input-compat`, `bedrock-convert-messages`, `constrained-sampling`, `context-overflow`, `deferred-tools`, `github-copilot-oauth`, `google-raw-stop-reason`, `lazy-module-load`, `mistral-raw-stop-reason`, `openai-codex-stream`, `openai-completions-tool-choice`, `openai-responses-compat`, `retry`, `stream`, `supports-xhigh`, `total-tokens`, `validation`.
- **3 new tests added**: `cloudflare-gateway-binding.test.ts`, `mistral-http-transport.test.ts`, `openai-responses-namespace.test.ts`.

## v0.84.2 dispositions

| Upstream file/group | Disposition | rs-ai evidence | Notes |
|---|---|---|---|
| `constrained-sampling.test.ts`, `validation.test.ts` | ADAPTED | `src/v0842_release_test.rs::{strict_json_schema_tools_require_optional_properties_as_nullable,optional_non_nullable_null_is_omitted_but_nullable_null_is_preserved}` | Strict JSON-schema conversion now requires all object properties, wraps optional non-nullable properties in `anyOf [..., null]`, rejects unsafe constructs; validation omits optional non-nullable nulls but preserves nullable nulls. |
| `anthropic-auth-token.test.ts`, `openai-codex-stream.test.ts` user-agent rows | ADAPTED | `src/v0842_release_test.rs::pi_runtime_user_agent_includes_platform_release_and_arch`, `src/provider/{anthropic,codex}.rs` | Kimi/Codex use `pi (${platform} ${release}; ${arch})`-style runtime user agent. Anthropic ordinary API-key path remains without this header. |
| `deferred-tools.test.ts`, `openai-responses-namespace.test.ts` | ADAPTED | `src/v0842_release_test.rs::{responses_additional_tools_supersedes_tool_search_for_deferred_tools,responses_replays_namespace_only_when_additional_tools_supported}`, `src/provider/responses.rs` | Responses/Codex message-anchored tools prefer `additional_tools` when supported, fall back to tool search/top-level tools otherwise; tool-call namespaces replay only for models that can replay load items. |
| `openai-codex-stream.test.ts` `endTurn` rows | ADAPTED | `Message.end_turn`, `src/provider/{codex,responses}.rs` | Terminal `response.end_turn` is preserved as `Message.endTurn` for diagnostics without changing stop-reason control flow. |
| `mistral-http-transport.test.ts`, `mistral-raw-stop-reason.test.ts` | ADAPTED | `src/v0842_release_test.rs::{mistral_http_sse_parses_utf8_usage_and_raw_tool_stop,mistral_http_stream_yields_delayed_chunks_incrementally,mistral_http_stream_preserves_utf8_split_across_byte_chunks,mistral_http_stream_cancel_while_waiting_for_chunk_cleans_up,mistral_http_stream_timeout_while_awaiting_chunk_reports_error,mistral_http_uses_bounded_branded_error_body_for_403,mistral_http_retries_with_replayable_json_body,mistral_http_affinity_override_and_suppression_are_honored,mistral_http_exact_wire_payload_matches_replay_contract}`, Mistral provider reqwest bytes_stream path | Native reqwest SSE path serializes/streams Mistral HTTP responses, preserves UTF-8 chunks, raw tool stop reason, tool calls, thinking/text, and cached-token usage. |
| `google-raw-stop-reason.test.ts` | ADAPTED | `src/provider/google.rs`, existing provider tests plus v0.84.2 compile gate | `MAX_TOKENS` with tool calls maps to `length` while `STOP` with tool calls maps to `toolUse`; raw stop reason is preserved. |
| `bedrock-convert-messages.test.ts` | COVERED / ADAPTED | `src/provider/bedrock.rs` existing stream/replay conversion tests | Streamed Bedrock tool args preserve empty-key data; replay conversion sanitizes only replayed Bedrock input as required. |
| `retry.test.ts` | ADAPTED | `src/v0842_release_test.rs::retry_classifier_matches_request_buffer_exhaustion_wording` | Retry classifier now treats `exceeded request buffer limit` as transient/retryable. |
| `openai-completions-tool-choice.test.ts`, `supports-xhigh.test.ts` | ADAPTED | regenerated `src/models_generated.rs`, `src/compat.rs`, `src/v0842_release_test.rs::deepseek_detection_is_case_insensitive_and_uses_max_tokens` | Case-insensitive DeepSeek URL detection; DeepSeek-compatible request shape uses `max_tokens`; catalog thinking levels regenerated. |
| `cloudflare-gateway-binding.test.ts` | N/A (Workers binding object semantics) / ASSESSED | `docs/upstream-parity-gaps.md`, existing `src/cloudflare_stream_test.rs` | Rust has reqwest/HTTP transport injection, not a Cloudflare Workers AI Gateway binding object. Tokenless binding routing is JS/Workers-specific; existing Rust Cloudflare gateway URL/header behavior remains covered. |
| Credential-gated/live matrices (`context-overflow`, `stream`, `total-tokens`, portions of `github-copilot-oauth`, `lazy-module-load`) | LIVE UNEXECUTED / generic deterministic counterparts remain covered | Existing provider/runtime suites plus focused v0.84.2 deterministic tests | Live credential behavior is not executed in Rust CI; shared deterministic request/parser/classifier behavior is covered separately. |

## Per-file 131-test disposition matrix

Executable validator: `python3 scripts/validate_v0842_manifests.py` asserts these **131** unique rows against the upstream tag inventory.

| Upstream test file | Disposition | Source | rs-ai evidence | Notes |
|---|---|---|---|---|
| `abort.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `anthropic-adaptive-thinking-models.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `anthropic-auth-token.test.ts` | ADAPTED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | `src/v0842_release_test.rs`, `src/github_copilot_oauth_test.rs`, provider-specific Rust modules/tests | Changed in v0.84.2 release range and covered by focused deterministic Rust evidence or documented adaptation. |
| `anthropic-cache-write-1h-cost.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `anthropic-eager-tool-input-compat.test.ts` | ADAPTED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | `src/v0842_release_test.rs`, `src/github_copilot_oauth_test.rs`, provider-specific Rust modules/tests | Changed in v0.84.2 release range and covered by focused deterministic Rust evidence or documented adaptation. |
| `anthropic-eager-tool-input-e2e.test.ts` | LIVE UNEXECUTED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing deterministic parser/request/runtime unit tests; live credentials intentionally not run in CI | Upstream live/credential matrix retained as N/A for deterministic Rust CI. |
| `anthropic-empty-thinking-signature-compat.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `anthropic-force-adaptive-thinking.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `anthropic-long-cache-retention-e2e.test.ts` | LIVE UNEXECUTED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing deterministic parser/request/runtime unit tests; live credentials intentionally not run in CI | Upstream live/credential matrix retained as N/A for deterministic Rust CI. |
| `anthropic-oauth.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `anthropic-opus-4-8-smoke.test.ts` | LIVE UNEXECUTED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing deterministic parser/request/runtime unit tests; live credentials intentionally not run in CI | Upstream live/credential matrix retained as N/A for deterministic Rust CI. |
| `anthropic-sse-parsing.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `anthropic-temperature-compat.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `anthropic-thinking-disable.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `anthropic-tool-name-normalization.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `azure-openai-base-url.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `azure-openai-responses-reasoning-replay.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `baseten-models.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `bedrock-convert-messages.test.ts` | ADAPTED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | `src/v0842_release_test.rs`, `src/github_copilot_oauth_test.rs`, provider-specific Rust modules/tests | Changed in v0.84.2 release range and covered by focused deterministic Rust evidence or documented adaptation. |
| `bedrock-credentials.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `bedrock-custom-headers.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `bedrock-endpoint-resolution.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `bedrock-error-metadata.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `bedrock-models.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `bedrock-raw-stop-reason.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `bedrock-thinking-payload.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `cache-retention.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `cloudflare-gateway-binding.test.ts` | N/A | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | `docs/upstream-parity-gaps.md`, `src/cloudflare_stream_test.rs` | Workers binding object semantics are JS/Cloudflare-only; Rust preserves HTTP routing/header adaptation. |
| `cloudflare-stream.test.ts` | LIVE UNEXECUTED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing deterministic parser/request/runtime unit tests; live credentials intentionally not run in CI | Upstream live/credential matrix retained as N/A for deterministic Rust CI. |
| `compat-env.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `constrained-sampling.test.ts` | ADAPTED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | `src/v0842_release_test.rs`, `src/github_copilot_oauth_test.rs`, provider-specific Rust modules/tests | Changed in v0.84.2 release range and covered by focused deterministic Rust evidence or documented adaptation. |
| `context-estimate.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `context-overflow.test.ts` | ADAPTED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | `src/v0842_release_test.rs`, `src/github_copilot_oauth_test.rs`, provider-specific Rust modules/tests | Changed in v0.84.2 release range and covered by focused deterministic Rust evidence or documented adaptation. |
| `cross-provider-handoff.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `deferred-tools.test.ts` | ADAPTED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | `src/v0842_release_test.rs`, `src/github_copilot_oauth_test.rs`, provider-specific Rust modules/tests | Changed in v0.84.2 release range and covered by focused deterministic Rust evidence or documented adaptation. |
| `empty.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `env-api-keys.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `error-body.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `faux-provider.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `fetch-option.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `fireworks-models.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `generate-models-strict.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `github-copilot-anthropic.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `github-copilot-oauth.test.ts` | ADAPTED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | `src/v0842_release_test.rs`, `src/github_copilot_oauth_test.rs`, provider-specific Rust modules/tests | Changed in v0.84.2 release range and covered by focused deterministic Rust evidence or documented adaptation. |
| `google-raw-stop-reason.test.ts` | ADAPTED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | `src/v0842_release_test.rs`, `src/github_copilot_oauth_test.rs`, provider-specific Rust modules/tests | Changed in v0.84.2 release range and covered by focused deterministic Rust evidence or documented adaptation. |
| `google-shared-convert-tools.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `google-shared-gemini3-unsigned-tool-call.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `google-shared-image-tool-result-routing.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `google-shared-retry.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `google-shared-signed-empty-blocks.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `google-thinking-disable.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `google-thinking-signature.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `google-vertex-api-key-resolution.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `image-model-data.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `image-tool-result.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `images-models.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `images.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `interleaved-thinking.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `kimi-coding-oauth.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `lax-message-content.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `lazy-module-load.test.ts` | ADAPTED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | `src/v0842_release_test.rs`, `src/github_copilot_oauth_test.rs`, provider-specific Rust modules/tests | Changed in v0.84.2 release range and covered by focused deterministic Rust evidence or documented adaptation. |
| `max-thinking.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `mistral-http-transport.test.ts` | ADAPTED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | `src/v0842_release_test.rs`, `src/github_copilot_oauth_test.rs`, provider-specific Rust modules/tests | Changed in v0.84.2 release range and covered by focused deterministic Rust evidence or documented adaptation. |
| `mistral-raw-stop-reason.test.ts` | ADAPTED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | `src/v0842_release_test.rs`, `src/github_copilot_oauth_test.rs`, provider-specific Rust modules/tests | Changed in v0.84.2 release range and covered by focused deterministic Rust evidence or documented adaptation. |
| `mistral-reasoning-mode.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `mistral-tool-schema.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `model-catalog-types.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `model-data-validation.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `models-runtime.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `node-http-proxy.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `oauth-auth.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `oauth-device-code.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `openai-codex-cache-affinity-e2e.test.ts` | LIVE UNEXECUTED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing deterministic parser/request/runtime unit tests; live credentials intentionally not run in CI | Upstream live/credential matrix retained as N/A for deterministic Rust CI. |
| `openai-codex-oauth.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `openai-codex-stream.test.ts` | ADAPTED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | `src/v0842_release_test.rs`, `src/github_copilot_oauth_test.rs`, provider-specific Rust modules/tests | Changed in v0.84.2 release range and covered by focused deterministic Rust evidence or documented adaptation. |
| `openai-completions-cache-control-format.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `openai-completions-empty-tools.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `openai-completions-prompt-cache.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `openai-completions-raw-stop-reason.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `openai-completions-reasoning-details.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `openai-completions-response-model.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `openai-completions-retry.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `openai-completions-thinking-as-text.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `openai-completions-thinking-token-budget.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `openai-completions-tool-choice.test.ts` | ADAPTED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | `src/v0842_release_test.rs`, `src/github_copilot_oauth_test.rs`, provider-specific Rust modules/tests | Changed in v0.84.2 release range and covered by focused deterministic Rust evidence or documented adaptation. |
| `openai-completions-tool-result-images.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `openai-responses-cache-affinity-e2e.test.ts` | LIVE UNEXECUTED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing deterministic parser/request/runtime unit tests; live credentials intentionally not run in CI | Upstream live/credential matrix retained as N/A for deterministic Rust CI. |
| `openai-responses-compat.test.ts` | ADAPTED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | `src/v0842_release_test.rs`, `src/github_copilot_oauth_test.rs`, provider-specific Rust modules/tests | Changed in v0.84.2 release range and covered by focused deterministic Rust evidence or documented adaptation. |
| `openai-responses-empty-tool-result.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `openai-responses-foreign-toolcall-id.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `openai-responses-message-id.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `openai-responses-namespace.test.ts` | ADAPTED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | `src/v0842_release_test.rs`, `src/github_copilot_oauth_test.rs`, provider-specific Rust modules/tests | Changed in v0.84.2 release range and covered by focused deterministic Rust evidence or documented adaptation. |
| `openai-responses-partial-json-cleanup.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `openai-responses-reasoning-replay-e2e.test.ts` | LIVE UNEXECUTED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing deterministic parser/request/runtime unit tests; live credentials intentionally not run in CI | Upstream live/credential matrix retained as N/A for deterministic Rust CI. |
| `openai-responses-terminal-event.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `openai-responses-tool-result-images.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `openrouter-cache-control-models.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `openrouter-cache-write-repro.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `openrouter-images.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `openrouter-oauth.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `overflow.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `pi-messages.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `provider-error-body-passthrough.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `provider-error-body-regression.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `provider-retry.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `providers.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `qwen-token-plan-models.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `radius-oauth.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `reasoning-options.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `responseid.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `retry.test.ts` | ADAPTED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | `src/v0842_release_test.rs`, `src/github_copilot_oauth_test.rs`, provider-specific Rust modules/tests | Changed in v0.84.2 release range and covered by focused deterministic Rust evidence or documented adaptation. |
| `sampling-options.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `stream.test.ts` | ADAPTED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | `src/v0842_release_test.rs`, `src/github_copilot_oauth_test.rs`, provider-specific Rust modules/tests | Changed in v0.84.2 release range and covered by focused deterministic Rust evidence or documented adaptation. |
| `supports-xhigh.test.ts` | ADAPTED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | `src/v0842_release_test.rs`, `src/github_copilot_oauth_test.rs`, provider-specific Rust modules/tests | Changed in v0.84.2 release range and covered by focused deterministic Rust evidence or documented adaptation. |
| `telemetry-options.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `text.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `together-models.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `tokens.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `tool-call-id-normalization.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `tool-call-without-result.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `total-tokens.test.ts` | ADAPTED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | `src/v0842_release_test.rs`, `src/github_copilot_oauth_test.rs`, provider-specific Rust modules/tests | Changed in v0.84.2 release range and covered by focused deterministic Rust evidence or documented adaptation. |
| `transform-messages-copilot-openai-to-anthropic.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `unicode-surrogate.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `uuid.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `validation.test.ts` | ADAPTED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | `src/v0842_release_test.rs`, `src/github_copilot_oauth_test.rs`, provider-specific Rust modules/tests | Changed in v0.84.2 release range and covered by focused deterministic Rust evidence or documented adaptation. |
| `xai-oauth.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `xai-responses.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `xhigh.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `xiaomi-models.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |
| `xiaomi-token-plan-ams-anthropic-empty-signature-smoke.test.ts` | LIVE UNEXECUTED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing deterministic parser/request/runtime unit tests; live credentials intentionally not run in CI | Upstream live/credential matrix retained as N/A for deterministic Rust CI. |
| `zen.test.ts` | COVERED | v0.84.2 tag `914cf1472e715297caa30db4b9535d534a9eb718` | Existing Rust parity suite | Unchanged in v0.84.2 and covered by prior accepted parity tests. |

## Full upstream filename inventory

1. `abort.test.ts`
2. `anthropic-adaptive-thinking-models.test.ts`
3. `anthropic-auth-token.test.ts`
4. `anthropic-cache-write-1h-cost.test.ts`
5. `anthropic-eager-tool-input-compat.test.ts`
6. `anthropic-eager-tool-input-e2e.test.ts`
7. `anthropic-empty-thinking-signature-compat.test.ts`
8. `anthropic-force-adaptive-thinking.test.ts`
9. `anthropic-long-cache-retention-e2e.test.ts`
10. `anthropic-oauth.test.ts`
11. `anthropic-opus-4-8-smoke.test.ts`
12. `anthropic-sse-parsing.test.ts`
13. `anthropic-temperature-compat.test.ts`
14. `anthropic-thinking-disable.test.ts`
15. `anthropic-tool-name-normalization.test.ts`
16. `azure-openai-base-url.test.ts`
17. `azure-openai-responses-reasoning-replay.test.ts`
18. `baseten-models.test.ts`
19. `bedrock-convert-messages.test.ts`
20. `bedrock-credentials.test.ts`
21. `bedrock-custom-headers.test.ts`
22. `bedrock-endpoint-resolution.test.ts`
23. `bedrock-error-metadata.test.ts`
24. `bedrock-models.test.ts`
25. `bedrock-raw-stop-reason.test.ts`
26. `bedrock-thinking-payload.test.ts`
27. `cache-retention.test.ts`
28. `cloudflare-gateway-binding.test.ts`
29. `cloudflare-stream.test.ts`
30. `compat-env.test.ts`
31. `constrained-sampling.test.ts`
32. `context-estimate.test.ts`
33. `context-overflow.test.ts`
34. `cross-provider-handoff.test.ts`
35. `deferred-tools.test.ts`
36. `empty.test.ts`
37. `env-api-keys.test.ts`
38. `error-body.test.ts`
39. `faux-provider.test.ts`
40. `fetch-option.test.ts`
41. `fireworks-models.test.ts`
42. `generate-models-strict.test.ts`
43. `github-copilot-anthropic.test.ts`
44. `github-copilot-oauth.test.ts`
45. `google-raw-stop-reason.test.ts`
46. `google-shared-convert-tools.test.ts`
47. `google-shared-gemini3-unsigned-tool-call.test.ts`
48. `google-shared-image-tool-result-routing.test.ts`
49. `google-shared-retry.test.ts`
50. `google-shared-signed-empty-blocks.test.ts`
51. `google-thinking-disable.test.ts`
52. `google-thinking-signature.test.ts`
53. `google-vertex-api-key-resolution.test.ts`
54. `image-model-data.test.ts`
55. `image-tool-result.test.ts`
56. `images-models.test.ts`
57. `images.test.ts`
58. `interleaved-thinking.test.ts`
59. `kimi-coding-oauth.test.ts`
60. `lax-message-content.test.ts`
61. `lazy-module-load.test.ts`
62. `max-thinking.test.ts`
63. `mistral-http-transport.test.ts`
64. `mistral-raw-stop-reason.test.ts`
65. `mistral-reasoning-mode.test.ts`
66. `mistral-tool-schema.test.ts`
67. `model-catalog-types.test.ts`
68. `model-data-validation.test.ts`
69. `models-runtime.test.ts`
70. `node-http-proxy.test.ts`
71. `oauth-auth.test.ts`
72. `oauth-device-code.test.ts`
73. `openai-codex-cache-affinity-e2e.test.ts`
74. `openai-codex-oauth.test.ts`
75. `openai-codex-stream.test.ts`
76. `openai-completions-cache-control-format.test.ts`
77. `openai-completions-empty-tools.test.ts`
78. `openai-completions-prompt-cache.test.ts`
79. `openai-completions-raw-stop-reason.test.ts`
80. `openai-completions-reasoning-details.test.ts`
81. `openai-completions-response-model.test.ts`
82. `openai-completions-retry.test.ts`
83. `openai-completions-thinking-as-text.test.ts`
84. `openai-completions-thinking-token-budget.test.ts`
85. `openai-completions-tool-choice.test.ts`
86. `openai-completions-tool-result-images.test.ts`
87. `openai-responses-cache-affinity-e2e.test.ts`
88. `openai-responses-compat.test.ts`
89. `openai-responses-empty-tool-result.test.ts`
90. `openai-responses-foreign-toolcall-id.test.ts`
91. `openai-responses-message-id.test.ts`
92. `openai-responses-namespace.test.ts`
93. `openai-responses-partial-json-cleanup.test.ts`
94. `openai-responses-reasoning-replay-e2e.test.ts`
95. `openai-responses-terminal-event.test.ts`
96. `openai-responses-tool-result-images.test.ts`
97. `openrouter-cache-control-models.test.ts`
98. `openrouter-cache-write-repro.test.ts`
99. `openrouter-images.test.ts`
100. `openrouter-oauth.test.ts`
101. `overflow.test.ts`
102. `pi-messages.test.ts`
103. `provider-error-body-passthrough.test.ts`
104. `provider-error-body-regression.test.ts`
105. `provider-retry.test.ts`
106. `providers.test.ts`
107. `qwen-token-plan-models.test.ts`
108. `radius-oauth.test.ts`
109. `reasoning-options.test.ts`
110. `responseid.test.ts`
111. `retry.test.ts`
112. `sampling-options.test.ts`
113. `stream.test.ts`
114. `supports-xhigh.test.ts`
115. `telemetry-options.test.ts`
116. `text.test.ts`
117. `together-models.test.ts`
118. `tokens.test.ts`
119. `tool-call-id-normalization.test.ts`
120. `tool-call-without-result.test.ts`
121. `total-tokens.test.ts`
122. `transform-messages-copilot-openai-to-anthropic.test.ts`
123. `unicode-surrogate.test.ts`
124. `uuid.test.ts`
125. `validation.test.ts`
126. `xai-oauth.test.ts`
127. `xai-responses.test.ts`
128. `xhigh.test.ts`
129. `xiaomi-models.test.ts`
130. `xiaomi-token-plan-ams-anthropic-empty-signature-smoke.test.ts`
131. `zen.test.ts`
