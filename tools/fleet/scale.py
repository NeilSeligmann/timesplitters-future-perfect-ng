"""Worker-slot scaling and launch gating (replaces autoscale.py and the sup-loop refill gates).

`decide` and `plan_refill` are pure functions of a host's policy, its probe and the queue length.
"""

from __future__ import annotations

import json
import time
from dataclasses import dataclass
from pathlib import Path

from tools.fleet import launch_health
from tools.fleet.config import FleetConfig, HostConfig
from tools.fleet.status import HostStatus


@dataclass(frozen=True)
class ScaleDecision:
    host: str
    old: int
    new: int
    reason: str

    @property
    def changed(self) -> bool:
        return self.new != self.old


def decide(host: HostConfig, status: HostStatus, target: int, queue: int) -> tuple[int, str]:
    """New worker target and the reason. At most one step per call; the caller applies the
    cooldown."""
    if not status.reachable:
        return target, "unreachable"
    paused = status.paused
    mem = status.mem_avail_gb
    disk = status.disk_free_gb
    load = status.load_per_core
    if (
        paused > 0
        or mem < host.mem_floor_gb
        or load > host.load_down
        or disk < host.disk_floor_gb + 3
        or status.overcap
    ):
        if paused:
            reason = f"paused={paused}"
        elif mem < host.mem_floor_gb:
            reason = f"mem={mem:.0f}G"
        elif load > host.load_down:
            reason = f"load/core={load:.2f}"
        elif disk < host.disk_floor_gb + 3:
            reason = f"disk={disk:.0f}G"
        else:
            reason = "overcap"
        return max(host.low, min(target, status.workers) - 1), f"down ({reason})"
    headroom = (
        mem - host.mem_floor_gb >= 2 * host.per_worker_gb
        and load < host.load_down - 0.5
        and disk > host.disk_floor_gb + 8
    )
    if status.workers >= target - 1 and queue > 10 and headroom:
        return min(
            host.high, target + 1
        ), f"up (mem={mem:.0f}G load/core={load:.2f} disk={disk:.0f}G)"
    return target, "hold"


@dataclass
class AutoscaleState:
    """Targets per host and the time of the last change (same JSON as the legacy
    autoscale-state.json)."""

    targets: dict[str, int]
    changed: dict[str, float]

    @classmethod
    def load(cls, path: Path, config: FleetConfig) -> AutoscaleState:
        data: dict = {}
        try:
            data = json.loads(path.read_text())
        except (OSError, ValueError):
            pass
        targets = {
            name: int(data.get("targets", {}).get(name, host.start))
            for name, host in config.hosts.items()
        }
        return cls(targets, {k: float(v) for k, v in data.get("changed", {}).items()})

    def ensure(self, config: FleetConfig) -> None:
        """Add hosts that appeared after a config reload (at their start target)."""
        for name, host in config.hosts.items():
            self.targets.setdefault(name, host.start)

    def dump(self) -> str:
        return json.dumps({"targets": self.targets, "changed": self.changed})


def autoscale(
    config: FleetConfig,
    statuses: dict[str, HostStatus],
    state: AutoscaleState,
    queue: int,
    now: float,
    cooldown_min: float = 15,
    draining: frozenset[str] = frozenset(),
) -> list[ScaleDecision]:
    """Apply `decide` to every probed host (mutates `state`); returns every host's decision."""
    state.ensure(config)
    out: list[ScaleDecision] = []
    for name, host in config.hosts.items():
        current = state.targets.get(name, host.start)
        if name not in statuses:
            continue
        new, reason = decide(host, statuses[name], current, queue)
        if new != current and now - state.changed.get(name, 0) < cooldown_min * 60:
            new, reason = current, reason + " (cooldown)"
        if new != current:
            state.changed[name] = now
        state.targets[name] = new
        out.append(ScaleDecision(name, current, new, reason))
    for (
        name
    ) in draining:  # <name>.drain: a host that hosts a struggling service keeps only its minimum
        host = config.hosts.get(name)
        if host and state.targets[name] > host.low:
            out.append(ScaleDecision(name, state.targets[name], host.low, "drain"))
            state.targets[name] = host.low
    return out


def replay_hosts(
    config: FleetConfig, statuses: dict[str, HostStatus], off: frozenset[str]
) -> list[dict]:
    """The TSFP_REPLAY_REMOTE_HOSTS list run.sh reads (replay-hosts.json).

    T3760: no `repo`; the replay syncs its tree into the host's exec dir (registry) and keeps
    staging under `remote_replay.REPLAY_DATA_ROOT`."""
    entries = []
    for name, host in config.hosts.items():
        if host.is_local or host.replay_max <= 0:
            continue
        status = statuses.get(name)
        slots = host.replay_max
        if (
            status is None
            or not status.reachable
            or status.number("mem_avail_gb") < host.mem_floor_gb + 4
        ):
            slots = 0
        if name in off:
            slots = 0
        if slots:
            target = host.replay_target or host.target
            entries.append(
                {
                    "name": host.replay_name,
                    "target": target,
                    "port": host.port,
                    "max_slots": slots,
                }
            )
    return entries


@dataclass(frozen=True)
class RefillPlan:
    host: str
    tag: str
    count: int
    foreground: bool
    reason: str


def plan_refill(
    host: HostConfig,
    status: HostStatus | None,
    target: int,
    queue: int,
    local_disk_gb: float,
    off: bool = False,
) -> RefillPlan | None:
    """How many light workers to launch on `host` this tick, or None (reason lost on purpose: use
    `why`)."""
    why = refill_blocker(host, status, target, queue, local_disk_gb, off)
    if why is not None:
        return None
    assert status is not None  # refill_blocker returned None, so the host is probed
    # Autoscale state is persisted independently of fleet.json and may be stale or malformed.
    # The host's configured maximum is the hard ceiling for every refill, even when a saved
    # target (or caller) is higher. Clamp against observed occupancy before applying the
    # per-tick launch limit.
    effective_target = min(target, host.high)
    gap = effective_target - status.workers
    count = gap if host.launch.max_per_tick is None else min(gap, host.launch.max_per_tick)
    return RefillPlan(
        host.name,
        host.tag,
        count,
        host.launch.foreground,
        f"workers={status.workers} target={target}",
    )


def refill_blocker(
    host: HostConfig,
    status: HostStatus | None,
    target: int,
    queue: int,
    local_disk_gb: float,
    off: bool,
) -> str | None:
    """First reason no launch happens, or None when a launch is due."""
    policy = host.launch
    if off:
        return "off-file"
    if status is None or not status.reachable:
        return "unreachable"
    if queue <= 0:
        return "queue-empty"
    if policy.needs_local_disk_gb is not None and local_disk_gb < policy.needs_local_disk_gb:
        return "local-disk-low"
    if status.overcap:
        return "over-cap"
    if launch_health.failing(status.fields):  # T3763: launched workers complete no step
        return "launch-failing"
    if status.workers >= min(target, host.high):
        return "at-target"
    if status.mem_avail_gb < policy.min_mem_gb:
        return f"mem<{policy.min_mem_gb:g}G"
    if policy.max_load is not None and status.number("load") >= policy.max_load:
        return "load-high"
    return None


def now() -> float:
    return time.time()
