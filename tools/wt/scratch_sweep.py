"""Sweep leaked merge_task scratch dirs (preflight-*, merge-task-*, wc-cand-*) under tmp/.

A merge_task killed by SIGKILL, an OOM or a session restart never runs its `finally`
cleanup. `scratch.gc` then keeps the tree because it is unsealed or dirty, so ~750 MB dirs
pile up (MEASURED 2026-10-24: 115 dirs, ~70 GB). This sweeper removes any such dir that is
idle (newest mtime at depth 3 older than --hours), has no creator or process inside it and
is not named by any live command line, then deregisters its worktree.
"""

from __future__ import annotations

import argparse
import contextlib
import json
import os
import shutil
import subprocess
import sys
import time
from pathlib import Path
from typing import NamedTuple

from tools.wt import core, safe_prune, scratch

PREFIXES = ("preflight-", "merge-task-", "wc-cand-")
DEFAULT_HOURS = 2.0
MTIME_DEPTH = 3


class Unreadable(Exception):
    """Kept for callers (hygiene, cli); T3805: process_view no longer raises it."""


def newest_mtime(path: Path, depth: int = MTIME_DEPTH) -> float:
    """Newest lstat mtime of path and its entries down to `depth` levels (no symlink follow)."""
    newest = path.lstat().st_mtime
    stack = [(path, 0)]
    while stack:
        current, level = stack.pop()
        try:
            with os.scandir(current) as entries:
                for entry in entries:
                    try:
                        info = entry.stat(follow_symlinks=False)
                    except OSError:
                        continue
                    newest = max(newest, info.st_mtime)
                    if level + 1 < depth and entry.is_dir(follow_symlinks=False):
                        stack.append((Path(entry.path), level + 1))
        except OSError:
            continue
    return newest


PROC_ROOT = Path("/proc")  # tests point this at a fake tree (rule 39)


class Opaque(NamedTuple):
    """An own-uid process whose cwd cannot be read (non-dumpable, ptrace-denied).

    `anchor` is the cwd of its nearest readable ancestor (a child starts in its parent's
    cwd), None when no ancestor could be read."""

    pid: str
    anchor: str | None


class ProcessView(NamedTuple):
    cwds: list[str]
    cmdlines: str
    opaque: tuple[Opaque, ...] = ()


def _parent(entry: Path) -> str | None:
    try:
        return (entry / "stat").read_text().rsplit(")", 1)[1].split()[1]
    except (OSError, IndexError):
        return None


def process_view(proc: Path | None = None) -> ProcessView:
    """Every readable process cwd, all command lines joined, and the opaque own processes.

    One scan per sweep. Processes of another uid are skipped when unreadable (a container-
    foreign or root process cannot be ours). An own-uid process whose cwd is unreadable (T3805:
    a non-dumpable tsfp-bevy of the t2 harness) no longer aborts the sweep: it is recorded as
    opaque with its nearest readable ancestor's cwd, and `in_use` keeps only the candidates it
    may be inside (see docs/decisions/scratch-sweep-opaque-processes.md)."""
    proc = PROC_ROOT if proc is None else proc
    uid = os.getuid()
    cwd_of: dict[str, str] = {}
    parent_of: dict[str, str | None] = {}
    opaque: list[str] = []
    cmdlines: list[str] = []
    for entry in proc.iterdir():
        if not entry.name.isdecimal():
            continue
        try:
            cwd_of[entry.name] = os.readlink(entry / "cwd")
        except (FileNotFoundError, ProcessLookupError):
            continue
        except OSError:
            try:
                foreign = entry.stat().st_uid != uid
            except OSError:
                continue  # exited meanwhile
            if foreign:
                continue
            opaque.append(entry.name)
            parent_of[entry.name] = _parent(entry)
        try:
            cmdlines.append(
                (entry / "cmdline").read_bytes().replace(b"\0", b" ").decode("utf-8", "replace")
            )
        except OSError:
            pass
    anchored: list[Opaque] = []
    for pid in opaque:
        seen = {pid}
        parent = parent_of.get(pid)
        while parent and parent not in seen and parent not in cwd_of:
            seen.add(parent)
            parent = parent_of.get(parent) or _parent(proc / parent)
        anchored.append(Opaque(pid, cwd_of.get(parent) if parent else None))
    return ProcessView(list(cwd_of.values()), "\n".join(cmdlines), tuple(anchored))


def _inside(path: str, root: str) -> bool:
    return path == root or path.startswith(root + os.sep)


def in_use(directory: Path, view: ProcessView) -> str | None:
    root = str(directory)
    if any(_inside(c, root) for c in view.cwds):
        return "process cwd inside"
    if root in view.cmdlines:
        return "named by a live command line"
    for process in view.opaque:
        if process.anchor is None:
            return f"unreadable own process {process.pid} (fail closed)"
        if _inside(process.anchor, root):
            return f"unreadable own process {process.pid} under a process inside"
    return None


def creator_alive(directory: Path) -> bool:
    """Manifest creator (merge_task) or the wc-cand-<pid> owner still running."""
    if directory.name.startswith("wc-cand-"):
        try:
            return (PROC_ROOT / str(int(directory.name.removeprefix("wc-cand-")))).exists()
        except ValueError:
            return False
    try:
        data = json.loads((directory / scratch.MANIFEST).read_text())
        return scratch.owner_alive(data)
    except (OSError, ValueError, KeyError, TypeError):
        return False
    except Exception:  # noqa: BLE001 - unreadable owner process: fail closed
        return True


def tree_bytes(path: Path) -> int:
    total = 0
    for parent, dirs, files in os.walk(path, followlinks=False):
        dirs[:] = [d for d in dirs if not (Path(parent) / d).is_symlink()]
        for name in files:
            try:
                total += (Path(parent) / name).lstat().st_size
            except OSError:
                pass
    return total


def _git(main: Path, *args: str, timeout: int = 600) -> None:
    subprocess.run(
        ["git", "-C", str(main), *args],
        check=False,
        capture_output=True,
        timeout=timeout,
    )


def sweep(
    main: Path,
    *,
    hours: float = DEFAULT_HOURS,
    apply: bool = False,
    now: float | None = None,
    proc: Path | None = None,
) -> dict[str, object]:
    """Remove (apply) or list leaked scratch dirs; returns counts and bytes."""
    tmp = main / "tmp"
    now = time.time() if now is None else now
    removed: list[str] = []
    kept: dict[str, str] = {}
    freed = 0
    candidates = sorted(
        p
        for p in tmp.glob("*")
        if p.name.startswith(PREFIXES) and p.is_dir() and not p.is_symlink()
    )
    view = process_view(proc) if candidates else ProcessView([], "")
    for directory in candidates:
        try:
            if now - newest_mtime(directory) < hours * 3600:
                kept[directory.name] = "recent activity"
                continue
            if scratch.active(directory / "candidate"):
                kept[directory.name] = "active owner lease"
                continue
            reason = in_use(directory, view)
            if reason is None and creator_alive(directory):
                reason = "creator alive"
            if reason:
                kept[directory.name] = reason
                continue
            size = tree_bytes(directory)
            if apply:
                if (directory / "candidate").exists():
                    _git(main, "worktree", "remove", "--force", str(directory / "candidate"))
                shutil.rmtree(directory, ignore_errors=False)
            removed.append(directory.name)
            freed += size
        except OSError as exc:
            kept[directory.name] = f"error: {exc}"
    if apply:
        with contextlib.suppress(safe_prune.PruneError, OSError, subprocess.SubprocessError):
            safe_prune.prune(main, timeout=600)  # dead entries only, never another mount (T3784)
    verb = "removed" if apply else "would remove"
    for name in removed:
        print(f"{verb} scratch {name}")
    for name, why in kept.items():
        print(f"keep scratch {name}: {why}")
    print(f"scratch-sweep: {verb} {len(removed)} dir(s), {freed / 1e9:.2f} GB, kept {len(kept)}")
    return {"removed": removed, "kept": kept, "freed_bytes": freed}


def add_arguments(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--hours", type=float, default=DEFAULT_HOURS, help="idle age (default 2)")
    parser.add_argument("--apply", action="store_true", help="delete (default: dry run)")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    add_arguments(parser)
    args = parser.parse_args(argv)
    try:
        sweep(core.main_root(core.worktree_root(Path.cwd())), hours=args.hours, apply=args.apply)
    except Unreadable as exc:
        print(f"refused: {exc}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
