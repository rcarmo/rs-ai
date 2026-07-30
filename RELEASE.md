# rs-ai upstream release parity

This file is the release-audit ledger for `rs-ai`. It must be updated in the same commit as every future upstream `@earendil-works/pi-ai` release audit.

## Current upstream baseline

- Upstream package: `@earendil-works/pi-ai`
- Current accepted release: `v0.83.0`
- Upstream tag/commit: `845d6ff1f6643aba440341cce877ce1c43ebbc39`
- Previous accepted upstream baseline: `v0.82.1` / `b4f293684bba718d59cc1157679bcf6157b3a7f5`
- Audited range: `b4f293684bba718d59cc1157679bcf6157b3a7f5..845d6ff1f6643aba440341cce877ce1c43ebbc39`
- Scope: `packages/ai` only; no newer-main chase.

## Exact upstream change set

The v0.83.0 release changes 41 `packages/ai` paths:

- Release/docs/package metadata:
  - `CHANGELOG.md`
  - `README.md`
  - `package.json`
- Catalog generation:
  - `scripts/generate-models.ts`
- API/runtime:
  - `src/api/anthropic-messages.ts`
  - `src/api/azure-openai-responses.ts`
  - `src/api/bedrock-converse-stream.ts`
  - `src/api/google-generative-ai.ts`
  - `src/api/google-vertex.ts`
  - `src/api/mistral-conversations.ts`
  - `src/api/openai-codex-responses.ts`
  - `src/api/openai-completions.ts`
  - `src/api/openai-responses-shared.ts`
  - `src/api/openai-responses.ts`
  - `src/api/openrouter-images.ts`
  - `src/api/pi-messages.ts`
  - `src/api/simple-options.ts`
- Auth/provider/runtime support:
  - `src/auth/oauth/openrouter.ts`
  - `src/auth/resolve.ts`
  - `src/providers/faux.ts`
  - `src/types.ts`
- Tests:
  - `test/anthropic-sse-parsing.test.ts`
  - `test/azure-openai-responses-reasoning-replay.test.ts`
  - `test/bedrock-credentials.test.ts`
  - `test/bedrock-raw-stop-reason.test.ts`
  - `test/constrained-sampling.test.ts`
  - `test/faux-provider.test.ts`
  - `test/fetch-option.test.ts`
  - `test/github-copilot-anthropic.test.ts`
  - `test/google-raw-stop-reason.test.ts`
  - `test/mistral-raw-stop-reason.test.ts`
  - `test/models-runtime.test.ts`
  - `test/oauth-auth.test.ts`
  - `test/openai-completions-raw-stop-reason.test.ts`
  - `test/openai-completions-tool-choice.test.ts`
  - `test/openai-responses-partial-json-cleanup.test.ts`
  - `test/openai-responses-terminal-event.test.ts`
  - `test/openrouter-oauth.test.ts`
  - `test/pi-messages.test.ts`
  - `test/qwen-token-plan-models.test.ts`
  - `test/validation.test.ts`

## Rust implementation and disposition

### Catalog and metadata

- Regenerated `src/models_generated.rs` from hydrated v0.83.0 JSON shards.
- Text provider/id comparator:
  - `upstream=1153 local=1153 missing=0 extra=0`
- Image provider/id comparator:
  - `upstream=40 local=40 missing=0 extra=0`
- Repro command:

```bash
PI_AI_MODEL_DATA_DIR=/workspace/tmp/pi-v0830-json \
  scripts/compare_upstream_registry_pairs.py \
  /workspace/tmp/pi-v0830 \
  845d6ff1f6643aba440341cce877ce1c43ebbc39
```

### Stop reason and raw stop reason

- Added `StopReason::Pending` to mirror the v0.83 public type surface.
- Added `Message.raw_stop_reason` with serde-compatible camelCase field handling.
- Streaming assistant partials now start as `Pending` instead of absent/successful stop state.
- `Pending` must not emit a successful `Done`; streams that end pending are surfaced as errors.
- Raw stop reason capture is wired for:
  - OpenAI Completions
  - OpenAI Responses / Azure OpenAI Responses
  - OpenAI Codex Responses
  - Anthropic Messages
  - Google Generative AI / Vertex shared paths
  - Mistral Conversations
  - Bedrock Converse Stream
- Tests include raw-stop and pending/no-terminal behavior in provider fixtures and `src/v0830_release_test.rs`.

### Constrained sampling

Already completed as part of the v0.82.0 corrective work and retained for v0.83.0:

- `Tool.constrained_sampling` with camelCase serde.
- JSON-schema strict constrained sampling.
- Grammar constrained sampling:
  - lark/regex variant resolution
  - one-required-string schema validation
  - custom-tool request shape
  - monotonic streamed JSON delta reconstruction
- Integrated request-shape handling for OpenAI Completions, Responses/Azure, and Codex payloads.
- Tests are in `src/v0830_release_test.rs` and provider request fixtures.

### Retry behavior

Already completed as part of v0.82.0 corrective work and retained:

- `do_with_retry` honors `x-should-retry`.
- Excessive provider-requested retry delays fail immediately instead of silently clamping.
- Backoff sleep can be aborted via `do_with_retry_cancel`.
- Transport/no-status request errors are retryable.
- Tests cover direct retry helper behavior and actual OpenAI provider request path.

### OAuth/auth/model runtime

Existing v0.82.1 work remains active:

- `ModelsError::with_cause` preserves underlying cause detail in surfaced messages.
- Production auth resolution uses cause-preserving errors for API-key and OAuth failure paths.
- Radius OAuth discovery-only gateway routing is supported.
- Radius dynamic catalog refresh supports ETag/`If-None-Match` and 304 cached reuse.
- Anthropic `ANTHROPIC_AUTH_TOKEN` participates in env discovery and is sent as `Authorization: Bearer`, not `x-api-key`.

### v0.83 corrective runtime details

- Malformed OpenAI tool delta regression (valid `function` payload plus empty `custom`) preserves function name and arguments.
- Added focused raw/pending parser tests for Anthropic, Responses/Azure, Codex payload, Google, Mistral, and Bedrock/profile behavior.

- OAuth resolution now refreshes tokens early by default when less than 5 minutes of validity remain. `min_oauth_validity_ms` is floored to that upstream default 5-minute window, and refreshed credentials are rejected when they still do not satisfy the effective minimum validity.
- OpenAI malformed/empty custom and grammar constrained-sampling request/stream behavior remains covered by v0.82/v0.83 release fixtures.
- Provider raw stop reason tests cover OpenAI plus provider-specific raw/pending behavior through release and provider tests.
- Bedrock explicit profile precedence is tested: an explicit `StreamOptions.profile`/profile seam suppresses standard-endpoint region pinning even when ambient access keys exist, while ARN regions and custom endpoints retain priority.

### Fetch option changes

- Upstream v0.83.0 adds optional custom fetch plumbing in TypeScript APIs.
- Rust uses `reqwest` clients and proxy-aware builders instead of JavaScript `fetch`; there is no direct `fetch` injection point.
- Disposition: **N/A for Rust runtime**, with existing request/proxy/client tests covering the equivalent Rust transport surface.

### TypeScript-only/model-catalog tests

- Type-only/export/build-surface changes are not applicable to Rust.
- Disposition: **N/A**, with Rust model-data structural validation and serde model shape tests covering runtime-valid metadata.

### Live/credential tests

- Upstream live credential/e2e matrix updates remain N/A when they require external provider credentials or nondeterministic live service behavior.
- Deterministic request, stream, retry, error, and metadata behavior is covered by local mock-server tests.

## Verification for current release

Executed from `/workspace/projects/rs-ai`:

```bash
cargo fmt --check
PI_AI_MODEL_DATA_DIR=/workspace/tmp/pi-v0830-json \
  scripts/compare_upstream_registry_pairs.py \
  /workspace/tmp/pi-v0830 \
  845d6ff1f6643aba440341cce877ce1c43ebbc39
cargo build
cargo test   # run 3 times
cargo clippy --all-targets -- -D warnings
```

Results:

- Comparator: text `1153/1153`, image `40/40`, missing `0`, extra `0`
- `cargo build`: passed
- `cargo test` x3: each run `822 passed; 0 failed; 0 ignored`; doctest `1 passed; 0 failed`
- `cargo clippy --all-targets -- -D warnings`: passed

## Release-audit policy

For every future upstream `@earendil-works/pi-ai` release audit:

1. Pin the exact previous accepted upstream tag/SHA and new upstream tag/SHA.
2. Do not chase upstream main beyond the requested release tag.
3. List the exact changed `packages/ai` paths or a grouped matrix that accounts for all paths by count.
4. Regenerate and compare model/image catalogs with the JSON-shard-aware comparator when applicable.
5. Implement all applicable runtime/API/provider/model/tool/stream/error/usage behavior in production Rust paths.
6. Document every N/A decision with the concrete reason.
7. Run the required Rust gates.
8. Update this `RELEASE.md` in the same release commit before reporting completion.
