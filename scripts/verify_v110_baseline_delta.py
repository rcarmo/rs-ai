#!/usr/bin/env python3
"""Verify exact typed v1.0.1 -> v1.1.0 full-record catalog deltas."""
from __future__ import annotations

import argparse
import sys
from pathlib import Path

from workspace_paths import temporary_directory
import verify_release_model_metadata as metadata
import verify_v101_baseline_delta as prior

ROOT = Path(__file__).resolve().parents[1]
OLD_SHA256 = "8a9e69b1309cf93405d87729fa123c8b11c6be7c646b16f34f8bef7b792f9138"
NEW_SHA256 = "6caab33cec57480ed02c57fe37428a030a77cc2a0662814b435a5cf8932ad829"
EXPECTED = {
    "chat": (1536, 1563, 79, 52, 199),
    "image": (59, 61, 2, 0, 0),
    "classifier": (20, 26, 6, 0, 1),
}


def verify(old: dict, new: dict, fault: str) -> list[str]:
    if fault:
        kind = fault.removesuffix("-record")
        # Independently perturb an otherwise unchanged full record, not only IDs.
        key = next(key for key in sorted(set(old[kind]) & set(new[kind])) if old[kind][key] == new[kind][key])
        new[kind][key] = {**new[kind][key], "name": new[kind][key]["name"] + " FAULT"}
    results = []
    for kind, expected in EXPECTED.items():
        got = (len(old[kind]), len(new[kind]), *prior.delta(old[kind], new[kind]))
        if got != expected:
            raise ValueError(f"{kind} full-record delta mismatch: got {got}, expected {expected}")
        results.append(f"{kind}={got[0]}->{got[1]} +{got[2]}/-{got[3]}/{got[4]} changed")
    # Azure identity migration is a semantic delta, not duplicate legacy data.
    if any(provider == "azure-openai-responses" for provider, _ in new["chat"]):
        raise ValueError("legacy Azure provider survived v1.1.0 migration")
    if not any(provider == "azure" for provider, _ in new["chat"]):
        raise ValueError("v1.1.0 Azure provider missing")
    return results


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--old-package", default="@earendil-works/pi-ai@1.0.1")
    parser.add_argument("--new-package", default="@earendil-works/pi-ai@1.1.0")
    parser.add_argument("--fault", choices=("", "chat-record", "image-record", "classifier-record"), default="")
    args = parser.parse_args()
    try:
        with temporary_directory("verify-v110-delta", prefix="typed-catalog-") as directory:
            work = Path(directory)
            old_work, new_work = work / "old", work / "new"
            old_work.mkdir(); new_work.mkdir()
            old = metadata.extract_npm_package(args.old_package, old_work, OLD_SHA256)
            new = metadata.extract_npm_package(args.new_package, new_work, NEW_SHA256)
            for package in (old, new):
                metadata.run([sys.executable, "-B", "scripts/validate_release_model_data.py", str(package / "dist/providers/data")], cwd=ROOT)
            results = verify(prior.records(old), prior.records(new), args.fault)
        print("v1.1.0 typed baseline delta verified: " + " ".join(results))
        return 0
    except (ValueError, RuntimeError) as error:
        print(str(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
