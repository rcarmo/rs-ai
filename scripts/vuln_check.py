#!/usr/bin/env python3
"""Pinned Rust vulnerability-scan wrapper with expiring exceptions.

Uses `cargo audit` at the pinned version below. Known advisories may be
accepted only with an owner, rationale, and expiry in `APPROVED_EXCEPTIONS`;
any new/unapproved high/critical advisory or warning fails the check.
"""
from __future__ import annotations

import json
import re
import shutil
import subprocess
import sys
from datetime import date
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PINNED_CARGO_AUDIT_VERSION = "0.22.2"

# Existing AWS SDK legacy HTTP/TLS stack pulled by aws-sdk-bedrockruntime.
# Owner: Rui Carmo <rui.carmo@gmail.com>
# Rationale: rs-ai needs Bedrock runtime coverage; current AWS Rust SDK still
# resolves legacy hyper-rustls/rustls-webpki/h2 via aws-smithy-http-client.
# This exception is temporary and must be revisited on the next dependency or
# release audit, or sooner if a patched AWS stack is available.
APPROVED_EXCEPTIONS = {
    ("RUSTSEC-2026-0258", "h2", "0.3.27"): {
        "owner": "Rui Carmo <rui.carmo@gmail.com>",
        "expires": "2026-09-30",
        "rationale": "AWS SDK legacy hyper 0.14 transport dependency; update AWS stack when patched transitives are available.",
    },
    ("RUSTSEC-2026-0099", "rustls-webpki", "0.101.7"): {
        "owner": "Rui Carmo <rui.carmo@gmail.com>",
        "expires": "2026-09-30",
        "rationale": "AWS SDK legacy rustls 0.21 dependency; update AWS stack when patched transitives are available.",
    },
    ("RUSTSEC-2026-0098", "rustls-webpki", "0.101.7"): {
        "owner": "Rui Carmo <rui.carmo@gmail.com>",
        "expires": "2026-09-30",
        "rationale": "AWS SDK legacy rustls 0.21 dependency; update AWS stack when patched transitives are available.",
    },
    ("RUSTSEC-2026-0104", "rustls-webpki", "0.101.7"): {
        "owner": "Rui Carmo <rui.carmo@gmail.com>",
        "expires": "2026-09-30",
        "rationale": "AWS SDK legacy rustls 0.21 dependency; update AWS stack when patched transitives are available.",
    },
}


def find_cargo_audit() -> str | None:
    exe = shutil.which("cargo-audit")
    if exe:
        return exe
    fallback = Path.home() / ".cargo/bin/cargo-audit"
    return str(fallback) if fallback.exists() else None


def ensure_version(exe: str) -> None:
    version = subprocess.check_output([exe, "--version"], text=True).strip()
    match = re.search(r"cargo-audit\s+([0-9]+\.[0-9]+\.[0-9]+)", version)
    got = match.group(1) if match else "unknown"
    if got != PINNED_CARGO_AUDIT_VERSION:
        raise SystemExit(
            f"cargo-audit version mismatch: got {got!r} from {version!r}, "
            f"expected {PINNED_CARGO_AUDIT_VERSION}"
        )


def exception_for(advisory: dict) -> dict | None:
    advisory_id = advisory["advisory"]["id"]
    package = advisory["package"]["name"]
    version = advisory["package"]["version"]
    item = APPROVED_EXCEPTIONS.get((advisory_id, package, version))
    if not item:
        return None
    expiry = date.fromisoformat(item["expires"])
    if expiry < date.today():
        raise SystemExit(f"expired vulnerability exception: {advisory_id} {package} {version} expired {expiry}")
    if not item.get("owner") or not item.get("rationale"):
        raise SystemExit(f"incomplete vulnerability exception: {advisory_id} {package} {version}")
    return item


def main() -> int:
    exe = find_cargo_audit()
    if not exe:
        print(
            "cargo-audit is required for vulnerability review. "
            f"Install the pinned scanner with: cargo install cargo-audit --version {PINNED_CARGO_AUDIT_VERSION} --locked",
            file=sys.stderr,
        )
        return 2
    ensure_version(exe)
    proc = subprocess.run([exe, "audit", "--json"], cwd=ROOT, text=True, capture_output=True)
    try:
        report = json.loads(proc.stdout)
    except json.JSONDecodeError as exc:
        print(proc.stdout, file=sys.stdout)
        print(proc.stderr, file=sys.stderr)
        raise SystemExit(f"cargo-audit did not produce JSON: {exc}") from exc

    failures: list[str] = []
    accepted: list[str] = []
    for advisory in report.get("vulnerabilities", {}).get("list", []):
        item = exception_for(advisory)
        label = f"{advisory['advisory']['id']} {advisory['package']['name']} {advisory['package']['version']}"
        if item:
            accepted.append(f"{label} (owner={item['owner']}, expires={item['expires']})")
        else:
            failures.append(f"unapproved vulnerability: {label} - {advisory['advisory'].get('title', '')}")
    for warning_kind, entries in (report.get("warnings") or {}).items():
        for advisory in entries:
            item = exception_for(advisory)
            label = f"{warning_kind}: {advisory['advisory']['id']} {advisory['package']['name']} {advisory['package']['version']}"
            if item:
                accepted.append(f"{label} (owner={item['owner']}, expires={item['expires']})")
            else:
                failures.append(f"unapproved warning: {label} - {advisory['advisory'].get('title', '')}")
    if failures:
        print("vulnerability review failed:", file=sys.stderr)
        print("\n".join(failures), file=sys.stderr)
        return 1
    for line in accepted:
        print(f"accepted temporary advisory exception: {line}")
    print(f"vulnerability scan passed with cargo-audit {PINNED_CARGO_AUDIT_VERSION}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
