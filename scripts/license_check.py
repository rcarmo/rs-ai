#!/usr/bin/env python3
"""Fail-closed Cargo dependency license review.

This is a small SPDX-expression subset evaluator for Cargo metadata license
strings. It rejects unknown/proprietary license identifiers, malformed
expressions, and any mandatory (`AND`) branch that cannot be satisfied by the
approved allowlist. `OR` expressions pass only when at least one selectable
branch is fully approved.
"""
from __future__ import annotations

import json
import re
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable

ROOT = Path(__file__).resolve().parents[1]
APPROVED = {
    "Apache-2.0",
    "BSD-2-Clause",
    "BSD-3-Clause",
    "BSL-1.0",
    "CC0-1.0",
    "CDLA-Permissive-2.0",
    "ISC",
    "MIT",
    "MIT-0",
    "Unicode-3.0",
    "Unlicense",
    "Zlib",
}
DENIED_PREFIXES = ("AGPL", "GPL", "LGPL", "MPL", "CDDL", "EPL")
TOKEN_RE = re.compile(r"\s*(AND|OR|WITH|\(|\)|[A-Za-z0-9][A-Za-z0-9.+-]*(?::[A-Za-z0-9][A-Za-z0-9.+-]*)?)")


@dataclass(frozen=True)
class Node:
    kind: str
    value: str | None = None
    left: "Node | None" = None
    right: "Node | None" = None


class LicenseParseError(ValueError):
    pass


class Parser:
    def __init__(self, expression: str):
        self.tokens = self._tokenize(expression)
        self.index = 0

    @staticmethod
    def _tokenize(expression: str) -> list[str]:
        # crates.io historically contains legacy separators like `MIT/Apache-2.0`
        # or `Apache-2.0 / MIT`; interpret `/` as an OR separator before parsing.
        expression = re.sub(r"\s*/\s*", " OR ", expression)
        tokens: list[str] = []
        pos = 0
        while pos < len(expression):
            match = TOKEN_RE.match(expression, pos)
            if not match:
                raise LicenseParseError(f"unexpected character at offset {pos}: {expression[pos:pos+16]!r}")
            token = match.group(1)
            tokens.append(token)
            pos = match.end()
        if not tokens:
            raise LicenseParseError("empty license expression")
        return tokens

    def peek(self) -> str | None:
        return self.tokens[self.index] if self.index < len(self.tokens) else None

    def consume(self) -> str:
        token = self.peek()
        if token is None:
            raise LicenseParseError("unexpected end of license expression")
        self.index += 1
        return token

    def parse(self) -> Node:
        node = self.parse_or()
        if self.peek() is not None:
            raise LicenseParseError(f"unexpected token {self.peek()!r}")
        return node

    def parse_or(self) -> Node:
        node = self.parse_and()
        while self.peek() == "OR":
            self.consume()
            node = Node("OR", left=node, right=self.parse_and())
        return node

    def parse_and(self) -> Node:
        node = self.parse_factor()
        while self.peek() == "AND":
            self.consume()
            node = Node("AND", left=node, right=self.parse_factor())
        return node

    def parse_factor(self) -> Node:
        token = self.consume()
        if token == "(":
            node = self.parse_or()
            if self.consume() != ")":
                raise LicenseParseError("missing closing parenthesis")
            return node
        if token in {"AND", "OR", "WITH", ")"}:
            raise LicenseParseError(f"unexpected token {token!r}")
        node = Node("LICENSE", value=token)
        if self.peek() == "WITH":
            self.consume()
            exception = self.consume()
            if exception in {"AND", "OR", "WITH", "(", ")"}:
                raise LicenseParseError("WITH must be followed by an exception identifier")
            node = Node("WITH", left=node, right=Node("EXCEPTION", value=exception))
        return node


def is_denied_identifier(identifier: str) -> bool:
    return identifier.startswith("LicenseRef-") or identifier.startswith("DocumentRef-") or identifier.startswith("Proprietary") or identifier.startswith(DENIED_PREFIXES)


def satisfiable(node: Node) -> bool:
    if node.kind == "LICENSE":
        assert node.value is not None
        if is_denied_identifier(node.value):
            return False
        return node.value in APPROVED
    if node.kind == "EXCEPTION":
        return True
    if node.kind == "WITH":
        return bool(node.left and node.right and satisfiable(node.left) and satisfiable(node.right))
    if node.kind == "AND":
        return bool(node.left and node.right and satisfiable(node.left) and satisfiable(node.right))
    if node.kind == "OR":
        return bool(node.left and node.right and (satisfiable(node.left) or satisfiable(node.right)))
    raise LicenseParseError(f"unknown AST node {node.kind!r}")


def expression_is_allowed(expression: str) -> tuple[bool, str]:
    if not expression or not expression.strip():
        return False, "missing license expression"
    try:
        node = Parser(expression).parse()
    except LicenseParseError as exc:
        return False, f"malformed license expression: {exc}"
    if not satisfiable(node):
        return False, f"no approved selectable SPDX branch in {expression!r}"
    return True, "ok"


def metadata() -> dict:
    out = subprocess.check_output(
        ["cargo", "metadata", "--locked", "--all-features", "--format-version", "1"],
        cwd=ROOT,
        text=True,
        stderr=subprocess.STDOUT,
    )
    return json.loads(out)


def check_packages(packages: Iterable[dict]) -> tuple[int, list[str]]:
    failures: list[str] = []
    checked = 0
    for package in sorted(packages, key=lambda p: (p["name"], p["version"])):
        if package.get("source") is None:
            continue
        checked += 1
        allowed, reason = expression_is_allowed(package.get("license") or "")
        if not allowed:
            failures.append(f"{package['name']} {package['version']}: {reason}")
    return checked, failures


def main() -> int:
    checked, failures = check_packages(metadata()["packages"])
    if failures:
        print("license review failed:", file=sys.stderr)
        print("\n".join(failures), file=sys.stderr)
        return 1
    print(f"license review passed: {checked} third-party packages; approved tokens={','.join(sorted(APPROVED))}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
