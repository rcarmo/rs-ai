#!/usr/bin/env python3
"""Sentinel tests for staged v1.0.1 catalog hydration and preflight validation."""
from __future__ import annotations

import argparse
import hashlib
import json
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
TARGETS = (
    ROOT / "src/models_generated.rs",
    ROOT / "src/images/models_generated.rs",
    ROOT / "src/classifier_models_generated.rs",
)


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def invoke(artifact: Path, *args: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [sys.executable, "scripts/hydrate_v101_catalog.py", str(artifact), *args],
        cwd=ROOT,
        text=True,
        capture_output=True,
    )


def recompute_manifest(data_dir: Path) -> None:
    manifest_path = data_dir / ".manifest.json"
    manifest = json.loads(manifest_path.read_text())
    manifest["files"] = {
        path.name: hashlib.sha256(path.read_bytes()).hexdigest()
        for path in sorted(data_dir.glob("*.json"))
        if path.name != ".manifest.json"
    }
    structure = {}
    for path in sorted(data_dir.glob("*.json")):
        if path.name == ".manifest.json":
            continue
        grouped = json.loads(path.read_text())
        structure[path.stem] = {
            stored_key: api
            for api, models in grouped.items()
            for stored_key in models
        }
    manifest["structureHash"] = hashlib.sha256(
        json.dumps(structure, sort_keys=True, separators=(",", ":")).encode()
    ).hexdigest()
    manifest_path.write_text(json.dumps(manifest, separators=(",", ":")) + "\n")


def validator_fixture_cases(artifact: Path) -> None:
    with tempfile.TemporaryDirectory(prefix="rs-ai-v101-validation-") as tmp:
        work = Path(tmp)
        subprocess.run(["tar", "-xzf", str(artifact), "-C", str(work)], check=True)
        pristine = work / "package/dist/providers/data"
        source = pristine / "anthropic.json"
        grouped = json.loads(source.read_text())
        api = next(iter(grouped))
        key = next(iter(grouped[api]))

        def run_fault(name: str, mutate) -> None:
            fixture = work / name
            shutil.copytree(pristine, fixture)
            target = fixture / source.name
            data = json.loads(target.read_text())
            mutate(data, api, key)
            target.write_text(json.dumps(data, separators=(",", ":")) + "\n")
            recompute_manifest(fixture)
            result = subprocess.run(
                [sys.executable, "scripts/validate_release_model_data.py", str(fixture)],
                cwd=ROOT,
                text=True,
                capture_output=True,
            )
            if result.returncode == 0:
                raise SystemExit(f"malformed fixture unexpectedly passed: {name}")

        run_fault("bad-provider", lambda d, a, k: d[a][k].__setitem__("provider", "wrong"))
        run_fault("bad-api", lambda d, a, k: d[a][k].__setitem__("api", "wrong"))
        run_fault("bad-cost", lambda d, a, k: d[a][k].__setitem__("cost", {"input": "bad"}))
        run_fault("missing-cost", lambda d, a, k: d[a][k].pop("cost", None))
        run_fault("bad-modalities", lambda d, a, k: d[a][k].__setitem__("input", ["bogus"]))
        run_fault("missing-context", lambda d, a, k: d[a][k].pop("contextWindow", None))

        image_source = pristine / "openrouter.json"
        image_grouped = json.loads(image_source.read_text())
        image_api, image_key = next(
            (api_name, stored_key)
            for api_name, models in image_grouped.items()
            for stored_key, model in models.items()
            if model.get("type") == "image"
        )
        image_fixture = work / "bad-image-output"
        shutil.copytree(pristine, image_fixture)
        image_target = image_fixture / image_source.name
        image_data = json.loads(image_target.read_text())
        image_data[image_api][image_key]["output"] = ["text"]
        image_target.write_text(json.dumps(image_data, separators=(",", ":")) + "\n")
        recompute_manifest(image_fixture)
        image_result = subprocess.run(
            [sys.executable, "scripts/validate_release_model_data.py", str(image_fixture)],
            cwd=ROOT,
            text=True,
            capture_output=True,
        )
        if image_result.returncode == 0:
            raise SystemExit("bad image output unexpectedly passed")

        missing = work / "missing-file"
        shutil.copytree(pristine, missing)
        victim = next(path for path in sorted(missing.glob("*.json")) if path.name != ".manifest.json")
        victim.unlink()
        result = subprocess.run(
            [sys.executable, "scripts/validate_release_model_data.py", str(missing)],
            cwd=ROOT,
            text=True,
            capture_output=True,
        )
        if result.returncode == 0:
            raise SystemExit("missing provider file unexpectedly passed")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("artifact", type=Path)
    args = parser.parse_args()
    artifact = args.artifact.resolve()
    before = [path.read_bytes() for path in TARGETS]
    clean = invoke(artifact, "--validate-only")
    if clean.returncode != 0:
        raise SystemExit(clean.stderr)
    if [path.read_bytes() for path in TARGETS] != before:
        raise SystemExit("validate-only changed accepted generated files")
    for fault in ("render", "format"):
        failed = invoke(artifact, "--fault", fault)
        if failed.returncode == 0:
            raise SystemExit(f"{fault} sentinel unexpectedly passed")
        if [path.read_bytes() for path in TARGETS] != before:
            raise SystemExit(f"{fault} sentinel changed accepted generated files")
    validator_fixture_cases(artifact)
    print(
        "v1.0.1 hydration sentinels passed: validateOnly and normal render/format failures "
        "preserve outputs; malformed provider/API/cost/capacity/modalities/output/missing-file fixtures reject"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
