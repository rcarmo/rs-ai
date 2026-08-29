#!/usr/bin/env python3
"""Self-tests for scripts/license_check.py fail-closed SPDX policy."""
from __future__ import annotations

import importlib.util
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("license_check", ROOT / "scripts/license_check.py")
assert SPEC and SPEC.loader
license_check = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = license_check
SPEC.loader.exec_module(license_check)


def assert_allowed(expression: str) -> None:
    allowed, reason = license_check.expression_is_allowed(expression)
    assert allowed, f"expected allowed for {expression!r}: {reason}"


def assert_denied(expression: str, needle: str = "") -> None:
    allowed, reason = license_check.expression_is_allowed(expression)
    assert not allowed, f"expected denied for {expression!r}"
    if needle:
        assert needle in reason, f"expected {needle!r} in {reason!r}"


def test_mit() -> None:
    assert_allowed("MIT")


def test_apache_or_mit() -> None:
    assert_allowed("Apache-2.0 OR MIT")


def test_mit_and_apache() -> None:
    assert_allowed("MIT AND Apache-2.0")


def test_mit_and_unknown_fails() -> None:
    assert_denied("MIT AND FooBar-1.0", "no approved selectable")


def test_mit_and_gpl_fails() -> None:
    assert_denied("MIT AND GPL-3.0-only", "no approved selectable")


def test_proprietary_licenseref_fails() -> None:
    assert_denied("LicenseRef-Proprietary-Foo", "no approved selectable")
    assert_denied("MIT AND LicenseRef-Proprietary-Foo", "no approved selectable")


def test_or_with_bad_branch_can_pass_on_good_branch() -> None:
    assert_allowed("MIT OR LicenseRef-Proprietary-Foo")


def test_legacy_slash_separator_is_or() -> None:
    assert_allowed("MIT/Apache-2.0")
    assert_allowed("Apache-2.0 / MIT")


def test_malformed_and_missing_fail() -> None:
    assert_denied("", "missing")
    assert_denied("MIT AND", "malformed")
    assert_denied("(MIT OR Apache-2.0", "malformed")


def main() -> int:
    tests = [
        test_mit,
        test_apache_or_mit,
        test_mit_and_apache,
        test_mit_and_unknown_fails,
        test_mit_and_gpl_fails,
        test_proprietary_licenseref_fails,
        test_or_with_bad_branch_can_pass_on_good_branch,
        test_legacy_slash_separator_is_or,
        test_malformed_and_missing_fail,
    ]
    for test in tests:
        test()
        print(f"ok {test.__name__}")
    print(f"license_check self-tests passed: {len(tests)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
