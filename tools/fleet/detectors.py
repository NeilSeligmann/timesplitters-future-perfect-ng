"""Supervisor health detectors (T3626): the shell checks of health-check.sh as tested Python.

Each `detect_<name>(ctx)` returns a list of `Finding(ident, detail)`. The ids and detail wording are
matched by the rea-wake watcher and must not change. `run_all` runs every detector with exception
isolation, `append_health_log` writes the `HH:MM:SS HEALTH <id> <detail>` lines.

Everything hangs off a `HealthContext`, so tests inject processes, clock, paths and external
commands.

Liveness rule for runners: `lane_alive` is flock-first. run.sh and run-claude.sh take
`exec 9>$LOG/.runner.lock; flock -n 9` for the life of the runner, and a held flock is visible from
every container sharing the disk, while `ps` only sees the local PID namespace. A process match
is the fallback for runners started before the lock existed.

TODO: the `claude-spend-sync.sh` call of the shell check (carries old-runner spend into
tmp/claudeapi/key-<k>.spent) is NOT ported and stays an external step of the supervisor loop.
"""

from __future__ import annotations

import calendar
import collections
import datetime
import json
import os
import re
import shutil
import subprocess
import tempfile
import time
from collections.abc import Callable, Mapping
from dataclasses import dataclass, field
from pathlib import Path
from typing import TYPE_CHECKING, Any

if TYPE_CHECKING:
    from tools.fleet.config import FleetConfig, HostConfig

from tools import host_registry, hostcaps
from tools.agents import bottlenecks
from tools.fleet import exec_load, integrity, launch_health
from tools.fleet.cache_health import degraded_caches
from tools.fleet.config import FleetConfig
from tools.fleet.core import (
    CmdResult,
    Finding,
    Runner,
    age_seconds,
    flock_held,
    local_runner,
    newest_mtime,
)
from tools.fleet.pipeline_stall import detect_pipeline_stalled
from tools.fleet.status import HostStatus, format_status, parse_status
from tools.wt import retired_hosts

STUCK_SECONDS = 2700


@dataclass(frozen=True)
class Proc:
    pid: int
    ppid: int
    etimes: int
    args: str


def default_processes() -> list[Proc]:
    """Parse `ps -eo pid=,ppid=,etimes=,args=`; empty when ps is unavailable."""
    result = local_runner()(["ps", "-eo", "pid=,ppid=,etimes=,args="], 20)
    procs: list[Proc] = []
    for line in result.out.splitlines():
        parts = line.split(None, 3)
        if len(parts) < 3:
            continue
        try:
            procs.append(
                Proc(
                    int(parts[0]), int(parts[1]), int(parts[2]), parts[3] if len(parts) > 3 else ""
                )
            )
        except ValueError:
            continue
    return procs


def read_cgroup_pids() -> tuple[int | None, int | None]:
    """(pids.current, pids.max) of the cgroup; None for unreadable or `max`."""

    def read(name: str) -> int | None:
        try:
            return int(Path("/sys/fs/cgroup", name).read_text().strip())
        except (OSError, ValueError):
            return None

    return read("pids.current"), read("pids.max")


@dataclass
class HealthContext:
    now: float
    codexapi_dir: Path
    repo_root: Path
    runner: Runner
    state_dir: Path
    status_text: str = ""
    processes: Callable[[], list[Proc]] = default_processes
    disk_free_gb: Callable[[], int | None] | None = None
    pids: Callable[[], tuple[int | None, int | None]] = read_cgroup_pids
    env: Mapping[str, str] = field(default_factory=lambda: os.environ)
    fleet: FleetConfig | None = None  # T3758: the controller's parsed config (owner loader)

    def fleet_config(self) -> FleetConfig | None:
        """The fleet config through its loader (None, reported by the caller, when invalid)."""
        if self.fleet is None:
            from tools.fleet.config import ConfigError, load_config

            try:
                self.fleet = load_config(self.repo_root)
            except ConfigError:
                return None
        return self.fleet

    def free_gb(self) -> int | None:
        if self.disk_free_gb is not None:
            return self.disk_free_gb()
        try:
            return shutil.disk_usage(self.repo_root).free // (1024**3)
        except OSError:
            return None

    def path_env(self, name: str, default: Path) -> Path:
        value = self.env.get(name)
        return Path(value) if value else default


def _read(path: Path) -> str:
    try:
        return path.read_text(errors="replace")
    except OSError:
        return ""


def lane_alive(ctx: HealthContext, lane: str) -> bool:
    """Flock-first runner liveness for lane `lane` (without the `CX-` prefix)."""
    if flock_held(ctx.codexapi_dir / f"CX-{lane}" / ".runner.lock"):
        return True
    prefix = f"sh run.sh CX-{lane} "
    return any(proc.args.startswith(prefix) for proc in ctx.processes())


# --- lane logs ------------------------------------------------------------------------------------


def _recent_lane_dirs(ctx: HealthContext) -> list[Path]:
    dirs = [p for p in ctx.codexapi_dir.glob("CX-*") if p.is_dir()]
    dirs.sort(key=lambda p: p.stat().st_mtime, reverse=True)
    return dirs[:40]


def _touched_within(directory: Path, now: float, seconds: float) -> bool:
    stamps = [directory, *directory.glob("*")]
    newest = newest_mtime(stamps)
    return newest is not None and now - newest < seconds


def _lane_log_findings(ctx: HealthContext) -> dict[str, list[Finding]]:
    out: dict[str, list[Finding]] = {
        "codex-home": [],
        "zero-input-steps": [],
        "claude-runner-failing": [],
    }
    for lane_dir in _recent_lane_dirs(ctx):
        if not _touched_within(lane_dir, ctx.now, 1800):
            continue
        name = lane_dir.name
        if "Error finding codex home" in _read(lane_dir / "step-1.err"):
            out["codex-home"].append(Finding("codex-home", name))
        log = lane_dir / "runner.log"
        if log.is_file():
            text = _read(log)
            if (
                "MAXSTEPS reached" in text
                and not re.search(r" key=[0-9]", text)
                and not re.search(r"last_turn_input=[1-9]", text)
            ):
                out["zero-input-steps"].append(Finding("zero-input-steps", name))
            if "STOP: 5 failed steps in a row" in text:
                out["claude-runner-failing"].append(Finding("claude-runner-failing", name))
    return out


def detect_codex_home(ctx: HealthContext) -> list[Finding]:
    return _lane_log_findings(ctx)["codex-home"]


def detect_zero_input_steps(ctx: HealthContext) -> list[Finding]:
    return _lane_log_findings(ctx)["zero-input-steps"]


def detect_claude_runner_failing(ctx: HealthContext) -> list[Finding]:
    return _lane_log_findings(ctx)["claude-runner-failing"]


# --- processes ------------------------------------------------------------------------------------


def _has_young_descendant(procs: list[Proc], root: int, limit: int) -> bool:
    parent = {p.pid: p.ppid for p in procs}
    for proc in procs:
        if proc.pid == root or proc.etimes >= limit:
            continue
        cur, hops = proc.pid, 0
        while cur in parent and cur != root and hops < 10000:
            cur, hops = parent[cur], hops + 1
        if cur == root:
            return True
    return False


def detect_stuck_job(ctx: HealthContext) -> list[Finding]:
    procs = ctx.processes()
    findings = []
    for proc in procs:
        fields = proc.args.split()
        if len(fields) < 2 or fields[0] != "sh" or proc.etimes <= STUCK_SECONDS:
            continue
        job = fields[1]
        if not re.search(r"CX-[^/]*/job-[0-9]+\.sh$", job):
            continue
        lane = re.search(r"(CX-[^/]*)/job-", job)
        if lane is None or _has_young_descendant(procs, proc.pid, STUCK_SECONDS):
            continue
        lane_dir = ctx.codexapi_dir / lane.group(1)
        try:
            recent = any(
                p.is_file() and ctx.now - p.stat().st_mtime < STUCK_SECONDS
                for p in lane_dir.iterdir()
            )
        except OSError:
            recent = False
        if not recent:
            findings.append(
                Finding(
                    "stuck-job",
                    f"{job} pid={proc.pid} running={proc.etimes // 60}min no-progress-45min",
                )
            )
    return findings


def detect_duplicate_runner(ctx: HealthContext) -> list[Finding]:
    seen: collections.Counter[str] = collections.Counter()
    for proc in ctx.processes():
        fields = proc.args.split()
        if len(fields) >= 3 and fields[0] == "sh" and fields[1] == "run.sh":
            seen[fields[2]] += 1
    dups = sorted(name for name, count in seen.items() if count > 1)
    if not dups:
        return []
    return [Finding("duplicate-runner", "".join(f"{name} " for name in dups))]


def detect_merge_queue_stall(ctx: HealthContext) -> list[Finding]:
    result = ctx.runner(
        ["python3", "-m", "tools.agents.merge_task", "--stats", "--hours", "0.5"], 60
    )
    wait = re.search(r"queue_wait_p90:\s+(\S+)", result.out)
    busy = re.search(r"lock_busy_share:\s+(\S+)", result.out)
    if not wait:
        return []
    try:
        seconds = int(float(wait.group(1)))
        share = float(busy.group(1)) if busy else 1.0
    except ValueError:
        return []
    if seconds > 300 and share < 0.5:
        return [
            Finding(
                "merge-queue-stall",
                f"queue_wait_p90={seconds}s lock_busy={busy.group(1) if busy else ''}",
            )
        ]
    return []


def detect_coord_wait_missing(ctx: HealthContext) -> list[Finding]:
    pattern = re.compile(
        r"^python3 -m tools\.agents\.coord (wait|watch) --owner claude-rea-supervisor"
    )
    if any(pattern.match(proc.args) for proc in ctx.processes()):
        return []
    return [Finding("coord-wait-missing", "no coord wait running")]


def _tunnel_up(ctx: HealthContext, script: str) -> bool:
    return any(proc.args.startswith(f"sh {script}") for proc in ctx.processes())


def _host_retired_or_off(ctx: HealthContext, name: str) -> bool:
    """True for a host the owner took away: `<name>.off` exists or it is `retired` (T3753)."""
    if (ctx.codexapi_dir / f"{name}.off").is_file():
        return True
    return name in retired_hosts.retired_names(host_registry.path_in(ctx.repo_root))


def detect_droplet_tunnel_down(ctx: HealthContext) -> list[Finding]:
    if _host_retired_or_off(ctx, "droplet") or _tunnel_up(ctx, "tunnel-droplet.sh"):
        return []
    return [
        Finding(
            "droplet-tunnel-down",
            "tunnel-droplet.sh not running (restart: setsid nohup python3 reaper.py sh "
            "tunnel-droplet.sh)",
        )
    ]


def detect_droplet2_tunnel_down(ctx: HealthContext) -> list[Finding]:
    if _host_retired_or_off(ctx, "droplet2") or _tunnel_up(ctx, "tunnel-droplet2.sh"):
        return []
    return [
        Finding(
            "droplet2-tunnel-down",
            "tunnel-droplet2.sh not running (restart: setsid nohup python3 reaper.py sh "
            "tunnel-droplet2.sh)",
        )
    ]


# --- host resources -------------------------------------------------------------------------------


def detect_disk_low(ctx: HealthContext) -> list[Finding]:
    free = ctx.free_gb()
    if free is not None and free < 100:
        return [Finding("disk-low", f"{free}G free on /workspace")]
    return []


def detect_disk_falling(ctx: HealthContext) -> list[Finding]:
    """Compare with the previous run's `{ts, free}` and always record this run's value."""
    free = ctx.free_gb()
    if free is None:
        return []
    prev_file = ctx.state_dir / "health-disk.prev.json"
    prev: dict[str, float] = {}
    try:
        prev = json.loads(prev_file.read_text())
    except (OSError, ValueError):
        prev = {}
    try:
        ctx.state_dir.mkdir(parents=True, exist_ok=True)
        prev_file.write_text(json.dumps({"ts": int(ctx.now), "free": free}))
    except OSError:
        pass
    if "ts" in prev and "free" in prev:
        elapsed = int(ctx.now) - int(prev["ts"])
        lost = int(prev["free"]) - free
        if elapsed <= 2400 and lost > 15:
            return [Finding("disk-falling", f"{lost}G lost in {elapsed // 60} min")]
    return []


def detect_disk_budget(ctx: HealthContext) -> list[Finding]:
    """T3745: the project disk budget (tools/data/disk_budget.json): root and data-disk free floors,
    the project-area cap, and a missing or stale project-area measurement (hygiene refreshes it)."""
    from tools import datadisk

    try:
        budget = datadisk.load_budget()
        found = datadisk.status(ctx.repo_root, budget)
    except (OSError, ValueError, KeyError) as exc:
        return [Finding("disk-budget-unreadable", str(exc)[:200])]
    findings = [Finding("disk-budget", problem) for problem in found.problems]
    try:
        age_h = (ctx.now - datadisk.usage_file(ctx.repo_root).stat().st_mtime) / 3600
    except OSError:
        age_h = float("inf")
    if (ctx.repo_root / "tmp").is_dir() and age_h > 2 * budget.usage_max_age_hours:
        findings.append(
            Finding(
                "disk-budget-unmeasured",
                f"project-area du is {age_h:.0f} h old (> {2 * budget.usage_max_age_hours:g} h); "
                "the hygiene tick runs python3 -m tools.datadisk measure",
            )
        )
    return findings


ROOT_SPILL_DIRS = ("/tmp", "/var/tmp")
ROOT_SPILL_MAX_GB = 5.0


def detect_root_disk_project_data(ctx: HealthContext) -> list[Finding]:
    """T3745 (disk-root-write): project bulk on the root disk. /tmp and /var/tmp over 5 GB while a
    separate data disk exists means a tool ignored the data-disk TMPDIR."""
    from tools import datadisk

    findings = []
    for spill in ROOT_SPILL_DIRS:
        try:
            datadisk.check_path(spill, what="spill")
            continue  # not the root disk, or a one-disk host
        except datadisk.DiskRefused:
            pass
        except OSError:
            continue
        done = ctx.runner(["du", "-sx", "--block-size=1", spill], 60)
        try:
            size_gb = int(done.stdout.split()[0]) / 1024**3
        except (AttributeError, IndexError, ValueError):
            continue
        if size_gb > ROOT_SPILL_MAX_GB:
            findings.append(
                Finding(
                    "disk-root-write",
                    f"{spill} holds {size_gb:.1f} GB on the root disk; project data belongs on the "
                    "data disk (python3 -m tools.datadisk tmpdir, docs/t3745-disk-safety.md)",
                )
            )
    return findings


def detect_local_overload(ctx: HealthContext) -> list[Finding]:
    """T3745: the local host is the owner PC: load over 2x cores for 10 min, or MemAvailable
    under the floor (tools/data/local_load.json)."""
    from tools.agents import local_load

    proc = ctx.path_env("TSFP_PROC_DIR", Path("/proc"))
    cfg = local_load.load_config()
    cores = int(ctx.env.get("TSFP_CORES", "") or hostcaps.host_cpus())
    prev_file = ctx.state_dir / "health-local-load.prev.json"
    try:
        since = float(json.loads(prev_file.read_text())["since"])
    except (OSError, ValueError, KeyError, TypeError):
        since = None
    problems, since = local_load.overload(
        cfg,
        cores,
        local_load.read_loadavg(proc),
        local_load.read_available_gb(proc),
        ctx.now,
        since,
    )
    try:
        ctx.state_dir.mkdir(parents=True, exist_ok=True)
        prev_file.write_text(json.dumps({"since": since}))
    except OSError:
        pass
    return [Finding("local-overload", "; ".join(problems))] if problems else []


def detect_local_game_hosts(ctx: HealthContext) -> list[Finding]:
    """T3823 (local-host-fanout): more game hosts (tsfp-bevy, tsfp_host) running in this PID
    namespace than the owner-PC limit of tools/data/local_game_hosts.json plus open play windows."""
    from tools import local_game_slots

    proc = ctx.path_env("TSFP_PROC_DIR", Path("/proc"))
    detail = local_game_slots.over_limit(ctx.repo_root, proc)
    return [Finding("local-game-hosts", detail)] if detail else []


def detect_pids_high(ctx: HealthContext) -> list[Finding]:
    current, maximum = ctx.pids()
    limit = maximum if maximum else 1000000
    if limit - (current or 0) < 8000:
        shown = "" if current is None else current
        return [
            Finding(
                "pids-high",
                f"pids.current={shown} of pids.max={limit} "
                "(zombies under PID 1 only clear on a container restart with --init)",
            )
        ]
    return []


# --- jsonl / state files --------------------------------------------------------------------------


def _tail_lines(path: Path, count: int) -> list[str]:
    return _read(path).splitlines()[-count:]


def detect_host_health_refusals(ctx: HealthContext) -> list[Finding]:
    log = ctx.path_env("TSFP_ADMISSION_LOG", ctx.repo_root / "tmp" / "verify-admission.jsonl")
    counts: collections.Counter[str] = collections.Counter()
    for line in _tail_lines(log, 5000):
        if '"decision": "host_health_refusal"' not in line:
            continue
        stamp = re.search(r'"ts": ([0-9.]+)', line)
        if not stamp or ctx.now - float(stamp.group(1)) > 3600:
            continue
        host = re.search(r'"host": "([^"]*)"', line)
        if host:
            counts[host.group(1)] += 1
    return [
        Finding(
            "host-health-refusals",
            f"{host} {count} refusals in the last hour (guard keeps refusing builds)",
        )
        for host, count in sorted(counts.items())
        if count > 20
    ]


def _json_field(text: str, key: str, pattern: str) -> str | None:
    match = re.search(rf'"{key}": {pattern}', text)
    return match.group(1) if match else None


def _origin_ref_count(ctx: HealthContext) -> int:
    result = ctx.runner(["git", "for-each-ref", "refs/remotes/origin"], 60)
    return len([line for line in result.out.splitlines() if line.strip()])


def _origin_state(ctx: HealthContext) -> tuple[str, dict[str, str | None]] | None:
    state = ctx.path_env(
        "TSFP_PRUNE_STATE", Path(tempfile.gettempdir()) / "tsfp-origin-branches.json"
    )
    text = _read(state)
    if not text.strip():
        return None
    return text, {
        "branches": _json_field(text, "branches", "([0-9]+)"),
        "ts": _json_field(text, "ts", "([0-9]+)"),
        "error": _json_field(text, "error", '"([^"]*)"'),
    }


def detect_origin_prune_failed(ctx: HealthContext) -> list[Finding]:
    parsed = _origin_state(ctx)
    if parsed and '"ok": false' in parsed[0]:
        return [Finding("origin-prune-failed", (parsed[1]["error"] or "")[:120])]
    return []


ORIGIN_BRANCH_LIMIT = 500


def _origin_record(text: str) -> dict[str, Any]:
    try:
        record = json.loads(text)
    except ValueError:
        return {}
    return record if isinstance(record, dict) else {}


def detect_origin_branches(ctx: HealthContext) -> list[Finding]:
    """T3804: the limit applies to branches the sweep may delete (`sweepable`, written by
    tools.wt.prune_remote); archive/*, rescue/*, keep/* and protected names are kept deliberately
    and only reported as a low-priority info finding."""
    parsed = _origin_state(ctx)
    if parsed:
        record = _origin_record(parsed[0])
        total = int(parsed[1]["branches"] or 0)
        sweepable = record.get("sweepable")
        count = sweepable if isinstance(sweepable, int) else total
        preserved = record.get("preserved")
        findings: list[Finding] = []
        if count > ORIGIN_BRANCH_LIMIT:
            findings.append(
                Finding(
                    "origin-branches",
                    f"origin holds {count} sweepable branches of {total} "
                    f"(limit {ORIGIN_BRANCH_LIMIT}): merged-branch sweep behind",
                )
            )
        if isinstance(preserved, int) and preserved > 0:
            refused = record.get("refused_count")
            note = f", {refused} server-refused deletes kept" if refused else ""
            findings.append(
                Finding(
                    "origin-branches-preserved-info",
                    f"low priority: {preserved} of {total} origin branches are kept deliberately "
                    f"(archive/*, rescue/*, keep/*, protected){note}",
                )
            )
        return findings
    count = _origin_ref_count(ctx)
    if count > 500:
        return [
            Finding(
                "origin-branches",
                f"no sweep state; {count} local refs/remotes/origin refs (limit 500)",
            )
        ]
    return []


def detect_origin_prune_stale(ctx: HealthContext) -> list[Finding]:
    parsed = _origin_state(ctx)
    stamp = parsed[1]["ts"] if parsed else None
    if stamp and int(ctx.now) - int(stamp) > 28800:
        return [
            Finding(
                "origin-prune-stale", f"sweep last ran {(int(ctx.now) - int(stamp)) // 3600} h ago"
            )
        ]
    return []


def _coordsvc_dir(ctx: HealthContext) -> Path:
    return ctx.path_env("TSFP_COORDSVC_STATE_DIR", ctx.repo_root / "tmp" / "coordsvc")


def _coordsvc_status(ctx: HealthContext) -> str | None:
    text = _read(_coordsvc_dir(ctx) / "status.json")
    return text if text.strip() else None


COORDSVC_DOWN_AFTER_S = 300


def detect_coordsvc_down(ctx: HealthContext) -> list[Finding]:
    text = _coordsvc_status(ctx)
    if text is None:
        return []
    updated = _json_field(text, "updated", "([0-9]+)")
    last_ok = _json_field(text, "last_ok", "([0-9]+)")
    # T3693: a single timed-out probe on the loaded server is not an outage (4 false alarms on
    # 2026-10-24); down means no successful call for COORDSVC_DOWN_AFTER_S.
    recent_success = last_ok and int(ctx.now) - int(last_ok) < COORDSVC_DOWN_AFTER_S
    if (
        '"ok": false' in text
        and updated
        and int(ctx.now) - int(updated) < 1800
        and not recent_success
    ):
        return [
            Finding("coordsvc-down", (_json_field(text, "last_error", '"([^"]*)"') or "")[:120])
        ]
    return []


def detect_coordsvc_shadow_diffs(ctx: HealthContext) -> list[Finding]:
    if _coordsvc_status(ctx) is None:
        return []
    count = 0
    for line in _read(_coordsvc_dir(ctx) / "shadow-diff.jsonl").splitlines():
        stamp = re.search(r'"ts": ([0-9.]+)', line)
        if stamp and ctx.now - float(stamp.group(1)) < 3600:
            count += 1
    if count > 0:
        return [
            Finding(
                "coordsvc-shadow-diffs",
                f"{count} shadow diffs in the last hour (python3 -m tools.agents.coordsvc diffs "
                f"--hours 1)",
            )
        ]
    return []


def detect_coordsvc_outbox(ctx: HealthContext) -> list[Finding]:
    if _coordsvc_status(ctx) is None:
        return []
    outbox = _coordsvc_dir(ctx) / "outbox.jsonl"
    text = _read(outbox)
    if text.strip() or text:
        return [
            Finding(
                "coordsvc-outbox",
                f"{text.count(chr(10))} messages queued while the service was down",
            )
        ]
    return []


def detect_merge_verify_sigsegv(ctx: HealthContext) -> list[Finding]:
    count = 0
    for log in (ctx.repo_root / "tmp" / "merge-submit").glob("*.log"):
        age = age_seconds(log, ctx.now)
        if age is None or age >= 3600:
            continue
        text = _read(log)
        if "SIGSEGV" in text or "no_std archive compile failed" in text:
            count += 1
    if count > 3:
        return [
            Finding(
                "merge-verify-sigsegv",
                f"{count} merge logs in the last hour show SIGSEGV or a no_std archive compile "
                f"failure "
                "(rustc pin? python3 -m tools.agents.no_std_archive; docs/incidents/I-T3472-1.md, "
                "I-T3574-1)",
            )
        ]
    return []


MERGE_RESULT_WINDOW_S = 24 * 3600
MERGE_RESULT_GRACE_S = 300  # origin/main in the primary may lag a push by a moment


def _on_origin(ctx: HealthContext, revision: str) -> bool:
    argv = ["git", "-C", str(ctx.repo_root), "merge-base", "--is-ancestor", revision, "origin/main"]
    return ctx.runner(argv, 30).ok


def detect_merge_reported_not_on_origin(ctx: HealthContext) -> list[Finding]:
    """A merge_task --result-file claiming pushed=true whose landed_sha origin/main lacks.

    Callers (lane runners, supervisors) trust pushed=true, so a false one silently loses work.
    --no-push results (pushed=false, kept on a keep/ ref by design) are ignored. Every result
    of the last 24 h is checked on each run, so a past false-ok stays visible."""
    findings: list[Finding] = []
    for path in sorted((ctx.repo_root / "tmp" / "merge-submit").glob("*.json")):
        age = age_seconds(path, ctx.now)
        if age is None or age < MERGE_RESULT_GRACE_S or age >= MERGE_RESULT_WINDOW_S:
            continue
        try:
            result = json.loads(_read(path))
        except ValueError:
            continue
        if not isinstance(result, dict) or result.get("pushed") is not True:
            continue
        sha = str(result.get("landed_sha") or "")
        if sha and _on_origin(ctx, sha):
            continue
        # A superseded train merge commit (a later main move replaced it, maybe pruned) whose
        # branch tip is on origin is resolved, not a loss (claude-sup, 2026-10-27). The tip is
        # the merge's branch side, the result's branch_sha, or (older results) the branch ref.
        tips = [f"{sha}^2"] if sha else []
        tips += [str(result["branch_sha"])] if result.get("branch_sha") else []
        tips += [f"refs/heads/{result['branch']}"] if result.get("branch") else []
        if not any(_on_origin(ctx, tip) for tip in tips):
            findings.append(
                Finding(
                    "merge-reported-not-on-origin",
                    f"{result.get('branch', path.stem)} {sha[:12] or '(no landed_sha)'}",
                )
            )
    return findings


NO_STD_REMOTE_FAIL = re.compile(
    r"FAILED remotely on (\S+) exit \d+: .*-m tools\.agents\.no_std_archive"
)
NO_STD_LOCAL_OK = re.compile(r"verify: ok [\d.]+s: .*-m tools\.agents\.no_std_archive")


def detect_no_std_remote_skew(ctx: HealthContext) -> list[Finding]:
    """I-T3797-1: the no_std archive failed on a verify host, then passed when re-run locally on
    the same candidate. The remote script then disagreed with the shipped tree (it was rendered
    from the caller's checkout), so every such job compiles twice."""
    hosts: list[str] = []
    for log in (ctx.repo_root / "tmp" / "merge-submit").glob("*.log"):
        age = age_seconds(log, ctx.now)
        if age is None or age >= 3600:
            continue
        lines = _read(log).splitlines()
        for index, line in enumerate(lines):
            failed = NO_STD_REMOTE_FAIL.search(line)
            if failed and any(NO_STD_LOCAL_OK.search(later) for later in lines[index + 1 :]):
                hosts.append(failed.group(1))
    if not hosts:
        return []
    return [
        Finding(
            "no-std-remote-skew",
            f"{len(hosts)} no_std archive check(s) failed remotely "
            f"({', '.join(sorted(set(hosts)))}) but passed locally in the last hour "
            "(remote script vs shipped tree? python3 -m tools.wt.remote_fallback --stats; "
            "docs/incidents/I-T3797-1.md)",
        )
    ]


def detect_integrity_changed(ctx: HealthContext) -> list[Finding]:
    """T3697: a hashed interpreter or toolchain file differs from the manifest (bit flip). Local
    status is rehashed at most hourly; remote hosts' statuses are written by the controller job."""
    statuses = [integrity.cached_status(ctx.repo_root, ctx.now)]
    for path in sorted((ctx.repo_root / "tmp" / "fleet").glob("integrity-remote-*.json")):
        try:
            statuses.append(json.loads(path.read_text()))
        except (OSError, ValueError):
            continue
    findings = []
    for status in statuses:
        for _, detail in integrity.format_findings(status):
            findings.append(
                Finding(
                    "integrity-changed",
                    f"{detail} (host binary corruption? compare with another host, then "
                    "python3 -m tools.fleet.integrity record --accept PATH --expect SHA; "
                    "docs/incidents/I-T3697-1.md)",
                )
            )
    return findings


CAPGUARD_STALE_PASSES = 3
CAPGUARD_STALE_MIN_S = 3600


def detect_capguard_not_running(ctx: HealthContext) -> list[Finding]:
    """cx-capguard-execonly: each controller capguard pass copies the remote heartbeat
    (~/csup/capguard.stamp) to tmp/fleet/capguard-<host>.json. A missing, unreadable or stale stamp
    (older than 3 passes, at least 1 h) or a pass without diskprune means the disk cap is not
    enforced: the deck's cargo target reached 34 GB against a 6 GB cap."""
    config = ctx.fleet_config()
    if config is None:
        return []
    findings = []
    for host in config.remote_hosts():
        policy = host.capguard
        if policy is None or (host.off_file and (ctx.codexapi_dir / host.off_file).exists()):
            continue
        limit = max(CAPGUARD_STALE_PASSES * policy.every_min * 60, CAPGUARD_STALE_MIN_S)
        path = ctx.repo_root / "tmp" / "fleet" / f"capguard-{host.name}.json"
        try:
            stamp = json.loads(path.read_text())
            last = float(stamp["ts"])
        except (OSError, ValueError, KeyError, TypeError):
            stamp, last = {}, None
        if last is None or ctx.now - last > limit:
            age = "never" if last is None else f"{(ctx.now - last) / 60:.0f} min ago"
            detail = f"{host.name}: last capguard pass {age} (limit {limit / 60:.0f} min)"
        elif not stamp.get("diskprune"):
            detail = f"{host.name}: last capguard pass ran no diskprune (~/csup/diskprune.py?)"
        else:
            continue
        findings.append(
            Finding(
                "capguard-not-running",
                f"{detail}; tail ~/csup/capguard.log, python3 -m tools.fleet provision "
                f"{host.name} --apply --only capguard (docs/incidents/I-T3769-5.md)",
            )
        )
    return findings


def detect_cargo_target_over_cap(ctx: HealthContext) -> list[Finding]:
    """Report host-wide target usage, including when active targets prevent pruning."""
    config = ctx.fleet_config()
    if config is None:
        return []
    findings = []
    for host in config.remote_hosts():
        if host.capguard is None or (host.off_file and (ctx.codexapi_dir / host.off_file).exists()):
            continue
        path = ctx.repo_root / "tmp" / "fleet" / f"capguard-{host.name}.json"
        try:
            stamp = json.loads(path.read_text())
            size = int(stamp["target_bytes"])
            cap = float(stamp["target_cap_gb"])
        except (OSError, ValueError, TypeError, KeyError):
            continue  # Old or absent heartbeat; capguard-not-running handles availability.
        if size < 0 or cap <= 0 or size > cap * 1024**3:
            detail = (
                "measurement failed"
                if size < 0 or cap <= 0
                else f"{size / 1024**3:.1f} GiB exceeds {cap:g} GiB"
            )
            findings.append(
                Finding("cargo-target-over-cap", f"{host.name}: {detail}; inspect capguard.log")
            )
    return findings


def detect_owner_file_outside_cache(ctx: HealthContext) -> list[Finding]:
    """Rule 1: owner files (disc, XBE, recordings, saves) live on a host only in the private replay
    input cache and the job dirs staged from it. The capguard heartbeat field `owner_outside_cache`
    counts regular owner files elsewhere (the laptop kept a second 4 GB disc in `tmp/owner`,
    2026-10-26, docs/incidents/I-T3769-9.md). A count above 0 or -1 (scan failed) is a finding."""
    config = ctx.fleet_config()
    if config is None:
        return []
    findings = []
    for host in config.remote_hosts():
        path = ctx.repo_root / "tmp" / "fleet" / f"capguard-{host.name}.json"
        try:
            count = int(json.loads(path.read_text()).get("owner_outside_cache", 0))
        except (OSError, ValueError, TypeError, AttributeError):
            continue  # a missing or stale stamp is capguard-not-running's finding
        if count == 0:
            continue
        what = "its scan failed" if count < 0 else f"{count} owner file(s) outside the input cache"
        findings.append(
            Finding(
                "owner-file-outside-cache",
                f"{host.name}: {what}; ssh the host and read ~/csup/ownerscan.log (paths only), "
                "delete the copies when no job has them open (fuser), never print them "
                "(docs/incidents/I-T3769-9.md)",
            )
        )
    return findings


MERGE_BATCH_WINDOW_S = 3 * 3600
MERGE_BATCH_MIN_LANDINGS = 10


def detect_merge_batch_idle(ctx: HealthContext) -> list[Finding]:
    """T3662: rule 40 makes batching the default, yet for days no batch formed (worker landings
    were --no-push and excluded) and --no-push train members re-landed on their own lock turn
    because their receipt was checked against a branch that never moves. Both are silent: the
    landings still succeed, only the queue grows."""
    stats = ctx.repo_root / "tmp" / "merge-stats.jsonl"
    records = []
    for line in _read(stats).splitlines():
        try:
            record = json.loads(line)
        except ValueError:
            continue
        stamp = record.get("ts") if isinstance(record, dict) else None
        if isinstance(stamp, (int, float)) and ctx.now - stamp < MERGE_BATCH_WINDOW_S:
            if not record.get("dry_run"):
                records.append(record)
    landed = [r for r in records if r.get("landed")]
    findings: list[Finding] = []
    leaders = sum(1 for r in records if (r.get("batch") or {}).get("role") == "leader")
    if len(landed) >= MERGE_BATCH_MIN_LANDINGS:
        # T3831: same condition as bottleneck-merge-no-batches (T3802): solo landing is healthy
        # while verify rounds stay low; rounds come from the stats owner
        from tools.agents.merge_task import quantile, record_rounds

        rounds_p90 = quantile([record_rounds(r) for r in landed], 0.9)
        if bottlenecks.no_batch_is_problem(leaders, rounds_p90):
            findings.append(
                Finding(
                    "merge-batch-idle",
                    f"{len(landed)} landings in 3 h, no batch formed and rounds_p90={rounds_p90} "
                    f">= {bottlenecks.NO_BATCH_ROUNDS_P90} (rule 40; merge_task --stats; "
                    "docs/incidents/I-T3662-2.md, T3802)",
                )
            )
    relanded = relanded_members(landed)
    if len(relanded) >= 3:
        counts: dict[str, int] = {}
        for branch in relanded:
            counts[branch] = counts.get(branch, 0) + 1
        named = ", ".join(
            f"{branch} x{count}" if count > 1 else branch
            for branch, count in sorted(counts.items(), key=lambda item: (-item[1], item[0]))[:6]
        )
        findings.append(
            Finding(
                "merge-train-relanded",
                f"{len(relanded)} train members took their own lock turn after their train "
                f"landed ({named}; docs/incidents/I-T3662-2.md, docs/incidents/I-T3815-1.md)",
            )
        )
    return findings


def relanded_members(landed: list[dict]) -> list[str]:
    """Branches that took a lock turn to land what a train had already landed (T3815).

    A lock turn (hold_s, not a train-member exit) counts when its sha is a member sha of a
    train in the window (either order: a member's turn can end before the head's record,
    which is written after the push), or when its own merge added nothing (noop_merge).
    Records from before T3815 carry no shas and fall back to the branch name, minus
    train-member exits; before, every resubmission of a long-lived worker branch with new
    commits counted (21 of 39 hits on 2026-10-26)."""
    member_shas: set[str] = set()
    legacy_members: dict[str, float] = {}
    for record in landed:
        shas = record.get("train_shas")
        if isinstance(shas, list):
            member_shas.update(str(sha) for sha in shas if sha)
        else:
            for member in record.get("train") or []:
                legacy_members.setdefault(str(member), float(record.get("ts") or 0))
    relanded: list[str] = []
    for record in landed:
        if not record.get("hold_s") or record.get("mode") == "train-member":
            continue
        branch = str(record.get("branch"))
        sha = record.get("branch_sha")
        if record.get("noop_merge") or (sha and sha in member_shas):
            relanded.append(branch)
        elif "branch_sha" not in record and branch in legacy_members:
            if float(record.get("ts") or 0) > legacy_members[branch]:
                relanded.append(branch)
    return relanded


MERGE_LOCK_HOG_S = 300


def detect_merge_lock_hog(ctx: HealthContext) -> list[Finding]:
    """T3662: one landing holding tmp/merge.lock over 300 s in the last hour (a verify under the
    lock: an urgent last round held it 1433 s while every other landing queued)."""
    worst: tuple[float, str] | None = None
    for line in _read(ctx.repo_root / "tmp" / "merge-stats.jsonl").splitlines():
        try:
            record = json.loads(line)
        except ValueError:
            continue
        if not isinstance(record, dict):
            continue
        stamp, hold = record.get("ts"), record.get("hold_s")
        if not isinstance(stamp, (int, float)) or not isinstance(hold, (int, float)):
            continue
        if ctx.now - stamp < 3600 and hold > MERGE_LOCK_HOG_S and (not worst or hold > worst[0]):
            worst = (hold, str(record.get("branch") or "?"))
    if worst is None:
        return []
    return [
        Finding(
            "merge-lock-hog",
            f"{worst[1]} held the merge lock {worst[0]:.0f} s (> {MERGE_LOCK_HOG_S} s); "
            "nothing heavy may run under it (docs/incidents/I-T3662-4.md)",
        )
    ]


MERGE_VERIFY_ROUNDS = 4


def detect_merge_verify_rounds(ctx: HealthContext) -> list[Finding]:
    """T3732: landings that needed MERGE_VERIFY_ROUNDS or more verify rounds in the last hour
    (primary moves kept invalidating them; T3710 re-verified round after round and ten REA
    landings sat in merge_task 1.5-6 h before the starvation hold, tools.agents.merge_starvation).
    """
    worst: list[tuple[int, str]] = []
    for line in _read(ctx.repo_root / "tmp" / "merge-stats.jsonl").splitlines():
        try:
            record = json.loads(line)
        except ValueError:
            continue
        if not isinstance(record, dict) or not record.get("landed"):
            continue
        stamp = record.get("ts")
        rounds = record.get("verify_rounds", record.get("attempt"))
        if not isinstance(stamp, (int, float)) or not isinstance(rounds, int):
            continue
        if ctx.now - stamp < 3600 and rounds >= MERGE_VERIFY_ROUNDS:
            worst.append((rounds, str(record.get("branch") or "?")))
    if not worst:
        return []
    rounds, branch = max(worst)
    return [
        Finding(
            "merge-verify-rounds",
            f"{len(worst)} landing(s) needed >= {MERGE_VERIFY_ROUNDS} verify rounds in the last "
            f"hour (worst {branch} {rounds}); primary moves keep invalidating candidates "
            "(docs/t3732-merge-starvation.md)",
        )
    ]


MERGE_CANDIDATE_ROUNDS = 3  # T3776: more lost rounds than this for one live candidate
MERGE_CANDIDATE_WINDOW_S = 7200.0  # its last round ended this recently (rounds take ~30 min)


def _branch_landed(ctx: HealthContext, branch: str) -> bool:
    """The branch tip is already on origin/main (the candidate landed, maybe in a later
    submission or a train)."""
    result = ctx.runner(
        ["git", "-C", str(ctx.repo_root), "merge-base", "--is-ancestor", branch, "origin/main"], 30
    )
    return result.ok


def _merge_session_alive(ctx: HealthContext, pid: object) -> bool:
    """The merge_task session that wrote the row still runs: its pid is a visible merge_task
    process, or (another container's PID namespace) its queue ticket `*-<pid>` is fresh."""
    if not isinstance(pid, int):
        return False
    from tools.agents.merge_task import TICKET_FRESH_S, queue_dir  # owner of the queue rules

    if any(p.pid == pid and "merge_task" in p.args for p in ctx.processes()):
        return True
    queue = queue_dir(ctx.repo_root / "tmp" / "merge.lock")
    try:
        tickets = list(queue.glob(f"*-{pid}"))
    except OSError:
        return False
    for ticket in tickets:
        try:
            if ctx.now - ticket.stat().st_mtime < TICKET_FRESH_S:
                return True
        except OSError:
            continue
    return False


def detect_merge_candidate_rounds(ctx: HealthContext) -> list[Finding]:
    """T3776: one candidate still in merge_task after more than MERGE_CANDIDATE_ROUNDS
    re-verify rounds (I-T3776-1: task/cx-cachekeys lost 7 rounds over 4.3 h and never landed,
    so the landed-only merge-verify-rounds detector stayed silent). A session is (branch,
    enqueued); it is live while its newest record is a recent "reverify" (no landing or failure
    since). The priority hold (merge_starvation.PRIORITY_ROUNDS) should land it next round.
    Stale rows are skipped (08:53 2026-10-26 it fired for cx-cachekeys' 04:01 session after the
    branch landed at 08:49): the branch tip is on origin/main, or the session's merge_task is
    gone (no visible process and no fresh queue ticket of its pid)."""
    sessions: dict[tuple[str, object], dict] = {}
    for line in _read(ctx.repo_root / "tmp" / "merge-stats.jsonl").splitlines():
        try:
            record = json.loads(line)
        except ValueError:
            continue
        if not isinstance(record, dict) or not isinstance(record.get("ts"), (int, float)):
            continue
        key = (str(record.get("branch") or "?"), record.get("enqueued"))
        if key not in sessions or record["ts"] >= sessions[key]["ts"]:
            sessions[key] = record
    stuck: list[tuple[int, str, float]] = []
    for (branch, enqueued), record in sessions.items():
        rounds = record.get("verify_rounds", record.get("attempt"))
        if record.get("landed") or record.get("result") != "reverify":
            continue
        if not isinstance(rounds, int) or rounds <= MERGE_CANDIDATE_ROUNDS:
            continue
        if ctx.now - record["ts"] >= MERGE_CANDIDATE_WINDOW_S:
            continue
        if _branch_landed(ctx, branch) or not _merge_session_alive(ctx, record.get("pid")):
            continue  # I-T3776-1 follow-up: a stale row of a session that ended or landed
        since = enqueued if isinstance(enqueued, (int, float)) else record["ts"]
        stuck.append((rounds, branch, since))
    if not stuck:
        return []
    rounds, branch, since = max(stuck)
    hours = max(0.0, ctx.now - since) / 3600
    return [
        Finding(
            "merge-candidate-rounds",
            f"{branch} lost {rounds} re-verify rounds in merge_task ({hours:.1f} h since submit"
            + (f", {len(stuck)} such candidates" if len(stuck) > 1 else "")
            + "); the T3776 priority hold should have landed it, read its tmp/merge-submit log "
            "(docs/t3776-starvation-priority.md)",
        )
    ]


def detect_primary_ahead(ctx: HealthContext) -> list[Finding]:
    """T3662 (class of T3461): primary main ahead of origin blocks every landing ("primary and
    origin diverged"); the reference-transaction guard prevents it, so report a missing guard."""
    findings: list[Finding] = []
    result = ctx.runner(
        ["git", "-C", str(ctx.repo_root), "rev-list", "--count", "origin/main..main"], 30
    )
    ahead = result.out.strip()
    if result.ok and ahead.isdigit() and int(ahead) > 0:
        findings.append(
            Finding(
                "primary-ahead",
                f"primary main has {ahead} commit(s) origin lacks; every merge_task refuses "
                "(docs/incidents/I-T3662-3.md)",
            )
        )
    hook = ctx.repo_root / ".git" / "hooks" / "reference-transaction"
    if (ctx.repo_root / ".git").is_dir() and "primary-ref-guard" not in _read(hook):
        findings.append(
            Finding(
                "primary-guard-missing",
                "reference-transaction hook lacks tools/ci/primary-ref-guard.sh (install: "
                "python3 -m tools.wt bootstrap; a foreign hook is never overwritten)",
            )
        )
    return findings


def detect_claude_keys_exhausted(ctx: HealthContext) -> list[Finding]:
    # claude-spend-sync.sh (spend carry-over) is not ported and stays an external supervisor step.
    keys = ctx.repo_root / "tmp" / "claudeapi"
    if (keys / "key-100.exhausted").exists() and (keys / "key-200.exhausted").exists():
        return [
            Finding(
                "claude-keys-exhausted",
                "both Claude API keys report no credit (only Claude subagents on the session "
                "subscription remain)",
            )
        ]
    return []


def detect_remote(ctx: HealthContext) -> list[Finding]:
    return [
        Finding("remote", line.strip())
        for line in ctx.status_text.splitlines()
        if re.search(r"UNREACHABLE|memguard=0|overcap=yes", line) and line.strip()
    ]


def _eligible_share(ctx: HealthContext) -> tuple[int, str] | None:
    """(remote-eligible admissions, their local share) of the last hour (T3782, I-T3782-2).

    T3760 cost routing keeps Python test sets local by design; counting them made a healthy
    hour read local_share 0.53 'remote-first broken'. Only remote-eligible rows (cargo builds,
    replays) count. None when there is no share yet; stats without the eligible keys (an older
    remote_fallback) are reported, not read as healthy."""
    result = ctx.runner(
        ["python3", "-m", "tools.wt.remote_fallback", "--stats", "--hours", "1"], 60
    )
    count = re.search(r'^ *"remote_eligible": ([0-9]+)', result.out, re.M)
    share = re.search(r'^ *"remote_eligible_local_share": ([0-9.]+)', result.out, re.M)
    if count is None and re.search(r'^ *"decisions": [1-9]', result.out, re.M):
        raise ValueError("remote_fallback --stats has no remote_eligible keys")
    if count is None or share is None:
        return None
    return int(count.group(1)), share.group(1)


def detect_verify_local_share(ctx: HealthContext) -> list[Finding]:
    try:
        found = _eligible_share(ctx)
    except ValueError as error:
        return [Finding("verify-local-share", f"unreadable admission stats: {error}")]
    if found and found[0] >= 6 and float(found[1]) > 0.5:
        return [
            Finding(
                "verify-local-share",
                f"remote-eligible local_share={found[1]} of {found[0]} verify admissions in the "
                "last hour (cargo builds/replays; Python test sets local by design, T3760) "
                "(remote-first broken? python3 -m tools.wt.remote_fallback --stats --hours 1; "
                "docs/t3582-remote-first-merge-verify.md)",
            )
        ]
    return []


def _verify_hosts_benched(ctx: HealthContext) -> bool:
    """True when every verify-capable remote host carries a live degraded mark."""
    from tools.wt import remote_exec

    try:
        names = [h.name for h in remote_exec.load_registry() if "verify" in h.capabilities]
    except (OSError, ValueError, KeyError):
        return False
    return bool(names) and all(remote_exec.degraded_reason(ctx.repo_root, n) for n in names)


def detect_verify_all_hosts_benched(ctx: HealthContext) -> list[Finding]:
    """I-T3678: local verify share over 0.8 while every remote verify host is benched (the
    staging-timeout death spiral: all verifies pile onto the overloaded owner PC)."""
    try:
        found = _eligible_share(ctx)
    except ValueError:
        return []  # verify-local-share reports the unreadable stats
    if not (found and found[0] >= 6 and float(found[1]) > 0.8):
        return []
    if not _verify_hosts_benched(ctx):
        return []
    return [
        Finding(
            "verify-all-hosts-benched",
            f"remote-eligible local_share={found[1]} of {found[0]} verify admissions in the last "
            "hour and every remote verify host is degraded (ls tmp/wt-cache/remote-degraded; "
            "rm the marks if transient; I-T3678)",
        )
    ]


def _regress_dir(ctx: HealthContext) -> Path:
    return ctx.path_env("TSFP_REGRESS_DIR", ctx.repo_root / "tmp" / "regress")


def detect_regress_fault(ctx: HealthContext) -> list[Finding]:
    text = _read(_regress_dir(ctx) / "block.json")
    if not text.strip():
        return []
    parts = []
    for line in text.splitlines():
        route = re.match(r'^  "route": "([^"]*)"', line)
        sha = re.match(r'^  "first_bad_sha": "([0-9a-f]{12})', line)
        poll = re.match(r'^  "first_fault_poll": ([0-9]+)', line)
        if route:
            parts.append(f"route {route.group(1)}")
        elif sha:
            parts.append(f"first_bad {sha.group(1)}")
        elif poll:
            parts.append(f"poll {poll.group(1)}")
    prefix = "".join(f"{part} " for part in parts)
    return [
        Finding(
            "regress-fault",
            f"{prefix}adoptions blocked (python3 -m tools.regress status; "
            f"docs/t3587-regression-replay.md)",
        )
    ]


def _regress_age(state: Path, now: float) -> int | None:
    try:
        stamp = json.loads(state.read_text())["last_good_all"]["at"]
        return int(now - calendar.timegm(time.strptime(stamp, "%Y-%m-%dT%H:%M:%SZ")))
    except (OSError, ValueError, KeyError, TypeError):
        return None


def detect_regress_stale(ctx: HealthContext) -> list[Finding]:
    regress = _regress_dir(ctx)
    if not (regress / "loop.last").is_file():
        return []
    age = _regress_age(regress / "state.json", ctx.now)
    if age is not None and 0 <= age <= 10800:
        return []
    since = f"{age // 3600} h" if age is not None and age >= 0 else "its history"
    return [
        Finding(
            "regress-stale",
            f"no all-routes PASS of the standing regression replay in {since} "
            f"(tail {regress}/loop.log; python3 -m tools.regress status)",
        )
    ]


def detect_regress_error(ctx: HealthContext) -> list[Finding]:
    """T3778: the newest standing regression run ended without a verdict (ERROR)."""
    try:
        state = json.loads((_regress_dir(ctx) / "state.json").read_text())
        error = state["last_error"]
        at, reason = str(error["at"]), str(error.get("reason", ""))
    except (OSError, ValueError, KeyError, TypeError):
        return []
    if str(state.get("last_verdict_at") or "") >= at:  # ISO UTC stamps order as strings
        return []
    return [
        Finding(
            "regress-error",
            f"standing regression run of {str(error.get('sha'))[:12]} at {at} had no verdict: "
            f"{reason[:200]} (python3 -m tools.regress status)",
        )
    ]


def detect_exec_sync_failing(ctx: HealthContext) -> list[Finding]:
    """T3760: a host whose last source syncs (`tools.wt.exec_sync`, rsync into the slot exec
    dir) failed twice in a row within the hour, or that needed the tar fallback 3+ times."""
    from tools.wt import exec_sync

    log = ctx.path_env(exec_sync.ENV_LOG, ctx.repo_root / "tmp" / "wt-cache" / exec_sync.LOG_NAME)
    rows = exec_sync.read_log(log, since=ctx.now - 3600)
    findings = []
    for host, count in sorted(exec_sync.failing_hosts(rows).items()):
        last = next(row for row in reversed(rows) if row.get("host") == host)
        errors = "; ".join(last.get("errors") or [])[-160:]
        findings.append(
            Finding(
                "exec-sync-failing",
                f"{host}: last {count} source syncs failed ({errors}); "
                "check ssh, disk and the host's exec dir lock",
            )
        )
    for host, summary in sorted(exec_sync.stats(rows).items()):
        if summary["tar"] >= 3:
            findings.append(
                Finding("exec-sync-tar-fallback", f"{host}: {summary['tar']} tar fallbacks in 1 h")
            )
    return findings


def detect_verify_step_stuck(ctx: HealthContext) -> list[Finding]:
    """T3678/T3856: report live stuck verifies and remove provably stale progress records."""
    from tools.wt import remote_exec, verify_progress

    directory = ctx.path_env(
        verify_progress.ENV_DIR, ctx.repo_root / "tmp" / "remote-verify-progress"
    )
    findings = []
    for record in verify_progress.read_all(directory):
        try:
            pid, started = int(record["pid"]), float(record["started"])
        except (KeyError, TypeError, ValueError):
            continue
        host, step = record.get("host", "?"), record.get("step", "?")
        age = max(0.0, ctx.now - started)
        alive = verify_progress.pid_alive(pid, record.get("pid_namespace"))
        stale = age > 2 * remote_exec.DEFAULT_COMMAND_TIMEOUT_S
        if alive is False or stale:
            try:
                Path(record["file"]).unlink()
            except OSError as error:
                findings.append(
                    Finding(
                        "verify-step-orphan",
                        f"cannot remove stale progress file {record['file']}: {error}",
                    )
                )
            else:
                print(f"HEALTH cleanup verify-step-orphan removed {record['file']}")
        elif alive is True and age > verify_progress.STUCK_AFTER_S:
            findings.append(
                Finding(
                    "verify-step-stuck",
                    f"remote verify step {step} on {host} running {int(age) // 60} "
                    f"min (pid {pid}); read the merge_task log for the last 'remote verify:' line",
                )
            )
    return findings


def detect_port_heatmap_stale(ctx: HealthContext) -> list[Finding]:
    """I-T3632 / T3736: the README heatmap and badges come from the `progress` branch, written by
    tools.progress_publish. Fires when its last run failed (tmp/progress-publish/last-result.json),
    or the last success (or, without a status file, the origin/progress tip) is over 24 h old.
    Silent while the kill switch tmp/progress-publish/off is present."""
    state = ctx.repo_root / "tmp" / "progress-publish"
    if (state / "off").exists():
        return []
    try:
        last = json.loads(_read(state / "last-result.json"))
    except ValueError:
        last = {}
    if last.get("exit") not in (None, 0):
        return [
            Finding(
                "port-heatmap-stale",
                f"last progress publish failed: {str(last.get('detail', ''))[:120]} "
                "(tmp/progress-publish/run.log, docs/t3736-progress-publisher.md)",
            )
        ]
    stamp = str(last.get("ok_at", ""))
    if not stamp:
        result = ctx.runner(
            [
                "git",
                "-C",
                str(ctx.repo_root),
                "log",
                "-1",
                "--format=%ct",
                "refs/remotes/origin/progress",
            ],
            30,
        )
        stamp = result.out.strip() if result.ok else ""
    if stamp.isdigit() and ctx.now - int(stamp) > 86400:
        return [
            Finding(
                "port-heatmap-stale",
                f"the progress branch was last published {int(ctx.now - int(stamp)) // 3600} h ago"
                " (python3 -m tools.progress_publish request; tmp/progress-publish/)",
            )
        ]
    return []


def detect_pytest_flake_recurring(ctx: HealthContext) -> list[Finding]:
    if not (ctx.repo_root / "tools" / "agents" / "pytest_retry.py").is_file():
        return []
    result: CmdResult = ctx.runner(
        ["python3", "-m", "tools.agents.pytest_retry", "flakes", "--check"], 60
    )
    if result.rc == 0:
        return []
    head = ";".join(result.out.strip("\n").splitlines()[:3])
    return [
        Finding(
            "pytest-flake-recurring",
            f"{head} (tmp/wt-cache/flakes/pytest-flakes.jsonl, "
            f"docs/decisions/main-breakage-guard.md)",
        )
    ]


def detect_main_guard_down(ctx: HealthContext) -> list[Finding]:
    if not (ctx.repo_root / "tools" / "agents" / "pytest_retry.py").is_file():
        return []
    guard = ctx.repo_root / "tmp" / "main-guard"
    if (guard / "off").exists() or flock_held(guard / "loop.lock"):
        return []
    return [
        Finding(
            "main-guard-down",
            "main guard loop not running (setsid nohup python3 -m tools.agents.main_guard --loop)",
        )
    ]


WARM_MAIN_WINDOW_S = 7200
WARM_MAIN_STALL_S = 6 * 3600
WARM_MAIN_SUCCESS = ("ready", "cached")
# T3834: rows written before warm-main recorded deferrals as "skipped" (T3801 classes)
WARM_MAIN_LEGACY_SKIPS = ("remote prebuild unavailable; local fallback disabled",)


def _warm_outcome(row: dict) -> str:
    outcome = str(row["outcome"])
    if outcome == "failed" and str(row.get("error") or "").startswith(WARM_MAIN_LEGACY_SKIPS):
        return "skipped"
    return outcome


def detect_warm_main_failing(ctx: HealthContext) -> list[Finding]:
    """T3737: warm-main failed for every tip it tried in 2 h (19 of 19 failed for 8 h unseen).

    Reads `.warm-main-stats.jsonl` in the prebuilt cache. T3834: a skip (no free remote
    builder, `RemoteUnavailable`) or a superseded tip is not a failure. It fires on real build
    errors in 2 h with no success, or when no warm success happened for 6 h of attempts while
    the headroom cache shows a remote builder free.
    """
    from tools.fleet import headroom
    from tools.wt import prebuilt, remote_prebuilt

    stats = ctx.repo_root / "tmp" / "wt-cache" / "prebuilt" / prebuilt.WARM_STATS
    rows: list[tuple[float, str, str]] = []
    for line in _read(stats).splitlines():
        try:
            row = json.loads(line)
            rows.append((float(row["at"]), _warm_outcome(row), str(row.get("error") or "")))
        except (ValueError, KeyError, TypeError, AttributeError):
            continue
    recent = [row for row in rows if ctx.now - row[0] <= WARM_MAIN_WINDOW_S]
    failed = [error for _, outcome, error in recent if outcome == "failed"]
    skipped = sum(1 for _, outcome, _ in recent if outcome == "skipped")
    if failed and not any(outcome in WARM_MAIN_SUCCESS for _, outcome, _ in recent):
        return [
            Finding(
                "warm-main-failing",
                f"warm-main failed {len(failed)} time(s) in 2 h with no success ({skipped} "
                f"skipped, no free builder); last: {failed[-1][:120]} "
                "(docs/t3737-prebuilt-warm-main.md)",
            )
        ]
    stall = [row for row in rows if ctx.now - row[0] <= WARM_MAIN_STALL_S]
    history = any(ctx.now - at >= WARM_MAIN_STALL_S for at, _, _ in rows)
    if not stall or not history or any(o in WARM_MAIN_SUCCESS for _, o, _ in stall):
        return []
    free = headroom.free_builders(remote_prebuilt.WARM_BUILD_ORDER, ctx.repo_root, now=ctx.now)
    if not free:
        return []  # every builder busy, benched or absent: deferring is the designed behaviour
    return [
        Finding(
            "warm-main-failing",
            f"no warm-main success in 6 h ({len(stall)} attempt(s), "
            f"{sum(1 for _, o, _ in stall if o == 'skipped')} skipped) while builder(s) "
            f"{','.join(free)} are free; last: {(stall[-1][2] or stall[-1][1])[:120]} "
            "(docs/t3737-prebuilt-warm-main.md)",
        )
    ]


# --- gate history and stalls ----------------------------------------------------------------------


def detect_gate_repeat(ctx: HealthContext) -> list[Finding]:
    """The same non-evidence gate class on one (profile, route) twice or more within 6 h."""
    from tools import gate_history

    path = ctx.repo_root / "tmp" / "wt-cache" / "receipts" / "gate-history" / "verdicts.jsonl"
    cut = ctx.now - 6 * 3600
    groups: dict[tuple[str, str, str], list[dict]] = collections.defaultdict(list)
    for line in _read(path).splitlines():
        try:
            row = json.loads(line)
        except ValueError:
            continue
        if (
            not isinstance(row, dict)
            or row.get("ts", 0) < cut
            or row.get("class") in (None, "valid")
        ):
            continue
        groups[(row["class"], row.get("profile") or "-", row.get("route") or "-")].append(row)
    findings = []
    for (cls, profile, route), rows in sorted(groups.items()):
        rows = gate_history._dedupe(rows)
        if len(rows) < 2:
            continue
        sig = re.sub(r"[^a-z0-9]+", "-", f"{cls}-{profile}".lower()).strip("-")
        commits = sorted({str(r.get("commit") or "?") for r in rows})
        findings.append(
            Finding(
                f"gate-repeat-{sig}",
                f"{len(rows)}x {cls} on {profile}/{route} in 6 h (commits "
                f"{','.join(c[:10] for c in commits[:4])}; "
                f"reason {rows[-1].get('reason')}); fix the cause or file a task, do not rerun "
                "(python3 -m tools.gate_history show)",
            )
        )
    return findings


def detect_incident_class_recurred(ctx: HealthContext) -> list[Finding]:
    """Alert on records filed during the last 24h; legacy date-only records use day precision."""
    path = ctx.repo_root / "tools" / "data" / "incident_classes.json"
    try:
        registry = json.loads(path.read_text())
        now = datetime.datetime.fromtimestamp(ctx.now, datetime.UTC)
        cutoff = now - datetime.timedelta(hours=24)
        recent_by_class: dict[str, list[str]] = collections.defaultdict(list)
        for record in (ctx.repo_root / "docs" / "incidents").glob("*.md"):
            text = _read(record)
            class_match = re.search(r"^- class: ([a-z0-9-]+)$", text, re.M)
            if not class_match:
                continue
            timestamp = re.search(r"^- created_at: (.+)$", text, re.M)
            date = re.search(r"^- date: (\d{4}-\d{2}-\d{2})$", text, re.M)
            is_recent = False
            if timestamp:
                filed = datetime.datetime.fromisoformat(timestamp.group(1).replace("Z", "+00:00"))
                is_recent = filed.tzinfo is not None and filed >= cutoff
            elif date:
                # Legacy records lack time-of-day precision.
                filed_date = datetime.date.fromisoformat(date.group(1))
                is_recent = filed_date >= cutoff.date()
            if is_recent:
                recent_by_class[class_match.group(1)].append(record.stem)
        found = []
        for item in registry.get("classes", []):
            recent = recent_by_class.get(item["id"], [])
            if recent:
                found.append(
                    Finding(
                        f"incident-class-recurred-{item['id']}",
                        f"new incident(s): {', '.join(recent)}",
                    )
                )
        return found
    except (OSError, ValueError, TypeError, KeyError):
        return []


def detect_stall_lane(ctx: HealthContext) -> list[Finding]:
    lanes = []
    for line in _read(ctx.codexapi_dir / "heavy-lanes.txt").splitlines():
        if line.startswith("#"):
            continue
        fields = line.split()
        if fields:
            lanes.append(fields[0])
    findings = []
    for lane in lanes:
        if not lane_alive(ctx, lane):
            continue
        lane_dir = ctx.codexapi_dir / f"CX-{lane}"
        newest = newest_mtime(
            [lane_dir / "progress.txt", lane_dir / "runner.log", *lane_dir.glob("job-*.log")]
        )
        if newest is None:
            continue
        if ctx.now - newest > STUCK_SECONDS:
            findings.append(
                Finding(
                    f"stall-lane-{lane}",
                    f"CX-{lane} progress unchanged for {int(ctx.now - newest) // 60} min with an "
                    f"active runner "
                    f"(read CX-{lane}/runner.log and the latest job log)",
                )
            )
    return findings


BLOCKED_UNROUTED_SECONDS = 1800
"""A stopped BLOCKED lane gets claude-1's attention after this long (I-T3879 cross-lane block)."""


def detect_lane_blocked(ctx: HealthContext) -> list[Finding]:
    """A stopped heavy lane whose last line is REPORT: BLOCKED. sup-heavy.sh relaunches it only
    when its UNBLOCK-WHEN condition holds (tools.agents.lane_unblock); without one, or once the
    condition holds, someone has to act (CX-memory sat stopped 3.5 h, 2026-10-29)."""
    from tools.agents import lane_unblock

    findings = []
    for line in _read(ctx.codexapi_dir / "heavy-lanes.txt").splitlines():
        fields = line.split()
        if not fields or line.startswith("#") or lane_alive(ctx, fields[0]):
            continue
        lane = fields[0]
        progress = ctx.codexapi_dir / f"CX-{lane}" / "progress.txt"
        age = age_seconds(progress, ctx.now)
        report = lane_unblock.last_report(_read(progress))
        if age is None or age <= BLOCKED_UNROUTED_SECONDS or not lane_unblock.is_blocked(report):
            continue
        condition = lane_unblock.parse_condition(report)
        if condition is None:
            detail = "has no UNBLOCK-WHEN condition: route the blocker to its owner lane"
        elif lane_unblock.satisfied(condition):
            detail = "has its UNBLOCK-WHEN condition met on origin/main: relaunch it"
        else:
            continue
        findings.append(
            Finding(
                f"lane-blocked-{lane}",
                f"CX-{lane} stopped BLOCKED for {int(age) // 60} min and {detail} "
                f"(tail -1 CX-{lane}/progress.txt)",
            )
        )
    return findings


TERRA_HIGH_LANES_MAX = 4
TERRA_ESCALATED_LANES_MAX = 3
"""Concurrent heavy lanes on gpt-6.1-terra at high effort before the weekly Codex limit is at risk:
14 lanes after the 2026-10-29 07:05 reset (8 on terra high) used the whole week in about 1 h."""


def detect_terra_burst(ctx: HealthContext) -> list[Finding]:
    lanes = []
    for line in _read(ctx.codexapi_dir / "heavy-lanes.txt").splitlines():
        fields = line.split()
        if not fields or line.startswith("#") or not lane_alive(ctx, fields[0]):
            continue
        lane_dir = ctx.codexapi_dir / f"CX-{fields[0]}"
        model = _read(lane_dir / "model").strip() or (fields[3] if len(fields) > 3 else "")
        effort = _read(lane_dir / "effort").strip() or (fields[1] if len(fields) > 1 else "")
        if model == "gpt-6.1-terra" and effort == "high":
            lanes.append(fields[0])
    findings = []
    if len(lanes) > TERRA_HIGH_LANES_MAX:
        findings.append(
            Finding(
                "codex-terra-burst",
                f"{len(lanes)} live lanes on gpt-6.1-terra high (max {TERRA_HIGH_LANES_MAX}): "
                f"{', '.join(sorted(lanes))}; move routine lanes to luna (rule 35) "
                "before the weekly "
                "Codex limit is spent",
            )
        )
    escalated = [
        marker.parent.name
        for marker in ctx.codexapi_dir.glob("CX-*/terra-escalated.json")
        if lane_alive(ctx, marker.parent.name.removeprefix("CX-"))
    ]
    if len(escalated) > TERRA_ESCALATED_LANES_MAX:
        findings.append(
            Finding(
                "codex-terra-burst-escalated",
                f"{len(escalated)} live auto-escalated lanes (max {TERRA_ESCALATED_LANES_MAX}): "
                f"{', '.join(sorted(escalated))}; clear a landed READY escalation "
                "before another switch",
            )
        )
    return findings


def detect_ready_malformed(ctx: HealthContext) -> list[Finding]:
    """A lane's last progress line mentions READY-TO-MERGE but the ready-lander cannot parse it
    (CX-render-materials 2026-10-29: "dry run passed <sha>, READY-TO-MERGE posted" sat unlanded)."""
    from tools.agents.ready_lander import READY

    findings = []
    for lane_dir in sorted(ctx.codexapi_dir.glob("CX-*")):
        if "." in lane_dir.name:  # archived run (CX-<lane>.<HHMMSS>), the lander never reads it
            continue
        lines = [line for line in _read(lane_dir / "progress.txt").splitlines() if line.strip()]
        if lines and "READY-TO-MERGE" in lines[-1] and not READY.search(lines[-1]):
            findings.append(
                Finding(
                    f"ready-malformed-{lane_dir.name[3:]}",
                    f"{lane_dir.name} posted an unparseable READY line; required form "
                    "'READY-TO-MERGE <branch> <sha> dry run passed': "
                    f"{lines[-1][:120]}",
                )
            )
    return findings


LANE_STOPPED_WINDOW_S = 6 * 3600
LANE_STOP_MARKERS = ("READY posted, stopping", "STOP: 3 idle steps", "REPORT seen", "DONE seen")


def detect_lane_stopped(ctx: HealthContext) -> list[Finding]:
    """A lane runner (run.sh or run-claude.sh) that just stopped on READY, DONE, BLOCKED or idle.
    The orchestrator otherwise noticed only at the next 20-minute check (2026-10-29). The signature
    carries the stop time, so every new stop wakes the watcher once."""
    findings = []
    for lane_dir in sorted(ctx.codexapi_dir.glob("CX-*")):
        if "." in lane_dir.name or not lane_dir.is_dir():
            continue
        log = lane_dir / "runner.log"
        lines = [line for line in _read(log).splitlines() if line.strip()]
        age = age_seconds(log, ctx.now)
        if not lines or age is None or age > LANE_STOPPED_WINDOW_S:
            continue
        last = lines[-1]
        if not any(marker in last for marker in LANE_STOP_MARKERS):
            continue
        lane = lane_dir.name[3:]
        prefixes = (f"sh run.sh CX-{lane} ", f"sh run-claude.sh CX-{lane} ")
        if lane_alive(ctx, lane) or any(p.args.startswith(prefixes) for p in ctx.processes()):
            continue
        stamp = time.strftime("%H%M", time.gmtime(ctx.now - age))
        findings.append(
            Finding(
                f"lane-stopped-{lane}-{stamp}",
                f"CX-{lane} runner stopped: {last[:140]} (tail -2 CX-{lane}/progress.txt)",
            )
        )
    return findings


REPLAY_JOB = re.compile(r"tools\.gate |tools\.wt replay submit|tools\.rust_trace capture")
REPLAY_JOBS_PER_HOUR = 3


def detect_lane_replay_heavy(ctx: HealthContext) -> list[Finding]:
    """Rule 53 (owner 2026-10-29): test on captures, replays only as a final check. A lane that
    queued more than REPLAY_JOBS_PER_HOUR replay or gate jobs in the last hour without a
    'replay reason:' line in its progress file is flagged."""
    findings = []
    for lane_dir in sorted(ctx.codexapi_dir.glob("CX-*")):
        if "." in lane_dir.name or not lane_dir.is_dir():
            continue
        recent = [
            job
            for job in lane_dir.glob("job-*.sh")
            if (age_seconds(job, ctx.now) or 1e9) < 3600 and REPLAY_JOB.search(_read(job))
        ]
        if len(recent) <= REPLAY_JOBS_PER_HOUR:
            continue
        if "replay reason:" in _read(lane_dir / "progress.txt").lower():
            continue
        findings.append(
            Finding(
                f"lane-replay-heavy-{lane_dir.name[3:]}",
                f"{lane_dir.name} queued {len(recent)} replay/gate jobs in 1 h without a "
                "'replay reason:' line (rule 53: test on captures, replay only as the final gate)",
            )
        )
    return findings


def detect_stall_landing(ctx: HealthContext) -> list[Finding]:
    findings = []
    for path in sorted((ctx.repo_root / "tmp" / "merge-submit").glob("*.json")):
        if not path.is_file() or '"finished"' in _read(path):
            continue
        age = age_seconds(path, ctx.now)
        if age is None or age <= 3600:
            continue
        name = path.stem
        ident = re.sub(r"[^a-z0-9-]", "-", name)[:40]
        findings.append(
            Finding(
                f"stall-landing-{ident}",
                f"landing {name} pending for {int(age) // 60} min (tail its log in "
                f"tmp/merge-submit/)",
            )
        )
    return findings


UNDERUSED_SECONDS = 900
"""A host below its fleet.json `min` workers for this long is idle capacity (owner 2026-10-23)."""


def detect_host_launch_failing(ctx: HealthContext) -> list[Finding]:
    """T3763 (I-T3763-3): a host whose launched workers complete no step. On 2026-10-26 every
    remote Codex step failed for about 9 h (an MCP flag the remote config rejected) while the
    refill kept popping queue ids for those hosts; 25 Tier 2 items were lost."""
    findings = []
    for name, status in parse_status(ctx.status_text).items():
        if not status.reachable or not launch_health.failing(status.fields):
            continue
        found = launch_health.from_fields(status.fields)
        findings.append(
            Finding(
                f"host-launch-failing-{name}",
                f"{name}: {found.launches} launch(es), {found.failed} failed step(s), 0 completed "
                f"in {launch_health.WINDOW_S // 60} min; refills paused (launch-failing), stopped "
                "lanes re-queue their ids; read step-*.err of a lane on the host",
            )
        )
    return findings


def detect_host_underused(ctx: HealthContext) -> list[Finding]:
    """Owner 2026-10-23: "How can we stop workers from being idle so long?".

    A host below its `min` workers for 15 min (and not switched off by its `off_file`) raises a
    finding with the status fields that usually explain it (over-cap, disk, memory). droplet2 sat
    at 0-1 of 4 for hours behind a false over-cap pause and a full root disk (I-T3505-18/19)."""
    config = ctx.fleet_config()  # T3758: through tools.fleet.config, not a raw fleet.json read
    if config is None:
        return []  # an invalid config is its own finding (fleet-config-invalid)
    state_file = ctx.state_dir / "health-underused.json"
    try:
        since: dict[str, float] = json.loads(state_file.read_text())
    except (OSError, ValueError):
        since = {}
    latest: dict[str, str] = {}
    for line in ctx.status_text.splitlines():
        fields = line.split()
        if len(fields) >= 3 and fields[2].startswith("workers="):
            latest[fields[1]] = line
    findings = []
    for name, host in config.hosts.items():  # retired hosts are not in the config
        # rule 47: a remote host is execution-only; its floor is concurrent exec jobs
        unit = "workers" if host.is_local else "exec_jobs"
        want = host.low if host.is_local else host.min_exec_jobs
        line = latest.get(name)
        off = host.off_file
        switched_off = bool(off) and (ctx.codexapi_dir / str(off)).exists()
        waiting = (
            ""
            if host.is_local
            else exec_load.work_waiting(ctx.repo_root, ctx.codexapi_dir, ctx.now)
        )
        if want <= 0 or line is None or switched_off or (not host.is_local and not waiting):
            since.pop(name, None)  # an idle host with nothing to run is fine (I-T3769-3)
            continue
        match = re.search(rf" {unit}=(\d+)", line)
        have = int(match.group(1)) if match else 0
        if have >= want:
            since.pop(name, None)
            continue
        start = since.setdefault(name, ctx.now)
        limit = UNDERUSED_SECONDS if host.is_local else exec_load.UNDERUSED_AFTER_S
        if ctx.now - start >= limit:
            hints = " ".join(re.findall(r"(?:overcap|disk_free|mem_avail_gb|load)=\S+", line))
            findings.append(
                Finding(
                    f"host-underused-{name}",
                    f"{name} has {have} {unit} < min {want} for {int(ctx.now - start) // 60} min "
                    f"({hints}){f' work waiting: {waiting}' if waiting else ''}; check over-cap, "
                    "the root disk, refill decisions in "
                    "tmp/fleet/decisions.jsonl and the newest runner step-*.err on the host",
                )
            )
    try:
        ctx.state_dir.mkdir(parents=True, exist_ok=True)
        state_file.write_text(json.dumps(since))
    except OSError:
        pass
    return findings


def detect_bottlenecks(ctx: HealthContext) -> list[Finding]:
    """Surface shared fleet bottlenecks in the normal controller health cycle."""
    return bottlenecks.detect(
        ctx.repo_root,
        ctx.codexapi_dir,
        now=ctx.now,
        runner=ctx.runner,
        status_text=ctx.status_text,
        disk_free_gb=ctx.free_gb(),
    )


def detect_light_queue(ctx: HealthContext) -> list[Finding]:
    """Report an empty validated REA queue and IDs rejected by the last feed/refill."""
    queue_path = ctx.codexapi_dir / "p3queue.txt"
    report_path = ctx.codexapi_dir / "queue-validation.json"
    # A supervisor checkout without queue artifacts is not evidence that the
    # managed light-worker queue is empty (notably in isolated detector tests).
    if not queue_path.exists() and not report_path.exists():
        return []
    try:
        depth = len(queue_path.read_text().split())
    except OSError:
        depth = 0
    try:
        report = json.loads(report_path.read_text())
    except (OSError, json.JSONDecodeError):
        report = {}
    findings: list[Finding] = []
    if depth == 0 or report.get("queue_empty") is True or report.get("valid_count") == 0:
        findings.append(Finding("light-queue-empty", "no resolvable open IDs remain in p3queue"))
    try:
        age = max(0.0, ctx.now - float(report.get("validated_at", 0)))
        rejected = int(report.get("rejected_count", 0)) if age <= 900 else 0
    except (TypeError, ValueError):
        rejected = 0
    if rejected:
        findings.append(
            Finding("light-queue-rejected", f"{rejected} ids rejected by feed/refill validation")
        )
    return findings


# --- orchestration --------------------------------------------------------------------------------


def detect_git_ssh_multiplex(ctx: HealthContext) -> list[Finding]:
    """T3682: the main checkout must multiplex git SSH (core.sshCommand with ControlMaster).

    Without it every git command opens a new connection and the loaded git server reset ~30%
    of them (2026-10-24). Remotes get it from provision's SSH_GIT; the local repo config is
    shared by both containers and is checked here.
    """
    config = ctx.repo_root / ".git" / "config"
    try:
        text = config.read_text()
    except OSError:
        return []  # a worktree checkout (.git is a file): the main checkout is checked elsewhere
    match = re.search(r"^\s*sshCommand\s*=\s*(.+)$", text, re.MULTILINE | re.IGNORECASE)
    if match and "ControlMaster=auto" in match.group(1):
        return []
    return [
        Finding(
            "git-ssh-not-multiplexed",
            "main checkout core.sshCommand lacks ControlMaster; set it to "
            "tools.fleet.provision.LOCAL_SSH_GIT (git config core.sshCommand ...)",
        )
    ]


def detect_cache_hit_rate(ctx: HealthContext) -> list[Finding]:
    """Report cache/host hit rates below half of their 24-hour baseline."""
    path = ctx.repo_root / "tmp/wt-cache/cache-telemetry"
    return [
        Finding(
            "cache-hit-rate",
            f"host={host} cache={cache} baseline={baseline:.1%} observed={observed:.1%}",
        )
        for host, cache, baseline, observed in degraded_caches(path, now=ctx.now)
    ]


def detect_build_cache_canary(ctx: HealthContext) -> list[Finding]:
    """Use the exact failing landing gate, including absent and expired evidence."""
    from tools import build_health_policy

    return [
        Finding("build-cache-canary", problem)
        for problem in build_health_policy.check(ctx.repo_root, now=ctx.now)
    ]


def _drift_hosts(config: FleetConfig) -> list[tuple[str, HostConfig]]:
    """Hosts the drift checks probe: remote and not switched off. The local host has no ssh
    route (an empty-hostname ssh failed every tick) and an off host is never provisioned, so
    either one only repeated an unfixable finding (I-T3865-11)."""
    from tools.wt import remote_replay

    off = remote_replay.switched_off()
    return [
        (name, host) for name, host in config.hosts.items() if not host.is_local and name not in off
    ]


LANDING_STARVED_S = 30 * 60
#: Commit subjects of a lane landing on main (manual_land merges and the lander's view regen).
LANE_LANDING = re.compile(
    r"^(Merge commit '[0-9a-f]+' into c1/manual-|Regenerate views \(manual_land |"
    r"Merge task/(?!cx-t2continuous))"
)


def detect_landing_starved(ctx: HealthContext) -> list[Finding]:
    """Owner 2026-10-28: 'If nothing arrives in landings for 30 minutes there should be an alert'.
    No lane landing on origin/main for LANDING_STARVED_S while Codex lanes run (the landing path
    hung for 3 h unseen, I-T3885)."""
    lanes = [p for p in ctx.processes() if re.match(r"sh run(-claude)?\.sh CX-", p.args or "")]
    if not lanes:
        return []
    since = int(ctx.now - LANDING_STARVED_S)
    result = ctx.runner(
        ["git", "-C", str(ctx.repo_root), "log", "origin/main", f"--since=@{since}", "--format=%s"],
        60,
    )
    if not result.ok:
        return [Finding("landing-starved-check-failed", (result.err or result.out)[:160])]
    if any(LANE_LANDING.match(line) for line in result.out.splitlines()):
        return []
    return [
        Finding(
            "landing-starved",
            f"no lane landing on origin/main for {LANDING_STARVED_S // 60} min while "
            f"{len(lanes)} lanes run; check tmp/ready-lander/state.json, a probe "
            "TSFP_LAND_DRY_RUN=1 sh tools/agents/manual_land.sh, and tools.dispatch status",
        )
    ]


def detect_light_workers_off(ctx: HealthContext) -> list[Finding]:
    """The Tier 2 queue has work but every host allowed light workers is switched off. Remote
    hosts run no light workers (rule 47), so one local off-file stopped every Tier 2 lane for
    ~1 day unseen (I-T3865-16: tmp/codexapi/local.off from 2026-10-25)."""
    from tools.fleet.config import load_config

    try:
        config = load_config(root=ctx.repo_root)
        queue = config.codexapi_dir / "p3queue.txt"
        depth = sum(1 for line in queue.read_text().splitlines() if line.strip())
    except (OSError, ValueError, KeyError):
        return []
    eligible = [host for host in config.hosts.values() if host.is_local]
    off = [
        host.name
        for host in eligible
        if host.off_file and (config.codexapi_dir / str(host.off_file)).exists()
    ]
    if depth and eligible and len(off) == len(eligible):
        return [
            Finding(
                "t2-light-workers-off",
                f"{depth} queued Tier 2 jobs but every light-worker host is off "
                f"({', '.join(off)}); remove the off-file or lower light-local.max",
            )
        ]
    return []


def detect_codex_runner_drift(ctx: HealthContext) -> list[Finding]:
    """Report remote hosts whose runner files differ (sha256) from the tools/fleet/runner render."""
    from tools.fleet.config import load_config
    from tools.fleet.provision import _make_ctx, runner_check

    findings: list[Finding] = []
    try:
        config = load_config(root=ctx.repo_root)
        for name, host in _drift_hosts(config):
            host_ctx = _make_ctx(config, host, ctx.runner, False, False, ctx.runner, Path.home())
            if not runner_check(host_ctx).ok:
                findings.append(Finding("codex-runner-drift", name))
    except (OSError, ValueError, KeyError, RuntimeError) as error:
        return [Finding("codex-runner-drift-check-failed", str(error)[:160])]
    return findings


def detect_host_python_deps_drift(ctx: HealthContext) -> list[Finding]:
    """Report remote hosts whose venv stamp differs from the current uv.lock export."""
    from tools.fleet.config import load_config
    from tools.fleet.provision import _make_ctx, python_deps_check

    findings: list[Finding] = []
    try:
        config = load_config(root=ctx.repo_root)
        for name, host in _drift_hosts(config):
            host_ctx = _make_ctx(config, host, ctx.runner, False, False, ctx.runner, Path.home())
            if not python_deps_check(host_ctx).ok:
                findings.append(Finding("host-python-deps-drift", name))
    except (OSError, ValueError, KeyError, RuntimeError, subprocess.SubprocessError) as error:
        return [Finding("host-python-deps-drift-check-failed", str(error)[:160])]
    return findings


def detect_provisioned_script_drift(ctx: HealthContext) -> list[Finding]:
    """Report tracked remote script payloads whose installed bytes differ from source."""
    from tools.fleet.config import load_config
    from tools.fleet.provision import _make_ctx, _script_payload_check

    findings: list[Finding] = []
    try:
        config = load_config(root=ctx.repo_root)
        for name, host in _drift_hosts(config):
            host_ctx = _make_ctx(
                config,
                host,
                ctx.runner,
                False,
                False,
                ctx.runner,
                Path.home(),
            )
            if not _script_payload_check(host_ctx).ok:
                findings.append(Finding("provisioned-script-drift", name))
    except (OSError, ValueError, KeyError, RuntimeError) as error:
        return [Finding("provisioned-script-drift-check-failed", str(error)[:160])]
    return findings


def detect_prebuilt_capture_mismatch(ctx: HealthContext) -> list[Finding]:
    """I-T3784-3: a prebuilt restore or reuse refused a host whose capture build kind differs
    from the request (a capture host under a normal key, or the reverse) in the last day."""
    from tools.t2capture import continuous
    from tools.wt import prebuilt

    rows: list[str] = []
    for cache in (
        ctx.repo_root / "tmp/wt-cache/prebuilt",
        ctx.repo_root / "tmp" / continuous.STATE_NAME / "hosts",
    ):
        rows += prebuilt.capture_mismatch_findings(cache, ctx.now)
    if not rows:
        return []
    detail = f"{len(rows)} refusal(s): {'; '.join(rows[-5:])}; evict the shared-store entry"
    return [Finding("prebuilt-capture-mismatch", detail + " (docs/incidents/I-T3784-3.md)")]


def detect_t2_continuous_stale(ctx: HealthContext) -> list[Finding]:
    """T3784: the continuous capture Tier 2 service beats every minute (also mid-pass); a live
    heartbeat older than 30 min means a hung pass or a dead loop the supervisor did not restart.
    T3826: also the harness stage failing FAIL_HEALTH passes in a row."""
    from tools.t2capture import continuous, harness_stage

    base = ctx.repo_root / "tmp" / continuous.STATE_NAME
    found = (
        continuous.stale_findings(base, ctx.now)
        + continuous.build_findings(base)
        + continuous.stale_findings_24h(base, ctx.now)
        + continuous.carry_findings(base)
        + harness_stage.findings(base)
    )
    return [Finding(i, d) for i, d in found]


def detect_orchestrator_silent(ctx: HealthContext) -> list[Finding]:
    """T3814: an orchestrator silent over 30 min with unread messages addressed to it."""
    from tools.agents import orchestrator_silence

    root = ctx.path_env("TSFP_COORD_DIR", ctx.repo_root / ".git" / "agent-coord")
    return [Finding(i, d) for i, d in orchestrator_silence.findings(root, ctx.now)]


DETECTORS: tuple[Callable[[HealthContext], list[Finding]], ...] = (
    detect_orchestrator_silent,
    detect_host_launch_failing,
    detect_verify_step_stuck,
    detect_exec_sync_failing,
    detect_cache_hit_rate,
    detect_build_cache_canary,
    detect_light_workers_off,
    detect_landing_starved,
    detect_codex_runner_drift,
    detect_host_python_deps_drift,
    detect_provisioned_script_drift,
    detect_git_ssh_multiplex,
    detect_codex_home,
    detect_zero_input_steps,
    detect_claude_runner_failing,
    detect_stuck_job,
    detect_duplicate_runner,
    detect_merge_queue_stall,
    detect_disk_low,
    detect_disk_falling,
    detect_disk_budget,
    detect_root_disk_project_data,
    detect_pids_high,
    detect_local_overload,
    detect_local_game_hosts,
    detect_coord_wait_missing,
    detect_host_health_refusals,
    detect_origin_prune_failed,
    detect_origin_branches,
    detect_origin_prune_stale,
    detect_coordsvc_down,
    detect_coordsvc_shadow_diffs,
    detect_coordsvc_outbox,
    detect_merge_verify_sigsegv,
    detect_merge_reported_not_on_origin,
    detect_no_std_remote_skew,
    detect_integrity_changed,
    detect_capguard_not_running,
    detect_cargo_target_over_cap,
    detect_owner_file_outside_cache,
    detect_merge_batch_idle,
    detect_primary_ahead,
    detect_merge_lock_hog,
    detect_merge_verify_rounds,
    detect_merge_candidate_rounds,
    detect_droplet_tunnel_down,
    detect_droplet2_tunnel_down,
    detect_claude_keys_exhausted,
    detect_remote,
    detect_verify_local_share,
    detect_verify_all_hosts_benched,
    detect_regress_fault,
    detect_regress_stale,
    detect_regress_error,
    detect_pytest_flake_recurring,
    detect_port_heatmap_stale,
    detect_main_guard_down,
    detect_gate_repeat,
    detect_incident_class_recurred,
    detect_stall_lane,
    detect_lane_blocked,
    detect_terra_burst,
    detect_ready_malformed,
    detect_lane_stopped,
    detect_lane_replay_heavy,
    detect_stall_landing,
    detect_pipeline_stalled,
    detect_host_underused,
    detect_warm_main_failing,
    detect_light_queue,
    detect_bottlenecks,
    detect_t2_continuous_stale,
    detect_prebuilt_capture_mismatch,
)


def run_all(ctx: HealthContext) -> list[Finding]:
    """Run every detector; an exception becomes one `detector-error` finding and the rest still
    run."""
    findings: list[Finding] = []
    for detector in DETECTORS:
        name = detector.__name__.removeprefix("detect_")
        try:
            findings.extend(detector(ctx))
        except Exception as error:  # noqa: BLE001 - isolation is the point
            findings.append(Finding("detector-error", f"{name}: {error}"))
    return findings


def append_health_log(findings: list[Finding], path: Path, stamp: str | None = None) -> None:
    """Append one `HH:MM:SS HEALTH <id> <detail>` line per finding."""
    if not findings:
        return
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("a") as handle:
        for finding in findings:
            handle.write(finding.line(stamp) + "\n")


def default_context(
    config: FleetConfig,
    runner: Runner,
    statuses: Mapping[str, HostStatus],
    now: float,
    state_dir: Path,
) -> HealthContext:
    """The context the controller uses: paths from the fleet config, status text from the last
    probe."""
    state_dir.mkdir(parents=True, exist_ok=True)
    return HealthContext(
        now=now,
        codexapi_dir=config.codexapi_dir,
        repo_root=config.root,
        runner=runner,
        state_dir=state_dir,
        status_text=format_status(statuses),
        fleet=config,
    )
