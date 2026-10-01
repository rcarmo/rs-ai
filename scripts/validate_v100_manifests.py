#!/usr/bin/env python3
"""Validate exact v1.0.0 path/test inventories and 171-row crosswalk."""
from __future__ import annotations

import argparse
import hashlib
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CROSSWALK = ROOT / "docs/v100-171-test-crosswalk.md"
CHANGED_PATHS = ROOT / "docs/manifests/v100-changed-paths-8.txt"
CHANGED_TESTS = ROOT / "docs/manifests/v100-changed-tests-3.txt"
CHANGED_BASENAMES = ROOT / "docs/manifests/v100-changed-tests-basename-3.txt"
CORPUS_PATHS = ROOT / "docs/manifests/v100-test-corpus-171.txt"
CORPUS_BASENAMES = ROOT / "docs/manifests/v100-test-corpus-basename-171.txt"
EXPECTED = {
    CHANGED_PATHS: ("b8db49581470036b68078ac093dc6b41eaa92222647b14cf44a92b870d54eab4", 8),
    CHANGED_TESTS: ("fbe3c63453261a58352b5e238f5f9488f33a13016485c7e3a49cce17bfdaaca2", 3),
    CHANGED_BASENAMES: ("61baa1f1d3e295e9331a4004010186b59ca4ea40530384f60afd38453139c86b", 3),
    CORPUS_PATHS: ("9d24da3ede393a95a7131b1c9ac494f57d8165161d6eb581109c86809131abfc", 171),
    CORPUS_BASENAMES: ("b772c48553275619413b19d1006bfa4759a3ce8e77abd68a8c3838be4fbb0478", 171),
}
EXPECTED_SHORTSTAT = (8, 192, 14)
EXPECTED_ORACLE = {
    "schema": 6,
    "structureHash": "235f2f320916ab6b0d7193e0bf66ec7983e9bc05abeddd7264923fb1e7eaf76e",
    "chat": (1532, 41, 10),
    "image": (57, 1, 1),
    "classifier": (15, 5, 2),
    "total": 1604,
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
        "**3 test paths**",
        "**171 unique basenames**",
        "f39b99c29b8598f175b10840e5d2a81983e7c0ce5cae4d7df83a1007447d2c2b",
        "Responses replay resolves grammar capability and the transcript tool name once",
        "Foreign function history retains the accepted `fc_<shortHash>` normalisation",
        "Missing or null grammar arguments replay as an empty input string",
        "Signed schema-v6 catalogs are regenerated only from the exact npm tarball shards",
    )
    missing_facts = [fact for fact in required_facts if fact not in text]
    if missing_facts:
        raise ValueError(f"v1.0.0 crosswalk lacks pinned release facts: {missing_facts}")

    path_rows = matrix_rows("## Changed-path disposition matrix")
    if set(path_rows) != set(parsed_changed_paths):
        raise ValueError(
            f"v1.0.0 path matrix mismatch: rows={len(path_rows)} "
            f"missing={sorted(set(parsed_changed_paths)-set(path_rows))} extra={sorted(set(path_rows)-set(parsed_changed_paths))}"
        )
    test_rows = matrix_rows("## Per-file 171-test/support disposition matrix")
    if set(test_rows) != set(corpus_basenames):
        raise ValueError(
            f"v1.0.0 corpus matrix mismatch: rows={len(test_rows)} "
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
            f"v1.0.0 crosswalk incomplete: pendingPaths={len(pending_paths)} pendingTests={len(pending_tests)}"
        )

    if not pending_paths:
        bad_paths = [n for n in parsed_changed_paths if not any(t in path_rows[n] for t in ("| ADAPTED |", "| N/A |", "| LIVE UNEXECUTED |"))]
        if bad_paths:
            raise ValueError(f"completed changed paths lack bounded dispositions: {bad_paths}")
    if not pending_tests:
        bad_tests = [n for n in changed_basenames if "v1.0.0 tag" not in test_rows[n] or not any(t in test_rows[n] for t in ("| ADAPTED |", "| N/A |", "| LIVE UNEXECUTED |"))]
        if bad_tests:
            raise ValueError(f"completed changed tests lack bounded evidence: {bad_tests}")

    return (
        "v1.0.0 manifests verified: changedPaths=8 shortstat=+192/-14 changedTests=3 "
        "corpusRows=171 schema=6 records=1604 "
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
