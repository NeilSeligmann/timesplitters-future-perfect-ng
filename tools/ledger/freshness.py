# SPDX-License-Identifier: GPL-3.0-or-later
"""T2134: snapshot freshness guard and safe swap for the gitignored generated/ledger."""

from __future__ import annotations

import os
import shutil
import subprocess
import time
from collections.abc import Sequence
from pathlib import Path


def input_time(path: Path) -> float | None:
    """Last commit time of a tracked input (checkouts reset mtimes, commits do not);
    falls back to mtime for untracked files or outside a repository."""
    if not path.exists():
        return None
    out = subprocess.run(
        ["git", "log", "-1", "--format=%ct", "--", path.name],
        capture_output=True,
        text=True,
        check=False,
        cwd=path.resolve().parent,
    ).stdout.strip()
    return float(out) if out.isdigit() else path.stat().st_mtime


def check(snapshot: Path, inputs: Sequence[Path]) -> tuple[float | None, list[str]]:
    """Return (snapshot age in seconds, warnings).

    Rule: the snapshot (index.json mtime = when it was generated) is stale when it is
    older than the last commit time of any input it is derived from (port registry,
    c2rust status csv). A missing snapshot index is also a warning."""
    index = snapshot / "index.json" if snapshot.is_dir() else snapshot
    if not index.is_file():
        return None, [f"snapshot index {index} missing"]
    made = index.stat().st_mtime
    warnings = []
    for path in inputs:
        newest = input_time(path)
        if newest is not None and made < newest:
            warnings.append(
                f"snapshot {index} is older than {path} by {(newest - made) / 3600:.1f} h: "
                "run `python3 -m tools.ledger refresh`"
            )
    return time.time() - made, warnings


def swap_in(tmp: Path, target: Path) -> None:
    """Replace target with tmp via renames: a reader that already opened the old files
    keeps them, a new reader sees the old or the new directory (never a half-written one)."""
    old = target.with_name(target.name + f".old{os.getpid()}")
    if target.exists():
        os.replace(target, old)
    os.replace(tmp, target)
    shutil.rmtree(old, ignore_errors=True)
