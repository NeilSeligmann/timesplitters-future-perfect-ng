"""T3748 shared headroom model: one view of every fleet host's spare capacity and eligibility.

Verify (`tools.wt.remote_exec.ranked_hosts`), replay and capture (`tools.wt.remote_replay`) and
remote prebuilds (`tools.wt.prebuilt._build_remote`) all order hosts with `score` / `order_names`,
and the balancer (`tools.fleet.balance`) diagnoses and fixes the hosts this marks ineligible.

Per host: load1 per core, MemAvailable, disk free against its floor, codex workers against the
configured max, and an eligibility REASON (empty when eligible):

- `off_file`               the owner reclaimed the host (`tmp/codexapi/<host>.off`)
- `unreachable`            the probe ssh failed
- `stale-code:<reason>`    a bench written before the last fix of the code that benches
- `benched:<reason>`       a live `tmp/wt-cache/remote-degraded/<host>.json` mark
- `disk-floor`             disk free under the host's `disk_floor_gb` (fleet.json)
- `disk-floor:<mount>`     a mount builds write to (`/`, `/tmp`, cargo, exec dirs) under its floor
- `toolchain-parity-fail`  (builds only) the host's toolchain differs from this machine's

Probes come from the controller's status file when it is fresh, else one batched ssh per host
(`status.PROBE_SH`); the result is cached in `tmp/fleet/headroom.json` for `TTL_S`. Consumers
outside the balancer only read that cache (`order_names`): they never ssh for ordering.
"""

from __future__ import annotations

import json
import time
from collections.abc import Callable, Mapping, Sequence
from dataclasses import asdict, dataclass
from pathlib import Path

from tools.fleet import status as fleet_status
from tools.fleet.config import FleetConfig, HostConfig
from tools.fleet.core import Runner
from tools.wt import disk_mounts

TTL_S = 60.0
"""Probe cache lifetime (one batched ssh per host per minute at most)."""
STATUS_FRESH_S = 180.0
"""The controller's status file is reused while younger than this."""
CONSUMER_MAX_AGE_S = 900.0
"""`order_names` ignores an older cache (no balancer running): callers keep their own order."""
LOAD_STEP = 0.25
"""Load-per-core bucket width: hosts within one step tie and free memory decides."""
DEGRADED_DIR = Path("tmp/wt-cache/remote-degraded")
CACHE_REL = Path("tmp/fleet/headroom.json")
TRANSIENT_BENCH = ("presence check timed out", "slot_taken", "slot taken", "slot busy")
"""Bench reasons that describe a momentary transport or slot race, not a broken host."""
BENCH_WINDOW_S = 30 * 60
"""A bench older than this is stale whatever its reason (the fault would have recurred)."""
TRANSIENT_MIN_AGE_S = 120
"""A transient bench is kept this long so concurrent callers do not stampede the host."""


@dataclass(frozen=True)
class Bench:
    reason: str
    at: float
    until: float

    def age(self, now: float) -> float:
        return max(0.0, now - self.at) if self.at else BENCH_WINDOW_S + 1

    def stale_reason(self, now: float, code_fixed_at: float | None) -> str:
        """Why this bench may be cleared now, else "" (a fresh real failure is kept)."""
        if code_fixed_at and self.at and self.at < code_fixed_at:
            return "pre-fix-code"
        age = self.age(now)
        if age > BENCH_WINDOW_S:
            return "older-than-window"
        if age >= TRANSIENT_MIN_AGE_S and any(t in self.reason for t in TRANSIENT_BENCH):
            return "transient"
        return ""


@dataclass(frozen=True)
class Headroom:
    host: str
    is_local: bool
    reachable: bool
    load1: float
    cores: int
    mem_avail_gb: float
    disk_free_gb: float
    disk_floor_gb: float
    workers: int
    max_workers: int
    memguard: int
    reason: str
    bench_reason: str = ""
    bench_age_s: float | None = None
    build_reason: str = ""

    @property
    def eligible(self) -> bool:
        return not self.reason and not self.is_local

    @property
    def load_per_core(self) -> float:
        return self.load1 / max(1, self.cores)

    @property
    def free_slots(self) -> int:
        return max(0, self.max_workers - self.workers)

    def score(self) -> tuple[int, float]:
        return score(self.load1, self.cores, self.mem_avail_gb)

    def to_json(self) -> dict:
        data = asdict(self)
        data["eligible"] = self.eligible
        data["load_per_core"] = round(self.load_per_core, 3)
        return data


def score(load1: float | None, cores: int | None, mem_gb: float) -> tuple[int, float]:
    """Sort key, smallest first: load-per-core bucket, then the most free memory."""
    per_core = 1.0 if load1 is None or not cores else load1 / cores
    return (min(int(per_core / LOAD_STEP), 400), -mem_gb)


def read_bench(root: Path, name: str) -> Bench | None:
    """The raw remote-degraded mark (expired marks included: the balancer reports them)."""
    try:
        data = json.loads((root / DEGRADED_DIR / f"{name}.json").read_text())
        until = float(data["until"])
    except (OSError, ValueError, KeyError, TypeError):
        return None
    return Bench(str(data.get("reason", "degraded")), float(data.get("at") or 0), until)


def mount_floor_breach(st: fleet_status.HostStatus | None) -> str:
    """Name of the lowest mount under its floor (registry floors, rule 45), else empty."""
    if st is None:
        return ""
    floor, root_floor = disk_mounts.floors_for(st.name)
    breaches = st.mount_violations(floor, root_floor)
    return min(breaches, key=lambda v: v.free_gb).mount if breaches else ""


def measure(
    config: FleetConfig,
    statuses: Mapping[str, fleet_status.HostStatus],
    now: float,
    *,
    code_fixed_at: float | None = None,
    toolchain: Callable[[HostConfig], str | None] | None = None,
) -> dict[str, Headroom]:
    """Headroom of every configured host from probe statuses (pure apart from the bench files)."""
    out: dict[str, Headroom] = {}
    for name, host in config.hosts.items():
        st = statuses.get(name)
        off = bool(host.off_file) and (config.codexapi_dir / host.off_file).exists()
        bench = None if host.is_local else read_bench(config.root, name)
        live_bench = bench if bench is not None and bench.until > now else None
        reachable = st is not None and st.reachable
        disk = st.disk_free_gb if reachable and st else 0.0
        if off:
            reason = "off_file"
        elif not reachable:
            reason = "unreachable"
        elif live_bench is not None and live_bench.stale_reason(now, code_fixed_at) == (
            "pre-fix-code"
        ):
            reason = f"stale-code:{live_bench.reason}"
        elif live_bench is not None:
            reason = f"benched:{live_bench.reason}"
        elif disk < host.disk_floor_gb:
            reason = "disk-floor"
        elif mount_low := mount_floor_breach(st):
            reason = f"disk-floor:{mount_low}"  # a full root / refuses builds, data volume roomy
        else:
            reason = ""
        build_reason = ""
        if toolchain is not None and not host.is_local and not reason:
            build_reason = "toolchain-parity-fail" if toolchain(host) else ""
        out[name] = Headroom(
            host=name,
            is_local=host.is_local,
            reachable=reachable,
            load1=st.number("load") if reachable and st else 0.0,
            cores=st.cores if reachable and st else 0,
            mem_avail_gb=st.mem_avail_gb if reachable and st else 0.0,
            disk_free_gb=disk,
            disk_floor_gb=host.disk_floor_gb,
            workers=st.workers if reachable and st else 0,
            max_workers=host.high,
            memguard=int(st.number("memguard")) if reachable and st else 0,
            reason=reason,
            bench_reason=live_bench.reason if live_bench else "",
            bench_age_s=round(live_bench.age(now), 1) if live_bench else None,
            build_reason=build_reason,
        )
    return out


def probe_statuses(
    config: FleetConfig, runner: Runner, now: float
) -> dict[str, fleet_status.HostStatus]:
    """The controller's status file while fresh, else one batched ssh per host."""
    try:
        age = now - config.status_file.stat().st_mtime
    except OSError:
        age = None
    if age is not None and age <= STATUS_FRESH_S:
        parsed = fleet_status.parse_status(fleet_status.read_status(config))
        if parsed:
            for host in config.hosts.values():
                if host.is_local and host.name not in parsed:
                    parsed[host.name] = fleet_status.local_status(config, host)
            return parsed
    return fleet_status.collect(config, runner)


def ranked(headrooms: Mapping[str, Headroom], *, build: bool = False) -> list[Headroom]:
    """Eligible remote hosts, least loaded per core first, memory as tie-break.

    The owner PC is never in this list (rule 26, T3745): it is not overflow capacity."""
    usable = [h for h in headrooms.values() if h.eligible and not (build and h.build_reason)]
    return sorted(usable, key=Headroom.score)


def names_of_ranked(headrooms: Mapping[str, Headroom], *, build: bool = False) -> list[str]:
    return [room.host for room in ranked(headrooms, build=build)]


def write_cache(root: Path, headrooms: Mapping[str, Headroom], now: float, aliases: dict) -> None:
    path = root / CACHE_REL
    path.parent.mkdir(parents=True, exist_ok=True)
    body = {
        "at": now,
        "hosts": {name: h.to_json() for name, h in headrooms.items()},
        "aliases": aliases,
    }
    tmp = path.with_name(path.name + ".new")
    tmp.write_text(json.dumps(body, indent=1, sort_keys=True) + "\n")
    tmp.replace(path)


def aliases(config: FleetConfig) -> dict[str, str]:
    """Every name a consumer may use for a host (replay name, address...) -> fleet host name."""
    out: dict[str, str] = {}
    for host in config.hosts.values():
        for alias in (host.name, host.replay_name, host.address, host.replay_target, host.target):
            if alias:
                out[alias] = host.name
    return out


def read_cache(root: Path, now: float | None = None) -> dict | None:
    try:
        data = json.loads((root / CACHE_REL).read_text())
    except (OSError, ValueError):
        return None
    current = time.time() if now is None else now
    if not isinstance(data, dict) or current - float(data.get("at", 0)) > CONSUMER_MAX_AGE_S:
        return None
    return data


def free_builders(names: Sequence[str], root: Path, *, now: float | None = None) -> list[str]:
    """Remote hosts among `names` the cached headroom marks eligible for a build (T3834).

    An absent or stale cache returns [] (nothing is known to be free)."""
    data = read_cache(root, now)
    if data is None:
        return []
    hosts, alias = data.get("hosts", {}), data.get("aliases", {})
    free = []
    for name in names:
        entry = hosts.get(alias.get(name, name))
        if (
            isinstance(entry, dict)
            and not entry.get("is_local")
            and entry.get("reachable", True)
            and not entry.get("reason")
            and not entry.get("build_reason")
        ):
            free.append(name)
    return free


def order_names(
    names: Sequence[str], root: Path, *, build: bool = False, now: float | None = None
) -> list[str]:
    """`names` reordered by the cached headroom: eligible by score, unknown, then ineligible.

    A missing or stale cache keeps the given order. Ineligible hosts are demoted, never dropped:
    the callers keep their own hard checks and an empty list would push work onto the owner PC."""
    data = read_cache(root, now)
    if data is None:
        return list(names)
    hosts, alias = data.get("hosts", {}), data.get("aliases", {})

    def key(item: tuple[int, str]) -> tuple:
        index, name = item
        entry = hosts.get(alias.get(name, name))
        if entry is None or entry.get("is_local"):
            return (1, (0, 0.0), index)
        bad = entry.get("reason") or (build and entry.get("build_reason"))
        if bad:
            return (2, (0, 0.0), index)
        return (
            0,
            score(entry.get("load1"), entry.get("cores"), entry.get("mem_avail_gb", 0)),
            index,
        )

    return [name for _, name in sorted(enumerate(names), key=key)]


def snapshot(
    config: FleetConfig,
    runner: Runner,
    now: float,
    *,
    code_fixed_at: float | None = None,
    toolchain: Callable[[HostConfig], str | None] | None = None,
    force: bool = False,
) -> dict[str, Headroom]:
    """Measure (or reuse the cache younger than TTL_S) and refresh `tmp/fleet/headroom.json`."""
    cached = None if force else read_cache(config.root, now)
    if cached is not None and now - float(cached["at"]) < TTL_S:
        fields = Headroom.__dataclass_fields__
        return {
            name: Headroom(**{k: v for k, v in entry.items() if k in fields})
            for name, entry in cached["hosts"].items()
        }
    result = measure(
        config,
        probe_statuses(config, runner, now),
        now,
        code_fixed_at=code_fixed_at,
        toolchain=toolchain,
    )
    write_cache(config.root, result, now, aliases(config))
    return result
