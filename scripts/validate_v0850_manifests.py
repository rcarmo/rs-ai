#!/usr/bin/env python3
"""Validate v0.85.0 release audit manifests."""
from __future__ import annotations
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
RELEASE = ROOT / "RELEASE.md"
CROSSWALK = ROOT / "docs/v0850-142-test-crosswalk.md"
EXPECTED_CHANGED_PATHS = ['packages/ai/CHANGELOG.md', 'packages/ai/README.md', 'packages/ai/package.json', 'packages/ai/scripts/generate-models.ts', 'packages/ai/src/api/anthropic-messages.ts', 'packages/ai/src/api/cloudflare-ai-binding.ts', 'packages/ai/src/api/cloudflare-gateway-binding.ts', 'packages/ai/src/api/openai-codex-responses.ts', 'packages/ai/src/api/openai-completions.ts', 'packages/ai/src/api/openai-responses-shared.ts', 'packages/ai/src/api/openai-responses.ts', 'packages/ai/src/api/pi-messages.ts', 'packages/ai/src/index.ts', 'packages/ai/src/models.ts', 'packages/ai/src/providers/cloudflare-ai-gateway.ts', 'packages/ai/src/providers/faux.ts', 'packages/ai/src/providers/openrouter.ts', 'packages/ai/src/types.ts', 'packages/ai/src/utils/assistant-message-frame.ts', 'packages/ai/src/utils/node-http-proxy.ts', 'packages/ai/src/utils/retry.ts', 'packages/ai/src/utils/uuid.ts', 'packages/ai/test/anthropic-auth-token.test.ts', 'packages/ai/test/anthropic-cache-write-1h-cost.test.ts', 'packages/ai/test/anthropic-mid-conversation-effort.test.ts', 'packages/ai/test/anthropic-sse-parsing.test.ts', 'packages/ai/test/anthropic-thinking-binding-e2e.test.ts', 'packages/ai/test/assistant-message-frame.test.ts', 'packages/ai/test/baseten-models.test.ts', 'packages/ai/test/cloudflare-ai-binding.test.ts', 'packages/ai/test/cloudflare-gateway-binding.test.ts', 'packages/ai/test/constrained-sampling.test.ts', 'packages/ai/test/generate-models-strict.test.ts', 'packages/ai/test/github-copilot-anthropic.test.ts', 'packages/ai/test/github-copilot-oauth.test.ts', 'packages/ai/test/node-http-proxy.test.ts', 'packages/ai/test/openai-codex-stream.test.ts', 'packages/ai/test/openai-completions-cache-control-format.test.ts', 'packages/ai/test/openai-completions-thinking-as-text.test.ts', 'packages/ai/test/openai-completions-tool-choice.test.ts', 'packages/ai/test/openai-completions-tool-result-images.test.ts', 'packages/ai/test/openai-completions-vllm-priority.test.ts', 'packages/ai/test/openai-responses-compat.test.ts', 'packages/ai/test/openai-responses-namespace.test.ts', 'packages/ai/test/openrouter-cache-control-models.test.ts', 'packages/ai/test/pi-messages.test.ts', 'packages/ai/test/pre-generation-error.test.ts', 'packages/ai/test/qwen-token-plan-models.test.ts', 'packages/ai/test/tool-call-id-normalization.test.ts', 'packages/ai/test/uuid.test.ts', 'packages/ai/test/xai-responses.test.ts']
EXPECTED_TEST_FILES = ['abort.test.ts', 'anthropic-adaptive-thinking-models.test.ts', 'anthropic-auth-token.test.ts', 'anthropic-cache-write-1h-cost.test.ts', 'anthropic-eager-tool-input-compat.test.ts', 'anthropic-eager-tool-input-e2e.test.ts', 'anthropic-empty-thinking-signature-compat.test.ts', 'anthropic-force-adaptive-thinking.test.ts', 'anthropic-long-cache-retention-e2e.test.ts', 'anthropic-mid-conversation-effort.test.ts', 'anthropic-oauth.test.ts', 'anthropic-opus-4-8-smoke.test.ts', 'anthropic-sse-parsing.test.ts', 'anthropic-temperature-compat.test.ts', 'anthropic-thinking-binding-e2e.test.ts', 'anthropic-thinking-disable.test.ts', 'anthropic-tool-name-normalization.test.ts', 'assistant-message-frame.test.ts', 'azure-openai-base-url.test.ts', 'azure-openai-responses-reasoning-replay.test.ts', 'azure-openai-tool-choice.test.ts', 'baseten-models.test.ts', 'bedrock-convert-messages.test.ts', 'bedrock-credentials.test.ts', 'bedrock-custom-headers.test.ts', 'bedrock-endpoint-resolution.test.ts', 'bedrock-error-metadata.test.ts', 'bedrock-models.test.ts', 'bedrock-raw-stop-reason.test.ts', 'bedrock-redacted-reasoning.test.ts', 'bedrock-response-headers.test.ts', 'bedrock-thinking-payload.test.ts', 'cache-retention.test.ts', 'cloudflare-ai-binding.test.ts', 'cloudflare-stream.test.ts', 'compat-env.test.ts', 'constrained-sampling.test.ts', 'context-estimate.test.ts', 'context-overflow.test.ts', 'cross-provider-handoff.test.ts', 'deferred-tools.test.ts', 'empty.test.ts', 'env-api-keys.test.ts', 'error-body.test.ts', 'faux-provider.test.ts', 'fetch-option.test.ts', 'fireworks-models.test.ts', 'generate-models-strict.test.ts', 'github-copilot-anthropic.test.ts', 'github-copilot-oauth.test.ts', 'google-raw-stop-reason.test.ts', 'google-shared-convert-tools.test.ts', 'google-shared-gemini3-unsigned-tool-call.test.ts', 'google-shared-image-tool-result-routing.test.ts', 'google-shared-retry.test.ts', 'google-shared-signed-empty-blocks.test.ts', 'google-thinking-disable.test.ts', 'google-thinking-level-map.test.ts', 'google-thinking-signature.test.ts', 'google-vertex-api-key-resolution.test.ts', 'image-model-data.test.ts', 'image-tool-result.test.ts', 'images-models.test.ts', 'images.test.ts', 'interleaved-thinking.test.ts', 'kimi-coding-oauth.test.ts', 'lax-message-content.test.ts', 'lazy-module-load.test.ts', 'max-thinking.test.ts', 'mistral-http-transport.test.ts', 'mistral-raw-stop-reason.test.ts', 'mistral-reasoning-mode.test.ts', 'mistral-tool-schema.test.ts', 'model-catalog-types.test.ts', 'model-data-validation.test.ts', 'models-runtime.test.ts', 'node-http-proxy.test.ts', 'oauth-auth.test.ts', 'oauth-device-code.test.ts', 'openai-codex-cache-affinity-e2e.test.ts', 'openai-codex-oauth.test.ts', 'openai-codex-stream.test.ts', 'openai-completions-cache-control-format.test.ts', 'openai-completions-empty-tools.test.ts', 'openai-completions-prompt-cache.test.ts', 'openai-completions-raw-stop-reason.test.ts', 'openai-completions-reasoning-details.test.ts', 'openai-completions-response-model.test.ts', 'openai-completions-retry.test.ts', 'openai-completions-thinking-as-text.test.ts', 'openai-completions-thinking-token-budget.test.ts', 'openai-completions-tool-choice.test.ts', 'openai-completions-tool-result-images.test.ts', 'openai-completions-vllm-priority.test.ts', 'openai-responses-cache-affinity-e2e.test.ts', 'openai-responses-compat.test.ts', 'openai-responses-empty-tool-result.test.ts', 'openai-responses-foreign-toolcall-id.test.ts', 'openai-responses-message-id.test.ts', 'openai-responses-namespace.test.ts', 'openai-responses-partial-json-cleanup.test.ts', 'openai-responses-reasoning-replay-e2e.test.ts', 'openai-responses-terminal-event.test.ts', 'openai-responses-tool-result-images.test.ts', 'openrouter-cache-control-models.test.ts', 'openrouter-cache-write-repro.test.ts', 'openrouter-images.test.ts', 'openrouter-oauth.test.ts', 'openrouter-reasoning-options.test.ts', 'overflow.test.ts', 'pi-messages.test.ts', 'pre-generation-error.test.ts', 'provider-error-body-passthrough.test.ts', 'provider-error-body-regression.test.ts', 'provider-retry.test.ts', 'providers.test.ts', 'qwen-token-plan-models.test.ts', 'radius-oauth.test.ts', 'reasoning-options.test.ts', 'responseid.test.ts', 'retry.test.ts', 'sampling-options.test.ts', 'stream.test.ts', 'supports-xhigh.test.ts', 'telemetry-options.test.ts', 'text.test.ts', 'together-models.test.ts', 'tokens.test.ts', 'tool-call-id-normalization.test.ts', 'tool-call-without-result.test.ts', 'total-tokens.test.ts', 'transform-messages-copilot-openai-to-anthropic.test.ts', 'unicode-surrogate.test.ts', 'uuid.test.ts', 'validation.test.ts', 'xai-oauth.test.ts', 'xai-responses.test.ts', 'xhigh.test.ts', 'xiaomi-models.test.ts', 'xiaomi-token-plan-ams-anthropic-empty-signature-smoke.test.ts', 'zai-coding-plan-models.test.ts', 'zen.test.ts']

def current_release_section() -> str:
    text = RELEASE.read_text()
    start = text.index("## Current audit target: v0.85.0")
    end = text.find("## Historical accepted release:", start)
    return text[start:] if end == -1 else text[start:end]

def release_changed_paths() -> set[str]:
    return set(re.findall(r"`(packages/ai/[^`]+)`", current_release_section()))

def crosswalk_rows() -> dict[str, str]:
    rows: dict[str, str] = {}
    in_matrix = False
    for line in CROSSWALK.read_text().splitlines():
        if line.startswith("## Per-file 142-test disposition matrix"):
            in_matrix = True
            continue
        if in_matrix and line.startswith("## "):
            break
        if not in_matrix or not line.startswith("| `"):
            continue
        cols = [col.strip() for col in line.strip().strip("|").split("|")]
        if len(cols) < 5:
            continue
        match = re.fullmatch(r"`([^`]+)`", cols[0])
        if match:
            rows[match.group(1)] = line
    return rows

def main() -> int:
    failures: list[str] = []
    expected_changed = set(EXPECTED_CHANGED_PATHS)
    got_changed = release_changed_paths()
    missing = sorted(expected_changed - got_changed)
    extra = sorted(p for p in got_changed - expected_changed if p.startswith("packages/ai/") and "*" not in p)
    if missing or extra:
        failures.append(f"v0.85.0 changed-path matrix mismatch\nmissing={missing}\nextra={extra}")
    rows = crosswalk_rows()
    expected_tests = set(EXPECTED_TEST_FILES)
    missing_tests = sorted(expected_tests - rows.keys())
    extra_tests = sorted(rows.keys() - expected_tests)
    if len(rows) != 142 or missing_tests or extra_tests:
        failures.append(f"v0.85.0 142-test crosswalk mismatch\nrows={len(rows)} missing={missing_tests} extra={extra_tests}")
    invalid = [name for name, line in rows.items() if all(token not in line for token in ["| ADAPTED", "| COVERED", "| LIVE UNEXECUTED", "| N/A"])]
    if invalid:
        failures.append(f"crosswalk rows without disposition: {invalid}")
    if failures:
        print("\n".join(failures), file=sys.stderr)
        return 1
    print("v0.85.0 manifests verified: changedPaths=51 testRows=142")
    return 0

if __name__ == "__main__":
    raise SystemExit(main())
