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


def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


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
    out_dir.mkdir(parents=True, exist_ok=True)

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

    if batch_ids:
        raise SystemExit(
            "release provider shards contain :batch ids; update audit policy with exact artifact evidence first:\n"
            + "\n".join(batch_ids[:100])
        )

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
        "batchAliasCount": 0,
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
