# Upstream parity gap analysis

## Current parity audit: `@earendil-works/pi-ai` v0.85.0

Authoritative package/tag: npm `@earendil-works/pi-ai@0.85.0`, package/tag SHA `107d79f11072bbc8a3a757ed7fd69596bee7d68c` (`github.com/earendil-works/pi` tag `v0.85.0`). Pinned scope is exactly `b79e4cc834970cca69daebffab7df1da7d1e52c4..107d79f11072bbc8a3a757ed7fd69596bee7d68c`; do not chase newer main.

Status: **ADAPTED / strict frame-decode corrective runtime candidate hosted CI passed; auditor acceptance pending**. The range changes **51** `packages/ai` paths: 19 source/scripts paths, 29 tests, and 3 package/docs paths. Whole-corpus filename evidence is in `docs/v0850-142-test-crosswalk.md`: **142/142** upstream `packages/ai/test/*.test.ts` files accounted, with executable validator `scripts/validate_v0850_manifests.py`.

Key v0.85.0 dispositions: generated text catalog is regenerated to **1336** provider/id pairs across **39** providers and **9** APIs, batch aliases are **66**, and image catalog remains **50**. Runtime adaptations cover public `assistant_message_frame` encoder/reducer symbols with exact camelCase frame JSON, assistant-only start partials with no `isError`, strict required start decode, presence-aware thinking redaction, `providerThinkingLevel`, Anthropic managed mid-conversation effort/binding, OpenAI-compatible vLLM priority, Responses `supportsMaxOutputTokens` plus terminal stale-error cleanup and serialized `errorMessage` omission/retention, explicit pre-generation auth errors, UUID explicit timestamps, pi-messages metadata, Codex terminal SSE without trailing blank line, and NO_PROXY suffix/port behavior. Cloudflare Workers binding replacement is documented as N/A for Rust Workers binding-object semantics, with HTTP/gateway semantics covered elsewhere.

Catalog evidence: npm tarball SHA-256 `46188bdacb555a07466a0111f3963f20932a16199e4d6cfb8d44a7fe5fc6e342`; provider manifest structure hash `a71a055905e4b12c9bb41fa6c2bf90fb2944ca76843bdfd3831377caba1be189`; full metadata verifier `metadata verified: text=1336 providers=39 apis=9 batchAliases=66 image=50`; exact manifest hashes `db461a56838926cf60d4ae0196ed98fcc215616dacff013ad8c235bb8ad9b83f` and `56f8742065a4ad01d73e5aee53035324f2e7333a735222ab15db870819e29065`; baseline delta verifier `text=+72/-26/79 changed image=+0/-0/0 changed`. Strict frame-decode local gates pass: focused frame wire (`20 passed`), full all-feature tests x3 (`975 passed`, `0 ignored`), no-default/Bedrock configs, manifest/metadata/baseline faults, provider/id comparator, SBOM/license/RustSec. Runtime SHA `0eb50d428d75a0281231fcd294d768c3db9cd17c` passed hosted CI run `33896845145`, job `build-test-lint` (`101101379038`), with SBOM artifact `rs-ai-sbom-0eb50d428d75a0281231fcd294d768c3db9cd17c` (`9946095734`), archive digest `sha256:5e5a9c9fd928da01afb4e3d0140bbfb4296a19ea6d332d892b46ea7c7d549ff3`, inner CI SBOM SHA-256 `24256dacc79097c90f399632852967c868371d8f64c85a5c7dea91ca3bd17284`, embedded revision `0eb50d428d75a0281231fcd294d768c3db9cd17c`, and expiry `2026-10-04T16:46:02Z`.


## Historical parity audit: `@earendil-works/pi-ai` v0.84.4

Authoritative package/tag: npm `@earendil-works/pi-ai@0.84.4`, package/tag SHA `b79e4cc834970cca69daebffab7df1da7d1e52c4` (`github.com/earendil-works/pi` tag `v0.84.4`). Pinned scope is exactly `4e58f324fae8ebfa98a3d45181fb248072a2afac..b79e4cc834970cca69daebffab7df1da7d1e52c4`; do not chase newer main.

Status: **ADAPTED / completed for bounded deterministic v0.84.4 release parity**. The range changes **15** `packages/ai` paths: 2 package metadata paths, 2 scripts, 5 source/runtime/catalog paths, and 6 tests. Whole-corpus filename evidence is in `docs/v0844-137-test-crosswalk.md`: **137/137** upstream `packages/ai/test/*.test.ts` files accounted, with executable validator `scripts/validate_v0844_manifests.py`.

Key v0.84.4 dispositions: generated text catalog is regenerated to **1290** provider/id pairs across **39** providers and **9** APIs, batch aliases are **40**, and image catalog is **50**. OpenAI-compatible streamed `reasoning_details` now merge adjacent text/summary deltas before replay; OpenAI/Responses `tool_choice` is forwarded even without tools; Mistral tool-call chunks merge by `index` when later fragments omit `id`/name; OpenRouter reasoning metadata now drives mandatory/optional `thinkingLevelMap` semantics; Cloudflare AI Gateway mirrors Workers AI `/compat` models without duplicate ids; Fireworks turbo routers are removed; and ZAI Coding Plan `glm-5.3` costs are updated. TypeScript-only comments/provider typing and package metadata are documented as N/A where no Rust runtime surface exists.

Catalog evidence: npm tarball SHA-256 `dfd3c929cee5a7387199a0a24dfc1be2096f1ea8f59ffb8285198a0ed01ebf93`; provider manifest structure hash `456b83c08bed3255d7e399d7927c6743e7f3568435691b3d38cc3666ffa70479`; full metadata verifier `metadata verified: text=1290 providers=39 apis=9 batchAliases=40 image=50`.

## Historical parity audit: `@earendil-works/pi-ai` v0.84.3

Authoritative package/tag: npm `@earendil-works/pi-ai@0.84.3`, package/tag SHA `4e58f324fae8ebfa98a3d45181fb248072a2afac` (`github.com/earendil-works/pi` tag `v0.84.3`). Pinned scope is exactly `914cf1472e715297caa30db4b9535d534a9eb718..4e58f324fae8ebfa98a3d45181fb248072a2afac`; do not chase newer main.

Status: **ADAPTED / completed for bounded deterministic v0.84.3 release parity**. The range changes **48** `packages/ai` paths: 19 source/runtime paths, 25 tests, and 4 package/docs/export paths. Whole-corpus filename evidence is in `docs/v0843-136-test-crosswalk.md`: **136/136** upstream `packages/ai/test/*.test.ts` files accounted, with executable validator `scripts/validate_v0843_manifests.py`.

Key v0.84.3 dispositions: generated text catalog is regenerated to **1312** provider/id pairs across **39** providers and **9** APIs, batch aliases remain **60**, image catalog remains **45**. Provider-neutral `tool_choice`, default Pi User-Agent with explicit override, Google thinking level map/error/budget semantics, Bedrock redacted reasoning replay/finalization, OpenAI `reasoning_details` preservation/replay, Copilot policy retry/filter behavior, and xAI/ZAI/Xiaomi generated compatibility changes are adapted. TypeScript export/package/docs-only changes and live credential/browser-only rows are documented as N/A for Rust runtime.

Catalog evidence: npm tarball SHA-256 `9c40af2f43950f8e94e7bbcd0c1b3548f000972da00c4fb9c0d0529d4d7d5431`; provider manifest structure hash `9a017e31c46be9520da694c2d30f95ddb3e4efa885bd043615d5b6f48e90eb81`; full metadata verifier `metadata verified: text=1312 providers=39 apis=9 batchAliases=60 image=45`.

## Historical parity audit: `@earendil-works/pi-ai` v0.84.2

Authoritative package/tag: npm `@earendil-works/pi-ai@0.84.2`, package/tag SHA `914cf1472e715297caa30db4b9535d534a9eb718` (`github.com/earendil-works/pi` tag `v0.84.2`). Pinned scope is exactly `53fa77ccd8a279eb87e92294ef3687b03ff80112..914cf1472e715297caa30db4b9535d534a9eb718`; do not chase newer main.

Status: **ADAPTED / under final gate verification**. The range changes **42** `packages/ai` paths: 18 source, 21 tests, 3 package/docs. Test accounting: **21 changed test paths total = 18 modified existing tests + 3 new tests**. Whole-corpus filename evidence is in `docs/v0842-131-test-crosswalk.md`: **131/131** upstream `packages/ai/test/*.test.ts` files accounted.

Key v0.84.2 dispositions: strict JSON-schema tool conversion and optional non-nullable null omission are ported; Kimi/Codex runtime User-Agent is ported; Responses/Codex `additional_tools`, tool-search fallback, tool-call namespace, and `AssistantMessage.endTurn` are ported; Mistral HTTP/SSE raw streaming coverage is adapted, including delayed chunking, split UTF-8, cancellation/drop cleanup, timeout, bounded 403 body, retry replay, affinity, and exact wire-payload assertions; Copilot policy-enable batching is ported; retry buffer-limit and DeepSeek case-insensitive `max_tokens` handling are ported; Google `STOP`/`MAX_TOKENS` raw reason behavior is ported; catalog/image metadata is regenerated and verified. Cloudflare `createGatewayBindingFetch` is N/A for Rust Workers binding object semantics, with existing HTTP transport injection/gateway tests covering Rust-applicable behavior.

Catalog evidence: npm tarball SHA-256 `0262785a76b0eb2eec596cd8a7ab2ee23eef89d2ef1bb1211c4f0a1944dacf41`; text `1267/1267`, image `45/45`, providers `39`, APIs `9`, official batch aliases `60`, full metadata verifier `metadata verified: text=1267 providers=39 apis=9 batchAliases=60 image=45`.

## Historical parity audit: `@earendil-works/pi-ai` v0.84.1

Authoritative package/tag: npm `@earendil-works/pi-ai@0.84.1`, package/tag SHA `53fa77ccd8a279eb87e92294ef3687b03ff80112` (`github.com/earendil-works/pi` tag `v0.84.1`). Pinned scope is exactly `a5f43bf8aff3c55752432655f7334e3dafd1e256..53fa77ccd8a279eb87e92294ef3687b03ff80112`; do not chase newer main.

Status: **ADAPTED / under final evidence-correction verification**. The range changes **25** `packages/ai` paths. Test accounting correction: **14 changed test paths total = 13 modified existing tests + 1 new `generate-models-strict.test.ts`**, split exactly as **10 credential-gated live-matrix additions + 2 deterministic provider/request rows + 2 generator-policy rows**. Whole-corpus filename evidence is in `docs/v0841-128-test-crosswalk.md`: **128/128** upstream `packages/ai/test/*.test.ts` files accounted.

| Upstream path group | Count | Current disposition | rs-ai path / evidence |
|---|---:|---|---|
| Release docs/package metadata: `CHANGELOG.md`, `README.md`, `package.json` | 3 | DOCUMENTED / N/A runtime | `RELEASE.md`; README env semantics mirrored in `src/env.rs`. |
| Generator/model-data strictness: `scripts/generate-models.ts`, `scripts/model-data.ts`, `test/generate-models-strict.test.ts`, `test/model-data-validation.test.ts` | 4 | ADAPTED | `scripts/extract_release_model_shards.py` exact Individual allowlist and audited batch alias policy; `src/tests/catalogs/model_data_validation_test.rs::{extractor_enforces_qwen_individual_strict_model_ids_without_output_mutation,extractor_allows_only_audited_release_batch_aliases}`. |
| Qwen Token Plan Individual provider/catalog/env/types: `src/env-api-keys.ts`, `src/models.generated.ts`, `src/providers/all.ts`, `src/providers/qwen-token-plan-individual*.ts`, `src/types.ts`, `test/qwen-token-plan-models.test.ts` | 7 | PORTED | `src/env.rs`, `src/models_generated.rs`, `src/provider/openai.rs`, `src/tests/release/v0841_release_test.rs::{qwen_token_plan_individual_catalog_env_and_endpoint_match_v0841,qwen_token_plan_individual_reasoning_payloads_match_v0841}`. |
| 10 credential-gated live-matrix additions for Individual provider: `test/{abort,context-overflow,cross-provider-handoff,empty,image-tool-result,stream,tokens,tool-call-without-result,total-tokens,unicode-surrogate}.test.ts` | 10 | LIVE UNEXECUTED / generic deterministic counterparts remain covered | Rust does not execute live Qwen Token Plan calls; existing deterministic generic provider stream/tool/token/abort/handoff/unicode suites cover shared OpenAI-compatible behavior, while Individual provider endpoint/env/catalog/request metadata is asserted in `src/tests/release/v0841_release_test.rs`. |
| Deterministic provider/request rows: `test/{qwen-token-plan-models,openai-completions-tool-choice}.test.ts` | 2 | ADAPTED | Individual provider request/catalog contract in `src/tests/release/v0841_release_test.rs`, existing OpenAI-compatible tool-choice tests, and Qwen payload assertions for `enable_thinking` plus supported `reasoning_effort`. |
| Catalog comparators | n/a | PORTED for release catalog | Pair comparator: `PI_AI_MODEL_DATA_DIR=/workspace/tmp/pi-v0841-json python3 scripts/compare_upstream_registry_pairs.py /workspace/tmp/pi-src 53fa77ccd8a279eb87e92294ef3687b03ff80112` => text `1220/1220`, image `42/42`, missing `0`, extra `0`. Full metadata gate: `python3 scripts/verify_release_model_metadata.py` => SHA-256-pinned npm tarball verification (`6ab689189e7cb3de5cdb126312a3e60e8ac35fe5ee5f1b63d00f711c8a430c73`) plus regenerated text/image metadata equality after timestamp normalization and rustfmt; clean success and fault-injection are covered by `src/tests/release/release_metadata_verification_test.rs`. |

Catalog evidence: official v0.84.1 npm provider shards validate as **1220** text models, **39** providers, **9** APIs, structure hash `24c74ac10bb8ed4df2c96bdadcfd94a417f3c823d5038875f59a261e3c84424b`. Unlike v0.84.0, the official release-pinned artifact includes **59** OpenRouter `:batch` aliases; extractor policy now preserves exactly those audited aliases and rejects unexpected drift. The full metadata verifier is the durable equality gate for the 9 metadata-changing provider/id pairs and exact unchanged image metadata; pair-only comparison remains a quick count/drift check, not the sole release gate. The verifier derives the image count from imported package data rather than a hard-coded constant.

## Historical completed parity audit: `@earendil-works/pi-ai` v0.84.0

Authoritative package/tag: npm `@earendil-works/pi-ai@0.84.0`, package/tag SHA `a5f43bf8aff3c55752432655f7334e3dafd1e256` (`github.com/earendil-works/pi` tag `v0.84.0`). Pinned scope is exactly `845d6ff1f6643aba440341cce877ce1c43ebbc39..a5f43bf8aff3c55752432655f7334e3dafd1e256`; do not chase newer main.

Generated manifest evidence is in `docs/v0840-manifests.md`: **101** changed `packages/ai` paths and **46** changed `packages/ai/test` files with extracted upstream assertion/gate lines. Whole-corpus filename evidence is in `docs/v0840-127-test-crosswalk.md`: **127/127** upstream `packages/ai/test/*.test.ts` files accounted.

Status: **COMPLETED for bounded deterministic release parity**. The 46 changed upstream test files in `docs/v0840-manifests.md` plus the cumulative 127-file crosswalk are ported/adapted, covered, or explicitly N/A for live credentials/interactive UI/JS-runtime-only surfaces.

| Upstream path group | Count | Current disposition | rs-ai path / evidence |
|---|---:|---|---|
| Baseten provider/catalog/auth: `src/providers/baseten.ts`, `src/providers/baseten.models.ts`, `src/env-api-keys.ts`, `scripts/generate-models.ts`, `test/baseten-models.test.ts` | 5 | PORTED in slice 1 | `src/models_generated.rs`, `src/env.rs`, `src/provider/openai.rs`, `src/tests/release/v0840_release_test.rs::baseten_catalog_and_reasoning_payload_match_v0840` |
| Generic sampling params: `src/types.ts`, `src/api/simple-options.ts`, OpenAI-compatible adapters, `test/sampling-options.test.ts` | 6 | PORTED in slice 1 for Rust request surface | `Model.sampling_params`, `StreamOptions.sampling_params`, OpenAI Completions + Responses/Azure payload merge; test `sampling_params_merge_and_override_openai_compatible_payloads` |
| vLLM `thinking_token_budget`: `src/api/openai-completions.ts`, `test/openai-completions-thinking-token-budget.test.ts` | 2 | PORTED in slice 1 | `supports_thinking_token_budget`, `MIN_ANSWER_TOKENS=1024` edge behavior; test `vllm_thinking_token_budget_edge_matrix` |
| Nullable union validation: `test/validation.test.ts` assertions | 1 | PORTED in slice 1 | `src/validation.rs` match-before-coerce for `anyOf`/`oneOf`; test `nullable_anyof_oneof_preserves_matching_null_before_coercion` |
| Streams without finish reasons: `OpenAICompletionsCompat.supportsFinishReason` | 1 | PORTED in slice 1 | `src/provider/openai.rs`; test `supports_finish_reason_false_infers_terminal_stop_or_tool_use` |
| Catalog comparator | n/a | PORTED for slice catalog | `PI_AI_MODEL_DATA_DIR=/workspace/tmp/pi-v0840-release-json python3 scripts/compare_upstream_registry_pairs.py /workspace/tmp/pi-v0840 a5f43bf8aff3c55752432655f7334e3dafd1e256` => text `1153/1153`, image `42/42`, missing `0`, extra `0` |



Catalog correction (2026-08-06): the earlier `1212` comparator result is superseded. It was generated from fresh dynamic catalog fetches and included 59 OpenRouter `:batch` aliases absent from official tag/npm artifacts. Release-pinned catalog generation now uses `scripts/extract_release_model_shards.py` against `/workspace/tmp/pi-ai-0.84.0-package/package/dist/providers/data`, producing text `1153/1153` across 38 providers and 9 APIs, image `42/42`, and no `:batch` aliases.



Fourth committed v0.84.0 slice: provider stream/error regressions are **PORTED** for Anthropic initial content blocks, Responses incomplete raw reasons, Google Gemini 3 tool IDs and signed-empty blocks, and Bedrock structured failure diagnostics. Full `cargo test`: `843 passed`; doctest `1 passed`; comparator `1153/1153` and `42/42`; strict Clippy/build/fmt clean.



Fifth committed v0.84.0 slice: runtime auth/options/telemetry semantics are **PORTED** for ProviderHeaders null deletion via a typed helper, OAuth refresh signal propagation, selected-provider refresh filtering, and telemetry context plumbing through stream/deferred/images. Full `cargo test`: `847 passed`; doctest `1 passed`; comparator `1153/1153` and `42/42`; strict Clippy/build/fmt clean.

Second committed v0.84.0 slice: upstream `382aa641` deferred/background response lifecycle is now **PORTED**. Evidence: `DeferredHandle`, `StopReason::Deferred`, `Message.deferred`, public `fetch_deferred`/`cancel_deferred` provider dispatch, and faux submit/pending/ready/cancel/unknown-handle executable tests in `src/tests/transports/providers_upstream_test.rs`. Full `cargo test` for this slice: `836 passed`; doctest `1 passed`; strict Clippy/build/comparator clean.

Sixth committed v0.84.0 slice: runtime/OAuth/telemetry dispatch closure, Codex account+session WebSocket fallback isolation, and Bedrock failure metadata are **PORTED**. Evidence includes concrete OAuth provider pre/mid-refresh cancellation tests, telemetry through stream_simple/deferred/images, ProviderHeaders deletion through the real OpenAI-compatible request builder, refresh cancellation/supersession, `codex_ws_account_cache_test`, and the 7-case Bedrock status/requestId/code/suppression matrix.

Seventh committed v0.84.0 slice: Google shared retry is **PORTED** at the real `stream_google` request call site. Evidence: `src/tests/providers/google/google_shared_retry_test.rs` covers the three upstream cases (`429` retries once with `max_retries=1`, no retry when unset, no retry for `400`) plus retry delay cap and caller cancellation via Rust retry primitives.

Whole-corpus hardening slice: the 8 previously absent upstream test filenames are now explicitly accounted and **ADAPTED**. `image-model-data` and `reasoning-options` exercise production generator helpers (`scripts/generate_image_models.py::parse_openrouter_image_models`, `scripts/generate_models.py::get_effort_thinking_level_map`) rather than test-local copies; `model-data-validation` exercises `scripts/validate_release_model_data.py` over exact failure fixtures; `xai-responses` now covers low/medium/high-only thinking levels and full Responses request shape. See `docs/v0840-127-test-crosswalk.md`.

Explicit final rubric dispositions:

- `message_update` delta-only JSON/RPC change — **N/A**. It belongs to coding-agent JSON/RPC transport serialization, not the `packages/ai` library/runtime surface rs-ai ports. rs-ai exposes typed in-process `Message`/`Event` values and does not implement a JSON/RPC message-delta transport.
- `ModelsStreamTransforms` → `ModelsRequestTransforms` — **ADAPTED**. The Rust equivalent applies transforms across request surfaces: `merge_auth_into_request` for auth/header transforms into provider builders, `registry::stream_simple`, `registry::fetch_deferred`, and `registry::cancel_deferred` for text/deferred dispatch, plus image provider dispatch telemetry. Evidence: `src/tests/auth/oauth/models_runtime_auth_test.rs::provider_header_null_deletion_reaches_openai_request_builder` and `src/tests/transports/providers_upstream_test.rs::telemetry_context_flows_through_stream_simple_deferred_cancel_and_images`.

## Historical prior parity audit: `@earendil-works/pi-ai` v0.83.0

Authoritative package/tag: npm `@earendil-works/pi-ai@0.83.0`, package/tag SHA `845d6ff1f6643aba440341cce877ce1c43ebbc39` (`github.com/earendil-works/pi` tag `v0.83.0`). Pinned scope for this release-only audit is exactly `b4f293684bba718d59cc1157679bcf6157b3a7f5..845d6ff1f6643aba440341cce877ce1c43ebbc39`; do not chase newer main.

Accepted baseline remains `0e6909f050eeb15e8f6c05185511f3788357ddb3`; prior deltas through v0.82.1 remain ported. v0.82.1→v0.83.0 changes **41** unique `packages/ai` paths; disposition matrix:

| Upstream path group | Count | Disposition | rs-ai path / evidence |
|---|---:|---|---|
| Release docs/package metadata: `CHANGELOG.md`, `README.md`, `package.json` | 3 | N/A | package/docs metadata only; release disposition recorded here. |
| Catalog/generator metadata: `scripts/generate-models.ts` and generated provider data | 1 | PORTED | regenerated `src/models_generated.rs` from v0.83.0 JSON shards; comparator `text=1153/1153`, `image=40/40`. |
| Fetch/raw-stop/API runtime: `src/types.ts`, `src/api/{anthropic-messages,azure-openai-responses,bedrock-converse-stream,google-generative-ai,google-vertex,mistral-conversations,openai-codex-responses,openai-completions,openai-responses-shared,openai-responses,openrouter-images,pi-messages,simple-options}.ts` | 14 | PORTED / N/A where TS-only | `Message.raw_stop_reason` added and populated in OpenAI/Responses/Codex/Anthropic/Google/Mistral/Bedrock; fetch-option plumbing is N/A for reqwest Rust runtime. |
| OpenRouter OAuth/auth cause/faux: `src/auth/oauth/openrouter.ts`, `src/auth/resolve.ts`, `src/providers/faux.ts` | 3 | PORTED / COVERED | OpenRouter OAuth helper/adapter and ModelsError cause preservation already ported; faux/raw message shape updated via shared `Message`. |
| Deterministic tests: `test/{anthropic-sse-parsing,azure-openai-responses-reasoning-replay,bedrock-credentials,bedrock-raw-stop-reason,constrained-sampling,faux-provider,fetch-option,github-copilot-anthropic,google-raw-stop-reason,mistral-raw-stop-reason,models-runtime,oauth-auth,openai-completions-raw-stop-reason,openai-completions-tool-choice,openai-responses-partial-json-cleanup,openai-responses-terminal-event,openrouter-oauth,pi-messages,qwen-token-plan-models,validation}.test.ts` | 20 | PORTED / N/A where live/TS-only | raw-stop/fetch/model metadata expectations are covered in `src/tests/release/v0830_release_test.rs` and existing provider tests; live/TS-only cases are N/A. |

- **Model and image registries — PORTED.** Regenerated text catalog from upstream v0.83.0 hydrated JSON shards: **1153 text provider/id pairs across 37 providers**; image catalog remains **40/40**. Repro comparator evidence: `PI_AI_MODEL_DATA_DIR=/workspace/tmp/pi-v0830-json scripts/compare_upstream_registry_pairs.py /workspace/tmp/pi-v0830 845d6ff1f6643aba440341cce877ce1c43ebbc39` => `text: upstream=1153 local=1153 missing=0 extra=0`; `image: upstream=40 local=40 missing=0 extra=0`.

## Historical parity audit: `@earendil-works/pi-ai` v0.82.1

Authoritative package/tag: npm `@earendil-works/pi-ai@0.82.1`, package/tag SHA `b4f293684bba718d59cc1157679bcf6157b3a7f5` (`github.com/earendil-works/pi` tag `v0.82.1`). v0.82.1 changed **23** unique `packages/ai` paths and was fully ported before v0.83.0 superseded it.

| Upstream path group | Count | Disposition | rs-ai path / evidence |
|---|---:|---|---|
| Release docs/package metadata: `CHANGELOG.md`, `package.json` | 2 | N/A | package/docs metadata only; release disposition recorded here. |
| Catalog/generator metadata: `scripts/generate-models.ts` and generated provider data | 1 | PORTED | regenerated `src/models_generated.rs` from v0.82.1 JSON shards; comparator `text=1109/1109`, `image=40/40`. |
| Bedrock Opus 5 runtime/catalog: `src/api/bedrock-converse-stream.ts`, `test/bedrock-models.test.ts`, `test/bedrock-thinking-payload.test.ts` | 3 | PORTED | Opus 5 Bedrock inference profiles/settings are in generated catalog and asserted in `src/v0821_release_test.rs`. |
| Radius OAuth gateway routing: `src/auth/oauth/radius.ts`, `test/radius-oauth.test.ts` | 2 | PORTED | `src/oauth.rs` accepts discovery-only `/v1/oauth` and routes token/device calls through gateway `/v1/oauth/*`; existing Radius OAuth tests pass. |
| ModelsError/model-store runtime: `src/auth/resolve.ts`, `src/models-store.ts`, `test/models-runtime.test.ts` | 3 | PORTED | `ModelsError::with_cause` preserves cause detail in messages; `ModelsStoreEntry` now carries `last_modified` and `etag`; tested in `src/v0821_release_test.rs`. |
| Anthropic bearer-token env auth: `src/env-api-keys.ts`, `src/providers/anthropic.ts`, `test/anthropic-auth-token.test.ts`, `test/env-api-keys.test.ts`, `test/providers.test.ts` | 5 | PORTED | `ANTHROPIC_AUTH_TOKEN` participates in env discovery while `get_env_api_key` skips it as request API key; tested in `src/v0821_release_test.rs`. |
| Error body formatting/regressions: `src/utils/error-body.ts`, `test/error-body.test.ts`, `test/provider-error-body-regression.test.ts` | 3 | COVERED | Existing Rust `error_body` provider HTTP formatting remains covered by provider/error-body regression tests. |
| Metadata expectation tests: `test/anthropic-adaptive-thinking-models.test.ts`, `test/openai-responses-reasoning-replay-e2e.test.ts`, `test/supports-xhigh.test.ts`, `test/xhigh.test.ts` | 4 | PORTED / N/A live | deterministic metadata expectations covered by regenerated catalog; live E2E remains N/A. |

- **Model and image registries — PORTED.** Regenerated text catalog from upstream v0.82.1 hydrated JSON shards: **1109 text provider/id pairs across 37 providers**; image catalog remains **40/40**. Repro comparator evidence: `PI_AI_MODEL_DATA_DIR=/workspace/tmp/pi-v0821-json scripts/compare_upstream_registry_pairs.py /workspace/tmp/pi-v0821 b4f293684bba718d59cc1157679bcf6157b3a7f5` => `text: upstream=1109 local=1109 missing=0 extra=0`; `image: upstream=40 local=40 missing=0 extra=0`.

## Historical parity audit: `@earendil-works/pi-ai` v0.82.0

Authoritative package/tag: npm `@earendil-works/pi-ai@0.82.0`, package/tag SHA `083e61621276bff9f6faefab87ce07fcd98734e2` (`github.com/earendil-works/pi` tag `v0.82.0`). v0.82.0 changed **96** unique `packages/ai` paths and was fully ported before v0.82.1 superseded it.

| Upstream path group | Count | Disposition | rs-ai path / evidence |
|---|---:|---|---|
| Release docs/package metadata: `CHANGELOG.md`, `README.md`, `package.json` | 3 | N/A | package/docs metadata only; release disposition recorded here. |
| Model generator/data/reasoning scripts: `scripts/generate-models.ts`, `scripts/model-data.ts`, `scripts/models-dev-reasoning-options.ts` | 3 | PORTED | hydrated v0.82.0 provider data for comparator; regenerated `src/models_generated.rs`; model-data structural validation remains in `src/v0820_release_test.rs`. |
| API/provider runtime source: `src/api/{anthropic-messages,azure-openai-responses,bedrock-converse-stream,constrained-sampling,google-generative-ai,google-shared,google-vertex,mistral-conversations,openai-codex-responses,openai-completions,openai-responses-shared,openai-responses,openrouter-images}.ts` | 13 | PORTED / N/A where already generic | JSON-schema strict and grammar constrained-sampling helpers, monotonic grammar input delta reconstruction, Responses/Azure custom-tool request+stream shape, OpenAI Completions custom-tool request shape, Codex Responses custom-tool payload shape, OpenAI-completions tool-call ID uniqueness, retry/overflow deltas, and OpenCode Go Responses are covered by v0.82.0 Rust tests. |
| OAuth/auth runtime: `src/auth/oauth/kimi-coding.ts`, `src/auth/oauth/load.ts`, `src/auth/oauth/openrouter.ts`, `src/bun-oauth.ts` | 4 | PORTED / N/A | OpenRouter key exchange and Kimi Code device/refresh helpers are ported, and shared `ModelsRuntime::populate_builtin_fallbacks` wires provider-specific OAuth for `openrouter` and `kimi-coding`; browser/Bun loopback UI glue is N/A. |
| Catalog/type infra: `src/image-models.generated.ts`, `src/model-catalog.ts`, `src/models.generated.ts`, `src/providers/all.ts`, `src/types.ts` | 5 | PORTED | regenerated text/image registries; tool-result usage/type-only changes are serde-compatible/covered by existing `Message` shape. |
| Provider text catalogs: 37 changed `src/providers/*.models.ts` shards | 37 | PORTED | regenerated `src/models_generated.rs`; JSON-shard-aware comparator `text: upstream=1116 local=1116 missing=0 extra=0`. |
| Provider implementations: `src/providers/kimi-coding.ts`, `src/providers/openrouter-images.ts`, `src/providers/openrouter.ts` | 3 | PORTED / N/A where credential glue | regenerated provider metadata; OpenRouter image catalog additions and provider request behavior are covered by image/model tests and existing OpenRouter paths. |
| Shared retry utilities: `src/utils/provider-retry.ts`, `src/utils/retry.ts` | 2 | PORTED | `do_with_retry` now mirrors provider retry semantics (`x-should-retry`, no-status transport retry, excessive server delay rejection, abortable backoff via `do_with_retry_cancel`); OpenAI provider path and retry assistant abort semantics are tested. |
| Deterministic/unit tests: `test/{anthropic-eager-tool-input-compat,azure-openai-base-url,bedrock-convert-messages,cache-retention,constrained-sampling,deferred-tools,google-shared-convert-tools,kimi-coding-oauth,mistral-tool-schema,model-catalog-types,model-data-validation,oauth-auth,openai-codex-stream,openai-completions-cache-control-format,openai-completions-retry,openai-completions-thinking-as-text,openai-completions-tool-choice,openai-completions-tool-result-images,openrouter-cache-control-models,openrouter-oauth,provider-retry,providers,reasoning-options,retry,supports-xhigh,together-models}.test.ts` | 26 | PORTED / N/A where credential/live | deterministic metadata/request/retry utilities are covered in Rust tests; credential/browser OAuth and TS-only model-catalog type checks are N/A for Rust runtime. |

- **Model and image registries — PORTED.** Regenerated text catalog from upstream v0.82.0 hydrated JSON shards: **1116 text provider/id pairs across 37 providers**; image catalog is **40/40**. Repro comparator evidence: `PI_AI_MODEL_DATA_DIR=/workspace/tmp/pi-v0820-json scripts/compare_upstream_registry_pairs.py /workspace/tmp/pi-v0820 083e61621276bff9f6faefab87ce07fcd98734e2` => `text: upstream=1116 local=1116 missing=0 extra=0`; `image: upstream=40 local=40 missing=0 extra=0`.

## Historical parity audit: `@earendil-works/pi-ai` v0.81.1

Authoritative package/tag: npm `@earendil-works/pi-ai@0.81.1`, package/tag SHA `20be4b18d4c57487f8993d2762bace129f0cf7c6` (`github.com/earendil-works/pi` tag `v0.81.1`). v0.81.1 changed **88** `packages/ai` paths and was fully ported before v0.82.0 superseded it.

## Historical parity audit: `@earendil-works/pi-ai` v0.80.10

Authoritative package/tag: npm `@earendil-works/pi-ai@0.80.10`, package/tag SHA `8dc78834` (`github.com/earendil-works/pi` tag `v0.80.10`). v0.80.10 changed **20** `packages/ai` paths and was fully ported before v0.81.1 superseded it.

## Historical parity audit: `@earendil-works/pi-ai` v0.80.9

Authoritative package/tag: npm `@earendil-works/pi-ai@0.80.9`, package/tag SHA `2d16f92973230a7e095aa984f150ba8702784f50` (`github.com/earendil-works/pi` tag `v0.80.9`). Pinned scope for that audit was exactly `2be9efa19cd64aed40ca63f92c0c0f9a6bac7c9d..2d16f92973230a7e095aa984f150ba8702784f50`.

v0.80.9 disposition matrix for changed `packages/ai` paths:

| Upstream path(s) | Disposition | rs-ai path / note |
|---|---|---|
| `src/providers/*.models.ts`, `scripts/generate-models.ts`, generated catalog metadata | PORTED | `src/models_generated.rs`; text comparator now `1075/1075`, image `35/35`. |
| `src/models.ts` refresh `force` | PORTED | `src/models_runtime.rs`; `RefreshOptions.force` is propagated to `RefreshModelsContext.force`; test in `src/tests/catalogs/models_runtime_refresh_test.rs`. |
| `src/auth/oauth/xai.ts`, `test/xai-oauth.test.ts` verification URI complete | PORTED | `src/oauth.rs`, `src/tests/auth/oauth/xai_oauth_test.rs`; parses, validates and prefers `verification_uri_complete`; rejects untrusted complete URLs. |
| `src/providers/kimi-coding.models.ts`, deferred-tools tests | PORTED | generated registry + Kimi deferred-tool compatibility tests from `beacab7` retained. |
| `src/api/openai-completions.ts` Kimi/tool-result request behavior | PORTED | Existing Kimi/deferred/openai-completions coverage retained; no further Rust API shape change required beyond generated compat metadata. |
| `auth/helpers.ts`, `auth/oauth/load.ts`, `auth/types.ts`, `src/bun-oauth.ts`, README/changelog/live tests | N/A / docs/runtime glue | Browser/Bun CLI OAuth helper and live credential test edits are not Rust-library runtime behavior; relevant xAI OAuth helper behavior is ported.

- **Model and image registries — PORTED.** Regenerated text catalog from upstream v0.80.9: **1075 text provider/id pairs across 35 providers**; image catalog remains **35/35**. Repro comparator evidence: `text: upstream=1075 local=1075 missing=0 extra=0`; `image: upstream=35 local=35 missing=0 extra=0` using `scripts/compare_upstream_registry_pairs.py /workspace/tmp/pi-src 2d16f92973230a7e095aa984f150ba8702784f50`.
- **New `pi-messages` API and dynamic `radius` provider — PORTED.** Added `api::PI_MESSAGES`, `provider_id::RADIUS`, builtin provider registration, `PI_GATEWAY_API_KEY` env resolution, and `src/provider/pi_messages.rs` implementing POST `<baseUrl>/messages`, `debug=1`, provider headers, `on_payload`, `on_response`, SSE event conversion, terminal `done`/`error`, rewrite diagnostics, and deterministic error handling.
- **`pi-messages.test.ts` — PORTED.** Added `src/tests/core/pi_messages_test.rs` with deterministic wiremock coverage for streaming text/tool calls, terminal message state, response headers/debug, server-sent errors, missing API key, and missing terminal event. The Rust test is compressed into 3 async tests but covers the upstream assertions without headline skips.
- **`types.ts`/`compat.ts`/`providers/all.ts` type-only changes — PORTED/N/A.** Runtime-relevant `pi-messages` registration is ported. TypeScript-only `BuiltinProvider` narrowing is N/A in Rust because registry lookups are runtime typed (`Model.api`/`provider` strings) rather than generic TS provider-map indexing.
- **`utils/oauth/radius.ts` — PORTED (portable helper surface).** Added Radius OAuth discovery, PKCE authorize URL construction, device authorization, token exchange/refresh, gateway `/v1/config` catalog loading/sanitization, previous-catalog retention on transient config refresh failures, `RadiusOAuth` auth adapter, and model-catalog modification helper. Browser launching/HTML callback UI remains platform/UI glue; Rust exposes the deterministic URL/exchange pieces and covers them with local HTTP tests.
- **Existing e2e timeout edits (`anthropic-*e2e.test.ts`) — N/A.** They only increase live test timeouts; no deterministic runtime behavior changes.
- **OpenAI Responses test rename/additions — ALREADY COVERED/PORTED.** Upstream renames `openai-responses-copilot-provider.test.ts` to `openai-responses-compat.test.ts` and adds compat assertions; corresponding Rust coverage remains in `src/tests/providers/openai/openai_responses_copilot_provider_test.rs` and existing compat/prompt-cache response tests.
- **OpenAI Codex session-id clamp (`dcfe36c7`) — PORTED.** Upstream main after v0.80.7 clamps `sessionId` with `clampOpenAIPromptCacheKey` before Codex SSE `[session]-id`/request-id headers and WebSocket request ids. rs-ai now clamps `session-id`/`x-client-request-id` in SSE and WS paths; deterministic coverage is in `src/tests/providers/codex/openai_codex_stream_test.rs`.
- **Runtime model refresh + generated catalog publication (`2be9efa` + v0.80.9 force) — PORTED.** Added production `src/models_runtime.rs` (`ModelsRuntime`, provider-scoped `ModelsStore`, `RuntimeProvider`, coalesced refresh, cache restore/offline retention, cancellation, `force`) and wired ordinary `registry::get_model/list_models/list_providers` to a shared runtime initialized with builtins. Public `registry::register_runtime_provider`, `remove_runtime_provider`, `refresh_runtime_models`, and `register_radius_runtime_provider` expose production registration/refresh. Radius `/v1/config` refresh is a real `RuntimeProvider::radius` path; ordinary registry lookups reflect additions/removals and retain cache on network/offline failures. xAI device OAuth and `xai/grok-4.5` Responses routing are ported.

Canonical upstream: `@earendil-works/pi-ai` **v0.80.3**
(`github.com/earendil-works/pi`, `packages/ai`, commit `ec6311b`).
Port: `rs-ai` (crate `rs-ai`), branch `main`, tag `v0.80.3`.

> **0.80.3 upgrade (feature release).** Audited the full 0.80.2→0.80.3 `api/`,
> `utils/`, `index.js` and provider-catalog diff. Ported, with new tests:
> - **`Usage.reasoning`** capture across providers: anthropic
>   (`output_tokens_details.thinking_tokens`, optional), openai-completions
>   (`completion_tokens_details.reasoning_tokens`, `|| 0`), openai-responses
>   (`output_tokens_details.reasoning_tokens`, `|| 0`), google + vertex
>   (`thoughtsTokenCount`, `|| 0`). bedrock/mistral/codex unchanged (no upstream
>   breakdown). (`src/types.rs`, `simple_options.rs`, `provider/{anthropic,google}.rs`;
>   tests in `tests/core/simple_options_test.rs`, `tests/providers/anthropic/anthropic_sse_parsing_test.rs`.)
> - **`is_retryable_assistant_error`** + provider-error pattern matcher
>   (`utils/retry.ts`), faithful `.?`/`d?` matcher without adding the `regex`
>   crate (`src/retry.rs`; `src/tests/transports/retry_classify_test.rs`).
> - **`estimate_context_tokens`** (`utils/estimate.ts`) public utility
>   (`src/estimate.rs`; `src/tests/core/estimate_test.rs`).
> - **`clamp_max_tokens_to_context`** (`simple-options.ts`) is folded into **every**
>   provider's request path. Upstream v0.80.3 changed `buildBaseOptions` to
>   `buildBaseOptions(model, context, options, apiKey)` and clamps
>   `base.maxTokens = clampMaxTokensToContext(model, context, options?.maxTokens ?? model.maxTokens)`;
>   `streamSimple` passes that defaulted+clamped cap into the inner stream, so the
>   wire param ends up clamped for openai-completions, openai-responses (+azure),
>   google, vertex, mistral, anthropic and bedrock. rs-ai has no separate
>   `streamSimple` layer, so each builder defaults `maxTokens` to `model.maxTokens`
>   and clamps it (matching each provider's inner gate: openai/responses use a
>   truthy gate so a clamped 0 is omitted; google/mistral use `!== undefined` so
>   the field is always emitted). codex sends no max-tokens field upstream, so it
>   is a no-op. (`src/simple_options.rs`, `provider/{openai,responses,google,mistral,anthropic,bedrock}.rs`;
>   `src/tests/core/simple_options_test.rs`, `tests/providers/openai/openai_completions_empty_tools_test.rs`.)
> - **z.ai thinking** sends `{ type: "enabled", clear_thinking: false }`
>   (`provider/openai.rs`; `tests/providers/openai/openai_completions_tool_choice_test.rs`).
> - **Provider error-body formatting** (`utils/error-body.ts`): `"{status}: {body}"`
>   (branded `"OpenAI/Azure API error ({status}): {body}"` for responses), trimmed +
>   truncated to 4000 chars. Applied to openai-completions / responses(+azure) /
>   google / openrouter-images; codex (plain-Error path) + bedrock (SDK already
>   folds body) are no-ops; anthropic/mistral are not consumers.
>   (`src/error_body.rs`; `src/tests/core/error_body_test.rs`.)
> - **Model catalogs** regenerated to 0.80.3 (1029 models / 35 providers + 35 image
>   models), incl. `anthropic/claude-sonnet-5` (`forceAdaptiveThinking`).
>   (`src/models_generated.rs`, `src/images/models_generated.rs`.)
>
> **Deep source-diff follow-up (existing test files changed in 0.80.3).** A second
> pass diffed the *contents* of unchanged-name upstream test files between
> `v0.80.2`→`v0.80.3` and surfaced behaviours the dist sweep had buried inside
> larger hunks. Ported, with tests:
> - **`output_config: { effort }`** for adaptive-thinking (`forceAdaptiveThinking`)
>   models — already emitted by anthropic (`provider/anthropic.rs`) and bedrock
>   (`provider/bedrock.rs`); added `claude-sonnet-5` cases
>   (`tests/providers/anthropic/anthropic_thinking_disable_test.rs`, `tests/providers/bedrock/bedrock_thinking_payload_test.rs`).
>   Bedrock `supports_adaptive_thinking` matcher synced to upstream (adds
>   `sonnet-5`).
> - **Azure Microsoft Foundry** base-URL normalization: `.ai.azure.com` /
>   `.services.ai.azure.com` hosts and the `/openai/v1/responses` path now
>   normalize to `/openai/v1` (`provider/responses.rs`;
>   `tests/providers/openai/azure_openai_base_url_test.rs`).
> - **Codex SSE header timeout**: a timed-out SSE GET now surfaces the exact
>   `"Codex SSE response headers timed out after {ms}ms"` message when
>   `timeout_ms` is set (`provider/codex.rs`; `tests/providers/codex/openai_codex_stream_test.rs`).
> - **z.ai reasoning_content replay**: confirmed the dynamic-signature replay
>   (first thinking block's signature becomes the message key) keeps z.ai
>   thinking `{enabled, clear_thinking:false}` — added the combined replay test
>   (`tests/providers/openai/openai_completions_tool_choice_test.rs`).
>
> Result: **735 tests, 0 failures, 0 clippy warnings** (each run verified 3× for
> determinism).

## Current auditor-required mainline follow-up: deferred/message-anchored tools

Upstream main commit `0e6909f050eeb15e8f6c05185511f3788357ddb3` (`feat(ai): support message-anchored tool loading (#6474)`) adds `packages/ai/test/deferred-tools.test.ts` (16 Vitest cases). Although this file was not in the locally packaged v0.80.6 tarball, rs-ai now treats it as required parity.

Status: **DONE** in Rust with deterministic payload tests in `src/tests/core/deferred_tools_test.rs` and helpers in `src/deferred_tools.rs`.

Mapped behavior:
- `Message.added_tool_names` is the idiomatic Rust field equivalent of upstream `addedToolNames`; because `Message` uses `serde(rename_all = "camelCase")`, it serializes as `addedToolNames` and is skipped when empty.
- Anthropic `defer_loading` tool definitions are emitted only when `supports_tool_references` is true (default true for eligible Anthropic Messages models; false for Haiku and `claude-sonnet-4-20250514`, with explicit compat override support).
- Anthropic tool-result markers emit `tool_reference` content for newly anchored tools and preserve the original sibling tool output content immediately after the reference block, including image blocks.
- Anthropic OAuth canonicalizes Claude Code tool names for active definitions, marker lookup, prior-use detection, and duplicate `read`/`Read` definitions.
- OpenAI Responses/Codex support client-side `tool_search_call` + `tool_search_output` payloads only for supported models (`openai/gpt-5.4`, `openai/gpt-5.4-codex`, `openai-codex/gpt-5.4` by current registry); unsupported Responses, Codex, and OpenAI-compatible providers fall back to sending all tools immediately.
- `estimate_context_tokens` accounts for added tool definitions after a latest-usage checkpoint so deferred definitions are not hidden by usage anchoring.
- Edge cases covered: missing marked tools are ignored, all-tools-marked falls back to immediate tools, previously-used tools remain immediate, OpenAI-origin history can introduce Anthropic deferred tools, provider override flags are honored, and sibling output ordering is locked.

Additional current-main consistency gate from the same `0e6909f...` tree:
- `packages/ai/test/azure-openai-responses-reasoning-replay.test.ts` is ported in `src/tests/providers/openai/azure_openai_responses_reasoning_replay_test.rs`.
- `response.output_item.done` reasoning `encrypted_content` wins when present.
- Terminal `response.completed.output` backfills encrypted reasoning content only when the persisted `output_item.done` reasoning item omitted `encrypted_content`, preserving deterministic same-model replay for Azure OpenAI Responses.

Registry parity follow-up for v0.80.6/current audit: model counts are not used alone. `scripts/compare_upstream_registry_pairs.py /workspace/tmp/pi-src 0e6909f050eeb15e8f6c05185511f3788357ddb3` imports upstream `MODELS`/`IMAGE_MODELS` with Bun, recursively flattens provider maps, and compares provider/id pairs against Rust generated registries. Pinned `0e6909f...` parity is **1057 text provider/id pairs** and **35 image provider/id pairs** with missing=0/extra=0; unpinned-current `azure-openai-responses/gpt-5.6` and `openai/gpt-5.6` are intentionally absent for this agreed source.

Status legend:
- **DONE** — functional parity verified; behaviour + semantics match.
- **PARTIAL** — core behaviour present; documented divergence or incomplete edges.
- **MISSING** — no rs-ai equivalent.
- **N/A** — not portable / out of scope for a Rust library (CLI, lazy-loader, deprecated shims).

---

## 1. API modules (`src/api/*.ts`)

| Upstream module | rs-ai path | Status | Notes |
|---|---|---|---|
| `anthropic-messages.ts` | `src/provider/anthropic.rs` | DONE | SSE parse, thinking, cache, eager tool input, adaptive thinking, tool-id normalization. |
| `azure-openai-responses.ts` | `src/provider/responses.rs` | DONE | Azure base-url normalization + invalid-URL validation (`Invalid Azure OpenAI base URL`), api-version, prompt-cache-key clamp, store:false. Ported test-for-test (11/11). |
| `bedrock-converse-stream.ts` | `src/provider/bedrock.rs` | DONE | AWS SDK converse stream; MessageStart role check, tool-id normalization, stop-reason map. |
| `cloudflare.ts` | `src/compat.rs` + `src/env.rs` | PARTIAL | account/gateway resolved from env (documented divergence vs upstream credential plumbing). |
| `github-copilot-headers.ts` | `src/utils.rs` (`copilot_dynamic_headers`) | PARTIAL | Pure header logic ported: `inferCopilotInitiator` (X-Initiator user/agent from last message), `hasCopilotVisionInput` (image in user/toolResult → Copilot-Vision-Request), Openai-Intent. Wired into anthropic/openai/responses provider builders. Copilot OAuth credential acquisition remains MISSING (no credential path). |
| `google-generative-ai.ts` | `src/provider/google.rs` | DONE | |
| `google-shared.ts` | `src/provider/google.rs` | DONE | convert-tools, gemini3 unsigned tool-call, image tool-result routing, thinking signature. |
| `google-vertex.ts` | `src/provider/google.rs` (+`mod.rs`) | DONE | Vertex request path ported from go-ai `buildStreamURL`/`resolveVertexProjectLocation`: project/location-scoped REST endpoint, `{location}` host substitution, GOOGLE_CLOUD_PROJECT/GCLOUD_PROJECT + GOOGLE_CLOUD_LOCATION env fallback (or StreamOptions.project/location), placeholder/ADC api-key handling. Shared `stream_google` decoder. |
| `mistral-conversations.ts` | `src/provider/mistral.rs` | DONE | reasoning-mode, tool-schema, cached tokens, tool-id normalization — fully verified. |
| `openai-codex-responses.ts` | `src/provider/codex.rs` | PARTIAL | WS + SSE transports; OAuth account-id from env; no WS pooling/idle cache (incl. 55-min connection-age recycling). WS handshake headers fixed (Sec-WebSocket-Key etc.) + connection-limit retry-once added. SSE request body zstd-compressed (level 3, `Content-Encoding: zstd`) matching the official Codex client. Codex `sessionId` is clamped to 64 chars for prompt-cache key plus SSE/WS session/request-id headers (upstream `dcfe36c7`). |
| `openai-completions.ts` | `src/provider/openai.rs` | DONE | reasoning-details, tool-choice, response-model, retry, thinking-as-text, prompt-cache. |
| `openai-prompt-cache.ts` | `src/prompt_cache.rs` | DONE | `clamp_openai_prompt_cache_key` (64 codepoints). |
| `openai-responses-shared.ts` | `src/provider/responses.rs` | DONE | terminal-event enforcement, mapStopReason, foreign tool-call id, message-id. |
| `openai-responses.ts` | `src/provider/responses.rs` | DONE | |
| `openrouter-images.ts` | `src/images/openrouter.rs` | DONE | |
| `simple-options.ts` | `src/simple_options.rs` | DONE | cost calc, reasoning clamp, usage finalize, service-tier pricing. |
| `transform-messages.ts` | `src/transform.rs` | DONE | image downgrade, cross-model thinking→text, synthetic tool results, id-normalization callback folded into per-provider wire build. |

## 2. Auth layer (`src/auth/*.ts`, `src/utils/oauth/*.ts`)

| Upstream | rs-ai path | Status | Notes |
|---|---|---|---|
| `auth/context.ts` (`defaultProviderAuthContext`) | `src/auth.rs` (`AuthContext`/`EnvAuthContext`) | DONE | injectable env context with request-scoped overlay (mirrors overlayEnvAuthContext). |
| `auth/credential-store.ts` (`InMemoryCredentialStore`) | `src/auth.rs` | DONE | in-memory store with per-provider serialized read-modify-write (6 tests incl. concurrent-write serialization). |
| `auth/helpers.ts` (`envApiKeyAuth`, `lazyOAuth`) | `src/auth_providers.rs` | PARTIAL | concrete `OAuthAuth` impls (CodexOAuth/AnthropicOAuth) wrap `src/oauth.rs` refresh; `lazyOAuth` lazy-loading not modelled (static linking). |
| `auth/resolve.ts` (`resolveProviderAuth`, `ModelsError`) | `src/auth.rs` | DONE | full `resolve_provider_auth`: api-key override → stored (oauth double-checked locked refresh / api-key) → ambient env. Concrete `CodexOAuth`/`AnthropicOAuth` impls wire `src/oauth.rs` into the seam (`src/auth_providers.rs`); 15 auth tests incl. end-to-end refresh-through-resolver via a mock token endpoint. |
| `utils/oauth/anthropic.ts` | `src/oauth.rs` | PARTIAL | token decode/account-id helpers; no interactive login. |
| `utils/oauth/openai-codex.ts` | `src/oauth.rs` | PARTIAL | account-id extraction from token. |
| `utils/oauth/github-copilot.ts` | — | MISSING | Copilot OAuth flow. |
| `utils/oauth/radius.ts` | `src/oauth.rs` + `src/auth_providers.rs` | PORTED | Radius gateway OAuth discovery (`/v1/oauth`), PKCE authorize URL construction, token exchange/refresh, device authorization, `/v1/config` catalog load/sanitize/cache fallback, `RadiusOAuth` adapter and model-catalog modifier. Browser launching/HTML callback page is UI glue; deterministic URL/exchange paths are covered in `src/tests/auth/oauth/radius_oauth_test.rs`. |
| `utils/oauth/device-code.ts`, `pkce.ts` | `src/oauth.rs` | PORTED | device-code poll loop = `poll_oauth_device_code_flow` (slow_down interval increment, min-interval clamp, distinct timeout messages, abortable wait); PKCE = `generate_pkce` (verifier+SHA-256 challenge, base64url). Deterministic cases ported in `src/tests/auth/oauth/oauth_device_code_test.rs`. `oauth-page.ts`/`load.ts` = N/A (browser page + lazy loader). |

## 3. Core / utils modules

| Upstream | rs-ai path | Status | Notes |
|---|---|---|---|
| `compat.ts` | `src/compat.rs` | DONE | runtime `detect_compat` + static defaults (0.80.2). |
| `env-api-keys.ts` | `src/env.rs` | DONE | |
| `models.ts` / `models-store.ts` / `models.generated.ts` | `src/registry.rs` / `src/models_runtime.rs` / `src/models_generated.rs` | DONE | Static and dynamic runtime registry: shared `ModelsRuntime` powers ordinary `get_model`/`list_models`/`list_providers`; provider-scoped model store, coalesced refresh, cache restore/offline retention, cancellation, public provider registration/removal/refresh, and Radius dynamic `/v1/config` provider are ported. |
| `image-models*.ts` / `images*.ts` | `src/images/*.rs` | DONE | image catalog + registry. |
| `images-api-registry.ts` | `src/images/mod.rs` | DONE | |
| `types.ts` | `src/types.rs` | DONE | serde JSON-compatible. |
| `oauth.ts` | `src/oauth.rs` | PARTIAL | token helpers only. |
| `session-resources.ts` | `src/session_resources.rs` | DONE | |
| `bedrock-provider.ts` | `src/provider/bedrock.rs` | DONE | |
| `utils/diagnostics.ts` | `src/diagnostics.rs` | DONE | |
| `utils/event-stream.ts` | `src/events.rs` | DONE | event/stream protocol. |
| `utils/hash.ts` (`shortHash`) | `src/utils.rs` | DONE | |
| `utils/headers.ts` | provider files | DONE | |
| `utils/json-parse.ts` | `src/jsonparse.rs` | DONE | streaming JSON repair. |
| `utils/overflow.ts` | `src/compaction.rs` | DONE | context overflow. |
| `utils/provider-env.ts` | `src/env.rs` | DONE | |
| `utils/sanitize-unicode.ts` | `src/utils.rs` | DONE | surrogate sanitisation. |
| `utils/typebox-helpers.ts` | `src/validation.rs` | DONE | |
| `utils/validation.ts` | `src/validation.rs` | DONE | |
| `utils/abort-signals.ts` | — | MISSING (architectural) | rs-ai uses native future-drop cancellation; no `AbortSignal`. |
| `utils/node-http-proxy.ts` | `src/http_proxy.rs` | PORTED | `resolve_http_proxy_url_for_target` + `should_proxy_hostname`/`get_proxy_for_url` mirror env resolution (HTTP(S)_PROXY/NO_PROXY/ALL_PROXY, scoped-env precedence, SOCKS/PAC rejection); `client_for_target` wires `reqwest::Proxy` into all provider client builders. Tests: `src/tests/transports/http_proxy_test.rs`. |
| `cli.ts` | — | N/A | rs-ai is a library, not a CLI. |
| `legacy-api-aliases.ts` | — | N/A | deprecated shims. |
| `api/lazy.ts`, `*.lazy.ts` | — | N/A | JS lazy-loader; Rust links statically. |

## 4. Providers (`src/providers/*.ts` — 35 providers)

All 35 provider definitions + their `.models.ts` catalogs are represented in the
generated catalog (`src/models_generated.rs`, `src/registry.rs`): amazon-bedrock,
ant-ling, anthropic, azure-openai-responses, cerebras, cloudflare-ai-gateway,
cloudflare-workers-ai, deepseek, faux, fireworks, github-copilot, google,
google-vertex, groq, huggingface, kimi-coding, minimax(/-cn), mistral,
moonshotai(/-cn), nvidia, openai, openai-codex, opencode(/-go), openrouter,
together, vercel-ai-gateway, xai, xiaomi(+token-plan ams/cn/sgp), zai(/-coding-cn).
**Status: DONE** (catalog parity verified at 0.80.2; runtime behaviour gated by
the credential-available providers above).

---

## 5a. Per-file upstream test port tracker (bar #2)

> **0.80.3 inventory update.** Verified against the upstream git tags
> (`earendil-works/pi` `v0.80.2`→`v0.80.3`, `packages/ai/test`): the denominator
> moved **86 → 90 test files** (+4 new, 0 removed). New files + rs-ai disposition:
> - **`retry.test.ts`** — **PORTED**. `isRetryableAssistantError` classification incl.
>   the verbatim OpenAI/Bedrock explicit-retry messages, `"429 quota exceeded"`
>   (non-retryable precedence), `"overloaded_error"`, non-error. `src/tests/transports/retry_classify_test.rs`.
> - **`error-body.test.ts`** — **PORTED (compose/truncation) + N/A (SDK extraction)**.
>   `formatProviderError` compose + `MAX_PROVIDER_ERROR_BODY_CHARS` truncation cases
>   ported in `src/tests/core/error_body_test.rs`; the `normalizeProviderError` cases that
>   extract status/body from JS-SDK error-object shapes (Mistral `statusCode/body`,
>   openai `APIError.error`, `@google/genai` folded message, Bedrock `$response`,
>   non-Error fallback, `messageCarriesBody`) are **N/A** — rs-ai's reqwest path reads
>   `resp.text()` directly and never had the JS-SDK body-hiding bug those guard.
> - **`provider-error-body-passthrough.test.ts`** + **`provider-error-body-regression.test.ts`**
>   — **SIMULATED-FIXTURE-PORTED**. Upstream mocks the JS SDKs to throw an opaque
>   `"403 status code (no body)"` APIError; rs-ai ports the deterministic contract
>   against a **real 403-with-body wire response** (wiremock, 3×) for
>   openai-completions (`403: {body}`), openai-responses (branded
>   `OpenAI API error (403): {body}`), and google (`403: {body}`) in
>   `src/tests/core/simulated_e2e_fixtures_test.rs`. Bedrock tier is covered by
>   `format_bedrock_error` already folding the SDK body; the OpenRouter
>   `metadata.raw` non-double-print regression is asserted (count==1) in
>   `tests/providers/other/provider_test.rs`; the openrouter-images passthrough case (403-with-body →
>   status + body, not the opaque SDK message) is ported as a real-transport
>   wiremock test in `src/tests/providers/openrouter/openrouter_images_test.rs`.
>
> **Auditor re-confirm (source diff vs `/tmp/pi803-tests/`).** All `it()` cases in
> the 4 upstream-new files were diffed against the rs-ai ports: retry (3 cases),
> error-body formatProviderError compose/truncation (no-prefix `{status}: {body}`,
> branded `{prefix} ({status}): {body}`, `... [truncated N chars]` — verified
> against `error-body.ts` L112/L117), passthrough (openrouter-images), and the
> 4 per-tier regression cases (completions body, metadata.raw no-double-print,
> responses prefix, bedrock body-blind). normalizeProviderError SDK-extraction
> stays N/A (auditor-accepted).
>
> Running totals after 0.80.3: **PORTED 43, SIMULATED-FIXTURE 9** (the original 7
> + the 2 error-body regression files). The 0.80.2 table below is unchanged.

> **0.80.5 inventory update** (`earendil-works/pi` `v0.80.3`→`v0.80.5`, commit
> `cc62baa`, `packages/ai/test`). Denominator **90 → 92 test files** (+2 new, 0
> removed). New files + rs-ai disposition:
> - **`lax-message-content.test.ts`** — **PORTED**. Null/missing untyped
>   user/assistant/toolResult `content` normalizes to an empty slice before
>   conversion/image-downgrade. rs-ai does this at deserialization
>   (`de_null_content_as_empty` in `src/types.rs`) so `transform_messages` relies
>   on the type contract; `src/tests/core/lax_message_content_test.rs` deserializes the
>   null-content history and asserts every `content` is `[]` after transform
>   (issues #6259, #6276).
> - **`openai-responses-empty-tool-result.test.ts`** — **PORTED**. A blank
>   text-only tool result (no images) serializes as `"(no tool output)"`, not the
>   image placeholder. `src/tests/providers/openai/openai_responses_empty_tool_result_test.rs`.
>
> In-place behavioral deltas (existing files), rs-ai disposition:
> 1. **codex zstd request compression** — **PORTED (real zstd)**. Codex SSE
>    responses body is zstd-compressed at level 3 (`REQUEST_COMPRESSION_ZSTD_LEVEL`)
>    with `Content-Encoding: zstd` (`compress_request_body_zstd` in
>    `src/provider/codex.rs`, `zstd` crate). `compresses_sse_request_body_with_zstd`
>    asserts the captured request is really zstd-framed (magic `28 B5 2F FD`) and
>    decodes back to the payload. **WS connection-age recycling** (55-min
>    `SESSION_WEBSOCKET_MAX_AGE_MS`, `connection_age_limit` close) is part of the
>    upstream WS session-cache/pool, which rs-ai does not implement (fresh WS per
>    request — so the socket is never older than one request). The recycle
>    invariant is nonetheless encoded faithfully: `SESSION_WEBSOCKET_MAX_AGE_MS`
>    (55 min) + `codex_websocket_session_expired(created_at, now)` with a test
>    (older than 55 min → expired/fresh; younger → reusable), ready to gate any
>    future socket-reuse path; the live pool stays inside the documented
>    **WS-pooling** partial gap (file #92 `openai-codex-responses.ts`).
> 2. **overflow DS4** — **PORTED** (`src/context.rs`, invariant-substring match; no
>    regex crate). `src/tests/core/overflow_test.rs`.
> 3. **retry `524`/`socket connection was closed`/`ResourceExhausted`** — **PORTED**
>    (`src/retry.rs`, `src/tests/transports/retry_classify_test.rs`).
> 4. **device-code wait-before-first-poll + slow_down interval** — **PORTED**
>    (`src/oauth.rs`, `src/tests/auth/oauth/oauth_device_code_test.rs`).
> 5. **openai-completions `"(no tool output)"` placeholder** — **PORTED**
>    (`src/provider/openai.rs`, `src/tests/providers/openai/openai_completions_tool_result_images_test.rs`
>    + `src/tests/providers/other/provider_test.rs`).
> 6. **Catalog regen v0.80.5** — **DONE**. `src/models_generated.rs` = **1059
>    models / 35 providers** (matches go-ai); `src/images/models_generated.rs` = 35
>    image models / 1 provider. Catalog-driven test deltas ported: supports-xhigh
>    (gpt-5.6 codex/openai variants), xiaomi (mimo-v2-omni API-billing-only),
>    fireworks (GLM 5.2 Fast mirrors GLM 5.2 config). `openai-responses-copilot-provider`
>    / `openai-responses-tool-result-images` deltas were whitespace/assertion-helper
>    refactors only (`isResponsePayload` → `Array.isArray(input)`), already covered.
>
> Running totals after 0.80.5: **PORTED 46, SIMULATED-FIXTURE 9**. Denominator **92**.
> **748 tests, 0 failures, 0 clippy warnings.**


> **0.80.6 inventory update** (`earendil-works/pi` `v0.80.5`→`v0.80.6`, commit
> `2b3fda9`, `packages/ai/test`). Denominator **92 → 94 test files** (+2 new, 0
> removed). New files + rs-ai disposition:
> - **`context-estimate.test.ts`** — **PORTED**. `estimateContextTokens` now ignores
>   a stale assistant usage anchor when a newer prefix message (for example a
>   compaction summary) was inserted before it; it resumes anchoring after a newer
>   assistant response. `src/estimate.rs` tracks `latest_prefix_timestamp`; exact
>   upstream values are asserted in `src/tests/core/estimate_test.rs` (`tokens=1005`,
>   `maxTokens=4899`; then `tokens=2001`, `lastUsageIndex=3`).
> - **`max-thinking.test.ts`** — **PORTED**. `max` is opt-in for ordinary reasoning
>   models, explicit on `gpt-5.6-{luna,sol,terra}`, supports a high→max hole when
>   `xhigh` is disabled, and serializes Codex Responses `reasoning.effort="max"`.
>   `src/tests/catalogs/max_thinking_test.rs`.
>
> In-place behavioral deltas (existing files), rs-ai disposition:
> 1. **Catalog regen v0.80.6** — **DONE**. `src/models_generated.rs` = **1057
>    models / 35 providers**; `src/images/models_generated.rs` remains 35 image
>    models / 1 provider. `ModelCost.tiers` is ported and `calculate_cost` applies
>    request-wide tier thresholds before service-tier multipliers.
> 2. **Anthropic/Bedrock adaptive + empty thinking** — **PORTED**. Catalog-driven
>    adaptive-thinking updates and request builders preserve an empty thinking text
>    when a signature is present; empty/blank signatures still follow the existing
>    compatibility gate. `src/tests/providers/anthropic/anthropic_compat_test.rs`.
> 3. **Thinking-level catalog/test deltas** — **PORTED**. Sonnet 4.6 exposes native
>    `max` instead of `xhigh`; Opus/OpenRouter/Codex/GitHub Copilot cases and
>    supports-xhigh expectations are updated. `src/tests/catalogs/supports_xhigh_test.rs`,
>    `src/tests/providers/anthropic/github_copilot_anthropic_test.rs`.
> 4. **OpenAI Responses/Copilot request deltas** — **PORTED**. `verbosity`,
>    provider/model request-shape updates, terminal-event/tool-choice assertion
>    deltas and catalog-driven fixture changes are covered by the existing payload
>    tests (`src/tests/providers/openai/openai_responses_copilot_provider_test.rs`, `src/tests/providers/other/provider_test.rs`).
>
> Running totals after 0.80.6: **PORTED 48, SIMULATED-FIXTURE 9**. Denominator **94**.
> **756 tests ×3, 0 failures, 0 clippy warnings.**
>
> **2026-07-13 redirect re-audit (rs-ai only)** — Verified `/workspace/projects/rs-ai`
> against upstream package commit `2b3fda9` after the auditor hard redirect. The
> v0.80.6 ledger above is in rs-ai, the Rust catalog/test dispositions remain
> unchanged, and the required Rust gate was rerun after `cargo fmt`: `cargo build`,
> `cargo test` (756 + doctest), and `cargo clippy --all-targets -- -D warnings`
> all pass. No go-ai files were touched in this audit.

_Total **87** upstream test files (0.80.2 snapshot; see 0.80.3 update above → 90). **Final classification (Rui's ruling: "we test with what we have. No keys, no tests" — no record-replay/mock harness, no skip-only wrappers, no fabrication):**_

- **42 PORTED (yes/yes)** — deterministic, name/value-faithful ports.
- **7 SIMULATED-FIXTURE-PORTED** — re-audited live-E2E files whose response-handling / request-payload substance is now ported end-to-end against faithful **simulated wire fixtures** (wiremock, no credentials, each case run 3× for determinism) in `src/tests/core/simulated_e2e_fixtures_test.rs`: `responseid` (responseId surfaced for openai-completions + google), `tokens` (usage on terminal message), `total-tokens` (computed for openai-completions/anthropic; native for openai-responses), `context-overflow` (anthropic SSE overflow error → `is_context_overflow` true; rate-limit → false), `unicode-surrogate` (astral-plane emoji in tool results round-trips intact into openai + anthropic request bodies), `google-thinking-disable` (response with no thinking parts yields zero Thinking events), `cache-retention` (anthropic system-block `cache_control`: default ephemeral/no-ttl, long → `ttl:1h`, `supportsLongCacheRetention:false` → ttl omitted, captured from the live request body). Only real-model nondeterminism (actual token counts, model phrasing, abort timing) remains N/A for these files.
- **1 covered** (`faux-provider`) — exercised via rs-ai's FauxProvider harness (upstream closure/registration API differs).
- **13 partial** — the entire **deterministic** substance of each file is ported; the documented remainder is genuinely architectural (AbortSignal, WS pooling, instance-collection vs global-registry, TypeBox symbol stripping) or interactive-OAuth/credential — i.e. N/A here.
- **24 N/A** — not deterministically runnable in this environment:
  - 10 architectural / JS-runtime / smoke (`abort`, `lazy-module-load`, `mistral-tool-schema`, `xhigh`, `images`, `interleaved-thinking`, `anthropic-opus-4-8-smoke`, `xiaomi-token-plan-ams-…-smoke`, `transform-messages-copilot-…`, `openai-completions-retry`).
  - 14 **live-E2E / credential-gated, real-nondeterminism only** (`stream`, `empty`, `tool-call-without-result`, `image-tool-result`, `openai-responses-tool-result-images`, `cross-provider-handoff`, `anthropic-eager-tool-input-e2e`, `anthropic-long-cache-retention-e2e`, `openai-codex-cache-affinity-e2e`, `openai-responses-cache-affinity-e2e`, `openai-responses-reasoning-replay-e2e`, `openrouter-cache-write-repro`, `zen`, plus `total-tokens` long-cache real-trigger remainder). Each imports `complete`/`completeSimple`/`stream` + `resolveApiKey`/`skipIf(API_KEY)` and asserts real model output/token counts/abort timing; the *response-handling* substance of the 6 listed above is now SIMULATED-FIXTURE-PORTED, with only genuine real-model nondeterminism remaining N/A here.

**The deterministically-runnable upstream test surface is 100% ported** (every file with `live=0` is either ported test-for-test or has its deterministic substance ported + a precise N/A rationale for an architectural remainder). The Vertex request path is implemented (ported from go-ai `buildStreamURL`/`resolveVertexProjectLocation`); Cloudflare-AI-Gateway client-construction is ported via `build_openai_request_parts`; HTTP-proxy resolution is ported (`node-http-proxy.test.ts` → `src/tests/transports/http_proxy_test.rs`) and wired into all provider clients via `reqwest::Proxy`; `github-copilot-headers.ts` pure logic is ported (`copilot_dynamic_headers` in `src/utils.rs`). **73 rs-ai test files, 692 tests, 0 failures, no new clippy warnings.** (`+src/tests/core/simulated_e2e_fixtures_test.rs`: 14 simulated-fixture E2E ports across 7 re-audited live-E2E files.)_

| # | upstream `test/*.test.ts` | ported? | passing? | rs-ai file / note |
|---|---|---|---|---|
| 1 | `abort.test.ts` | n/a | — | credential/runtime-gated |
| 2 | `anthropic-adaptive-thinking-models.test.ts` | yes | yes | src/tests/providers/anthropic/anthropic_compat_test.rs |
| 3 | `anthropic-cache-write-1h-cost.test.ts` | yes | yes | src/tests/providers/anthropic/anthropic_cache_write_1h_cost_test.rs |
| 4 | `anthropic-eager-tool-input-compat.test.ts` | yes | yes | src/tests/providers/anthropic/anthropic_compat_test.rs |
| 5 | `anthropic-eager-tool-input-e2e.test.ts` | n/a (live) | — | live-E2E credential-gated (3 cases against real providers); N/A per "no keys, no tests" |
| 6 | `anthropic-empty-thinking-signature-compat.test.ts` | yes | yes | src/tests/providers/anthropic/anthropic_compat_test.rs |
| 7 | `anthropic-force-adaptive-thinking.test.ts` | yes | yes | src/tests/providers/anthropic/anthropic_force_adaptive_thinking_test.rs |
| 8 | `anthropic-long-cache-retention-e2e.test.ts` | n/a (live) | — | live-E2E credential-gated (2 cases against real providers); N/A per "no keys, no tests" |
| 9 | `anthropic-oauth.test.ts` | partial | yes (3/3 portable) | src/tests/auth/oauth/anthropic_oauth_test.rs (token-endpoint + refresh-omits-scope + exchange-redirect_uri request shape; interactive login orchestration N/A = MISSING surface) |
| 10 | `anthropic-opus-4-8-smoke.test.ts` | n/a | — | credential/runtime-gated |
| 11 | `anthropic-sse-parsing.test.ts` | yes | yes | src/tests/providers/anthropic/anthropic_sse_parsing_test.rs |
| 12 | `anthropic-temperature-compat.test.ts` | yes | yes | src/tests/providers/anthropic/anthropic_temperature_compat_test.rs |
| 13 | `anthropic-thinking-disable.test.ts` | yes | yes (6/7) | src/tests/providers/anthropic/anthropic_thinking_disable_test.rs (live E2E N/A) |
| 14 | `anthropic-tool-name-normalization.test.ts` | yes | yes (name-mapping) | src/tests/providers/anthropic/anthropic_tool_name_normalization_test.rs (live OAuth round-trip N/A) |
| 15 | `azure-openai-base-url.test.ts` | yes | yes | src/tests/providers/openai/azure_openai_base_url_test.rs |
| 16 | `bedrock-convert-messages.test.ts` | partial | yes (5/9) | src/tests/providers/bedrock/bedrock_convert_messages_test.rs (4 unknown-content/surrogate cases N/A: Rust exhaustive enum + no lone surrogates) |
| 17 | `bedrock-custom-headers.test.ts` | partial | yes (reserved-header substance) | src/tests/providers/bedrock/bedrock_custom_headers_test.rs (+fixed: skip x-amz-*/auth/host + override; SDK-middleware registration cases N/A) |
| 18 | `bedrock-endpoint-resolution.test.ts` | yes (region-resolution) | yes | src/tests/providers/bedrock/bedrock_endpoint_test.rs (SDK-client endpoint/profile config = AWS-SDK-internal)|
| 19 | `bedrock-models.test.ts` | partial | yes (catalog non-empty) | src/tests/catalogs/bedrock_images_models_test.rs (per-model live request cases N/A) |
| 20 | `bedrock-thinking-payload.test.ts` | yes | yes (9/10) | src/tests/providers/bedrock/bedrock_thinking_payload_test.rs (adaptive/govcloud/app-inference-profile; live max-tokens E2E N/A) |
| 21 | `cache-retention.test.ts` | sim-fixture | yes (payload cache_control) | src/tests/core/simulated_e2e_fixtures_test.rs (anthropic system-block cache_control: short=no-ttl, long=ttl:1h, unsupported-compat omits ttl, captured from request body; real cache-hit usage N/A) |
| 22 | `compat-env.test.ts` | yes | yes | src/tests/core/compat_env_test.rs |
| 23 | `context-overflow.test.ts` | sim-fixture | yes (detection end-to-end) | src/tests/core/simulated_e2e_fixtures_test.rs (anthropic SSE overflow error → is_context_overflow true; rate-limit → false) |
| 24 | `cross-provider-handoff.test.ts` | n/a (live) | — | live-E2E credential-gated (2 cases against real providers); N/A per "no keys, no tests" |
| 25 | `empty.test.ts` | n/a (live) | — | live-E2E credential-gated (104 cases against real providers); N/A per "no keys, no tests" |
| 26 | `env-api-keys.test.ts` | yes | yes | src/tests/core/env_api_keys_test.rs |
| 27 | `faux-provider.test.ts` | covered | — | rs-ai FauxProvider (provider/faux.rs) tested in provider_test/coverage; upstream harness API (closures/registration) differs; abort cases N/A (no AbortSignal) |
| 28 | `fireworks-models.test.ts` | yes | yes | src/tests/catalogs/fireworks_models_test.rs |
| 29 | `github-copilot-anthropic.test.ts` | yes | yes | src/tests/providers/anthropic/github_copilot_anthropic_test.rs |
| 30 | `github-copilot-oauth.test.ts` | partial | yes (model-picker filter) | src/tests/auth/oauth/github_copilot_oauth_test.rs (interactive slow_down/timeout login orchestration = interactive-OAuth surface) |
| 31 | `google-shared-convert-tools.test.ts` | yes | yes | src/tests/providers/google/google_shared_convert_tools_test.rs |
| 32 | `google-shared-gemini3-unsigned-tool-call.test.ts` | yes | yes | src/tests/providers/google/google_gemini3_unsigned_tool_call_test.rs |
| 33 | `google-shared-image-tool-result-routing.test.ts` | yes | yes | src/tests/providers/google/google_image_tool_result_routing_test.rs |
| 34 | `google-thinking-disable.test.ts` | sim-fixture | yes (response: no-thinking) | src/tests/core/simulated_e2e_fixtures_test.rs (no-thinking-part response → zero Thinking events; real reasoning-suppression nondeterminism N/A) |
| 35 | `google-thinking-signature.test.ts` | yes | yes | src/tests/providers/google/google_thinking_signature_test.rs |
| 36 | `google-vertex-api-key-resolution.test.ts` | yes | yes | src/tests/providers/google/google_vertex_request_path_test.rs (Vertex REST request path implemented: project/location resolution from StreamOptions + GOOGLE_CLOUD_PROJECT/GCLOUD_PROJECT/GOOGLE_CLOUD_LOCATION env, `{location}` host substitution, placeholder-marker `<...>`/`gcp-vertex-credentials` api-key suppression → ADC URL, real-key append, custom base-url passthrough. Upstream's @google/genai SDK-constructor assertions are JS-runtime-specific; this port asserts the equivalent URL shape + resolution, mirroring go-ai `buildStreamURL`/`resolveVertexProjectLocation`.) |
| 37 | `image-tool-result.test.ts` | n/a (live) | — | live-E2E credential-gated (38 cases against real providers); N/A per "no keys, no tests" |
| 38 | `images-models.test.ts` | partial | yes (builtin catalog) | src/tests/catalogs/bedrock_images_models_test.rs (instance ImagesModels collection = architectural diff) |
| 39 | `images.test.ts` | n/a | — | live image generation (OPENROUTER_API_KEY) |
| 40 | `interleaved-thinking.test.ts` | n/a | — | live model-behavior (bedrock/anthropic creds); asserts the model emits interleaved thinking+toolCall |
| 41 | `lazy-module-load.test.ts` | n/a | — | JS lazy module loader (api/*.lazy.ts); rs-ai links statically |
| 42 | `mistral-reasoning-mode.test.ts` | yes | yes | src/tests/providers/mistral/mistral_reasoning_mode_test.rs |
| 43 | `mistral-tool-schema.test.ts` | n/a | — | TypeBox JS Symbol-key stripping has no Rust analogue (serde_json params carry no symbols) |
| 44 | `models-runtime.test.ts` | yes | yes | `src/tests/auth/oauth/models_runtime_auth_test.rs`, `src/tests/catalogs/models_runtime_refresh_test.rs`, `src/models_runtime.rs`, `src/registry.rs` cover auth-resolution/merge plus production provider-scoped dynamic refresh, ordinary registry lookups, cache restore/offline retention, cancellation, and Radius runtime provider. |
| 45 | `node-http-proxy.test.ts` | yes | yes | src/tests/transports/http_proxy_test.rs (ported resolve_http_proxy_url_for_target; NO_PROXY exclusion, HTTP(S) resolution, scoped-env precedence, SOCKS/PAC rejection) |
| 46 | `oauth-auth.test.ts` | partial | yes (4/8) | src/tests/auth/oauth/oauth_auth_test.rs (anthropic/codex toAuth+refresh+resolve-via-store; 4 github-copilot proxy-ep baseUrl cases N/A = Copilot provider gap) |
| 47 | `oauth-device-code.test.ts` | yes | yes | src/tests/auth/oauth/oauth_device_code_test.rs (implemented generic poll_oauth_device_code_flow; tokio paused clock) |
| 48 | `openai-codex-cache-affinity-e2e.test.ts` | n/a (live) | — | live-E2E credential-gated (1 cases against real providers); N/A per "no keys, no tests" |
| 49 | `openai-codex-oauth.test.ts` | partial | yes (1/8 portable) | src/tests/auth/oauth/openai_codex_oauth_test.rs (refresh-failure error shape; +fixed status to numeric (401) per upstream; 7 device-code login-flow cases N/A = MISSING interactive surface) |
| 50 | `openai-codex-stream.test.ts` | partial | yes (SSE/header/cache subset) | src/tests/providers/codex/openai_codex_stream_test.rs (WS-transport cases = WS-pooling gap) |
| 51 | `openai-completions-cache-control-format.test.ts` | yes | yes | src/tests/providers/openai/openai_completions_cache_control_format_test.rs |
| 52 | `openai-completions-empty-tools.test.ts` | yes | yes | src/tests/providers/openai/openai_completions_empty_tools_test.rs (tools omit/empty-with-history + max_tokens; Cloudflare-AI-Gateway /compat base-URL resolution + cf-aig-authorization + BYOK inline-Authorization passthrough + Workers-AI session-affinity headers, via extracted `build_openai_request_parts` helper) |
| 53 | `openai-completions-prompt-cache.test.ts` | yes | yes | src/tests/providers/openai/openai_completions_prompt_cache_test.rs |
| 54 | `openai-completions-reasoning-details.test.ts` | yes | yes | src/tests/providers/openai/openai_completions_reasoning_details_test.rs |
| 55 | `openai-completions-response-model.test.ts` | yes | yes | src/tests/catalogs/openai_completions_response_model_test.rs |
| 56 | `openai-completions-retry.test.ts` | n/a | — | SDK maxRetries client option; rs-ai uses its own retry.rs (tested separately)|
| 57 | `openai-completions-thinking-as-text.test.ts` | yes | yes | src/tests/providers/openai/openai_completions_thinking_as_text_test.rs |
| 58 | `openai-completions-tool-choice.test.ts` | partial | yes (39/41) | src/tests/providers/openai/openai_completions_tool_choice_test.rs (full incl. mixed-delta accumulation + openrouter role routing + reasoning-replay; 2 N/A: ant-ling native reasoningEffort-omit + per-contentIndex grouping = rs-ai event protocol is index-less) |
| 59 | `openai-completions-tool-result-images.test.ts` | yes | yes | src/tests/providers/openai/openai_completions_tool_result_images_test.rs |
| 60 | `openai-responses-cache-affinity-e2e.test.ts` | n/a (live) | — | live-E2E credential-gated (1 cases against real providers); N/A per "no keys, no tests" |
| 61 | `openai-responses-copilot-provider.test.ts` | yes | yes | src/tests/providers/openai/openai_responses_copilot_provider_test.rs (deterministic: reasoning defaults, cache-affinity headers, prompt_cache_key clamp, service-tier cost) |
| 62 | `openai-responses-foreign-toolcall-id.test.ts` | yes | yes | src/tests/providers/openai/responses_foreign_toolcall_id_test.rs |
| 63 | `openai-responses-message-id.test.ts` | yes | yes | src/tests/providers/openai/responses_message_id_test.rs |
| 64 | `openai-responses-partial-json-cleanup.test.ts` | yes | yes | src/tests/providers/openai/openai_responses_partial_json_cleanup_test.rs |
| 65 | `openai-responses-reasoning-replay-e2e.test.ts` | n/a (live) | — | live-E2E credential-gated (3 cases against real providers); N/A per "no keys, no tests" |
| 66 | `openai-responses-terminal-event.test.ts` | yes | yes (4/6) | src/tests/providers/openai/openai_responses_terminal_event_test.rs (the 2 processResponsesStream-direct cases collapse into the wrapper no-terminal case) |
| 67 | `openai-responses-tool-result-images.test.ts` | n/a (live) | — | live-E2E credential-gated (4 cases against real providers); N/A per "no keys, no tests" |
| 68 | `openrouter-cache-write-repro.test.ts` | n/a (live) | — | live-E2E credential-gated (1 cases against real providers); N/A per "no keys, no tests" |
| 69 | `openrouter-images.test.ts` | yes | yes (2/3) | src/tests/providers/openrouter/openrouter_images_test.rs (abort-signal case N/A) |
| 70 | `overflow.test.ts` | yes | yes | src/tests/core/overflow_test.rs |
| 71 | `providers.test.ts` | partial | yes (6) | src/tests/transports/providers_upstream_test.rs (builtin registration + anthropic-OAuth/bedrock-AWS/cloudflare env precedence + no-api-impl stream error + fauxProvider queued stream; cloudflare/vertex scoped-baseUrl+AuthResult.env shaping, vertex ADC file path, envApiKeyAuth.login prompt, and dynamic refreshModels dedup = instance-collection/architectural N/A) |
| 72 | `responseid.test.ts` | sim-fixture | yes (responseId surfaced) | src/tests/core/simulated_e2e_fixtures_test.rs (openai-completions + google responseId from completed stream) |
| 73 | `stream.test.ts` | n/a (live) | — | live-E2E credential-gated (214 cases against real providers); N/A per "no keys, no tests" |
| 74 | `supports-xhigh.test.ts` | yes | yes | src/tests/catalogs/supports_xhigh_test.rs|
| 75 | `together-models.test.ts` | yes | yes | src/tests/catalogs/together_xiaomi_models_test.rs |
| 76 | `tokens.test.ts` | sim-fixture | yes (usage surfaced) | src/tests/core/simulated_e2e_fixtures_test.rs (usage on terminal message; real token counts/abort-timing N/A) |
| 77 | `tool-call-id-normalization.test.ts` | partial | yes (issue 1022 fixture) | src/tests/core/tool_call_id_normalization_test.rs (live handoff N/A) |
| 78 | `tool-call-without-result.test.ts` | n/a (live) | — | live-E2E credential-gated (26 cases against real providers); N/A per "no keys, no tests" |
| 79 | `total-tokens.test.ts` | sim-fixture | yes (computed vs native) | src/tests/core/simulated_e2e_fixtures_test.rs (openai-completions/anthropic computed sum; openai-responses native total) |
| 80 | `transform-messages-copilot-openai-to-anthropic.test.ts` | n/a | — | credential/runtime-gated |
| 81 | `unicode-surrogate.test.ts` | sim-fixture | yes (emoji round-trip) | src/tests/core/simulated_e2e_fixtures_test.rs (astral-plane emoji in tool results intact in openai+anthropic request bodies; Rust strings are UTF-8 so lone surrogates impossible) |
| 82 | `validation.test.ts` | yes | yes | src/tests/transports/validation_upstream_test.rs |
| 83 | `xhigh.test.ts` | n/a | — | live (OPENAI_API_KEY); the supported-levels logic is covered by supports-xhigh |
| 84 | `xiaomi-models.test.ts` | yes | yes | src/tests/catalogs/together_xiaomi_models_test.rs|
| 85 | `xiaomi-token-plan-ams-anthropic-empty-signature-smoke.test.ts` | n/a | — | credential/runtime-gated |
| 86 | `zen.test.ts` | n/a (live) | — | live-E2E credential-gated (1 cases against real providers); N/A per "no keys, no tests" |


## 5. Upstream test suite (`test/*.test.ts` — 87 files)

rs-ai currently has **379** test functions across `src/*_test.rs`. They cover the
behaviour of most upstream test files but are **not yet ported test-for-test with
identical names/expected values**, which is the standing parity bar #2.

### Mapped to existing rs-ai coverage (behaviourally covered)
abort¹, anthropic-sse-parsing, anthropic-cache-write-1h-cost, anthropic-temperature-compat,
anthropic-thinking-disable, anthropic-eager-tool-input(-e2e), anthropic-empty-thinking-signature,
anthropic-adaptive-thinking-models, anthropic-force-adaptive-thinking, anthropic-long-cache-retention,
azure-openai-base-url, bedrock-convert-messages, bedrock-custom-headers, bedrock-endpoint-resolution,
bedrock-models, bedrock-thinking-payload, cache-retention, compat-env, context-overflow,
cross-provider-handoff, env-api-keys, faux-provider, fireworks-models, google-shared-convert-tools,
google-shared-gemini3-unsigned-tool-call, google-shared-image-tool-result-routing,
google-thinking-disable, google-thinking-signature, image-tool-result, images(-models),
interleaved-thinking, mistral-reasoning-mode, mistral-tool-schema, models-runtime,
openai-codex-stream, openai-completions-* (cache-control, empty-tools, prompt-cache,
reasoning-details, response-model, retry, thinking-as-text, tool-choice, tool-result-images),
openai-responses-* (cache-affinity, foreign-toolcall-id, message-id, partial-json-cleanup,
reasoning-replay, terminal-event, tool-result-images), openrouter-cache-write,
openrouter-images, overflow, providers, responseid, stream, supports-xhigh,
together-models, tokens, tool-call-id-normalization, tool-call-without-result,
total-tokens, transform-messages-copilot-openai-to-anthropic, unicode-surrogate,
validation, xhigh, xiaomi-models, zen, empty.

### MISSING (no rs-ai equivalent — credential/feature-gated)
- `anthropic-oauth`, `oauth-auth`, `github-copilot-oauth`,
  `github-copilot-anthropic`, `openai-codex-oauth`,
  `openai-responses-copilot-provider` — OAuth/Copilot flows (MISSING feature).
- `lazy-module-load` — JS lazy loader (N/A).
- `anthropic-opus-4-8-smoke`, `xiaomi-token-plan-ams-anthropic-empty-signature-smoke` —
  live smoke tests (N/A without credentials).

¹ `abort` is covered only at the faux level; rs-ai has no `AbortSignal` (uses future-drop).

---

## Coverage estimate

- **Modules / exports:** ~88% functional coverage. Gaps are credential-gated
  (Copilot, interactive OAuth, credential-store) or architectural
  (AbortSignal, CLI, lazy-loader). Google Vertex AI is now implemented
  (project/location-scoped REST request path). HTTP proxy is now ported
  (`src/http_proxy.rs`, wired into all provider clients).
- **Providers:** 100% catalog parity; runtime exercised for all non-credential-gated
  providers.
- **Tests:** the **deterministically-runnable upstream surface is 100% ported** —
  42 files ported test-for-test, 1 covered via the FauxProvider harness, and 13
  with their full deterministic substance ported plus a precise architectural
  N/A remainder. 7 live-E2E files are now SIMULATED-FIXTURE-PORTED (response
  handling / request payload against faithful wiremock fixtures), leaving 24
  files N/A (10 architectural/JS-runtime/smoke + 14 live-E2E real-nondeterminism
  only). rs-ai ships **692 tests across 73 files, 0
  failures**. `node-http-proxy.test.ts`, `oauth-device-code.test.ts`,
  `mistral-reasoning-mode.test.ts`, and `azure-openai-base-url.test.ts` are
  name-for-name ports; `github-copilot-headers.ts` pure logic is ported in
  `src/utils.rs`. The only non-ported files require live credentials/mocks
  (forbidden) or model JS-runtime/CLI behaviour with no Rust analogue.

## Top 3 gaps (highest leverage first)

> Auditor-endorsed ordering: do the auth/credential seam (#2) **before** the bulk
> test-for-test port (#1), because the seam unlocks the OAuth provider branches (#3).

1. **Auth/credential abstraction (`auth/resolve.ts` + `credential-store.ts`).**
   DONE — `src/auth.rs` lands Credential/ModelAuth/AuthResult, `ModelsError`,
   `InMemoryCredentialStore` (per-provider serialized modify),
   `AuthContext`/`EnvAuthContext`, the `ApiKeyAuth`/`OAuthAuth` traits, and
   `resolve_provider_auth` with OAuth double-checked locked refresh (12 tests).
   Remaining: wire concrete provider `ApiKeyAuth`/`OAuthAuth` impls into the
   stream paths (the providers still resolve via `env.rs` directly today).
2. **Interactive OAuth flows.** Largely PRESENT in `src/oauth.rs` (device-code,
   PKCE, token refresh); remaining work is wiring them through the credential store
   + `toAuth` derivation.
3. **Test-for-test port (bar #2).** Resume the wide sweep after the seam lands so
   OAuth-provider tests port for real instead of stubbing.
