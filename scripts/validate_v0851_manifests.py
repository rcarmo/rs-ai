#!/usr/bin/env python3
"""Validate v0.85.1 release audit manifests.

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
CROSSWALK = ROOT / "docs/v0851-142-test-crosswalk.md"
CHANGED_PATHS_MANIFEST = ROOT / "docs/manifests/v0851-changed-paths.txt"
TEST_CORPUS_MANIFEST = ROOT / "docs/manifests/v0851-test-corpus-142.txt"
EXPECTED_CHANGED_SHA256 = "ee26f669d92dc77b265731165a2ff69ccb67defba92517cbbd5f97a186e187d2"
EXPECTED_TEST_CORPUS_SHA256 = "56f8742065a4ad01d73e5aee53035324f2e7333a735222ab15db870819e29065"
EXPECTED_CHANGED_PATHS = [
    "packages/ai/CHANGELOG.md",
    "packages/ai/package.json",
    "packages/ai/scripts/generate-models.ts",
    "packages/ai/src/api/openai-responses.ts",
    "packages/ai/src/image-models.generated.ts",
    "packages/ai/src/types.ts",
    "packages/ai/test/cache-retention.test.ts",
    "packages/ai/test/max-thinking.test.ts",
    "packages/ai/test/supports-xhigh.test.ts",
]
EXPECTED_CHANGED_TESTS = {
    "cache-retention.test.ts",
    "max-thinking.test.ts",
    "supports-xhigh.test.ts",
}


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
        if "\t" in line:
            cols = line.split("\t", 1)
            if len(cols) != 2 or cols[0] not in {"A", "M", "D"}:
                raise ValueError(f"invalid changed-path inventory row: {line!r}")
            path = cols[1]
        else:
            path = line
        if not path.startswith("packages/ai/"):
            raise ValueError(f"invalid changed-path inventory row: {line!r}")
        paths.add(path)
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
    start = text.index("## Current audit target: v0.85.1")
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
        9,
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
            failures.append(f"v0.85.1 changed-path matrix mismatch in {label}\nmissing={missing}\nextra={extra}")
    rows = crosswalk_rows()
    got_manifest_tests = manifest_test_files(test_lines)
    for label, got_tests in [("inventory", got_manifest_tests), ("crosswalk", set(rows.keys()))]:
        missing_tests = sorted(got_manifest_tests - got_tests) if label == "crosswalk" else []
        extra_tests = sorted(got_tests - got_manifest_tests)
        if len(got_tests) != 142 or missing_tests or extra_tests:
            failures.append(f"v0.85.1 142-test corpus mismatch in {label}\nrows={len(got_tests)} missing={missing_tests} extra={extra_tests}")
    for changed in EXPECTED_CHANGED_TESTS:
        row = rows.get(changed, "")
        if "| ADAPTED" not in row:
            failures.append(f"changed test row is not marked ADAPTED: {changed}")
    invalid = [name for name, line in rows.items() if all(token not in line for token in ["| ADAPTED", "| COVERED", "| LIVE UNEXECUTED", "| N/A"])]
    if invalid:
        failures.append(f"crosswalk rows without disposition: {invalid}")
    if failures:
        raise ValueError("\n".join(failures))
    return (
        "v0.85.1 manifests verified: changedPaths=9 testRows=142 "
        f"changedSha256={changed_sha} testCorpusSha256={corpus_sha}"
    )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--fault", choices=["", "v0851-changed-paths", "v0851-test-corpus-142"], default="")
    args = parser.parse_args()
    try:
        print(validate(args.fault))
        return 0
    except ValueError as exc:
        print(str(exc), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
