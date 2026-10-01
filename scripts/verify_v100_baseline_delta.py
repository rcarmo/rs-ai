#!/usr/bin/env python3
"""Verify exact typed full-record v0.99.2 -> v1.0.0 catalog deltas."""
from __future__ import annotations

import argparse
import json
import sys
import tempfile
from pathlib import Path
from typing import Any

import verify_release_model_metadata as metadata
import verify_v0991_baseline_delta as prior

ROOT = Path(__file__).resolve().parents[1]
OLD_PACKAGE = "@earendil-works/pi-ai@0.99.2"
NEW_PACKAGE = "@earendil-works/pi-ai@1.0.0"
OLD_PACKAGE_SHA256 = "0b3df8791b488216f309d908789294a744bb61bbaad123d94098e56df9538d25"
NEW_PACKAGE_SHA256 = "f39b99c29b8598f175b10840e5d2a81983e7c0ce5cae4d7df83a1007447d2c2b"
EXPECTED = {
    "chat": (1529, 1532, 5, 2, 19),
    "image": (57, 57, 0, 0, 0),
    "classifier": (15, 15, 0, 0, 1),
}


def normalize(value: Any) -> Any:
    if isinstance(value, dict):
        return {key: normalize(value[key]) for key in sorted(value)}
    if isinstance(value, list):
        return [normalize(item) for item in value]
    return value


def records(package_dir: Path) -> dict[str, dict[tuple[str, str], dict[str, Any]]]:
    return prior.schema_v6_records(package_dir)


def delta(old: dict, new: dict) -> tuple[int, int, int]:
    old_keys, new_keys = set(old), set(new)
    return (
        len(new_keys - old_keys),
        len(old_keys - new_keys),
        sum(1 for key in old_keys & new_keys if normalize(old[key]) != normalize(new[key])),
    )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--fault",
        choices=["", "chat-record", "image-record", "classifier-record"],
        default="",
    )
    args = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix="rs-ai-v100-delta-") as tmp:
        work = Path(tmp)
        old_work, new_work = work / "old", work / "new"
        old_work.mkdir(); new_work.mkdir()
        old_package = metadata.extract_npm_package(OLD_PACKAGE, old_work, OLD_PACKAGE_SHA256)
        new_package = metadata.extract_npm_package(NEW_PACKAGE, new_work, NEW_PACKAGE_SHA256)
        for package in (old_package, new_package):
            metadata.run(
                [sys.executable, "scripts/validate_release_model_data.py", str(package / "dist/providers/data")],
                cwd=ROOT,
            )
        old, new = records(old_package), records(new_package)
        if args.fault == "chat-record":
            key = next(key for key in sorted(set(old["chat"]) & set(new["chat"])) if old["chat"][key] == new["chat"][key])
            new["chat"][key] = {**new["chat"][key], "name": new["chat"][key]["name"] + " FAULT"}
        elif args.fault == "image-record":
            key = sorted(new["image"])[0]
            new["image"][key] = {**new["image"][key], "name": new["image"][key]["name"] + " FAULT"}
        elif args.fault == "classifier-record":
            del new["classifier"][sorted(new["classifier"])[0]]

        failures, results = [], {}
        for model_type in ("chat", "image", "classifier"):
            got_delta = delta(old[model_type], new[model_type])
            got = (len(old[model_type]), len(new[model_type]), *got_delta)
            results[model_type] = got
            if got != EXPECTED[model_type]:
                expected = EXPECTED[model_type]
                failures.append(
                    f"{model_type} full-record delta mismatch: got {got[0]}->{got[1]} "
                    f"+{got[2]}/-{got[3]}/{got[4]} changed, expected {expected[0]}->{expected[1]} "
                    f"+{expected[2]}/-{expected[3]}/{expected[4]} changed"
                )
        if failures:
            print("\n".join(failures), file=sys.stderr)
            return 1
        print(
            "v1.0.0 typed baseline delta verified: "
            f"chat={results['chat'][0]}->{results['chat'][1]} +{results['chat'][2]}/-{results['chat'][3]}/{results['chat'][4]} changed "
            f"image={results['image'][0]}->{results['image'][1]} +{results['image'][2]}/-{results['image'][3]}/{results['image'][4]} changed "
            f"classifier={results['classifier'][0]}->{results['classifier'][1]} +{results['classifier'][2]}/-{results['classifier'][3]}/{results['classifier'][4]} changed"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
