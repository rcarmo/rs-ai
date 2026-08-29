# v0.84.4 cumulative 137-file test crosswalk

Source: official upstream `@earendil-works/pi-ai` tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` (`v0.84.4`). Baseline: accepted `v0.84.3` tag `4e58f324fae8ebfa98a3d45181fb248072a2afac`.

Total upstream `packages/ai/test/*.test.ts` files: **137**.

## Release-delta test accounting

The bounded v0.84.4 range changes **6 test paths total**:

- **5 existing tests modified**: `fireworks-models`, `mistral-http-transport`, `openai-completions-reasoning-details`, `openai-completions-tool-choice`, `zai-coding-plan-models`.
- **1 new test added**: `openrouter-reasoning-options.test.ts`.

## Key v0.84.4 dispositions

| Upstream file/group | Disposition | rs-ai evidence | Notes |
|---|---|---|---|
| `openai-completions-reasoning-details.test.ts` | ADAPTED | `src/openai_completions_reasoning_details_test.rs::merges_adjacent_text_and_summary_reasoning_details_before_replay`, `src/provider/openai.rs` | Buffered streamed `reasoning_details` merge adjacent text/summary chunks before replay; encrypted detail fallback remains single-copy. |
| `openai-completions-tool-choice.test.ts` | ADAPTED | `src/v0844_release_test.rs::tool_choice_none_serializes_without_tools` | `tool_choice` is forwarded even when the request has no tools. |
| `mistral-http-transport.test.ts` | ADAPTED | `src/v0844_release_test.rs::mistral_indexed_tool_call_fragments_merge_without_repeated_ids_or_names` | Tool-call chunks are keyed by stream `index`, so later fragments without `id`/name still append to the original call. |
| `openrouter-reasoning-options.test.ts`, `scripts/openrouter-reasoning-options.ts`, `scripts/generate-models.ts` | ADAPTED | `src/v0844_release_test.rs::openrouter_mandatory_and_optional_reasoning_payloads_match_v0844`, regenerated `src/models_generated.rs` | OpenRouter reasoning metadata now controls `thinkingLevelMap`; mandatory models omit reasoning for background calls while optional models can explicitly disable it. |
| `cloudflare-ai-gateway.ts`, generated model shards | ADAPTED | `src/v0844_release_test.rs::cloudflare_workers_ai_models_are_mirrored_into_gateway_compat_catalog`, metadata verifier | Workers AI models are mirrored into Cloudflare AI Gateway `/compat` catalog without duplicate ids. |
| `fireworks-models.test.ts`, `zai-coding-plan-models.test.ts`, image/text generated catalogs | ADAPTED | `src/fireworks_models_test.rs::omits_removed_fire_pass_turbo_router_models`, `src/v0844_release_test.rs::{release_pinned_catalog_counts_match_v0844,zai_coding_plan_glm_5_3_cost_matches_v0844}` | Text catalog matches 1290 pairs/39 providers/9 APIs/40 batch aliases; image catalog matches 50 pairs; Fireworks turbo routers are absent; ZAI GLM 5.3 costs match v0.84.4. |
| Package/docs/TS-only exports | N/A / DOCUMENTED | `RELEASE.md`, this crosswalk | Changelog/package metadata and TypeScript-only comments/typing are documented rather than executed in Rust CI. |

## Per-file 137-test disposition matrix

Executable validator: `python3 scripts/validate_v0844_manifests.py` asserts these **137** unique rows against the upstream tag inventory.

| Upstream test file | Disposition | Source | rs-ai evidence | Notes |
|---|---|---|---|---|
| `abort.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `anthropic-adaptive-thinking-models.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `anthropic-auth-token.test.ts` | ADAPTED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | `src/v0843_release_test.rs`, provider/catalog-specific Rust tests | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `anthropic-cache-write-1h-cost.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `anthropic-eager-tool-input-compat.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `anthropic-eager-tool-input-e2e.test.ts` | LIVE UNEXECUTED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing deterministic parser/request/runtime unit tests; live credentials intentionally not run in CI | Upstream live/credential matrix retained as N/A for deterministic Rust CI. |
| `anthropic-empty-thinking-signature-compat.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `anthropic-force-adaptive-thinking.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `anthropic-long-cache-retention-e2e.test.ts` | LIVE UNEXECUTED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing deterministic parser/request/runtime unit tests; live credentials intentionally not run in CI | Upstream live/credential matrix retained as N/A for deterministic Rust CI. |
| `anthropic-oauth.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `anthropic-opus-4-8-smoke.test.ts` | LIVE UNEXECUTED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing deterministic parser/request/runtime unit tests; live credentials intentionally not run in CI | Upstream live/credential matrix retained as N/A for deterministic Rust CI. |
| `anthropic-sse-parsing.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `anthropic-temperature-compat.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `anthropic-thinking-disable.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `anthropic-tool-name-normalization.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `azure-openai-base-url.test.ts` | ADAPTED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | `src/v0843_release_test.rs`, provider/catalog-specific Rust tests | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `azure-openai-responses-reasoning-replay.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `azure-openai-tool-choice.test.ts` | ADAPTED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | `src/v0843_release_test.rs`, provider/catalog-specific Rust tests | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `baseten-models.test.ts` | ADAPTED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | `src/v0843_release_test.rs`, provider/catalog-specific Rust tests | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `bedrock-convert-messages.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `bedrock-credentials.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `bedrock-custom-headers.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `bedrock-endpoint-resolution.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `bedrock-error-metadata.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `bedrock-models.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `bedrock-raw-stop-reason.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `bedrock-redacted-reasoning.test.ts` | ADAPTED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | `src/v0843_release_test.rs`, provider/catalog-specific Rust tests | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `bedrock-response-headers.test.ts` | ADAPTED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | `src/v0843_release_test.rs`, provider/catalog-specific Rust tests | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `bedrock-thinking-payload.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `cache-retention.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `cloudflare-gateway-binding.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `cloudflare-stream.test.ts` | LIVE UNEXECUTED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing deterministic parser/request/runtime unit tests; live credentials intentionally not run in CI | Upstream live/credential matrix retained as N/A for deterministic Rust CI. |
| `compat-env.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `constrained-sampling.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `context-estimate.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `context-overflow.test.ts` | LIVE UNEXECUTED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing deterministic parser/request/runtime unit tests; live credentials intentionally not run in CI | Upstream live/credential matrix retained as N/A for deterministic Rust CI. |
| `cross-provider-handoff.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `deferred-tools.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `empty.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `env-api-keys.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `error-body.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `faux-provider.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `fetch-option.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `fireworks-models.test.ts` | ADAPTED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | `src/fireworks_models_test.rs::omits_removed_fire_pass_turbo_router_models` | v0.84.4 removes Fireworks Fire Pass turbo routers; Rust catalog assertion now requires zero remaining `accounts/fireworks/routers/*-turbo` entries. |
| `generate-models-strict.test.ts` | ADAPTED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | `src/v0843_release_test.rs`, provider/catalog-specific Rust tests | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `github-copilot-anthropic.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `github-copilot-oauth.test.ts` | ADAPTED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | `src/v0843_release_test.rs`, provider/catalog-specific Rust tests | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `google-raw-stop-reason.test.ts` | ADAPTED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | `src/v0843_release_test.rs`, provider/catalog-specific Rust tests | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `google-shared-convert-tools.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `google-shared-gemini3-unsigned-tool-call.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `google-shared-image-tool-result-routing.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `google-shared-retry.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `google-shared-signed-empty-blocks.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `google-thinking-disable.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `google-thinking-level-map.test.ts` | ADAPTED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | `src/v0843_release_test.rs`, provider/catalog-specific Rust tests | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `google-thinking-signature.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `google-vertex-api-key-resolution.test.ts` | ADAPTED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | `src/v0843_release_test.rs`, provider/catalog-specific Rust tests | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `image-model-data.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `image-tool-result.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `images-models.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `images.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `interleaved-thinking.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `kimi-coding-oauth.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `lax-message-content.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `lazy-module-load.test.ts` | LIVE UNEXECUTED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing deterministic parser/request/runtime unit tests; live credentials intentionally not run in CI | Upstream live/credential matrix retained as N/A for deterministic Rust CI. |
| `max-thinking.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `mistral-http-transport.test.ts` | ADAPTED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | `src/v0844_release_test.rs::mistral_indexed_tool_call_fragments_merge_without_repeated_ids_or_names` | Fragmented indexed Mistral tool-call SSE chunks are merged by `index` even when later fragments omit `id` and carry an empty `function.name`. |
| `mistral-raw-stop-reason.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `mistral-reasoning-mode.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `mistral-tool-schema.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `model-catalog-types.test.ts` | ADAPTED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | `src/v0843_release_test.rs`, provider/catalog-specific Rust tests | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `model-data-validation.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `models-runtime.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `node-http-proxy.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `oauth-auth.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `oauth-device-code.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `openai-codex-cache-affinity-e2e.test.ts` | LIVE UNEXECUTED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing deterministic parser/request/runtime unit tests; live credentials intentionally not run in CI | Upstream live/credential matrix retained as N/A for deterministic Rust CI. |
| `openai-codex-oauth.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `openai-codex-stream.test.ts` | LIVE UNEXECUTED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing deterministic parser/request/runtime unit tests; live credentials intentionally not run in CI | Upstream live/credential matrix retained as N/A for deterministic Rust CI. |
| `openai-completions-cache-control-format.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `openai-completions-empty-tools.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `openai-completions-prompt-cache.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `openai-completions-raw-stop-reason.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `openai-completions-reasoning-details.test.ts` | ADAPTED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | `src/openai_completions_reasoning_details_test.rs::merges_adjacent_text_and_summary_reasoning_details_before_replay` | Consecutive `reasoning.text` and `reasoning.summary` deltas are merged into one replay signature array, with encrypted details preserved once. |
| `openai-completions-response-model.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `openai-completions-retry.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `openai-completions-thinking-as-text.test.ts` | ADAPTED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | `src/v0843_release_test.rs`, provider/catalog-specific Rust tests | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `openai-completions-thinking-token-budget.test.ts` | ADAPTED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | `src/v0843_release_test.rs`, provider/catalog-specific Rust tests | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `openai-completions-tool-choice.test.ts` | ADAPTED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | `src/v0844_release_test.rs::tool_choice_none_serializes_without_tools` | Provider-neutral `tool_choice: "none"` is serialized even when no tools are present, and no empty `tools` array is emitted. |
| `openai-completions-tool-result-images.test.ts` | ADAPTED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | `src/v0843_release_test.rs`, provider/catalog-specific Rust tests | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `openai-responses-cache-affinity-e2e.test.ts` | LIVE UNEXECUTED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing deterministic parser/request/runtime unit tests; live credentials intentionally not run in CI | Upstream live/credential matrix retained as N/A for deterministic Rust CI. |
| `openai-responses-compat.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `openai-responses-empty-tool-result.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `openai-responses-foreign-toolcall-id.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `openai-responses-message-id.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `openai-responses-namespace.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `openai-responses-partial-json-cleanup.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `openai-responses-reasoning-replay-e2e.test.ts` | LIVE UNEXECUTED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing deterministic parser/request/runtime unit tests; live credentials intentionally not run in CI | Upstream live/credential matrix retained as N/A for deterministic Rust CI. |
| `openai-responses-terminal-event.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `openai-responses-tool-result-images.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `openrouter-cache-control-models.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `openrouter-cache-write-repro.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `openrouter-images.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `openrouter-oauth.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `openrouter-reasoning-options.test.ts` | ADAPTED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | `src/v0844_release_test.rs::openrouter_mandatory_and_optional_reasoning_payloads_match_v0844` | OpenRouter mandatory reasoning maps unavailable/off efforts to omitted background-call reasoning, while optional models still send `{ effort: "none" }`. |
| `overflow.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `pi-messages.test.ts` | ADAPTED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | `src/v0843_release_test.rs`, provider/catalog-specific Rust tests | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `provider-error-body-passthrough.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `provider-error-body-regression.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `provider-retry.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `providers.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `qwen-token-plan-models.test.ts` | ADAPTED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | `src/v0843_release_test.rs`, provider/catalog-specific Rust tests | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `radius-oauth.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `reasoning-options.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `responseid.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `retry.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `sampling-options.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `stream.test.ts` | ADAPTED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | `src/v0843_release_test.rs`, provider/catalog-specific Rust tests | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `supports-xhigh.test.ts` | ADAPTED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | `src/v0843_release_test.rs`, provider/catalog-specific Rust tests | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `telemetry-options.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `text.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `together-models.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `tokens.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `tool-call-id-normalization.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `tool-call-without-result.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `total-tokens.test.ts` | LIVE UNEXECUTED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing deterministic parser/request/runtime unit tests; live credentials intentionally not run in CI | Upstream live/credential matrix retained as N/A for deterministic Rust CI. |
| `transform-messages-copilot-openai-to-anthropic.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `unicode-surrogate.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `uuid.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `validation.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `xai-oauth.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `xai-responses.test.ts` | ADAPTED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | `src/v0843_release_test.rs`, provider/catalog-specific Rust tests | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `xhigh.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `xiaomi-models.test.ts` | ADAPTED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | `src/v0843_release_test.rs`, provider/catalog-specific Rust tests | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
| `xiaomi-token-plan-ams-anthropic-empty-signature-smoke.test.ts` | LIVE UNEXECUTED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing deterministic parser/request/runtime unit tests; live credentials intentionally not run in CI | Upstream live/credential matrix retained as N/A for deterministic Rust CI. |
| `zai-coding-plan-models.test.ts` | ADAPTED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | `src/v0844_release_test.rs::zai_coding_plan_glm_5_3_cost_matches_v0844` | Regenerated catalog carries the v0.84.4 `glm-5.3` input/output/cacheRead pricing for ZAI Coding Plan CN. |
| `zen.test.ts` | COVERED | v0.84.4 tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` | Existing Rust parity suite | Unchanged from v0.84.3 and covered by prior accepted parity tests. |
