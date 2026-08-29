#!/usr/bin/env python3
"""Pinned Rust vulnerability-scan wrapper with expiring exceptions.

Uses `cargo audit` at the pinned version below. Known advisories may be
accepted only with an owner, rationale, and expiry in `APPROVED_EXCEPTIONS`;
any new/unapproved advisory or warning fails the check. Scanner/runtime errors
fail closed even if a partial JSON report is emitted.
"""
from __future__ import annotations

import json
import re
import shutil
import subprocess
import sys
from datetime import date
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
PINNED_CARGO_AUDIT_VERSION = "0.22.2"
# cargo-audit returns 0 for no findings and 1 for reported vulnerabilities /
# denied warnings. Any other status is an execution/runtime/database failure.
EXPECTED_AUDIT_RETURN_CODES = {0, 1}

ExceptionKey = tuple[str, str, str]
ExceptionPolicy = dict[ExceptionKey, dict[str, str]]

# Existing AWS SDK legacy HTTP/TLS stack pulled by aws-sdk-bedrockruntime.
# Owner: Rui Carmo <rui.carmo@gmail.com>
# Rationale: rs-ai needs Bedrock runtime coverage; current AWS Rust SDK still
# resolves legacy hyper-rustls/rustls-webpki/h2 via aws-smithy-http-client.
# This exception is temporary and must be revisited on the next dependency or
# release audit, or sooner if a patched AWS stack is available.
APPROVED_EXCEPTIONS: ExceptionPolicy = {
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


def finding_key(finding: dict[str, Any]) -> ExceptionKey:
    try:
        return (
            str(finding["advisory"]["id"]),
            str(finding["package"]["name"]),
            str(finding["package"]["version"]),
        )
    except (KeyError, TypeError) as exc:
        raise ValueError(f"incomplete advisory finding: {finding!r}") from exc


def validate_exception(
    finding: dict[str, Any],
    exceptions: ExceptionPolicy = APPROVED_EXCEPTIONS,
    today: date | None = None,
) -> dict[str, str] | None:
    advisory_id, package, version = finding_key(finding)
    item = exceptions.get((advisory_id, package, version))
    if not item:
        return None
    today = today or date.today()
    expiry_text = item.get("expires", "")
    try:
        expiry = date.fromisoformat(expiry_text)
    except ValueError as exc:
        raise ValueError(f"invalid vulnerability exception expiry: {advisory_id} {package} {version}: {expiry_text!r}") from exc
    if expiry < today:
        raise ValueError(f"expired vulnerability exception: {advisory_id} {package} {version} expired {expiry}")
    if not item.get("owner") or not item.get("rationale"):
        raise ValueError(f"incomplete vulnerability exception: {advisory_id} {package} {version}")
    return item


def validate_report_shape(report: Any) -> tuple[list[dict[str, Any]], list[tuple[str, dict[str, Any]]]]:
    if not isinstance(report, dict):
        raise ValueError("cargo-audit report must be a JSON object")
    vulnerabilities = report.get("vulnerabilities")
    warnings = report.get("warnings")
    if not isinstance(vulnerabilities, dict) or not isinstance(vulnerabilities.get("list"), list):
        raise ValueError("cargo-audit report missing vulnerabilities.list")
    if warnings is None:
        warnings = {}
    if not isinstance(warnings, dict):
        raise ValueError("cargo-audit report warnings must be an object")
    vuln_list = vulnerabilities["list"]
    warning_list: list[tuple[str, dict[str, Any]]] = []
    for warning_kind, entries in warnings.items():
        if not isinstance(entries, list):
            raise ValueError(f"cargo-audit warning bucket {warning_kind!r} must be a list")
        for entry in entries:
            if not isinstance(entry, dict):
                raise ValueError(f"cargo-audit warning entry in {warning_kind!r} must be an object")
            # Validate required shape now so incomplete reports cannot pass as clean.
            finding_key(entry)
            warning_list.append((str(warning_kind), entry))
    for entry in vuln_list:
        if not isinstance(entry, dict):
            raise ValueError("cargo-audit vulnerability entry must be an object")
        finding_key(entry)
    return vuln_list, warning_list


def review_report(
    report: Any,
    exceptions: ExceptionPolicy = APPROVED_EXCEPTIONS,
    today: date | None = None,
) -> tuple[list[str], list[str], int]:
    vulnerabilities, warnings = validate_report_shape(report)
    failures: list[str] = []
    accepted: list[str] = []
    for finding in vulnerabilities:
        advisory_id, package, version = finding_key(finding)
        item = validate_exception(finding, exceptions, today)
        label = f"{advisory_id} {package} {version}"
        if item:
            accepted.append(f"{label} (owner={item['owner']}, expires={item['expires']})")
        else:
            failures.append(f"unapproved vulnerability: {label} - {finding['advisory'].get('title', '')}")
    for warning_kind, finding in warnings:
        advisory_id, package, version = finding_key(finding)
        item = validate_exception(finding, exceptions, today)
        label = f"{warning_kind}: {advisory_id} {package} {version}"
        if item:
            accepted.append(f"{label} (owner={item['owner']}, expires={item['expires']})")
        else:
            failures.append(f"unapproved warning: {label} - {finding['advisory'].get('title', '')}")
    return failures, accepted, len(vulnerabilities) + len(warnings)


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
        if proc.stdout:
            print(proc.stdout, file=sys.stdout)
        if proc.stderr:
            print(proc.stderr, file=sys.stderr)
        print(f"cargo-audit did not produce JSON: {exc}", file=sys.stderr)
        return 2
    if proc.returncode not in EXPECTED_AUDIT_RETURN_CODES:
        if proc.stderr:
            print(proc.stderr, file=sys.stderr)
        print(f"cargo-audit failed with unexpected exit status {proc.returncode}", file=sys.stderr)
        return 2
    try:
        failures, accepted, finding_count = review_report(report)
    except ValueError as exc:
        print(f"invalid cargo-audit report: {exc}", file=sys.stderr)
        return 2
    if proc.returncode == 0 and finding_count:
        print("cargo-audit returned success but reported findings", file=sys.stderr)
        return 2
    if proc.returncode == 1 and not finding_count:
        print("cargo-audit returned findings status but report is empty", file=sys.stderr)
        return 2
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
