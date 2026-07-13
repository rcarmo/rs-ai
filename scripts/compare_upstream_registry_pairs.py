#!/usr/bin/env python3
"""Mechanically compare rs-ai registry provider/id pairs with an upstream pi-ai worktree.

This intentionally imports/flattens upstream TypeScript exports with Bun instead of
regex-counting provider files. It checks both text MODELS and image IMAGE_MODELS.

Usage:
  python3 scripts/compare_upstream_registry_pairs.py /workspace/tmp/pi-src 0e6909f050eeb15e8f6c05185511f3788357ddb3

The second argument is optional but recommended; when provided, the script verifies
that the upstream worktree's HEAD is exactly that commit.
"""

from __future__ import annotations

import json
import re
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def run(cmd: list[str], cwd: Path | None = None) -> str:
    return subprocess.check_output(cmd, cwd=cwd, text=True)


def upstream_pairs(upstream: Path, module_rel: str, export_name: str) -> set[tuple[str, str]]:
    module = (upstream / module_rel).resolve()
    if not module.exists():
        raise SystemExit(f"missing upstream module: {module}")
    script = """
import { __EXPORT__ } from __MODULE__;
function flattenModels(value, out = []) {
  if (!value) return out;
  if (Array.isArray(value)) {
    for (const item of value) flattenModels(item, out);
    return out;
  }
  if (typeof value === "object") {
    if (typeof value.provider === "string" && typeof value.id === "string") {
      out.push(value);
      return out;
    }
    for (const item of Object.values(value)) flattenModels(item, out);
  }
  return out;
}
const values = flattenModels(__EXPORT__);
const pairs = values.map((m) => [m.provider, m.id]).sort((a, b) =>
  a[0].localeCompare(b[0]) || a[1].localeCompare(b[1])
);
process.stdout.write(JSON.stringify(pairs));
""".replace("__EXPORT__", export_name).replace("__MODULE__", json.dumps(module.as_uri()))
    with tempfile.NamedTemporaryFile("w", suffix=".mjs", delete=False) as f:
        f.write(script)
        script_path = Path(f.name)
    try:
        out = run(["bun", str(script_path)], cwd=upstream)
    finally:
        script_path.unlink(missing_ok=True)
    return {tuple(pair) for pair in json.loads(out)}


def rust_pairs(path: Path) -> set[tuple[str, str]]:
    text = path.read_text()
    pattern = re.compile(
        r'id:\s*"([^"]+)"\.into\(\),\s*\n'
        r'\s*name:[\s\S]*?\n'
        r'\s*api:[\s\S]*?\n'
        r'\s*provider:\s*"([^"]+)"\.into\(\)',
        re.MULTILINE,
    )
    return {(provider, model_id) for model_id, provider in pattern.findall(text)}


def report(label: str, upstream_set: set[tuple[str, str]], local_set: set[tuple[str, str]]) -> bool:
    missing = sorted(upstream_set - local_set)
    extra = sorted(local_set - upstream_set)
    print(
        f"{label}: upstream={len(upstream_set)} local={len(local_set)} "
        f"missing={len(missing)} extra={len(extra)}"
    )
    if missing:
        print("missing:")
        for provider, model_id in missing:
            print(f"  {provider}/{model_id}")
    if extra:
        print("extra:")
        for provider, model_id in extra:
            print(f"  {provider}/{model_id}")
    return not missing and not extra


def main() -> int:
    if len(sys.argv) < 2 or len(sys.argv) > 3:
        print(__doc__.strip(), file=sys.stderr)
        return 2
    upstream = Path(sys.argv[1]).resolve()
    expected_ref = sys.argv[2] if len(sys.argv) == 3 else None
    if expected_ref:
        head = run(["git", "rev-parse", "HEAD"], cwd=upstream).strip()
        expected = run(["git", "rev-parse", expected_ref], cwd=upstream).strip()
        if head != expected:
            print(f"upstream HEAD mismatch: got {head}, expected {expected}", file=sys.stderr)
            return 2

    text_up = upstream_pairs(upstream, "packages/ai/src/models.generated.ts", "MODELS")
    image_up = upstream_pairs(upstream, "packages/ai/src/image-models.generated.ts", "IMAGE_MODELS")
    text_local = rust_pairs(ROOT / "src/models_generated.rs")
    image_local = rust_pairs(ROOT / "src/images/models_generated.rs")

    ok = True
    ok &= report("text", text_up, text_local)
    ok &= report("image", image_up, image_local)
    return 0 if ok else 1


if __name__ == "__main__":
    raise SystemExit(main())
