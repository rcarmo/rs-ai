# rs-ai

[![CI](https://github.com/rcarmo/rs-ai/actions/workflows/ci.yml/badge.svg)](https://github.com/rcarmo/rs-ai/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

A Rust port of [@earendil-works/pi-ai](https://www.npmjs.com/package/@earendil-works/pi-ai) with model discovery, streaming events, tool calls, OAuth helpers, image generation, and multi-provider request plumbing.

> **Experimental.** This crate is still pre-`v1` and is not published to crates.io. The accepted v0.85.0 runtime audit embeds 1336 text/chat models across 39 providers, 9 text/chat API protocols, and 50 image models.

## Documentation

- [RELEASE.md](RELEASE.md) records upstream release bounds, catalog counts, runtime evidence, CI/SBOM evidence, and rollback notes.
- [docs/upstream-parity-gaps.md](docs/upstream-parity-gaps.md) tracks current parity decisions, adapted surfaces, and documented N/A cases.
- [docs/local-tests-shared.md](docs/local-tests-shared.md) records local gate history and shared test evidence.
- [docs/v0850-142-test-crosswalk.md](docs/v0850-142-test-crosswalk.md), [docs/manifests/v0850-changed-paths.txt](docs/manifests/v0850-changed-paths.txt), and [docs/manifests/v0850-test-corpus-142.txt](docs/manifests/v0850-test-corpus-142.txt) capture the accepted v0.85.0 audit inventory.

## Features

- Public `stream` and `complete` entry points over registered provider implementations.
- Generated text/chat and image model registries regenerated from the pinned upstream v0.85.0 release data.
- JSON-compatible message, context, tool, usage, diagnostics, assistant-frame, deferred-tool, and stream-option types for cross-language transcript hand-off.
- Tool calling with JSON Schema parameters, strict/constrained sampling helpers where providers expose them, partial JSON parsing for streamed arguments, and deferred tool loading metadata.
- Reasoning/thinking support, including provider thinking levels, signed/redacted thinking replay, raw stop reasons, and provider-specific compatibility flags.
- OAuth and credential helpers for Anthropic, OpenAI Codex, GitHub Copilot, Kimi Coding, xAI, and Radius flows.
- HTTP/SSE transports, OpenAI Codex WebSocket support, retry/proxy helpers, request/response hooks, cancellation-by-drop, and deterministic faux-provider tests.
- Image generation support through the `images` module and OpenRouter image provider registration.
- Local release gates for regenerated catalog drift, full-record baseline deltas, manifest/crosswalk integrity, SBOM generation, license policy, RustSec scanning, and reproducible test runs.

## Installation

This repository is currently intended for source or Git dependency use rather than crates.io publication:

```toml
[dependencies]
rs-ai = { git = "https://github.com/rcarmo/rs-ai" }
```

For local development, clone the repository and run the standard Rust gates:

```bash
cargo fmt -- --check
cargo build
cargo test --all-targets --all-features
cargo clippy --all-targets --all-features -- -D warnings
```

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
| OpenAI Responses and Azure OpenAI Responses | Implemented |
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
- Bedrock request construction uses the typed AWS SDK `ConverseStream` builder, so JSON `on_payload` mutation hooks apply to HTTP JSON providers but not to the Bedrock SDK builder path.

## Compatibility/versioning

The current accepted runtime tracks upstream `@earendil-works/pi-ai` v0.85.0. Contexts, messages, events, tools, usage, assistant frames, catalog records, and provider compatibility fields are intended to serialize in the same shape as upstream where the Rust surface overlaps.

Release audits update `RELEASE.md`, regenerated catalogs, and the per-release manifests in `docs/`. Repository tags should be treated as upstream-aligned checkpoints for the audited Rust port rather than a guarantee that every upstream JavaScript runtime surface exists unchanged in Rust.

## Upstream and attribution

This project is a derivative port of [@earendil-works/pi-ai](https://www.npmjs.com/package/@earendil-works/pi-ai), part of the [earendil-works/pi](https://github.com/earendil-works/pi/tree/main/packages/ai) project, originally created by [Mario Zechner](https://mariozechner.at). The TypeScript API design, event protocol, provider implementations, model registry, and OAuth flows originate upstream. This port adapts them idiomatically for Rust. All credit for the original design goes to Mario and the upstream contributors.

## License

MIT.
