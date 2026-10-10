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
| M | `packages/ai/src/api/bedrock-converse-stream.ts` | PENDING | Not yet accepted |
| A | `packages/ai/src/api/classifier-shared.ts` | PENDING | Not yet accepted |
| M | `packages/ai/src/api/cloudflare-workers-ai-system-one.ts` | PENDING | Not yet accepted |
| M | `packages/ai/src/api/lazy.ts` | PENDING | Not yet accepted |
| M | `packages/ai/src/api/llama-cpp-classify.ts` | PENDING | Not yet accepted |
| M | `packages/ai/src/api/mistral-conversations.ts` | PENDING | Not yet accepted |
| M | `packages/ai/src/api/openai-codex-responses.ts` | PENDING | Not yet accepted |
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
| M | `packages/ai/src/models.ts` | PENDING | Not yet accepted |
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
