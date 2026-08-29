# Shared local-test adaptation tracker (rs-ai)

## v0.84.4 bounded release evidence

Source: upstream tag `b79e4cc834970cca69daebffab7df1da7d1e52c4` (`v0.84.4`), exact release-only delta `4e58f324fae8ebfa98a3d45181fb248072a2afac..b79e4cc834970cca69daebffab7df1da7d1e52c4`.

Status: **ADAPTED / completed for bounded deterministic v0.84.4 release parity**. The v0.84.4 audit spans 15 changed `packages/ai` paths and **6 changed test paths total**: 5 existing tests modified plus 1 new `openrouter-reasoning-options.test.ts`. The final upstream test corpus is **137 files**, recorded in `docs/v0844-137-test-crosswalk.md` and mechanically checked by `scripts/validate_v0844_manifests.py`.

Named tests in `src/v0844_release_test.rs` cover v0.84.4 catalog counts, provider-neutral `tool_choice` with no tools, OpenRouter mandatory/optional reasoning payload semantics, Cloudflare AI Gateway Workers AI mirror dedupe, ZAI Coding Plan `glm-5.3` pricing, and Mistral fragmented indexed tool-call chunk merging. `src/openai_completions_reasoning_details_test.rs::merges_adjacent_text_and_summary_reasoning_details_before_replay` covers the v0.84.4 OpenAI-compatible `reasoning_details` merge/replay delta, and `src/fireworks_models_test.rs::omits_removed_fire_pass_turbo_router_models` covers the Fireworks catalog removal. Metadata verifier targets pinned tarball SHA `dfd3c929cee5a7387199a0a24dfc1be2096f1ea8f59ffb8285198a0ed01ebf93` and reports text `1290`, providers `39`, APIs `9`, batch aliases `40`, image `50`.

## v0.84.3 bounded release evidence

Source: upstream tag `4e58f324fae8ebfa98a3d45181fb248072a2afac` (`v0.84.3`), exact release-only delta `914cf1472e715297caa30db4b9535d534a9eb718..4e58f324fae8ebfa98a3d45181fb248072a2afac`.

Status: **ADAPTED / completed for bounded deterministic v0.84.3 release parity**. The v0.84.3 audit spans 48 changed `packages/ai` paths and **25 changed test paths total**: 20 existing tests modified plus 5 new tests. The final upstream test corpus is **136 files**, recorded in `docs/v0843-136-test-crosswalk.md` and mechanically checked by `scripts/validate_v0843_manifests.py`.

Named tests in `src/v0843_release_test.rs` cover v0.84.3 catalog counts, Azure Responses provider-neutral `tool_choice`, default/override Pi User-Agent behavior, and Google thinking-level map/error/budget semantics. Additional v0.84.3 evidence is in `src/anthropic_fallback_test.rs`, `src/bedrock_thinking_payload_test.rs`, `src/bedrock_error_metadata_test.rs`, `src/openai_completions_reasoning_details_test.rs`, `src/github_copilot_oauth_test.rs`, and `src/xai_grok45_responses_test.rs`. Metadata verifier targets pinned tarball SHA `9c40af2f43950f8e94e7bbcd0c1b3548f000972da00c4fb9c0d0529d4d7d5431` and reports text `1312`, providers `39`, APIs `9`, batch aliases `60`, image `45`.

## v0.84.2 bounded release evidence

Source: upstream tag `914cf1472e715297caa30db4b9535d534a9eb718` (`v0.84.2`), exact release-only delta `53fa77ccd8a279eb87e92294ef3687b03ff80112..914cf1472e715297caa30db4b9535d534a9eb718`.

Status: **ADAPTED / completed pending hosted CI confirmation**. The v0.84.2 audit spans 42 changed `packages/ai` paths and **21 changed test paths total**: 18 existing tests modified plus 3 new tests. The final upstream test corpus is **131 files**, recorded in `docs/v0842-131-test-crosswalk.md`.

Named tests in `src/v0842_release_test.rs` cover v0.84.2 catalog counts, strict JSON-schema conversion, optional-null omission, DeepSeek detection/max_tokens, retry buffer exhaustion, Kimi/Codex user-agent helper, Responses additional_tools/namespace replay, and Mistral HTTP/SSE production-path behavior (delayed chunks, split UTF-8, cancellation/drop cleanup, timeout, bounded 403 body, retry replay, affinity, exact wire payload). `src/github_copilot_oauth_test.rs` covers Copilot Individual policy fallback and bounded policy-enable batching (`COPILOT_POLICY_CONCURRENCY = 4`). `scripts/validate_v0842_manifests.py` is the executable 42-path/131-row manifest validator. Metadata verifier now targets the v0.84.2 pinned tarball SHA `0262785a76b0eb2eec596cd8a7ab2ee23eef89d2ef1bb1211c4f0a1944dacf41` and reports text `1267`, providers `39`, APIs `9`, batch aliases `60`, image `45`.

## v0.84.1 bounded release evidence

Source: upstream tag `53fa77ccd8a279eb87e92294ef3687b03ff80112` (`v0.84.1`), exact release-only delta `a5f43bf8aff3c55752432655f7334e3dafd1e256..53fa77ccd8a279eb87e92294ef3687b03ff80112`.

Status: **ADAPTED / under final evidence-correction verification**. The v0.84.1 audit spans 25 changed `packages/ai` paths and **14 changed test paths total**: 13 existing tests modified plus 1 new `generate-models-strict.test.ts`. Exact split: **10 credential-gated live-matrix additions**, **2 deterministic provider/request rows**, and **2 generator-policy rows**. The final upstream test corpus is **128 files**, recorded in `docs/v0841-128-test-crosswalk.md`.

Named tests in `src/v0841_release_test.rs`:

- `release_pinned_catalog_counts_include_individual_and_batch_aliases`
- `qwen_token_plan_individual_catalog_env_and_endpoint_match_v0841`
- `qwen_token_plan_individual_reasoning_payloads_match_v0841`

Model-data/extractor/metadata evidence:

- `src/model_data_validation_test.rs::extractor_enforces_qwen_individual_strict_model_ids_without_output_mutation`
- `src/model_data_validation_test.rs::extractor_allows_only_audited_release_batch_aliases`
- `scripts/verify_release_model_metadata.py` full clean-run text/image metadata regeneration equality gate with SHA-256-pinned npm tarball (`6ab689189e7cb3de5cdb126312a3e60e8ac35fe5ee5f1b63d00f711c8a430c73`)
- `src/release_metadata_verification_test.rs::release_metadata_verifier_clean_run_succeeds_with_expected_counts`
- `src/release_metadata_verification_test.rs::release_metadata_verifier_detects_fault_injected_text_metadata`

Ported/adapted behavior:

- Built-in `qwen-token-plan-individual` provider with shared `QWEN_TOKEN_PLAN_API_KEY`, shared international Token Plan endpoint, and exact 7-model Individual allowlist.
- OpenAI-compatible Qwen request shape now emits `enable_thinking` plus supported `reasoning_effort` for `thinkingFormat = "qwen"`, with no top-level `thinking` object.
- Release-pinned catalog extraction now accepts only the exact 59 audited OpenRouter `:batch` aliases present in the official v0.84.1 npm shards and rejects any unaudited batch alias.
- Strict Individual allowlist validation fails before writing output when the release-shard model IDs drift, mirroring upstream `generate-models-strict.test.ts` rollback intent.

Catalog evidence: `python3 scripts/validate_release_model_data.py /workspace/tmp/pi-ai-0841-pkg/package/dist/providers/data` => `1220` models, `39` providers, structure hash `24c74ac10bb8ed4df2c96bdadcfd94a417f3c823d5038875f59a261e3c84424b`; `PI_AI_MODEL_DATA_DIR=/workspace/tmp/pi-v0841-json python3 scripts/compare_upstream_registry_pairs.py /workspace/tmp/pi-src 53fa77ccd8a279eb87e92294ef3687b03ff80112` => text `1220/1220`, image `42/42`, missing `0`, extra `0`; `python3 scripts/verify_release_model_metadata.py` => SHA-pinned tarball verification plus full regenerated text/image metadata equality after timestamp normalization and rustfmt (`metadata verified: text=1220 providers=39 apis=9 batchAliases=59 image=42`).

## v0.84.0 completed bounded release evidence

Source: upstream tag `a5f43bf8aff3c55752432655f7334e3dafd1e256` (`v0.84.0`), exact release-only delta `845d6ff1f6643aba440341cce877ce1c43ebbc39..a5f43bf8aff3c55752432655f7334e3dafd1e256`.

Status: **ADAPTED / COMPLETED for deterministic release parity**. The v0.84.0 audit spans 101 changed `packages/ai` paths and 46 changed upstream test files; deterministic assertions are ported/adapted, covered, or explicitly N/A for live credentials/interactive UI/JS-runtime-only surfaces.

Named tests in `src/v0840_release_test.rs`:

- `sampling_params_merge_and_override_openai_compatible_payloads`
- `baseten_catalog_and_reasoning_payload_match_v0840`
- `vllm_thinking_token_budget_edge_matrix`
- `nullable_anyof_oneof_preserves_matching_null_before_coercion`
- `supports_finish_reason_false_infers_terminal_stop_or_tool_use`

Generated audit manifests: `docs/v0840-manifests.md` (101 changed `packages/ai` paths; 46 changed tests with assertion/gate extraction) and `docs/v0840-127-test-crosswalk.md` (127/127 upstream test filenames accounted, including the 8 newly explicit whole-corpus dispositions).

Slice gate results: comparator text `1153/1153`, image `42/42`; `cargo build`; `cargo test v0840_release_test`; strict Clippy.



Catalog correction fixture: `src/v0840_release_test.rs::release_pinned_catalog_has_no_unpinned_batch_aliases` asserts the corrected official-release catalog counts: text pairs `1153`, providers `38`, APIs `9`, image pairs `42`, and no unpinned `:batch` aliases. The extractor `scripts/extract_release_model_shards.py` records npm provider-shard source hashes and rejects `:batch` drift.

Second committed v0.84.0 slice: upstream `382aa641` deferred/background response lifecycle is now **PORTED**. Evidence: `DeferredHandle`, `StopReason::Deferred`, `Message.deferred`, public `fetch_deferred`/`cancel_deferred` provider dispatch, and faux submit/pending/ready/cancel/unknown-handle executable tests in `src/providers_upstream_test.rs`. Full `cargo test` for this slice: `836 passed`; doctest `1 passed`; strict Clippy/build/comparator clean.

Final v0.84.0 closure evidence now also includes provider stream/error regressions, runtime/OAuth/telemetry dispatch semantics, Codex account+session WebSocket fallback isolation, Bedrock status/requestId diagnostics, and Google shared retry at the real `stream_google` call site. Named final-gap evidence: `src/google_shared_retry_test.rs` (429 retry once with explicit budget, no retry when unset, no retry for 400, delay cap, caller cancellation). Whole-corpus hardening evidence adds `src/cloudflare_stream_test.rs`, `src/image_model_data_test.rs`, `src/model_data_validation_test.rs`, `src/openrouter_cache_control_models_test.rs`, `src/provider_retry_upstream_test.rs`, `src/reasoning_options_test.rs`, `src/uuid_test.rs`, and expanded `src/xai_grok45_responses_test.rs`; image/reasoning tests call production generator helpers and model-data tests call the production release-shard validator.


Mirror of the cross-port requirement: adapt locally-authored regression/edge-case
tests from the reference ports — primarily **@go-ai** (`/workspace/projects/go-ai`,
`docs/local-tests-shared.md`, 188 local Go tests) — into idiomatic Rust. This file
tracks adaptation status; upstream 1:1 ports are tracked separately in
`docs/upstream-parity-gaps.md`.

Status legend: **ADAPTED** (ported to a named rs-ai test), **COVERED**
(behaviourally guarded by an existing rs-ai test), **PENDING**, **N/A**
(feature gated/architectural — see parity-gaps doc).

## Current upstream test-for-test fixture: deferred/message-anchored tools

Source: upstream main `packages/ai/test/deferred-tools.test.ts` at `0e6909f050eeb15e8f6c05185511f3788357ddb3` (`feat(ai): support message-anchored tool loading (#6474)`). rs-ai adaptation: `src/deferred_tools_test.rs` plus `src/deferred_tools.rs`.

Status: **ADAPTED**. The Rust tests cover the upstream branches for `addedToolNames`/`added_tool_names`, Anthropic `defer_loading`, `tool_reference`, sibling `tool_result` output preservation, OAuth Claude Code name canonicalization/deduplication, unsupported/all-marked/prior-use fallback cases, OpenAI Responses `tool_search_call`/`tool_search_output`, Codex support gating, OpenAI-compatible fallback behavior, and context-estimator accounting for deferred definitions.

This is an upstream parity fixture, not counted as a local-only conformance test.

## Current upstream test-for-test fixture: Azure Responses reasoning replay

Source: upstream main `packages/ai/test/azure-openai-responses-reasoning-replay.test.ts` at `0e6909f050eeb15e8f6c05185511f3788357ddb3`. rs-ai adaptation: `src/azure_openai_responses_reasoning_replay_test.rs`.

Status: **ADAPTED**. The two deterministic cases verify that completed-output `encrypted_content` backfill preserves `response.output_item.done` when it already included encrypted content, and uses terminal `response.completed.output` only when the done item omitted it.

## Current upstream registry provider/id comparison fixture

Status: **ADAPTED**. Run `scripts/compare_upstream_registry_pairs.py /workspace/tmp/pi-src 0e6909f050eeb15e8f6c05185511f3788357ddb3` to import upstream `MODELS`/`IMAGE_MODELS` with Bun, recursively flatten provider maps, and compare provider/id pairs against rs-ai. Expected for the agreed source: text `1057/1057`, image `35/35`, missing `0`, extra `0`.

## v0.83.0 release catalog/metadata/runtime disposition fixture

Source: upstream tag `845d6ff1f6643aba440341cce877ce1c43ebbc39` (`v0.83.0`), exact release-only delta `b4f293684bba718d59cc1157679bcf6157b3a7f5..845d6ff1f6643aba440341cce877ce1c43ebbc39`. rs-ai adaptation: regenerated `src/models_generated.rs`, added `Message.raw_stop_reason` provider capture, plus `src/v0830_release_test.rs`, and prior v0.82.x runtime work.

Status: **ADAPTED**. The v0.81.1→v0.82.0 release span changes 96 `packages/ai` paths. Catalog/generator/model-data validation, generated image/model JSON shape, image catalog additions, Qwen Token Plan providers/env, shared text/UUIDv7 utilities, retry abort-finished/provider retry semantics, overflow phrase matching, OpenCode Go Responses support, OpenAI-completions pipe-delimited tool-call ID uniqueness, Kimi/K3/xAI/OpenRouter/OpenCode/Gemini metadata, and related deterministic tests are represented by regenerated registries and Rust tests. README/changelog/package/TS build metadata and credential/browser OAuth matrix edits are N/A for Rust runtime. Expected comparator: `PI_AI_MODEL_DATA_DIR=/workspace/tmp/pi-v0830-json scripts/compare_upstream_registry_pairs.py /workspace/tmp/pi-v0830 845d6ff1f6643aba440341cce877ce1c43ebbc39` -> text `1153/1153`, image `40/40`, missing `0`, extra `0`.

## v0.80.9 runtime refresh, Radius dynamic catalog, and xAI OAuth fixture

Source: upstream tag `2d16f92973230a7e095aa984f150ba8702784f50` (`v0.80.9`), including prior `2be9efa19cd64aed40ca63f92c0c0f9a6bac7c9d` (`feat(ai): publish generated model catalogs to R2 (#6720)`) plus `5220aba6` (`feat(ai): add xAI device OAuth and route grok-4.5 through Responses (#6651)`). rs-ai adaptation: `src/models_runtime.rs`, `src/registry.rs`, `src/models_runtime_refresh_test.rs`, `src/oauth.rs`, `src/auth_providers.rs`, `src/xai_oauth_test.rs`, `src/xai_grok45_responses_test.rs`, and regenerated `src/models_generated.rs`.

Status: **ADAPTED**. Tests cover provider-scoped model-store entries, shared runtime-backed ordinary registry lookup/list paths, public runtime provider register/remove/refresh APIs, dynamic refresh replacement/removal, concurrent refresh dedupe, cancellation, `force` propagation, cache restore/offline retention, production Radius `/v1/config` provider refresh through `registry::refresh_runtime_models`, xAI device OAuth pending/slow_down/success/terminal/timeout/cancel/refresh behavior, `verification_uri_complete` validation/preference, and an actual `xai/grok-4.5` OpenAI Responses request. Comparator expectation for v0.80.9: text `1075/1075`, image `35/35`, missing `0`, extra `0`.

## Current upstream main OpenAI Codex session-id clamp fixture

Source: upstream main commit `dcfe36c79702ec240b146c45f167ab75ecddd205` (`clamp session-id to 64 chars for openai-codex (#6653)`). rs-ai adaptation: `src/provider/codex.rs` and `src/openai_codex_stream_test.rs`.

Status: **ADAPTED**. `clamps_prompt_cache_key_and_codex_session_headers_to_64_chars` verifies a long Codex `session_id` is clamped to the same 64-character value for `prompt_cache_key`, `session-id`, and `x-client-request-id` in the SSE path; the same clamp is applied to the WebSocket request id/session headers.

## v0.80.7 Radius OAuth helper fixture

Source: upstream `packages/ai/src/utils/oauth/radius.ts` at `818d67457cdd6b60bce6b121d16b23141c252dd8`. rs-ai adaptation: `src/oauth.rs`, `src/auth_providers.rs`, and `src/radius_oauth_test.rs`.

Status: **ADAPTED**. Local HTTP tests cover `/v1/oauth` discovery, PKCE authorization URL construction, authorization-code exchange, refresh through `RadiusOAuth`, device authorization request shape, `/v1/config` catalog loading/sanitization, transient config-failure fallback to cached catalog, API-key derivation, and gateway catalog model injection without duplicates. Browser launching/HTML callback rendering is treated as platform UI glue; the deterministic URL/exchange/resource-cleanup boundaries are exposed as Rust helpers.

## v0.80.3 conformance fixtures (for go-ai / swift-ai adoption)

Upstream **0.80.3** feature release. Authoritative constants + truth-tables so
all three ports use identical expected values. Source of truth:
`@earendil-works/pi-ai@0.80.3` `utils/{estimate,retry,error-body}.ts`,
`api/simple-options.ts`. rs-ai tests named per item.

### A. `estimateContextTokens` / text-token estimation

Constants: `CHARS_PER_TOKEN = 4`, `ESTIMATED_IMAGE_CHARS = 4800`.
`estimateTextTokens(s) = ceil(s.length / 4)`; an image block counts as 4800
chars (= 1200 tokens). rs-ai: `src/estimate.rs`, `src/estimate_test.rs`.

| case | input | expected |
|---|---|---|
| text ceil-div-4 | `"12345678"` (8) / `"123456789"` (9) / `""` | `2` / `3` / `0` |
| text+image content | `[text "abcd"(4), image]` = 4804 chars | `ceil(4804/4) = 1201` tokens |
| `calculateContextTokens` prefers total | usage `{in:10,out:20,total:99}` | `99` |
| …else sums in+out+cacheRead+cacheWrite | `{in:10,out:20,total:0,cR:5,cW:3}` | `38` |
| assistant msg = text+thinking+toolcall-JSON | `"hello"(5)+"think"(5)+(2+len('{"a":1}')=7)` = 19 chars | `ceil(19/4) = 5` |
| context anchors on last **non-aborted** assistant usage | sys `"sys"`, [user(ignored), assistant total=100, user `"abcd"`] | `tokens=101` (usage 100 + trailing 1; **no** system prefix when anchored), `lastUsageIndex=1` |
| no usage anchor → adds system prefix | sys `"12345678"`(2 tok), [assistant total=500 **aborted**, user `"abcd"`] | `tokens=4` (msgs 1+1 + sys 2; aborted usage skipped), `lastUsageIndex=None` |
| v0.80.6 stale usage after inserted prefix | sys `"system"`, [user `"summary"` ts=200, assistant usage=9500 ts=100, user `"x"*4000` ts=300] | `tokens=1005`, `usageTokens=0`, `trailingTokens=1005`, `lastUsageIndex=None`; context clamp for 10k/8k model → `maxTokens=4899` |
| v0.80.6 usage resumes after newer assistant | [user ts=200, stale assistant ts=100, user ts=300, assistant usage=2000 ts=400, user `"tail"` ts=500] | `tokens=2001`, `usageTokens=2000`, `trailingTokens=1`, `lastUsageIndex=3` |

### B. `clampMaxTokensToContext` boundary values

Constants: `CONTEXT_SAFETY_TOKENS = 4096`, `MIN_MAX_TOKENS = 1`. Formula:
```
if contextWindow <= 0:            return max(MIN_MAX_TOKENS, maxTokens)        # unknown window: floor only
used      = estimateContextTokens(context).tokens + CONTEXT_SAFETY_TOKENS
available = max(MIN_MAX_TOKENS, contextWindow - used)
return min(maxTokens, available)
```
rs-ai: `src/simple_options.rs`, `src/simple_options_test.rs`.

| case | inputs | expected |
|---|---|---|
| unknown window only floors | `contextWindow=0`, empty ctx, `maxTokens=5000` | `5000` |
| unknown window floors to MIN | `contextWindow=0`, `maxTokens=0` | `1` |
| room available, request fits | `contextWindow=200000`, small ctx, `maxTokens=8192` | `8192` (unchanged) |
| request exceeds available → clamp down | `contextWindow=10000`, `used` s.t. available `< maxTokens` | `available` |
| **canon request-path boundary** | `contextWindow=5000`, `"hello"` (2 tokens), `maxTokens=2000` | `902` (`5000-(2+4096)`) — asserted on the **anthropic** request `max_tokens` and the **bedrock** inferenceConfig resolver |
| available underflows → floor | `contextWindow - used < 1` | `1` |

_Wiring note (architectural, document per port): in upstream v0.80.3
`buildBaseOptions(model, context, options, apiKey)` clamps
`base.maxTokens = clamp(model, context, options?.maxTokens ?? model.maxTokens)`,
and `streamSimple` feeds that into the inner stream for **all** providers, so the
request param is defaulted + clamped for openai-completions, openai-responses
(+azure), google, vertex, mistral, anthropic and bedrock (anthropic/bedrock also
fold the thinking-budget re-fit `min(thinkingBudget, max(0, maxTokens - 1024))`).
codex sends no max-tokens field. rs-ai has no separate `streamSimple` layer, so
it folds default+clamp into every provider builder, matching each provider's
inner gate (openai/responses truthy → omit a clamped 0; google/mistral
`!== undefined` → always emit). The shared canon fixture (contextWindow=10000,
`"x"*8000`, maxTokens=8000 → **3904**) is asserted on the openai-completions wire
param (`openai_completions_empty_tools_test.rs`)._

### C. `Usage.reasoning` — `thinking_tokens` / `reasoning_tokens` mapping

New `Usage.reasoning` field. Capture per provider (all are a **subset of
output**, not added to it):

| provider / api | source field | absent → |
|---|---|---|
| anthropic-messages | `usage.output_tokens_details.thinking_tokens` (message_delta) | `None` (optional) |
| openai-completions | `usage.completion_tokens_details.reasoning_tokens` | `Some(0)` (`\|\| 0`) |
| openai-responses(+shared) | `usage.output_tokens_details.reasoning_tokens` | `Some(0)` (`\|\| 0`) |
| google-generative-ai / google-vertex | `usageMetadata.thoughtsTokenCount` | `Some(0)` (`\|\| 0`) |
| bedrock / mistral / codex | — (no upstream breakdown) | `None` |

Fixtures: anthropic delta `output_tokens_details.thinking_tokens=25` →
`reasoning=Some(25)`; openai `completion_tokens_details.reasoning_tokens=30` →
`Some(30)`, absent → `Some(0)`; responses `output_tokens_details.reasoning_tokens=12`
→ `Some(12)`. rs-ai: `simple_options_test.rs`, `anthropic_sse_parsing_test.rs`.

### D. `isRetryableAssistantError` truth-table

Gate: returns `false` unless `stopReason == "error"` **and** `errorMessage` is
non-empty. Then **non-retryable (quota/billing) wins** over retryable; checked
case-insensitively. rs-ai: `src/retry.rs`, `src/retry_classify_test.rs`.

| errorMessage (stopReason=error) | result |
|---|---|
| `"overloaded"` / `"429 Too Many Requests"` / `"rate limit"` / `"rate-limit"` / `"ratelimit"` | `true` |
| `"503 Service Unavailable"` / `"internal server error"` / `"Provider returned error"` | `true` |
| `"fetch failed"` / `"upstream connect error"` / `"socket hang up"` / `"connection refused"` | `true` |
| `"Request timed out"` / `"request timeout"` / `"WebSocket closed"` | `true` |
| `"stream ended before message_stop"` / `"ended without a stop reason"` / `"http2 request did not get a response"` / `"you can retry your request"` | `true` |
| `"GoUsageLimitError"` / `"FreeUsageLimitError"` / `"Monthly usage limit reached"` / `"available balance"` | `false` |
| `"insufficient_quota"` / `"out of budget"` / `"quota exceeded"` / `"billing issue"` | `false` |
| `"429 insufficient_quota: out of credits"` (both present) | `false` (non-retryable precedence) |
| `"invalid api key"` (unrelated) | `false` |
| any retryable text but `stopReason != error`, or empty message | `false` |

Pattern families (regex `.?` = optional single char between segments;
`timed? out` = `"timed out"`/`"time out"`): retryable =
`overloaded, rate.?limit, too many requests, 429/500/502/503/504,
service.?unavailable, server.?error, internal.?error, provider.?returned.?error,
network.?error, connection.?error/refused/lost, other side closed, fetch failed,
upstream.?connect, reset before headers, socket hang up, timed?out/timeout,
terminated, websocket.?closed/error, ended without,
stream ended before message_stop, http2 request did not get a response,
retry delay, you can/try/please retry…`. non-retryable =
`GoUsageLimitError, FreeUsageLimitError, monthly usage limit reached,
available balance, insufficient_quota, out of budget, quota exceeded, billing`.

### E. error-body normalization + truncation

`MAX_PROVIDER_ERROR_BODY_CHARS = 4000`. Body is **trimmed**; over-cap bodies get
`"<first 4000 chars>... [truncated N chars]"` (N = totalChars − 4000). Format:
`"{status}: {body}"`, or branded `"{prefix} ({status}): {body}"` for
openai-responses (`"OpenAI API error"`) / azure (`"Azure OpenAI API error"`).
Empty body → `"{status}"` (or `"{prefix} ({status})"`). rs-ai: `src/error_body.rs`,
`src/error_body_test.rs`.

| case | inputs | expected |
|---|---|---|
| no prefix | `403`, `{"error":"forbidden"}` | `403: {"error":"forbidden"}` |
| responses prefix | `429`, `rate limited` | `OpenAI API error (429): rate limited` |
| azure prefix | `500`, `boom` | `Azure OpenAI API error (500): boom` |
| trims body | `503`, `"   spaced   "` | `503: spaced` |
| empty body | `503`, `"   "` | `503` |
| empty + prefix | `503`, `""`, `OpenAI API error` | `OpenAI API error (503)` |
| truncation | `400`, `"x"*4025` | `400: ` + `"x"*4000` + `... [truncated 25 chars]` |

Consumers: openai-completions / openai-responses(+azure) / google(+vertex) /
openrouter-images use the format above. codex (plain-Error friendly message)
and bedrock (SDK already folds body via `messageCarriesBody`) are **no-ops**;
anthropic + mistral are **not** error-body consumers.

## Shareable conformance corpus (for go-ai / swift-ai adoption)

Per the parity auditor, the following rs-ai work is adoptable cross-port. Each
item is a real upstream-conformance behavior other ports still need.

- **4 upstream-divergence fixes (caching-gate family):** session-affinity
  headers (`session_id` / `x-client-request-id` / `x-session-affinity` /
  `prompt_cache_key`) must be cleared when `cacheRetention: "none"`. Fixed across
  all four affinity providers (anthropic, fireworks, openai-responses,
  openai-completions). Tests: `src/openai_completions_prompt_cache_test.rs`,
  `src/anthropic_cache_write_1h_cost_test.rs`, `src/codex_request_shape_test.rs`.
- **WS connection-limit retry-once + WS handshake header fix:** see the WS
  section below (`src/codex_ws_connection_limit_test.rs`,
  `src/codex_ws_protocol_test.rs`).
- **Device-code OAuth semantics:** `poll_oauth_device_code_flow` implements
  RFC8628 `slow_down` (+5s), min-interval clamp, and the
  Failed/TIMEOUT-vs-SLOW_DOWN message distinctions
  (`src/oauth_device_code_test.rs`, `src/oauth.rs`).
- **HTTP proxy:** `src/http_proxy.rs` resolves HTTP(S)/NO/ALL_PROXY with scoped
  precedence (SOCKS/PAC rejected), wired into all 6 client builders
  (`src/http_proxy_test.rs`, 4/4).
- **Vertex provider request path:** `src/google_vertex_request_path_test.rs`
  (project/location REST URL, ADC marker / gcp-vertex-credentials fallback,
  real-key path) — origin=go-ai `resolveVertexProjectLocation`.
- **Simulated-fixture ports (faithful wire-format, no fabricated output):**
  `src/simulated_e2e_fixtures_test.rs` (+ peers) — responseId, tokens,
  total-tokens (proves openai-completions ignores native total),
  context-overflow, unicode-surrogate reassembly, google-thinking-disable,
  cache-retention (request-shape).
- **Live-gated E2E wrappers:** `src/stream_e2e_live_test.rs` mirrors upstream
  `stream.test.ts` `describe.skipIf(!<KEY>)` — same names/assertions, run with a
  key, skip cleanly without. Pattern other ports can copy verbatim.

## Newly adapted this cycle

| go-ai test | rs-ai test | Notes |
|---|---|---|
| `TestExtractRegionFromURL` | `src/bedrock_endpoint_test.rs::extract_region_from_url` | 6 cases incl. fips + `.com.cn`; `None == ""`. Previously rs-ai's `bedrock_standard_endpoint_region` was untested. |
| `TestShouldUseExplicitBedrockEndpoint` | `src/bedrock_endpoint_test.rs::should_use_explicit_bedrock_endpoint` | custom→pinned, standard+clean-env→pinned, standard+`AWS_REGION`→not pinned. |
| (bonus) ARN region | `src/bedrock_endpoint_test.rs::extract_region_from_arn_model_id` | covers `bedrock_arn_region`. |
| `TestBuildCodexRequestMatchesPiaiShape` | `src/codex_request_shape_test.rs::build_codex_request_matches_piai_shape` | full pi-ai request-shape snapshot: stream/store, instructions, prompt_cache_key, tool_choice=auto, parallel_tool_calls, include, reasoning{effort,summary}, text{verbosity}, user-first input, tool strict:null. rs-ai shape confirmed matching. |

## Coverage status by area (go-ai corpus → rs-ai)

| go-ai area (count) | rs-ai status | Where |
|---|---|---|
| Retry helper (5: backoff/retryable/retry-after/duration) | COVERED | `src/retry.rs` tests + `src/coverage_test.rs` |
| SSE transport (4: parse/multiline/sticky-id/reader-errors) | COVERED | `src/transports/`, sse parser tests |
| Streaming/partial JSON (3) | COVERED | `src/jsonparse.rs` tests |
| Faux provider (10) | COVERED | `src/provider/faux.rs`, `provider_test.rs` |
| Transforms / synthetic tool results / image downgrade | COVERED | `src/transform.rs` tests |
| OpenAI completions payload/usage/cache (8) | COVERED | `src/provider_test.rs`, `coverage_test.rs` |
| OpenAI Codex request/headers/ws (9) | COVERED | `src/provider_test.rs` (`build_codex_payload`, ws replay) |
| Azure/Responses (10) | COVERED + ADAPTED | `src/azure_openai_base_url_test.rs` (11/11), `provider_test.rs` |
| Mistral (1) | COVERED + ADAPTED | `src/mistral_reasoning_mode_test.rs` (7/7) |
| Bedrock endpoint/region/headers (11) | PARTIAL → ADAPTED | endpoint+region now adapted; convert-messages/thinking-payload COVERED in `provider_test.rs` |
| Anthropic messages/retry/copilot (6) | COVERED | `src/provider_test.rs`, `provider_retry_test.rs` |
| Google/Vertex (4) | PARTIAL | google SSE/url COVERED; Vertex ADC = N/A (gated) |
| OAuth providers (8: PKCE/refresh/copilot-filtering) | PARTIAL / N/A | token helpers COVERED; interactive refresh/login = N/A (gated) |
| Model registry/metadata (3) | COVERED | `src/registry_test.rs`, `models_generated.rs` |
| Logger (5) | COVERED | `src/logger.rs` |
| Image generation / OpenRouter images (7) | COVERED | `src/images/`, image tests |
| Harness/context/compaction (many) | COVERED | `src/harness_test.rs`, `compaction.rs` |

## Pending high-value adaptations (next cycles)

1. `TestProcessSSEStreamAttachesPendingEncryptedReasoningDetails` — ADAPTED: `src/openai_encrypted_reasoning_test.rs` (encrypted `reasoning_details` attached to the matching tool call's `thought_signature`, order-independent; decoded fields asserted since the blob is opaque). All 3 originally-pending go-ai adaptations now done (codex request-shape, bedrock coalescing, SSE encrypted-reasoning).
2. `TestConvertMessagesCoalescesConsecutiveToolResults` (bedrock) — ADAPTED: extracted a testable `build_bedrock_messages` and ported the coalescing + cache-point assertion (`src/bedrock_coalesce_test.rs`).
3. `TestBedrockOptionPrecedenceAndRequestMetadata` — region option precedence + request metadata propagation.
4. OAuth `TestGetAPIKeyRefreshesExpiredCredential` — blocked on credential-store seam (parity-gaps top-3 #2).

## Reverse-direction gaps (rs-ai AHEAD — @go-ai / @swift-ai should adopt)

Cases where rs-ai matches canonical upstream (`ec6311b`) but the reference port
@go-ai currently diverges. Routed for the auditor to push downstream.

| # | Edge case | rs-ai (matches upstream) | @go-ai (diverges) |
|---|---|---|---|
| R1 | "No API key for provider" casing | `No API key for provider: <p>` across all providers + images | lowercase `no API key for provider` (openai/codex); `No API key **available** for provider` (images) |
| R2 | bedrock `MessageStart` role validation | throws `Unexpected assistant message start but got user message start instead` | `MessageStart` case just emits StartEvent; no role check |
| R3 | codex SSE/WS terminal-error strings | SSE `OpenAI Responses stream ended before a terminal response event`; WS `WebSocket stream closed before response.completed` | no codex-specific terminal-error strings |
| R4 | anthropic SSE `error` event surfacing | raw `sse.data` verbatim (no prefix) | no equivalent assertion |

_Note: the WS missing-handshake-header bug rs-ai fixed was rs-ai-specific (rs-ai
hand-built the `http::Request`); go-ai uses `coder/websocket` `Dial`, which adds
the RFC6455 headers itself — not a reverse gap._

### Reverse-direction TEST-METHOD items (origin = rs-ai; go-ai/swift-ai should adopt)

| # | rs-ai test | Why go-ai should adopt |
|---|---|---|
| RT1 | `src/codex_ws_connection_limit_test.rs` | **Real WS-server** integration test (not a mock): stands up a TcpListener that rejects attempt 1 with `websocket_connection_limit_reached` and serves a valid stream on the retry. This is the method that exposed rs-ai's silent handshake bug; if go-ai only has mock/replay WS tests it should add a real-handshake one. |
| RT2 | `src/codex_ws_protocol_test.rs` | Real WS-server happy-path: captures the outbound `response.create` (asserts `model`) and streams created/output_item.added/delta/done/completed; asserts Start+TextDelta+Done(Stop). Locks the handshake against regressions. |

Principle (auditor-endorsed): prefer **real server integration tests over mocks**
wherever a transport/handshake is involved — mocks can't catch a malformed
handshake request.

## rs-ai locally-authored regression tests (for @go-ai / @swift-ai to adapt)

These are edge cases rs-ai fixed against canonical upstream (`ec6311b`) that are
**not** upstream test ports. Per the auditor, logged here so sibling ports can
adapt them. Notably, on all four the **reference port @go-ai currently diverges
from canonical upstream** — rs-ai is ahead.

| rs-ai test / fix | Guards | Canonical upstream | @go-ai status |
|---|---|---|---|
| `provider_test.rs::test_openai_missing_api_key` (+ casing across 7 files) | Exact error string **`No API key for provider: <p>`** (capital N) in all 6 providers + openrouter images | `No API key for provider` (36 occurrences) | **DIVERGES**: lowercase `no API key for provider` (openai/codex) and `No API key **available** for provider` (images). rs-ai ahead. |
| `provider_test.rs::test_codex_sse_no_terminal_is_error` (+ WS string) | Codex SSE no-terminal emits the shared-decoder `OpenAI Responses stream ended before a terminal response event`; WS emits `WebSocket stream closed before response.completed` | both strings present | **MISSING**: go-ai has no codex-specific terminal-error strings. rs-ai ahead. |
| `provider_test.rs::test_anthropic_error_event_emits_error` | Anthropic SSE `error` event surfaces **raw `sse.data`** verbatim (no `SSE error:` prefix) | `throw new Error(sse.data)` | Different handling; no equivalent assertion. rs-ai ahead/divergent. |
| bedrock `MessageStart` role validation (`bedrock.rs:609`; arm, AWS-SDK-mock-hard to unit test) | Emits `Unexpected assistant message start but got user message start instead` when the first converse message is not assistant | `bedrock-converse-stream` throws same | **MISSING**: go-ai's `MessageStart` case just emits StartEvent, no role check. rs-ai ahead. |

## Newly identified gap (from go-ai corpus)

- **WS connection-limit retry-once — CLOSED.** Implemented
  `is_ws_connection_limit_error` + retry-once-before-SSE-fallback in
  `src/provider/codex.rs`, mirroring upstream
  `isWebSocketConnectionLimitReachedError` / `retriedWebSocketConnectionLimit`.
  Adapted as `src/codex_ws_connection_limit_test.rs` (real WS server rejects
  attempt 1 with the limit code, serves a valid stream on the retry).
- **WS handshake header bug — FIXED (latent).** While building the retry test,
  found rs-ai's Codex WebSocket request was a fully-built `http::Request`, which
  bypasses tungstenite's automatic handshake-header generation, so it shipped
  **without `Sec-WebSocket-Key`/`Upgrade`/`Connection`/`Sec-WebSocket-Version`/
  `Host`** — every WS handshake was rejected and the provider silently fell back
  to SSE. Now supplies all RFC6455 headers (fresh `generate_key()`), so the
  WebSocket transport actually connects. No prior test exercised a real handshake.
- **WS happy-path protocol flow — ADAPTED.** `TestStreamViaWebSocketProtocolFlow`
  -> `src/codex_ws_protocol_test.rs::stream_via_websocket_protocol_flow`: real WS
  server captures the outbound `response.create` (asserts `model`), streams
  created/output_item.added/output_text.delta/output_item.done/completed; client
  must emit Start + TextDelta("ok") + Done(Stop). This locks the handshake fix.

### Pending WS adaptations (need extra harness/feature)

- `TestStreamCodexWebSocketSetupFailureFallsBackToSSEWithDiagnostic` — needs a
  combined WS+HTTP server (same host serves the WS upgrade AND the SSE fallback)
  plus WS debug-stats counters (`WebSocketFailures`/`SSEFallbacks`/
  `WebSocketFallbackActive`), which rs-ai does not expose. rs-ai already covers
  SSE fallback + the `provider_transport_failure` diagnostic separately
  (`provider_test.rs`). Debug-stats counters live in the documented WS-pooling gap.
- `TestStreamViaWebSocketAutoUsesCachedDeltaAndDebugStats` — websocket-cached
  transport + debug stats (WS-pooling gap; N/A until pooling lands).


Fourth committed v0.84.0 slice: provider stream/error regressions are **PORTED** for Anthropic initial content blocks, Responses incomplete raw reasons, Google Gemini 3 tool IDs and signed-empty blocks, and Bedrock structured failure diagnostics. Full `cargo test`: `843 passed`; doctest `1 passed`; comparator `1153/1153` and `42/42`; strict Clippy/build/fmt clean.


Fifth committed v0.84.0 slice: runtime auth/options/telemetry semantics are **PORTED** for ProviderHeaders null deletion via a typed helper, OAuth refresh signal propagation, selected-provider refresh filtering, and telemetry context plumbing through stream/deferred/images. Full `cargo test`: `847 passed`; doctest `1 passed`; comparator `1153/1153` and `42/42`; strict Clippy/build/fmt clean.
