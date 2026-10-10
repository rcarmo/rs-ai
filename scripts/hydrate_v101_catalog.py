#!/usr/bin/env python3
"""Validate and stage all three v1.0.1 Rust catalogs before replacement."""
from __future__ import annotations

import argparse
import os
import shutil
import subprocess
import sys
import tarfile
from pathlib import Path

from workspace_paths import configure_process_env, temporary_directory

configure_process_env("hydrate-v101-catalog")
ROOT = Path(__file__).resolve().parents[1]
TARGETS = (
    Path("src/models_generated.rs"),
    Path("src/images/models_generated.rs"),
    Path("src/classifier_models_generated.rs"),
)
EXPECTED_SHA256 = "8a9e69b1309cf93405d87729fa123c8b11c6be7c646b16f34f8bef7b792f9138"
V110_SHA256 = "6caab33cec57480ed02c57fe37428a030a77cc2a0662814b435a5cf8932ad829"


def run(cmd: list[str], cwd: Path = ROOT) -> None:
    subprocess.run(cmd, cwd=cwd, check=True)


def sha256(path: Path) -> str:
    import hashlib
    h = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def hydrate(artifact: Path, validate_only: bool = False, fault: str = "", version: str = "1.0.1") -> None:
    expected = {"1.0.1": EXPECTED_SHA256, "1.1.0": V110_SHA256}.get(version)
    if expected is None or sha256(artifact) != expected:
        raise RuntimeError(f"v{version} artifact SHA-256 mismatch")
    before = {target: (ROOT / target).read_bytes() for target in TARGETS}
    with temporary_directory("hydrate-v101-catalog", prefix="rs-ai-v101-hydrate-") as tmp:
        work = Path(tmp)
        unpack = work / "unpack"
        unpack.mkdir()
        with tarfile.open(artifact, "r:gz") as archive:
            archive.extractall(unpack)
        package = unpack / "package"
        run([sys.executable, "scripts/validate_release_model_data.py", str(package / "dist/providers/data")])
        extracted = work / "extracted"
        run([sys.executable, "scripts/extract_release_model_shards.py", str(package), str(extracted)])

        stage = work / "stage"
        shutil.copytree(ROOT, stage, ignore=shutil.ignore_patterns(".git", "target"))
        run([sys.executable, "scripts/generate_models.py", str(extracted / "models.json")], stage)
        run([sys.executable, "scripts/generate_image_models.py", str(extracted / "image-models.json")], stage)
        run([sys.executable, "scripts/generate_classifier_models.py", str(extracted / "classifier-models.json")], stage)
        if fault == "render":
            raise RuntimeError("deliberate render failure")
        run(["rustfmt", *[str(path) for path in TARGETS]], stage)
        if fault == "format":
            raise RuntimeError("deliberate format failure")
        for target in TARGETS:
            if not (stage / target).is_file() or not (stage / target).read_bytes():
                raise RuntimeError(f"missing generated output: {target}")
        if validate_only:
            for target, content in before.items():
                if (ROOT / target).read_bytes() != content:
                    raise RuntimeError(f"validate-only mutated accepted output: {target}")
            return

        # Validation, rendering and formatting complete before the first accepted-file
        # replacement. POSIX does not provide one transaction across all three files.
        replacements: list[tuple[Path, Path]] = []
        try:
            for target in TARGETS:
                accepted = ROOT / target
                staged = stage / target
                temp = accepted.with_name(f".{accepted.name}.v101-{os.getpid()}")
                temp.write_bytes(staged.read_bytes())
                replacements.append((temp, accepted))
            for temp, accepted in replacements:
                os.replace(temp, accepted)
        finally:
            for temp, _ in replacements:
                temp.unlink(missing_ok=True)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("artifact", type=Path)
    parser.add_argument("--version", choices=["1.0.1", "1.1.0"], default="1.0.1")
    parser.add_argument("--validate-only", action="store_true")
    parser.add_argument("--fault", choices=["", "render", "format"], default="")
    args = parser.parse_args()
    try:
        hydrate(args.artifact.resolve(), args.validate_only, args.fault, args.version)
        print(f"v{args.version} catalogs validated" + (" without mutation" if args.validate_only else " and replaced"))
        return 0
    except Exception as error:
        print(str(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
