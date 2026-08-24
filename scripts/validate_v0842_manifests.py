#!/usr/bin/env python3
"""Validate v0.84.2 release audit manifests.

This executable gate keeps the release ledger mechanical: the documented
upstream changed-path set must match the official 42-path range exactly, and
the cumulative test crosswalk must contain one unique per-file disposition row
for each of the 131 upstream `packages/ai/test/*.test.ts` files.
"""
from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
RELEASE = ROOT / "RELEASE.md"
CROSSWALK = ROOT / "docs/v0842-131-test-crosswalk.md"

EXPECTED_CHANGED_PATHS = ['packages/ai/CHANGELOG.md', 'packages/ai/package.json', 'packages/ai/scripts/generate-models.ts', 'packages/ai/src/api/anthropic-messages.ts', 'packages/ai/src/api/bedrock-converse-stream.ts', 'packages/ai/src/api/cloudflare-gateway-binding.ts', 'packages/ai/src/api/constrained-sampling.ts', 'packages/ai/src/api/google-generative-ai.ts', 'packages/ai/src/api/google-shared.ts', 'packages/ai/src/api/google-vertex.ts', 'packages/ai/src/api/mistral-conversations.ts', 'packages/ai/src/api/openai-codex-responses.ts', 'packages/ai/src/api/openai-completions.ts', 'packages/ai/src/api/openai-responses-shared.ts', 'packages/ai/src/api/openai-responses.ts', 'packages/ai/src/auth/oauth/github-copilot.ts', 'packages/ai/src/image-models.generated.ts', 'packages/ai/src/types.ts', 'packages/ai/src/utils/pi-user-agent.ts', 'packages/ai/src/utils/retry.ts', 'packages/ai/src/utils/validation.ts', 'packages/ai/test/anthropic-auth-token.test.ts', 'packages/ai/test/anthropic-eager-tool-input-compat.test.ts', 'packages/ai/test/bedrock-convert-messages.test.ts', 'packages/ai/test/cloudflare-gateway-binding.test.ts', 'packages/ai/test/constrained-sampling.test.ts', 'packages/ai/test/context-overflow.test.ts', 'packages/ai/test/deferred-tools.test.ts', 'packages/ai/test/github-copilot-oauth.test.ts', 'packages/ai/test/google-raw-stop-reason.test.ts', 'packages/ai/test/lazy-module-load.test.ts', 'packages/ai/test/mistral-http-transport.test.ts', 'packages/ai/test/mistral-raw-stop-reason.test.ts', 'packages/ai/test/openai-codex-stream.test.ts', 'packages/ai/test/openai-completions-tool-choice.test.ts', 'packages/ai/test/openai-responses-compat.test.ts', 'packages/ai/test/openai-responses-namespace.test.ts', 'packages/ai/test/retry.test.ts', 'packages/ai/test/stream.test.ts', 'packages/ai/test/supports-xhigh.test.ts', 'packages/ai/test/total-tokens.test.ts', 'packages/ai/test/validation.test.ts']
EXPECTED_TEST_FILES = ['abort.test.ts', 'anthropic-adaptive-thinking-models.test.ts', 'anthropic-auth-token.test.ts', 'anthropic-cache-write-1h-cost.test.ts', 'anthropic-eager-tool-input-compat.test.ts', 'anthropic-eager-tool-input-e2e.test.ts', 'anthropic-empty-thinking-signature-compat.test.ts', 'anthropic-force-adaptive-thinking.test.ts', 'anthropic-long-cache-retention-e2e.test.ts', 'anthropic-oauth.test.ts', 'anthropic-opus-4-8-smoke.test.ts', 'anthropic-sse-parsing.test.ts', 'anthropic-temperature-compat.test.ts', 'anthropic-thinking-disable.test.ts', 'anthropic-tool-name-normalization.test.ts', 'azure-openai-base-url.test.ts', 'azure-openai-responses-reasoning-replay.test.ts', 'baseten-models.test.ts', 'bedrock-convert-messages.test.ts', 'bedrock-credentials.test.ts', 'bedrock-custom-headers.test.ts', 'bedrock-endpoint-resolution.test.ts', 'bedrock-error-metadata.test.ts', 'bedrock-models.test.ts', 'bedrock-raw-stop-reason.test.ts', 'bedrock-thinking-payload.test.ts', 'cache-retention.test.ts', 'cloudflare-gateway-binding.test.ts', 'cloudflare-stream.test.ts', 'compat-env.test.ts', 'constrained-sampling.test.ts', 'context-estimate.test.ts', 'context-overflow.test.ts', 'cross-provider-handoff.test.ts', 'deferred-tools.test.ts', 'empty.test.ts', 'env-api-keys.test.ts', 'error-body.test.ts', 'faux-provider.test.ts', 'fetch-option.test.ts', 'fireworks-models.test.ts', 'generate-models-strict.test.ts', 'github-copilot-anthropic.test.ts', 'github-copilot-oauth.test.ts', 'google-raw-stop-reason.test.ts', 'google-shared-convert-tools.test.ts', 'google-shared-gemini3-unsigned-tool-call.test.ts', 'google-shared-image-tool-result-routing.test.ts', 'google-shared-retry.test.ts', 'google-shared-signed-empty-blocks.test.ts', 'google-thinking-disable.test.ts', 'google-thinking-signature.test.ts', 'google-vertex-api-key-resolution.test.ts', 'image-model-data.test.ts', 'image-tool-result.test.ts', 'images-models.test.ts', 'images.test.ts', 'interleaved-thinking.test.ts', 'kimi-coding-oauth.test.ts', 'lax-message-content.test.ts', 'lazy-module-load.test.ts', 'max-thinking.test.ts', 'mistral-http-transport.test.ts', 'mistral-raw-stop-reason.test.ts', 'mistral-reasoning-mode.test.ts', 'mistral-tool-schema.test.ts', 'model-catalog-types.test.ts', 'model-data-validation.test.ts', 'models-runtime.test.ts', 'node-http-proxy.test.ts', 'oauth-auth.test.ts', 'oauth-device-code.test.ts', 'openai-codex-cache-affinity-e2e.test.ts', 'openai-codex-oauth.test.ts', 'openai-codex-stream.test.ts', 'openai-completions-cache-control-format.test.ts', 'openai-completions-empty-tools.test.ts', 'openai-completions-prompt-cache.test.ts', 'openai-completions-raw-stop-reason.test.ts', 'openai-completions-reasoning-details.test.ts', 'openai-completions-response-model.test.ts', 'openai-completions-retry.test.ts', 'openai-completions-thinking-as-text.test.ts', 'openai-completions-thinking-token-budget.test.ts', 'openai-completions-tool-choice.test.ts', 'openai-completions-tool-result-images.test.ts', 'openai-responses-cache-affinity-e2e.test.ts', 'openai-responses-compat.test.ts', 'openai-responses-empty-tool-result.test.ts', 'openai-responses-foreign-toolcall-id.test.ts', 'openai-responses-message-id.test.ts', 'openai-responses-namespace.test.ts', 'openai-responses-partial-json-cleanup.test.ts', 'openai-responses-reasoning-replay-e2e.test.ts', 'openai-responses-terminal-event.test.ts', 'openai-responses-tool-result-images.test.ts', 'openrouter-cache-control-models.test.ts', 'openrouter-cache-write-repro.test.ts', 'openrouter-images.test.ts', 'openrouter-oauth.test.ts', 'overflow.test.ts', 'pi-messages.test.ts', 'provider-error-body-passthrough.test.ts', 'provider-error-body-regression.test.ts', 'provider-retry.test.ts', 'providers.test.ts', 'qwen-token-plan-models.test.ts', 'radius-oauth.test.ts', 'reasoning-options.test.ts', 'responseid.test.ts', 'retry.test.ts', 'sampling-options.test.ts', 'stream.test.ts', 'supports-xhigh.test.ts', 'telemetry-options.test.ts', 'text.test.ts', 'together-models.test.ts', 'tokens.test.ts', 'tool-call-id-normalization.test.ts', 'tool-call-without-result.test.ts', 'total-tokens.test.ts', 'transform-messages-copilot-openai-to-anthropic.test.ts', 'unicode-surrogate.test.ts', 'uuid.test.ts', 'validation.test.ts', 'xai-oauth.test.ts', 'xai-responses.test.ts', 'xhigh.test.ts', 'xiaomi-models.test.ts', 'xiaomi-token-plan-ams-anthropic-empty-signature-smoke.test.ts', 'zen.test.ts']


def current_release_section() -> str:
    text = RELEASE.read_text()
    marker = "## Current audit target: v0.84.2"
    if marker not in text:
        marker = "## Historical accepted release: v0.84.2"
    start = text.index(marker)
    end = text.find("## Historical accepted release:", start + len(marker))
    return text[start:] if end == -1 else text[start:end]


def release_changed_paths() -> set[str]:
    return set(re.findall(r"`(packages/ai/[^`]+)`", current_release_section()))


def crosswalk_rows() -> dict[str, str]:
    rows: dict[str, str] = {}
    in_matrix = False
    for line in CROSSWALK.read_text().splitlines():
        if line.startswith("## Per-file 131-test disposition matrix"):
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
    release_paths = release_changed_paths()
    expected_changed = set(EXPECTED_CHANGED_PATHS)
    missing = sorted(expected_changed - release_paths)
    extra = sorted(
        path
        for path in release_paths - expected_changed
        if path.startswith("packages/ai/") and "*" not in path
    )
    failures: list[str] = []
    if missing or extra:
        failures.append(
            "v0.84.2 changed-path matrix mismatch\n"
            f"missing={missing}\nextra={extra}"
        )

    rows = crosswalk_rows()
    expected_tests = set(EXPECTED_TEST_FILES)
    missing_tests = sorted(expected_tests - rows.keys())
    extra_tests = sorted(rows.keys() - expected_tests)
    if len(rows) != 131 or missing_tests or extra_tests:
        failures.append(
            "v0.84.2 131-test crosswalk mismatch\n"
            f"rows={len(rows)} missing={missing_tests} extra={extra_tests}"
        )
    invalid_rows = [
        name
        for name, line in rows.items()
        if "| ADAPTED" not in line
        and "| COVERED" not in line
        and "| LIVE UNEXECUTED" not in line
        and "| N/A" not in line
    ]
    if invalid_rows:
        failures.append(f"crosswalk rows without disposition: {invalid_rows}")
    if failures:
        print("\n".join(failures), file=sys.stderr)
        return 1
    print("v0.84.2 manifests verified: changedPaths=42 testRows=131")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
