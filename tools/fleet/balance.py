"""T3748 fleet balancer (rule 44, no idle workers): `python3 -m tools.fleet.balance --loop`.

Every tick (default 300 s) it measures the shared headroom (`tools.fleet.headroom`) and, per host:

1. diagnoses an ineligible host and auto-fixes the safe causes: a stale bench is cleared (older
   than its window, a known transient, or written before the last fix of the benching code), a
   drifted memguard is re-provisioned (`tools.fleet provision <host> --only memguard --apply`), a
   host under its disk floor or over its cap gets a capguard run;
2. tracks idleness: load1/cores under IDLE_LOAD with free worker slots, for IDLE_AFTER_S;
3. for an idle eligible host with queued codex work, nudges the controller: a worker-target floor
   in tmp/fleet/balance-nudge.json (read by `Controller.targets` through `apply_nudges`, within
   the host's max, never for an off_file host or the owner PC). Replay, capture, verify and
   prebuild placement follows the headroom cache this writes (least loaded first);
4. writes tmp/fleet/balance-status.json (one line per host: headroom, eligibility, reason, action);
5. emits `HEALTH host-idle <host> <reason>` (once an hour per reason) when it cannot fix the cause.

The owner PC is never a target (rule 26, T3745). Kill switch: tmp/fleet/balance.off. Single
instance: tmp/fleet/balance.lock. Run it under tmp/codexapi/reaper.py (PID 1 does not reap).
"""

from __future__ import annotations

import argparse
import json
import shutil
import subprocess
import sys
import time
from collections.abc import Callable, Mapping, Sequence
from dataclasses import dataclass, field
from pathlib import Path

from tools import host_registry
from tools.agents import canonical_mount, loops
from tools.fleet import exec_load, launch_health, queue_validation, scale
from tools.fleet import headroom as hr
from tools.fleet import status as fleet_status
from tools.fleet.config import FleetConfig, HostConfig, load_config
from tools.fleet.core import Finding, Runner, SingleInstance, flock_held, local_runner, ssh_argv
from tools.fleet.status import HostStatus

IDLE_LOAD = 0.3
IDLE_AFTER_S = 15 * 60
NUDGE_TTL_S = 15 * 60
FIX_COOLDOWN_S = 30 * 60
HEALTH_REPEAT_S = 60 * 60
STATUS_REL = "balance-status.json"
OFF_REL = "balance.off"
REPO_BEHIND_H = 6.0
"""A host repo whose HEAD is older than origin/main's tip by more than this is stale."""


def repo_probe(host_name: str) -> str | None:
    """The host-repo probe command for `host_name`, or None when the registry has no data_root."""
    try:
        data_root = host_registry.host_key(host_name, "data_root")
    except host_registry.RegistryError:
        return None
    return (
        # GIT_NO_LAZY_FETCH=1 on every host git call: host repos are blobless partial clones and a
        # lazy fetch hung 95 processes on the laptop (2026-10-25).
        f"cd {data_root} && export GIT_NO_LAZY_FETCH=1 && "
        "echo dirty=$(git status --porcelain --untracked-files=no | wc -l) "
        "head=$(git rev-parse HEAD) head_ct=$(git log -1 --format=%ct HEAD)"
    )


BENCH_CODE = ("tools/wt/remote_exec.py",)
"""The code that writes benches: a bench older than its last change is from pre-fix code."""
BROKEN_PIPE_BENCH = "run: BrokenPipeError"
"""Only this exact historical bench can use the verified-operation recovery below."""

Spawn = Callable[[str, Sequence[str], float], None]


@dataclass
class State:
    idle_since: dict[str, float] = field(default_factory=dict)
    health_sent: dict[str, float] = field(default_factory=dict)
    fixed_at: dict[str, float] = field(default_factory=dict)
    requeued: dict[str, float] = field(default_factory=dict)  # T3763: id -> when re-queued

    @classmethod
    def load(cls, path: Path) -> State:
        try:
            data = json.loads(path.read_text())
            return cls(
                data["idle_since"], data["health_sent"], data["fixed_at"], data.get("requeued", {})
            )
        except (OSError, ValueError, KeyError, TypeError):
            return cls()

    def dump(self, path: Path) -> None:
        path.write_text(json.dumps(self.__dict__, sort_keys=True) + "\n")


@dataclass
class Row:
    host: str
    eligible: bool
    reason: str
    idle_s: float
    actions: list[str]
    headroom: dict
    repo: str = ""

    def line(self) -> str:
        return json.dumps(
            {
                "host": self.host,
                "eligible": self.eligible,
                "reason": self.reason,
                "idle_min": round(self.idle_s / 60, 1),
                "actions": self.actions,
                "load_per_core": self.headroom.get("load_per_core"),
                "mem_avail_gb": self.headroom.get("mem_avail_gb"),
                "disk_free_gb": self.headroom.get("disk_free_gb"),
                "disk_floor_gb": self.headroom.get("disk_floor_gb"),
                "workers": f"{self.headroom.get('workers')}/{self.headroom.get('max_workers')}",
                "bench": self.headroom.get("bench_reason") or None,
                "bench_age_s": self.headroom.get("bench_age_s"),
                "build": self.headroom.get("build_reason") or "ok",
                "repo": self.repo or "ok",
            },
            sort_keys=True,
        )


def apply_nudges(
    config: FleetConfig, targets: Mapping[str, int], path: Path, now: float, off: set[str]
) -> dict[str, int]:
    """Worker targets with live balancer floors applied, clamped to each host's max.

    Off-file hosts and the owner PC are never raised. Returns a new dict (autoscale keeps its
    own state)."""
    out = dict(targets)
    try:
        nudges = json.loads(path.read_text())
    except (OSError, ValueError):
        return out
    if not isinstance(nudges, dict):
        return out
    for name, entry in nudges.items():
        host = config.hosts.get(name)
        if host is None or host.is_local or name in off or not isinstance(entry, dict):
            continue
        try:
            floor, until = int(entry["target"]), float(entry["until"])
        except (KeyError, TypeError, ValueError):
            continue
        if until > now:
            out[name] = min(host.high, max(out.get(name, host.start), floor))
    return out


def queue_depth(config: FleetConfig) -> int:
    try:
        text = (config.codexapi_dir / "p3queue.txt").read_text()
    except OSError:
        return 0
    return sum(1 for line in text.splitlines() if line.strip())


def code_fixed_at(root: Path, runner: Runner) -> float | None:
    result = runner(
        ["git", "-C", str(root), "log", "-1", "--format=%ct", "origin/main", "--", *BENCH_CODE], 20
    )
    try:
        return float(result.out.strip()) if result.ok else None
    except ValueError:
        return None


def repo_staleness(host: HostConfig, runner: Runner, root: Path, main_ct: float | None) -> str:
    """ "dirty", "behind N" (commits, when more than REPO_BEHIND_H hours behind origin/main) or "".

    Report only: dirty work on a host is never discarded automatically (capguard's fast-forward
    update stops on it, so the host keeps running old tools)."""
    probe = repo_probe(host.name)
    if probe is None:
        return ""
    result = runner(ssh_argv(host.target, host.port, probe), 30)
    fields = dict(part.split("=", 1) for part in result.out.split() if "=" in part)
    if not result.ok or "head" not in fields:
        return ""
    if fields.get("dirty", "0") not in ("", "0"):
        return "dirty"
    try:
        head_ct = float(fields["head_ct"])
    except (KeyError, ValueError):
        return ""
    if main_ct is None or main_ct - head_ct <= REPO_BEHIND_H * 3600:
        return ""
    count = runner(
        ["git", "-C", str(root), "rev-list", "--count", f"{fields['head']}..origin/main"], 20
    )
    behind = count.out.strip() if count.ok and count.out.strip().isdigit() else "?"
    return f"behind {behind}"


def main_commit_time(root: Path, runner: Runner) -> float | None:
    result = runner(["git", "-C", str(root), "log", "-1", "--format=%ct", "origin/main"], 20)
    try:
        return float(result.out.strip()) if result.ok else None
    except ValueError:
        return None


def toolchain_check(root: Path) -> Callable[[HostConfig], str | None]:
    """Build parity per host (I-T3737-4) via remote_prebuilt.toolchain_skip; unknown = ok."""

    def check(host: HostConfig) -> str | None:
        if "build" not in host.registry.get("capabilities", []):
            return None
        try:
            from tools.wt import remote_prebuilt

            worker = remote_prebuilt.workers().get(host.name)
            return remote_prebuilt.toolchain_skip(root, worker) if worker else None
        except Exception:  # noqa: BLE001 - an unreadable fingerprint never benches a host
            return None

    return check


@dataclass
class Balancer:
    config: FleetConfig
    state: State
    spawn: Spawn
    now: float
    dry_run: bool = False
    verified_broken_pipe_hosts: frozenset[str] = frozenset()
    findings: list[Finding] = field(default_factory=list)
    repo_check: Callable[[HostConfig], str] | None = None

    @property
    def state_dir(self) -> Path:
        return self.config.state_dir

    def _cooled(self, key: str) -> bool:
        return self.now - self.state.fixed_at.get(key, 0) >= FIX_COOLDOWN_S

    def _fix(self, key: str, argv: Sequence[str], timeout: float, actions: list[str]) -> None:
        if not self._cooled(key):
            actions.append(f"{key}:cooldown")
            return
        self.state.fixed_at[key] = self.now
        actions.append(key if not self.dry_run else f"{key}:dry-run")
        if not self.dry_run:
            self.spawn(key, argv, timeout)

    def clear_bench(self, name: str, bench: hr.Bench, why: str, actions: list[str]) -> bool:
        actions.append(f"cleared-bench({why}: {bench.reason[:60]})")
        if not self.dry_run:
            (self.config.root / hr.DEGRADED_DIR / f"{name}.json").unlink(missing_ok=True)
        return True

    def repair(
        self, host: HostConfig, room: hr.Headroom, status: HostStatus | None, fixed: float | None
    ) -> tuple[str, list[str]]:
        """Fix what is safe; returns the reason still blocking (empty when eligible) + actions."""
        actions: list[str] = []
        reason = room.reason
        if reason.startswith(("benched:", "stale-code:")):
            bench = hr.read_bench(self.config.root, host.name)
            why = bench.stale_reason(self.now, fixed) if bench else "gone"
            if (
                bench is not None
                and bench.reason == BROKEN_PIPE_BENCH
                and host.name in self.verified_broken_pipe_hosts
            ):
                why = "verified-post-bench-operation"
            if bench is None or (why and self.clear_bench(host.name, bench, why, actions)):
                reason = "disk-floor" if room.disk_free_gb < room.disk_floor_gb else ""
        if not room.reachable or reason == "off_file":
            return reason, actions
        if room.memguard == 0 and host.memguard:
            self._fix(
                f"provision-memguard:{host.name}",
                [
                    "python3",
                    "-m",
                    "tools.fleet",
                    "provision",
                    host.name,
                    "--only",
                    "memguard",
                    "--apply",
                ],
                600,
                actions,
            )
        overcap = status is not None and status.overcap
        # Disk is checked on its own: a bench or other reason must not hide a full disk.
        disk_low = room.disk_free_gb < room.disk_floor_gb
        if (disk_low or overcap) and host.capguard is not None:
            policy = host.capguard
            remote = (
                f"MIN_FREE_GB={policy.min_free_gb} "
                # cargo-target and input-cache caps come from the provisioned ~/csup/capguard.env
                # (registry target_cap_gb; the old cargo_target_cap_gb default 30 was no key).
                f"sh ~/csup/capguard.sh {policy.cap_gb} >> ~/csup/capguard.log 2>&1"
            )
            self._fix(
                f"capguard:{host.name}", ssh_argv(host.target, host.port, remote), 3000, actions
            )
        return reason, actions

    def nudge(self, host: HostConfig, status: HostStatus, target: int, depth: int) -> str:
        """Raise the worker-target floor of an idle eligible host; returns the action or ""."""
        if host.is_local or target >= host.high:
            return ""
        step = host.launch.max_per_tick or 3
        new = min(host.high, max(target, status.workers) + step)
        path = self.state_dir / "balance-nudge.json"
        try:
            nudges = json.loads(path.read_text())
        except (OSError, ValueError):
            nudges = {}
        nudges = {k: v for k, v in nudges.items() if float(v.get("until", 0)) > self.now}
        nudges[host.name] = {"target": new, "until": self.now + NUDGE_TTL_S, "queue": depth}
        if not self.dry_run:
            path.write_text(json.dumps(nudges, sort_keys=True) + "\n")
        return f"nudge-target {target}->{new}" + (":dry-run" if self.dry_run else "")

    def health(self, name: str, reason: str, kind: str = "host-idle") -> None:
        key = f"{name}:{reason}" if kind == "host-idle" else f"{kind}:{name}:{reason}"
        if self.now - self.state.health_sent.get(key, 0) < HEALTH_REPEAT_S:
            return
        self.state.health_sent[key] = self.now
        self.findings.append(Finding(kind, f"{name} {reason}"))

    def tick(
        self,
        rooms: Mapping[str, hr.Headroom],
        statuses: Mapping[str, HostStatus],
        targets: Mapping[str, int],
        depth: int,
        fixed: float | None,
        local_disk_gb: float,
    ) -> list[Row]:
        rows: list[Row] = []
        for name, host in self.config.hosts.items():
            room = rooms[name]
            if host.is_local:  # rule 26 / T3745: measured, never a balancing target
                rows.append(Row(name, False, "owner-pc-never-overflow", 0, [], room.to_json()))
                continue
            status = statuses.get(name)
            reason, actions = self.repair(host, room, status, fixed)
            idle = (  # rule 47: no worker slots on a remote host, idle is its load alone
                room.reachable and reason != "off_file" and room.load_per_core < IDLE_LOAD
            )
            if idle:
                self.state.idle_since.setdefault(name, self.now)
            else:
                self.state.idle_since.pop(name, None)
            idle_s = self.now - self.state.idle_since[name] if idle else 0.0
            if idle_s >= IDLE_AFTER_S:
                cause = reason
                if not cause and status is not None:
                    # rule 47 (T3769): remote hosts are execution-only; light Codex workers run on
                    # the owner PC, so an idle host is one below its floor of exec jobs, and no
                    # worker target is nudged for it (that fired host-idle at-max(0) forever)
                    # only while runnable work waits, for 20 min (I-T3769-3)
                    have, want = exec_load.exec_jobs(status.fields), host.min_exec_jobs
                    waiting = exec_load.work_waiting(
                        self.config.root, self.config.codexapi_dir, self.now
                    )
                    if have < want and waiting and idle_s >= exec_load.UNDERUSED_AFTER_S:
                        cause = f"exec-jobs-{have}<{want}"
                if cause and cause != "off_file":
                    self.health(name, cause)
                    actions.append(f"HEALTH host-idle {cause}")
            repo = ""
            if self.repo_check is not None and room.reachable and reason != "off_file":
                repo = self.repo_check(host)
                if repo:
                    self.health(name, repo, "host-repo-stale")
                    actions.append(f"HEALTH host-repo-stale {repo}")
            rows.append(Row(name, not reason, reason, idle_s, actions, room.to_json(), repo))
        return rows


def verify_historical_broken_pipe(
    config: FleetConfig, runner: Runner, now: float
) -> frozenset[str]:
    """Clear only this exact bench with later green cache evidence and a fresh live probe."""
    bench = hr.read_bench(config.root, "deck")
    host = config.hosts.get("deck")
    if (
        bench is None
        or bench.reason != BROKEN_PIPE_BENCH
        or host is None
        or host.is_local
        or (host.off_file and (config.codexapi_dir / host.off_file).exists())
    ):
        return frozenset()
    try:
        from tools import build_health, build_health_policy
        from tools.wt import core

        receipt_path = core.main_root(config.root) / build_health_policy.RECEIPT
        receipt = build_health_policy.read_object(receipt_path)
        measured_at = receipt.get("measured_at")
        if (
            isinstance(measured_at, bool)
            or not isinstance(measured_at, (int, float))
            or measured_at <= bench.at
            or measured_at > now
        ):
            return frozenset()
        if build_health_policy.check(config.root, now=now):
            return frozenset()
        deck = receipt.get("hosts", {}).get("deck")
        if not isinstance(deck, dict) or build_health.assess_probe(deck, warm_seconds=5):
            return frozenset()
        status = fleet_status.remote_status(host, runner)
        if not status.reachable or status.disk_free_gb < host.disk_floor_gb:
            return frozenset()
        if hr.mount_floor_breach(status):
            return frozenset()
    except (OSError, ValueError, TypeError, KeyError):
        return frozenset()
    return frozenset({"deck"})


def write_status(path: Path, rows: Sequence[Row], now: float) -> None:
    body = ",\n".join(row.line() for row in rows)
    tmp = path.with_name(path.name + ".new")
    tmp.write_text(f'{{"at": {now:.0f}, "hosts": [\n{body}\n]}}\n')
    tmp.replace(path)


def read_targets(config: FleetConfig) -> dict[str, int]:
    state = scale.AutoscaleState.load(config.state_dir / "autoscale-state.json", config)
    return state.targets


class _Spawner:
    """Background fix commands, logged under tmp/fleet/logs and polled each tick."""

    def __init__(self, root: Path, logs: Path) -> None:
        self.root, self.logs, self.procs = root, logs, []

    def __call__(self, key: str, argv: Sequence[str], timeout: float) -> None:
        self.logs.mkdir(parents=True, exist_ok=True)
        log = (self.logs / f"balance-{key.replace(':', '-')}.log").open("ab")
        self.procs.append(
            subprocess.Popen(  # noqa: S603
                ["timeout", "--kill-after=30", str(int(timeout)), *argv],
                cwd=self.root,
                stdin=subprocess.DEVNULL,
                stdout=log,
                stderr=log,
                start_new_session=True,
            )
        )

    def reap(self) -> None:
        self.procs = [proc for proc in self.procs if proc.poll() is None]


REQUEUE_TTL_S = 24 * 3600


def requeue_failed_lanes(
    config: FleetConfig,
    statuses: Mapping[str, HostStatus],
    state: State,
    now: float,
    dry_run: bool = False,
) -> list[str]:
    """T3763: ids handed back by lanes that failed every step go back to the head of the queue,
    once per id per REQUEUE_TTL_S (the hosts' requeue files are append-only)."""
    state.requeued = {k: v for k, v in state.requeued.items() if now - v < REQUEUE_TTL_S}
    try:
        local = (config.codexapi_dir / launch_health.REQUEUE).read_text()
    except OSError:
        local = ""
    ids: list[str] = []
    for name, status in statuses.items():
        host = config.hosts.get(name)
        text = local if host is not None and host.is_local else ""
        ids += launch_health.requeue_ids(status.fields, text)
    fresh = [item for item in dict.fromkeys(ids) if item not in state.requeued]
    if not fresh or dry_run:
        return fresh
    queue_validation.requeue(config.codexapi_dir / "p3queue.txt", fresh)
    state.requeued.update({item: now for item in fresh})
    return fresh


def run_tick(
    config: FleetConfig, runner: Runner, spawn: Spawn, now: float, dry_run: bool = False
) -> list[Row]:
    state_path = config.state_dir / "balance-state.json"
    state = State.load(state_path)
    fixed = code_fixed_at(config.root, runner)
    statuses = hr.probe_statuses(config, runner, now)
    rooms = hr.measure(
        config, statuses, now, code_fixed_at=fixed, toolchain=toolchain_check(config.root)
    )
    hr.write_cache(config.root, rooms, now, hr.aliases(config))
    main_ct = main_commit_time(config.root, runner)
    balancer = Balancer(
        config,
        state,
        spawn,
        now,
        dry_run,
        verified_broken_pipe_hosts=verify_historical_broken_pipe(config, runner, now),
        repo_check=lambda host: repo_staleness(host, runner, config.root, main_ct),
    )
    rows = balancer.tick(
        rooms,
        statuses,
        read_targets(config),
        queue_depth(config),
        fixed,
        shutil.disk_usage(config.root).free / 2**30,
    )
    requeued = requeue_failed_lanes(config, statuses, state, now, dry_run)
    if requeued:
        print(f"balance: re-queued {len(requeued)} id(s) of failed lanes: {' '.join(requeued)}")
    write_status(config.state_dir / STATUS_REL, rows, now)
    if balancer.findings and not dry_run:
        with (config.codexapi_dir / "health.log").open("a") as handle:
            for finding in balancer.findings:
                handle.write(finding.line() + "\n")
    state.dump(state_path)
    return rows


LOOP_NAME = "fleet-balance"  # tools/data/loops.json


def loop_alive(root: Path) -> bool:
    """T3763: the supervisor's liveness probe (the --loop holds this flock while it runs)."""
    return flock_held(load_config(root).state_dir / "balance.lock")


def kill_switch(root: Path) -> Path:
    """T3830: the --loop's kill switch path (tools/data/loops.json `switch`)."""
    return load_config(root).state_dir / OFF_REL


def loop_stopped(root: Path) -> bool:
    """T3777: the --loop's own kill switch (balance.off): the supervisor does not restart it."""
    return kill_switch(root).exists()


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--loop", action="store_true", help="tick forever (single instance)")
    parser.add_argument("--interval", type=float, default=300, help="seconds between ticks")
    parser.add_argument("--dry-run", action="store_true", help="diagnose only, change nothing")
    parser.add_argument("--ticks", type=int, default=0, help="stop after N ticks (0 = forever)")
    args = parser.parse_args(argv)
    config = load_config()
    config.state_dir.mkdir(parents=True, exist_ok=True)
    lock = SingleInstance(config.state_dir / "balance.lock")
    if args.loop and not lock.acquire():
        print("balancer already running (tmp/fleet/balance.lock)", file=sys.stderr)
        return 1
    if args.loop:
        canonical_mount.require(LOOP_NAME, config.root)  # T3784: exits 0 off /workspace
        loops.install_term_handler()  # T3763: SIGTERM unwinds, spawned children are stopped
    spawner = _Spawner(config.root, config.state_dir / "logs")
    runner = local_runner(config.root)
    done = 0
    while True:
        if (config.state_dir / OFF_REL).exists():
            print("tmp/fleet/balance.off present: stopping", file=sys.stderr)
            return 0
        spawner.reap()
        for row in run_tick(load_config(), runner, spawner, time.time(), args.dry_run):
            print(row.line(), flush=True)
        done += 1
        if args.loop:
            loops.tick(LOOP_NAME, config.root)
        if not args.loop or (args.ticks and done >= args.ticks):
            return 0
        deadline = time.time() + args.interval
        while time.time() < deadline and not (config.state_dir / OFF_REL).exists():
            time.sleep(min(5.0, max(0.0, deadline - time.time())))


if __name__ == "__main__":
    raise SystemExit(main())
