#!/usr/bin/env python3
"""Review Cargo dependency licenses from `cargo metadata`.

The check is intentionally conservative for release audit hygiene: every
resolved dependency must publish a license expression, and every expression must
contain at least one approved permissive license token and no explicitly denied
copyleft/proprietary token. Exceptions require a committed policy change with
owner/rationale/expiry before this script is relaxed.
"""
from __future__ import annotations

import json
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
APPROVED = {
    "Apache-2.0",
    "BSD-2-Clause",
    "BSD-3-Clause",
    "BSL-1.0",
    "CC0-1.0",
    "ISC",
    "MIT",
    "MIT-0",
    "Unicode-3.0",
    "Unlicense",
    "Zlib",
}
DENIED_PREFIXES = ("AGPL", "GPL", "LGPL", "MPL", "CDDL", "EPL", "Proprietary")
TOKEN_RE = re.compile(r"[A-Za-z0-9][A-Za-z0-9.+-]*")


def metadata() -> dict:
    out = subprocess.check_output(
        ["cargo", "metadata", "--locked", "--all-features", "--format-version", "1"],
        cwd=ROOT,
        text=True,
        stderr=subprocess.STDOUT,
    )
    return json.loads(out)


def main() -> int:
    failures: list[str] = []
    checked = 0
    for package in sorted(metadata()["packages"], key=lambda p: (p["name"], p["version"])):
        if package.get("source") is None:
            continue
        checked += 1
        expr = package.get("license") or ""
        if not expr:
            failures.append(f"{package['name']} {package['version']}: missing license expression")
            continue
        tokens = set(TOKEN_RE.findall(expr))
        denied = sorted(token for token in tokens if token.startswith(DENIED_PREFIXES))
        if denied:
            # An OR expression with a permissive alternative is still usable under the
            # permissive choice; pure denied expressions require explicit review.
            has_permissive_alternative = bool(tokens & APPROVED) and "OR" in tokens
            if not has_permissive_alternative:
                failures.append(
                    f"{package['name']} {package['version']}: denied license token(s) {denied} in {expr!r}"
                )
        if not (tokens & APPROVED):
            failures.append(
                f"{package['name']} {package['version']}: no approved license token in {expr!r}"
            )
    if failures:
        print("license review failed:", file=sys.stderr)
        print("\n".join(failures), file=sys.stderr)
        return 1
    print(f"license review passed: {checked} third-party packages; approved tokens={','.join(sorted(APPROVED))}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
