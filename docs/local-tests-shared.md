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

1. `TestBuildCodexRequestMatchesPiaiShape` — exact codex request-shape snapshot vs pi-ai. rs-ai has `build_codex_payload` tests but not a full shape-parity snapshot.
2. `TestProcessSSEStreamAttachesPendingEncryptedReasoningDetails` — encrypted-reasoning replay edge.
3. `TestConvertMessagesCoalescesConsecutiveToolResults` (bedrock) — verify rs-ai coalescing has a named regression test.
4. `TestBedrockOptionPrecedenceAndRequestMetadata` — region option precedence + request metadata propagation.
5. OAuth `TestGetAPIKeyRefreshesExpiredCredential` — blocked on credential-store seam (parity-gaps top-3 #2).
