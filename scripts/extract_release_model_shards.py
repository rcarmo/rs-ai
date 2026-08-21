#!/usr/bin/env python3
"""Extract release-pinned pi-ai provider data shards into models.json.

This script is intentionally offline: it reads the official package artifact's
`dist/providers/data/*.json` shards and never calls models.dev/OpenRouter APIs.
Use it for release audits instead of `packages/ai/scripts/generate-models.ts`,
which can fetch dynamic post-tag catalogs.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
from pathlib import Path


def flatten(value):
    out = []
    if isinstance(value, list):
        for item in value:
            out.extend(flatten(item))
    elif isinstance(value, dict):
        if isinstance(value.get("provider"), str) and isinstance(value.get("id"), str):
            out.append(value)
        else:
            for item in value.values():
                out.extend(flatten(item))
    return out


# Official @earendil-works/pi-ai v0.84.1 provider shards include these
# release-pinned OpenRouter batch aliases. Keep this list exact so fresh dynamic
# models.dev/OpenRouter aliases still fail loudly during release audits.
QWEN_TOKEN_PLAN_INDIVIDUAL_MODEL_IDS = {
    "deepseek-v4-flash-0731",
    "deepseek-v4-pro",
    "glm-5.2",
    "qwen3.6-flash",
    "qwen3.7-max",
    "qwen3.7-plus",
    "qwen3.8-max",
}

ALLOWED_BATCH_ALIASES = {
    "openrouter/anthropic/claude-fable-5:batch",
    "openrouter/anthropic/claude-haiku-4.5:batch",
    "openrouter/anthropic/claude-opus-4.1:batch",
    "openrouter/anthropic/claude-opus-4.5:batch",
    "openrouter/anthropic/claude-opus-4.6:batch",
    "openrouter/anthropic/claude-opus-4.7:batch",
    "openrouter/anthropic/claude-opus-4.8:batch",
    "openrouter/anthropic/claude-opus-5:batch",
    "openrouter/anthropic/claude-sonnet-4.5:batch",
    "openrouter/anthropic/claude-sonnet-4.6:batch",
    "openrouter/anthropic/claude-sonnet-5:batch",
    "openrouter/google/gemini-2.5-flash-lite:batch",
    "openrouter/google/gemini-2.5-flash:batch",
    "openrouter/google/gemini-2.5-pro:batch",
    "openrouter/google/gemini-3-flash-preview:batch",
    "openrouter/google/gemini-3.1-flash-lite:batch",
    "openrouter/google/gemini-3.1-pro-preview:batch",
    "openrouter/google/gemini-3.5-flash-lite:batch",
    "openrouter/google/gemini-3.5-flash:batch",
    "openrouter/google/gemini-3.6-flash:batch",
    "openrouter/google/gemini-3.7-flash:batch",
    "openrouter/minimax/minimax-m3:batch",
    "openrouter/moonshotai/kimi-k2.7-code:batch",
    "openrouter/nvidia/nemotron-3-ultra-550b-a55b:batch",
    "openrouter/openai/gpt-3.5-turbo:batch",
    "openrouter/openai/gpt-4-turbo:batch",
    "openrouter/openai/gpt-4.1-mini:batch",
    "openrouter/openai/gpt-4.1-nano:batch",
    "openrouter/openai/gpt-4.1:batch",
    "openrouter/openai/gpt-4o-mini:batch",
    "openrouter/openai/gpt-4o:batch",
    "openrouter/openai/gpt-5-codex:batch",
    "openrouter/openai/gpt-5-mini:batch",
    "openrouter/openai/gpt-5-nano:batch",
    "openrouter/openai/gpt-5-pro:batch",
    "openrouter/openai/gpt-5.1:batch",
    "openrouter/openai/gpt-5.2-pro:batch",
    "openrouter/openai/gpt-5.2:batch",
    "openrouter/openai/gpt-5.4-mini:batch",
    "openrouter/openai/gpt-5.4-nano:batch",
    "openrouter/openai/gpt-5.4-pro:batch",
    "openrouter/openai/gpt-5.4:batch",
    "openrouter/openai/gpt-5.5-pro:batch",
    "openrouter/openai/gpt-5.5:batch",
    "openrouter/openai/gpt-5.6-luna-pro:batch",
    "openrouter/openai/gpt-5.6-luna:batch",
    "openrouter/openai/gpt-5.6-sol-pro:batch",
    "openrouter/openai/gpt-5.6-sol:batch",
    "openrouter/openai/gpt-5.6-terra-pro:batch",
    "openrouter/openai/gpt-5.6-terra:batch",
    "openrouter/openai/gpt-5:batch",
    "openrouter/openai/o1:batch",
    "openrouter/openai/o3-mini-high:batch",
    "openrouter/openai/o3-mini:batch",
    "openrouter/openai/o3-pro:batch",
    "openrouter/openai/o3:batch",
    "openrouter/openai/o4-mini-high:batch",
    "openrouter/openai/o4-mini:batch",
    "openrouter/thinkingmachines/inkling:batch",
    "openrouter/z-ai/glm-5.2:batch",
}


def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def assert_exact_model_ids(label: str, expected: set[str], actual: set[str]) -> None:
    missing = sorted(expected - actual)
    extra = sorted(actual - expected)
    if missing or extra:
        parts = []
        if missing:
            parts.append("missing: " + ", ".join(missing))
        if extra:
            parts.append("extra: " + ", ".join(extra))
        raise SystemExit(f"{label} model IDs do not match (" + "; ".join(parts) + ")")


def git_rev_parse(repo: Path, ref: str) -> str | None:
    try:
        return subprocess.check_output(["git", "rev-parse", ref], cwd=repo, text=True).strip()
    except Exception:
        return None


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("package_dir", help="unpacked npm package dir containing dist/providers/data")
    ap.add_argument("out_dir", help="output directory for models.json and source-metadata.json")
    ap.add_argument("--tag-worktree", default="", help="exact upstream git worktree used for source SHA evidence")
    ap.add_argument("--tag-sha", default="", help="expected upstream tag SHA")
    args = ap.parse_args()

    package_dir = Path(args.package_dir).resolve()
    data_dir = package_dir / "dist" / "providers" / "data"
    if not data_dir.is_dir():
        raise SystemExit(f"missing provider shard directory: {data_dir}")
    out_dir = Path(args.out_dir).resolve()

    manifest_path = data_dir / ".manifest.json"
    manifest = json.loads(manifest_path.read_text()) if manifest_path.exists() else {}

    models = {}
    shard_hashes = {}
    batch_ids = []
    for path in sorted(data_dir.glob("*.json")):
        if path.name == ".manifest.json":
            continue
        provider = path.stem
        shard_hashes[path.name] = sha256(path)
        entries = {m["id"]: m for m in flatten(json.loads(path.read_text()))}
        for model_id in entries:
            if ":batch" in model_id:
                batch_ids.append(f"{provider}/{model_id}")
        models[provider] = dict(sorted(entries.items()))

    batch_ids = sorted(batch_ids)
    unexpected_batch_ids = sorted(set(batch_ids) - ALLOWED_BATCH_ALIASES)
    if unexpected_batch_ids:
        raise SystemExit(
            "release provider shards contain unaudited :batch ids; update audit policy with exact artifact evidence first:\n"
            + "\n".join(unexpected_batch_ids[:100])
        )
    if "qwen-token-plan-individual" in models:
        assert_exact_model_ids(
            "qwen-token-plan-individual",
            QWEN_TOKEN_PLAN_INDIVIDUAL_MODEL_IDS,
            set(models["qwen-token-plan-individual"].keys()),
        )

    out_dir.mkdir(parents=True, exist_ok=True)
    (out_dir / "models.json").write_text(json.dumps(models, indent=2, sort_keys=True) + "\n")
    package_json = package_dir / "package.json"
    metadata = {
        "source": "npm-dist-provider-shards",
        "packageDir": str(package_dir),
        "packageJsonSha256": sha256(package_json) if package_json.exists() else None,
        "providerDataDir": str(data_dir),
        "manifest": manifest,
        "manifestSha256": sha256(manifest_path) if manifest_path.exists() else None,
        "providerShardSha256": shard_hashes,
        "providerCount": len(models),
        "modelCount": sum(len(v) for v in models.values()),
        "apiCount": len({m["api"] for mods in models.values() for m in mods.values()}),
        "batchAliasCount": len(batch_ids),
        "batchAliases": batch_ids,
        "allowedBatchAliasPolicySha256": hashlib.sha256(
            "\n".join(sorted(ALLOWED_BATCH_ALIASES)).encode()
        ).hexdigest(),
        "qwenTokenPlanIndividualModelIds": sorted(QWEN_TOKEN_PLAN_INDIVIDUAL_MODEL_IDS),
    }
    if args.tag_worktree:
        tag_worktree = Path(args.tag_worktree).resolve()
        metadata["tagWorktree"] = str(tag_worktree)
        metadata["tagHead"] = git_rev_parse(tag_worktree, "HEAD")
        if args.tag_sha:
            metadata["expectedTagSha"] = args.tag_sha
            if metadata.get("tagHead") != args.tag_sha:
                raise SystemExit(f"tag worktree HEAD mismatch: {metadata.get('tagHead')} != {args.tag_sha}")
    (out_dir / "source-metadata.json").write_text(json.dumps(metadata, indent=2, sort_keys=True) + "\n")
    print(f"wrote {out_dir / 'models.json'} ({metadata['modelCount']} models, {metadata['providerCount']} providers, {metadata['apiCount']} apis)")
    print(f"wrote {out_dir / 'source-metadata.json'}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
