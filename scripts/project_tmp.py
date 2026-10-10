#!/usr/bin/env python3
"""Portable project-owned scratch resolver for rs-ai Python tools."""
from __future__ import annotations

import os
import tempfile
from pathlib import Path

PROJECT = "rs-ai"
ORIGINAL_TMPDIR = os.environ.get("PROJECT_ORIGINAL_TMPDIR", os.environ.get("TMPDIR", ""))
os.environ.setdefault("PROJECT_ORIGINAL_TMPDIR", ORIGINAL_TMPDIR)


def _is_ci() -> bool:
    value = os.environ.get("CI", "")
    if value not in {"", "0", "false", "FALSE"}:
        return True
    return any(os.environ.get(name, "").lower() == "true" for name in ("GITHUB_ACTIONS", "GITLAB_CI", "TF_BUILD", "CIRCLECI"))


def _usable(path: Path) -> bool:
    if not path.is_absolute() or any(part in {".", ".."} for part in path.parts):
        return False
    candidate = path
    while True:
        if candidate.is_symlink() and candidate != Path("/workspace"):
            return False
        if candidate == candidate.parent:
            break
        candidate = candidate.parent
    existing = path
    while not existing.exists() and existing != existing.parent:
        existing = existing.parent
    if path.exists() and (not path.is_dir() or (hasattr(os, "getuid") and path.stat().st_uid != os.getuid())):
        return False
    return existing.is_dir() and os.access(existing, os.W_OK | os.X_OK)


def _project_path(base: str | Path | None) -> Path | None:
    if not base:
        return None
    base_path = Path(base)
    if not base_path.is_absolute():
        return None
    candidate = base_path / PROJECT
    return candidate if _usable(candidate) else None


def resolve_project_tmp_root(
    workspace_base: Path = Path("/workspace/tmp"),
    platform_base: Path | None = None,
) -> Path:
    explicit_base = os.environ.get("PROJECT_TMP_BASE")
    explicit_root = os.environ.get("PROJECT_TMP_ROOT", os.environ.get("RS_AI_TMP_ROOT"))
    base_root: Path | None = None

    if explicit_base is not None:
        if not explicit_base or any(p in {".", ".."} for p in explicit_base.split("/")) or (base_root := _project_path(explicit_base)) is None:
            raise RuntimeError("PROJECT_TMP_BASE must be a usable absolute base")
    if explicit_root is not None:
        candidate = Path(explicit_root.rstrip("/"))
        if not explicit_root or any(p in {".", ".."} for p in explicit_root.split("/")) or candidate.name != PROJECT or not _usable(candidate):
            raise RuntimeError("PROJECT_TMP_ROOT must be a usable absolute project-named directory, not a symlink")
        if base_root is not None and candidate != base_root:
            raise RuntimeError("Conflicting PROJECT_TMP_BASE and PROJECT_TMP_ROOT")
        return candidate
    if base_root is not None:
        return base_root

    # Platform fallback must not re-read redirected TMPDIR or prefer home/CWD.
    platform = platform_base or (Path(os.environ.get("SystemRoot", r"C:\Windows")) / "Temp" if os.name == "nt" else Path("/tmp"))
    if _is_ci():
        bases: tuple[str | Path | None, ...] = (os.environ.get("RUNNER_TEMP"), ORIGINAL_TMPDIR, platform)
    else:
        bases = (workspace_base, platform)
    for base in bases:
        if (candidate := _project_path(base)) is not None:
            return candidate
    raise RuntimeError("No writable project-owned temporary root available")
