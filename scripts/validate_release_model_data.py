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

SUPPORTED_SCHEMA_VERSIONS = {3, 6}
MODEL_TYPES = {"chat", "image", "classifier"}
REQUIRED_STRING_FIELDS = {"id", "provider", "api", "name"}
MODALITIES = {"text", "image"}


def finite_number(value) -> bool:
    return isinstance(value, (int, float)) and not isinstance(value, bool) and value == value and abs(value) != float("inf")


def validate_model_fields(model: dict, filename: str, stored_key: str, schema_version: int) -> None:
    for field in REQUIRED_STRING_FIELDS:
        if not isinstance(model.get(field), str) or not model[field].strip():
            raise ValueError(f"model {stored_key} has invalid {field} in {filename}")
    if schema_version != 6:
        return
    model_type = model.get("type")
    if model_type not in MODEL_TYPES:
        raise ValueError(f"model {stored_key} has invalid type in {filename}")
    if not isinstance(model.get("baseUrl"), str):
        raise ValueError(f"model {stored_key} has invalid baseUrl in {filename}")
    inputs = model.get("input")
    if (
        not isinstance(inputs, list)
        or not inputs
        or any(item not in MODALITIES for item in inputs)
    ):
        raise ValueError(f"model {stored_key} has invalid input modalities in {filename}")
    output = model.get("output")
    if model_type == "image":
        if not isinstance(output, list) or "image" not in output or any(item not in MODALITIES for item in output):
            raise ValueError(f"model {stored_key} has invalid output modalities in {filename}")
    elif output is not None:
        raise ValueError(f"model {stored_key} must not define output modalities in {filename}")
    if model_type == "chat":
        if not isinstance(model.get("reasoning"), bool):
            raise ValueError(f"model {stored_key} has invalid reasoning in {filename}")
        for field in ("contextWindow", "maxTokens"):
            if not finite_number(model.get(field)) or model[field] <= 0:
                raise ValueError(f"model {stored_key} has invalid {field} in {filename}")
    elif model_type == "classifier":
        if not finite_number(model.get("contextWindow")) or model["contextWindow"] <= 0:
            raise ValueError(f"model {stored_key} has invalid contextWindow in {filename}")
    cost = model.get("cost")
    if not isinstance(cost, dict):
        raise ValueError(f"model {stored_key} has invalid cost in {filename}")
    for field in ("input", "output", "cacheRead", "cacheWrite"):
        value = cost.get(field)
        # Negative finite costs are upstream sentinel values for dynamic routing.
        if not finite_number(value):
            raise ValueError(f"model {stored_key} has invalid cost.{field} in {filename}")


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
    schema_version = manifest.get("schemaVersion")
    if schema_version not in SUPPORTED_SCHEMA_VERSIONS:
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

    disk_files = sorted(path.name for path in data_dir.glob("*.json") if path.name != ".manifest.json")
    missing_files = sorted(set(files) - set(disk_files))
    if missing_files:
        raise ValueError(f"missing provider shard: {missing_files[0]}")
    if disk_files != sorted(files):
        raise ValueError("manifest provider shard file set mismatch")

    seen: dict[tuple[str, str, str], str] = {}
    structure: dict[str, dict[str, str]] = {}
    type_counts = {model_type: 0 for model_type in sorted(MODEL_TYPES)}
    type_providers = {model_type: set() for model_type in MODEL_TYPES}
    type_apis = {model_type: set() for model_type in MODEL_TYPES}
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
            for stored_key, model in values.items():
                if not isinstance(model, dict):
                    raise ValueError(f"invalid model entry in {filename}")
                validate_model_fields(model, filename, stored_key, schema_version)
                model_id = model.get("id")
                model_provider = model.get("provider")
                model_api = model.get("api")
                if schema_version == 6:
                    if ":" not in stored_key:
                        raise ValueError(f"typed model key lacks discriminator in {filename}: {stored_key}")
                    model_type, expected_id = stored_key.split(":", 1)
                    if model_type not in MODEL_TYPES:
                        raise ValueError(f"unknown model type in {filename}: {model_type}")
                    if model.get("type") != model_type:
                        raise ValueError(
                            f"model {stored_key} has type {model.get('type')}, expected {model_type}"
                        )
                else:
                    model_type = "chat"
                    expected_id = stored_key
                    if model.get("type") not in (None, "chat"):
                        raise ValueError(f"schema-v3 model has non-chat type in {filename}: {stored_key}")
                if model_id != expected_id:
                    raise ValueError(f"model entry has id {model_id}, expected {expected_id}")
                if model_provider != provider:
                    raise ValueError(f"model {model_id} has provider {model_provider}, expected {provider}")
                if model_api != api:
                    raise ValueError(f"model {model_id} has api {model_api}, grouped under API {api}")
                key = (model_type, provider, model_id)
                if key in seen:
                    raise ValueError(
                        f"model {model_type}:{provider}/{model_id} appears in more than one API group"
                    )
                seen[key] = api
                provider_structure[stored_key] = api
                type_counts[model_type] += 1
                type_providers[model_type].add(provider)
                type_apis[model_type].add(api)
        structure[provider] = provider_structure

    structure_hash = hashlib.sha256(json.dumps(structure, sort_keys=True, separators=(",", ":")).encode()).hexdigest()
    if manifest.get("structureHash") != structure_hash:
        raise ValueError("model data generation stamp mismatch")
    return {
        "schemaVersion": schema_version,
        "providers": len(structure),
        "providerFiles": len(files),
        "models": len(seen),
        "structureHash": structure_hash,
        "types": {
            model_type: {
                "models": type_counts[model_type],
                "providers": len(type_providers[model_type]),
                "apis": len(type_apis[model_type]),
            }
            for model_type in sorted(MODEL_TYPES)
        },
    }


def main() -> int:
    if len(sys.argv) != 2:
        print("Usage: validate_release_model_data.py /path/to/dist/providers/data", file=sys.stderr)
        return 2
    result = validate_model_data_directory(Path(sys.argv[1]))
    print(json.dumps(result, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
