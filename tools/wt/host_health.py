"""Per-host host-health admission guard for remote build and replay hosts (T3516).

Some remote hosts share their machine with the owner's services: the server (<private>) runs
the owner's Forgejo and its Postgres. On 2026-10-21 about 19:15 our remote builds (load about 70,
3 GB free) on top of everything else pushed the host to load 170 and about 14 GB available;
Postgres crashed into recovery and the forge was down for 12 minutes (incident 61 in
docs/t3505-fleet-incident-register.md, cause INFERRED). Our cgroup never hit its limit, so only a
host-wide check sees this.

A host opts in with a `host_health` object in tools/data/remote_hosts.json (all keys optional,
absent object = no guard, so the change is additive):

    "host_health": {
      "min_host_mem_mb": 12000,     # host MemAvailable (/proc/meminfo) needed to admit
      "max_load_per_core": 2.0,     # admit only while 1-min load / cores is below this
      "max_builds": 2,              # concurrent builds from us (held slot locks, ours included)
      "max_cargo_jobs": 8           # CARGO_BUILD_JOBS (and -j / --jobs) ceiling on that host
    }

The check runs twice. At probe time (`evaluate` on the probe values) so a loaded host is skipped
before the tree is staged, and again inside the remote command that holds the slot lock
(`gate_command`), because staging takes 40-150 s and the host changes meanwhile (the same race
that produced exit 75). A refusal is exit HEALTH_EXIT with a `@@HOST_HEALTH <reasons>` line. It
says nothing about the candidate or the transport: callers put the host back into its wait window
and never mark it degraded.
"""

from __future__ import annotations

import fcntl
import json
import os
import re
import time
from collections.abc import Callable, Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path

from tools import host_registry

REGISTRY = host_registry.REGISTRY
HEALTH_EXIT = 76
"""Exit code of the remote run-time gate (75 is a busy slot, 70/71 staging and fetch)."""
HEALTH_MARK = "@@HOST_HEALTH"
HEALTH_PREFIX = "host_health:"
"""Prefix of every probe-time refusal reason, so callers tell it from disk or tool refusals."""
UNREADABLE = "host_health_unreadable"
DECISION = "host_health_refusal"
"""`decision` value of the refusal rows in tmp/verify-admission.jsonl."""

BACKOFF_REL = Path("tmp/wt-cache/host-health-backoff")
"""Per-host back-off state, in the main checkout so every session and worktree shares it."""
BACKOFF_ENV = "TSFP_HOST_HEALTH_BACKOFF_DIR"
BACKOFF_BASE_S = 600.0
BACKOFF_MAX_S = 1800.0
LOAD_BACKOFF_BASE_S = 120.0
"""Back-off after a load-only refusal. Load is a 1-minute average that falls within minutes once
our builds finish, so the 600 s base benched the laptop for 9 more minutes after it was idle again
(load 15 on 16 cores, 2026-11-01). Load refusals start here, doubling up to BACKOFF_BASE_S."""
BACKOFF_PREFIX = "host_health_backoff_"
"""Reason of a probe skipped by the back-off (not a refusal: nothing was asked of the host)."""

_JOBS_ENV = re.compile(r"\bCARGO_BUILD_JOBS=(\d+)")
_JOBS_FLAG = re.compile(r"(?<![\w-])(-j|--jobs)([ =]?)(\d+)\b")


@dataclass(frozen=True)
class HostHealth:
    min_host_mem_mb: int = 0
    max_load_per_core: float = 0.0
    max_builds: int = 0
    max_cargo_jobs: int = 0

    @property
    def active(self) -> bool:
        return bool(
            self.min_host_mem_mb or self.max_load_per_core or self.max_builds or self.max_cargo_jobs
        )


def from_entry(entry: Mapping[str, object] | None) -> HostHealth | None:
    """The guard configured on a registry entry, or None when the host has none."""
    raw = (entry or {}).get("host_health")
    if not isinstance(raw, Mapping):
        return None
    config = HostHealth(
        min_host_mem_mb=int(raw.get("min_host_mem_mb", 0)),  # type: ignore[call-overload]
        max_load_per_core=float(raw.get("max_load_per_core", 0.0)),  # type: ignore[arg-type]
        max_builds=int(raw.get("max_builds", 0)),  # type: ignore[call-overload]
        max_cargo_jobs=int(raw.get("max_cargo_jobs", 0)),  # type: ignore[call-overload]
    )
    return config if config.active else None


def for_host(name: str, path: Path = REGISTRY) -> HostHealth | None:
    """The registry's guard for host `name` (None for an unknown, unguarded or unreadable one)."""
    return facts_for_host(name, path)[0]


def facts_for_host(name: str, path: Path = REGISTRY) -> tuple[HostHealth | None, int, int]:
    """(guard, host cores, build slots) of registry host `name`, for the callers that do not
    hold a remote_exec.HostSpec (replays and remote prebuilt builds)."""
    for entry in host_registry.hosts_or_empty(path):  # T3754: the one registry loader
        if entry.get("name") == name:
            return from_entry(entry), int(entry.get("cores", 1)), int(entry.get("slots", 1))
    return None, 1, 1


CAPTURE_RESERVED_KEY = "capture_reserved_cores"
PIN_VAR = "TSFP_BUILD_PIN"


def capture_reserved_cores(name: str, path: Path = REGISTRY) -> int:
    """T3803: cores of registry host `name` that builds leave to capture replays (0 = none)."""
    for entry in host_registry.hosts_or_empty(path):
        if entry.get("name") == name:
            return max(0, int(entry.get(CAPTURE_RESERVED_KEY, 0)))
    return 0


def build_pin_assign(reserved: int) -> str:
    """T3803: shell assignment of $TSFP_BUILD_PIN, the prefix that keeps a build off the last
    `reserved` CPUs so capture replays (one guest thread at 100%) always find a free core.

    Priority alone cannot do it on every host: the laptop runs its whole session under
    SCHED_IDLE (MEASURED 2026-10-26), where nice values have no effect. The CPU count is read on
    the host at run time (`nproc --all`); with no taskset, or too few CPUs, the prefix is empty
    and the build runs unpinned."""
    if reserved <= 0:
        return f"{PIN_VAR}="
    return (
        f"{PIN_VAR}=$(n=$(nproc --all 2>/dev/null || echo 0); last=$((n - {int(reserved)} - 1)); "
        'if [ "$last" -ge 1 ] && command -v taskset >/dev/null 2>&1; '
        'then echo "taskset -c 0-$last"; fi)'
    )


def evaluate(
    config: HostHealth | None,
    *,
    mem_mb: int | None,
    load1: float | None,
    cores: int,
    builds: int | None,
) -> list[str]:
    """Refusal reasons (empty = admit). `builds` counts our builds INCLUDING the one being
    admitted (probe time: held slots + 1; run time: held slots, ours among them); None skips
    that limit (replays are not build slots). A value the host did not report fails closed."""
    if config is None:
        return []
    reasons: list[str] = []
    if config.min_host_mem_mb:
        if mem_mb is None:
            reasons.append(UNREADABLE + "_mem")
        elif mem_mb < config.min_host_mem_mb:
            reasons.append(f"host_mem_{mem_mb}MB_below_{config.min_host_mem_mb}MB")
    if config.max_load_per_core:
        if load1 is None:
            reasons.append(UNREADABLE + "_load")
        elif load1 / max(1, cores) >= config.max_load_per_core:
            reasons.append(
                f"host_load_{load1:g}_over_{cores}_cores_not_below_{config.max_load_per_core:g}"
            )
    if config.max_builds and builds is not None and builds > config.max_builds:
        reasons.append(f"builds_{builds}_above_max_{config.max_builds}")
    return reasons


def prefixed(reasons: Sequence[str]) -> list[str]:
    return [HEALTH_PREFIX + reason for reason in reasons]


def is_refusal(reasons: Sequence[str]) -> bool:
    """True when every reason is a host-health one (a host-health-only refusal waits)."""
    return bool(reasons) and all(reason.startswith(HEALTH_PREFIX) for reason in reasons)


def probe_fragment() -> str:
    """Shell lines printing the host-wide values the probe-time check needs."""
    return (
        "echo \"host_mem_mb=$(awk '/^MemAvailable:/ {print int($2/1024)}' /proc/meminfo)\"; "
        "echo \"load1=$(cut -d' ' -f1 /proc/loadavg)\"; "
    )


def gate_command(
    config: HostHealth | None,
    *,
    cores: int,
    slot_files: Sequence[str],
    meminfo: str = "/proc/meminfo",
    loadavg: str = "/proc/loadavg",
) -> str:
    """Shell that refuses (echo `@@HOST_HEALTH <reasons>`, exit HEALTH_EXIT) when the host is
    too loaded. Run it AFTER the slot lock is held (its fd is counted among `max_builds`) and
    before anything is unpacked. `slot_files` are the already shell-quoted slot lock paths."""
    if config is None:
        return "true"
    lines = [
        f"hh_m=$(awk '/^MemAvailable:/ {{print int($2/1024)}}' {meminfo})",
        f"hh_l=$(cut -d' ' -f1 {loadavg})",
        "hh_b=0",
    ]
    if config.max_builds and slot_files:
        lines.append(
            f"for hh_f in {' '.join(slot_files)}; do "
            '( exec 6>"$hh_f"; flock -n 6 ) 2>/dev/null || hh_b=$((hh_b+1)); done'
        )
    lines.append("hh_r=")
    if config.min_host_mem_mb:
        lines.append(
            f'[ "${{hh_m:-0}}" -ge {config.min_host_mem_mb} ] || '
            f'hh_r="$hh_r host_mem_${{hh_m:-unreadable}}MB_below_{config.min_host_mem_mb}MB"'
        )
    if config.max_load_per_core:
        lines.append(
            f'awk -v l="${{hh_l:-999999}}" -v c={max(1, cores)} -v x={config.max_load_per_core:g} '
            "'BEGIN {exit !(l / c < x)}' || "
            f'hh_r="$hh_r host_load_${{hh_l:-unreadable}}_over_{cores}_cores_not_below_'
            f'{config.max_load_per_core:g}"'
        )
    if config.max_builds and slot_files:
        lines.append(
            f'[ "$hh_b" -le {config.max_builds} ] || '
            f'hh_r="$hh_r builds_${{hh_b}}_above_max_{config.max_builds}"'
        )
    lines.append(f'if [ -n "$hh_r" ]; then echo "{HEALTH_MARK}$hh_r"; exit {HEALTH_EXIT}; fi')
    return "; ".join(lines)


def refusal_in(log: str) -> list[str]:
    """Reasons of the last `@@HOST_HEALTH` line in a remote log (empty when there is none)."""
    found = [line for line in log.splitlines() if line.startswith(HEALTH_MARK)]
    return found[-1][len(HEALTH_MARK) :].split() if found else []


def cap_jobs(command: str, cap: int) -> str:
    """Lower `CARGO_BUILD_JOBS=N`, `-j N` and `--jobs N` inside a command to at most `cap`
    (a portable verify step carries the local host's job count, which would override the
    host ceiling exported around it)."""
    if cap <= 0:
        return command
    command = _JOBS_ENV.sub(lambda m: f"CARGO_BUILD_JOBS={min(int(m.group(1)), cap)}", command)
    return _JOBS_FLAG.sub(lambda m: f"{m.group(1)}{m.group(2)}{min(int(m.group(3)), cap)}", command)


# --- refusal back-off (T3588) ------------------------------------------------------------------
#
# A refused host used to go back to a 60 s wait window, so busy periods probed the owner's Forgejo
# host about 3 times a minute (about 200 refusals an hour all night, MEASURED in
# tmp/verify-admission.jsonl). After a refusal the host is not probed at all for BACKOFF_BASE_S,
# doubling per repeated refusal up to BACKOFF_MAX_S (10, 20, 30, 30 ... minutes); the first
# admission resets it. The state is one small JSON file per host under tmp/wt-cache, updated under
# an flock so concurrent sessions agree; a refusal noted while the back-off is already running
# (several processes refusing the same load) does not escalate it.


def backoff_dir(root: Path | None = None) -> Path:
    """Directory of the shared back-off files (`BACKOFF_ENV` overrides it, for tests)."""
    override = os.environ.get(BACKOFF_ENV)
    if override:
        return Path(override)
    if root is None:
        root = Path(__file__).resolve().parents[2]
    try:
        from tools.wt import core

        root = core.main_root(core.worktree_root(root))
    except (Exception, SystemExit):  # noqa: BLE001 - bookkeeping must not fail a verify
        pass
    return root / BACKOFF_REL


def backoff_seconds(level: int, load_only: bool = False) -> float:
    """Back-off after the `level`-th consecutive refusal (1 = first). A load-only refusal starts
    at LOAD_BACKOFF_BASE_S and never exceeds BACKOFF_BASE_S."""
    if load_only:
        return min(BACKOFF_BASE_S, LOAD_BACKOFF_BASE_S * 2 ** max(0, level - 1))
    return min(BACKOFF_MAX_S, BACKOFF_BASE_S * 2 ** max(0, level - 1))


def is_load_only(reasons: Sequence[str]) -> bool:
    """True when every refusal reason is a host-load one (transient, see LOAD_BACKOFF_BASE_S)."""
    return bool(reasons) and all("host_load_" in reason for reason in reasons)


def _read_backoff(path: Path) -> tuple[int, float]:
    try:
        data = json.loads(path.read_text())
        return int(data["level"]), float(data["until"])
    except (OSError, ValueError, KeyError, TypeError):
        return 0, 0.0


def backoff_remaining(name: str, *, root: Path | None = None, now: float | None = None) -> float:
    """Seconds left of `name`'s back-off (0 = probe it). Lock-free: a torn read is a miss."""
    _, until = _read_backoff(backoff_dir(root) / f"{name}.json")
    return max(0.0, until - (time.time() if now is None else now))


def _locked_update(directory: Path, name: str, update: Callable[[Path], float]) -> float:
    """Run `update(state file)` under an exclusive flock of the host's lock file."""
    try:
        directory.mkdir(parents=True, exist_ok=True)
        with (directory / f"{name}.lock").open("w") as lock:
            fcntl.flock(lock, fcntl.LOCK_EX)
            return update(directory / f"{name}.json")
    except OSError:
        return 0.0


def note_refusal(
    name: str,
    *,
    root: Path | None = None,
    now: float | None = None,
    reasons: Sequence[str] = (),
) -> float:
    """Record a host-health refusal of `name`; returns the back-off now in force (seconds).
    `reasons` (the refusal's reasons, when known) picks the load-only schedule."""
    moment = time.time() if now is None else now
    load_only = is_load_only(reasons)

    def update(path: Path) -> float:
        level, until = _read_backoff(path)
        if until > moment:  # already backing off: another process saw the same load
            return until - moment
        seconds = backoff_seconds(level + 1, load_only)
        scratch = path.with_suffix(".tmp")
        scratch.write_text(json.dumps({"level": level + 1, "until": moment + seconds}) + "\n")
        scratch.replace(path)
        return seconds

    return _locked_update(backoff_dir(root), name, update)


def note_admission(name: str, *, root: Path | None = None) -> None:
    """The host was probed healthy: forget its back-off level."""
    directory = backoff_dir(root)
    if not (directory / f"{name}.json").exists():
        return  # the common case on every healthy probe: no lock, no write

    def update(path: Path) -> float:
        path.unlink(missing_ok=True)
        return 0.0

    _locked_update(directory, name, update)


def backoff_reason(seconds: float) -> str:
    """Probe reason of a host skipped without ssh during its back-off."""
    return f"{BACKOFF_PREFIX}{int(seconds)}s"
