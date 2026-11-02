"""Resolve owner-provided run inputs from a worktree or its shared main checkout.

Owner files stay in the main checkout's ignored ``tmp/owner`` tree. A linked worker
worktree may not have the symlinks installed, so tools must use this resolver instead
of assuming that relative paths exist in their own checkout.
"""

from __future__ import annotations

import argparse
import os
import subprocess
import sys
from collections.abc import Iterable, Mapping
from pathlib import Path

DISC_NAME = "TimeSplitters - Future Perfect (USA) (XBOX).iso"
FILES_DIR = Path("tmp/Xbox-Emulator-Files")
#: Conventional checkout-relative disc image locations, in lookup order (T715 layout, then the
#: `tmp/owner/` inputs directory of T2419). One definition for every caller.
DISC_LOCATIONS = (Path("tmp") / DISC_NAME, Path("tmp/owner") / DISC_NAME)


def _checkouts(root: Path) -> tuple[Path, Path]:
    """Return worktree and common-dir checkouts; tolerate non-git test roots."""
    start = Path(root).expanduser().resolve()
    git_root = next(
        (candidate for candidate in (start, *start.parents) if (candidate / ".git").exists()), None
    )
    if git_root is None:
        from tools.wt import source_identity

        exec_tree = source_identity.exec_root(start)
        if exec_tree is not None:  # T3760: a remote exec tree, its inputs live in TSFP_MAIN_ROOT
            return exec_tree, (source_identity.main_override() or exec_tree).resolve()
        parts = start.parts
        for index in range(len(parts) - 2):
            if parts[index : index + 2] == ("tmp", "wt") and index > 0:
                return start, Path(*parts[:index])
        return start, start
    result = subprocess.run(
        ["git", "rev-parse", "--show-toplevel", "--git-common-dir"],
        cwd=git_root,
        check=False,
        capture_output=True,
        text=True,
        timeout=10,
        close_fds=not os.environ.get("DMTCP_VIRTUAL_PID"),
    )
    lines = result.stdout.splitlines()
    if result.returncode or len(lines) < 2:
        return start, git_root
    top, common = lines[:2]
    top_path = Path(top).resolve()
    common_path = Path(common)
    if not common_path.is_absolute():
        common_path = top_path / common_path
    return top_path, common_path.resolve().parent


def main_checkout(root: Path) -> Path:
    """Return the primary checkout containing the git common directory for root."""
    return _checkouts(root)[1]


def candidates(
    root: Path,
    path: str | Path,
    *,
    extra: Iterable[Path] = (),
) -> tuple[Path, ...]:
    """Candidate paths in explicit, worktree, then git-common checkout order."""
    selected = Path(path).expanduser()
    if selected.is_absolute():
        # T3740: a stamped absolute path from another machine or checkout is stale; the
        # caller's fallbacks (e.g. <rec dir>/<name>.saves) still apply after it.
        ordered = [selected, *extra]
    else:
        primary, main = _checkouts(root)
        ordered = [*extra, primary / selected, main / selected]
    unique: list[Path] = []
    seen: set[Path] = set()
    for item in ordered:
        resolved = item.expanduser().resolve()
        if resolved not in seen:
            unique.append(resolved)
            seen.add(resolved)
    return tuple(unique)


def resolve_path(
    root: Path,
    path: str | Path,
    *,
    extra: Iterable[Path] = (),
    kind: str = "any",
) -> Path:
    """Resolve an input path, preferring existing worktree data before main-checkout data.

    When no candidate exists, return the worktree location so callers can report the
    expected path without accidentally creating files in the main checkout.
    """
    choices = candidates(root, path, extra=extra)
    predicate = {"any": Path.exists, "file": Path.is_file, "dir": Path.is_dir}[kind]
    for candidate in choices:
        if predicate(candidate):
            return candidate
    return choices[0]


def resolve_first(
    root: Path,
    paths: Iterable[str | Path],
    *,
    kind: str = "any",
) -> Path:
    """Resolve the first existing member of a worktree/main-checkout path list."""
    choices = tuple(candidate for path in paths for candidate in candidates(root, path))
    predicate = {"any": Path.exists, "file": Path.is_file, "dir": Path.is_dir}[kind]
    for candidate in choices:
        if predicate(candidate):
            return candidate
    return choices[0] if choices else Path(root).expanduser().resolve()


def disc_image(root: Path, env: Mapping[str, str] | None = None) -> Path:
    """Resolve the configured disc image or its conventional owner-file locations."""
    environ = os.environ if env is None else env
    configured = environ.get("TSFP_DISC")
    if configured:
        return resolve_path(root, configured, kind="file")
    return resolve_first(root, DISC_LOCATIONS, kind="file")


def xbox_files_dir(root: Path, env: Mapping[str, str] | None = None) -> Path:
    """Resolve the owner BIOS/MCPX/HDD directory, including a main-checkout fallback."""
    environ = os.environ if env is None else env
    return resolve_path(root, environ.get("TSFP_FILES_DIR") or FILES_DIR, kind="dir")


def disc_directory(
    root: Path,
    env: Mapping[str, str] | None = None,
    default: str | Path = "discs",
) -> Path:
    """Resolve the registry-based disc directory or TSFP_DISC_DIR override."""
    environ = os.environ if env is None else env
    return resolve_path(root, environ.get("TSFP_DISC_DIR") or default, kind="dir")


def recording_dirs(root: Path) -> tuple[Path, ...]:
    """Return recording and route directories from the worktree and main checkout."""
    paths = (
        Path("tmp/owner/recordings"),
        Path("tmp/owner/routes"),
        Path("tmp/routes"),
    )
    return tuple(dict.fromkeys(path for item in paths for path in candidates(root, item)))


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("kind", choices=("disc", "files-dir"))
    parser.add_argument("--root", type=Path, default=Path.cwd())
    args = parser.parse_args()
    path = disc_image(args.root) if args.kind == "disc" else xbox_files_dir(args.root)
    print(path)
    return 0


if __name__ == "__main__":
    sys.exit(main())
