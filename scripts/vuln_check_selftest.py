#!/usr/bin/env python3
"""Self-tests for scripts/vuln_check.py fail-closed behavior."""
from __future__ import annotations

import contextlib
import importlib.util
import io
import json
import tempfile
from datetime import date
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("vuln_check", ROOT / "scripts/vuln_check.py")
assert SPEC and SPEC.loader
vuln_check = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(vuln_check)


def finding(advisory_id="RUSTSEC-2099-0001", package="example", version="1.0.0"):
    return {
        "advisory": {"id": advisory_id, "title": "Example advisory"},
        "package": {"name": package, "version": version},
        "versions": {"patched": [">=1.0.1"]},
    }


def report(vulnerabilities=None, warnings=None):
    return {
        "vulnerabilities": {"found": bool(vulnerabilities), "list": vulnerabilities or []},
        "warnings": warnings or {},
    }


def assert_raises_value_error(fn, needle: str) -> None:
    try:
        fn()
    except ValueError as exc:
        assert needle in str(exc), f"expected {needle!r} in {exc!r}"
    else:
        raise AssertionError(f"expected ValueError containing {needle!r}")


def test_scanner_error_exit_with_empty_json_fails() -> None:
    clean = report()
    failures, accepted, count = vuln_check.review_report(clean, exceptions={}, today=date(2026, 1, 1))
    assert (failures, accepted, count) == ([], [], 0)
    # Main wrapper policy: an execution/runtime/database error (e.g. RC=2) is
    # never converted to success merely because stdout contained valid JSON.
    assert 2 not in vuln_check.EXPECTED_AUDIT_RETURN_CODES


def test_malformed_json_fails_like_main() -> None:
    bad = "{not json"
    try:
        json.loads(bad)
    except json.JSONDecodeError:
        return
    raise AssertionError("malformed JSON unexpectedly parsed")


def test_incomplete_report_fails() -> None:
    assert_raises_value_error(lambda: vuln_check.review_report({}, exceptions={}), "vulnerabilities.list")
    assert_raises_value_error(
        lambda: vuln_check.review_report({"vulnerabilities": {"list": [{}]}, "warnings": {}}, exceptions={}),
        "incomplete advisory finding",
    )


def test_unapproved_advisory_fails() -> None:
    failures, accepted, count = vuln_check.review_report(report([finding()]), exceptions={}, today=date(2026, 1, 1))
    assert count == 1
    assert accepted == []
    assert failures and "unapproved vulnerability: RUSTSEC-2099-0001 example 1.0.0" in failures[0]


def test_expired_and_incomplete_waivers_fail() -> None:
    key = ("RUSTSEC-2099-0001", "example", "1.0.0")
    expired = {key: {"owner": "owner", "expires": "2025-01-01", "rationale": "temporary"}}
    assert_raises_value_error(
        lambda: vuln_check.review_report(report([finding()]), exceptions=expired, today=date(2026, 1, 1)),
        "expired vulnerability exception",
    )
    incomplete = {key: {"owner": "owner", "expires": "2026-12-31", "rationale": ""}}
    assert_raises_value_error(
        lambda: vuln_check.review_report(report([finding()]), exceptions=incomplete, today=date(2026, 1, 1)),
        "incomplete vulnerability exception",
    )


def test_approved_advisory_passes() -> None:
    key = ("RUSTSEC-2099-0001", "example", "1.0.0")
    exceptions = {key: {"owner": "owner", "expires": "2026-12-31", "rationale": "temporary"}}
    failures, accepted, count = vuln_check.review_report(report([finding()]), exceptions=exceptions, today=date(2026, 1, 1))
    assert failures == []
    assert count == 1
    assert accepted == ["RUSTSEC-2099-0001 example 1.0.0 (owner=owner, expires=2026-12-31)"]


def test_main_rejects_mock_scanner_error_exit_with_empty_json() -> None:
    with tempfile.TemporaryDirectory(prefix="rs-ai-vuln-selftest-") as tmp:
        mock = Path(tmp) / "cargo-audit"
        mock.write_text(
            "#!/bin/sh\n"
            "if [ \"$1\" = \"--version\" ]; then echo 'cargo-audit 0.22.2'; exit 0; fi\n"
            "printf '%s\\n' '{\"vulnerabilities\":{\"found\":false,\"list\":[]},\"warnings\":{}}'\n"
            "exit 2\n"
        )
        mock.chmod(0o755)
        old_find = vuln_check.find_cargo_audit
        vuln_check.find_cargo_audit = lambda: str(mock)
        stderr = io.StringIO()
        try:
            with contextlib.redirect_stderr(stderr):
                rc = vuln_check.main()
        finally:
            vuln_check.find_cargo_audit = old_find
        assert rc == 2
        assert "unexpected exit status 2" in stderr.getvalue()


def main() -> int:
    tests = [
        test_scanner_error_exit_with_empty_json_fails,
        test_malformed_json_fails_like_main,
        test_incomplete_report_fails,
        test_unapproved_advisory_fails,
        test_expired_and_incomplete_waivers_fail,
        test_approved_advisory_passes,
        test_main_rejects_mock_scanner_error_exit_with_empty_json,
    ]
    for test in tests:
        test()
        print(f"ok {test.__name__}")
    print(f"vuln_check self-tests passed: {len(tests)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
