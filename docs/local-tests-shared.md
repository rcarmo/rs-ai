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

1. `TestProcessSSEStreamAttachesPendingEncryptedReasoningDetails` — encrypted-reasoning replay edge.
2. `TestConvertMessagesCoalescesConsecutiveToolResults` (bedrock) — rs-ai coalesces correctly (verified) but the logic is inline in `stream_bedrock`; needs a testable `build_bedrock_messages` extraction.
3. `TestBedrockOptionPrecedenceAndRequestMetadata` — region option precedence + request metadata propagation.
4. OAuth `TestGetAPIKeyRefreshesExpiredCredential` — blocked on credential-store seam (parity-gaps top-3 #2).

## Newly identified gap (from go-ai corpus)

- **WS connection-limit retry-once.** Upstream/go-ai detect
  `websocket_connection_limit_reached` (`isWebSocketConnectionLimitReachedError`)
  and retry the WS connection once before falling back to SSE. rs-ai's codex WS
  path falls straight back to SSE on any pre-stream failure (functionally
  equivalent end result, but missing the retry-once). Lives in the documented
  WS-pooling gap area (no idle cache / connection reuse). Codex *nested*
  event-error extraction (`/error/message`, `/error/code`) IS covered
  (`codex.rs` `process_event` "error"/"response.failed").
