#!/usr/bin/env python3
"""Generate and validate the rs-ai CycloneDX SBOM.

The generator is intentionally repo-local and deterministic: it consumes
`cargo metadata --locked --all-features`, omits local filesystem paths, and
writes canonical JSON plus a SHA-256 checksum.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
import sys
import tempfile
from collections import deque
from pathlib import Path
from urllib.parse import quote

ROOT = Path(__file__).resolve().parents[1]
GENERATOR_NAME = "rs-ai-sbom.py"
GENERATOR_VERSION = "1.0.0"
SPEC_VERSION = "1.5"
DEFAULT_SBOM = ROOT / "artifacts/sbom.cdx.json"
DEFAULT_SHA = ROOT / "artifacts/sbom.cdx.json.sha256"


def run(cmd: list[str], cwd: Path = ROOT) -> str:
    return subprocess.check_output(cmd, cwd=cwd, text=True, stderr=subprocess.STDOUT)


def cargo_metadata() -> dict:
    return json.loads(
        subprocess.check_output(
            ["cargo", "metadata", "--locked", "--all-features", "--format-version", "1"],
            cwd=ROOT,
            text=True,
        )
    )


def git_revision() -> str:
    return run(["git", "rev-parse", "HEAD"]).strip()


def package_url(package: dict, root: bool = False) -> str:
    name = quote(package["name"], safe="")
    version = quote(package["version"], safe="")
    return f"pkg:cargo/{name}@{version}" if root else f"pkg:cargo/{name}@{version}"


def license_entry(package: dict) -> list[dict]:
    license_expr = package.get("license")
    if license_expr:
        return [{"expression": license_expr}]
    return []


def component_for(package: dict, *, root: bool = False, revision: str | None = None) -> dict:
    component = {
        "type": "library",
        "bom-ref": package_url(package, root=root),
        "name": package["name"],
        "version": package["version"],
        "purl": package_url(package, root=root),
    }
    licenses = license_entry(package)
    if licenses:
        component["licenses"] = licenses
    description = package.get("description")
    if description:
        component["description"] = description
    repository = package.get("repository")
    if repository:
        component["externalReferences"] = [{"type": "vcs", "url": repository}]
    if root and revision:
        component["properties"] = [{"name": "rs-ai:vcs:revision", "value": revision}]
    return component


def reachable_package_ids(metadata: dict) -> tuple[str, set[str], dict[str, list[str]]]:
    resolve = metadata.get("resolve") or {}
    root_id = resolve.get("root")
    if not root_id:
        raise SystemExit("cargo metadata did not include resolve.root")
    node_deps: dict[str, list[str]] = {}
    for node in resolve.get("nodes", []):
        deps = []
        for dep in node.get("deps", []):
            pkg = dep.get("pkg")
            if pkg:
                deps.append(pkg)
        node_deps[node["id"]] = sorted(set(deps))
    seen: set[str] = set()
    queue: deque[str] = deque([root_id])
    while queue:
        current = queue.popleft()
        if current in seen:
            continue
        seen.add(current)
        queue.extend(node_deps.get(current, []))
    return root_id, seen, node_deps


def build_sbom() -> dict:
    metadata = cargo_metadata()
    root_id, reachable, node_deps = reachable_package_ids(metadata)
    packages = {package["id"]: package for package in metadata.get("packages", [])}
    if root_id not in packages:
        raise SystemExit("resolve.root package is missing from metadata packages")
    revision = git_revision()
    root_package = packages[root_id]

    # Include only resolved reachable third-party packages in components. The root
    # crate is represented by metadata.component so consumers can distinguish it.
    component_packages = [
        packages[pkg_id]
        for pkg_id in reachable
        if pkg_id != root_id and packages[pkg_id].get("source") is not None
    ]
    component_packages.sort(key=lambda package: (package["name"], package["version"], package.get("source") or ""))

    refs = {pkg_id: package_url(package, root=(pkg_id == root_id)) for pkg_id, package in packages.items()}
    dependencies = []
    for pkg_id in sorted(reachable, key=lambda item: refs[item]):
        depends_on = [refs[dep] for dep in node_deps.get(pkg_id, []) if dep in reachable and dep in refs]
        dependencies.append({"ref": refs[pkg_id], "dependsOn": sorted(set(depends_on))})

    return {
        "bomFormat": "CycloneDX",
        "specVersion": SPEC_VERSION,
        "version": 1,
        "metadata": {
            "tools": {
                "components": [
                    {
                        "type": "application",
                        "name": GENERATOR_NAME,
                        "version": GENERATOR_VERSION,
                    }
                ]
            },
            "component": component_for(root_package, root=True, revision=revision),
        },
        "components": [component_for(package) for package in component_packages],
        "dependencies": dependencies,
    }


def canonical_json_bytes(value: dict) -> bytes:
    return (json.dumps(value, indent=2, sort_keys=True, ensure_ascii=False) + "\n").encode("utf-8")


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def validate_sbom(value: dict, checksum_text: str | None = None, raw: bytes | None = None) -> list[str]:
    failures: list[str] = []
    if value.get("bomFormat") != "CycloneDX":
        failures.append("bomFormat must be CycloneDX")
    if value.get("specVersion") != SPEC_VERSION:
        failures.append(f"specVersion must be {SPEC_VERSION}")
    metadata = value.get("metadata")
    if not isinstance(metadata, dict):
        failures.append("metadata must be an object")
        metadata = {}
    root = metadata.get("component")
    if not isinstance(root, dict) or root.get("name") != "rs-ai":
        failures.append("metadata.component must identify root crate rs-ai")
    else:
        props = root.get("properties") or []
        if not any(prop.get("name") == "rs-ai:vcs:revision" and prop.get("value") for prop in props if isinstance(prop, dict)):
            failures.append("root component must include rs-ai:vcs:revision")
    components = value.get("components")
    if not isinstance(components, list) or not components:
        failures.append("components must be a non-empty list")
        components = []
    for index, component in enumerate(components):
        if not isinstance(component, dict):
            failures.append(f"component[{index}] must be an object")
            continue
        for key in ("type", "bom-ref", "name", "version", "purl"):
            if not component.get(key):
                failures.append(f"component[{index}] missing {key}")
        blob = json.dumps(component, sort_keys=True)
        if str(ROOT) in blob or "/workspace/" in blob or "file://" in blob:
            failures.append(f"component[{index}] leaks local path data")
    dependencies = value.get("dependencies")
    if not isinstance(dependencies, list) or not dependencies:
        failures.append("dependencies must be a non-empty list")
    tools = (((metadata.get("tools") or {}).get("components") or []) if isinstance(metadata.get("tools"), dict) else [])
    if not any(tool.get("name") == GENERATOR_NAME and tool.get("version") == GENERATOR_VERSION for tool in tools if isinstance(tool, dict)):
        failures.append("metadata.tools must identify the pinned SBOM generator")
    if checksum_text is not None and raw is not None:
        expected = checksum_text.strip().split()[0] if checksum_text.strip() else ""
        actual = sha256_bytes(raw)
        if expected != actual:
            failures.append(f"checksum mismatch: expected {expected}, got {actual}")
    return failures


def write_artifacts(sbom_path: Path, checksum_path: Path) -> None:
    sbom = build_sbom()
    raw = canonical_json_bytes(sbom)
    failures = validate_sbom(sbom)
    if failures:
        raise SystemExit("\n".join(failures))
    sbom_path.parent.mkdir(parents=True, exist_ok=True)
    sbom_path.write_bytes(raw)
    digest = sha256_bytes(raw)
    checksum_path.write_text(f"{digest}  {sbom_path.name}\n")
    print(f"generated {sbom_path} ({len(sbom['components'])} dependency components)")
    print(f"sha256 {digest}")


def check_artifacts(sbom_path: Path, checksum_path: Path) -> None:
    if not sbom_path.exists():
        raise SystemExit(f"SBOM missing: {sbom_path}; run `make sbom`")
    if not checksum_path.exists():
        raise SystemExit(f"SBOM checksum missing: {checksum_path}; run `make sbom`")
    raw = sbom_path.read_bytes()
    if not raw.strip():
        raise SystemExit(f"SBOM is empty: {sbom_path}")
    try:
        current = json.loads(raw)
    except json.JSONDecodeError as exc:
        raise SystemExit(f"SBOM is malformed JSON: {exc}") from exc
    failures = validate_sbom(current, checksum_path.read_text(), raw)
    if failures:
        raise SystemExit("\n".join(failures))

    expected = canonical_json_bytes(build_sbom())
    if raw != expected:
        with tempfile.NamedTemporaryFile("wb", prefix="rs-ai-sbom-expected-", suffix=".json", delete=False) as fh:
            fh.write(expected)
            expected_path = fh.name
        raise SystemExit(f"SBOM is stale; run `make sbom` (expected snapshot: {expected_path})")
    expected_sha = sha256_bytes(expected)
    got_sha = checksum_path.read_text().strip().split()[0]
    if got_sha != expected_sha:
        raise SystemExit(f"SBOM checksum is stale; expected {expected_sha}, got {got_sha}")
    print(f"validated {sbom_path} ({len(current['components'])} dependency components)")
    print(f"sha256 {expected_sha}")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("action", choices=["generate", "check"])
    parser.add_argument("--output", type=Path, default=DEFAULT_SBOM)
    parser.add_argument("--checksum", type=Path, default=DEFAULT_SHA)
    args = parser.parse_args()
    if args.action == "generate":
        write_artifacts(args.output, args.checksum)
    else:
        check_artifacts(args.output, args.checksum)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
