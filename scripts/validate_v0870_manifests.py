#!/usr/bin/env python3
"""Validate exact v0.87.0 changed-path/test inventories and 150-row crosswalk."""
from __future__ import annotations

import argparse
import hashlib
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CROSSWALK = ROOT / "docs/v0870-150-test-crosswalk.md"
CHANGED_PATHS = ROOT / "docs/manifests/v0870-changed-paths-127.txt"
CHANGED_TESTS = ROOT / "docs/manifests/v0870-changed-tests-82.txt"
CHANGED_BASENAMES = ROOT / "docs/manifests/v0870-changed-tests-basename-82.txt"
CORPUS_BASENAMES = ROOT / "docs/manifests/v0870-test-corpus-basename-150.txt"
EXPECTED = {
    CHANGED_PATHS: ("e6bd9733d8fff626838d386df8e6bb543d40d77af74340ee4f2f271b411b9828", 127),
    CHANGED_TESTS: ("a12a1453c8fbabfd6902ced06304cbd2fa9f7d82ce89b91a1403366bc11fe5b2", 82),
    CHANGED_BASENAMES: ("cb66d8f12cc4e23509e41e07c5bdddf546d961e731751254f01fbf0989676040", 82),
    CORPUS_BASENAMES: ("042cdfbc8cc089da71409e615fb54cfe7273a960f8e0d10e63e07584ae9f2e75", 150),
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


def crosswalk_rows() -> dict[str, str]:
    rows: dict[str, str] = {}
    in_matrix = False
    for line in CROSSWALK.read_text().splitlines():
        if line.startswith("## Per-file 150-test disposition matrix"):
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
                raise ValueError(f"duplicate crosswalk row: {name}")
            rows[name] = line
    return rows


def validate(fault: str = "") -> str:
    changed_paths, paths_hash = read_exact(CHANGED_PATHS, fault)
    changed_tests, tests_hash = read_exact(CHANGED_TESTS, fault)
    changed_basenames, basenames_hash = read_exact(CHANGED_BASENAMES, fault)
    corpus, corpus_hash = read_exact(CORPUS_BASENAMES, fault)

    if any(not row.startswith("packages/ai/") for row in changed_paths):
        raise ValueError("changed-path inventory contains a non-packages/ai path")
    if any(not row.startswith("packages/ai/test/") or not row.endswith(".ts") for row in changed_tests):
        raise ValueError("changed-test inventory contains an invalid path")
    derived_basenames = {row.rsplit("/", 1)[1] for row in changed_tests}
    if derived_basenames != set(changed_basenames):
        raise ValueError("changed-test full paths and basename inventory differ")
    changed_test_files = {name for name in changed_basenames if name.endswith(".test.ts")}
    changed_helpers = set(changed_basenames) - changed_test_files
    if changed_helpers != {"codex-websocket-cached-probe.ts"}:
        raise ValueError(f"unexpected changed test helpers: {sorted(changed_helpers)}")
    deleted_tests = changed_test_files - set(corpus)
    if deleted_tests != {"deferred-tools.test.ts"}:
        raise ValueError(f"unexpected changed tests absent from final corpus: {sorted(deleted_tests)}")
    live_changed_tests = changed_test_files - deleted_tests

    rows = crosswalk_rows()
    if set(rows) != set(corpus):
        missing = sorted(set(corpus) - set(rows))
        extra = sorted(set(rows) - set(corpus))
        raise ValueError(f"v0.87.0 crosswalk mismatch: rows={len(rows)} missing={missing} extra={extra}")
    allowed = ("| PENDING", "| ADAPTED", "| COVERED", "| LIVE UNEXECUTED", "| N/A")
    invalid = [name for name, line in rows.items() if all(token not in line for token in allowed)]
    if invalid:
        raise ValueError(f"crosswalk rows without a valid disposition: {invalid}")
    missing_changed_marker = [name for name in live_changed_tests if "v0.85.1→v0.87.0" not in rows[name] and "| PENDING" in rows[name]]
    if missing_changed_marker:
        raise ValueError(f"pending changed rows without bounded-range note: {missing_changed_marker}")

    return (
        "v0.87.0 manifests verified: changedPaths=127 changedTests=82 testRows=150 "
        f"changedPathsSha256={paths_hash} changedTestsSha256={tests_hash} "
        f"changedBasenamesSha256={basenames_hash} corpusBasenamesSha256={corpus_hash}"
    )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--fault",
        choices=["", "v0870-changed-paths-127", "v0870-changed-tests-82", "v0870-changed-tests-basename-82", "v0870-test-corpus-basename-150"],
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
