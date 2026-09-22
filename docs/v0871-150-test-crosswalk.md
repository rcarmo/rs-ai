# v0.87.1 bounded 150-file test crosswalk

Source: official upstream `@earendil-works/pi-ai` tag `f07218c4d4bbc12bef056a7058c3dd49dfe41abe` (`v0.87.1`). Baseline: accepted `v0.87.0` tag `16787ad5b2dc748047f314ca1bfe7708f30f54f3`.

The bounded release delta changes **16 package paths**, including **9 test paths**, and the final corpus remains **150 unique basenames**. Exact inventories and hashes are validated by `scripts/validate_v0871_manifests.py`. The nine changed test rows below name their production Rust evidence; all other test rows inherit accepted v0.87.0 coverage.

## Changed-path disposition matrix

| Upstream changed path | Disposition | rs-ai evidence | Notes |
|---|---|---|---|
| `packages/ai/CHANGELOG.md` | N/A | Pinned tag/npm provenance and this bounded ledger | Upstream release prose has no runtime surface. Current release publication remains blocked pending acceptance. |
| `packages/ai/README.md` | N/A | Pinned tag/npm provenance and this bounded ledger | Upstream package documentation has no Rust runtime surface. README publication remains blocked pending acceptance. |
| `packages/ai/package.json` | ADAPTED | v0.87.1 package/version/gitHead/tarball pins in the metadata and delta verifiers | Package metadata is represented as audit provenance, not a Rust package-version change. |
| `packages/ai/scripts/generate-models.ts` | ADAPTED | `extract_release_model_shards.py`, complete metadata comparison, exact pair comparator, and reproducibility gate | Rust uses the signed npm provider shards offline rather than upstream's network-capable generator. Seven newly signed batch aliases are allowed exactly. |
| `packages/ai/src/api/anthropic-messages.ts` | ADAPTED | `v0871_release_test::tests::anthropic_oauth_uses_claude_code_2_1_280_user_agent` | Production OAuth request capture verifies `claude-cli/2.1.280`, `x-app: cli`, Bearer auth, and no `x-api-key`. |
| `packages/ai/src/api/openai-completions.ts` | ADAPTED | `openai_completions_tool_result_images_test::tests::{omits_empty_text_parts_from_image_only_user_messages,preserves_whitespace_text_parts_in_multimodal_user_messages}` | Production conversion filters only `text.is_empty()` and preserves whitespace. |
| `packages/ai/src/image-models.generated.ts` | COVERED | Generated image registry plus full metadata/pair/reproducibility gates | Exact signed image delta is `+1/-0/0`; OpenRouter `inclusionai/ming-image-0.1-design` is asserted. |
| `packages/ai/test/cache-retention.test.ts` | ADAPTED | `v0871_release_test::tests::gpt_6_sol_and_luna_catalog_cache_and_default_reasoning_match_upstream` | Sol/Luna production Responses payloads use long-cache TTL options. |
| `packages/ai/test/github-copilot-anthropic.test.ts` | ADAPTED | `github_copilot_anthropic_test::tests::applies_copilot_specific_adaptive_thinking_effort_overrides` and `v0871_release_test` | Copilot Claude Opus 5.5 API, context, and effort levels are pinned. |
| `packages/ai/test/max-thinking.test.ts` | ADAPTED | `max_thinking_test::tests::{exposes_xhigh_and_max_for_openai_codex_current_variants,sends_max_to_the_codex_responses_api}` | Codex Sol/Luna expose xhigh/max and serialize max through the production payload. |
| `packages/ai/test/model-catalog-types.test.ts` | ADAPTED | `v0871_release_test::tests::{grok_4_7_catalog_matches_upstream,claude_opus_5_5_catalog_matches_upstream}` | Exact API, input, context, effort, and pricing metadata are asserted. |
| `packages/ai/test/openai-completions-tool-result-images.test.ts` | ADAPTED | Exact empty-text and whitespace-preservation production payload tests | Image-only user messages remain valid after exactly-empty text removal. |
| `packages/ai/test/openai-responses-compat.test.ts` | ADAPTED | `v0871_release_test::tests::gpt_6_sol_and_luna_catalog_cache_and_default_reasoning_match_upstream` | Sol/Luna emit default `reasoning.effort=none` through the production Responses builder. |
| `packages/ai/test/stream.test.ts` | LIVE UNEXECUTED | `xai_grok45_responses_test::tests::xai_grok_47_uses_responses_xhigh_encrypted_reasoning_and_user_agent_override` | Credentialed live smoke is not run; deterministic production HTTP/SSE request shape is covered locally. |
| `packages/ai/test/supports-xhigh.test.ts` | ADAPTED | Extended `supports_xhigh_test` catalog assertions | Claude Opus 5.5 and GPT-6 Sol/Luna thinking maps are exercised through the public helper. |
| `packages/ai/test/xai-responses.test.ts` | ADAPTED | `xai_grok45_responses_test::tests::xai_grok_47_uses_responses_xhigh_encrypted_reasoning_and_user_agent_override` | Grok 4.7 uses `/responses`, xhigh effort, and encrypted reasoning. |

## Per-file 150-test disposition matrix

| Upstream test file | Disposition | Source | rs-ai evidence | Notes |
|---|---|---|---|---|
| `abort.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `anthropic-adaptive-thinking-models.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `anthropic-auth-token.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `anthropic-cache-write-1h-cost.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `anthropic-eager-tool-input-compat.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `anthropic-eager-tool-input-e2e.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `anthropic-empty-thinking-signature-compat.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `anthropic-force-adaptive-thinking.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `anthropic-long-cache-retention-e2e.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `anthropic-mid-conversation-effort.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `anthropic-oauth.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `anthropic-opus-4-8-smoke.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `anthropic-sse-parsing.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `anthropic-temperature-compat.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `anthropic-thinking-binding-e2e.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `anthropic-thinking-disable.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `anthropic-tool-name-normalization.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `assistant-message-frame.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `azure-openai-base-url.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `azure-openai-responses-reasoning-replay.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `azure-openai-tool-choice.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `baseten-models.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `bedrock-cache-write-1h-cost.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `bedrock-convert-messages.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `bedrock-credentials.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `bedrock-custom-headers.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `bedrock-endpoint-resolution.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `bedrock-error-metadata.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `bedrock-models.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `bedrock-raw-stop-reason.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `bedrock-redacted-reasoning.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `bedrock-response-headers.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `bedrock-thinking-payload.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `cache-retention.test.ts` | ADAPTED | v0.87.1 tag `f07218c4d4bbc12bef056a7058c3dd49dfe41abe` | `v0871_release_test::gpt_6_sol_and_luna_catalog_cache_and_default_reasoning_match_upstream` | OpenAI Sol/Luna long-cache TTL and default `none` reasoning are exercised through the production Responses payload. |
| `cloudflare-ai-binding.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `cloudflare-stream.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `compat-env.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `constrained-sampling.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `context-estimate.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `context-overflow.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `cross-provider-handoff.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `empty.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `env-api-keys.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `error-body.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `event-stream.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `faux-provider.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `fetch-option.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `fireworks-model-generation.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `fireworks-models.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `generate-models-strict.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `github-copilot-anthropic.test.ts` | ADAPTED | v0.87.1 tag `f07218c4d4bbc12bef056a7058c3dd49dfe41abe` | `github_copilot_anthropic_test::tests::applies_copilot_specific_adaptive_thinking_effort_overrides` plus `v0871_release_test` | Copilot Claude Opus 5.5 catalog API, context and effort levels are release-pinned. |
| `github-copilot-oauth.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `google-raw-stop-reason.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `google-shared-convert-tools.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `google-shared-gemini3-unsigned-tool-call.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `google-shared-image-tool-result-routing.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `google-shared-retry.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `google-shared-signed-empty-blocks.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `google-thinking-disable.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `google-thinking-level-map.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `google-thinking-signature.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `google-vertex-api-key-resolution.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `image-model-data.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `image-tool-result.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `images-models.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `images.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `interleaved-thinking.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `kimi-coding-oauth.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `lax-message-content.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `lazy-module-load.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `max-thinking.test.ts` | ADAPTED | v0.87.1 tag `f07218c4d4bbc12bef056a7058c3dd49dfe41abe` | `max_thinking_test::tests::{exposes_xhigh_and_max_for_openai_codex_current_variants,sends_max_to_the_codex_responses_api}` extended for GPT-6 Sol/Luna | Codex Sol/Luna expose xhigh/max and serialize `max` through the production Codex payload. |
| `message-types.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `meta-oauth.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `mistral-http-transport.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `mistral-raw-stop-reason.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `mistral-reasoning-mode.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `mistral-tool-schema.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `model-catalog-types.test.ts` | ADAPTED | v0.87.1 tag `f07218c4d4bbc12bef056a7058c3dd49dfe41abe` | `v0871_release_test::{grok_4_7_catalog_matches_upstream,claude_opus_5_5_catalog_matches_upstream}` | Exact API, input, effort, context and pricing metadata are asserted from the generated registry. |
| `model-data-validation.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `models-runtime.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `node-http-proxy.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `oauth-auth.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `oauth-device-code.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `openai-codex-cache-affinity-e2e.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `openai-codex-oauth.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `openai-codex-stream.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `openai-completions-cache-control-format.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `openai-completions-empty-tools.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `openai-completions-prompt-cache.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `openai-completions-raw-stop-reason.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `openai-completions-reasoning-details.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `openai-completions-response-model.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `openai-completions-retry.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `openai-completions-thinking-as-text.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `openai-completions-thinking-token-budget.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `openai-completions-tool-choice.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `openai-completions-tool-result-images.test.ts` | ADAPTED | v0.87.1 tag `f07218c4d4bbc12bef056a7058c3dd49dfe41abe` | `openai_completions_tool_result_images_test::tests::{omits_empty_text_parts_from_image_only_user_messages,preserves_whitespace_text_parts_in_multimodal_user_messages}` | Production OpenAI-compatible conversion omits only exactly-empty text and preserves whitespace. |
| `openai-completions-vllm-priority.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `openai-responses-cache-affinity-e2e.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `openai-responses-compat.test.ts` | ADAPTED | v0.87.1 tag `f07218c4d4bbc12bef056a7058c3dd49dfe41abe` | `v0871_release_test::gpt_6_sol_and_luna_catalog_cache_and_default_reasoning_match_upstream` | Production Responses payload emits `reasoning.effort=none` for Sol/Luna without requested reasoning. |
| `openai-responses-empty-tool-result.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `openai-responses-foreign-toolcall-id.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `openai-responses-message-id.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `openai-responses-namespace.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `openai-responses-partial-json-cleanup.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `openai-responses-reasoning-replay-e2e.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `openai-responses-terminal-event.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `openai-responses-tool-result-images.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `opencode-provider-headers.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `openrouter-cache-control-models.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `openrouter-cache-write-repro.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `openrouter-images.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `openrouter-oauth.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `openrouter-reasoning-options.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `overflow.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `pi-messages.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `pre-generation-error.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `provider-error-body-passthrough.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `provider-error-body-regression.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `provider-retry.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `providers.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `qwen-token-plan-models.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `radius-oauth.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `radius-provider.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `reasoning-options.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `responseid.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `retry.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `sampling-options.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `stream.test.ts` | LIVE UNEXECUTED | v0.87.1 tag `f07218c4d4bbc12bef056a7058c3dd49dfe41abe` | `xai_grok45_responses_test::tests::xai_grok_47_uses_responses_xhigh_encrypted_reasoning_and_user_agent_override` | Credentialed live smoke remains unexecuted; deterministic production HTTP/SSE request shape is exercised against wiremock. |
| `supports-xhigh.test.ts` | ADAPTED | v0.87.1 tag `f07218c4d4bbc12bef056a7058c3dd49dfe41abe` | `supports_xhigh_test` extended for Claude Opus 5.5 and GPT-6 Sol/Luna | Generated thinking maps are exercised through `get_supported_thinking_levels`. |
| `system-message-replay.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `telemetry-options.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `text.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `together-models.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `tokens.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `tool-call-id-normalization.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `tool-call-without-result.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `total-tokens.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `transcript-tool-changes.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `transform-messages-copilot-openai-to-anthropic.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `unicode-surrogate.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `uuid.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `validation.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `xai-oauth.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `xai-responses.test.ts` | ADAPTED | v0.87.1 tag `f07218c4d4bbc12bef056a7058c3dd49dfe41abe` | `xai_grok45_responses_test::tests::xai_grok_47_uses_responses_xhigh_encrypted_reasoning_and_user_agent_override` | Grok 4.7 uses `/responses`, xhigh effort and encrypted reasoning through the production stream path. |
| `xhigh.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `xiaomi-models.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `xiaomi-token-plan-ams-anthropic-empty-signature-smoke.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `zai-coding-plan-models.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
| `zen.test.ts` | COVERED | accepted v0.87.0 baseline | Existing accepted Rust parity suite | Unchanged in v0.87.0→v0.87.1; disposition inherited from the accepted baseline. |
