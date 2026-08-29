# v0.84.0 cumulative 127-file test crosswalk

Source: official upstream `@earendil-works/pi-ai` tag `a5f43bf8aff3c55752432655f7334e3dafd1e256`.

Total upstream `packages/ai/test/*.test.ts` files: **127**.

## Disposition counts

- **8 newly explicit absent-filename dispositions in this hardening slice**: 8 adapted with named executable Rust tests or production-script/artifact validators.
- **119 previously ported/adapted/covered/N/A dispositions**: tracked in `docs/upstream-parity-gaps.md` and `docs/v0840-manifests.md`; no new absence found in this pass.
- **127/127 accounted** in the cumulative filename walk.

## Newly explicit absent-filename dispositions

| Upstream file | Disposition | rs-ai evidence | Notes |
|---|---|---|---|
| cloudflare-stream.test.ts | ADAPTED | `src/tests/transports/cloudflare_stream_test.rs` | Cloudflare placeholders resolve from env before normal `stream_openai` and `registry::stream_simple` dispatch; unresolved placeholders are preserved by `resolve_cloudflare_base_url`. |
| image-model-data.test.ts | ADAPTED | `scripts/generate_image_models.py::parse_openrouter_image_models`, `src/tests/catalogs/image_model_data_test.rs` | Production generator helper validation: missing/empty strict catalog, no usable image models, valid image model parse. |
| model-data-validation.test.ts | ADAPTED | `scripts/validate_release_model_data.py`, `src/tests/catalogs/model_data_validation_test.rs` | Executable offline release-shard validation: missing data dir, wrong id, wrong provider, wrong API group, duplicate IDs across groups, stale hashes/missing IDs, incompatible schema, stale structure hash/generation stamp, invalid timestamp, missing shard. |
| openrouter-cache-control-models.test.ts | ADAPTED | `src/tests/catalogs/openrouter_cache_control_models_test.rs` | All four `~anthropic/claude-*-latest` OpenRouter model IDs expose `compat.cache_control_format = anthropic`. |
| provider-retry.test.ts | ADAPTED | `src/tests/transports/provider_retry_upstream_test.rs` | Provider retry helper behavior: retry-after-ms retry, x-should-retry=false no retry, excessive retry-after cap, cap disabled, cancellation during provider-requested delay. |
| reasoning-options.test.ts | ADAPTED | `scripts/generate_models.py::get_effort_thinking_level_map`, `src/tests/core/reasoning_options_test.rs` | Production generator helper mapping for effort values: verified values only, `none` only with toggle, toggle/budget/unverified controls left to adapter-specific handling. |
| uuid.test.ts | ADAPTED | `src/tests/core/uuid_test.rs` | UUIDv7 RFC 9562 layout and monotonic ordering; earlier release invariant remains in `src/tests/release/v0830_release_test.rs`. |
| xai-responses.test.ts | ADAPTED | `src/tests/providers/xai/xai_grok45_responses_test.rs`, `src/tests/transports/providers_upstream_test.rs::excludes_retired_xai_models_from_builtin_catalog` | xAI Grok 4.5 routes through OpenAI Responses; low/medium/high-only thinking levels; bearer auth, `store:false`, session header + `prompt_cache_key`, no long retention, medium reasoning effort, encrypted-content include, developer system prompt; retired xAI models excluded. |

## Full upstream filename inventory

1. `abort.test.ts`
2. `anthropic-adaptive-thinking-models.test.ts`
3. `anthropic-auth-token.test.ts`
4. `anthropic-cache-write-1h-cost.test.ts`
5. `anthropic-eager-tool-input-compat.test.ts`
6. `anthropic-eager-tool-input-e2e.test.ts`
7. `anthropic-empty-thinking-signature-compat.test.ts`
8. `anthropic-force-adaptive-thinking.test.ts`
9. `anthropic-long-cache-retention-e2e.test.ts`
10. `anthropic-oauth.test.ts`
11. `anthropic-opus-4-8-smoke.test.ts`
12. `anthropic-sse-parsing.test.ts`
13. `anthropic-temperature-compat.test.ts`
14. `anthropic-thinking-disable.test.ts`
15. `anthropic-tool-name-normalization.test.ts`
16. `azure-openai-base-url.test.ts`
17. `azure-openai-responses-reasoning-replay.test.ts`
18. `baseten-models.test.ts`
19. `bedrock-convert-messages.test.ts`
20. `bedrock-credentials.test.ts`
21. `bedrock-custom-headers.test.ts`
22. `bedrock-endpoint-resolution.test.ts`
23. `bedrock-error-metadata.test.ts`
24. `bedrock-models.test.ts`
25. `bedrock-raw-stop-reason.test.ts`
26. `bedrock-thinking-payload.test.ts`
27. `cache-retention.test.ts`
28. `cloudflare-stream.test.ts` — newly explicit in this hardening slice
29. `compat-env.test.ts`
30. `constrained-sampling.test.ts`
31. `context-estimate.test.ts`
32. `context-overflow.test.ts`
33. `cross-provider-handoff.test.ts`
34. `deferred-tools.test.ts`
35. `empty.test.ts`
36. `env-api-keys.test.ts`
37. `error-body.test.ts`
38. `faux-provider.test.ts`
39. `fetch-option.test.ts`
40. `fireworks-models.test.ts`
41. `github-copilot-anthropic.test.ts`
42. `github-copilot-oauth.test.ts`
43. `google-raw-stop-reason.test.ts`
44. `google-shared-convert-tools.test.ts`
45. `google-shared-gemini3-unsigned-tool-call.test.ts`
46. `google-shared-image-tool-result-routing.test.ts`
47. `google-shared-retry.test.ts`
48. `google-shared-signed-empty-blocks.test.ts`
49. `google-thinking-disable.test.ts`
50. `google-thinking-signature.test.ts`
51. `google-vertex-api-key-resolution.test.ts`
52. `image-model-data.test.ts` — newly explicit in this hardening slice
53. `image-tool-result.test.ts`
54. `images-models.test.ts`
55. `images.test.ts`
56. `interleaved-thinking.test.ts`
57. `kimi-coding-oauth.test.ts`
58. `lax-message-content.test.ts`
59. `lazy-module-load.test.ts`
60. `max-thinking.test.ts`
61. `mistral-raw-stop-reason.test.ts`
62. `mistral-reasoning-mode.test.ts`
63. `mistral-tool-schema.test.ts`
64. `model-catalog-types.test.ts`
65. `model-data-validation.test.ts` — newly explicit in this hardening slice
66. `models-runtime.test.ts`
67. `node-http-proxy.test.ts`
68. `oauth-auth.test.ts`
69. `oauth-device-code.test.ts`
70. `openai-codex-cache-affinity-e2e.test.ts`
71. `openai-codex-oauth.test.ts`
72. `openai-codex-stream.test.ts`
73. `openai-completions-cache-control-format.test.ts`
74. `openai-completions-empty-tools.test.ts`
75. `openai-completions-prompt-cache.test.ts`
76. `openai-completions-raw-stop-reason.test.ts`
77. `openai-completions-reasoning-details.test.ts`
78. `openai-completions-response-model.test.ts`
79. `openai-completions-retry.test.ts`
80. `openai-completions-thinking-as-text.test.ts`
81. `openai-completions-thinking-token-budget.test.ts`
82. `openai-completions-tool-choice.test.ts`
83. `openai-completions-tool-result-images.test.ts`
84. `openai-responses-cache-affinity-e2e.test.ts`
85. `openai-responses-compat.test.ts`
86. `openai-responses-empty-tool-result.test.ts`
87. `openai-responses-foreign-toolcall-id.test.ts`
88. `openai-responses-message-id.test.ts`
89. `openai-responses-partial-json-cleanup.test.ts`
90. `openai-responses-reasoning-replay-e2e.test.ts`
91. `openai-responses-terminal-event.test.ts`
92. `openai-responses-tool-result-images.test.ts`
93. `openrouter-cache-control-models.test.ts` — newly explicit in this hardening slice
94. `openrouter-cache-write-repro.test.ts`
95. `openrouter-images.test.ts`
96. `openrouter-oauth.test.ts`
97. `overflow.test.ts`
98. `pi-messages.test.ts`
99. `provider-error-body-passthrough.test.ts`
100. `provider-error-body-regression.test.ts`
101. `provider-retry.test.ts` — newly explicit in this hardening slice
102. `providers.test.ts`
103. `qwen-token-plan-models.test.ts`
104. `radius-oauth.test.ts`
105. `reasoning-options.test.ts` — newly explicit in this hardening slice
106. `responseid.test.ts`
107. `retry.test.ts`
108. `sampling-options.test.ts`
109. `stream.test.ts`
110. `supports-xhigh.test.ts`
111. `telemetry-options.test.ts`
112. `text.test.ts`
113. `together-models.test.ts`
114. `tokens.test.ts`
115. `tool-call-id-normalization.test.ts`
116. `tool-call-without-result.test.ts`
117. `total-tokens.test.ts`
118. `transform-messages-copilot-openai-to-anthropic.test.ts`
119. `unicode-surrogate.test.ts`
120. `uuid.test.ts` — newly explicit in this hardening slice
121. `validation.test.ts`
122. `xai-oauth.test.ts`
123. `xai-responses.test.ts` — newly explicit in this hardening slice
124. `xhigh.test.ts`
125. `xiaomi-models.test.ts`
126. `xiaomi-token-plan-ams-anthropic-empty-signature-smoke.test.ts`
127. `zen.test.ts`
