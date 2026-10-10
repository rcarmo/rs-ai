#!/usr/bin/env python3
"""Project-scoped cache, build and disposable-run paths for rs-ai tools."""
from __future__ import annotations

import os
import sys
import tempfile
import uuid
from datetime import datetime, timezone
from pathlib import Path
from typing import IO, Any

from project_tmp import resolve_project_tmp_root

# Direct script invocation must not leave Python bytecode in the source tree.
sys.dont_write_bytecode = True


def _safe_segment(value: str, label: str) -> str:
    if not value or value in {".", ".."} or any(ch not in "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789._-" for ch in value):
        raise RuntimeError(f"invalid {label}: {value!r}")
    return value


def _owned_directory(path: Path) -> Path:
    path = path.absolute()
    boundary = project_tmp_root().absolute()
    if path != boundary:
        path.relative_to(boundary)
    candidates = [path]
    while candidates[-1] != boundary:
        candidates.append(candidates[-1].parent)
    for candidate in candidates:
        if candidate.exists() and (candidate.is_symlink() or not candidate.is_dir()):
            raise RuntimeError(f"scratch path component is not a real directory: {candidate}")
    path.mkdir(parents=True, exist_ok=True)
    if path.is_symlink() or not path.is_dir():
        raise RuntimeError(f"scratch path is not a real directory: {path}")
    if hasattr(os, "getuid") and path.stat().st_uid != os.getuid():
        raise RuntimeError(f"scratch path is not owned by this user: {path}")
    return path


def project_tmp_root() -> Path:
    return resolve_project_tmp_root()


def configure_process_env(purpose: str) -> Path:
    """Set cache/build/temp variables and return one isolated run directory."""
    purpose = _safe_segment(purpose, "run purpose")
    root = _owned_directory(project_tmp_root())
    cache = _owned_directory(root / "cache")
    build = _owned_directory(root / "build")
    runs = _owned_directory(root / "runs")

    supplied_run = os.environ.get("RS_AI_RUN_DIR")
    if supplied_run:
        run = Path(supplied_run).absolute()
        try:
            relative = run.relative_to(runs.absolute())
        except ValueError as exc:
            raise RuntimeError(f"RS_AI_RUN_DIR must be under {runs}: {run}") from exc
        if len(relative.parts) != 2 or any(_safe_segment(part, "run path segment") != part for part in relative.parts):
            raise RuntimeError(f"RS_AI_RUN_DIR must be runs/<purpose>/<run-id>: {run}")
    else:
        stamp = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%S%fZ")
        run = runs / purpose / f"{stamp}-{os.getpid()}-{uuid.uuid4().hex[:8]}"
    run = _owned_directory(run)
    tmp = _owned_directory(run / "tmp")

    defaults = {
        "CARGO_HOME": cache / "cargo",
        "CARGO_TARGET_DIR": build / "cargo-target",
        "PYTHONPYCACHEPREFIX": cache / "python" / "pycache",
        "npm_config_cache": cache / "npm",
        "BUN_INSTALL_CACHE_DIR": cache / "bun",
        "XDG_CACHE_HOME": cache / "xdg",
        "TMPDIR": tmp,
        "TMP": tmp,
        "TEMP": tmp,
    }
    for name, value in defaults.items():
        configured = Path(value)
        os.environ[name] = str(configured)
        if name != "CARGO_TARGET_DIR":
            _owned_directory(configured)
    os.environ["RS_AI_TMP_ROOT"] = str(root)
    os.environ["RS_AI_RUN_DIR"] = str(run)
    return run


def temporary_directory(purpose: str, *, prefix: str) -> tempfile.TemporaryDirectory[str]:
    run = configure_process_env(purpose)
    return tempfile.TemporaryDirectory(prefix=prefix, dir=run)


def named_temporary_file(purpose: str, *args: Any, **kwargs: Any) -> IO[Any]:
    run = configure_process_env(purpose)
    kwargs["dir"] = run
    return tempfile.NamedTemporaryFile(*args, **kwargs)
