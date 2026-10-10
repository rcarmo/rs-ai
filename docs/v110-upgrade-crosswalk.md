# pi-ai v1.1.0 upgrade crosswalk

Status: **IN PROGRESS — NOT ACCEPTED OR PUBLISHED**.

Official range: `a7229ddc21810d6245105978033b7df645ecc2f7..abe508e1b89912adde45528136c3221eb69acdd7` (v1.0.1 → v1.1.0). Pinned npm package: `@earendil-works/pi-ai@1.1.0`, SHA-256 `6caab33cec57480ed02c57fe37428a030a77cc2a0662814b435a5cf8932ad829`. Registry SHA-512 integrity matches; provenance signature/transparency has not been independently verified. npm metadata omits gitHead; the official tag supplies the source bound.

Inventory: 82 changed paths; 197 source paths; 167 executable test paths; 174 total test/fixture/support paths. Exact Git shortstat: 82 files changed, 2580 insertions(+), 653 deletions(-).

Pinned catalog: 1563 chat, 61 image, 26 classifier records; 1650 total, 42 provider shards, schema v6. Full-record delta: chat +79/-52/199 changed (includes the Azure provider rename), image +2/-0/0 changed, classifier +6/-0/1 changed.

## Changed-path disposition

| Status | Official path | Disposition | Native evidence |
|---|---|---|---|
| M | `packages/ai/CHANGELOG.md` | PENDING | Not yet accepted |
| M | `packages/ai/README.md` | PENDING | Not yet accepted |
| M | `packages/ai/package.json` | PENDING | Not yet accepted |
| A | `packages/ai/scripts/ai-gateway-pricing.ts` | PENDING | Not yet accepted |
| M | `packages/ai/scripts/generate-models.ts` | PENDING | Not yet accepted |
| M | `packages/ai/scripts/openrouter-catalog.ts` | PENDING | Not yet accepted |
| A | `packages/ai/src/api/azure-openai-config.ts` | PENDING | Not yet accepted |
| M | `packages/ai/src/api/azure-openai-responses.ts` | PENDING | Not yet accepted |
| M | `packages/ai/src/api/bedrock-converse-stream.ts` | IMPLEMENTED reasoning/model-family delta; broader audit pending | `src/provider/bedrock.rs` adds Haiku-5 adaptive/xhigh/binding/cache eligibility and flat GPT-OSS versus nested GPT reasoning fields. Added field-builder regressions exercise all GPT-OSS levels, custom maps and nonreasoning guard; two new tests pass. Live Bedrock verification has not run. |
| A | `packages/ai/src/api/classifier-shared.ts` | PENDING | Not yet accepted |
| M | `packages/ai/src/api/cloudflare-workers-ai-system-one.ts` | PENDING | Not yet accepted |
| M | `packages/ai/src/api/lazy.ts` | PENDING | Not yet accepted |
| M | `packages/ai/src/api/llama-cpp-classify.ts` | PENDING | Not yet accepted |
| M | `packages/ai/src/api/mistral-conversations.ts` | IMPLEMENTED server-error finish delta | Mistral terminal `error` keeps raw reason and reports `Provider stopped with: error (server error)`. HTTP/SSE regression in `mistral_reasoning_mode_test.rs` verifies terminal error, partial text and billed usage. |
| M | `packages/ai/src/api/openai-codex-responses.ts` | IMPLEMENTED configurable-header delta; null deletion gap | `src/provider/codex.rs` shares case-insensitive default/model/request header merging for SSE and WebSocket. Request `originator`/`User-Agent` override defaults; bearer/account identity is enforced last. SSE wire and WS handshake regression coverage; 49 focused Codex tests pass. Rust request headers hold strings, so upstream null deletion is not expressible. |
| M | `packages/ai/src/api/openai-completions.ts` | PENDING | Not yet accepted |
| A | `packages/ai/src/api/openai-decisions.lazy.ts` | PENDING | Not yet accepted |
| A | `packages/ai/src/api/openai-decisions.ts` | PENDING | Not yet accepted |
| M | `packages/ai/src/api/openai-responses.ts` | PENDING | Not yet accepted |
| M | `packages/ai/src/api/simple-options.ts` | PENDING | Not yet accepted |
| M | `packages/ai/src/api/system-one-shared.ts` | PENDING | Not yet accepted |
| M | `packages/ai/src/api/typesafe-system-one.ts` | PENDING | Not yet accepted |
| M | `packages/ai/src/auth/oauth/anthropic.ts` | PENDING | Not yet accepted |
| M | `packages/ai/src/auth/oauth/openai-chatgpt.ts` | PENDING | Not yet accepted |
| M | `packages/ai/src/auth/oauth/openai-codex.ts` | PENDING | Not yet accepted |
| M | `packages/ai/src/auth/resolve.ts` | IMPLEMENTED DELTA; final acceptance pending | `src/auth.rs`: `refresh_stored_oauth_credential` rechecks under lock; admitted rotations persist after cancellation/drop, bounded by 15 s; explicit-only post-refresh validity check. Six added regression tests plus real-provider HTTP cancellation/persistence cases. `ProviderAuth.oauth` uses `Arc` for worker ownership. |
| M | `packages/ai/src/auth/types.ts` | PENDING | Not yet accepted |
| M | `packages/ai/src/env-api-keys.ts` | PENDING | Not yet accepted |
| M | `packages/ai/src/models.generated.ts` | PENDING | Not yet accepted |
| M | `packages/ai/src/models.ts` | IMPLEMENTED OAuth/publication deltas; broader runtime audit pending | `src/models_runtime.rs` preserves admitted rotations, serializes persistence across replacements, fences stale store writes/deletes and updates memory after persistence. Source tasks abort on supersession/drop; ready cache restores survive pre-cancellation. Twenty-four runtime-refresh tests pass, including OAuth, in-flight writes, replacement/delete/clear, failed-store and drop regressions. Custom non-settling stores have no independent timeout. |
| M | `packages/ai/src/providers/all.ts` | PENDING | Not yet accepted |
| D | `packages/ai/src/providers/azure-openai-responses.models.ts` | PENDING | Not yet accepted |
| D | `packages/ai/src/providers/azure-openai-responses.ts` | PENDING | Not yet accepted |
| A | `packages/ai/src/providers/azure.models.ts` | PENDING | Not yet accepted |
| A | `packages/ai/src/providers/azure.ts` | PENDING | Not yet accepted |
| M | `packages/ai/src/providers/faux.ts` | PENDING | Not yet accepted |
| M | `packages/ai/src/providers/openai.ts` | PENDING | Not yet accepted |
| M | `packages/ai/src/providers/radius.ts` | PENDING | Not yet accepted |
| M | `packages/ai/src/types.ts` | PENDING | Not yet accepted |
| M | `packages/ai/src/utils/estimate.ts` | PENDING | Not yet accepted |
| M | `packages/ai/src/utils/event-stream.ts` | PENDING | Not yet accepted |
| M | `packages/ai/src/utils/model-operations.ts` | PENDING | Not yet accepted |
| M | `packages/ai/src/utils/provider-retry.ts` | PENDING | Not yet accepted |
| M | `packages/ai/src/utils/retry.ts` | PENDING | Not yet accepted |
| M | `packages/ai/test/abort.test.ts` | PENDING | Not yet accepted |
| M | `packages/ai/test/anthropic-adaptive-thinking-models.test.ts` | PENDING | Not yet accepted |
| M | `packages/ai/test/anthropic-oauth.test.ts` | PENDING | Not yet accepted |
| M | `packages/ai/test/azure-openai-base-url.test.ts` | PENDING | Not yet accepted |
| A | `packages/ai/test/azure-openai-completions.test.ts` | PENDING | Not yet accepted |
| M | `packages/ai/test/azure-openai-responses-reasoning-replay.test.ts` | PENDING | Not yet accepted |
| M | `packages/ai/test/azure-openai-tool-choice.test.ts` | PENDING | Not yet accepted |
| M | `packages/ai/test/bedrock-thinking-payload.test.ts` | PENDING | Not yet accepted |
| M | `packages/ai/test/classifier-models.test.ts` | PENDING | Not yet accepted |
| M | `packages/ai/test/context-estimate.test.ts` | PENDING | Not yet accepted |
| M | `packages/ai/test/context-overflow.test.ts` | PENDING | Not yet accepted |
| M | `packages/ai/test/cross-provider-handoff.test.ts` | PENDING | Not yet accepted |
| M | `packages/ai/test/empty.test.ts` | PENDING | Not yet accepted |
| M | `packages/ai/test/event-stream.test.ts` | PENDING | Not yet accepted |
| M | `packages/ai/test/faux-provider.test.ts` | PENDING | Not yet accepted |
| M | `packages/ai/test/image-tool-result.test.ts` | PENDING | Not yet accepted |
| M | `packages/ai/test/mistral-raw-stop-reason.test.ts` | PENDING | Not yet accepted |
| A | `packages/ai/test/model-cost-tiers.test.ts` | PENDING | Not yet accepted |
| M | `packages/ai/test/models-runtime.test.ts` | PENDING | Not yet accepted |
| M | `packages/ai/test/openai-chatgpt-oauth.test.ts` | PENDING | Not yet accepted |
| M | `packages/ai/test/openai-codex-oauth.test.ts` | PENDING | Not yet accepted |
| M | `packages/ai/test/openai-codex-stream.test.ts` | PENDING | Not yet accepted |
| M | `packages/ai/test/openai-completions-empty-tools.test.ts` | PENDING | Not yet accepted |
| A | `packages/ai/test/openai-decisions.test.ts` | PENDING | Not yet accepted |
| M | `packages/ai/test/openai-responses-namespace.test.ts` | PENDING | Not yet accepted |
| M | `packages/ai/test/openai-responses-tool-result-images.test.ts` | PENDING | Not yet accepted |
| M | `packages/ai/test/provider-retry.test.ts` | PENDING | Not yet accepted |
| M | `packages/ai/test/radius-provider.test.ts` | PENDING | Not yet accepted |
| M | `packages/ai/test/responseid.test.ts` | PENDING | Not yet accepted |
| M | `packages/ai/test/retry.test.ts` | PENDING | Not yet accepted |
| M | `packages/ai/test/sampling-options.test.ts` | PENDING | Not yet accepted |
| M | `packages/ai/test/stream.test.ts` | PENDING | Not yet accepted |
| M | `packages/ai/test/supports-xhigh.test.ts` | PENDING | Not yet accepted |
| M | `packages/ai/test/tokens.test.ts` | PENDING | Not yet accepted |
| M | `packages/ai/test/tool-call-without-result.test.ts` | PENDING | Not yet accepted |
| M | `packages/ai/test/total-tokens.test.ts` | PENDING | Not yet accepted |
| M | `packages/ai/test/typesafe-system-one.test.ts` | PENDING | Not yet accepted |
| M | `packages/ai/test/unicode-surrogate.test.ts` | PENDING | Not yet accepted |

## Full test and support corpus

| Official path | Disposition | Native evidence |
|---|---|---|
| `packages/ai/test/abort.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/anthropic-adaptive-thinking-models.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/anthropic-auth-token.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/anthropic-cache-write-1h-cost.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/anthropic-eager-tool-input-compat.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/anthropic-eager-tool-input-e2e.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/anthropic-empty-thinking-signature-compat.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/anthropic-federation-sdk.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/anthropic-federation.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/anthropic-force-adaptive-thinking.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/anthropic-long-cache-retention-e2e.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/anthropic-mid-conversation-effort.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/anthropic-oauth.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/anthropic-opus-4-8-smoke.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/anthropic-sse-parsing.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/anthropic-strict-tool-schema.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/anthropic-temperature-compat.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/anthropic-thinking-binding-e2e.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/anthropic-thinking-disable.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/anthropic-tool-name-normalization.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/assistant-message-frame.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/azure-openai-base-url.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/azure-openai-completions.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/azure-openai-responses-reasoning-replay.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/azure-openai-tool-choice.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/azure-utils.ts` | INVENTORIED SUPPORT | Not yet accepted |
| `packages/ai/test/baseten-models.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/bedrock-cache-write-1h-cost.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/bedrock-convert-messages.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/bedrock-credentials.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/bedrock-custom-headers.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/bedrock-endpoint-resolution.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/bedrock-error-metadata.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/bedrock-models.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/bedrock-raw-stop-reason.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/bedrock-redacted-reasoning.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/bedrock-response-headers.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/bedrock-thinking-payload.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/bedrock-utils.ts` | INVENTORIED SUPPORT | Not yet accepted |
| `packages/ai/test/cache-retention.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/classifier-models.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/cloudflare-ai-binding.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/cloudflare-stream.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/cloudflare-utils.ts` | INVENTORIED SUPPORT | Not yet accepted |
| `packages/ai/test/cloudflare-workers-ai-system-one.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/codex-websocket-cached-probe.ts` | INVENTORIED SUPPORT | Not yet accepted |
| `packages/ai/test/compat-env.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/constrained-sampling.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/context-estimate.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/context-overflow.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/cross-provider-handoff.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/data/red-circle.png` | INVENTORIED SUPPORT | Not yet accepted |
| `packages/ai/test/empty.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/env-api-keys.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/error-body.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/event-stream.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/faux-provider.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/fetch-option.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/fireworks-model-generation.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/fireworks-models.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/generate-models-strict.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/github-copilot-anthropic.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/github-copilot-oauth.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/google-raw-stop-reason.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/google-shared-convert-tools.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/google-shared-gemini3-unsigned-tool-call.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/google-shared-image-tool-result-routing.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/google-shared-retry.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/google-shared-signed-empty-blocks.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/google-thinking-disable.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/google-thinking-level-map.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/google-thinking-signature.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/google-vertex-api-key-resolution.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/image-model-data.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/image-tool-result.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/images-models.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/images.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/interleaved-thinking.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/kimi-coding-oauth.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/lax-message-content.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/lazy-module-load.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/llama-cpp-classify.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/max-thinking.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/message-types.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/meta-oauth.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/mistral-http-transport.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/mistral-raw-stop-reason.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/mistral-reasoning-mode.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/mistral-tool-schema.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/model-catalog-types.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/model-cost-tiers.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/model-data-validation.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/model-types.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/models-entry.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/models-runtime.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/node-http-proxy.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/oauth-auth.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/oauth-callback-server.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/oauth-device-code.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/oauth.ts` | INVENTORIED SUPPORT | Not yet accepted |
| `packages/ai/test/openai-chatgpt-oauth.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/openai-codex-cache-affinity-e2e.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/openai-codex-oauth.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/openai-codex-stream.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/openai-completions-cache-control-format.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/openai-completions-empty-tools.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/openai-completions-prompt-cache.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/openai-completions-provider-stream-event.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/openai-completions-raw-stop-reason.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/openai-completions-reasoning-details.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/openai-completions-response-model.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/openai-completions-retry.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/openai-completions-thinking-as-text.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/openai-completions-thinking-token-budget.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/openai-completions-tool-choice.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/openai-completions-tool-result-images.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/openai-completions-vllm-priority.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/openai-decisions.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/openai-responses-cache-affinity-e2e.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/openai-responses-chatgpt-sign-in.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/openai-responses-compat.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/openai-responses-empty-tool-result.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/openai-responses-foreign-toolcall-id.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/openai-responses-message-id.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/openai-responses-namespace.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/openai-responses-partial-json-cleanup.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/openai-responses-reasoning-replay-e2e.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/openai-responses-terminal-event.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/openai-responses-tool-result-images.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/openai-responses-usage-limit.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/opencode-provider-headers.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/openrouter-cache-control-models.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/openrouter-cache-write-repro.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/openrouter-images.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/openrouter-oauth.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/openrouter-reasoning-options.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/overflow.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/pi-messages.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/pre-generation-error.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/provider-error-body-passthrough.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/provider-error-body-regression.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/provider-retry.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/providers.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/qwen-token-plan-models.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/radius-oauth.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/radius-provider.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/reasoning-options.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/responseid.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/retry.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/sampling-options.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/scratch.ts` | INVENTORIED SUPPORT | Not yet accepted |
| `packages/ai/test/stream.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/supports-xhigh.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/system-message-replay.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/telemetry-options.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/text.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/together-models.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/tokens.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/tool-call-id-normalization.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/tool-call-without-result.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/total-tokens.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/transcript-tool-changes.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/transform-messages-copilot-openai-to-anthropic.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/typesafe-system-one.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/unicode-surrogate.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/uuid.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/validation.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/xai-oauth.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/xai-responses.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/xhigh.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/xiaomi-models.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/xiaomi-token-plan-ams-anthropic-empty-signature-smoke.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/zai-coding-plan-models.test.ts` | PENDING | Not yet accepted |
| `packages/ai/test/zen.test.ts` | PENDING | Not yet accepted |

## Full source corpus

| Official path | Disposition | Native evidence |
|---|---|---|
| `packages/ai/src/api/anthropic-messages.lazy.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/api/anthropic-messages.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/api/azure-openai-config.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/api/azure-openai-responses.lazy.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/api/azure-openai-responses.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/api/bedrock-converse-stream.lazy.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/api/bedrock-converse-stream.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/api/classifier-shared.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/api/cloudflare-ai-binding.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/api/cloudflare-workers-ai-system-one.lazy.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/api/cloudflare-workers-ai-system-one.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/api/cloudflare.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/api/constrained-sampling.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/api/github-copilot-headers.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/api/google-generative-ai.lazy.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/api/google-generative-ai.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/api/google-shared.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/api/google-vertex.lazy.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/api/google-vertex.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/api/lazy.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/api/llama-cpp-classify.lazy.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/api/llama-cpp-classify.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/api/mistral-conversations.lazy.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/api/mistral-conversations.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/api/openai-codex-responses.lazy.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/api/openai-codex-responses.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/api/openai-completions.lazy.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/api/openai-completions.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/api/openai-decisions.lazy.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/api/openai-decisions.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/api/openai-prompt-cache.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/api/openai-responses-shared.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/api/openai-responses.lazy.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/api/openai-responses.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/api/openrouter-images.lazy.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/api/openrouter-images.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/api/pi-messages.lazy.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/api/pi-messages.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/api/simple-options.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/api/system-one-shared.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/api/transform-messages.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/api/typesafe-system-one.lazy.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/api/typesafe-system-one.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/auth/context.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/auth/credential-store.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/auth/helpers.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/auth/oauth/anthropic.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/auth/oauth/callback-server.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/auth/oauth/device-code.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/auth/oauth/github-copilot.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/auth/oauth/kimi-coding.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/auth/oauth/load.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/auth/oauth/meta.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/auth/oauth/openai-chatgpt.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/auth/oauth/openai-codex.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/auth/oauth/openrouter.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/auth/oauth/pkce.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/auth/oauth/radius.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/auth/oauth/xai.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/auth/resolve.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/auth/types.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/bedrock-provider.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/bun-oauth.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/cli.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/compat.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/compat/extension-oauth-types.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/env-api-keys.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/image-models.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/images-api-registry.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/images.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/index.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/legacy-api-aliases.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/model-catalog.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/models-store.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/models.generated.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/models.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/oauth.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/providers/all.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/providers/amazon-bedrock.models.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/providers/amazon-bedrock.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/providers/ant-ling.models.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/providers/ant-ling.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/providers/anthropic.models.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/providers/anthropic.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/providers/azure.models.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/providers/azure.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/providers/baseten.models.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/providers/baseten.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/providers/cerebras.models.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/providers/cerebras.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/providers/cloudflare-ai-gateway.models.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/providers/cloudflare-ai-gateway.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/providers/cloudflare-auth.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/providers/cloudflare-stream.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/providers/cloudflare-workers-ai.models.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/providers/cloudflare-workers-ai.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/providers/data-json.d.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/providers/deepseek.models.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/providers/deepseek.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/providers/faux.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/providers/fireworks.models.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/providers/fireworks.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/providers/github-copilot.models.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/providers/github-copilot.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/providers/google-vertex.models.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/providers/google-vertex.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/providers/google.models.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/providers/google.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/providers/groq.models.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/providers/groq.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/providers/huggingface.models.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/providers/huggingface.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/providers/images/register-builtins.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/providers/kimi-coding.models.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/providers/kimi-coding.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/providers/meta.models.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/providers/meta.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/providers/minimax-cn.models.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/providers/minimax-cn.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/providers/minimax.models.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/providers/minimax.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/providers/mistral.models.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/providers/mistral.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/providers/moonshotai-cn.models.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/providers/moonshotai-cn.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/providers/moonshotai.models.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/providers/moonshotai.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/providers/nvidia.models.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/providers/nvidia.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/providers/openai-codex.models.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/providers/openai-codex.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/providers/openai.models.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/providers/openai.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/providers/opencode-go.models.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/providers/opencode-go.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/providers/opencode-headers.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/providers/opencode.models.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/providers/opencode.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/providers/openrouter.models.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/providers/openrouter.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/providers/qwen-token-plan-cn.models.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/providers/qwen-token-plan-cn.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/providers/qwen-token-plan-individual.models.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/providers/qwen-token-plan-individual.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/providers/qwen-token-plan.models.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/providers/qwen-token-plan.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/providers/radius-config.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/providers/radius.models.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/providers/radius.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/providers/together.models.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/providers/together.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/providers/typesafe.models.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/providers/typesafe.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/providers/vercel-ai-gateway.models.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/providers/vercel-ai-gateway.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/providers/xai.models.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/providers/xai.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/providers/xiaomi-token-plan-ams.models.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/providers/xiaomi-token-plan-ams.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/providers/xiaomi-token-plan-cn.models.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/providers/xiaomi-token-plan-cn.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/providers/xiaomi-token-plan-sgp.models.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/providers/xiaomi-token-plan-sgp.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/providers/xiaomi.models.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/providers/xiaomi.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/providers/zai-coding-cn.models.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/providers/zai-coding-cn.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/providers/zai.models.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/providers/zai.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/session-resources.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/types.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/utils/abort-signals.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/utils/abort.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/utils/assistant-message-frame.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/utils/diagnostics.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/utils/error-body.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/utils/estimate.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/utils/event-stream.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/utils/hash.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/utils/headers.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/utils/json-parse.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/utils/model-operations.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/utils/models-error.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/utils/node-http-proxy.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/utils/oauth-page.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/utils/overflow.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/utils/pi-user-agent.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/utils/provider-env.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/utils/provider-retry.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/utils/retry.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/utils/sanitize-unicode.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/utils/sleep.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/utils/text.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/utils/transcript.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/utils/typebox-helpers.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/utils/uuid.ts` | PENDING | No full-contract acceptance yet |
| `packages/ai/src/utils/validation.ts` | PENDING | No full-contract acceptance yet |
