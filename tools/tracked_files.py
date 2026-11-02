# SPDX-License-Identifier: GPL-3.0-or-later
"""Read tracked inputs that may be omitted by a sparse worktree."""

from __future__ import annotations

import subprocess
from pathlib import Path


def read_tracked_bytes(root: Path, relative: str | Path) -> bytes:
    """Read a worktree file, falling back to its committed HEAD blob when sparse-omitted."""
    path = root / relative
    try:
        return path.read_bytes()
    except FileNotFoundError:
        git_path = Path(relative).as_posix()
        try:
            return subprocess.run(
                ["git", "show", f"HEAD:{git_path}"],
                cwd=root,
                check=True,
                capture_output=True,
            ).stdout
        except (OSError, subprocess.CalledProcessError) as exc:
            raise FileNotFoundError(
                f"required tracked file is missing from the worktree and Git HEAD: {path}"
            ) from exc


def read_tracked_text(root: Path, relative: str | Path) -> str:
    """Read a tracked UTF-8 text input, including from a sparse checkout."""
    return read_tracked_bytes(root, relative).decode()


def read_optional_tracked_bytes(root: Path, relative: str | Path) -> bytes | None:
    """Read an optional tracked file, returning None only when it is unavailable."""
    try:
        return read_tracked_bytes(root, relative)
    except FileNotFoundError:
        return None


def read_optional_path_bytes(path: Path | None) -> bytes | None:
    """Read an optional path, including its Git blob when a sparse checkout omits it."""
    if path is None:
        return None
    try:
        return path.read_bytes()
    except FileNotFoundError:
        cwd = path.parent
        while not cwd.is_dir() and cwd != cwd.parent:
            cwd = cwd.parent
        try:
            result = subprocess.run(
                ["git", "rev-parse", "--show-toplevel"],
                cwd=cwd,
                check=True,
                capture_output=True,
                text=True,
            )
            root = Path(result.stdout.strip())
            relative = path.resolve(strict=False).relative_to(root)
        except (OSError, subprocess.CalledProcessError, ValueError):
            return None
        return read_optional_tracked_bytes(root, relative)


def read_optional_tracked_text(root: Path, relative: str | Path) -> str | None:
    """Read an optional tracked file, returning None only when it is unavailable."""
    content = read_optional_tracked_bytes(root, relative)
    return content.decode() if content is not None else None
