#!/usr/bin/env python3
"""Generate src/images/models_generated.rs from upstream image-models JSON.

Usage:
    # First dump image models to JSON via bun:
    bun --eval "import { IMAGE_MODELS } from 'path/to/image-models.generated.js'; process.stdout.write(JSON.stringify(IMAGE_MODELS));" > /tmp/image_models.json
    # Then generate:
    python3 scripts/generate_image_models.py /tmp/image_models.json
"""

import json
import sys
import datetime
from pathlib import Path


def rust_string(s):
    return json.dumps(s)


def gen_model(m) -> str:
    lines = []
    lines.append("        ImagesModel {")
    lines.append(f'            id: {rust_string(m["id"])}.into(),')
    lines.append(f'            name: {rust_string(m["name"])}.into(),')
    lines.append(f'            api: {rust_string(m["api"])}.into(),')
    lines.append(f'            provider: {rust_string(m["provider"])}.into(),')
    lines.append(f'            base_url: {rust_string(m.get("baseUrl", ""))}.into(),')
    inputs = ", ".join(f'{rust_string(i)}.into()' for i in m.get("input", []))
    lines.append(f"            input: vec![{inputs}],")
    outputs = ", ".join(f'{rust_string(o)}.into()' for o in m.get("output", []))
    lines.append(f"            output: vec![{outputs}],")
    cost = m.get("cost", {})
    ci = cost.get("input", 0)
    co = cost.get("output", 0)
    cr = cost.get("cacheRead", 0)
    cw = cost.get("cacheWrite", 0)
    lines.append(f"            cost: ModelCost {{ input: {ci}_f64, output: {co}_f64, cache_read: {cr}_f64, cache_write: {cw}_f64, tiers: vec![] }},")
    lines.append("        }")
    return "\n".join(lines)


def main():
    if len(sys.argv) < 2:
        print("Usage: python3 scripts/generate_image_models.py /tmp/image_models.json", file=sys.stderr)
        sys.exit(1)

    models = json.loads(Path(sys.argv[1]).read_text())

    all_models = []
    for provider in sorted(models.keys()):
        for model_id in sorted(models[provider].keys()):
            all_models.append(models[provider][model_id])

    total = len(all_models)
    providers = len(models)
    now = datetime.datetime.now(datetime.UTC).isoformat()

    out = []
    out.append("//! Auto-generated image model registry from @earendil-works/pi-ai. DO NOT EDIT.")
    out.append("//!")
    out.append(f"//! Source: image-models.generated.js ({total} image models, {providers} provider)")
    out.append(f"//! Generated: {now}")
    out.append("")
    out.append("use crate::images::types::ImagesModel;")
    out.append("use crate::types::ModelCost;")
    out.append("")
    out.append("/// Returns all built-in image models from the upstream pi-ai registry.")
    out.append("pub fn builtin_image_models() -> Vec<ImagesModel> {")
    out.append("    vec![")
    for m in all_models:
        out.append(gen_model(m) + ",")
    out.append("    ]")
    out.append("}")

    output_path = Path(__file__).parent.parent / "src" / "images" / "models_generated.rs"
    output_path.write_text("\n".join(out) + "\n")
    print(f"Wrote {output_path} ({total} image models, {providers} provider)")


if __name__ == "__main__":
    main()
