#!/usr/bin/env python3
"""Validate v0.85.0 release audit manifests.

This checks both documentation dispositions and the exact authoritative release
inventories. The committed inventory files preserve the canonical byte content
whose SHA-256 values were recorded during release discovery.
"""
from __future__ import annotations

import argparse
import hashlib
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
RELEASE = ROOT / "RELEASE.md"
CROSSWALK = ROOT / "docs/v0850-142-test-crosswalk.md"
CHANGED_PATHS_MANIFEST = ROOT / "docs/manifests/v0850-changed-paths.txt"
TEST_CORPUS_MANIFEST = ROOT / "docs/manifests/v0850-test-corpus-142.txt"
EXPECTED_CHANGED_SHA256 = "db461a56838926cf60d4ae0196ed98fcc215616dacff013ad8c235bb8ad9b83f"
EXPECTED_TEST_CORPUS_SHA256 = "56f8742065a4ad01d73e5aee53035324f2e7333a735222ab15db870819e29065"
EXPECTED_CHANGED_PATHS = ['packages/ai/CHANGELOG.md', 'packages/ai/README.md', 'packages/ai/package.json', 'packages/ai/scripts/generate-models.ts', 'packages/ai/src/api/anthropic-messages.ts', 'packages/ai/src/api/cloudflare-ai-binding.ts', 'packages/ai/src/api/cloudflare-gateway-binding.ts', 'packages/ai/src/api/openai-codex-responses.ts', 'packages/ai/src/api/openai-completions.ts', 'packages/ai/src/api/openai-responses-shared.ts', 'packages/ai/src/api/openai-responses.ts', 'packages/ai/src/api/pi-messages.ts', 'packages/ai/src/index.ts', 'packages/ai/src/models.ts', 'packages/ai/src/providers/cloudflare-ai-gateway.ts', 'packages/ai/src/providers/faux.ts', 'packages/ai/src/providers/openrouter.ts', 'packages/ai/src/types.ts', 'packages/ai/src/utils/assistant-message-frame.ts', 'packages/ai/src/utils/node-http-proxy.ts', 'packages/ai/src/utils/retry.ts', 'packages/ai/src/utils/uuid.ts', 'packages/ai/test/anthropic-auth-token.test.ts', 'packages/ai/test/anthropic-cache-write-1h-cost.test.ts', 'packages/ai/test/anthropic-mid-conversation-effort.test.ts', 'packages/ai/test/anthropic-sse-parsing.test.ts', 'packages/ai/test/anthropic-thinking-binding-e2e.test.ts', 'packages/ai/test/assistant-message-frame.test.ts', 'packages/ai/test/baseten-models.test.ts', 'packages/ai/test/cloudflare-ai-binding.test.ts', 'packages/ai/test/cloudflare-gateway-binding.test.ts', 'packages/ai/test/constrained-sampling.test.ts', 'packages/ai/test/generate-models-strict.test.ts', 'packages/ai/test/github-copilot-anthropic.test.ts', 'packages/ai/test/github-copilot-oauth.test.ts', 'packages/ai/test/node-http-proxy.test.ts', 'packages/ai/test/openai-codex-stream.test.ts', 'packages/ai/test/openai-completions-cache-control-format.test.ts', 'packages/ai/test/openai-completions-thinking-as-text.test.ts', 'packages/ai/test/openai-completions-tool-choice.test.ts', 'packages/ai/test/openai-completions-tool-result-images.test.ts', 'packages/ai/test/openai-completions-vllm-priority.test.ts', 'packages/ai/test/openai-responses-compat.test.ts', 'packages/ai/test/openai-responses-namespace.test.ts', 'packages/ai/test/openrouter-cache-control-models.test.ts', 'packages/ai/test/pi-messages.test.ts', 'packages/ai/test/pre-generation-error.test.ts', 'packages/ai/test/qwen-token-plan-models.test.ts', 'packages/ai/test/tool-call-id-normalization.test.ts', 'packages/ai/test/uuid.test.ts', 'packages/ai/test/xai-responses.test.ts']
EXPECTED_TEST_FILES = ['abort.test.ts', 'anthropic-adaptive-thinking-models.test.ts', 'anthropic-auth-token.test.ts', 'anthropic-cache-write-1h-cost.test.ts', 'anthropic-eager-tool-input-compat.test.ts', 'anthropic-eager-tool-input-e2e.test.ts', 'anthropic-empty-thinking-signature-compat.test.ts', 'anthropic-force-adaptive-thinking.test.ts', 'anthropic-long-cache-retention-e2e.test.ts', 'anthropic-mid-conversation-effort.test.ts', 'anthropic-oauth.test.ts', 'anthropic-opus-4-8-smoke.test.ts', 'anthropic-sse-parsing.test.ts', 'anthropic-temperature-compat.test.ts', 'anthropic-thinking-binding-e2e.test.ts', 'anthropic-thinking-disable.test.ts', 'anthropic-tool-name-normalization.test.ts', 'assistant-message-frame.test.ts', 'azure-openai-base-url.test.ts', 'azure-openai-responses-reasoning-replay.test.ts', 'azure-openai-tool-choice.test.ts', 'baseten-models.test.ts', 'bedrock-convert-messages.test.ts', 'bedrock-credentials.test.ts', 'bedrock-custom-headers.test.ts', 'bedrock-endpoint-resolution.test.ts', 'bedrock-error-metadata.test.ts', 'bedrock-models.test.ts', 'bedrock-raw-stop-reason.test.ts', 'bedrock-redacted-reasoning.test.ts', 'bedrock-response-headers.test.ts', 'bedrock-thinking-payload.test.ts', 'cache-retention.test.ts', 'cloudflare-ai-binding.test.ts', 'cloudflare-stream.test.ts', 'compat-env.test.ts', 'constrained-sampling.test.ts', 'context-estimate.test.ts', 'context-overflow.test.ts', 'cross-provider-handoff.test.ts', 'deferred-tools.test.ts', 'empty.test.ts', 'env-api-keys.test.ts', 'error-body.test.ts', 'faux-provider.test.ts', 'fetch-option.test.ts', 'fireworks-models.test.ts', 'generate-models-strict.test.ts', 'github-copilot-anthropic.test.ts', 'github-copilot-oauth.test.ts', 'google-raw-stop-reason.test.ts', 'google-shared-convert-tools.test.ts', 'google-shared-gemini3-unsigned-tool-call.test.ts', 'google-shared-image-tool-result-routing.test.ts', 'google-shared-retry.test.ts', 'google-shared-signed-empty-blocks.test.ts', 'google-thinking-disable.test.ts', 'google-thinking-level-map.test.ts', 'google-thinking-signature.test.ts', 'google-vertex-api-key-resolution.test.ts', 'image-model-data.test.ts', 'image-tool-result.test.ts', 'images-models.test.ts', 'images.test.ts', 'interleaved-thinking.test.ts', 'kimi-coding-oauth.test.ts', 'lax-message-content.test.ts', 'lazy-module-load.test.ts', 'max-thinking.test.ts', 'mistral-http-transport.test.ts', 'mistral-raw-stop-reason.test.ts', 'mistral-reasoning-mode.test.ts', 'mistral-tool-schema.test.ts', 'model-catalog-types.test.ts', 'model-data-validation.test.ts', 'models-runtime.test.ts', 'node-http-proxy.test.ts', 'oauth-auth.test.ts', 'oauth-device-code.test.ts', 'openai-codex-cache-affinity-e2e.test.ts', 'openai-codex-oauth.test.ts', 'openai-codex-stream.test.ts', 'openai-completions-cache-control-format.test.ts', 'openai-completions-empty-tools.test.ts', 'openai-completions-prompt-cache.test.ts', 'openai-completions-raw-stop-reason.test.ts', 'openai-completions-reasoning-details.test.ts', 'openai-completions-response-model.test.ts', 'openai-completions-retry.test.ts', 'openai-completions-thinking-as-text.test.ts', 'openai-completions-thinking-token-budget.test.ts', 'openai-completions-tool-choice.test.ts', 'openai-completions-tool-result-images.test.ts', 'openai-completions-vllm-priority.test.ts', 'openai-responses-cache-affinity-e2e.test.ts', 'openai-responses-compat.test.ts', 'openai-responses-empty-tool-result.test.ts', 'openai-responses-foreign-toolcall-id.test.ts', 'openai-responses-message-id.test.ts', 'openai-responses-namespace.test.ts', 'openai-responses-partial-json-cleanup.test.ts', 'openai-responses-reasoning-replay-e2e.test.ts', 'openai-responses-terminal-event.test.ts', 'openai-responses-tool-result-images.test.ts', 'openrouter-cache-control-models.test.ts', 'openrouter-cache-write-repro.test.ts', 'openrouter-images.test.ts', 'openrouter-oauth.test.ts', 'openrouter-reasoning-options.test.ts', 'overflow.test.ts', 'pi-messages.test.ts', 'pre-generation-error.test.ts', 'provider-error-body-passthrough.test.ts', 'provider-error-body-regression.test.ts', 'provider-retry.test.ts', 'providers.test.ts', 'qwen-token-plan-models.test.ts', 'radius-oauth.test.ts', 'reasoning-options.test.ts', 'responseid.test.ts', 'retry.test.ts', 'sampling-options.test.ts', 'stream.test.ts', 'supports-xhigh.test.ts', 'telemetry-options.test.ts', 'text.test.ts', 'together-models.test.ts', 'tokens.test.ts', 'tool-call-id-normalization.test.ts', 'tool-call-without-result.test.ts', 'total-tokens.test.ts', 'transform-messages-copilot-openai-to-anthropic.test.ts', 'unicode-surrogate.test.ts', 'uuid.test.ts', 'validation.test.ts', 'xai-oauth.test.ts', 'xai-responses.test.ts', 'xhigh.test.ts', 'xiaomi-models.test.ts', 'xiaomi-token-plan-ams-anthropic-empty-signature-smoke.test.ts', 'zai-coding-plan-models.test.ts', 'zen.test.ts']


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def read_manifest(path: Path, fault: str, expected_hash: str, expected_lines: int) -> tuple[list[str], str]:
    data = path.read_bytes()
    if fault == path.stem:
        data += b"# deliberate corruption\n"
    digest = sha256_bytes(data)
    lines = data.decode().splitlines()
    if digest != expected_hash:
        raise ValueError(f"{path.name} sha256 mismatch: got {digest}, expected {expected_hash}")
    if len(lines) != expected_lines:
        raise ValueError(f"{path.name} row count mismatch: got {len(lines)}, expected {expected_lines}")
    return lines, digest


def manifest_changed_paths(lines: list[str]) -> set[str]:
    paths = set()
    for line in lines:
        cols = line.split("\t", 1)
        if len(cols) != 2 or cols[0] not in {"A", "M", "D"}:
            raise ValueError(f"invalid changed-path inventory row: {line!r}")
        paths.add(cols[1])
    return paths


def manifest_test_files(lines: list[str]) -> set[str]:
    files = set()
    for line in lines:
        if not line.startswith("packages/ai/test/") or not line.endswith(".test.ts"):
            raise ValueError(f"invalid test-corpus inventory row: {line!r}")
        files.add(line.rsplit("/", 1)[1])
    return files


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


def validate(fault: str = "") -> str:
    changed_lines, changed_sha = read_manifest(
        CHANGED_PATHS_MANIFEST,
        fault,
        EXPECTED_CHANGED_SHA256,
        51,
    )
    test_lines, corpus_sha = read_manifest(
        TEST_CORPUS_MANIFEST,
        fault,
        EXPECTED_TEST_CORPUS_SHA256,
        142,
    )
    failures: list[str] = []
    expected_changed = set(EXPECTED_CHANGED_PATHS)
    got_manifest_changed = manifest_changed_paths(changed_lines)
    got_doc_changed = release_changed_paths()
    for label, got_changed in [("inventory", got_manifest_changed), ("release docs", got_doc_changed)]:
        missing = sorted(expected_changed - got_changed)
        extra = sorted(p for p in got_changed - expected_changed if p.startswith("packages/ai/") and "*" not in p)
        if missing or extra:
            failures.append(f"v0.85.0 changed-path matrix mismatch in {label}\nmissing={missing}\nextra={extra}")
    rows = crosswalk_rows()
    expected_tests = set(EXPECTED_TEST_FILES)
    got_manifest_tests = manifest_test_files(test_lines)
    for label, got_tests in [("inventory", got_manifest_tests), ("crosswalk", set(rows.keys()))]:
        missing_tests = sorted(expected_tests - got_tests)
        extra_tests = sorted(got_tests - expected_tests)
        if len(got_tests) != 142 or missing_tests or extra_tests:
            failures.append(f"v0.85.0 142-test corpus mismatch in {label}\nrows={len(got_tests)} missing={missing_tests} extra={extra_tests}")
    invalid = [name for name, line in rows.items() if all(token not in line for token in ["| ADAPTED", "| COVERED", "| LIVE UNEXECUTED", "| N/A"])]
    if invalid:
        failures.append(f"crosswalk rows without disposition: {invalid}")
    if failures:
        raise ValueError("\n".join(failures))
    return (
        "v0.85.0 manifests verified: changedPaths=51 testRows=142 "
        f"changedSha256={changed_sha} testCorpusSha256={corpus_sha}"
    )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--fault", choices=["", "v0850-changed-paths", "v0850-test-corpus-142"], default="")
    args = parser.parse_args()
    try:
        print(validate(args.fault))
        return 0
    except ValueError as exc:
        print(str(exc), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
