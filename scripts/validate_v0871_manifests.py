#!/usr/bin/env python3
"""Validate exact v0.87.1 changed-path/test inventories and 150-row crosswalk."""
from __future__ import annotations

import argparse
import hashlib
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CROSSWALK = ROOT / "docs/v0871-150-test-crosswalk.md"
CHANGED_PATHS = ROOT / "docs/manifests/v0871-changed-paths-16.txt"
CHANGED_TESTS = ROOT / "docs/manifests/v0871-changed-tests-9.txt"
CHANGED_BASENAMES = ROOT / "docs/manifests/v0871-changed-tests-basename-9.txt"
CORPUS_BASENAMES = ROOT / "docs/manifests/v0871-test-corpus-basename-150.txt"
EXPECTED = {
    CHANGED_PATHS: ("2756fce613d0163b6eb5c47b599584589a229e5c7ed30a86b65380031e272eb6", 16),
    CHANGED_TESTS: ("5b66a8cf9050b36a8dbae7a1b802c12037a2ec2332cf2e3f370953ea9ef9ac43", 9),
    CHANGED_BASENAMES: ("e06264a330f331e86f75a5b1b0abc3f30dfa130f5299d6de019ab39a43a8258c", 9),
    CORPUS_BASENAMES: ("042cdfbc8cc089da71409e615fb54cfe7273a960f8e0d10e63e07584ae9f2e75", 150),
}
CHANGED_SOURCE_PATHS = {
    "packages/ai/src/api/anthropic-messages.ts",
    "packages/ai/src/api/openai-completions.ts",
    "packages/ai/src/image-models.generated.ts",
}


def read_exact(path: Path, fault: str) -> tuple[list[str], str]:
    raw = path.read_bytes()
    if fault == path.stem:
        raw += b"# deliberate corruption\n"
    digest = hashlib.sha256(raw).hexdigest()
    expected_hash, expected_count = EXPECTED[path]
    if digest != expected_hash:
        raise ValueError(f"{path.name} sha256 mismatch: got {digest}, expected {expected_hash}")
    rows = raw.decode().splitlines()
    if len(rows) != expected_count:
        raise ValueError(f"{path.name} row count mismatch: got {len(rows)}, expected {expected_count}")
    if len(set(rows)) != len(rows):
        raise ValueError(f"{path.name} contains duplicate rows")
    return rows, digest


def matrix_rows(heading: str) -> dict[str, str]:
    rows: dict[str, str] = {}
    in_matrix = False
    for line in CROSSWALK.read_text().splitlines():
        if line.startswith(heading):
            in_matrix = True
            continue
        if in_matrix and line.startswith("## "):
            break
        if not in_matrix or not line.startswith("| `"):
            continue
        cols = [col.strip() for col in line.strip().strip("|").split("|")]
        match = re.fullmatch(r"`([^`]+)`", cols[0]) if cols else None
        if match:
            name = match.group(1)
            if name in rows:
                raise ValueError(f"duplicate matrix row: {name}")
            rows[name] = line
    return rows


def validate(fault: str = "") -> str:
    changed_paths, paths_hash = read_exact(CHANGED_PATHS, fault)
    changed_tests, tests_hash = read_exact(CHANGED_TESTS, fault)
    changed_basenames, basenames_hash = read_exact(CHANGED_BASENAMES, fault)
    corpus, corpus_hash = read_exact(CORPUS_BASENAMES, fault)

    if any(not row.startswith("packages/ai/") for row in changed_paths):
        raise ValueError("changed-path inventory contains a non-packages/ai path")
    if any(not row.startswith("packages/ai/test/") or not row.endswith(".test.ts") for row in changed_tests):
        raise ValueError("changed-test inventory contains an invalid path")
    if set(changed_tests) - set(changed_paths):
        raise ValueError("changed tests are not a subset of changed paths")
    derived_basenames = {row.rsplit("/", 1)[1] for row in changed_tests}
    if derived_basenames != set(changed_basenames):
        raise ValueError("changed-test full paths and basename inventory differ")
    if not CHANGED_SOURCE_PATHS.issubset(set(changed_paths)):
        raise ValueError("required v0.87.1 source paths are absent")

    path_rows = matrix_rows("## Changed-path disposition matrix")
    if set(path_rows) != set(changed_paths):
        missing = sorted(set(changed_paths) - set(path_rows))
        extra = sorted(set(path_rows) - set(changed_paths))
        raise ValueError(f"v0.87.1 path matrix mismatch: rows={len(path_rows)} missing={missing} extra={extra}")
    invalid_paths = [
        name for name, line in path_rows.items()
        if not any(token in line for token in ("| ADAPTED |", "| COVERED |", "| LIVE UNEXECUTED |", "| N/A |"))
    ]
    if invalid_paths:
        raise ValueError(f"changed path rows lack dispositions: {invalid_paths}")

    rows = matrix_rows("## Per-file 150-test disposition matrix")
    if set(rows) != set(corpus):
        missing = sorted(set(corpus) - set(rows))
        extra = sorted(set(rows) - set(corpus))
        raise ValueError(f"v0.87.1 crosswalk mismatch: rows={len(rows)} missing={missing} extra={extra}")
    changed = set(changed_basenames)
    invalid_changed = [
        name for name in sorted(changed)
        if "v0.87.1 tag" not in rows[name]
        or not any(token in rows[name] for token in ("| ADAPTED |", "| LIVE UNEXECUTED |", "| N/A |"))
    ]
    if invalid_changed:
        raise ValueError(f"changed crosswalk rows lack bounded dispositions: {invalid_changed}")
    stale_pending = [name for name, line in rows.items() if "PENDING" in line]
    if stale_pending:
        raise ValueError(f"crosswalk retains pending rows: {stale_pending}")

    return (
        "v0.87.1 manifests verified: changedPaths=16 changedTests=9 testRows=150 "
        f"changedPathsSha256={paths_hash} changedTestsSha256={tests_hash} "
        f"changedBasenamesSha256={basenames_hash} corpusBasenamesSha256={corpus_hash}"
    )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--fault",
        choices=["", "v0871-changed-paths-16", "v0871-changed-tests-9", "v0871-changed-tests-basename-9", "v0871-test-corpus-basename-150"],
        default="",
    )
    args = parser.parse_args()
    try:
        print(validate(args.fault))
        return 0
    except ValueError as exc:
        print(str(exc), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
