# SPDX-License-Identifier: GPL-3.0-or-later
"""T3871: host limits for local solver runs (claude-1 2026-10-28, after three parallel CBMC runs
took the owner PC to 0.5 GB available).

- At most `MAX_LOCAL_JOBS` (2) solver jobs per host at once: a job holds one of the flock'd slot
  files under the main checkout's `tmp/t2symbolic/slots` (host-wide, shared by every worktree and
  process; a killed holder releases its lock with its file descriptor).
- Each job's whole process tree runs under `RLIMIT_AS` = `MEMORY_LIMIT_BYTES` (6 GB, the same as
  `prlimit --as=6000000000`); a job that needs more fails and is reported as `timeout` with an
  out-of-memory reason, never retried locally (bigger runs go to the remote hosts, rule 47).
- Peak RSS of the tree is measured with GNU time (`/usr/bin/time -f`), one number per function.
"""

from __future__ import annotations

import contextlib
import fcntl
import os
import re
import resource
import time
from collections.abc import Callable, Iterator
from pathlib import Path

MAX_LOCAL_JOBS = 2
MEMORY_LIMIT_BYTES = 6_000_000_000
SLOT_DIR = Path("tmp/t2symbolic/slots")
GNU_TIME = Path("/usr/bin/time")
MAXRSS_TAG = "t2s-maxrss-kb"
_MAXRSS = re.compile(rf"^{MAXRSS_TAG} (\d+)$", re.M)


class SlotTimeout(TimeoutError):
    """No solver slot became free in time."""


def try_slot(slot_dir: Path, jobs: int = MAX_LOCAL_JOBS) -> int | None:
    """An open, locked slot file descriptor, or None when all `jobs` slots are held."""
    slot_dir.mkdir(parents=True, exist_ok=True)
    for index in range(jobs):
        fd = os.open(slot_dir / f"slot-{index}.lock", os.O_RDWR | os.O_CREAT, 0o644)
        try:
            fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            os.close(fd)
            continue
        return fd
    return None


@contextlib.contextmanager
def slot(
    slot_dir: Path,
    jobs: int = MAX_LOCAL_JOBS,
    wait: float = 3600.0,
    sleep: Callable[[float], None] = time.sleep,
    clock: Callable[[], float] = time.monotonic,
) -> Iterator[int]:
    """Hold one of `jobs` host-wide slots for the duration of the block."""
    deadline = clock() + wait
    while True:
        fd = try_slot(slot_dir, jobs)
        if fd is not None:
            break
        if clock() >= deadline:
            raise SlotTimeout(f"no free solver slot in {slot_dir} after {wait:.0f} s")
        sleep(1.0)
    try:
        yield fd
    finally:
        fcntl.flock(fd, fcntl.LOCK_UN)
        os.close(fd)


EXEC_MARKER = ".tsfp-source.json"
"""The `tools.wt.exec_sync` marker: a tree shipped to a remote execution host (rule 47)."""
REMOTE_MAX_BYTES = 16_000_000_000
"""Per-job ceiling on a remote host (T3874: the laptop's 38 GB holds 2 jobs at 16 GB)."""


def remote_exec_tree(root: Path) -> bool:
    """True in a tree shipped by `tools.wt.exec_sync` to a remote host: its marker is present
    and there is no git checkout (remote hosts keep none). The owner PC is never one."""
    return (root / EXEC_MARKER).is_file() and not (root / ".git").exists()


def job_memory(root: Path, requested_gb: float | None = None) -> int:
    """Address-space cap for one solver job. The local cap (`MEMORY_LIMIT_BYTES`) is the default
    and the maximum on the owner PC; a larger request is honoured only on a remote exec tree,
    up to `REMOTE_MAX_BYTES`."""
    if requested_gb is None:
        return MEMORY_LIMIT_BYTES
    requested = int(requested_gb * 1_000_000_000)
    if requested <= 0:
        raise ValueError("memory cap must be positive")
    if requested <= MEMORY_LIMIT_BYTES:
        return requested
    if not remote_exec_tree(root):
        raise ValueError(
            f"{requested_gb} GB is over the local cap of {MEMORY_LIMIT_BYTES / 1e9:.0f} GB; "
            "larger jobs run on a remote host (tools.wt.exec_sync tree)"
        )
    return min(requested, REMOTE_MAX_BYTES)


def limit_memory(limit: int = MEMORY_LIMIT_BYTES) -> Callable[[], None]:
    """A `preexec_fn` that caps the child's (and its descendants') address space."""

    def apply() -> None:
        resource.setrlimit(resource.RLIMIT_AS, (limit, limit))

    return apply


def measured(command: list[str]) -> list[str]:
    """`command` under GNU time printing the tree's peak RSS (falls back to the bare command)."""
    if not GNU_TIME.exists():
        return command
    return [str(GNU_TIME), "-f", f"{MAXRSS_TAG} %M", *command]


def peak_rss_mb(output: str) -> int | None:
    found = _MAXRSS.findall(output)
    return round(int(found[-1]) / 1024) if found else None


def out_of_memory(output: str) -> bool:
    markers = ("memory allocation of", "std::bad_alloc", "Cannot allocate memory", "out of memory")
    return any(marker in output for marker in markers)
