#!/usr/bin/env python3
"""Validate official v1.1.0 inventories; fail closed on incomplete acceptance."""
from __future__ import annotations

import argparse
import hashlib
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
TAG_SHA = "abe508e1b89912adde45528136c3221eb69acdd7"
BASE_SHA = "a7229ddc21810d6245105978033b7df645ecc2f7"
SPECS = {
    "ai": {
        "prefix": "v110", "crosswalk": "v110-upgrade-crosswalk.md",
        "counts": (82, 197, 174), "tests": 167,
        "hashes": (
            "f0bedbb09beb53ad80c7beea7748f0ac8ca13059b3aabaa91dd37d71f4b6e2c7",
            "f80ff58e0796c1c69de7699b819c404174ed7877c72458025070596a07f31e17",
            "8fd4ee4b95bc617468cb3f9e55bde1faaa0f93235fb0bf1602d2cde54f9d6d37",
            "8b8d198cc07227d2685c3bc3692d28644f3721dd8a5e73f71a1d7a9514c3f0a8",
        ),
        "artifact": "6caab33cec57480ed02c57fe37428a030a77cc2a0662814b435a5cf8932ad829",
    },
    "durable": {
        "prefix": "pi-durable-v110", "crosswalk": "pi-durable-v110-crosswalk.md",
        "counts": (59, 67, 90), "tests": 49,
        "hashes": (
            "b1b9b6ba2256e1dae31b3b4d0c9541a8d3758550057551a29eb02880c293275a",
            "ba6c49c3f81d64db273c4226832b6662454500f6722f656830915b6b3aa7ce07",
            "66642d504462ca000bcb8a09b357ef854ef5eb69954c983c488a81122cc83934",
            "23526c5dcceb5161edacdc914611f22cb02213c67d2af8f691c820e4e5dda0a0",
        ),
        "artifact": "a0f95b4a418e8bc219e9cbde06baedada940c62c47068829208e4fff278c07be",
    },
}


def rows(section: str, changed: bool) -> list[tuple[str, str, str]]:
    result = []
    for line in section.splitlines():
        cells = [cell.strip() for cell in line.strip().strip("|").split("|")]
        index = 1 if changed else 0
        if len(cells) != (4 if changed else 3) or not cells[index].startswith("`packages/"):
            continue
        path = cells[index].strip("`")
        if changed:
            path = f"{cells[0]}\t{path}"
        result.append((path, cells[index + 1], cells[index + 2]))
    return result


def section(text: str, heading: str) -> str:
    marker = f"## {heading}\n"
    if text.count(marker) != 1:
        raise ValueError(f"missing or duplicate crosswalk heading: {heading}")
    return text.split(marker, 1)[1].split("\n## ", 1)[0]


def validate(package: str, complete: bool, fault: str) -> str:
    spec = SPECS[package]
    inventories = []
    for index, kind in enumerate(("changed-paths", "source-corpus", "test-corpus", "shortstat")):
        path = ROOT / "docs/manifests" / f"{spec['prefix']}-{kind}.txt"
        data = path.read_bytes()
        if fault == "inventory" and index == 0:
            data += b"M\tpackages/invalid\n"
        if hashlib.sha256(data).hexdigest() != spec["hashes"][index]:
            raise ValueError(f"official inventory hash mismatch: {path.name}")
        lines = data.decode().splitlines()
        if index < 3:
            if len(lines) != spec["counts"][index] or len(set(lines)) != len(lines):
                raise ValueError(f"inventory cardinality/uniqueness mismatch: {path.name}")
            prefix = f"packages/{package}/"
            paths = [line.split("\t", 1)[-1] for line in lines]
            if any(not item.startswith(prefix) for item in paths):
                raise ValueError(f"foreign inventory path: {path.name}")
            inventories.append(lines)
    if sum(path.endswith(".test.ts") for path in inventories[2]) != spec["tests"]:
        raise ValueError("executable test inventory mismatch")
    text = (ROOT / "docs" / spec["crosswalk"]).read_text()
    if any(value not in text for value in (TAG_SHA, BASE_SHA, spec["artifact"])):
        raise ValueError("crosswalk source/artifact pin mismatch")
    mappings = [
        rows(section(text, "Changed-path disposition"), True),
        rows(section(text, "Full source corpus"), False),
        rows(section(text, "Full test and support corpus"), False),
    ]
    if fault == "crosswalk":
        mappings[0] = mappings[0][1:]
    pending = 0
    for inventory, mapping in zip(inventories, mappings):
        paths = [row[0] for row in mapping]
        if len(paths) != len(set(paths)) or paths != inventory:
            raise ValueError("crosswalk rows do not match exact ordered inventory")
        for path, disposition, evidence in mapping:
            incomplete = any(word in disposition.upper() for word in ("PENDING", "GAP", "SUBSET")) or disposition.startswith("INVENTORIED")
            placeholder = bool(re.search(r"not yet accepted|no full-contract acceptance|^pending$", evidence, re.I))
            if not disposition or not evidence:
                raise ValueError(f"empty disposition/evidence: {path}")
            pending += int(incomplete or placeholder)
    if complete and pending:
        raise ValueError(f"acceptance incomplete: {package} has {pending} unresolved crosswalk rows")
    return f"{package}: changed={spec['counts'][0]} source={spec['counts'][1]} corpus={spec['counts'][2]} tests={spec['tests']} unresolved={pending}"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--package", choices=("ai", "durable", "both"), default="both")
    parser.add_argument("--require-complete", action="store_true")
    parser.add_argument("--fault", choices=("", "inventory", "crosswalk"), default="")
    args = parser.parse_args()
    try:
        packages = ("ai", "durable") if args.package == "both" else (args.package,)
        for package in packages:
            print(validate(package, args.require_complete, args.fault))
    except (OSError, ValueError) as error:
        print(f"ERROR: {error}")
        return 1
    print("v1.1.0 official inventory/crosswalk structure verified" + ("; complete dispositions" if args.require_complete else "; acceptance not evaluated"))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
