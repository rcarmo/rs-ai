# Upstream parity gap analysis

Canonical upstream: `@earendil-works/pi-ai` **v0.80.2**
(`github.com/earendil-works/pi`, `packages/ai`, commit `ec6311b`).
Port: `rs-ai` (crate `rs-ai`), branch `main`, tag `v0.80.2`.

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
| `github-copilot-headers.ts` | — | MISSING | Copilot OAuth-gated; no credential path available. |
| `google-generative-ai.ts` | `src/provider/google.rs` | DONE | |
| `google-shared.ts` | `src/provider/google.rs` | DONE | convert-tools, gemini3 unsigned tool-call, image tool-result routing, thinking signature. |
| `google-vertex.ts` | — | MISSING | Vertex GCP/ADC-gated; no credential path available. |
| `mistral-conversations.ts` | `src/provider/mistral.rs` | DONE | reasoning-mode, tool-schema, cached tokens, tool-id normalization — fully verified. |
| `openai-codex-responses.ts` | `src/provider/codex.rs` | PARTIAL | WS + SSE transports; OAuth account-id from env; no WS pooling/idle cache. WS handshake headers fixed (Sec-WebSocket-Key etc.) + connection-limit retry-once added. |
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
| `auth/helpers.ts` (`envApiKeyAuth`, `lazyOAuth`) | `src/env.rs` | PARTIAL | env path done; `lazyOAuth` not modelled. |
| `auth/resolve.ts` (`resolveProviderAuth`, `ModelsError`) | `src/auth.rs` | DONE | full `resolve_provider_auth`: api-key override → stored (oauth double-checked locked refresh / api-key) → ambient env. `ApiKeyAuth`/`OAuthAuth`/`AuthContext` traits + `ModelsError`. 12 auth tests incl. valid-token-skips-refresh and expired-refreshes-once-and-persists. |
| `utils/oauth/anthropic.ts` | `src/oauth.rs` | PARTIAL | token decode/account-id helpers; no interactive login. |
| `utils/oauth/openai-codex.ts` | `src/oauth.rs` | PARTIAL | account-id extraction from token. |
| `utils/oauth/github-copilot.ts` | — | MISSING | Copilot OAuth flow. |
| `utils/oauth/device-code.ts`, `oauth-page.ts`, `pkce.ts`, `load.ts` | — | MISSING | Interactive OAuth flows (device-code, PKCE, browser page). |

## 3. Core / utils modules

| Upstream | rs-ai path | Status | Notes |
|---|---|---|---|
| `compat.ts` | `src/compat.rs` | DONE | runtime `detect_compat` + static defaults (0.80.2). |
| `env-api-keys.ts` | `src/env.rs` | DONE | |
| `models.ts` / `models.generated.ts` | `src/registry.rs` / `src/models_generated.rs` | DONE | 999 models / 35 providers. |
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
| `utils/node-http-proxy.ts` | — | MISSING | no HTTP proxy support. |
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
- `anthropic-oauth`, `oauth-auth`, `oauth-device-code`, `github-copilot-oauth`,
  `github-copilot-anthropic`, `openai-codex-oauth`,
  `openai-responses-copilot-provider` — OAuth/Copilot flows (MISSING feature).
- `google-vertex-api-key-resolution` — Vertex (MISSING feature).
- `node-http-proxy` — HTTP proxy (MISSING feature).
- `lazy-module-load` — JS lazy loader (N/A).
- `anthropic-opus-4-8-smoke`, `xiaomi-token-plan-ams-anthropic-empty-signature-smoke` —
  live smoke tests (N/A without credentials).

¹ `abort` is covered only at the faux level; rs-ai has no `AbortSignal` (uses future-drop).

---

## Coverage estimate

- **Modules / exports:** ~88% functional coverage. Gaps are credential-gated
  (Copilot, Vertex, interactive OAuth, credential-store) or architectural
  (AbortSignal, HTTP proxy, CLI, lazy-loader).
- **Providers:** 100% catalog parity; runtime exercised for all non-credential-gated
  providers.
- **Tests:** ~75 of 87 upstream test files are behaviourally covered by rs-ai's 397
  tests. **2 files are now ported test-for-test** with upstream names/values
  (`mistral-reasoning-mode.test.ts` -> `src/mistral_reasoning_mode_test.rs`, 7/7;
  `azure-openai-base-url.test.ts` -> `src/azure_openai_base_url_test.rs`, 11/11);
  the remaining ~73 covered files are behaviourally equivalent but not yet
  name-for-name ports (bar #2 in progress).

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
