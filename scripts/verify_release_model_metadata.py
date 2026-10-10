#!/usr/bin/env python3
"""Verify release-pinned generated model metadata against committed Rust registries.

This is stricter than provider/id pair comparison: it regenerates the complete
Rust chat, image and classifier registries from the official npm package artifact, normalizes
only generated timestamps, rustfmt-formats the temporary output, and compares the
full Rust-representable metadata byte-for-byte against the committed files.

The command is self-contained for clean clones: it downloads the requested npm
package with `npm pack`, unpacks it into a temporary directory, validates/extracts
provider data shards, imports package IMAGE_MODELS with Bun, regenerates Rust
source into a temporary project copy, and compares.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import tarfile
from pathlib import Path

from workspace_paths import configure_process_env, temporary_directory

configure_process_env("verify-release-model-metadata")

ROOT = Path(__file__).resolve().parents[1]
TIMESTAMP_RE = re.compile(r"//! Generated: .*", re.MULTILINE)
DEFAULT_PACKAGE_SHA256 = "6caab33cec57480ed02c57fe37428a030a77cc2a0662814b435a5cf8932ad829"


def run(cmd: list[str], cwd: Path | None = None, env: dict[str, str] | None = None) -> str:
    merged_env = os.environ.copy()
    if env:
        merged_env.update(env)
    return subprocess.check_output(cmd, cwd=cwd, env=merged_env, text=True, stderr=subprocess.STDOUT)


def js_runtime() -> str:
    for candidate in ("bun", "node"):
        path = shutil.which(candidate)
        if path:
            return path
    raise SystemExit("neither bun nor node is available to import package image metadata")


def normalize_generated(text: str) -> str:
    return TIMESTAMP_RE.sub("//! Generated: <normalized>", text)


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def flatten_models(value) -> list[dict]:
    out: list[dict] = []
    if isinstance(value, list):
        for item in value:
            out.extend(flatten_models(item))
    elif isinstance(value, dict):
        if isinstance(value.get("provider"), str) and isinstance(value.get("id"), str):
            out.append(value)
        else:
            for item in value.values():
                out.extend(flatten_models(item))
    return out


def compare_file(label: str, expected: Path, actual: Path) -> list[str]:
    expected_text = normalize_generated(expected.read_text())
    actual_text = normalize_generated(actual.read_text())
    if expected_text == actual_text:
        return []
    # Keep the failure bounded but concrete; full diff can be obtained by rerunning
    # with the printed temp directory before cleanup in a debugger.
    expected_lines = expected_text.splitlines()
    actual_lines = actual_text.splitlines()
    limit = max(len(expected_lines), len(actual_lines))
    for index in range(limit):
        left = expected_lines[index] if index < len(expected_lines) else "<missing>"
        right = actual_lines[index] if index < len(actual_lines) else "<missing>"
        if left != right:
            return [
                f"{label} metadata mismatch at normalized line {index + 1}",
                f"committed: {left[:240]}",
                f"generated: {right[:240]}",
            ]
    return [f"{label} metadata mismatch"]


def extract_npm_package(package: str, work: Path, expected_sha256: str) -> Path:
    pack_dir = work / "npm-pack"
    pack_dir.mkdir()
    out = run(["npm", "pack", "--silent", package], cwd=pack_dir).strip().splitlines()
    if not out:
        raise SystemExit("npm pack produced no output")
    tarball = pack_dir / out[-1]
    if not tarball.exists():
        raise SystemExit(f"npm pack tarball missing: {tarball}")
    actual_sha256 = sha256_file(tarball)
    if expected_sha256 and actual_sha256 != expected_sha256:
        raise SystemExit(
            f"npm tarball sha256 mismatch for {package}: got {actual_sha256}, expected {expected_sha256}"
        )
    package_dir = work / "npm-package"
    package_dir.mkdir()
    with tarfile.open(tarball, "r:gz") as tar:
        tar.extractall(package_dir)
    unpacked = package_dir / "package"
    if not unpacked.is_dir():
        raise SystemExit(f"unpacked npm package missing package dir: {unpacked}")
    return unpacked


def copy_project_for_generation(work: Path) -> Path:
    generated_root = work / "generated-project"
    shutil.copytree(ROOT, generated_root, ignore=shutil.ignore_patterns(".git", "target"))
    return generated_root


def maybe_fault(path: Path, fault: str) -> None:
    if not fault:
        return
    target = path.read_text()
    if fault == "text-name":
        old = 'name: "Qwen3.8 Max".into()'
        new = 'name: "Qwen3.8 Max FAULT".into()'
    elif fault == "image-name":
        old = 'name: "Black Forest Labs: FLUX.2 Flex".into()'
        new = 'name: "Black Forest Labs: FLUX.2 Flex FAULT".into()'
    elif fault == "classifier-name":
        old = 'name: "Jev".into()'
        new = 'name: "Jev FAULT".into()'
    else:
        raise SystemExit(f"unknown fault mode: {fault}")
    if old not in target:
        raise SystemExit(f"fault target not found: {old}")
    path.write_text(target.replace(old, new, 1))


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--package", default="@earendil-works/pi-ai@1.1.0")
    ap.add_argument("--package-sha256", default=DEFAULT_PACKAGE_SHA256)
    ap.add_argument("--upstream", default="", help="ignored compatibility option; npm artifact is authoritative")
    ap.add_argument("--tag-sha", default="", help="ignored compatibility option; npm artifact is authoritative")
    ap.add_argument("--fault", default="", choices=["", "text-name", "image-name", "classifier-name"], help="test-only metadata fault injection")
    args = ap.parse_args()

    with temporary_directory("verify-release-model-metadata", prefix="rs-ai-model-meta-") as tmp:
        work = Path(tmp)
        package_dir = extract_npm_package(args.package, work, args.package_sha256)
        data_dir = package_dir / "dist/providers/data"
        run([sys.executable, "scripts/validate_release_model_data.py", str(data_dir)], cwd=ROOT)
        extracted = work / "release-json"
        run([
            sys.executable,
            "scripts/extract_release_model_shards.py",
            str(package_dir),
            str(extracted),
        ], cwd=ROOT)

        generated_root = copy_project_for_generation(work)
        run([sys.executable, "scripts/generate_models.py", str(extracted / "models.json")], cwd=generated_root)
        image_json = extracted / "image-models.json"
        classifier_json = extracted / "classifier-models.json"
        run([sys.executable, "scripts/generate_image_models.py", str(image_json)], cwd=generated_root)
        run([sys.executable, "scripts/generate_classifier_models.py", str(classifier_json)], cwd=generated_root)

        # rustfmt only the generated temp files; this normalizes generator formatting
        # without touching the committed working tree.
        run([
            "rustfmt",
            "src/models_generated.rs",
            "src/images/models_generated.rs",
            "src/classifier_models_generated.rs",
        ], cwd=generated_root)

        maybe_fault(generated_root / "src/models_generated.rs", args.fault if args.fault.startswith("text") else "")
        maybe_fault(generated_root / "src/images/models_generated.rs", args.fault if args.fault.startswith("image") else "")
        maybe_fault(generated_root / "src/classifier_models_generated.rs", args.fault if args.fault.startswith("classifier") else "")

        failures: list[str] = []
        failures.extend(compare_file("text", ROOT / "src/models_generated.rs", generated_root / "src/models_generated.rs"))
        failures.extend(compare_file("image", ROOT / "src/images/models_generated.rs", generated_root / "src/images/models_generated.rs"))
        failures.extend(compare_file("classifier", ROOT / "src/classifier_models_generated.rs", generated_root / "src/classifier_models_generated.rs"))
        if failures:
            print("\n".join(failures), file=sys.stderr)
            return 1

        metadata = json.loads((extracted / "source-metadata.json").read_text())
        print(
            "metadata verified: "
            f"text={metadata['modelCount']} providers={metadata['providerCount']} apis={metadata['apiCount']} "
            f"batchAliases={metadata['batchAliasCount']} "
            f"image={metadata['imageModelCount']} imageProviders={metadata['imageProviderCount']} "
            f"imageApis={metadata['imageApiCount']} classifier={metadata['classifierModelCount']} "
            f"classifierProviders={metadata['classifierProviderCount']} "
            f"classifierApis={metadata['classifierApiCount']} total={metadata['totalRecordCount']}"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
