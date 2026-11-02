"""T3472 phase 2: retention for the canonical build tree pool and the build verdict cache.

`tmp/wt-cache/canon-src/<key>[-pN]` are source mirrors of the canonical build pool
(tools.wt.canonical_build); each family key leaves up to N trees behind and nothing deleted
them. This removes a member only when nobody holds its lock (non-blocking flock, the same lock a
build takes) and it was idle for `idle_hours`; with `cap_gb` it also drops the least recently
used idle members until the pool fits. A removed member is rebuilt by the next build (a sync
copy, the cargo target lives elsewhere). `build-results/*.json` older than `results_days` go too
(a verdict is recomputed on demand).

    python3 -m tools.wt.canon_gc [--idle-hours 72] [--cap-gb 40] [--results-days 14] [--apply]
"""

from __future__ import annotations

import argparse
import fcntl
import json
import os
import shutil
import sys
import time
from pathlib import Path

from tools.wt import canonical_build, exec_sync

DEFAULT_IDLE_HOURS = 72.0
DEFAULT_CAP_GB = 40.0
DEFAULT_RESULTS_DAYS = 14.0


def tree_bytes(path: Path) -> int:
    total = 0
    for base, _dirs, files in os.walk(path):
        for name in files:
            try:
                total += os.lstat(os.path.join(base, name)).st_size
            except OSError:
                continue
    return total


def _last_used(base: Path, member: str) -> float:
    manifest = base / member / canonical_build.MANIFEST
    for path in (manifest, base / member, base / f"{member}.lock"):
        try:
            return path.stat().st_mtime
        except OSError:
            continue
    return 0.0


def _try_lock(path: Path) -> int | None:
    """The member lock when nobody holds it, else None (caller closes the fd)."""
    try:
        fd = os.open(path, os.O_RDWR | os.O_CREAT, 0o666)
    except OSError:
        return None
    try:
        fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except BlockingIOError:
        os.close(fd)
        return None
    return fd


def prune_pool(
    main: Path,
    idle_hours: float = DEFAULT_IDLE_HOURS,
    cap_gb: float = DEFAULT_CAP_GB,
    *,
    apply: bool = False,
    now: float | None = None,
    base: Path | None = None,
) -> dict[str, object]:
    """Remove idle, unlocked pool members: older than idle_hours, then LRU down to cap_gb."""
    base = base or main / canonical_build.CANON_REL
    now = time.time() if now is None else now
    stats: dict[str, object] = {"members": 0, "removed": [], "freed_gb": 0.0, "kept_gb": 0.0}
    if base.is_symlink() or not base.is_dir():
        return stats
    rows = []
    for entry in sorted(base.iterdir()):
        if entry.is_symlink() or not entry.is_dir() or entry.suffix in (".roots",):
            continue
        if not (entry / canonical_build.MANIFEST).exists() and not (entry / "Cargo.toml").exists():
            continue
        rows.append((_last_used(base, entry.name), entry.name, tree_bytes(entry)))
    stats["members"] = len(rows)
    total = sum(size for _used, _name, size in rows)
    cap = int(cap_gb * 1024**3)
    removed: list[str] = []
    freed = 0
    for used, name, size in sorted(rows):
        too_old = now - used > idle_hours * 3600
        over_cap = total - freed > cap
        if not too_old and not over_cap:
            continue
        member = base / name
        synced = any((member / data).exists() for data in (exec_sync.MARKER, exec_sync.LISTING))
        if synced and exec_sync.leased(member):
            continue  # T3760: a remote exec dir a job holds or a sync just filled (shared root)
        fd = _try_lock(base / f"{name}.lock")
        if fd is None:  # a build holds this tree
            continue
        try:
            if apply:
                shutil.rmtree(base / name, ignore_errors=True)
                shutil.rmtree(base / f"{name}.roots", ignore_errors=True)
            removed.append(name)
            freed += size
        finally:
            os.close(fd)
    stats["removed"] = removed
    stats["freed_gb"] = round(freed / 1024**3, 2)
    stats["kept_gb"] = round((total - freed) / 1024**3, 2)
    return stats


def prune_results(
    main: Path, days: float = DEFAULT_RESULTS_DAYS, *, apply: bool = False, now: float | None = None
) -> int:
    """Delete recorded build verdicts older than `days`; returns how many."""
    base = main / "tmp/wt-cache/build-results"
    now = time.time() if now is None else now
    count = 0
    if not base.is_dir() or base.is_symlink():
        return 0
    for path in base.glob("*.json"):
        try:
            if now - path.stat().st_mtime <= days * 86400:
                continue
            if apply:
                path.unlink()
            count += 1
        except OSError:
            continue
    return count


def run(main: Path, *, apply: bool = False) -> dict[str, object]:
    """The hygiene tick pass: defaults, apply optional."""
    pool = prune_pool(main, apply=apply)
    shared = canonical_build.shared_root(main, dict(os.environ))  # T3472 p3: the fixed-root pool
    if shared is not None:
        pool["shared"] = prune_pool(main, apply=apply, base=shared)
    pool["results_removed"] = prune_results(main, apply=apply)
    return pool


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(prog="python3 -m tools.wt.canon_gc", description=__doc__)
    parser.add_argument("--idle-hours", type=float, default=DEFAULT_IDLE_HOURS)
    parser.add_argument("--cap-gb", type=float, default=DEFAULT_CAP_GB)
    parser.add_argument("--results-days", type=float, default=DEFAULT_RESULTS_DAYS)
    parser.add_argument("--apply", action="store_true", help="delete (default: dry run)")
    args = parser.parse_args(argv)
    from tools.wt import core

    main_dir = core.main_root(core.worktree_root(Path.cwd()))
    pool = prune_pool(main_dir, args.idle_hours, args.cap_gb, apply=args.apply)
    shared = canonical_build.shared_root(main_dir, dict(os.environ))
    if shared is not None:
        pool["shared"] = prune_pool(
            main_dir, args.idle_hours, args.cap_gb, apply=args.apply, base=shared
        )
    pool["results_removed"] = prune_results(main_dir, args.results_days, apply=args.apply)
    pool["applied"] = args.apply
    print(json.dumps(pool))
    return 0


if __name__ == "__main__":
    sys.exit(main())
