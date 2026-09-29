#!/usr/bin/env python3
"""Generate src/classifier_models_generated.rs from signed schema-v6 classifier JSON."""
from __future__ import annotations

import json
import sys
from pathlib import Path


def rust_string(value: str) -> str:
    return json.dumps(value)


def generation_timestamp(input_path: Path) -> str:
    metadata_path = input_path.parent / "source-metadata.json"
    if metadata_path.exists():
        metadata = json.loads(metadata_path.read_text())
        generated_at = (metadata.get("manifest") or {}).get("generatedAt")
        if isinstance(generated_at, str) and generated_at:
            return generated_at
    return "deterministic"


def gen_model(model: dict) -> str:
    inputs = ", ".join(f"{rust_string(item)}.into()" for item in model.get("input", []))
    cost = model.get("cost") or {}
    if model.get("inputLimits") is not None:
        input_limits = (
            "Some(serde_json::from_str("
            + rust_string(json.dumps(model["inputLimits"], separators=(",", ":"), sort_keys=True))
            + ").unwrap())"
        )
    else:
        input_limits = "None"
    headers = model.get("headers")
    if headers:
        items = ", ".join(
            f"({rust_string(key)}.into(), {rust_string(value)}.into())"
            for key, value in sorted(headers.items())
        )
        header_value = f"Some(std::collections::HashMap::from([{items}]))"
    else:
        header_value = "None"
    return "\n".join(
        [
            "        ClassifierModel {",
            "            model_type: ModelType::Classifier,",
            f"            id: {rust_string(model['id'])}.into(),",
            f"            name: {rust_string(model['name'])}.into(),",
            f"            api: {rust_string(model['api'])}.into(),",
            f"            provider: {rust_string(model['provider'])}.into(),",
            f"            base_url: {rust_string(model.get('baseUrl', ''))}.into(),",
            f"            input: vec![{inputs}],",
            f"            input_limits: {input_limits},",
            "            cost: ModelCost {",
            f"                input: {cost.get('input', 0)}_f64,",
            f"                output: {cost.get('output', 0)}_f64,",
            f"                cache_read: {cost.get('cacheRead', 0)}_f64,",
            f"                cache_write: {cost.get('cacheWrite', 0)}_f64,",
            "                tiers: vec![],",
            "            },",
            f"            context_window: {model.get('contextWindow', 0)},",
            f"            headers: {header_value},",
            "            api_key: None,",
            "        }",
        ]
    )


def main() -> int:
    if len(sys.argv) != 2:
        print("Usage: generate_classifier_models.py classifier-models.json", file=sys.stderr)
        return 2
    input_path = Path(sys.argv[1])
    grouped = json.loads(input_path.read_text())
    models = [
        grouped[provider][model_id]
        for provider in sorted(grouped)
        for model_id in sorted(grouped[provider])
    ]
    output = [
        "//! Auto-generated classifier model registry from @earendil-works/pi-ai. DO NOT EDIT.",
        "//!",
        f"//! Source: signed schema-v6 provider shards ({len(models)} classifier models, {len(grouped)} providers)",
        f"//! Generated: {generation_timestamp(input_path)}",
        "",
        "use crate::types::{ClassifierModel, ModelCost, ModelType};",
        "",
        "/// Returns all built-in classifier models from the signed release registry.",
        "pub fn builtin_classifier_models() -> Vec<ClassifierModel> {",
        "    vec![",
    ]
    for model in models:
        output.append(gen_model(model) + ",")
    output += ["    ]", "}", ""]
    target = Path(__file__).resolve().parents[1] / "src/classifier_models_generated.rs"
    target.write_text("\n".join(output))
    print(f"Wrote {target} ({len(models)} classifier models, {len(grouped)} providers)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
