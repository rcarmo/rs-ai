#!/usr/bin/env python3
"""Validate release-pinned pi-ai model-data shards.

This is the rs-ai offline counterpart for upstream model-data-validation tests.
It validates the official npm `dist/providers/data` layout before extraction.
"""
from __future__ import annotations

import datetime as dt
import hashlib
import json
import sys
from pathlib import Path

EXPECTED_SCHEMA_VERSION = 3


def sha256_text(text: str) -> str:
    return hashlib.sha256(text.encode()).hexdigest()


def flatten(value):
    if isinstance(value, dict) and isinstance(value.get("id"), str):
        yield value
        return
    if isinstance(value, dict):
        for item in value.values():
            yield from flatten(item)
    elif isinstance(value, list):
        for item in value:
            yield from flatten(item)


def validate_model_data_directory(data_dir: Path) -> dict:
    if not data_dir.is_dir():
        raise ValueError(f"model data directory does not exist: {data_dir}")
    manifest_path = data_dir / ".manifest.json"
    if not manifest_path.exists():
        raise ValueError("missing model data manifest")
    manifest = json.loads(manifest_path.read_text())
    if manifest.get("schemaVersion") != EXPECTED_SCHEMA_VERSION:
        raise ValueError("incompatible model data schema")
    generated_at = manifest.get("generatedAt")
    try:
        if not isinstance(generated_at, str):
            raise ValueError
        dt.datetime.fromisoformat(generated_at.replace("Z", "+00:00"))
    except Exception as exc:
        raise ValueError("invalid generation timestamp") from exc
    files = manifest.get("files")
    if not isinstance(files, dict) or not files:
        raise ValueError("manifest has no provider shard files")

    seen: dict[tuple[str, str], str] = {}
    structure: dict[str, dict[str, str]] = {}
    for filename, expected_hash in files.items():
        path = data_dir / filename
        if not path.exists():
            raise ValueError(f"missing provider shard: {filename}")
        text = path.read_text()
        actual = sha256_text(text)
        if actual != expected_hash:
            raise ValueError(f"manifest hash mismatch for {filename}")
        provider = path.stem
        try:
            grouped = json.loads(text)
        except Exception as exc:
            raise ValueError(f"invalid provider shard JSON: {filename}") from exc
        if not isinstance(grouped, dict) or not grouped:
            raise ValueError(f"empty provider shard: {filename}")
        provider_structure: dict[str, str] = {}
        for api, values in grouped.items():
            if not isinstance(api, str) or not isinstance(values, dict):
                raise ValueError(f"invalid API group in {filename}")
            for expected_id, model in values.items():
                if not isinstance(model, dict):
                    raise ValueError(f"invalid model entry in {filename}")
                model_id = model.get("id")
                model_provider = model.get("provider")
                model_api = model.get("api")
                if model_id != expected_id:
                    raise ValueError(f"model entry has id {model_id}, expected {expected_id}")
                if model_provider != provider:
                    raise ValueError(f"model {model_id} has provider {model_provider}, expected {provider}")
                if model_api != api:
                    raise ValueError(f"model {model_id} has api {model_api}, grouped under API {api}")
                key = (provider, model_id)
                if key in seen:
                    raise ValueError(f"model {provider}/{model_id} appears in more than one API group")
                seen[key] = api
                provider_structure[model_id] = api
        structure[provider] = provider_structure

    structure_hash = hashlib.sha256(json.dumps(structure, sort_keys=True, separators=(",", ":")).encode()).hexdigest()
    if manifest.get("structureHash") != structure_hash:
        raise ValueError("model data generation stamp mismatch")
    return {"providers": len(structure), "models": len(seen), "structureHash": structure_hash}


def main() -> int:
    if len(sys.argv) != 2:
        print("Usage: validate_release_model_data.py /path/to/dist/providers/data", file=sys.stderr)
        return 2
    result = validate_model_data_directory(Path(sys.argv[1]))
    print(json.dumps(result, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
