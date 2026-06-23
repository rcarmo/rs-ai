# Shared local-test adaptation tracker (rs-ai)

Mirror of the cross-port requirement: adapt locally-authored regression/edge-case
tests from the reference ports — primarily **@go-ai** (`/workspace/projects/go-ai`,
`docs/local-tests-shared.md`, 188 local Go tests) — into idiomatic Rust. This file
tracks adaptation status; upstream 1:1 ports are tracked separately in
`docs/upstream-parity-gaps.md`.

Status legend: **ADAPTED** (ported to a named rs-ai test), **COVERED**
(behaviourally guarded by an existing rs-ai test), **PENDING**, **N/A**
(feature gated/architectural — see parity-gaps doc).

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
