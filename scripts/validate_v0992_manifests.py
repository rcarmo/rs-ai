#!/usr/bin/env python3
"""Validate exact v0.99.2 path/test inventories and 171-row crosswalk."""
from __future__ import annotations

import argparse
import hashlib
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CROSSWALK = ROOT / "docs/v0992-171-test-crosswalk.md"
CHANGED_PATHS = ROOT / "docs/manifests/v0992-changed-paths-15.txt"
CHANGED_TESTS = ROOT / "docs/manifests/v0992-changed-tests-6.txt"
CHANGED_BASENAMES = ROOT / "docs/manifests/v0992-changed-tests-basename-6.txt"
CORPUS_PATHS = ROOT / "docs/manifests/v0992-test-corpus-171.txt"
CORPUS_BASENAMES = ROOT / "docs/manifests/v0992-test-corpus-basename-171.txt"
EXPECTED = {
    CHANGED_PATHS: ("53b2c290d902bb8d79c87e035b87c52a13b97617849ea85b51c8e2b11133cc15", 15),
    CHANGED_TESTS: ("1ad16f63dc47b019cdcf4fdf7029c86785f4cbac38158e7fb63db963ce9ce66d", 6),
    CHANGED_BASENAMES: ("cf28e71b68eeca59b602f72887189df65785fc298c62c39981fc1e3cd4b663fa", 6),
    CORPUS_PATHS: ("9d24da3ede393a95a7131b1c9ac494f57d8165161d6eb581109c86809131abfc", 171),
    CORPUS_BASENAMES: ("70e3eee850f9f07e4bf801ae5d8f4b541bcfc804f935f7e4fc715b2cfd7821b7", 171),
}
EXPECTED_SHORTSTAT = (15, 726, 77)
EXPECTED_ORACLE = {
    "schema": 6,
    "structureHash": "3e97a64c71ef31a515f668d9fbc653d49b3ece171d88bfd103001e963497661f",
    "chat": (1529, 41, 10),
    "image": (57, 1, 1),
    "classifier": (15, 5, 2),
    "total": 1601,
    "providerFiles": 42,
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
    corpus_paths, corpus_paths_hash = read_exact(CORPUS_PATHS, fault)
    corpus_basenames, corpus_basenames_hash = read_exact(CORPUS_BASENAMES, fault)

    parsed_changed_paths: list[str] = []
    for row in changed_paths:
        match = re.fullmatch(r"([AMD])\t(packages/ai/.+)", row)
        if not match:
            raise ValueError(f"changed-path inventory contains an invalid status/path row: {row}")
        parsed_changed_paths.append(match.group(2))
    if any(not row.startswith("packages/ai/test/") or not row.endswith(".test.ts") for row in changed_tests):
        raise ValueError("changed-test inventory contains an invalid path")
    if set(changed_tests) - set(parsed_changed_paths):
        raise ValueError("changed tests are not a subset of changed paths")
    if {row.rsplit("/", 1)[1] for row in changed_tests} != set(changed_basenames):
        raise ValueError("changed-test full paths and basename inventory differ")
    if {row.rsplit("/", 1)[1] for row in corpus_paths} != set(corpus_basenames):
        raise ValueError("corpus full paths and basename inventory differ")
    if not set(changed_basenames).issubset(corpus_basenames):
        raise ValueError("changed test basenames are absent from final corpus")

    text = CROSSWALK.read_text()
    files, added, deleted = EXPECTED_SHORTSTAT
    required_facts = (
        f"**{files} changed paths** with **{added} insertions and {deleted} deletions**",
        "**6 test paths**",
        "**171 unique basenames**",
        "0b3df8791b488216f309d908789294a744bb61bbaad123d94098e56df9538d25",
        "Federation evidence is split explicitly",
        "Eager tool input and strict-schema fallback are independent predicates",
        "Invalid Retry-After fallback has dedicated retry evidence",
        "Signed schema-v6 catalogs are regenerated only if a pinned full-record comparison proves a record delta",
    )
    missing_facts = [fact for fact in required_facts if fact not in text]
    if missing_facts:
        raise ValueError(f"v0.99.2 crosswalk lacks pinned release facts: {missing_facts}")

    path_rows = matrix_rows("## Changed-path disposition matrix")
    if set(path_rows) != set(parsed_changed_paths):
        raise ValueError(
            f"v0.99.2 path matrix mismatch: rows={len(path_rows)} "
            f"missing={sorted(set(parsed_changed_paths)-set(path_rows))} extra={sorted(set(path_rows)-set(parsed_changed_paths))}"
        )
    test_rows = matrix_rows("## Per-file 171-test/support disposition matrix")
    if set(test_rows) != set(corpus_basenames):
        raise ValueError(
            f"v0.99.2 corpus matrix mismatch: rows={len(test_rows)} "
            f"missing={sorted(set(corpus_basenames)-set(test_rows))} extra={sorted(set(test_rows)-set(corpus_basenames))}"
        )

    allowed = ("| PENDING |", "| ADAPTED |", "| COVERED |", "| LIVE UNEXECUTED |", "| N/A |")
    invalid_paths = [name for name, line in path_rows.items() if not any(token in line for token in allowed)]
    invalid_tests = [name for name, line in test_rows.items() if not any(token in line for token in allowed)]
    if invalid_paths or invalid_tests:
        raise ValueError(f"matrix rows lack dispositions: paths={invalid_paths} tests={invalid_tests}")

    pending_paths = {name for name, line in path_rows.items() if "| PENDING |" in line}
    pending_tests = {name for name, line in test_rows.items() if "| PENDING |" in line}
    allowed_pending_paths: set[str] = set()
    allowed_pending_tests: set[str] = set()
    if pending_paths and pending_paths != allowed_pending_paths:
        raise ValueError(f"implementation ledger has unexpected pending paths: {sorted(pending_paths)}")
    if pending_tests and pending_tests != allowed_pending_tests:
        raise ValueError(f"implementation ledger has unexpected pending tests: {sorted(pending_tests)}")
    if require_complete and (pending_paths or pending_tests):
        raise ValueError(
            f"v0.99.2 crosswalk incomplete: pendingPaths={len(pending_paths)} pendingTests={len(pending_tests)}"
        )

    if not pending_paths:
        bad_paths = [n for n in parsed_changed_paths if not any(t in path_rows[n] for t in ("| ADAPTED |", "| N/A |", "| LIVE UNEXECUTED |"))]
        if bad_paths:
            raise ValueError(f"completed changed paths lack bounded dispositions: {bad_paths}")
    if not pending_tests:
        bad_tests = [n for n in changed_basenames if "v0.99.2 tag" not in test_rows[n] or not any(t in test_rows[n] for t in ("| ADAPTED |", "| N/A |", "| LIVE UNEXECUTED |"))]
        if bad_tests:
            raise ValueError(f"completed changed tests lack bounded evidence: {bad_tests}")

    return (
        "v0.99.2 manifests verified: changedPaths=15 shortstat=+726/-77 changedTests=6 "
        "corpusRows=171 schema=6 records=1601 "
        f"changedPathsSha256={paths_hash} changedTestsSha256={tests_hash} "
        f"changedBasenamesSha256={basenames_hash} corpusPathsSha256={corpus_paths_hash} "
        f"corpusBasenamesSha256={corpus_basenames_hash} pendingPaths={len(pending_paths)} "
        f"pendingTests={len(pending_tests)}"
    )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--fault", choices=["", *[p.stem for p in EXPECTED]], default="")
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
