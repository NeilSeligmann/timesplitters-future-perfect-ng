"""One CPU and memory budget per remote host for every job kind we dispatch there (T3837).

Before T3837 a host's verify slots (`remote_exec`, remote flocks, shared with remote prebuilt
builds) and its replay/capture slots (`remote_replay.exec_slot`, owner-PC flocks) were two
independent pools. On the laptop (16 cores) 4 verify jobs (6 cargo jobs each) plus 4 captures
(about 1.5 cores each) model to 30 core units; the host sat at load 32-37 from 02:40 to 05:20 on
2026-10-27 (MEASURED, host-health refusal rows), so its 2.0-per-core health guard refused it and
the shared back-off benched it for 10 minutes at a time: verifies fell back to the owner PC.

Now every dispatch (verify, remote prebuilt build, replay or capture) holds a lease from ONE
ledger per host on the owner PC before it touches the host. A lease is admitted while the sum of
the live leases plus its own cost stays within the host's budget:

    remote_hosts.json, per host:   "budget": {"load_per_core": 1.5, "mem_mb": 30000}
    remote_hosts.json, top level:  "job_costs": {"verify": {"cores_per_cargo_job": 1.0,
                                                            "mem_mb": 4500}, ...}

Core units = host cores x load_per_core; a verify costs its cargo jobs (the CARGO_BUILD_JOBS it
runs with), a capture its configured cores. A host without `budget` is not gated (additive).

Ledger: `<dir>/<host>/<kind>-<pid>-<token>.lease` files, each flocked by its holder for the
whole job, so a crashed holder frees its share with the process (no stale accounting). Admission
scans under `<dir>/<host>/.mutex`. Dispatch happens only on the owner PC (rule 47), and every
session (worktrees, the other container) shares the main checkout's data dir.
"""

from __future__ import annotations

import argparse
import contextlib
import errno
import fcntl
import json
import os
import secrets
import sys
import time
from collections.abc import Callable, Iterator, Mapping
from dataclasses import dataclass
from pathlib import Path
from typing import IO

from tools import host_registry

DIR_ENV = "TSFP_HOST_BUDGET_DIR"
"""Ledger directory override (tests point it at their tmp dir)."""
SECTION = "job_costs"
KINDS = ("verify", "build", "capture")
"""verify: remote_exec verify; build: remote prebuilt build; capture: remote replay/capture."""
WAIT_POLL_S = 10.0


class BudgetFull(RuntimeError):
    """The host's budget has no room for the job now (a busy host, never a degraded one)."""


@dataclass(frozen=True)
class Budget:
    cores: float
    mem_mb: int


@dataclass(frozen=True)
class Cost:
    cores: float
    mem_mb: int

    def __str__(self) -> str:
        return f"{self.cores:g} cores/{self.mem_mb} MB"


@dataclass
class Reservation:
    """An already-admitted budget held from dispatch through remote execution.

    Passed explicitly, never through process-global environment (pytest dispatch has threads).
    The executor verifies the live handle, host, kind and price before reusing it.
    """

    host: str
    kind: str
    cost: Cost
    handle: IO[str]

    def covers(self, host: str, kind: str, cost: Cost) -> bool:
        if (
            self.handle.closed
            or self.host != host
            or self.kind != kind
            or self.cost.cores < cost.cores
            or self.cost.mem_mb < cost.mem_mb
        ):
            return False
        try:
            path = Path(self.handle.name)
            if os.fstat(self.handle.fileno()).st_ino != path.stat().st_ino:
                return False
            with path.open() as probe:
                if not _held(probe):
                    return False
                data = json.loads(path.read_text())
            return (
                data.get("pid") == os.getpid()
                and data.get("kind") == kind
                and float(data.get("cores", 0)) >= cost.cores
                and int(data.get("mem_mb", 0)) >= cost.mem_mb
            )
        except (OSError, ValueError, TypeError):
            return False

    def close(self) -> None:
        if not self.handle.closed:
            try:
                Path(self.handle.name).unlink(missing_ok=True)
            finally:
                self.handle.close()


def from_entry(entry: Mapping[str, object] | None) -> Budget | None:
    """The budget configured on a registry host entry, or None (ungated)."""
    raw = (entry or {}).get("budget")
    if not isinstance(raw, Mapping):
        return None
    cores = int(entry.get("cores", 0) or 0)  # type: ignore[union-attr, call-overload]
    per_core = float(raw.get("load_per_core", 0) or 0)  # type: ignore[arg-type]
    mem_mb = int(raw.get("mem_mb", 0) or 0)  # type: ignore[call-overload]
    if cores < 1 or per_core <= 0 or mem_mb < 1:
        raise ValueError(f"budget of {entry.get('name')}: needs cores, load_per_core, mem_mb")  # type: ignore[union-attr]
    return Budget(cores=round(cores * per_core, 3), mem_mb=mem_mb)


def for_host(name: str, path: Path | None = None) -> Budget | None:
    for entry in host_registry.hosts_or_empty(path):
        if entry.get("name") == name:
            return from_entry(entry)
    return None


def job_cost(kind: str, *, cargo_jobs: int = 0, path: Path | None = None) -> Cost:
    """The tracked cost of one `kind` job (`job_costs` in the registry). Raises on an unknown
    kind or a missing section: a budgeted host must never admit an unpriced job (rule 45)."""
    if kind not in KINDS:
        raise ValueError(f"unknown job kind {kind!r} (known: {', '.join(KINDS)})")
    costs = host_registry.load(path).section(SECTION)
    spec = costs.get(kind)
    if not isinstance(spec, Mapping):
        raise ValueError(f"{SECTION}.{kind} is missing from the host registry")
    cores = float(spec.get("cores", 0)) + float(spec.get("cores_per_cargo_job", 0)) * cargo_jobs
    return Cost(cores=round(cores, 3), mem_mb=int(spec.get("mem_mb", 0)))


def ledger_dir() -> Path:
    override = os.environ.get(DIR_ENV)
    if override:
        return Path(override)
    from tools.datadir import data_dir

    return data_dir("wt-cache/host-budget")


@dataclass(frozen=True)
class Lease:
    """A live lease (`.lease`) or a live waiter reservation (`.wait`)."""

    path: Path
    kind: str
    cost: Cost
    pid: int
    priority: int = 0
    since: float = 0.0


LEASE = ".lease"
WAIT = ".wait"
KIND_PRIORITY = {"build": 2, "capture": 1, "verify": 1}
"""Admission order among waiters: a prebuilt build (regress, playtest-blocker confirmation,
gates) outranks a routine verify or capture; equal priorities are first come, first served."""
PRIORITY_ENV = "TSFP_HOST_BUDGET_PRIORITY"
"""Explicit priority override for a dispatching process (an int)."""
WAITING_KINDS = ("build", "capture")
"""Kinds whose refusal registers a waiter reservation. Verify does not: its queue window polls
and a verify without room falls back to the owner PC, so it must never hold a build back."""
WAITER_STALE_S = 300.0
"""A waiter not refreshed by a new attempt for this long is dropped even if its holder lives
(the prebuilt loop retries at most every 120 s; a caller that gave up stops refreshing)."""


def priority_of(kind: str, environ: Mapping[str, str] | None = None) -> int:
    env = os.environ if environ is None else environ
    override = env.get(PRIORITY_ENV, "").strip()
    if override.lstrip("-").isdigit():
        return int(override)
    bonus = 1 if env.get("TSFP_MERGE_PLAYTEST_BLOCKER") == "1" else 0
    return KIND_PRIORITY.get(kind, 0) + bonus


def _held(handle: IO[str]) -> bool:
    try:
        fcntl.flock(handle, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except OSError as error:
        if error.errno not in (errno.EAGAIN, errno.EACCES, errno.EWOULDBLOCK):
            raise
        return True
    return False


def _live(host_dir: Path, suffix: str, now: float | None = None) -> list[Lease]:
    """Entries whose holder still has them flocked; dead ones are removed, and so are waiters
    not refreshed within WAITER_STALE_S (call under the mutex)."""
    live: list[Lease] = []
    for path in sorted(host_dir.glob(f"*{suffix}")):
        try:
            handle = path.open("r")
        except FileNotFoundError:
            continue
        with handle:
            if not _held(handle):
                path.unlink(missing_ok=True)  # nobody holds it: the holder died
                continue
            try:
                data = json.loads(path.read_text(encoding="utf-8") or "{}")
            except (OSError, ValueError):
                data = {}
            if (
                suffix == WAIT
                and (now if now is not None else time.time()) - float(data.get("seen", 0))
                > WAITER_STALE_S
            ):
                path.unlink(missing_ok=True)  # its caller stopped asking
                continue
            live.append(
                Lease(
                    path,
                    str(data.get("kind", "?")),
                    Cost(float(data.get("cores", 0)), int(data.get("mem_mb", 0))),
                    int(data.get("pid", 0)),
                    int(data.get("priority", 0)),
                    float(data.get("since", 0)),
                )
            )
    return live


def _live_leases(host_dir: Path) -> list[Lease]:
    return _live(host_dir, LEASE)


@contextlib.contextmanager
def _mutex(host_dir: Path) -> Iterator[None]:
    host_dir.mkdir(parents=True, exist_ok=True)
    with (host_dir / ".mutex").open("a") as handle:
        fcntl.flock(handle, fcntl.LOCK_EX)
        try:
            yield
        finally:
            fcntl.flock(handle, fcntl.LOCK_UN)


def usage(host: str, base: Path | None = None) -> tuple[Cost, list[Lease]]:
    """(summed cost, live leases) of `host` now."""
    host_dir = (base or ledger_dir()) / host
    if not host_dir.is_dir():
        return Cost(0.0, 0), []
    with _mutex(host_dir):
        live = _live_leases(host_dir)
    return _sum(live), live


def waiters(host: str, base: Path | None = None, now: float | None = None) -> list[Lease]:
    """Live waiter reservations of `host`, head first."""
    host_dir = (base or ledger_dir()) / host
    if not host_dir.is_dir():
        return []
    with _mutex(host_dir):
        return _ordered(_live(host_dir, WAIT, now))


def _ordered(entries: list[Lease]) -> list[Lease]:
    return sorted(entries, key=lambda item: (-item.priority, item.since))


def _sum(leases: list[Lease]) -> Cost:
    return Cost(
        round(sum(lease.cost.cores for lease in leases), 3),
        sum(lease.cost.mem_mb for lease in leases),
    )


def _plus(left: Cost, right: Cost) -> Cost:
    return Cost(round(left.cores + right.cores, 3), left.mem_mb + right.mem_mb)


def fits(used: Cost, cost: Cost, budget: Budget) -> bool:
    """Admit when the job fits beside the live leases. An idle host always admits one job, so a
    job priced above the whole budget still runs alone instead of never."""
    if used.cores == 0 and used.mem_mb == 0:
        return True
    return (
        used.cores + cost.cores <= budget.cores + 1e-9
        and used.mem_mb + cost.mem_mb <= budget.mem_mb
    )


def _admissible(
    used: Cost,
    cost: Cost,
    budget: Budget,
    waiting: list[Lease],
    *,
    priority: int,
    since: float,
    own: Path | None = None,
) -> tuple[bool, Lease | None]:
    """(admit, the waiter that blocks it). T3837 fairness: a job fits AND the head waiter ahead
    of it (higher priority, or equal and older) still fits after it. Without this a stream of
    6-core verifies kept the laptop at 18 of 24 units and an 8-core regress build waited 35+ min
    (2026-10-27 11:47)."""
    if not fits(used, cost, budget):
        return False, None
    ahead = [
        item
        for item in waiting
        if item.path != own
        and (item.priority > priority or (item.priority == priority and item.since < since))
    ]
    if not ahead:
        return True, None
    head = _ordered(ahead)[0]
    return (fits(_plus(used, cost), head.cost, budget), head)


def admits(
    host: str,
    cost: Cost,
    budget: Budget | None,
    base: Path | None = None,
    kind: str = "verify",
    now: float | None = None,
) -> bool:
    """Would `cost` be admitted on `host` now (the probe-time check; `acquire` decides)."""
    if budget is None:
        return True
    host_dir = (base or ledger_dir()) / host
    if not host_dir.is_dir():
        return True
    stamp = time.time() if now is None else now
    with _mutex(host_dir):
        used = _sum(_live_leases(host_dir))
        waiting = _live(host_dir, WAIT, stamp)
    admit, _head = _admissible(used, cost, budget, waiting, priority=priority_of(kind), since=stamp)
    return admit


_WAITERS: dict[tuple[str, str, Cost], tuple[IO[str], float]] = {}
"""This process's waiter reservations by (host dir, kind, cost): kept across the caller's
retries (the prebuilt loop calls build_to_staging again every 5-120 s)."""


def _drop_waiter(key: tuple[str, str, Cost]) -> None:
    entry = _WAITERS.pop(key, None)
    if entry is None:
        return
    handle, _since = entry
    Path(handle.name).unlink(missing_ok=True)
    with contextlib.suppress(OSError):
        fcntl.flock(handle, fcntl.LOCK_UN)
    handle.close()


def _register_waiter(
    host_dir: Path, key: tuple[str, str, Cost], kind: str, cost: Cost, priority: int, now: float
) -> Path:
    """Create or refresh this process's waiter file (call under the mutex)."""
    entry = _WAITERS.get(key)
    since = now
    if entry is not None:
        handle, since = entry
        if Path(handle.name).exists():
            body = {"kind": kind, "cores": cost.cores, "mem_mb": cost.mem_mb}
            body |= {"pid": os.getpid(), "priority": priority, "since": since, "seen": now}
            handle.seek(0)
            handle.truncate()
            handle.write(json.dumps(body))
            handle.flush()
            return Path(handle.name)
        _drop_waiter(key)  # expired by a scan: re-register with the original place in line
    path = host_dir / f"{kind}-{os.getpid()}-{secrets.token_hex(4)}{WAIT}"
    handle = path.open("w")
    fcntl.flock(handle, fcntl.LOCK_EX)
    body = {"kind": kind, "cores": cost.cores, "mem_mb": cost.mem_mb, "pid": os.getpid()}
    handle.write(json.dumps(body | {"priority": priority, "since": since, "seen": now}))
    handle.flush()
    _WAITERS[key] = (handle, since)
    return path


def try_acquire(
    host: str,
    kind: str,
    cost: Cost,
    budget: Budget,
    base: Path | None = None,
    *,
    priority: int | None = None,
    register: bool | None = None,
    now: float | None = None,
) -> tuple[IO[str] | None, Cost]:
    """A held lease handle, or (None, current use) when the budget has no room or a waiter
    ahead would no longer fit. A refused `build` or `capture` (`register`) keeps a waiter
    reservation, so later attempts keep its place in line."""
    host_dir = (base or ledger_dir()) / host
    level = priority_of(kind) if priority is None else priority
    queue = kind in WAITING_KINDS if register is None else register
    stamp = time.time() if now is None else now
    key = (str(host_dir), kind, cost)
    with _mutex(host_dir):
        used = _sum(_live_leases(host_dir))
        waiting = _live(host_dir, WAIT, stamp)
        mine = _WAITERS.get(key)
        own = Path(mine[0].name) if mine else None
        since = mine[1] if mine else stamp
        admit, _head = _admissible(
            used, cost, budget, waiting, priority=level, since=since, own=own
        )
        if not admit:
            if queue:
                _register_waiter(host_dir, key, kind, cost, level, stamp)
            return None, used
        _drop_waiter(key)
        path = host_dir / f"{kind}-{os.getpid()}-{secrets.token_hex(4)}{LEASE}"
        handle = path.open("w")
        fcntl.flock(handle, fcntl.LOCK_EX)
        body = {"kind": kind, "cores": cost.cores, "mem_mb": cost.mem_mb, "pid": os.getpid()}
        handle.write(json.dumps({**body, "since": round(stamp)}))
        handle.flush()
        return handle, used


@contextlib.contextmanager
def lease(
    host: str,
    kind: str,
    cost: Cost,
    budget: Budget | None,
    *,
    wait_s: float = 0.0,
    base: Path | None = None,
    sleep: Callable[[float], None] = time.sleep,
    clock: Callable[[], float] = time.monotonic,
) -> Iterator[None]:
    """Hold `cost` of `host`'s budget for the block; BudgetFull when it has no room within
    `wait_s` (polled every WAIT_POLL_S). An unbudgeted host (budget None) is not gated."""
    if budget is None:
        yield
        return
    deadline = clock() + wait_s
    while True:
        handle, used = try_acquire(host, kind, cost, budget, base)
        if handle is not None:
            break
        if clock() + WAIT_POLL_S > deadline:
            if wait_s > 0:  # waited here and gave up: leave no reservation behind
                _drop_waiter((str((base or ledger_dir()) / host), kind, cost))
            raise BudgetFull(
                f"{host}:budget_full({kind} {cost} beside {used} of "
                f"{budget.cores:g} cores/{budget.mem_mb} MB)"
            )
        sleep(WAIT_POLL_S)
    try:
        yield
    finally:
        path = Path(handle.name)
        path.unlink(missing_ok=True)
        fcntl.flock(handle, fcntl.LOCK_UN)
        handle.close()


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="T3837 per-host job budget: live leases")
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args(argv)
    report: dict[str, object] = {}
    for entry in host_registry.hosts_or_empty():
        budget = from_entry(entry)
        if budget is None:
            continue
        used, live = usage(str(entry["name"]))
        report[str(entry["name"])] = {
            "budget": {"cores": budget.cores, "mem_mb": budget.mem_mb},
            "used": {"cores": used.cores, "mem_mb": used.mem_mb},
            "leases": [
                {"kind": item.kind, "pid": item.pid, "cost": str(item.cost)} for item in live
            ],
            "waiters": [
                {
                    "kind": item.kind,
                    "pid": item.pid,
                    "cost": str(item.cost),
                    "priority": item.priority,
                }
                for item in waiters(str(entry["name"]))
            ],
        }
    if args.json:
        print(json.dumps(report, indent=1, sort_keys=True))
    else:
        for name, row in report.items():
            print(
                f"{name}: {row['used']} of {row['budget']}, {len(row['leases'])} leases, "  # type: ignore[index]
                f"waiters {row['waiters']}"  # type: ignore[index]
            )
    return 0


if __name__ == "__main__":
    sys.exit(main())
