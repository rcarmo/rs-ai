#!/usr/bin/env python3
"""Verify exact typed full-record v0.87.1 -> v0.99.1 catalog deltas."""
from __future__ import annotations

import argparse
import json
import sys
import tempfile
from pathlib import Path
from typing import Any

import verify_release_model_metadata as metadata
import verify_v0851_baseline_delta as legacy

ROOT = Path(__file__).resolve().parents[1]
OLD_PACKAGE = "@earendil-works/pi-ai@0.87.1"
NEW_PACKAGE = "@earendil-works/pi-ai@0.99.1"
OLD_PACKAGE_SHA256 = "35b4432f27cc2665f86beebb9af6a39b1251970883c3044bd8be4f4e8c731ca0"
NEW_PACKAGE_SHA256 = "f9f44692157d0bf5679c4a17304a310028231d7daaeaaea3b73252f4b7a264d3"
EXPECTED = {
    "chat": (1495, 1523, 59, 31, 110),
    "image": (55, 57, 2, 0, 54),
    "classifier": (0, 12, 12, 0, 0),
}


def normalize(value: Any) -> Any:
    if isinstance(value, dict):
        return {key: normalize(value[key]) for key in sorted(value)}
    if isinstance(value, list):
        return [normalize(item) for item in value]
    return value


def typed_key(record: dict[str, Any]) -> tuple[str, str]:
    return record["provider"], record["id"]


def legacy_chat_records(package_dir: Path) -> dict[tuple[str, str], dict[str, Any]]:
    records = legacy.provider_records(package_dir)
    return {key: normalize({**record, "type": "chat"}) for key, record in records.items()}


def legacy_image_records(package_dir: Path) -> dict[tuple[str, str], dict[str, Any]]:
    records = legacy.image_records(package_dir)
    # The sole normalization allowed by the release audit: schema-v3 image
    # records lacked the required v6 discriminator. Preserve all other fields.
    return {key: normalize({**record, "type": "image"}) for key, record in records.items()}


def schema_v6_records(package_dir: Path) -> dict[str, dict[tuple[str, str], dict[str, Any]]]:
    result: dict[str, dict[tuple[str, str], dict[str, Any]]] = {
        "chat": {},
        "image": {},
        "classifier": {},
    }
    data_dir = package_dir / "dist/providers/data"
    for path in sorted(data_dir.glob("*.json")):
        if path.name == ".manifest.json":
            continue
        grouped = json.loads(path.read_text())
        for records in grouped.values():
            for stored_key, record in records.items():
                model_type, expected_id = stored_key.split(":", 1)
                if model_type not in result:
                    raise ValueError(f"unknown schema-v6 model type: {model_type}")
                if record.get("type") != model_type or record.get("id") != expected_id:
                    raise ValueError(f"typed key/record mismatch: {path.name}/{stored_key}")
                key = typed_key(record)
                if key in result[model_type]:
                    raise ValueError(f"duplicate typed record: {model_type}:{key}")
                result[model_type][key] = normalize(record)
    return result


def delta(
    old: dict[tuple[str, str], dict[str, Any]],
    new: dict[tuple[str, str], dict[str, Any]],
) -> tuple[int, int, int]:
    old_keys = set(old)
    new_keys = set(new)
    return (
        len(new_keys - old_keys),
        len(old_keys - new_keys),
        sum(1 for key in old_keys & new_keys if old[key] != new[key]),
    )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--fault",
        choices=["", "legacy-image-record", "new-chat-record", "new-classifier-record"],
        default="",
    )
    args = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix="rs-ai-v0991-delta-") as tmp:
        work = Path(tmp)
        old_work = work / "old"
        new_work = work / "new"
        old_work.mkdir()
        new_work.mkdir()
        old_package = metadata.extract_npm_package(OLD_PACKAGE, old_work, OLD_PACKAGE_SHA256)
        new_package = metadata.extract_npm_package(NEW_PACKAGE, new_work, NEW_PACKAGE_SHA256)
        metadata.run(
            [sys.executable, "scripts/validate_release_model_data.py", str(old_package / "dist/providers/data")],
            cwd=ROOT,
        )
        metadata.run(
            [sys.executable, "scripts/validate_release_model_data.py", str(new_package / "dist/providers/data")],
            cwd=ROOT,
        )
        old = {
            "chat": legacy_chat_records(old_package),
            "image": legacy_image_records(old_package),
            "classifier": {},
        }
        new = schema_v6_records(new_package)

        if args.fault == "legacy-image-record":
            key = next(
                key
                for key in sorted(set(old["image"]) & set(new["image"]))
                if old["image"][key] == new["image"][key]
            )
            old["image"][key] = {
                **old["image"][key],
                "name": old["image"][key]["name"] + " FAULT",
            }
        elif args.fault == "new-chat-record":
            key = next(
                key
                for key in sorted(set(old["chat"]) & set(new["chat"]))
                if old["chat"][key] == new["chat"][key]
            )
            new["chat"][key] = {
                **new["chat"][key],
                "name": new["chat"][key]["name"] + " FAULT",
            }
        elif args.fault == "new-classifier-record":
            del new["classifier"][sorted(new["classifier"])[0]]

        failures = []
        results = {}
        for model_type in ("chat", "image", "classifier"):
            got_delta = delta(old[model_type], new[model_type])
            got = (len(old[model_type]), len(new[model_type]), *got_delta)
            results[model_type] = got
            if got != EXPECTED[model_type]:
                failures.append(
                    f"{model_type} full-record delta mismatch: got "
                    f"{got[0]}->{got[1]} +{got[2]}/-{got[3]}/{got[4]} changed, expected "
                    f"{EXPECTED[model_type][0]}->{EXPECTED[model_type][1]} "
                    f"+{EXPECTED[model_type][2]}/-{EXPECTED[model_type][3]}/"
                    f"{EXPECTED[model_type][4]} changed"
                )
        if failures:
            print("\n".join(failures), file=sys.stderr)
            return 1
        print(
            "v0.99.1 typed baseline delta verified: "
            f"chat={results['chat'][0]}->{results['chat'][1]} +{results['chat'][2]}/-{results['chat'][3]}/{results['chat'][4]} changed "
            f"image={results['image'][0]}->{results['image'][1]} +{results['image'][2]}/-{results['image'][3]}/{results['image'][4]} changed "
            f"classifier={results['classifier'][0]}->{results['classifier'][1]} +{results['classifier'][2]}/-{results['classifier'][3]}/{results['classifier'][4]} changed"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
