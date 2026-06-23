# rs-ai

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

A Rust port of [@earendil-works/pi-ai](https://www.npmjs.com/package/@earendil-works/pi-ai) — unified LLM API with automatic model discovery, streaming, tool calling, and multi-provider support.

> **⚠️ Early scaffold.** This crate is in initial development. The type system, event protocol, and registry are in place; provider implementations are being ported.

## Architecture

```
rs-ai/
├── src/
│   ├── lib.rs              # Crate root + re-exports
│   ├── types.rs            # Core types (Message, Context, Model, etc.)
│   ├── events.rs           # Stream event enum
│   ├── registry.rs         # Provider/model registry + stream/complete API
│   ├── env.rs              # Environment-based API key resolution
│   ├── compat.rs           # OpenAI-compatible provider detection
│   ├── models_generated.rs # Generated model registry (placeholder)
│   ├── provider/           # Provider implementations
│   │   └── openai.rs       # OpenAI Completions (placeholder)
│   ├── transports/
│   │   └── sse.rs          # SSE parser with sticky-field spec compliance
│   └── images/
│       ├── mod.rs
│       └── types.rs        # Image generation types
├── scripts/                # Code generator (model registry)
├── Cargo.toml
└── README.md
```

## Status

| Component | Status |
|---|---|
| Core types | ✅ Implemented |
| Event types | ✅ Implemented |
| Registry (model + provider) | ✅ Implemented |
| Env key resolution | ✅ Implemented |
| Compat detection | ✅ Implemented |
| SSE transport | ✅ Implemented + tested |
| Image types | ✅ Implemented |
| Message transform | ✅ Implemented + tested |
| Simple options / thinking | ✅ Implemented |
| Retry logic | ✅ Implemented + tested |
| Logger | ✅ Implemented |
| Diagnostics | ✅ Implemented |
| Azure normalization | ✅ Implemented + tested |
| Session resources | ✅ Implemented + tested |
| Prompt cache helpers | ✅ Implemented + tested |
| Input validation | ✅ Implemented + tested |
| Context overflow | ✅ Implemented + tested |
| OpenRouter image gen | ✅ Implemented |
| OpenAI provider | ✅ Streaming implemented |
| OpenAI Responses | ✅ Streaming implemented |
| Anthropic provider | ✅ Streaming implemented |
| Google Gemini | ✅ Streaming implemented |
| Mistral | ✅ Streaming implemented |
| Faux (test double) | ✅ Implemented + tested |
| Partial JSON parser | ✅ Implemented + tested |
| Harness helpers | ✅ Implemented + tested |
| Bedrock | ✅ Implemented (AWS SDK) |
| Codex (WebSocket + SSE) | ✅ Implemented |
| Gemini CLI | ✅ Implemented |
| OAuth flows | ✅ Framework + PKCE |

## Versioning

This port is tagged to match the upstream `@earendil-works/pi-ai` release it tracks
(e.g. `v0.79.5`). The tag is moved forward to the latest commit as additional
parity fixes land against that same upstream version, and a new `vX.Y.Z` tag is
cut when the port is realigned to a newer upstream release.

## Known limitations

Tracks `@earendil-works/pi-ai` `0.80.1`. Known divergences from upstream:

- **Google Vertex AI**: Vertex is **not functional** via this port. The streamed
  response format matches Gemini (so the shared decoder is reused), but Vertex's
  request path differs fundamentally from the Gemini API — a project/location-scoped
  endpoint (`/v1/projects/{project}/locations/{location}/publishers/google/models/...`),
  a `{location}` host placeholder, and Bearer/ADC (or service-account) auth instead of
  an API-key query param. Implementing it would require a GCP auth dependency and a
  Vertex-specific request builder. `google-vertex` models should be treated as
  unsupported here; use the `google-generative-ai` (Gemini API) models instead.
- **Provider SDK retries**: upstream relies on vendor SDK retry behavior. This port
  honors `StreamOptions` retry fields (`max_retries`, `max_retry_delay_ms`,
  `retry_config`) via `retry::do_with_retry` across the HTTP providers; Bedrock uses
  the AWS SDK's own retry. There is no implicit default retry when no options are set.
- **Header-based request auth (Anthropic)**: upstream 0.80.x added `assertRequestAuth`,
  allowing a request to authenticate via an `authorization`/`x-api-key`/`cf-aig-authorization`
  header instead of an explicit API key. This port still requires a resolved API key for the
  Anthropic provider (it does not thread per-request custom headers into that request path),
  so header-only auth for Anthropic is not yet supported. All other parity holds.
- **Scoped `options.env`**: upstream 0.79.5 threads an optional `options.env` map
  through every provider via `getProviderEnvValue(name, env)` so callers can supply a
  per-request environment overlay (plus a Bun `/proc/self/environ` sandbox fallback).
  This port reads the process environment natively; with no scoped overlay supplied the
  behavior is identical (the upstream fallback chain ends at `process.env`), so the
  overlay parameter is intentionally not plumbed through.

## Credits

Rust port of [**@earendil-works/pi-ai**](https://www.npmjs.com/package/@earendil-works/pi-ai), originally by [Mario Zechner](https://mariozechner.at).
