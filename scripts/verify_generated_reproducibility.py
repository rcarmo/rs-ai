#!/usr/bin/env python3
"""Verify generated catalog output is byte-for-byte reproducible."""
from __future__ import annotations

import argparse
import sys
import tempfile
from pathlib import Path

import verify_release_model_metadata as meta

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_PACKAGE = "@earendil-works/pi-ai@0.87.1"
DEFAULT_PACKAGE_SHA256 = "35b4432f27cc2665f86beebb9af6a39b1251970883c3044bd8be4f4e8c731ca0"


def generate_once(label: str, work: Path, extracted: Path, image_json: Path) -> Path:
    run_root = work / label
    run_root.mkdir()
    generated_root = meta.copy_project_for_generation(run_root)
    meta.run([sys.executable, "scripts/generate_models.py", str(extracted / "models.json")], cwd=generated_root)
    meta.run([sys.executable, "scripts/generate_image_models.py", str(image_json)], cwd=generated_root)
    meta.run(["rustfmt", "src/models_generated.rs", "src/images/models_generated.rs"], cwd=generated_root)
    return generated_root


def compare_bytes(label: str, left: Path, right: Path) -> list[str]:
    left_bytes = left.read_bytes()
    right_bytes = right.read_bytes()
    if left_bytes == right_bytes:
        return []
    return [
        f"{label} is not reproducible byte-for-byte",
        f"left={left} sha256={meta.sha256_file(left)}",
        f"right={right} sha256={meta.sha256_file(right)}",
    ]


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--package", default=DEFAULT_PACKAGE)
    parser.add_argument("--package-sha256", default=DEFAULT_PACKAGE_SHA256)
    args = parser.parse_args()

    with tempfile.TemporaryDirectory(prefix="rs-ai-generated-repro-") as tmp:
        work = Path(tmp)
        package_dir = meta.extract_npm_package(args.package, work, args.package_sha256)
        data_dir = package_dir / "dist/providers/data"
        meta.run([sys.executable, "scripts/validate_release_model_data.py", str(data_dir)], cwd=ROOT)
        extracted = work / "release-json"
        meta.run(
            [sys.executable, "scripts/extract_release_model_shards.py", str(package_dir), str(extracted)],
            cwd=ROOT,
        )
        image_json = extracted / "image-models.json"
        meta.package_image_json(package_dir, image_json)

        first = generate_once("first", work, extracted, image_json)
        second = generate_once("second", work, extracted, image_json)

        failures: list[str] = []
        failures.extend(compare_bytes("text catalog", first / "src/models_generated.rs", second / "src/models_generated.rs"))
        failures.extend(compare_bytes("image catalog", first / "src/images/models_generated.rs", second / "src/images/models_generated.rs"))
        if failures:
            print("\n".join(failures), file=sys.stderr)
            return 1
        print("generated catalog reproducibility verified byte-for-byte")
        print(f"text sha256={meta.sha256_file(first / 'src/models_generated.rs')}")
        print(f"image sha256={meta.sha256_file(first / 'src/images/models_generated.rs')}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
