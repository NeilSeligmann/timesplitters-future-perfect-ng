"""Effective CPU and memory caps of THIS host or container (T3844).

The owner capped the agent containers with `docker update --cpus 9 --memory 24g`
(docs/decisions/container-caps.md). `os.cpu_count()`, `nproc`, /proc/meminfo and
/proc/loadavg still describe the whole host (32 cores, 64 GB, every container's load), so
every local sizing or load judgement reads the caps here:

* `effective_cpus()`: min(sched_getaffinity count, ceil(cgroup cpu.max quota / period)).
* `host_cpus()`: the affinity count, the denominator of the host-wide load average.
* `effective_load(host_load)`: load on the effective CPUs, so that
  `effective_cpus() - effective_load()` = min(quota - own use, host cores - host load).
* `effective_mem_available_bytes()`: min(MemAvailable, memory.max - memory.current), the
  cgroup's reclaimable inactive page cache counted as free.
* `effective_mem_total_bytes()`: min(MemTotal, memory.max).

Missing or unreadable cgroup files degrade to the plain OS values. `TSFP_CGROUP_ROOT`
overrides /sys/fs/cgroup (tests point it at an empty directory). Remote hosts are described
by their own probes (tools.wt.remote_exec), not by this module.

CLI for shell scripts: `python3 -m tools.hostcaps cpus|mem-available-mb`.
"""

from __future__ import annotations

import argparse
import functools
import math
import os
import time
from collections.abc import Callable
from pathlib import Path

CGROUP_ROOT = Path("/sys/fs/cgroup")
PROC_ROOT = Path("/proc")
USAGE_SAMPLE_SECONDS = 0.2
"""Window for the container's own CPU use (cpu.stat usage_usec) when a quota applies."""


def cgroup_root() -> Path:
    return Path(os.environ.get("TSFP_CGROUP_ROOT") or CGROUP_ROOT)


def _read(path: Path) -> str | None:
    try:
        return path.read_text().strip()
    except OSError:
        return None


def _meminfo_bytes(meminfo: Path, key: str) -> int | None:
    for line in (_read(meminfo) or "").splitlines():
        if line.startswith(key + ":"):
            try:
                return int(line.split()[1]) * 1024
            except (IndexError, ValueError):
                return None
    return None


def host_cpus() -> int:
    """CPUs this process may be scheduled on, ignoring any cgroup quota."""
    try:
        return len(os.sched_getaffinity(0)) or 1
    except (AttributeError, OSError):
        return os.cpu_count() or 1


def cgroup_cpu_limit(cgroup: Path | None = None) -> int | None:
    """ceil(quota / period) from cgroup v2 cpu.max, None when unlimited or unreadable."""
    text = _read((cgroup or cgroup_root()) / "cpu.max")
    if not text:
        return None
    parts = text.split()
    if parts[0] == "max":
        return None
    try:
        quota = int(parts[0])
        period = int(parts[1]) if len(parts) > 1 else 100000
    except ValueError:
        return None
    if quota <= 0 or period <= 0:
        return None
    return max(1, math.ceil(quota / period))


def compute_cpus(cgroup: Path | None = None, affinity: int | None = None) -> int:
    """Uncached effective CPU count (tests inject `cgroup` and `affinity`)."""
    cpus = affinity if affinity is not None else host_cpus()
    limit = cgroup_cpu_limit(cgroup)
    return max(1, min(cpus, limit) if limit is not None else cpus)


@functools.cache
def _cached_cpus(root: str) -> int:
    return compute_cpus(Path(root))


def effective_cpus() -> int:
    """CPUs this process may really use (cached per cgroup root: the quota is fixed per run)."""
    return _cached_cpus(str(cgroup_root()))


def _usage_usec(cgroup: Path) -> int | None:
    for line in (_read(cgroup / "cpu.stat") or "").splitlines():
        key, _, value = line.partition(" ")
        if key == "usage_usec":
            try:
                return int(value)
            except ValueError:
                return None
    return None


def own_busy_cpus(
    cgroup: Path | None = None,
    *,
    interval: float = USAGE_SAMPLE_SECONDS,
    sleep: Callable[[float], object] = time.sleep,
) -> float | None:
    """CPUs the cgroup used over `interval` seconds, None when cpu.stat is unreadable."""
    cgroup = cgroup or cgroup_root()
    before = _usage_usec(cgroup)
    if before is None:
        return None
    sleep(interval)
    after = _usage_usec(cgroup)
    if after is None or interval <= 0:
        return None
    return max(0.0, (after - before) / 1e6 / interval)


def effective_load(
    host_load: float | None = None,
    *,
    cgroup: Path | None = None,
    cpus: int | None = None,
    host: int | None = None,
    busy: float | None = None,
    sleep: Callable[[float], object] = time.sleep,
) -> float:
    """Load measured against `effective_cpus()`: the larger of the container's own CPU use
    and the host load the other host cores cannot absorb. Without a quota it is the host load.
    """
    load = os.getloadavg()[0] if host_load is None else host_load
    cgroup = cgroup or cgroup_root()
    cpus = compute_cpus(cgroup) if cpus is None else cpus
    host = host_cpus() if host is None else host
    if cpus >= host:
        return load
    if busy is None:
        busy = own_busy_cpus(cgroup, sleep=sleep)
    spill = max(0.0, load - (host - cpus))
    return max(busy or 0.0, spill)


def _inactive_file_bytes(cgroup: Path) -> int:
    """Reclaimable page cache charged to memory.current (docker stats subtracts it too)."""
    for line in (_read(cgroup / "memory.stat") or "").splitlines():
        key, _, value = line.partition(" ")
        if key == "inactive_file":
            try:
                return int(value)
            except ValueError:
                return 0
    return 0


def _cgroup_mem_max(cgroup: Path) -> int | None:
    text = _read(cgroup / "memory.max")
    if not text or text == "max":
        return None
    try:
        return int(text)
    except ValueError:
        return None


def cgroup_mem_headroom_bytes(cgroup: Path | None = None) -> int | None:
    """memory.max - (memory.current - inactive_file), None when memory.max is "max"
    or unreadable."""
    cgroup = cgroup or cgroup_root()
    limit = _cgroup_mem_max(cgroup)
    current = _read(cgroup / "memory.current")
    if limit is None or current is None:
        return None
    try:
        used = max(0, int(current) - _inactive_file_bytes(cgroup))
    except ValueError:
        return None
    return max(0, limit - used)


def _min_known(*values: int | None) -> int | None:
    known = [value for value in values if value is not None]
    return min(known) if known else None


def compute_mem_available_bytes(
    cgroup: Path | None = None, proc: Path = PROC_ROOT, meminfo: Path | None = None
) -> int | None:
    """Effective available memory in bytes (`meminfo` overrides proc/meminfo); None only
    when no source is readable."""
    available = _meminfo_bytes(meminfo or proc / "meminfo", "MemAvailable")
    return _min_known(available, cgroup_mem_headroom_bytes(cgroup))


def compute_mem_total_bytes(cgroup: Path | None = None, proc: Path = PROC_ROOT) -> int | None:
    """min(MemTotal, memory.max); None only when no source is readable."""
    return _min_known(
        _meminfo_bytes(proc / "meminfo", "MemTotal"), _cgroup_mem_max(cgroup or cgroup_root())
    )


def effective_mem_available_bytes() -> int | None:
    """Available memory under the container ceiling (not cached: it moves with load)."""
    return compute_mem_available_bytes()


def effective_mem_available_mb() -> int | None:
    value = effective_mem_available_bytes()
    return None if value is None else value // (1024 * 1024)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=(__doc__ or "").splitlines()[0])
    parser.add_argument("fact", choices=("cpus", "mem-available-mb"))
    args = parser.parse_args(argv)
    if args.fact == "cpus":
        print(effective_cpus())
    else:
        print(effective_mem_available_mb() or 0)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
