# rs-ai

[![CI](https://github.com/rcarmo/rs-ai/actions/workflows/ci.yml/badge.svg)](https://github.com/rcarmo/rs-ai/actions/workflows/ci.yml)
[![CycloneDX SBOM](https://img.shields.io/badge/SBOM-CycloneDX-4c1.svg)](https://github.com/rcarmo/rs-ai/releases/download/v1.0.1/sbom.cdx.json)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

A Rust port of [@earendil-works/pi-ai](https://www.npmjs.com/package/@earendil-works/pi-ai) with model discovery, streaming events, tool calls, OAuth helpers, image generation, and multi-provider request plumbing.

> **Experimental.** This crate is not published to crates.io. `main` contains the v1.1.0 upgrade in development, with 1,563 chat models, 61 image models and 26 classifiers. The latest accepted release is v1.0.1; the v1.1.0 contract audit and release checks are incomplete.

## Documentation

- [RELEASE.md](RELEASE.md) records upstream release bounds, catalog counts, runtime evidence, CI/SBOM evidence, and rollback notes.
- [docs/upstream-parity-gaps.md](docs/upstream-parity-gaps.md) tracks current parity decisions, adapted surfaces, and documented N/A cases.
- [docs/local-tests-shared.md](docs/local-tests-shared.md) records local gate history and shared test evidence.
* [docs/v110-upgrade-crosswalk.md](docs/v110-upgrade-crosswalk.md) and [docs/pi-durable-v110-crosswalk.md](docs/pi-durable-v110-crosswalk.md) track the current upgrade and unfinished contracts.
* [docs/v101-171-test-crosswalk.md](docs/v101-171-test-crosswalk.md) records the accepted v1.0.1 AI audit. Earlier crosswalks and manifests stay in `docs/` as historical records.

## Features

- Public `stream` and `complete` entry points over registered provider implementations.
* Generated chat, image and classifier registries from the pinned official v1.1.0 artifact, with offline metadata and regeneration checks.
- JSON-compatible message, context, tool, usage, diagnostics, assistant-frame, deferred-tool, and stream-option types for cross-language transcript hand-off.
- Tool calling with JSON Schema parameters, strict/constrained sampling helpers where providers expose them, partial JSON parsing for streamed arguments, and deferred tool loading metadata.
- Reasoning/thinking support, including provider thinking levels, signed/redacted thinking replay, raw stop reasons, and provider-specific compatibility flags.
- OAuth and credential helpers for Anthropic, OpenAI ChatGPT, OpenAI Codex, GitHub Copilot, Kimi Coding, xAI, Meta, and Radius flows.
- HTTP/SSE transports, OpenAI Codex WebSocket support, retry/proxy helpers, request/response hooks, cancellation-by-drop, and deterministic faux-provider tests.
* Image generation through `images`, plus classifiers for TypeSafe System One, Cloudflare Workers AI, llama.cpp and OpenAI Decisions. Decisions accepts image inputs and named choice, score and predicate answers.
- Local release gates for regenerated catalog drift, full-record baseline deltas, manifest/crosswalk integrity, SBOM generation, license policy, RustSec scanning, and reproducible test runs.

## Installation

This repository is currently intended for source or Git dependency use rather than crates.io publication:

```toml
[dependencies]
rs-ai = { git = "https://github.com/rcarmo/rs-ai" }
```

For local development, use the Make targets so Cargo and test scratch stay outside the source tree:

```bash
make fmt build test-all clippy
```

The vendored resolver uses `/workspace/tmp/rs-ai` locally when writable, otherwise the platform temporary directory plus `/rs-ai`. CI prefers `RUNNER_TEMP`, then the inherited `TMPDIR`, then platform temp. An absolute `PROJECT_TMP_BASE` override selects `<base>/rs-ai`; `PROJECT_TMP_ROOT` must be absolute and end in `rs-ai`. See [AGENTS.md](AGENTS.md#project-scoped-caches-and-temporary-files) for direct-command environment setup and safe cleanup.

The default feature set includes Bedrock support. To avoid the AWS SDK dependencies in a lightweight build, disable default features:

```toml
[dependencies]
rs-ai = { git = "https://github.com/rcarmo/rs-ai", default-features = false }
```

## Quick start

```rust,no_run
use rs_ai::{complete, provider_id, registry, user_message, ContentBlock, Context, StreamOptions};

#[tokio::main]
async fn main() {
    registry::register_builtin_models();

    let model = registry::get_model(provider_id::OPENAI, "gpt-4o-mini")
        .expect("model not found");

    let context = Context {
        system_prompt: Some("You are a helpful assistant.".to_string()),
        messages: vec![user_message("What is 2+2?")],
        tools: Vec::new(),
    };

    let message = complete(
        &model,
        &context,
        &StreamOptions {
            api_key: std::env::var("OPENAI_API_KEY").ok(),
            ..Default::default()
        },
    )
    .await
    .expect("request failed");

    for block in &message.content {
        if let ContentBlock::Text { text, .. } = block {
            print!("{text}");
        }
    }
}
```

Set provider API keys in the process environment or pass per-request credentials through `StreamOptions`. Provider-specific headers, environment overlays, OAuth credentials, retry settings, timeout settings, and request/response hooks are also carried through `StreamOptions`.

## v1.1.0 API changes on main

Assistant messages expose optional `duration_ms` (serialized as `durationMs`). Terminal stream timing uses a monotonic clock, preserves supplied durations and leaves replayed messages untouched. Token estimates use 3.5 UTF-16 code units per token and include system tool-definition updates. Sampling resolves model defaults, clamped thinking-level defaults, then request overrides.

Azure models use provider ID `azure` and support both Responses and Completions, with request-scoped endpoint, API version and deployment configuration. ChatGPT OAuth hosts can override the login agent name; Anthropic callback binding falls back to an ephemeral port when port 53692 is occupied. Codex SSE and WebSocket requests accept `originator` and `User-Agent` overrides through `StreamOptions.headers`; credential authorization and account identity stay enforced.

`auth::ProviderAuth.oauth` takes `Arc<dyn OAuthAuth>`. Stored OAuth refreshes wait cancellably for the provider lock, then persist any admitted token rotation before releasing it -- even after the caller cancels or drops its future. The worker has an independent 15-second timeout. Model-catalog refresh shares that path; offline refresh does not rotate credentials. `InMemoryCredentialStore::clone` shares credentials and locks.

## Durable sessions

The native `durable` module provides a partial R1 implementation with memory/journal storage, writer fencing, submissions, model/tool execution and recovery. Sessions persist a `pi.provider` document containing the provider session UUID; generation intents forward it as `StreamOptions.session_id`. Tasks expose optional `started_at`/`ended_at` timestamps, and completed executions record `durationMs`. Snapshot scans preserve cursor direction; `context_with_options(ContextOptions { at })` provides inclusive historical cuts of the native user/assistant text context without dispatching work.

Full pi-durable parity is unfinished. Generic tasks/documents, forks, inbox modes, events, hooks, partial output, deferred polling, compaction, subagents, remote/SQLite storage and cross-process leases need further implementation or verification. The [durable crosswalk](docs/pi-durable-v110-crosswalk.md) lists the source and test scope; [native design](docs/durable-native-design.md) describes the implemented subset.

## Package/source layout

```text
rs-ai/
├── src/
│   ├── lib.rs                   # crate root and re-exports
│   ├── types.rs                 # Message, Context, Tool, Model, Usage, StreamOptions
│   ├── events.rs                # streaming event enum
│   ├── registry.rs              # model/provider registry plus stream/complete APIs
│   ├── assistant_message_frame.rs
│   ├── auth.rs                  # runtime auth abstraction
│   ├── oauth.rs                 # OAuth/device-code/refresh helpers
│   ├── compat.rs                # OpenAI-compatible provider flags
│   ├── models_generated.rs      # generated text/chat model registry
│   ├── models_runtime.rs        # dynamic model-provider registry support
│   ├── provider/                # text/chat provider implementations
│   ├── transports/              # SSE and transport primitives
│   ├── images/                  # image API, OpenRouter provider, generated image registry
│   ├── classifiers/             # classifier APIs and generated registry
│   ├── durable/                 # partial native session/execution/recovery runtime
│   └── tests/                   # crate-private deterministic parity tests
├── docs/                        # parity ledgers and upstream test crosswalks
├── scripts/                     # catalog, manifest, release, and SBOM validation gates
├── .github/workflows/ci.yml     # hosted Rust/SBOM/license/RustSec CI
├── Cargo.toml
├── Cargo.lock
├── Makefile
└── RELEASE.md
```

## Provider status

| Surface | Status |
|---|---|
| OpenAI Chat Completions and compatible APIs | Implemented |
| OpenAI Responses; Azure Responses and Completions | Implemented |
| OpenAI Codex Responses, SSE and WebSocket paths | Implemented |
| Anthropic Messages, including managed effort and signed-thinking replay | Implemented |
| Google Generative AI and Google Vertex REST path | Implemented |
| Mistral Conversations | Implemented |
| Amazon Bedrock ConverseStream | Implemented behind the default `bedrock` feature |
| GitHub Copilot aggregate provider/OAuth runtime helpers | Implemented |
| Cloudflare Workers AI / AI Gateway compatible HTTP routes | Implemented for HTTP dispatch; Workers `env.AI.fetch` is a JavaScript binding with no Rust runtime global |
| OpenRouter image generation | Implemented in `images::openrouter` |
| Faux provider | Implemented for deterministic tests |

The generated catalog also includes provider metadata for OpenRouter, xAI, Groq, Cerebras, Vercel AI Gateway, Fireworks, Together, Moonshot AI, Kimi Coding, Qwen Token Plan, ZAI, NVIDIA, Baseten, Xiaomi/MiMo, Cloudflare, GitHub Copilot, OpenCode, Minimax, Hugging Face, and related OpenAI- or Anthropic-compatible endpoints where upstream models define them.

## Known limitations/divergences

- This is a Rust library, so JavaScript-only runtime surfaces such as a Workers `env.AI.fetch` binding are represented by HTTP model/provider paths rather than copied as runtime globals.
- Provider SDK behaviour is not always byte-for-byte identical. Where Rust uses `reqwest`, `tokio-tungstenite`, or the AWS SDK instead of upstream JavaScript SDKs, request and stream semantics are tested against deterministic fixtures and recorded in the release ledger.
- Live-provider smoke tests that require credentials stay out of the local gate. Deterministic wire, parser, replay, catalog, OAuth, and validation tests are preferred, with live-only gaps labelled in the crosswalk.
- Cancellation is idiomatic Rust cancellation: drop the returned stream, wrap it in `tokio::time::timeout`, or use `tokio::select!`. The HTTP providers do not expose an `AbortSignal` option or synthesize an upstream-style aborted terminal event.
- `StreamOptions.timeout_ms` controls the explicit `reqwest` request timeout. If it is absent, this crate does not add an extra timeout on top of the underlying transport.
* Bedrock request construction uses the typed AWS SDK `ConverseStream` builder, so JSON `on_payload` mutation hooks apply to HTTP JSON providers but not to the Bedrock SDK builder path.
* `StreamOptions.headers` stores string values. Upstream request-header null deletion has no direct representation in this map.

## Compatibility/versioning

The latest accepted runtime tracks upstream `@earendil-works/pi-ai` v1.0.1. `main` targets official pi-ai and pi-durable v1.1.0, with final acceptance still open. Contexts, messages, events, tools, usage, assistant frames, catalog records, and provider compatibility fields are intended to serialize in the same shape as upstream where the Rust surface overlaps.

Release audits update `RELEASE.md`, regenerated catalogs, and the per-release manifests in `docs/`. Repository tags should be treated as upstream-aligned checkpoints for the audited Rust port rather than a guarantee that every upstream JavaScript runtime surface exists unchanged in Rust.

## Upstream and attribution

This project is a derivative port of [@earendil-works/pi-ai](https://www.npmjs.com/package/@earendil-works/pi-ai), part of the [earendil-works/pi](https://github.com/earendil-works/pi/tree/main/packages/ai) project, originally created by [Mario Zechner](https://mariozechner.at). The TypeScript API design, event protocol, provider implementations, model registry, and OAuth flows originate upstream. This port adapts them idiomatically for Rust. All credit for the original design goes to Mario and the upstream contributors.

## Supply-chain metadata

The accepted v1.0.1 release publishes [`sbom.cdx.json`](https://github.com/rcarmo/rs-ai/releases/download/v1.0.1/sbom.cdx.json) and [`sbom.cdx.json.sha256`](https://github.com/rcarmo/rs-ai/releases/download/v1.0.1/sbom.cdx.json.sha256). [RELEASE.md](RELEASE.md) records the runtime, hosted checks and asset receipts. The v1.1.0 development checkpoints have no accepted release SBOM yet. Generate and validate one locally with `make sbom && make sbom-check`; the publisher validates the explicit accepted runtime ref.

## License

MIT.
