#!/usr/bin/env python3
"""Validate exact v0.99.1 path/test inventories and 160-row crosswalk."""
from __future__ import annotations

import argparse
import hashlib
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CROSSWALK = ROOT / "docs/v0991-160-test-crosswalk.md"
CHANGED_PATHS = ROOT / "docs/manifests/v0991-changed-paths-169.txt"
CHANGED_TESTS = ROOT / "docs/manifests/v0991-changed-tests-58.txt"
CHANGED_BASENAMES = ROOT / "docs/manifests/v0991-changed-tests-basename-58.txt"
CORPUS_BASENAMES = ROOT / "docs/manifests/v0991-test-corpus-basename-160.txt"
EXPECTED = {
    CHANGED_PATHS: ("086beb5b751f144f9e45034bfbb2b0f4e8a0d3a00f9e17ec1b073b3a8397221e", 169),
    CHANGED_TESTS: ("d8eefa94ed87c03de0545965351de4d800cf76ce1db33b0f62fab0f9ab3c7acc", 58),
    CHANGED_BASENAMES: ("420e7febc74c03318675c308342172499e8c7e81971aba7554eb93a91e875df6", 58),
    CORPUS_BASENAMES: ("7ad5f140edc5bc49a348b7b7e36ea266dd82a3075c23ad9997b6e8bc21992b06", 160),
}
EXPECTED_SHORTSTAT = (169, 7001, 2913)
EXPECTED_ORACLE = {
    "schema": 6,
    "structureHash": "58511a57fb2db5e984ee62857d8079aec6ff800e19226c327c118e7f57ea916b",
    "chat": (1523, 41, 10),
    "image": (57, 1, 1),
    "classifier": (12, 5, 2),
    "total": 1592,
    "providerFiles": 42,
    "chatDelta": (59, 31, 110),
    "imageDelta": (2, 0, 54),
    "classifierDelta": (12, 0, 0),
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
    active = False
    for line in CROSSWALK.read_text().splitlines():
        if line == heading:
            active = True
            continue
        if active and line.startswith("## "):
            break
        if not active or not line.startswith("| `"):
            continue
        cols = [col.strip() for col in line.strip().strip("|").split("|")]
        match = re.fullmatch(r"`([^`]+)`", cols[0]) if cols else None
        if match:
            name = match.group(1)
            if name in rows:
                raise ValueError(f"duplicate matrix row: {name}")
            rows[name] = line
    return rows


def validate(fault: str = "", require_complete: bool = False) -> str:
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
    if not set(changed_basenames).issubset(corpus):
        raise ValueError("changed test basenames are absent from final corpus")

    text = CROSSWALK.read_text()
    files, added, deleted = EXPECTED_SHORTSTAT
    required_facts = (
        f"**{files} changed paths** with **{added} insertions and {deleted} deletions**",
        f"structure hash `{EXPECTED_ORACLE['structureHash']}`",
        "chat `1523/41/10`",
        "image `57/1/1`",
        "classifier `12/5/2`",
        "total 1592 records across 42 provider files",
        "chat `+59/-31/110 changed`",
        "image `+2/-0/54 changed`",
        "classifier `+12/-0/0 changed`",
        "adds `type: \"image\"` only to legacy records",
        "Live hydration is outside this audit.",
    )
    missing_facts = [fact for fact in required_facts if fact not in text]
    if missing_facts:
        raise ValueError(f"v0.99.1 crosswalk lacks pinned release facts: {missing_facts}")

    path_rows = matrix_rows("## Changed-path disposition matrix")
    if set(path_rows) != set(changed_paths):
        missing = sorted(set(changed_paths) - set(path_rows))
        extra = sorted(set(path_rows) - set(changed_paths))
        raise ValueError(f"v0.99.1 path matrix mismatch: rows={len(path_rows)} missing={missing} extra={extra}")

    test_rows = matrix_rows("## Per-file 160-test disposition matrix")
    if set(test_rows) != set(corpus):
        missing = sorted(set(corpus) - set(test_rows))
        extra = sorted(set(test_rows) - set(corpus))
        raise ValueError(f"v0.99.1 test matrix mismatch: rows={len(test_rows)} missing={missing} extra={extra}")

    allowed = ("| PENDING |", "| ADAPTED |", "| COVERED |", "| LIVE UNEXECUTED |", "| N/A |")
    invalid_paths = [name for name, line in path_rows.items() if not any(token in line for token in allowed)]
    invalid_tests = [name for name, line in test_rows.items() if not any(token in line for token in allowed)]
    if invalid_paths or invalid_tests:
        raise ValueError(f"matrix rows lack dispositions: paths={invalid_paths} tests={invalid_tests}")

    pending_paths = {name for name, line in path_rows.items() if "| PENDING |" in line}
    pending_tests = {name for name, line in test_rows.items() if "| PENDING |" in line}
    if not require_complete:
        # The initial ledger is deliberately all-pending; the accepted ledger is
        # complete. Reject partial hand-edited states in ordinary validation too.
        if pending_paths and pending_paths != set(changed_paths):
            raise ValueError("implementation ledger has a partial pending path set")
        if pending_tests and pending_tests != set(changed_basenames):
            raise ValueError("implementation ledger has a partial pending test set")
    elif pending_paths or pending_tests:
        raise ValueError(
            f"v0.99.1 crosswalk incomplete: pendingPaths={len(pending_paths)} pendingTests={len(pending_tests)}"
        )

    if not pending_paths:
        invalid_completed_paths = [
            name
            for name in changed_paths
            if not any(
                token in path_rows[name]
                for token in ("| ADAPTED |", "| LIVE UNEXECUTED |", "| N/A |")
            )
        ]
        if invalid_completed_paths:
            raise ValueError(
                f"completed changed paths lack bounded dispositions: {invalid_completed_paths}"
            )
    if not pending_tests:
        invalid_completed_tests = [
            name
            for name in changed_basenames
            if "v0.99.1 tag" not in test_rows[name]
            or not any(
                token in test_rows[name]
                for token in ("| ADAPTED |", "| LIVE UNEXECUTED |", "| N/A |")
            )
        ]
        if invalid_completed_tests:
            raise ValueError(
                f"completed changed tests lack bounded evidence: {invalid_completed_tests}"
            )

    return (
        "v0.99.1 manifests verified: changedPaths=169 shortstat=+7001/-2913 "
        "changedTests=58 testRows=160 schema=6 records=1592 "
        f"changedPathsSha256={paths_hash} changedTestsSha256={tests_hash} "
        f"changedBasenamesSha256={basenames_hash} corpusBasenamesSha256={corpus_hash} "
        f"pendingPaths={len(pending_paths)} pendingTests={len(pending_tests)}"
    )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--fault",
        choices=["", "v0991-changed-paths-169", "v0991-changed-tests-58", "v0991-changed-tests-basename-58", "v0991-test-corpus-basename-160"],
        default="",
    )
    parser.add_argument("--require-complete", action="store_true")
    args = parser.parse_args()
    try:
        print(validate(args.fault, args.require_complete))
        return 0
    except ValueError as exc:
        print(str(exc), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
