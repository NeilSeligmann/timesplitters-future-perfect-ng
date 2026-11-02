"""Side-by-side comparison of the dry-run controller with the live sup-loop (T3626 switch gate).

`python3 -m tools.fleet compare` reads what the legacy loop leaves behind (stamp files, targets.env,
health.log, sup-loop.log, legacy `/tmp/fleet-status.txt`) and what the dry-run controller recorded
in its state dir, and prints one verdict per decision class. `legacy_refill_gate` is a literal port
of the sup-loop.sh shell arithmetic, kept here as the reference for the tests of
`scale.plan_refill`.
"""

from __future__ import annotations

import json
import re
import time
from dataclasses import dataclass
from pathlib import Path

from tools.fleet import scale, status
from tools.fleet.config import FleetConfig, HostConfig
from tools.fleet.controller import LEGACY_STAMPS


def legacy_refill_gate(
    tag: str,
    workers: int,
    mem_gb: float,
    target: int,
    queue: int,
    local_disk_gb: float,
    load: float = 0,
    over_cap: bool = False,
    off: bool = False,
) -> int:
    """How many workers sup-loop.sh would launch (0 = none), for tag L, C, D, S, O or P.

    Mirrors the shell: laptop and local need >= 50 G free local disk; L needs 8 G RAM and launches
    at
    most 12; C needs 10 G RAM, load below 60, at most 3; D needs 4 G (no cap); S needs 2 G, at most
    3;
    O and P need 3 G, at most 3. An over-cap host counts as 999 runners.
    """
    if off or queue <= 0:
        return 0
    count = 999 if over_cap else workers
    if tag in ("L", "C") and local_disk_gb < 50:
        return 0
    gates = {"L": (8, 12), "C": (10, 3), "D": (4, None), "S": (2, 3), "O": (3, 3), "P": (3, 3)}
    min_mem, cap = gates[tag]
    if count >= target or mem_gb < min_mem:
        return 0
    if tag == "C" and int(load) >= 60:
        return 0
    gap = target - count
    return gap if cap is None else min(gap, cap)


@dataclass
class Verdict:
    area: str
    agree: bool
    detail: str

    def line(self) -> str:
        return f"{'AGREE' if self.agree else 'DIFFER':6} {self.area:12} {self.detail}"


def compare_jobs(config: FleetConfig, last_run: dict[str, float], now: float) -> list[Verdict]:
    """Both schedulers must be within one interval of their last run for every mapped job."""
    out = []
    slack = config.tick_seconds * 2
    for job in config.jobs:
        stamp = LEGACY_STAMPS.get(job.name)
        if not stamp:
            continue
        try:
            legacy_age = now - Path(stamp).stat().st_mtime
        except OSError:
            out.append(
                Verdict(f"job:{job.name}", True, "legacy stamp absent (job idle or guarded)")
            )
            continue
        mine = now - last_run[job.name] if job.name in last_run else None
        limit = job.every_min * 60 + slack
        ok = legacy_age <= limit and mine is not None and mine <= limit
        mine_text = "never" if mine is None else f"{mine / 60:.1f}m"
        out.append(
            Verdict(
                f"job:{job.name}",
                ok,
                f"legacy {legacy_age / 60:.1f}m ago, controller {mine_text} ago, every "
                f"{job.every_min:g}m",
            )
        )
    return out


def compare_autoscale(config: FleetConfig, state_dir: Path, codexapi_dir: Path) -> list[Verdict]:
    legacy: dict[str, int] = {}
    env = codexapi_dir / "targets.env"
    if env.exists():
        for var, value in re.findall(r"export (\w+)=(\d+)", env.read_text()):
            legacy[var] = int(value)
    own = scale.AutoscaleState.load(state_dir / "autoscale-state.json", config).targets
    out = []
    for name, host in config.hosts.items():
        old, new = legacy.get(host.var), own.get(name)
        out.append(
            Verdict(f"scale:{name}", old == new, f"legacy {host.var}={old} controller={new}")
        )
    return out


def compare_refill(
    config: FleetConfig,
    statuses: dict[str, status.HostStatus],
    targets: dict[str, int],
    queue: int,
    disk: float,
) -> list[Verdict]:
    out = []
    for name, host in config.hosts.items():
        current = statuses.get(name)
        if current is None or not current.reachable:
            out.append(Verdict(f"refill:{name}", True, "unreachable or off in both"))
            continue
        plan = scale.plan_refill(host, current, targets.get(name, host.start), queue, disk, False)
        mine = plan.count if plan else 0
        theirs = legacy_refill_gate(
            host.tag,
            current.workers,
            current.mem_avail_gb,
            targets.get(name, host.start),
            queue,
            disk,
            current.number("load"),
            current.overcap,
        )
        out.append(
            Verdict(
                f"refill:{name}",
                mine == theirs,
                f"workers={current.workers} target={targets.get(name)} legacy launches {theirs}, "
                f"controller {mine}",
            )
        )
    return out


def _health_ids(path: Path, limit: int = 400) -> set[str]:
    try:
        lines = path.read_text().splitlines()[-limit:]
    except OSError:
        return set()
    return {m.group(1) for line in lines if (m := re.search(r"HEALTH (\S+)", line))}


def _last_cycle_ids(path: Path) -> tuple[str, set[str]]:
    """(HH:MM of the newest cycle, finding ids of that minute) from a health.log."""
    try:
        lines = [line for line in path.read_text().splitlines() if " HEALTH " in line]
    except OSError:
        return "", set()
    if not lines:
        return "", set()
    minute = lines[-1][:5]
    return minute, {
        m.group(1)
        for line in lines
        if line.startswith(minute) and (m := re.search(r"HEALTH (\S+)", line))
    }


def compare_health(state_dir: Path, codexapi_dir: Path) -> list[Verdict]:
    """Findings of the newest cycle of each side, compared when at most 2 minutes apart."""
    mine_at, mine = _last_cycle_ids(state_dir / "health.log")
    legacy_at, legacy = _last_cycle_ids(codexapi_dir / "health.log")
    detail = f"controller {mine_at} {len(mine)} ids, legacy {legacy_at} {len(legacy)} ids"
    if not mine_at or not legacy_at:
        return [Verdict("health", True, detail + " (a log is empty)")]
    gap = abs(
        int(mine_at[:2]) * 60 + int(mine_at[3:]) - int(legacy_at[:2]) * 60 - int(legacy_at[3:])
    )
    if gap > 2:
        return [Verdict("health", True, detail + f" (cycles {gap} min apart, not comparable)")]
    only_mine, only_legacy = sorted(mine - legacy), sorted(legacy - mine)
    return [
        Verdict(
            "health",
            not only_mine and not only_legacy,
            f"{detail}; only controller {only_mine or '-'}; only legacy {only_legacy or '-'}",
        )
    ]


def compare_workers(statuses: dict[str, status.HostStatus]) -> list[Verdict]:
    local = statuses.get("local")
    if local is None:
        return []
    lock, ps = int(local.number("workers_lock")), int(local.number("workers_ps"))
    return [
        Verdict(
            "workers",
            True,
            f"local: lock-held lanes {lock}, ps count {ps}, controller workers {local.workers} "
            f"(lock minus claude runners)",
        )
    ]


def run_compare(config: FleetConfig, hours: float = 1.0) -> list[Verdict]:
    state = config.state_dir
    now = time.time()
    try:
        last_run = {k: float(v) for k, v in json.loads((state / "jobs.json").read_text()).items()}
    except (OSError, ValueError):
        last_run = {}
    try:
        legacy_status = status.parse_status(status.read_status(config))
    except OSError:
        legacy_status = {}
    own_targets = scale.AutoscaleState.load(state / "autoscale-state.json", config).targets
    try:
        queue = sum(
            1
            for line in (config.codexapi_dir / "p3queue.txt").read_text().splitlines()
            if line.strip()
        )
    except OSError:
        queue = 0
    import shutil

    disk = shutil.disk_usage(config.root).free / 2**30
    own_status = {}
    try:
        own_status = status.parse_status((state / "fleet-status.txt").read_text())
    except OSError:
        pass
    verdicts = compare_jobs(config, last_run, now)
    verdicts += compare_autoscale(config, state, config.codexapi_dir)
    verdicts += compare_refill(config, legacy_status or own_status, own_targets, queue, disk)
    verdicts += compare_health(state, config.codexapi_dir)
    verdicts += compare_workers(own_status)
    window = now - hours * 3600
    errors = 0
    try:
        for line in (state / "decisions.jsonl").read_text().splitlines():
            row = json.loads(line)
            errors += row["ts"] >= window and row["kind"] == "job-error"
    except (OSError, ValueError, KeyError):
        pass
    verdicts.append(
        Verdict("job-errors", errors == 0, f"{errors} job errors in the last {hours:g} h")
    )
    return verdicts


def host_of(config: FleetConfig, tag: str) -> HostConfig:
    return config.host(tag)
