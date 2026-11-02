"""Canonical main-repository path classification for T3676/T3677.

This module is intentionally small and dependency-free so data-repo inventory and sparse
worktree planning use the same definitions.
"""

from __future__ import annotations

import re
from pathlib import PurePosixPath

DATA_ROOTS = {
    "data": "docs/data",
    "evidence": "docs/evidence",
}
_SEPARATOR = re.compile(r"[\\/]+")


def normalize_repo_path(path: str | PurePosixPath) -> str:
    """Normalize a repository-relative path for stable classification."""
    raw = str(path).replace("\\", "/").strip()
    while raw.startswith("./"):
        raw = raw[2:]
    return "/".join(part for part in PurePosixPath(raw).parts if part not in (".", ""))


def classify_repo_path(path: str | PurePosixPath) -> str:
    """Return ``data``, ``evidence``, or ``main`` for a repository-relative path.

    The roots themselves and descendants belong to their data class. Similar names such as
    ``docs/database`` stay in main. Both POSIX and Windows separators are accepted.
    """
    normalized = normalize_repo_path(path)
    for name, root in DATA_ROOTS.items():
        if normalized == root or normalized.startswith(root + "/"):
            return name
    # Accept an absolute path that includes a recognizable repository suffix while ensuring
    # a parent directory named "data" cannot classify an unrelated path.
    components = [part for part in _SEPARATOR.split(normalized) if part]
    for name, root in DATA_ROOTS.items():
        root_parts = root.split("/")
        for index in range(len(components) - len(root_parts) + 1):
            if components[index : index + len(root_parts)] == root_parts:
                return name
    return "main"
