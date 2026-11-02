"""The fleet controller: one daemon that replaces tmp/codexapi/sup-loop.sh.

Every tick it reloads its configuration if a file changed, then runs each job of `fleet.json` that
is due. Jobs are either builtins (methods `job_<name>` below, tested as pure logic where possible),
plain commands, or `legacy` supervisor scripts still living in tmp/codexapi (listed in the decision
doc as the remaining lift). In dry-run mode nothing with a side effect runs: every action becomes a
`Decision` line in `decisions.jsonl` so it can be compared with the live sup-loop (`compare.py`).
"""

from __future__ import annotations

import collections
import fcntl
import json
import os
import re
import shlex
import signal
import subprocess
import time
from collections.abc import Callable, Sequence
from dataclasses import asdict, dataclass, field
from pathlib import Path

from tools import datadir, host_registry
from tools.fleet import detectors, queue, queue_validation, scale, status
from tools.fleet.config import ConfigError, ConfigWatcher, FleetConfig, HostConfig, JobSpec
from tools.fleet.core import (
    CmdResult,
    Finding,
    Runner,
    SingleInstance,
    flock_held,
    local_runner,
    ssh_argv,
)
from tools.process_priority import MIN_NICE

# Legacy sup-loop stamp files that seed a job's last-run time at start (so a switch does not
# stampede)
# and that `compare` checks against the controller's own schedule.
# Script-only provision steps for hosts whose heavy provision runs only when the repo is missing.
#: Each HEALTH drift finding -> the provision step that repairs it. Exec-only hosts get only the
#: payload provision, so every repair step must be in it or the finding repeats forever (I-T3865-6:
#: the deck reported host-python-deps-drift every tick for hours).
DRIFT_REPAIR_STEPS = {
    "provisioned-script-drift": "payload",
    "host-python-deps-drift": "python-deps",
    "codex-runner-drift": "codex-runner",
}
PAYLOAD_PROVISION_STEPS = ",".join(
    dict.fromkeys(
        [
            "payload",
            "capguard",
            "lclean",
            "memguard",
            "build-tools",
            "owner-inputs",
            *DRIFT_REPAIR_STEPS.values(),
        ]
    )
)
#: tmp/fleet/capguard-<host>.json: the remote ~/csup/capguard.stamp after each controller pass, read
#: by HEALTH capguard-not-running (cx-capguard-execonly).
CAPGUARD_STAMP_GLOB = "capguard-*.json"


def _remote_dir(path: str) -> str:
    return '"$HOME"/' + shlex.quote(path[2:]) if path.startswith("~/") else shlex.quote(path)


def exec_layout_probe(host: HostConfig) -> str | None:
    """Shell that succeeds when the host's exec-only layout is in place (rule 47): every registry
    layout key is set and names an existing dir. None when a key is missing (full provision)."""
    values = [str(host.registry.get(key) or "") for key in host_registry.HOST_LAYOUT_KEYS]
    if not all(values):
        return None
    return " && ".join(f"test -d {_remote_dir(value)}" for value in values)


def integrity_remote_command(host: HostConfig) -> str | None:
    """Integrity check through the shipped standalone ~/csup/integrity.py against the registry
    data_root (toolchain and interpreters live there); no host repo is entered (rule 47)."""
    root = str(host.registry.get("data_root") or "")
    if not root:
        return None
    return (
        f"python3 ~/csup/integrity.py --root {_remote_dir(root)} check --init --json "
        f"--host {shlex.quote(host.name)}"
    )


LEGACY_STAMPS = {
    "autoscale": "/tmp/autoscale.stamp",
    "health": "/tmp/health-check.stamp",
    "beat": "/tmp/sup-beat.stamp",
    "coord-reap": "/tmp/coord-reap.stamp",
    "hygiene-cargo": "/tmp/hyg-cargo.stamp",
    "hygiene-cache": "/tmp/hyg-cargo.stamp",
    "cargo-cap": "/tmp/cargo-cap.stamp",
    "replay-retention": "/tmp/replay-retention.stamp",
    "integrate-remote": "/tmp/integrate-remote.stamp",
    "legacy-heavy": "/tmp/sup-heavy.stamp",
    "legacy-regress": "/tmp/regress-loop.stamp",
    "legacy-codex-renewal": "/tmp/codex-renewal.stamp",
    "lclean": "/tmp/lclean-server.stamp",
    "probe": "/tmp/fleet-status.stamp",
}


def _tooling_command(command: str) -> bool:
    """Whether a process command line identifies local build/verification tooling."""
    try:
        argv = shlex.split(command)
    except ValueError:
        argv = command.split()
    if not argv:
        return False
    executable = Path(argv[0]).name.lower()
    if executable in {"bash", "dash", "tsfp-bash"}:
        return True
    if executable.startswith(("pytest", "cargo", "rustc", "sccache", "tsfp")):
        return True
    if executable.startswith("python") and "-m" in argv:
        index = argv.index("-m")
        module = argv[index + 1] if index + 1 < len(argv) else ""
        return module == "pytest" or module.startswith("tools.")
    return False


# T3831: busy-loop load generators (I-T3831-1: 28 `while :; do :; done` burners at load 133)
LOAD_GENERATOR_EXECUTABLES = {"yes", "stress", "stress-ng"}
LOAD_GENERATOR_SHELLS = {"sh", "bash", "dash", "zsh", "tsfp-bash"}
BARE_BUSY_LOOP = re.compile(r"while\s+(?::|true)\s*;\s*do\s*(?::|true)?\s*;?\s*done")
LOAD_GENERATOR_MIN_CPU = 50.0
LOAD_GENERATOR_MIN_AGE_S = 30
INHERITED_PRIORITY_MIN_AGE_S = 60


def _load_generator_command(command: str) -> bool:
    """Whether a command line is a CPU burner: `yes`, `stress(-ng)` or a shell bare busy loop."""
    try:
        argv = shlex.split(command)
    except ValueError:
        argv = command.split()
    if not argv:
        return False
    executable = Path(argv[0]).name.lower()
    if executable in LOAD_GENERATOR_EXECUTABLES:
        return True
    if executable in LOAD_GENERATOR_SHELLS:
        return bool(BARE_BUSY_LOOP.search(command))
    return False


def is_load_generator(command: str, cpu_percent: float, age_s: int) -> bool:
    return (
        cpu_percent >= LOAD_GENERATOR_MIN_CPU
        and age_s > LOAD_GENERATOR_MIN_AGE_S
        and _load_generator_command(command)
    )


def _process_cwd_is_workspace(pid: int) -> bool:
    """Limit generic shell matching to commands actually running inside the checkout fleet."""
    try:
        cwd = Path(os.readlink(f"/proc/{pid}/cwd"))
        return cwd.as_posix().startswith(f"{datadir.main_root().as_posix()}/")  # T3755
    except OSError:
        return False


def _tooling_parent_chain(pid: int, processes: dict[int, tuple[int, int, str]]) -> str:
    chain: list[str] = []
    visited: set[int] = set()
    current = pid
    while current and current not in visited and len(chain) < 8:
        visited.add(current)
        process = processes.get(current)
        if process is None:
            break
        parent, nice, command = process
        executable = Path(command.split(maxsplit=1)[0]).name if command else "?"
        chain.append(f"{current}:{executable}(nice={nice})")
        current = parent
    return "<-".join(chain) or str(pid)


def _task_ids(pid: int) -> list[int]:
    try:
        return [int(path.name) for path in Path(f"/proc/{pid}/task").iterdir()]
    except (OSError, ValueError):
        return [pid]


@dataclass
class Decision:
    ts: float
    job: str
    kind: str
    host: str
    detail: str
    executed: bool

    def line(self) -> str:
        return json.dumps(asdict(self), sort_keys=True)


@dataclass
class _Background:
    name: str
    proc: subprocess.Popen[bytes]
    started: float


@dataclass
class Controller:
    watcher: ConfigWatcher
    dry_run: bool = True
    runner: Runner | None = None
    clock: Callable[[], float] = time.time
    decisions: list[Decision] = field(default_factory=list)
    statuses: dict[str, status.HostStatus] = field(default_factory=dict)
    statuses_at: float = 0.0
    last_run: dict[str, float] = field(default_factory=dict)
    autoscale_state: scale.AutoscaleState | None = None
    _background: list[_Background] = field(default_factory=list)
    failures: list[tuple[float, str, int]] = field(default_factory=list)
    _seeded: bool = False
    ticks: int = 0

    # ---- plumbing -------------------------------------------------------------------------------

    @property
    def config(self) -> FleetConfig:
        return self.watcher.current()

    @property
    def run(self) -> Runner:
        if self.runner is None:
            self.runner = local_runner(self.config.root)
        return self.runner

    def state_path(self, name: str) -> Path:
        path = self.config.state_dir / name
        path.parent.mkdir(parents=True, exist_ok=True)
        return path

    def record(
        self, job: str, kind: str, host: str, detail: str, executed: bool = False
    ) -> Decision:
        decision = Decision(self.clock(), job, kind, host, detail, executed and not self.dry_run)
        self.decisions.append(decision)
        with self.state_path("decisions.jsonl").open("a") as handle:
            handle.write(decision.line() + "\n")
        return decision

    def spawn(
        self,
        job: str,
        argv: Sequence[str],
        cwd: Path,
        timeout: float,
        background: bool,
        host: str = "-",
    ) -> None:
        """Run a command (or record it in dry-run). Background jobs log to
        state_dir/logs/<job>.log."""
        wrapped = ["timeout", "--kill-after=30", str(int(timeout)), *argv]
        self.record(job, "run", host, " ".join(argv)[:300], executed=True)
        if self.dry_run:
            return
        if background:
            log = self.state_path(f"logs/{job.replace(':', '-')}.log").open("ab")
            proc = subprocess.Popen(  # noqa: S603
                wrapped,
                cwd=cwd,
                stdin=subprocess.DEVNULL,
                stdout=log,
                stderr=log,
                start_new_session=True,
            )
            self._background.append(_Background(job, proc, self.clock()))
        else:
            subprocess.run(
                wrapped, cwd=cwd, stdin=subprocess.DEVNULL, check=False, timeout=timeout + 60
            )  # noqa: S603

    def reap_background(self) -> None:
        """Collect finished children (no zombies under a non-reaping PID 1) and note failures."""
        alive = []
        for bg in self._background:
            code = bg.proc.poll()
            if code is None:
                alive.append(bg)
            elif code != 0:
                self.failures.append((self.clock(), bg.name, code))
                self.record(bg.name, "job-failed", "-", f"exit {code}")
        self._background = alive
        horizon = self.clock() - 3600
        self.failures = [item for item in self.failures if item[0] >= horizon]

    def ssh(self, host: HostConfig, remote: str, timeout: float = 30) -> CmdResult:
        return self.run(ssh_argv(host.target, host.port, remote), timeout)

    def off(self, host: HostConfig) -> bool:
        return bool(host.off_file) and (self.config.codexapi_dir / str(host.off_file)).exists()

    # ---- scheduling -----------------------------------------------------------------------------

    def _load_last_run(self) -> None:
        path = self.state_path("jobs.json")
        try:
            self.last_run = {
                key: float(value) for key, value in json.loads(path.read_text()).items()
            }
        except (OSError, ValueError):
            self.last_run = {}
        for (
            name,
            stamp,
        ) in LEGACY_STAMPS.items():  # seed from the sup-loop stamps: no stampede at the switch
            if name not in self.last_run:
                try:
                    self.last_run[name] = Path(stamp).stat().st_mtime
                except OSError:
                    pass
        self._seeded = True

    def _save_last_run(self) -> None:
        self.state_path("jobs.json").write_text(json.dumps(self.last_run, sort_keys=True))

    def due(self, job: JobSpec, now: float) -> bool:
        return now - self.last_run.get(job.name, 0) >= job.every_min * 60 - 1

    def tick(self) -> list[Decision]:
        """One scheduling pass. Never raises: a failing job is recorded and the rest still run."""
        if not self._seeded:
            self._load_last_run()
        start = len(self.decisions)
        config = self.config
        if self.watcher.error:
            self.record("config", "finding", "-", f"fleet-config-invalid {self.watcher.error}")
        now = self.clock()
        self.reap_background()
        for job in config.jobs:
            if not self.due(job, now):
                continue
            if job.when_repo_file and not (config.root / job.when_repo_file).exists():
                continue
            if job.when_file and not (config.codexapi_dir / job.when_file).exists():
                continue
            if (
                job.needs_fresh_status_min
                and now - self.statuses_at > job.needs_fresh_status_min * 60
            ):
                continue
            self.last_run[job.name] = now
            try:
                self._run_job(job)
            except Exception as error:  # noqa: BLE001 - one broken job must not stop the loop
                self.record(job.name, "job-error", "-", f"{type(error).__name__}: {error}")
        self._save_last_run()
        self.ticks += 1
        self._write_heartbeat(now)
        return self.decisions[start:]

    def _run_job(self, job: JobSpec) -> None:
        config = self.config
        if job.builtin:
            getattr(self, f"job_{job.builtin}")()
        elif job.legacy:
            self.spawn(
                job.name, ["sh", job.legacy], config.codexapi_dir, job.timeout, job.background
            )
        else:
            self.spawn(job.name, job.command, config.resolve(job.cwd), job.timeout, job.background)

    def _write_heartbeat(self, now: float) -> None:
        beat = {
            "pid": os.getpid(),
            "dry_run": self.dry_run,
            "last_tick": now,
            "ticks": self.ticks,
            "config_reloads": self.watcher.reloads,
            "config_error": self.watcher.error,
            "background": [bg.name for bg in self._background],
        }
        self.state_path("controller.json").write_text(json.dumps(beat))

    # ---- builtin jobs ---------------------------------------------------------------------------

    def queue_file(self, name: str) -> Path:
        return self.config.codexapi_dir / name

    def queue_length(self) -> int:
        try:
            return sum(
                1
                for line in self.queue_file("p3queue.txt").read_text().splitlines()
                if line.strip()
            )
        except OSError:
            return 0

    def job_probe(self) -> None:
        self.statuses = status.collect(self.config, self.run)
        self.statuses_at = self.clock()
        text = status.format_status(self.statuses)
        target = self.state_path("fleet-status.txt") if self.dry_run else self.config.status_file
        target.parent.mkdir(parents=True, exist_ok=True)
        tmp = target.with_name(target.name + ".new")
        tmp.write_text(text)
        tmp.replace(target)
        down = [name for name, st in self.statuses.items() if not st.reachable]
        if down:
            self.record("probe", "finding", ",".join(down), "unreachable")

    def job_autoscale(self) -> None:
        config = self.config
        path = self.state_path("autoscale-state.json")
        if self.autoscale_state is None:
            seed = path if path.exists() else config.codexapi_dir / "autoscale-state.json"
            self.autoscale_state = scale.AutoscaleState.load(seed, config)
        draining = frozenset(
            name for name in config.hosts if (config.codexapi_dir / f"{name}.drain").exists()
        )
        decisions = scale.autoscale(
            config,
            self.statuses,
            self.autoscale_state,
            self.queue_length(),
            self.clock(),
            draining=draining,
        )
        for item in decisions:
            if item.changed:
                self.record(
                    "autoscale",
                    "target",
                    item.host,
                    f"{item.old}->{item.new} {item.reason}",
                    executed=True,
                )
        path.write_text(self.autoscale_state.dump())
        if self.dry_run:
            return
        base = config.codexapi_dir
        (base / "targets.env.new").write_text(
            "".join(
                f"export {h.var}={self.autoscale_state.targets[n]}\n"
                for n, h in config.hosts.items()
            )
        )
        (base / "targets.env.new").replace(base / "targets.env")
        off = frozenset(name for name in config.hosts if (base / f"{name}.off").exists())
        replay = scale.replay_hosts(config, self.statuses, off)
        (base / "replay-hosts.json.new").write_text(json.dumps(replay))
        (base / "replay-hosts.json.new").replace(base / "replay-hosts.json")

    def targets(self) -> dict[str, int]:
        if self.autoscale_state is None:
            self.job_autoscale_seed()
        assert self.autoscale_state is not None
        self.autoscale_state.ensure(self.config)
        from tools.fleet.balance import apply_nudges  # T3748 balancer: idle host + queued work

        return apply_nudges(
            self.config,
            self.autoscale_state.targets,
            self.state_path("balance-nudge.json"),
            self.clock(),
            {name for name, h in self.config.hosts.items() if self.off(h)},
        )

    def job_autoscale_seed(self) -> None:
        path = self.state_path("autoscale-state.json")
        seed = path if path.exists() else self.config.codexapi_dir / "autoscale-state.json"
        self.autoscale_state = scale.AutoscaleState.load(seed, self.config)

    def job_refill(self) -> None:
        config = self.config
        depth = self.queue_length()
        import shutil

        disk = shutil.disk_usage(config.root).free / 2**30
        targets = self.targets()
        for name, host in config.hosts.items():
            if not host.is_local:
                continue  # rule 47 (T3769): remote hosts are execution-only, no light workers
            plan = scale.plan_refill(
                host, self.statuses.get(name), targets[name], depth, disk, self.off(host)
            )
            if plan is None:
                continue
            # sup-refill.sh gates on the same numbers: fleet.json is their one source (rule 45)
            gates = [f"LIGHT_LOCAL_MAX={host.high}"]
            if host.launch.max_load is not None:
                gates.append(f"LIGHT_LOAD_MAX={host.launch.max_load:g}")
            self.spawn(
                "refill:local",
                ["env", *gates, "sh", "sup-refill.sh", plan.tag, str(plan.count)],
                config.codexapi_dir,
                900,
                not plan.foreground,
                name,
            )

    def job_tier2_feed(self) -> None:
        """Top up the light queue with Tier 2 items when it runs low. queue-feed only offers REA
        unknown ids, nearly all answered or rejected now, and the generator call lived in the
        retired sup-feed.sh, so the light queue stayed empty (2026-10-30, light-queue-empty)."""
        low = int(self.config.queue.get("low_water", 15))
        depth = self.queue_length()
        if depth >= low:
            return
        argv = [
            "python3",
            "-m",
            "tools.fleet.tier2_queue",
            "generate",
            "--root",
            str(self.config.root),
            "--append",
            "--max-items",
            str(2 * low),
        ]
        result = self.run(argv, 300)
        kind = "append" if result.ok else "finding"
        self.record("tier2-feed", kind, "-", f"depth={depth} low={low} ok={result.ok}", result.ok)

    def job_queue_feed(self) -> None:
        config = self.config
        low = int(config.queue.get("low_water", 15))
        index = self.run(
            [
                "git",
                "-C",
                str(config.root),
                "show",
                "origin/main:docs/data/rea-continuous/index.json",
            ],
            60,
        )
        if not index.ok:
            self.record("queue-feed", "finding", "-", "index.json unreadable on origin/main")
            return
        all_rows = queue.parse_index(index.out)
        rows, rejected_feed = queue_validation.resolvable_work_rows(config.root, all_rows)
        branches_run = self.run(
            ["git", "-C", str(config.root), "ls-remote", "origin", "refs/heads/task/cx-*"], 60
        )
        if not branches_run.ok:
            return
        branches = queue.parse_branch_names(branches_run.out)
        dirs = [p.name for p in config.codexapi_dir.glob("CX-*")]
        for host in config.remote_hosts():
            if self.off(host):
                continue
            listing = self.ssh(host, "ls -d ~/csup/codexapi/CX-* ~/csup/wt-cx-*", 30)
            dirs.extend(listing.out.split())
        branches |= queue.normalize_worker_dirs(dirs)
        queued_rows = (
            self.queue_file("p3queue.txt").read_text().split()
            if self.queue_file("p3queue.txt").exists()
            else []
        )
        valid_queued, rejected_queued = queue_validation.resolvable_ids(
            config.root, queued_rows, all_rows
        )
        queued = set(valid_queued)
        depth = len(valid_queued)
        if depth >= low:
            if not self.dry_run:
                queue_validation.write_validation_report(
                    config.codexapi_dir,
                    depth,
                    {**rejected_feed, **rejected_queued},
                    before_count=len(queued_rows),
                )
            return
        launched_list = (
            self.queue_file("launched.txt").read_text().split()
            if self.queue_file("launched.txt").exists()
            else []
        )
        bad = (
            set(self.queue_file("bad-ids.txt").read_text().split())
            if self.queue_file("bad-ids.txt").exists()
            else set()
        )
        valid_bad, _ = queue_validation.resolvable_ids(config.root, list(bad), all_rows)
        bad.difference_update(valid_bad)
        feed, seen, remaining = queue.select_feed(
            rows, branches, set(launched_list), queued, int(config.queue.get("feed_batch", 60))
        )
        retry = queue.select_retry(rows, launched_list, bad, queued | set(feed), branches)
        extra = feed + retry
        inv: list[str] = []
        if depth + len(extra) < int(config.queue.get("inv_low_water", 10)):
            inv = queue.select_inv(
                rows,
                _inv_seen(self.queue_file("launched-inv.txt")),
                bad,
                queued | set(extra),
                self.clock(),
                float(config.queue.get("inv_repeat_hours", 24)),
                int(config.queue.get("inv_batch", 30)),
            )
        ids = extra + inv
        rejected = {**rejected_feed, **rejected_queued}
        self.record(
            "queue-feed",
            "append",
            "-",
            f"feed={len(feed)} retry={len(retry)} inv={len(inv)} remaining={remaining} "
            f"depth={depth} rejected={len(rejected)} after={depth + len(ids)}",
            executed=True,
        )
        if self.dry_run:
            return
        self.state_path("feed-remaining.txt").write_text(str(remaining))
        _locked_append(Path("/tmp/p3queue.lock"), self.queue_file("p3queue.txt"), ids)
        queue_validation.write_validation_report(
            config.codexapi_dir,
            depth + len(ids),
            rejected,
            before_count=len(queued_rows),
            added_count=len(ids),
        )
        if seen:
            _append_lines(self.queue_file("launched.txt"), seen)
        if inv:
            _append_lines(
                self.queue_file("launched-inv.txt"), [f"{i} {self.clock():.0f}" for i in inv]
            )

    def job_health(self) -> None:
        ctx = detectors.default_context(
            self.config, self.run, self.statuses, self.clock(), self.state_path("health-state")
        )
        findings = detectors.run_all(ctx)
        findings.extend(self.controller_findings())
        log = (
            self.state_path("health.log")
            if self.dry_run
            else self.config.codexapi_dir / "health.log"
        )
        stamp = time.strftime("%T")
        with log.open("a") as handle:
            for finding in findings:
                handle.write(finding.line(stamp) + "\n")
        for finding in findings:
            self.record("health", "finding", "-", f"{finding.ident} {finding.detail}"[:300])

    def controller_findings(self) -> list[Finding]:
        out = []
        if self.watcher.error:
            out.append(Finding("fleet-config-invalid", self.watcher.error[:200]))
        repeated = collections.Counter(name for _, name, _ in self.failures)
        for name, count in sorted(repeated.items()):
            if count >= 2:
                out.append(Finding("fleet-job-failed", f"{name} failed {count}x in the last hour"))
        stale = [bg.name for bg in self._background if self.clock() - bg.started > 7200]
        if stale:
            out.append(Finding("fleet-background-stuck", ",".join(stale)))
        return out

    def job_beat(self) -> None:
        """Keep the claims of live Codex workers fresh (replaces sup-beat.sh); flock-first
        liveness."""
        config = self.config
        listing = self.run(["python3", "-m", "tools.agents.coord", "list"], 60)
        if not listing.ok:
            return
        claude_agents: set[str] = set()
        agents_file = config.codexapi_dir / "claude-agents.txt"
        if agents_file.exists():
            claude_agents = set(agents_file.read_text().split())
        ctx = detectors.default_context(
            config, self.run, self.statuses, self.clock(), self.state_path("health-state")
        )
        for line in listing.out.splitlines():
            parts = line.split()
            if len(parts) < 2:
                continue
            task, owner = parts[0], parts[1]
            lane = owner.removeprefix("codex-").removeprefix("cx-")
            live = (
                owner.startswith("codex-") and detectors.lane_alive(ctx, lane)
            ) or owner in claude_agents
            if live:
                self.record("beat", "beat", "-", f"{task} {owner}", executed=True)
                if not self.dry_run:
                    self.run(
                        ["python3", "-m", "tools.agents.coord", "beat", task, "--owner", owner], 30
                    )

    def job_renice(self) -> None:
        listing = self.run(["ps", "-eo", "uid=,pid=,ppid=,pgid=,ni=,etimes=,pcpu=,args="], 20)
        uid = str(os.getuid())
        processes: dict[int, tuple[int, int, str]] = {}
        tooling: list[tuple[int, int, int, str]] = []
        ages: dict[int, int] = {}
        burners: list[tuple[int, str]] = []
        groups: set[str] = set()
        for line in listing.out.splitlines():
            parts = line.split(None, 7)
            if len(parts) != 8:
                continue
            process_uid, pid_text, parent_text, group, nice_text, age_text, cpu_text, command = (
                parts
            )
            if process_uid != uid:
                continue
            try:
                pid, parent, nice = int(pid_text), int(parent_text), int(nice_text)
                age, cpu = int(age_text), float(cpu_text)
            except ValueError:
                continue
            processes[pid] = (parent, nice, command)
            ages[pid] = age
            if is_load_generator(command, cpu, age):
                burners.append((pid, command))
            if command.startswith("sh run.sh CX-"):
                groups.add(group)
            if _tooling_command(command) and _process_cwd_is_workspace(pid):
                tooling.append((pid, parent, nice, command))

        affected = 0
        repair_details: list[str] = []
        for pid, _parent, process_nice, _command in tooling:
            low_threads: list[tuple[int, int]] = []
            for task_id in _task_ids(pid):
                try:
                    nice = os.getpriority(os.PRIO_PROCESS, task_id)
                except (AttributeError, OSError):
                    nice = process_nice if task_id == pid else MIN_NICE
                if nice < MIN_NICE:
                    low_threads.append((task_id, nice))
            if not low_threads:
                continue

            outcomes: list[str] = []
            for task_id, _nice in low_threads:
                if self.dry_run:
                    outcomes.append(f"{task_id}:would-renice")
                    continue
                renice = self.run(["renice", "--priority", "15", "--pid", str(task_id)], 20)
                io = self.run(["ionice", "-c", "3", "-p", str(task_id)], 20)
                outcomes.append(
                    f"{task_id}:{'ok' if renice.ok and io.ok else f'failed({renice.rc},{io.rc})'}"
                )

            chain = ""
            if ages.get(pid, 0) > INHERITED_PRIORITY_MIN_AGE_S:
                chain = f" chain={_tooling_parent_chain(pid, processes)}"  # T3831: find the source
            repair_details.append(
                f"{pid}[{','.join(f'{tid}:{nice}' for tid, nice in low_threads)}]="
                f"{','.join(outcomes)}{chain}"
            )
            affected += 1

        # Keep the operational log useful during a renice storm: one aggregate line/hour.
        if repair_details:
            marker = self.state_path("renice-health-hour")
            now = self.clock()
            try:
                last = float(marker.read_text().strip())
            except (OSError, ValueError):
                last = float("-inf")
            if now - last >= 3600:
                detail = (
                    f"repaired_processes={affected} action="
                    f"{'would-renice15' if self.dry_run else 'renice15+ionice-idle'} "
                    f"details={' ; '.join(repair_details[:40])}"
                )
                log = (
                    self.state_path("health.log")
                    if self.dry_run
                    else self.config.codexapi_dir / "health.log"
                )
                detectors.append_health_log([Finding("tooling-priority-repaired", detail)], log)
                self.record("renice", "health", "local", detail, executed=not self.dry_run)
                marker.parent.mkdir(parents=True, exist_ok=True)
                marker.write_text(str(now))

        self._repair_load_generators(burners, processes)
        self.record(
            "renice",
            "renice",
            "local",
            f"{affected} low-priority tooling processes; {len(groups)} runner groups",
            executed=True,
        )
        if not self.dry_run:
            for group in sorted(groups):
                self.run(["renice", "--priority", "15", "--pgrp", group], 20)
                self.run(["ionice", "-c", "3", "-P", group], 20)

    def _repair_load_generators(
        self, burners: list[tuple[int, str]], processes: dict[int, tuple[int, int, str]]
    ) -> None:
        """T3831: renice CPU burners to 19 + idle IO and report each once; never kill them, the
        orchestrator decides (I-T3831-1)."""
        if not burners:
            return
        marker = self.state_path("load-generators-reported")
        try:
            reported = set(marker.read_text().split())
        except OSError:
            reported = set()
        findings: list[Finding] = []
        for pid, command in burners:
            if not self.dry_run:
                self.run(["renice", "--priority", "19", "--pid", str(pid)], 20)
                self.run(["ionice", "-c", "3", "-p", str(pid)], 20)
            if str(pid) in reported:
                continue
            reported.add(str(pid))
            findings.append(
                Finding(
                    "local-load-generator",
                    f"{pid} {command[:160]} chain={_tooling_parent_chain(pid, processes)} "
                    "action=renice19+ionice-idle, kill by PID if not wanted "
                    "(docs/incidents/I-T3831-1.md)",
                )
            )
        log = (
            self.state_path("health.log")
            if self.dry_run
            else self.config.codexapi_dir / "health.log"
        )
        detectors.append_health_log(findings, log)
        live = {str(pid) for pid in processes}
        marker.parent.mkdir(parents=True, exist_ok=True)
        marker.write_text("\n".join(sorted(reported & live)))

    def job_provision_due(self) -> None:
        config = self.config
        for host in config.remote_hosts():
            every = host.provision_every_min
            if not every or self.off(host):
                continue
            key = f"provision:{host.name}"
            now = self.clock()
            if now - self.last_run.get(key, 0) < every * 60:
                continue
            self.last_run[key] = now
            # provision_only_if_repo_missing (historic name): skip the heavy provision while the
            # exec-only layout exists; hosts hold no repo (rule 47, cx-capguard-execonly).
            probe = exec_layout_probe(host) if host.provision_only_if_repo_missing else None
            if probe is not None and self.ssh(host, probe, 20).ok:
                self.payload_provision_due(host, now)
                continue
            self.spawn(
                key,
                ["python3", "-m", "tools.fleet", "provision", host.name, "--apply"],
                config.root,
                2400,
                True,
                host.name,
            )

    def payload_provision_due(self, host: HostConfig, now: float) -> None:
        """A host provisioned only while its repo is missing still refreshes its installed
        scripts (I-T3678: the deck kept a stale capguard.sh with no input-cache evict)."""
        every = host.payload_provision_every_min
        key = f"provision-payload:{host.name}"
        if not every or now - self.last_run.get(key, 0) < every * 60:
            return
        self.last_run[key] = now
        self.spawn(
            key,
            ["python3", "-m", "tools.fleet", "provision", host.name, "--apply"]
            + ["--only", PAYLOAD_PROVISION_STEPS],
            self.config.root,
            600,
            True,
            host.name,
        )

    def job_capguard_due(self) -> None:
        for host in self.config.remote_hosts():
            policy = host.capguard
            if policy is None or self.off(host):
                continue
            key = f"capguard:{host.name}"
            now = self.clock()
            if now - self.last_run.get(key, 0) < policy.every_min * 60:
                continue
            self.last_run[key] = now
            remote = (
                f"MIN_FREE_GB={policy.min_free_gb} "
                f"sh ~/csup/capguard.sh {policy.cap_gb} "
                ">> ~/csup/capguard.log 2>&1; cat ~/csup/capguard.stamp"
            )
            target = f"tmp/fleet/capguard-{host.name}.json"
            command = shlex.join(ssh_argv(host.target, host.port, remote))
            self.spawn(
                key,
                [
                    "sh",
                    "-c",
                    f"mkdir -p tmp/fleet && {command} > {target}.new; "
                    f"grep -q '\"ts\"' {target}.new && mv {target}.new {target} "
                    f"|| rm -f {target}.new",
                ],
                self.config.root,
                3000,
                True,
                host.name,
            )

    def job_codex_sessions_due(self) -> None:
        """T3678: archive stale session logs on enabled remote hosts; host off files win."""
        for host in self.config.remote_hosts():
            if self.off(host):
                continue
            self.spawn(
                f"codex-sessions:{host.name}",
                ssh_argv(host.target, host.port, "sh ~/csup/rotate-codex-sessions.sh"),
                self.config.root,
                900,
                True,
                host.name,
            )

    def job_integrity_due(self) -> None:
        """T3697: hourly `tools.fleet.integrity check` on each remote; the JSON status lands in
        tmp/fleet/integrity-remote-<host>.json for detect_integrity_changed."""
        for host in self.config.remote_hosts():
            if self.off(host):
                continue
            key = f"integrity:{host.name}"
            now = self.clock()
            if now - self.last_run.get(key, 0) < 3600:
                continue
            self.last_run[key] = now
            remote = integrity_remote_command(host)
            if remote is None:
                continue
            target = f"tmp/fleet/integrity-remote-{host.name}.json"
            command = shlex.join(ssh_argv(host.target, host.port, remote))
            self.spawn(
                key,
                [
                    "sh",
                    "-c",
                    f"mkdir -p tmp/fleet && {command} > {target}.new; "
                    f"grep -q '\"state\"' {target}.new && mv {target}.new {target} "
                    f"|| rm -f {target}.new",
                ],
                self.config.root,
                300,
                True,
                host.name,
            )

    def job_lclean(self) -> None:
        for host in self.config.remote_hosts():
            if host.lclean and not self.off(host):
                try:
                    data_root = host_registry.host_key(host.name, "data_root")
                except host_registry.RegistryError:
                    continue
                target_cap = host.registry.get("target_cap_gb")
                target_dir = host.registry.get("target_dir")
                if not target_dir:
                    workdir = host.registry.get("workdir")
                    target_dir = f"{str(workdir).rstrip('/')}/cargo-target" if workdir else ""
                if (
                    isinstance(target_cap, bool)
                    or not isinstance(target_cap, (int, float))
                    or target_cap <= 0
                    or not isinstance(target_dir, str)
                    or not target_dir
                ):
                    self.log(f"lclean:{host.name}: missing valid registry cargo-target cap/path")
                    continue
                target_arg = shlex.quote(target_dir)
                self.spawn(
                    f"lclean:{host.name}",
                    ssh_argv(
                        host.target,
                        host.port,
                        f"sh ~/csup/lclean.sh {data_root} {target_cap:g} {target_arg}",
                    ),
                    self.config.root,
                    1500,
                    True,
                    host.name,
                )

    def job_sync_git(self) -> None:
        """Push the local main when it is ahead of origin (remote hosts get no git, rule 47)."""
        config = self.config
        # Rule 47 (I-T3769-4): remote hosts are execution-only; no git is pushed to them. The push
        # into rea-codex2/repo left orphaned receive-pack/rev-list processes loading the deck.
        # Count main's own commits: `status -sb` described whatever branch the checkout had, so a
        # landing's c1/manual-* branch (ahead) pushed a main that was behind, failing (2026-10-30).
        ahead = self.run(
            ["git", "-C", str(config.root), "rev-list", "--count", "origin/main..main"], 30
        )
        if ahead.ok and ahead.out.strip().isdigit() and int(ahead.out.strip()) > 0:
            self.spawn(
                "push-main",
                ["git", "-C", str(config.root), "push", "-q", "origin", "main"],
                config.root,
                120,
                True,
            )

    def job_prebuilt(self) -> None:
        worktree = self.config.root / "tmp/wt/prebuilt-builder"
        if (
            not worktree.is_dir()
            or "tools.wt prebuilt build" in self.run(["ps", "-eo", "args"], 20).out
        ):
            return
        if self.dry_run:
            self.record(
                "prebuilt",
                "check",
                "local",
                "would refresh prebuilt-builder and build when missing",
            )
            return
        if not self.run(["git", "-C", str(worktree), "fetch", "-q", "origin", "main"], 120).ok:
            return
        self.run(["git", "-C", str(worktree), "checkout", "-q", "--detach", "origin/main"], 60)
        if not self.run(["python3", "-m", "tools.wt", "prebuilt", "path"], 60).ok:
            self.spawn(
                "prebuilt-build",
                ["nice", "python3", "-m", "tools.wt", "prebuilt", "build", "--keep", "12"],
                worktree,
                5400,
                True,
            )

    def planners_paused(self) -> bool:
        """T3629 hook: `queue_health --pause-check` exits 1 (PAUSE) over the backlog target."""
        config = self.config
        if not (config.root / "tools/agents/queue_health.py").exists():
            return False
        queue_file = config.codexapi_dir / "p3queue.txt"
        argv = [
            "python3",
            "-m",
            "tools.agents.queue_health",
            "--pause-check",
            "--queue",
            str(queue_file),
        ]
        return self.run(argv, 120).rc == 1

    def job_planners(self) -> None:
        config = self.config
        if (config.codexapi_dir / "codex-limit.until").exists() or self.queue_length() >= 40:
            return
        try:
            remaining = int(self.state_path("feed-remaining.txt").read_text())
        except (OSError, ValueError):
            remaining = 0
        if remaining >= 50:
            return
        laptop = config.hosts.get("laptop")
        if laptop and not self.off(laptop):
            count = self.ssh(laptop, 'ps -eo args | grep -c "^sh .*run.sh CX-planner"', 30)
            running, script = int(count.out.strip() or 1) if count.ok else 1, "sup-planners.sh"
        else:
            running = self.run(
                ["sh", "-c", "ps -eo args | grep -c '^sh run.sh CX-planner'"], 20
            ).out.strip()
            running, script = int(running or 1), "sup-planners-local.sh"
        if running < 4 and self.planners_paused():
            self.record(
                "planners", "paused", "-", "queue_health --pause-check: backlog over target"
            )
        elif running < 4:
            self.spawn("planners", ["sh", script], config.codexapi_dir, 300, False)


def _inv_seen(path: Path) -> dict[str, float]:
    seen: dict[str, float] = {}
    try:
        for line in path.read_text().splitlines():
            parts = line.split()
            if len(parts) == 2:
                seen[parts[0]] = float(parts[1])
    except (OSError, ValueError):
        pass
    return seen


def _append_lines(path: Path, lines: Sequence[str]) -> None:
    if lines:
        with path.open("a") as handle:
            handle.write("".join(f"{line}\n" for line in lines))


def _locked_append(lock: Path, path: Path, lines: Sequence[str]) -> None:
    """Append under the same flock sup-refill.sh takes, so a launch never reads a half-written
    queue."""
    with lock.open("a") as guard:
        fcntl.flock(guard, fcntl.LOCK_EX)
        _append_lines(path, lines)


def fleet_code_changed(controller: Controller, startup_revision: str) -> bool:
    """Whether origin/main added fleet Python changes since this process started."""
    result = controller.run(
        [
            "git",
            "-C",
            str(controller.config.root),
            "diff",
            "--quiet",
            f"{startup_revision}..origin/main",
            "--",
            "tools/fleet/",
        ],
        10,
    )
    return result.rc == 1


def run_forever(
    controller: Controller,
    stop_file: Path | None = None,
    interval: float | None = None,
    on_code_update: Callable[[], None] | None = None,
) -> int:
    """The daemon loop: SIGHUP forces a config reload, SIGTERM/SIGINT or the STOP file ends it."""
    from tools.process_priority import lower_priority

    lower_priority()
    stopping = False

    def _stop(_signum: int, _frame: object) -> None:
        nonlocal stopping
        stopping = True

    def _reload(_signum: int, _frame: object) -> None:
        try:
            controller.watcher.current(force=True)
        except ConfigError:
            pass

    signal.signal(signal.SIGTERM, _stop)
    signal.signal(signal.SIGINT, _stop)
    signal.signal(signal.SIGHUP, _reload)
    root = controller.config.root
    revision = controller.run(["git", "-C", str(root), "rev-parse", "HEAD"], 10)
    startup_revision = revision.out.strip() if revision.ok else ""
    while not stopping:
        if stop_file is not None and stop_file.exists():
            break
        controller.tick()
        if not controller.dry_run:  # T3763: heartbeat, and restart dead peer loops
            from tools.agents import loops

            loops.tick(LOOP_NAME, root)
        if (
            startup_revision
            and on_code_update is not None
            and fleet_code_changed(controller, startup_revision)
        ):
            on_code_update()
            return 0
        deadline = time.time() + (interval or controller.config.tick_seconds)
        while not stopping and time.time() < deadline:
            time.sleep(min(1.0, max(0.0, deadline - time.time())))
    return 0


def single_instance(controller: Controller) -> SingleInstance:
    name = "controller-dry.lock" if controller.dry_run else "controller.lock"
    return SingleInstance(controller.state_path(name))


def controller_alive(state_dir: Path) -> bool:
    """True while a live controller holds its flock (cross-container safe, unlike kill -0)."""
    return flock_held(state_dir / "controller.lock")


LOOP_NAME = "fleet-controller"  # tools/data/loops.json
STOP_NAME = "STOP"  # kill switch in the state dir, read by run_forever and loop_stopped


def loop_alive(root: Path) -> bool:
    """T3763: the supervisor's liveness probe for the live controller."""
    from tools.fleet.config import load_config

    return controller_alive(load_config(root).state_dir)


def kill_switch(root: Path) -> Path:
    """T3830: the live loop's kill switch path (tools/data/loops.json `switch`)."""
    from tools.fleet.config import load_config

    return load_config(root).state_dir / STOP_NAME


def loop_stopped(root: Path) -> bool:
    """T3777: the live loop's own kill switch (STOP in the state dir, the documented rollback):
    the supervisor does not restart a controller that was stopped on purpose."""
    return kill_switch(root).exists()
