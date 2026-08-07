# v0.84.1 cumulative 128-file test crosswalk

Source: official upstream `@earendil-works/pi-ai` tag `53fa77ccd8a279eb87e92294ef3687b03ff80112` (`v0.84.1`). Baseline: accepted `v0.84.0` tag `a5f43bf8aff3c55752432655f7334e3dafd1e256`.

Total upstream `packages/ai/test/*.test.ts` files: **128**.

## Release-delta test accounting

The bounded v0.84.1 range changes **14 test paths total**:

- **13 existing tests modified**: `abort`, `context-overflow`, `cross-provider-handoff`, `empty`, `image-tool-result`, `model-data-validation`, `openai-completions-tool-choice`, `qwen-token-plan-models`, `stream`, `tokens`, `tool-call-without-result`, `total-tokens`, `unicode-surrogate`.
- **1 new test added**: `generate-models-strict.test.ts`.

Disposition: the modified live/runtime tests add `qwen-token-plan-individual` coverage to the existing Qwen Token Plan provider matrix and remain deterministic in Rust via registry/env/payload assertions. The new strict generator test is adapted through `scripts/extract_release_model_shards.py` and `src/model_data_validation_test.rs`, which enforce the exact Individual allowlist before writing generated output.

## Newly explicit v0.84.1 dispositions

| Upstream file | Disposition | rs-ai evidence | Notes |
|---|---|---|---|
| `generate-models-strict.test.ts` | ADAPTED | `scripts/extract_release_model_shards.py::assert_exact_model_ids`, `src/model_data_validation_test.rs::extractor_enforces_qwen_individual_strict_model_ids_without_output_mutation` | Exact Qwen Token Plan Individual allowlist is enforced (`deepseek-v4-flash-0731`, `deepseek-v4-pro`, `glm-5.2`, `qwen3.6-flash`, `qwen3.7-max`, `qwen3.7-plus`, `qwen3.8-max`); drift fails before output mutation. |
| `qwen-token-plan-models.test.ts` | ADAPTED | `src/v0841_release_test.rs::{qwen_token_plan_individual_catalog_env_and_endpoint_match_v0841,qwen_token_plan_individual_reasoning_payloads_match_v0841}` | Individual provider registered; shares `QWEN_TOKEN_PLAN_API_KEY`; exact 7-model catalog; retired `qwen3.8-max-preview` omitted; `thinkingFormat=qwen` emits `enable_thinking` and supported `reasoning_effort`. |
| `model-data-validation.test.ts` | ADAPTED | `scripts/validate_release_model_data.py`, `scripts/extract_release_model_shards.py`, `src/model_data_validation_test.rs` | Existing strict shard validation remains; extractor now preserves official release-pinned `:batch` aliases only when exactly allowlisted. |
| 12 live/provider tests adding Individual provider cases | COVERED | `src/v0841_release_test.rs` and existing provider stream suites | Upstream adds Individual provider cases to live Qwen/simple/tool/tokens/overflow/abort/handoff/unicode matrices. Rust covers request/catalog contract deterministically without live credentials. |

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
28. `cloudflare-stream.test.ts`
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
41. `generate-models-strict.test.ts` — new in v0.84.1; adapted
42. `github-copilot-anthropic.test.ts`
43. `github-copilot-oauth.test.ts`
44. `google-raw-stop-reason.test.ts`
45. `google-shared-convert-tools.test.ts`
46. `google-shared-gemini3-unsigned-tool-call.test.ts`
47. `google-shared-image-tool-result-routing.test.ts`
48. `google-shared-retry.test.ts`
49. `google-shared-signed-empty-blocks.test.ts`
50. `google-thinking-disable.test.ts`
51. `google-thinking-signature.test.ts`
52. `google-vertex-api-key-resolution.test.ts`
53. `image-model-data.test.ts`
54. `image-tool-result.test.ts`
55. `images-models.test.ts`
56. `images.test.ts`
57. `interleaved-thinking.test.ts`
58. `kimi-coding-oauth.test.ts`
59. `lax-message-content.test.ts`
60. `lazy-module-load.test.ts`
61. `max-thinking.test.ts`
62. `mistral-raw-stop-reason.test.ts`
63. `mistral-reasoning-mode.test.ts`
64. `mistral-tool-schema.test.ts`
65. `model-catalog-types.test.ts`
66. `model-data-validation.test.ts`
67. `models-runtime.test.ts`
68. `node-http-proxy.test.ts`
69. `oauth-auth.test.ts`
70. `oauth-device-code.test.ts`
71. `openai-codex-cache-affinity-e2e.test.ts`
72. `openai-codex-oauth.test.ts`
73. `openai-codex-stream.test.ts`
74. `openai-completions-cache-control-format.test.ts`
75. `openai-completions-empty-tools.test.ts`
76. `openai-completions-prompt-cache.test.ts`
77. `openai-completions-raw-stop-reason.test.ts`
78. `openai-completions-reasoning-details.test.ts`
79. `openai-completions-response-model.test.ts`
80. `openai-completions-retry.test.ts`
81. `openai-completions-thinking-as-text.test.ts`
82. `openai-completions-thinking-token-budget.test.ts`
83. `openai-completions-tool-choice.test.ts`
84. `openai-completions-tool-result-images.test.ts`
85. `openai-responses-cache-affinity-e2e.test.ts`
86. `openai-responses-compat.test.ts`
87. `openai-responses-empty-tool-result.test.ts`
88. `openai-responses-foreign-toolcall-id.test.ts`
89. `openai-responses-message-id.test.ts`
90. `openai-responses-partial-json-cleanup.test.ts`
91. `openai-responses-reasoning-replay-e2e.test.ts`
92. `openai-responses-terminal-event.test.ts`
93. `openai-responses-tool-result-images.test.ts`
94. `openrouter-cache-control-models.test.ts`
95. `openrouter-cache-write-repro.test.ts`
96. `openrouter-images.test.ts`
97. `openrouter-oauth.test.ts`
98. `overflow.test.ts`
99. `pi-messages.test.ts`
100. `provider-error-body-passthrough.test.ts`
101. `provider-error-body-regression.test.ts`
102. `provider-retry.test.ts`
103. `providers.test.ts`
104. `qwen-token-plan-models.test.ts`
105. `radius-oauth.test.ts`
106. `reasoning-options.test.ts`
107. `responseid.test.ts`
108. `retry.test.ts`
109. `sampling-options.test.ts`
110. `stream.test.ts`
111. `supports-xhigh.test.ts`
112. `telemetry-options.test.ts`
113. `text.test.ts`
114. `together-models.test.ts`
115. `tokens.test.ts`
116. `tool-call-id-normalization.test.ts`
117. `tool-call-without-result.test.ts`
118. `total-tokens.test.ts`
119. `transform-messages-copilot-openai-to-anthropic.test.ts`
120. `unicode-surrogate.test.ts`
121. `uuid.test.ts`
122. `validation.test.ts`
123. `xai-oauth.test.ts`
124. `xai-responses.test.ts`
125. `xhigh.test.ts`
126. `xiaomi-models.test.ts`
127. `xiaomi-token-plan-ams-anthropic-empty-signature-smoke.test.ts`
128. `zen.test.ts`
