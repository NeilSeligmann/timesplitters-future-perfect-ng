"""LRU size cap for the remote replay input cache (T3678 remote footprint, I-T3678-1).

The cache (`<staging>/input-cache/<xx>/<sha256>-<name>`) holds one prebuilt host, source
archive or library per content hash. Without a cap it reached 32 GB in 24 h on the laptop.
`evict` is the ONE function used by the replay start (before staging) and by capguard.

Least recently USED first: `_copy_cached_input` touches an entry on every hit, so mtime is the
last-use time. An entry is in use while a copier holds a shared flock on `<entry>.lock`
(`use_lock_command`); eviction skips it, and the kernel drops the lock if the holder dies.
Stdlib only: it runs on remote hosts from the repo checkout.
"""

from __future__ import annotations

import argparse
import contextlib
import fcntl
import json
import shlex
import shutil
import time
from pathlib import Path
from typing import Any

DEFAULT_CAP_GB = 10
CONFIG_KEY = "replay_input_cache_gb"
LOCK_SUFFIX = ".lock"
INCOMING_STALE_S = 6 * 3600
GB = 1024**3


def cap_gb_for(host: dict[str, Any] | None, default: int = DEFAULT_CAP_GB) -> int:
    """Per-host cap from the remote host data (`replay_input_cache_gb`), else the default."""
    value = (host or {}).get(CONFIG_KEY, default)
    if isinstance(value, bool) or not isinstance(value, (int, float)) or value < 0:
        raise ValueError(f"{CONFIG_KEY} must be a non-negative number, got {value!r}")
    return value


def use_lock_command(entry_path: str, command: str) -> str:
    """Shell text running `command` while holding the shared in-use lock of a cache entry."""
    lock = shlex.quote(entry_path + LOCK_SUFFIX)
    return f"flock -s {lock} sh -c {shlex.quote(command)}"


def _in_use(entry: Path) -> bool:
    lock = Path(str(entry) + LOCK_SUFFIX)
    if not lock.exists():
        return False
    try:
        with lock.open("rb") as handle:
            try:
                fcntl.flock(handle, fcntl.LOCK_EX | fcntl.LOCK_NB)
            except OSError:
                return True
            fcntl.flock(handle, fcntl.LOCK_UN)
    except OSError:
        return True
    return False


def _entries(cache_root: Path, now: float) -> list[tuple[float, int, Path]]:
    found: list[tuple[float, int, Path]] = []
    for path in cache_root.rglob("*"):
        if path.is_dir() and path.parent.name == "source-trees":
            size = sum(item.stat().st_size for item in path.rglob("*") if item.is_file())
            found.append((path.stat().st_mtime, size, path))
            continue
        if any(parent.name == "source-trees" for parent in path.parents):
            continue
        if not path.is_file() or path.is_symlink() or path.name.endswith(LOCK_SUFFIX):
            continue
        stat = path.stat()
        if path.parent.name == "incoming" and now - stat.st_mtime < INCOMING_STALE_S:
            continue  # an upload in flight
        found.append((stat.st_mtime, stat.st_size, path))
    return found


def evict(cache_root: Path, cap_bytes: int, now: float | None = None) -> dict[str, int]:
    """Delete least-recently-used entries until the cache fits `cap_bytes`; skip in-use ones."""
    now = time.time() if now is None else now
    result = {"before": 0, "after": 0, "evicted": 0, "skipped_in_use": 0}
    if not cache_root.is_dir():
        return result
    entries = _entries(cache_root, now)
    total = sum(size for _, size, _ in entries)
    result["before"] = result["after"] = total
    for _, size, path in sorted(entries, key=lambda item: (item[0], str(item[2]))):
        if total <= cap_bytes:
            break
        if _in_use(path):
            result["skipped_in_use"] += 1
            continue
        with contextlib.suppress(FileNotFoundError):
            if path.is_dir():
                shutil.rmtree(path)
            else:
                path.unlink()
        with contextlib.suppress(FileNotFoundError):
            Path(str(path) + LOCK_SUFFIX).unlink()
        total -= size
        result["evicted"] += 1
    result["after"] = total
    return result


def evict_command(tools_dir: str, cache_root: str, cap_gb: float) -> str:
    """Shell text that runs `evict` on a worker from a dir holding the tools tree (T3760: the
    replay's synced exec dir; `tools_dir` is already shell-quoted). Non-fatal if missing."""
    return (
        f"cd {tools_dir} && python3 -m tools.wt.replay_input_cache evict "
        f"--cache-root {shlex.quote(cache_root)} --cap-gb {cap_gb}"
    )


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    run = sub.add_parser("evict", help="evict LRU entries down to the cap")
    run.add_argument("--cache-root", required=True, type=Path)
    target = run.add_mutually_exclusive_group()
    target.add_argument("--cap-gb", type=float, default=DEFAULT_CAP_GB)
    target.add_argument("--free-gb", type=float, help="evict this many GiB from current contents")
    args = parser.parse_args(argv)
    cache_root = args.cache_root.expanduser()
    if args.free_gb is None:
        cap_bytes = int(args.cap_gb * GB)
    else:
        total = sum(size for _, size, _ in _entries(cache_root, time.time()))
        cap_bytes = max(0, total - int(args.free_gb * GB))
    result = evict(cache_root, cap_bytes)
    print(json.dumps(result))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
