"""Role based cone-mode sparse checkout profiles for disposable worktrees."""

from __future__ import annotations

import subprocess
from pathlib import Path

from tools.data_repo_paths import classify_repo_path
from tools.wt.safety import Unsafe

CODE_PATHS = (
    "tools",
    "tests",
    "crates",
    "cmake",
    "src",
    "generated",
    "third_party",  # vendored lifter: a prebuilt build relifts when the lift key moves (I-T3785-1)
    "docs/data",  # gate profiles and registered inputs are required by code-only gates
    "docs/tasks",
    "docs/decisions",
    "docs/run-notes",
)
FULL_EXTRA = ("docs/data", "docs/evidence")


def _git(root: Path, *args: str) -> str:
    result = subprocess.run(
        ["git", "-C", str(root), *args], capture_output=True, text=True, timeout=60
    )
    if result.returncode:
        raise Unsafe(result.stderr.strip() or f"git {' '.join(args)} failed")
    return result.stdout


def init(root: Path, profile: str = "code") -> None:
    if profile not in {"code", "full"}:
        raise Unsafe("sparse profile must be 'code' or 'full'")
    _git(root, "sparse-checkout", "init", "--cone")
    paths = [*CODE_PATHS]
    # Tracked configuration directories also contain test inputs (for example skills).
    # Derive these from the candidate tree so new directories are not silently omitted.
    paths.extend(
        name
        for name in _git(root, "ls-tree", "-d", "--name-only", "HEAD").splitlines()
        if name.startswith(".")
    )
    doc_dirs = _git(root, "ls-tree", "-d", "--name-only", "HEAD:docs").splitlines()
    paths.extend(
        f"docs/{name}" for name in doc_dirs if classify_repo_path(f"docs/{name}") == "main"
    )
    if profile == "full":
        paths.extend(FULL_EXTRA)
    _git(root, "sparse-checkout", "set", "--cone", *paths)


def add(root: Path, path: str) -> None:
    clean = Path(path)
    if clean.is_absolute() or ".." in clean.parts or not path.strip("/"):
        raise Unsafe("sparse path must be a repository-relative path")
    _git(root, "sparse-checkout", "add", clean.as_posix().strip("/"))
