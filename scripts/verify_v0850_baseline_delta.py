#!/usr/bin/env python3
"""Verify exact full-record v0.84.4 -> v0.85.0 model deltas."""
from __future__ import annotations

import argparse
import hashlib
import json
import shutil
import subprocess
import sys
import tarfile
import tempfile
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
OLD_PACKAGE = "@earendil-works/pi-ai@0.84.4"
NEW_PACKAGE = "@earendil-works/pi-ai@0.85.0"
OLD_PACKAGE_SHA256 = "dfd3c929cee5a7387199a0a24dfc1be2096f1ea8f59ffb8285198a0ed01ebf93"
NEW_PACKAGE_SHA256 = "46188bdacb555a07466a0111f3963f20932a16199e4d6cfb8d44a7fe5fc6e342"
EXPECTED_TEXT = (72, 26, 79)
EXPECTED_IMAGE = (0, 0, 0)


def run(cmd: list[str], cwd: Path | None = None) -> str:
    return subprocess.check_output(cmd, cwd=cwd, text=True, stderr=subprocess.STDOUT)


def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def extract_package(package: str, expected_sha256: str, work: Path) -> Path:
    pack_dir = work / package.rsplit("@", 1)[-1]
    pack_dir.mkdir()
    out = run(["npm", "pack", "--silent", package], cwd=pack_dir).strip().splitlines()
    if not out:
        raise SystemExit(f"npm pack produced no output for {package}")
    tarball = pack_dir / out[-1]
    actual = sha256(tarball)
    if actual != expected_sha256:
        raise SystemExit(f"{package} sha256 mismatch: got {actual}, expected {expected_sha256}")
    with tarfile.open(tarball, "r:gz") as tar:
        tar.extractall(pack_dir)
    unpacked = pack_dir / "package"
    if not unpacked.is_dir():
        raise SystemExit(f"unpacked package missing: {unpacked}")
    return unpacked


def flatten(value: Any) -> list[dict[str, Any]]:
    out: list[dict[str, Any]] = []
    if isinstance(value, list):
        for item in value:
            out.extend(flatten(item))
    elif isinstance(value, dict):
        if isinstance(value.get("provider"), str) and isinstance(value.get("id"), str):
            out.append(value)
        else:
            for item in value.values():
                out.extend(flatten(item))
    return out


def provider_records(package_dir: Path) -> dict[tuple[str, str], dict[str, Any]]:
    records: dict[tuple[str, str], dict[str, Any]] = {}
    for path in sorted((package_dir / "dist/providers/data").glob("*.json")):
        if path.name == ".manifest.json":
            continue
        for record in flatten(json.loads(path.read_text())):
            key = (record["provider"], record["id"])
            if key in records:
                raise SystemExit(f"duplicate text model record: {key}")
            records[key] = normalize(record)
    return records


def js_runtime() -> str:
    for candidate in ("bun", "node"):
        path = shutil.which(candidate)
        if path:
            return path
    raise SystemExit("neither bun nor node is available")


def image_records(package_dir: Path) -> dict[tuple[str, str], dict[str, Any]]:
    module = (package_dir / "dist/image-models.generated.js").resolve()
    script = package_dir / "image-records.mjs"
    script.write_text(
        "import { IMAGE_MODELS } from "
        + json.dumps(module.as_uri())
        + ";\nprocess.stdout.write(JSON.stringify(IMAGE_MODELS));\n"
    )
    try:
        data = json.loads(run([js_runtime(), str(script)], cwd=package_dir))
    finally:
        script.unlink(missing_ok=True)
    records: dict[tuple[str, str], dict[str, Any]] = {}
    for record in flatten(data):
        key = (record["provider"], record["id"])
        if key in records:
            raise SystemExit(f"duplicate image model record: {key}")
        records[key] = normalize(record)
    return records


def normalize(value: Any) -> Any:
    if isinstance(value, dict):
        return {key: normalize(value[key]) for key in sorted(value)}
    if isinstance(value, list):
        return [normalize(item) for item in value]
    return value


def delta(old: dict[tuple[str, str], dict[str, Any]], new: dict[tuple[str, str], dict[str, Any]]) -> tuple[int, int, int]:
    old_keys = set(old)
    new_keys = set(new)
    added = len(new_keys - old_keys)
    removed = len(old_keys - new_keys)
    changed = sum(1 for key in old_keys & new_keys if old[key] != new[key])
    return added, removed, changed


def check_delta(label: str, got: tuple[int, int, int], expected: tuple[int, int, int]) -> list[str]:
    if got == expected:
        return []
    return [f"{label} full-record delta mismatch: got +{got[0]}/-{got[1]}/{got[2]} changed, expected +{expected[0]}/-{expected[1]}/{expected[2]} changed"]


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--fault", choices=["", "baseline-record"], default="")
    args = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix="rs-ai-v0850-delta-") as tmp:
        work = Path(tmp)
        old_pkg = extract_package(OLD_PACKAGE, OLD_PACKAGE_SHA256, work)
        new_pkg = extract_package(NEW_PACKAGE, NEW_PACKAGE_SHA256, work)
        old_text = provider_records(old_pkg)
        new_text = provider_records(new_pkg)
        old_image = image_records(old_pkg)
        new_image = image_records(new_pkg)
        if args.fault:
            key = sorted(set(old_text) & set(new_text))[0]
            old_text[key] = {**old_text[key], "name": str(old_text[key].get("name", "")) + " FAULT"}
        text_delta = delta(old_text, new_text)
        image_delta = delta(old_image, new_image)
        failures = []
        failures.extend(check_delta("text", text_delta, EXPECTED_TEXT))
        failures.extend(check_delta("image", image_delta, EXPECTED_IMAGE))
        if failures:
            print("\n".join(failures), file=sys.stderr)
            return 1
        print(
            "v0.85.0 baseline delta verified: "
            f"text=+{text_delta[0]}/-{text_delta[1]}/{text_delta[2]} changed "
            f"image=+{image_delta[0]}/-{image_delta[1]}/{image_delta[2]} changed"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
