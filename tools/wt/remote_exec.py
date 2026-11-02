"""Remote build, verify and gate executors (T3435).

Offloads merge-verify command sets and Tier 2 gate replays from the memory-limited main
host to the owner's laptop, Steam Deck and server (tools/data/remote_hosts.json, mirroring
docs/remote-build-hosts.md). Built on the T3407/T3428 transport in tools.wt.remote_replay
(`_ssh`, `_copy_input` with SHA-256 verification); nothing here copies owner files.

Opt-in: `TSFP_REMOTE_EXEC=1` (or `python3 -m tools.gate --remote`). Guards: refuse a host
below its disk floor (30 GB free) or above its tsfp data cap (100 GB), one heavy slot per
host (flock), per-command `timeout`, run directories removed after every run.
"""

from __future__ import annotations

import argparse
import contextlib
import json
import os
import re
import secrets
import shlex
import subprocess
import sys
import tempfile
import threading
import time
from collections.abc import Callable, Iterator, Sequence
from dataclasses import asdict, dataclass, field
from pathlib import Path

from tools.wt import (
    cargo_registry_sync,
    cdiff_bundle,
    disk_mounts,
    exec_sync,
    host_budget,
    host_health,
    owner_links,
    remote_replay,
    retired_hosts,
    sccache_hosts,
    sccache_shared,
    verify_progress,
)

REGISTRY = Path(__file__).resolve().parents[2] / "tools" / "data" / "remote_hosts.json"
ENABLE_ENV = "TSFP_REMOTE_EXEC"
OWNER_INPUTS = ("disc", "recordings", "saves")
LOCK_WAIT_S = 1800
DEFAULT_COMMAND_TIMEOUT_S = 1800
LOCK_HEARTBEAT_S = 60  # T3678: the remote slot-lock wait prints a heartbeat this often
PROBE_TIMEOUT_S = 90
DEFAULT_TARGET_CAP_GB = 6
SCCACHE_WRAPPER = Path(__file__).resolve().parent / "sccache-rustc.sh"
SCCACHE_BIN = "$HOME/.local/lib/tsfp/sccache"
SCCACHE_PASS = "$HOME/.config/tsfp/sccache-webdav.pass"
SLOT_LOCK = "~/.cache/tsfp-remote-exec/slot.lock"
"""T3459: host-wide build slot shared by remote_exec verify and remote_prebuilt builds."""
BUSY_EXIT = 75
SLOT_HEARTBEAT_INTERVAL_S = 15
SLOT_HEARTBEAT_STALE_S = 60
SLOT_RESERVATION_MAX_S = 3600
STAGE_EXIT = 70
FETCH_EXIT = 71
HEALTH_EXIT = host_health.HEALTH_EXIT
"""T3516: the host-health gate refused (host too loaded); not a verdict on the candidate."""
# 139 and -11 are signal 11 (T3697: a flipped bit in a host binary): host fault, local fallback.
INFRA_EXITS = frozenset(
    {STAGE_EXIT, FETCH_EXIT, BUSY_EXIT, HEALTH_EXIT, 124, 127, 137, 139, 255, -11}
)
INFRA_LOG = re.compile(
    r"@@LOCK_(?:TIMEOUT|BUSY)|@@HOST_HEALTH|@@FETCH_FAILED|command not found"
    r"|No space left on device|is `?cmake`? not installed"
    r"|failed to run custom build command for `cmake"
    r"|Unable to find libclang|could not find system library|pkg-config (?:exited|command)"
    r"|toolchain '[^']+' is not installed|error: no such command|Cannot allocate memory"
    r"|signal: 9, SIGKILL|memory allocation of \d+ bytes failed"
)
_TREE = re.compile(r"^[0-9a-f]{40}$")

SshRunner = Callable[..., subprocess.CompletedProcess]
Copier = Callable[..., int | None]


TRANSFER_CAP_S = 240
"""Longest archive copy (rsync + remote hash) before the host is given up on for this run."""
TRANSFER_SLOW_S = 300
"""A copy slower than this marks the host degraded. Above the 240 s copy cap, so a slow but
completed transfer never benches a host (2026-10-21: Deck and droplet at 154-209 s were both
benched while the server was down, leaving no remote host)."""
DEGRADED_MIN = 20.0
RUN_TIMEOUT_DEGRADED_MIN = 2.0
STORE_ENSURE_TIMEOUT_S = (
    120  # T3678: ensure_host_store is bounded (env TSFP_STORE_ENSURE_TIMEOUT_S)
)
STEP_MARK = "@@STEP "
"""Prefix of the progress lines the remote verify script echoes (streamed to the local log)."""
STAGING_BENCH_MIN = 5.0  # T3461: a staging timeout benches a host 5 min, not 20-30
STAGING_MIN_MBPS = 1.5  # slowest link we still wait for (the droplet's WAN staging took 154 s)


def staging_timeout(size_bytes: int) -> int:
    """T3461: the staging copy timeout scales with the archive (680 MB at 1.5 MB/s = 453 s),
    never below TRANSFER_CAP_S; with the rsync delta basis a typical copy is far shorter."""
    return max(TRANSFER_CAP_S, int(size_bytes / (STAGING_MIN_MBPS * 1024 * 1024)) + 60)


DEGRADED_REL = Path("tmp/wt-cache/remote-degraded")
TRANSPORT_ERRORS = (subprocess.SubprocessError, OSError)
"""ssh/rsync failures that say nothing about the candidate (TimeoutExpired included)."""


def _degraded_file(main: Path, name: str) -> Path:
    return main / DEGRADED_REL / f"{name}.json"


TIMEOUT_LOG_REL = Path("tmp/wt-cache/remote-timeouts.jsonl")


def _record_timeout(main: Path, name: str, item: CommandResult) -> None:
    """Append the timed-out remote command (tmp/wt-cache/remote-timeouts.jsonl)."""
    path = main / TIMEOUT_LOG_REL
    entry = {"ts": round(time.time(), 1), "host": name, "seconds": item.seconds}
    entry["command"] = item.command[-500:]
    try:
        path.parent.mkdir(parents=True, exist_ok=True)
        with path.open("a") as handle:
            handle.write(json.dumps(entry) + "\n")
    except OSError:
        pass


def host_at_fault(error: BaseException) -> bool:
    """A lost remote run benches the host, except when the local side closed the pipe: a
    BrokenPipeError means our reader went away (a killed caller, `| head`), and benching the laptop
    20 min for it idled the strongest host (2026-10-30, I-T3885)."""
    return not isinstance(error, BrokenPipeError)


def mark_degraded(main: Path, name: str, reason: str, minutes: float | None = None) -> None:
    """Stop sending verify work to `name` for a while (shared by every session of this host)."""
    if minutes is None:
        minutes = float(os.environ.get("TSFP_REMOTE_DEGRADED_MIN", DEGRADED_MIN))
    path = _degraded_file(main, name)
    try:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(
            json.dumps(
                {
                    "until": time.time() + minutes * 60,
                    "reason": reason[:200],
                    "at": time.time(),
                }
            )
            + "\n"
        )
    except OSError:
        pass


def degraded_reason(main: Path, name: str, now: float | None = None) -> str:
    """The reason while `name` is degraded, else an empty string (expired marks are removed)."""
    path = _degraded_file(main, name)
    try:
        data = json.loads(path.read_text())
        until = float(data["until"])
    except (OSError, ValueError, KeyError, TypeError):
        return ""
    if (time.time() if now is None else now) >= until:
        path.unlink(missing_ok=True)
        return ""
    return str(data.get("reason", "degraded"))


def degraded_at(main: Path, name: str) -> float:
    """When the live mark on `name` was written (mark time, else file mtime; 0 if unreadable)."""
    path = _degraded_file(main, name)
    try:
        return float(json.loads(path.read_text()).get("at") or path.stat().st_mtime)
    except (OSError, ValueError, TypeError, AttributeError):
        return 0.0


def spare_least_recently_failed(main: Path, names: Sequence[str]) -> str:
    """I-T3678: when every candidate in `names` is degraded, the one whose mark is OLDEST stays
    eligible, so staging timeouts can never bench the whole fleet onto the owner PC."""
    if not names or any(not degraded_reason(main, name) for name in names):
        return ""
    return min(names, key=lambda name: degraded_at(main, name))


def _main_of(root: Path) -> Path:
    from tools.wt import core

    try:
        return core.main_root(core.worktree_root(root))
    except (Exception, SystemExit):  # noqa: BLE001 - bookkeeping must not fail a verify
        return root


def _quiet_rm(ssh: SshRunner, worker: remote_replay.Worker, path: str) -> None:
    try:
        ssh(worker, f"rm -f {rpath(path)}", timeout=30)
    except TRANSPORT_ERRORS:
        pass


class RemoteExecError(RuntimeError):
    """A remote run could not be staged, was refused or its transport failed."""


class RemoteExecBusy(RemoteExecError):
    """A remote slot became unavailable before staging began."""


@dataclass(frozen=True)
class HostSpec:
    name: str
    address: str
    port: int
    user: str
    cores: int
    ram_gb: int
    slots: int
    disk_floor_gb: int
    data_cap_gb: int
    workdir: str
    data_paths: tuple[str, ...]
    capabilities: tuple[str, ...]
    owner_files: dict[str, bool]
    replay_repo: str | None = None
    data_root: str = ""
    """Registry `data_root` (owner inputs live under it, `tools.wt.owner_links`)."""
    env: dict[str, str] = field(default_factory=dict)
    max_jobs: int | None = None
    min_mem_mb: int = 0
    target_dir: str = ""
    target_cap_gb: int = 6
    legacy_workdirs: tuple[str, ...] = ()
    canon_root: str = ""
    sccache: str = "none"
    sccache_store_cap_gib: float | None = None
    sccache_port: int | None = None
    slot_targets: bool = False
    health: host_health.HostHealth | None = None
    """T3516: host-health admission guard (registry `host_health`), None = unguarded."""
    host_venv: str = ""
    """cx-execlayout: the host's project venv dir (registry `host_venv`), no git repo needed."""
    toolchain_dir: str = ""
    """cx-execlayout: parent of the host's rustup/cargo homes (registry `toolchain_dir`)."""
    replay_cache_dir: str = ""
    capture_reserved_cores: int = 0
    """T3803: CPUs builds leave to capture replays (registry `capture_reserved_cores`)."""
    """cx-execlayout: remote-replay staging and input-cache root (registry `replay_cache_dir`)."""
    mount_paths: tuple[str, ...] = ()
    """Paths whose filesystems builds write to (`tools.wt.disk_mounts`, registry `disk_mounts`)."""
    root_floor_gb: int = 0
    """Floor for the root filesystem `/` (registry `root_floor_gb`), 0 = unchecked."""
    budget: host_budget.Budget | None = None
    """T3837: the ONE CPU/memory budget verify, build and capture jobs share (registry
    `budget`, `tools.wt.host_budget`), None = ungated."""
    automatic_capabilities: tuple[str, ...] | None = None

    @property
    def cargo_target(self) -> str:
        """The ONE cargo target of the host, under its counted data root (never /workspace/tmp)."""
        return self.target_dir or f"{self.workdir}/cargo-target"

    def exports(self) -> str:
        """Shell prefix exporting the host's toolchain environment (T3459)."""
        if not self.env:
            return ""
        pairs = " ".join(f"{key}={_shell_value(value)}" for key, value in self.env.items())
        return f"export {pairs}; "

    @property
    def target(self) -> str:
        return f"{self.user}@{self.address}"

    def worker(self, repo: str | None = None) -> remote_replay.Worker:
        return remote_replay.Worker(
            self.name, self.target, self.port, repo or self.workdir, self.slots
        )

    def has_owner_inputs(self) -> bool:
        return all(self.owner_files.get(item, False) for item in OWNER_INPUTS)


@dataclass
class Probe:
    host: str
    reachable: bool
    cores: int = 0
    mem_available_mb: int = 0
    disk_free_gb: int = 0
    data_gb: int = 0
    tools: list[str] = field(default_factory=list)
    reasons: list[str] = field(default_factory=list)
    seconds: float = 0.0
    free_slots: int | None = None  # T3461: None when the probe did not report it
    host_mem_mb: int | None = None  # T3516: host-wide MemAvailable (not the cgroup headroom)
    load1: float | None = None  # T3516: host 1-minute load average
    mounts: dict[str, int] = field(default_factory=dict)  # free GB per mount (disk_mounts)

    @property
    def ok(self) -> bool:
        return self.reachable and not self.reasons


@dataclass
class CommandResult:
    index: int
    command: str
    returncode: int
    seconds: float
    peak_rss_kb: int | None
    passed_tests: int | None = None
    """Per-command test evidence, retained even when the diagnostic log is truncated."""


@dataclass
class VerifyResult:
    host: str
    tree: str
    returncode: int
    seconds: float
    transfer_seconds: float
    commands: list[CommandResult]
    log: str
    source_mode: str = ""
    """`rsync`, `rsync-retry` or `tar` (T3760, `exec_sync.SyncResult.mode`)."""
    bytes_sent: int | None = None
    cache_hits: int = 0
    step_seconds: dict[str, float] = field(default_factory=dict)
    diagnostic_path: str | None = None


def _shell_value(value: str) -> str:
    """Quote a value but keep $HOME/$PATH expansion for registry entries such as PATH."""
    return '"' + value.replace("\\", "\\\\").replace('"', '\\"').replace("`", "\\`") + '"'


def slot_paths(slots: int) -> list[str]:
    """Host slot lock files: slot 0 is SLOT_LOCK (also taken by remote_prebuilt), then
    slot-1.lock ... for hosts with more build slots (T3459)."""
    base = SLOT_LOCK.removesuffix(".lock")
    return [SLOT_LOCK] + [f"{base}-{index}.lock" for index in range(1, max(1, slots))]


def _cleanup_stale_reservation_dirs_command(host: HostSpec) -> str:
    """Remove only immediate reservation directories that have aged past the lease cap."""
    reservations = rpath(f"{host.workdir}/reservations")
    return (
        f"if [ -d {reservations} ]; then "
        f"find {reservations} -mindepth 1 -maxdepth 1 -type d -mmin +60 "
        "-exec rm -rf -- {} +; fi"
    )


def slot_lock_command(nonblocking: bool, wait_s: int = 0, slots: int = 1) -> str:
    """Take a free host slot on fd 9 and set $SLOT to its index; exit BUSY_EXIT when none
    is free (nonblocking) or slot 0 stays busy for wait_s seconds."""
    paths = [rpath(path) for path in slot_paths(slots)]
    tries = " ".join(
        f"{{ exec 9>{path} && flock -n 9 && SLOT={index}; }} ||" for index, path in enumerate(paths)
    )
    busy = f'{{ echo "@@LOCK_BUSY"; exit {BUSY_EXIT}; }}'
    # T3678: a blocking wait announces itself and prints a heartbeat every LOCK_HEARTBEAT_S, so
    # the local log shows the verify is queued behind the host's slot, not hung.
    wait = (
        f'echo "{STEP_MARK}lock-wait start (up to {wait_s} s)"; '
        f"( i=0; while :; do sleep 1; i=$((i+1)); "
        f"[ $((i % {LOCK_HEARTBEAT_S})) -eq 0 ] && "
        f'echo "{STEP_MARK}lock-wait still waiting $i s"; '
        "done ) & HB=$!; "
        f"if flock -w {wait_s} 9; then kill $HB 2>/dev/null; wait $HB 2>/dev/null; SLOT=0; "
        "else kill $HB 2>/dev/null; wait $HB 2>/dev/null; false; fi"
    )
    last = busy if nonblocking else f"{{ exec 9>{paths[0]} && {{ {wait}; }}; }} || {busy}"
    return f"mkdir -p $(dirname {paths[0]}) && {{ {tries} {last}; }}"


def _reserve_slot_command(host: HostSpec, reservation: str) -> str:
    """Start a detached process that holds one verify slot across staging and execution."""
    directory = f"{host.workdir}/reservations/{reservation}"
    alive = rpath(directory + "/alive")
    release = rpath(directory + "/release")
    # The slot holder is intentionally detached from SSH so staging can use another SSH
    # connection. Its lease follows the client's heartbeat and still has a hard one-hour cap.
    hold = (
        f"n=0; while [ -d {rpath(directory)} ] && [ ! -e {release} ]; do "
        f"now=$(date +%s); beat=$(stat -c %Y {alive} 2>/dev/null || echo 0); "
        f'[ -n "$beat" ] && [ "$beat" -gt 0 ] '
        f"&& [ $((now - beat)) -le {SLOT_HEARTBEAT_STALE_S} ] || exit 0; "
        f"sleep 1; n=$((n + 1)); [ $n -lt {SLOT_RESERVATION_MAX_S} ] || exit 0; done; exit 0"
    )
    # T3798: a slot whose exec dir is still leased is busy even when its lock is free. A job
    # whose caller stopped (yield, signal) keeps running in the dir after its reservation was
    # released; the next reservation got that slot and its sync waited ~120 s on the lease, then
    # benched the whole host for 5 min (9 lease failures in 12 h, 4 of 5 laptop ones after a
    # stopped job).
    dirs = [rpath(exec_dir(host, index)) for index in range(len(slot_paths(host.slots)))]
    # T3837: a busy answer names the locked and the leased slots, so a no-verdict row says why.
    attempts = " ".join(
        f"exec 9>{rpath(path)}; if flock -n 9; then if ! tsfp_leased {dirs[index]}; then "
        f"printf '%s\\n' {index} >{rpath(directory + '/slot')}; "
        f'{hold}; else D="$D,{index}"; fi; else L="$L,{index}"; fi;'
        for index, path in enumerate(slot_paths(host.slots))
    )
    body = (
        f"{exec_sync.LEASED_SH}; L=; D=; {attempts} exec 9>&-; "
        f'printf \'busy locked=%s leased=%s\\n\' "${{L#,}}" "${{D#,}}" '
        f">{rpath(directory + '/slot')}"
    )
    # Orphaned or overdue jobs in the slot dirs are reaped first (`exec_sync.REAP_SH`, the same
    # step every sync runs), so their slots come back instead of staying skipped.
    reap = f'{{ {exec_sync.REAP_SH} for d in {" ".join(dirs)}; do tsfp_reap "$d"; done; }}'
    return (
        f"mkdir -p {rpath(directory)} $(dirname {rpath(slot_paths(host.slots)[0])}) && "
        f"touch {alive} && {{ {reap} 2>/dev/null || true; }} && {{ "
        f"nohup sh -c {shlex.quote(body)} </dev/null >/dev/null 2>&1 & "
        f"echo $! >{rpath(directory + '/pid')}; }}"
    )


_slot_heartbeats: dict[tuple[str, str], tuple[threading.Event, threading.Thread]] = {}
_slot_heartbeats_lock = threading.Lock()
_slot_event_factory = threading.Event


def _start_slot_heartbeat(host: HostSpec, ssh: SshRunner, reservation: str) -> None:
    """Renew a detached remote slot lease while this dispatcher owns the reservation."""
    key = (host.name, reservation)
    stopped = _slot_event_factory()
    directory = f"{host.workdir}/reservations/{reservation}"
    command = f"touch {rpath(directory + '/alive')} {rpath(directory)}"

    def renew() -> None:
        while not stopped.wait(SLOT_HEARTBEAT_INTERVAL_S):
            try:
                ssh(host.worker(), command, timeout=SLOT_HEARTBEAT_INTERVAL_S)
            except TRANSPORT_ERRORS:
                # The daemon expires the lease if the client cannot renew it.
                continue

    thread = threading.Thread(target=renew, name=f"remote-slot-{host.name}", daemon=True)
    with _slot_heartbeats_lock:
        _slot_heartbeats[key] = (stopped, thread)
    thread.start()


def _reserve_slot(host: HostSpec, ssh: SshRunner, reservation: str) -> int:
    """Acquire remote admission before the caller spends time staging an archive."""
    directory = f"{host.workdir}/reservations/{reservation}"
    worker = host.worker()
    started = ssh(worker, _reserve_slot_command(host, reservation), timeout=30)
    if started.returncode:
        _release_slot(host, ssh, reservation)  # a holder may have started before the error
        raise RemoteExecError(f"cannot start slot reservation on {host.name}")
    deadline = time.monotonic() + 30.0
    while time.monotonic() < deadline:
        result = ssh(
            worker,
            f"test ! -f {rpath(directory + '/slot')} || cat {rpath(directory + '/slot')}",
            timeout=15,
        )
        value = result.stdout.strip()
        if value.startswith("busy"):
            # T3837: the probe counts free slots with the same predicate (lock free AND exec dir
            # not leased), so this is a real race: every slot was taken in the second between.
            # Nothing is staged yet; the caller requeues the host or tries the next one.
            _release_slot(host, ssh, reservation)
            detail = value.removeprefix("busy").strip().replace(" ", ",")
            raise RemoteExecBusy(f"{host.name}:slot_busy_at_reserve({detail})")
        if value.isdigit() and int(value) < host.slots:
            _start_slot_heartbeat(host, ssh, reservation)
            return int(value)
        time.sleep(0.25)
    # A late holder would keep the flock for up to an hour: release (it exits once its
    # directory is gone) before giving up.
    _release_slot(host, ssh, reservation)
    raise RemoteExecError(f"slot reservation on {host.name} did not become ready")


def _release_slot(host: HostSpec, ssh: SshRunner, reservation: str) -> None:
    directory = f"{host.workdir}/reservations/{reservation}"
    key = (host.name, reservation)
    with _slot_heartbeats_lock:
        heartbeat = _slot_heartbeats.pop(key, None)
    if heartbeat is not None:
        stopped, thread = heartbeat
        stopped.set()
        thread.join(timeout=2)
    try:
        ssh(
            host.worker(),
            f"touch {rpath(directory + '/release')}; "
            f"for n in 1 2 3 4 5; do "
            f"kill -0 $(cat {rpath(directory + '/pid')}) 2>/dev/null || break; sleep 1; done; "
            f"rm -rf {rpath(directory)}",
            timeout=15,
        )
    except TRANSPORT_ERRORS:
        pass


def all_slots_command(slots: int, wait_s: int) -> str:
    """Hold every slot of the host (fds 20+), for maintenance such as clean."""
    paths = [rpath(path) for path in slot_paths(slots)]
    return f"mkdir -p $(dirname {paths[0]}) && " + " && ".join(
        f"exec {20 + index}>{path} && flock -w {wait_s} {20 + index}"
        for index, path in enumerate(paths)
    )


def enabled(environ: dict[str, str] | None = None) -> bool:
    return (os.environ if environ is None else environ).get(ENABLE_ENV, "") == "1"


def load_registry(path: Path = REGISTRY) -> list[HostSpec]:
    data = json.loads(path.read_text())
    mount_block = data.get(disk_mounts.KEY, {})
    hosts: list[HostSpec] = []
    seen: set[str] = set()
    for entry in data["hosts"]:
        name = entry["name"]
        if retired_hosts.is_retired(entry):
            continue  # T3753: a retired host is never selected and never validated
        if name in seen:
            raise ValueError(f"duplicate remote host: {name}")
        seen.add(name)
        for key in ("cores", "slots", "disk_floor_gb", "data_cap_gb", "port"):
            if not isinstance(entry[key], int) or entry[key] < 1:
                raise ValueError(f"remote host {name}: {key} must be a positive integer")
        if not str(entry["workdir"]).startswith("/"):
            raise ValueError(f"remote host {name}: workdir must be absolute")
        hosts.append(
            HostSpec(
                name=name,
                address=entry["address"],
                port=entry["port"],
                user=entry["user"],
                cores=entry["cores"],
                ram_gb=entry["ram_gb"],
                slots=entry["slots"],
                disk_floor_gb=entry["disk_floor_gb"],
                data_cap_gb=entry["data_cap_gb"],
                workdir=entry["workdir"],
                data_paths=tuple(entry.get("data_paths", [entry["workdir"]])),
                capabilities=tuple(entry.get("capabilities", [])),
                owner_files={
                    key: bool(entry.get("owner_files", {}).get(key)) for key in OWNER_INPUTS
                },
                replay_repo=entry.get("replay_repo"),
                data_root=str(entry.get("data_root", "")),
                env={str(k): str(v) for k, v in entry.get("env", {}).items()},
                max_jobs=entry.get("max_jobs"),
                min_mem_mb=int(entry.get("min_mem_mb", 0)),
                target_dir=str(entry.get("target_dir", "")),
                target_cap_gb=int(entry.get("target_cap_gb", DEFAULT_TARGET_CAP_GB)),
                legacy_workdirs=tuple(entry.get("legacy_workdirs", [])),
                canon_root=str(entry.get("canon_root", "")),
                sccache=str(entry.get("sccache", "none")),
                sccache_store_cap_gib=(
                    float(entry["sccache_store_cap_gib"])
                    if entry.get("sccache_store_cap_gib") is not None
                    else None
                ),
                sccache_port=(
                    int(entry["sccache_port"]) if entry.get("sccache_port") is not None else None
                ),
                slot_targets=bool(entry.get("slot_targets", False)),
                health=host_health.from_entry(entry),
                budget=host_budget.from_entry(entry),
                capture_reserved_cores=max(0, int(entry.get(host_health.CAPTURE_RESERVED_KEY, 0))),
                mount_paths=tuple(disk_mounts.watched_paths(entry, mount_block)),
                root_floor_gb=disk_mounts.root_floor_gb(entry, mount_block),
                host_venv=str(entry.get("host_venv", "")),
                toolchain_dir=str(entry.get("toolchain_dir", "")),
                replay_cache_dir=str(entry.get("replay_cache_dir", "")),
                automatic_capabilities=(
                    tuple(entry["automatic_capabilities"])
                    if "automatic_capabilities" in entry
                    else None
                ),
            )
        )
    return hosts


def find_host(name: str, hosts: Sequence[HostSpec] | None = None) -> HostSpec:
    for host in hosts if hosts is not None else load_registry():
        if host.name == name:
            return host
    raise RemoteExecError(f"unknown remote host {name!r}")


def rpath(path: str) -> str:
    """Shell-quote a remote path, keeping a leading ~/ expandable."""
    if path.startswith("~/"):
        return '"$HOME"/' + shlex.quote(path[2:])
    return shlex.quote(path)


DATA_GB_CACHE_MIN = 10
"""Minutes a host's measured data size is reused by probes (I-T3505-20)."""


def probe_slots_command(host: HostSpec) -> str:
    """Reclaim expired/orphaned job leases before counting usable slots (T3878).

    Hold each slot lock while checking its dir. Live reservations and live job leases
    remain busy; only the existing identity/deadline-aware reaper can release a lease.
    """
    return (
        f"mkdir -p $(dirname {rpath(SLOT_LOCK)}); {exec_sync.LEASED_SH}; "
        f"{exec_sync.REAP_SH} n=0; "
        + " ".join(
            f"( exec 9>{rpath(path)} && flock -n 9 && {{ "
            f"if tsfp_leased {rpath(exec_dir(host, index))}; then "
            f"tsfp_reap {rpath(exec_dir(host, index))} >&2; fi; "
            f"! tsfp_leased {rpath(exec_dir(host, index))}; }} ) 2>/dev/null && n=$((n+1));"
            for index, path in enumerate(slot_paths(host.slots))
        )
        + ' echo "slot_free=$n"; true'
    )


def probe_command(host: HostSpec) -> str:
    data = " ".join(rpath(path) for path in host.data_paths)
    free_gib = disk_mounts.free_gib_command('"$p"', suffix=False)
    return (
        f"{host.exports()}"
        f'p={rpath(host.workdir)}; while [ ! -e "$p" ]; do p=$(dirname "$p"); done; '
        'echo "cores=$(nproc)"; '
        # T3472 p3: a container cgroup limit caps what is really free (the server: memory.max
        # 10 GiB under a 46 GiB /proc/meminfo); the host's memguard pauses builds on this number.
        "m=$(awk '/^MemAvailable:/ {print int($2/1024)}' /proc/meminfo); "
        "l=$(cat /sys/fs/cgroup/memory.max 2>/dev/null); c=$(cat /sys/fs/cgroup/memory.current "
        '2>/dev/null); case "$l" in ""|max) ;; *) [ -n "$c" ] && g=$(( (l - c) / 1048576 )) '
        '&& [ "$g" -lt "$m" ] && m=$g;; esac; echo "mem_mb=$m"; '
        f'echo "free_gb=$({free_gib})"; '
        f"{disk_mounts.probe_fragment(host.mount_paths) if host.mount_paths else ''}"
        # I-T3505-20: a full `du` of the data tree on every probe (up to 700 GB on the server) ran
        # many times at once and drove the server to load 45, so host health refused it. Cache the
        # size for 10 min; one refresh at a time (flock), at nice 19 and idle I/O, else reuse it.
        # I-T3885-laptopret: one `du` of the whole tree never finished in 60 s, so a stale value
        # stuck (475 shown, 289 real). Size each root on its own: one cache file per root, a
        # detached refresh (nice 19, idle I/O, 30 min cap, flock per root), data_gb their sum.
        f"d=$HOME/.cache/tsfp-data-gb; mkdir -p $HOME/.cache; tot=0; "
        f"for r in {data}; do "
        'k="$d.$(printf %s "$r" | tr -c A-Za-z0-9 _)"; '
        f'if [ -z "$(find "$k" -mmin -{DATA_GB_CACHE_MIN} 2>/dev/null)" ]; then '
        '( exec 8>"$k.lock"; flock -n 8 && nice -n 19 ionice -c3 timeout 1800 du -sBG "$r" '
        '2>/dev/null | tail -n 1 | cut -f1 | tr -dc 0-9 > "$k.tmp" '
        '&& [ -s "$k.tmp" ] && mv -f "$k.tmp" "$k" ) >/dev/null 2>&1 & fi; '
        'v=$(cat "$k" 2>/dev/null | tr -dc 0-9); tot=$((tot + ${v:-0})); done; '
        'echo "data_gb=$tot"; '
        f"{host_health.probe_fragment()}"
        "for t in cargo python3 rsync flock git tar /usr/bin/time; do "
        'command -v "$t" >/dev/null 2>&1 && echo "tool=$t"; done; ' + probe_slots_command(host)
        # T3461: free verify slots now, so a busy host is skipped before the tree is staged.
        # T3837: free = lock free AND exec dir not leased, the reservation's own predicate. The
        # probe counted a slot whose stopped job still ran in its dir as free, the reservation
        # refused it, and callers retried up to 20 times per queue window
        # (slot_taken_before_staging, 2026-10-27).
    )


def admission(
    host: HostSpec, probe: Probe, *, need: Sequence[str] = ("rsync", "flock", "tar")
) -> list[str]:
    reasons: list[str] = []
    if probe.disk_free_gb < host.disk_floor_gb:
        reasons.append(f"disk_free_{probe.disk_free_gb}GB_below_floor_{host.disk_floor_gb}GB")
    reasons.extend(  # a full root `/` refuses new builds although the data volume is roomy
        f"disk_mount_{v.mount}_{v.free_gb}GB_below_floor_{v.floor_gb}GB"
        for v in disk_mounts.violations(
            probe.mounts, floor_gb=host.disk_floor_gb, root_floor=host.root_floor_gb
        )
    )
    if probe.data_gb > host.data_cap_gb:
        reasons.append(f"tsfp_data_{probe.data_gb}GB_above_cap_{host.data_cap_gb}GB")
    if host.min_mem_mb and probe.mem_available_mb < host.min_mem_mb:
        reasons.append(f"mem_{probe.mem_available_mb}MB_below_{host.min_mem_mb}MB")
    reasons.extend(f"missing_tool_{tool}" for tool in need if tool not in probe.tools)
    reasons.extend(host_health.prefixed(health_reasons(host, probe)))
    return reasons


def health_reasons(host: HostSpec, probe: Probe) -> list[str]:
    """T3516 probe-time host-health refusals (empty = admit); the run-time gate repeats them.

    Our builds counted are the held slot locks plus the one being admitted."""
    if host.health is None:
        return []
    held = host.slots - probe.free_slots if probe.free_slots is not None else None
    return host_health.evaluate(
        host.health,
        mem_mb=probe.host_mem_mb,
        load1=probe.load1,
        cores=host.cores,
        builds=None if held is None else held + 1,
    )


def probe(host: HostSpec, *, ssh: SshRunner | None = None, root: Path | None = None) -> Probe:
    """One SSH round trip: CPU, memory, disk free, tsfp data size and tools.

    T3588: a host with a `host_health` guard that refused its last probe is not probed again
    for the shared back-off (host_health.backoff_remaining): the result is an unreachable
    Probe with a `host_health_backoff_<n>s` reason and no ssh. `root` locates the shared
    back-off state (default: this checkout's main)."""
    ssh = ssh or remote_replay._ssh
    if host.health is not None:
        left = host_health.backoff_remaining(host.name, root=root)
        if left > 0:
            return Probe(host.name, False, reasons=[host_health.backoff_reason(left)])
    started = time.monotonic()
    try:
        result = ssh(host.worker(), probe_command(host), timeout=PROBE_TIMEOUT_S)
    except (OSError, subprocess.TimeoutExpired) as error:
        return Probe(host.name, False, reasons=[f"ssh_error:{type(error).__name__}"])
    seconds = round(time.monotonic() - started, 2)
    if result.returncode:
        return Probe(host.name, False, reasons=["unreachable"], seconds=seconds)
    values: dict[str, str] = {}
    tools: list[str] = []
    mounts_text = ""
    for line in result.stdout.splitlines():
        key, _, value = line.partition("=")
        if key == disk_mounts.KEY:
            mounts_text = value.strip()
            continue
        if key == "tool":
            tools.append(Path(value).name if value.startswith("/") else value)
        elif key:
            values[key] = value.strip()
    try:
        found = Probe(
            host.name,
            True,
            cores=int(values["cores"]),
            mem_available_mb=int(values["mem_mb"]),
            disk_free_gb=int(values["free_gb"] or 0),
            data_gb=int(values.get("data_gb") or 0),
            tools=tools,
            seconds=seconds,
            free_slots=int(values["slot_free"]) if values.get("slot_free") else None,
            host_mem_mb=int(values["host_mem_mb"]) if values.get("host_mem_mb") else None,
            load1=float(values["load1"]) if values.get("load1") else None,
            mounts=disk_mounts.parse(mounts_text),
        )
    except (KeyError, ValueError):
        return Probe(host.name, False, reasons=["unparsable_probe"], seconds=seconds)
    found.reasons = admission(host, found)
    if host.health is not None:  # T3588: the answer sets or clears the shared back-off
        refused = health_reasons(host, found)
        if refused:
            host_health.note_refusal(host.name, root=root, reasons=refused)
        else:
            host_health.note_admission(host.name, root=root)
    return found


def tree_of(root: Path, commit: str) -> str:
    result = subprocess.run(
        ["git", "-C", str(root), "rev-parse", f"{commit}^{{tree}}"],
        capture_output=True,
        text=True,
        timeout=30,
        check=False,
    )
    tree = result.stdout.strip()
    if result.returncode or not _TREE.fullmatch(tree):
        raise RemoteExecError(f"cannot resolve tree of {commit!r}: {result.stderr.strip()[-200:]}")
    return tree


def worktree_tree(root: Path) -> str:
    """T3461: a tree object of the working state (HEAD, tracked edits and deletions, and
    untracked files git does not ignore), written through a private index, so a dirty
    worktree goes remote as an exact, hash-checked tree like a commit does. 13 of 44 remote
    attempts were refused as dirty_tree (worker_check and tools.wt build run in task worktrees
    with uncommitted changes)."""
    with tempfile.TemporaryDirectory(prefix="tsfp-worktree-tree-") as temp:
        env = {**os.environ, "GIT_INDEX_FILE": str(Path(temp) / "index")}
        for args in (["read-tree", "HEAD"], ["add", "-A", "--", "."]):
            result = subprocess.run(
                ["git", "-C", str(root), *args],
                env=env,
                capture_output=True,
                text=True,
                timeout=300,
                check=False,
            )
            if result.returncode:
                raise RemoteExecError(f"git {args[0]} failed: {result.stderr.strip()[-200:]}")
        written = subprocess.run(
            ["git", "-C", str(root), "write-tree"],
            env=env,
            capture_output=True,
            text=True,
            timeout=120,
            check=False,
        )
    tree = written.stdout.strip()
    if written.returncode or not _TREE.fullmatch(tree):
        raise RemoteExecError(f"cannot write the working tree: {written.stderr.strip()[-200:]}")
    return tree


def archive_tree(root: Path, tree: str, destination: Path) -> str:
    with destination.open("wb") as output:
        result = subprocess.run(
            ["git", "-C", str(root), "archive", "--format=tar", tree],
            stdout=output,
            stderr=subprocess.PIPE,
            timeout=300,
            check=False,
        )
    if result.returncode:
        raise RemoteExecError(
            f"git archive {tree} failed: {result.stderr.decode(errors='replace')[-300:]}"
        )
    return remote_replay.hash_file(destination)


def target_guard(host: HostSpec) -> str:
    """Shell: migrate legacy targets away and enforce the per-host target size cap (T3470).

    Everything is skipped while a process has the directory in its environment or open files."""
    legacy_rm = " ".join(rpath(base) + "/target*" for base in (*host.legacy_workdirs, host.workdir))
    target = rpath(host.cargo_target)
    cap_kb = host.target_cap_gb * 1024 * 1024
    if host.slot_targets:
        # T3472 phase 3: one target per source dir (`<target>-src`, `<target>-src-<slot>`), each
        # under the cap; the old single target is migrated away like the other legacy ones.
        legacy_rm += f" {target}"
        return (
            'tsfp_busy() { { grep -alF "CARGO_TARGET_DIR=$1" /proc/[0-9]*/environ 2>/dev/null '
            '| grep -v "^/proc/$$/"; ls -l /proc/[0-9]*/fd 2>/dev/null | grep -F "$1"; } '
            "| grep -q .; }; "
            f'for d in {legacy_rm}; do [ -e "$d" ] && ! tsfp_busy "$d" && rm -rf "$d"; done; '
            f'for t in {target}-src*; do [ -d "$t" ] '
            f'&& [ "$(du -sk "$t" | cut -f1)" -gt {cap_kb} ] '
            '&& ! tsfp_busy "$t" && rm -rf "$t" && echo "@@TARGET_CAP_CLEARED"; done; true'
        )
    return (
        'tsfp_busy() { { grep -alF "CARGO_TARGET_DIR=$1" /proc/[0-9]*/environ 2>/dev/null '
        '| grep -v "^/proc/$$/"; ls -l /proc/[0-9]*/fd 2>/dev/null | grep -F "$1"; } '
        "| grep -q .; }; "
        f'for d in {legacy_rm}; do [ -e "$d" ] && ! tsfp_busy "$d" && rm -rf "$d"; done; '
        f't={target}; if [ -d "$t" ] && [ "$(du -sk "$t" | cut -f1)" -gt {cap_kb} ] '
        f'&& ! tsfp_busy "$t"; then rm -rf "$t"; echo "@@TARGET_CAP_CLEARED"; fi; true'
    )


def canon_prelude(host: HostSpec, wrapper_text: str) -> list[str]:
    """T3472 phase 2: shell lines giving every remote run the SAME absolute cargo home and
    wrapper path on every host, plus the shared sccache environment when the store answers.

    sccache keys a compile by the absolute source dir and CARGO_HOME (MEASURED: the same tree
    at another path = 0 % hits), so source and cargo home live at fixed paths under
    `canon_root`, equal across hosts: `<root>/src-<slot>` and `<root>/cargo`."""
    root = host.canon_root
    store = sccache_hosts.alive_command()
    lines = [
        f"mkdir -p {rpath(root)}/bin {rpath(root)}/cargo",
        # Seed the fixed cargo home once from the host's default one (offline commands work).
        f"[ -d {rpath(root)}/cargo/registry ] || {{ for d in registry git; do "
        f'[ -d "$HOME/.cargo/$d" ] && cp -a "$HOME/.cargo/$d" {rpath(root)}/cargo/; done; true; }}',
        f"cat > {rpath(root)}/bin/sccache-rustc.sh <<'TSFP_WRAPPER_EOF'\n{wrapper_text}"
        f"{'' if wrapper_text.endswith(chr(10)) else chr(10)}TSFP_WRAPPER_EOF",
        f"chmod +x {rpath(root)}/bin/sccache-rustc.sh",
        # TSFP_CARGO_INCREMENTAL=0: a nested `tools.wt build` on the host must not turn
        # incremental back on (209 GB of debug/incremental on the droplet, 2026-10-22).
        f"export CARGO_HOME={rpath(root)}/cargo CARGO_INCREMENTAL=0 TSFP_CARGO_INCREMENTAL=0 "
        "TSFP_CLOSE_LOCK_FDS=1",
        # T3863: loss of the shared store must not remove both compiler wrappers.
        # The Deck installs the WebDAV-capable binary outside PATH.
        f'if [ "${{TSFP_NO_CCACHE:-0}}" != 1 ]; then '
        f'if [ -x "{SCCACHE_BIN}" ]; then TSFP_SCCACHE="{SCCACHE_BIN}"; '
        "else TSFP_SCCACHE=$(command -v sccache || true); fi; "
        'if [ -n "$TSFP_SCCACHE" ]; then '
        f"export TSFP_SCCACHE RUSTC_WRAPPER={rpath(root)}/bin/sccache-rustc.sh "
        f"CMAKE_C_COMPILER_LAUNCHER={rpath(root)}/bin/sccache-rustc.sh "
        f"SCCACHE_DIR={rpath(root)}/sccache-local; fi; fi",
    ]
    if host.sccache != "none":
        lines.append(
            f'if [ "${{TSFP_NO_CCACHE:-0}}" != 1 ] && [ -x "{SCCACHE_BIN}" ] '
            f'&& [ -s "{SCCACHE_PASS}" ] && {store}; then '
            f'export RUSTC_WRAPPER={rpath(root)}/bin/sccache-rustc.sh TSFP_SCCACHE="{SCCACHE_BIN}" '
            f"SCCACHE_WEBDAV_ENDPOINT=http://127.0.0.1:{sccache_shared.store_port()}/ "
            f"SCCACHE_WEBDAV_USERNAME={sccache_shared.USER} "
            f"SCCACHE_WEBDAV_KEY_PREFIX={sccache_shared.KEY_PREFIX} "
            f"SCCACHE_SERVER_UDS={rpath(root)}/sccache.sock SCCACHE_IDLE_TIMEOUT=7200 "
            f'TSFP_SCCACHE_PASS_FILE="{SCCACHE_PASS}"; '
            f'"$TSFP_SCCACHE" --zero-stats >/dev/null 2>&1; echo "@@SCCACHE shared"; '
            'else echo "@@SCCACHE local"; fi'
        )
    return lines


def sccache_stats_line(host: HostSpec) -> str:
    """Shell: print Rust and C/C++ hit/miss counters from the shared daemon."""
    awk = (
        "/^Cache hits \\(Rust\\) +[0-9]+$/ {rh=$NF} "
        "/^Cache misses \\(Rust\\) +[0-9]+$/ {rm=$NF} "
        "/^Cache hits \\(C\\/C\\+\\+\\) +[0-9]+$/ {ch=$NF} "
        "/^Cache misses \\(C\\/C\\+\\+\\) +[0-9]+$/ {cm=$NF} "
        'END {print "@@STATS " rh+0 " " rm+0 " " ch+0 " " cm+0}'
    )
    return (
        'if [ -n "${TSFP_SCCACHE:-}" ]; then "$TSFP_SCCACHE" --show-stats 2>/dev/null | '
        f"awk {shlex.quote(awk)}; fi; true"
    )


def exec_root(host: HostSpec) -> str:
    """T3760: the host's parent of exec dirs and the exec venv (`canon_root`, equal on every
    host, else `<workdir>/exec`). Never inside a git repository."""
    return host.canon_root or f"{host.workdir}/exec"


def replay_exec_dir(host: HostSpec, slot: int) -> str:
    """T3760: the stable exec dir of replay slot `slot`, synced by `exec_sync` per job."""
    return f"{exec_root(host)}/replay-{slot}"


def exec_venv(host: HostSpec) -> str:
    """T3760: the project venv beside the exec dirs (fleet provisioning `python-deps`)."""
    return f"{exec_root(host)}/venv"


def project_python_line(host: HostSpec) -> str:
    """Put the host's project venv first on PATH, so `python3 -m pytest` verify forms run with
    pytest and the project's packages instead of the bare system interpreter (I-T3505-16: four
    landings lost at the final remote verify with "No module named pytest"). T3760: the exec
    venv comes first; the registry `host_venv` (cx-execlayout) is the fallback."""
    venvs = [f"{exec_venv(host)}/bin"]
    venvs += [f"{host.host_venv.rstrip('/')}/bin"] if host.host_venv else []
    candidates = " ".join(_shell_value(venv.replace("~/", "$HOME/", 1)) for venv in venvs)
    return (
        f"for v in {candidates}; do "
        '[ -x "$v/python3" ] && "$v/python3" -c "import pytest" 2>/dev/null '
        '&& export PATH="$v:$PATH" && break; done'
    )


def exec_dir(host: HostSpec, slot: int) -> str:
    """T3760: the stable exec dir of `slot` on `host`, synced by `exec_sync` before every job.

    With `canon_root` (equal on every host, so the shared sccache hits across hosts) slot 0 uses
    `<canon_root>/src` and slot k `<canon_root>/src-<k>`, the dirs the T3472 unpack used, so the
    warm caches carry over; without it `<workdir>/exec/src-<slot>`."""
    if host.canon_root:
        return f"{host.canon_root}/src" + (f"-{slot}" if slot else "")
    return f"{host.workdir}/exec/src-{slot}"


def passed_count_line(index: int, log: str) -> str:
    """Shell: print `@@PASSED <index> <n>`, the tests passed over command `index`'s WHOLE log.

    T3798: the section printed for a command is its last `tail_lines` lines; a crate with many
    test binaries pushed its one passing `test result:` line out of it, and the job read as
    zero_tests_remote (no verdict, verified again on the owner PC)."""
    return (
        f"echo \"@@PASSED {index} $(grep -aE '^test result: ok\\. [0-9]+ passed' {log} "
        "| awk '{s += $4} END {print s + 0}')\""
    )


def verify_script(
    host: HostSpec,
    *,
    run_id: str,
    source_dir: str,
    commands: Sequence[str],
    command_timeout_s: int,
    jobs: int,
    lock_wait_s: int = LOCK_WAIT_S,
    nonblocking: bool = False,
    fetch: Sequence[str] = (),
    tail_lines: int = 60,
    wrapper_text: str = "",
    reserved_slot: int | None = None,
    tree: str = "",
    commit: str = "",
    deadline_s: int = 0,
) -> str:
    """Remote shell: take the host slot, enter the synced exec dir, run each command under timeout.

    The slot (SLOT_LOCK) is shared with remote_prebuilt builds (T3459). `nonblocking` makes a
    busy host exit BUSY_EXIT at once; `fetch` lists manifests whose crates are fetched online
    first (`--offline` commands then work on a fresh host), a failure exits FETCH_EXIT.

    T3760: `source_dir` already holds the exact tree (`exec_sync.sync`); the script checks its
    marker names `tree`, holds the dir's lease for the whole job (host cleaners skip it) and
    never deletes the tree, so the next sync is a delta and cargo/sccache stay warm. With
    `host.canon_root` CARGO_HOME is `<canon_root>/cargo` (shared sccache hits across hosts)."""
    del run_id  # kept for callers; the exec dir is per slot now
    base = host.workdir
    canon = bool(host.canon_root)
    if host.health and host.health.max_cargo_jobs:  # T3516: ceiling of CARGO_BUILD_JOBS
        jobs = min(jobs, host.health.max_cargo_jobs)
    run_dir = '"$SRC"'
    marker = f'"$SRC"/{exec_sync.MARKER}'
    needle = f'"tree": "{tree}"'
    check = f"grep -qF {shlex.quote(needle)} {marker}" if tree else f"test -f {marker}"
    lines = [
        host.exports().rstrip("; ") or "true",
        # Only the per-slot log dir goes at exit; the synced tree stays for the next delta.
        f"trap 'rm -rf \"${{LOGS:-/nonexistent-tsfp}}\"; {target_guard(host)}' EXIT",
        f"SLOT={reserved_slot}"
        if reserved_slot is not None
        else slot_lock_command(nonblocking, lock_wait_s, host.slots),
        f'echo "{STEP_MARK}lock-acquired slot=$SLOT"',
        # T3516: re-check the host's health now that our slot is held (the probe is 40-150 s
        # old after staging); a loaded host exits HEALTH_EXIT before anything is unpacked.
        host_health.gate_command(
            host.health,
            cores=host.cores,
            slot_files=[rpath(path) for path in slot_paths(host.slots)],
        ),
        f'LOGS={rpath(base + "/runs")}/slot-"$SLOT"; rm -rf "$LOGS"; mkdir -p "$LOGS"',
        f"SRC={rpath(source_dir)}",
        f'{check} || {{ echo "@@SOURCE_MISSING $SRC"; exit {STAGE_EXIT}; }}',
        # fd 7: the sccache wrapper closes 7-9, so its daemon never inherits the lease. The
        # lease ends with the ssh session or at the caller's limit (`exec_sync.REAP_SH`).
        exec_sync.lease_hold('"$SRC"', fd=7, deadline_s=deadline_s, attached=True),
        *owner_links.link_lines(host.data_root),
        f'echo "{STEP_MARK}source-ready"',
    ]
    # T3472 phase 3: with `slot_targets` each exec dir gets its own cargo target. One per-host
    # target made every job on the host wait on cargo's directory lock behind whichever build
    # came first (server: 3 jobs idle for 17 min). sccache keys on source paths, not the
    # target dir, so hits are unchanged.
    if canon and host.slot_targets:
        target = f'T={rpath(host.cargo_target)}-"${{SRC##*/}}"; mkdir -p "$T"; '
        exported = 'export CARGO_TARGET_DIR="$T" '
    else:
        target = f"mkdir -p {rpath(host.cargo_target)}; "
        exported = f"export CARGO_TARGET_DIR={rpath(host.cargo_target)} "
    lines += [
        f"cd {run_dir}",
        target + exported + f"CARGO_BUILD_JOBS={jobs} "
        f"TSFP_REMOTE_EXEC_HOST={shlex.quote(host.name)} CARGO_TERM_COLOR=never "
        # T3501: c-diff lifted bundles shipped by run_verify (cdiff_bundle.ship).
        f"{cdiff_bundle.ENV_DIR}={rpath(cdiff_bundle.remote_base(base))}",
        # T3760: the job's source identity as data; there is no git repo on the host.
        exec_sync.identity_exports(commit, tree) if (commit or tree) else "true",
    ]
    lines.append("export CARGO_INCREMENTAL=0 TSFP_CARGO_INCREMENTAL=0")
    if canon:
        lines += canon_prelude(host, wrapper_text)
    for manifest in fetch:
        lines.append(
            f"timeout --kill-after=10s 900s cargo fetch --manifest-path {shlex.quote(manifest)} "
            f'> "$LOGS"/fetch.log 2>&1 || {{ echo "@@FETCH_FAILED {manifest}"; '
            f'tail -n 20 "$LOGS"/fetch.log; '
            f"exit {FETCH_EXIT}; }}"
        )
    lines.append(project_python_line(host))
    lines.append(host_health.build_pin_assign(host.capture_reserved_cores))
    lines.append("rc=0")
    cap = host.health.max_cargo_jobs if host.health else 0
    commands = [host_health.cap_jobs(command, cap) for command in commands]
    stats = f"{sccache_stats_line(host)}; " if canon and host.sccache != "none" else ""
    for index, command in enumerate(commands):
        log = f'"$LOGS"/cmd{index}.log'
        rss = f'"$LOGS"/cmd{index}.rss'
        lines.append(f'echo "{STEP_MARK}command {index + 1}/{len(commands)} started"')
        lines.append(
            f"s=$(date +%s.%N); /usr/bin/time -f '%M' -o {rss} "
            f"timeout --kill-after=10s {command_timeout_s}s ${host_health.PIN_VAR} "
            f"bash -c {shlex.quote(command)} "
            f"> {log} 2>&1; c=$?; e=$(date +%s.%N); "
            f'echo "@@BEGIN {index}"; tail -n {tail_lines} {log}; '
            f"{passed_count_line(index, log)}; "
            f'echo "@@RC {index} $c $(awk "BEGIN {{print $e - $s}}") '
            f'$(tail -n 1 {rss} 2>/dev/null | tr -dc 0-9)"; '
            f'if [ "$c" -ne 0 ]; then {stats}rc=$c; exit $rc; fi'
        )
    if stats:
        lines.append(sccache_stats_line(host))
    lines.append("exit $rc")
    return "\n".join(lines)


def parse_results(stdout: str, commands: Sequence[str]) -> list[CommandResult]:
    results = []
    # Parse BEFORE VerifyResult.log is capped. Long command chains used to lose early
    # @@PASSED sections and either repeat passed tests or borrow a later step's count.
    from tools.wt.remote_fallback import command_tests

    for match in re.finditer(r"^@@RC (\d+) (-?\d+) (\S*) ?(\d*)$", stdout, re.MULTILINE):
        index = int(match.group(1))
        try:
            seconds = round(float(match.group(3)), 2)
        except ValueError:
            seconds = 0.0
        results.append(
            CommandResult(
                index,
                commands[index] if index < len(commands) else "?",
                int(match.group(2)),
                seconds,
                int(match.group(4)) if match.group(4) else None,
                command_tests({}, stdout, index),
            )
        )
    return results


def _bounded(call: Callable[[], object], seconds: float) -> bool:
    """Run `call` on a daemon thread for at most `seconds`; False when it is still running."""
    outcome: list[object] = []

    def target() -> None:
        with contextlib.suppress(Exception):  # the cache is optional, never fail a verify
            outcome.append(call())

    thread = threading.Thread(target=target, daemon=True, name="remote-exec-bounded")
    thread.start()
    thread.join(seconds)
    return not thread.is_alive()


def stream_ssh(
    worker: remote_replay.Worker,
    remote_command: str,
    timeout: float,
    on_line: Callable[[str], None],
    *,
    popen: Callable[..., subprocess.Popen] = subprocess.Popen,
) -> subprocess.CompletedProcess:
    """ssh that hands `@@STEP` lines to `on_line` while the command runs; output still captured.

    The ssh process runs in its own session, so a timeout kills it and any child that holds the
    pipes (raises TimeoutExpired like `subprocess.run`)."""
    process = popen(
        remote_replay.ssh_argv(worker, remote_command),
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        start_new_session=True,
    )
    out: list[str] = []
    err: list[str] = []

    def pump_out() -> None:
        for line in process.stdout:
            out.append(line)
            if line.startswith(STEP_MARK):
                on_line(line.rstrip("\n"))

    threads = [
        threading.Thread(target=pump_out, daemon=True),
        threading.Thread(target=lambda: err.extend(process.stderr), daemon=True),
    ]
    for thread in threads:
        thread.start()
    try:
        code = process.wait(timeout=timeout)
    except subprocess.TimeoutExpired:
        with contextlib.suppress(OSError):
            os.killpg(process.pid, 9)
        process.kill()
        process.wait()
        for thread in threads:
            thread.join(2)
        raise
    for thread in threads:
        thread.join(10)
    return subprocess.CompletedProcess(process.args, code, "".join(out), "".join(err))


Syncer = Callable[[HostSpec, Path, str, str, str], exec_sync.SyncResult]
"""`sync(host, root, tree, commit, dest)`: places the tree at `dest` on the host (T3760)."""


def default_sync(
    host: HostSpec, root: Path, tree: str, commit: str, dest: str
) -> exec_sync.SyncResult:
    worker = host.worker()
    endpoint = exec_sync.Endpoint(
        host.name, host.target, host.port, dest, remote_replay._ssh_transport(worker)
    )
    result = exec_sync.sync(endpoint, root, tree, commit=commit)
    cargo_registry_sync.topup(
        host.name, host.canon_root, host.target, remote_replay._ssh_transport(worker)
    )
    return result


def _commit_id(root: Path, rev: str) -> str:
    """The commit `rev` names, or "" for a bare tree (a dirty worktree's state)."""
    done = subprocess.run(
        ["git", "-C", str(root), "rev-parse", "--verify", "--quiet", f"{rev}^{{commit}}"],
        capture_output=True,
        text=True,
        timeout=30,
        check=False,
    )
    return done.stdout.strip() if done.returncode == 0 else ""


def _reserve_waiting(
    host: HostSpec,
    ssh: SshRunner,
    reservation: str,
    wait_s: float,
    *,
    sleep: Callable[[float], None] = time.sleep,
    clock: Callable[[], float] = time.monotonic,
) -> int:
    """`_reserve_slot`, retried every RESERVE_POLL_S while every slot is busy, for `wait_s`."""
    deadline = clock() + wait_s
    while True:
        try:
            return _reserve_slot(host, ssh, reservation)
        except RemoteExecBusy:
            if clock() + RESERVE_POLL_S > deadline:
                raise
            sleep(RESERVE_POLL_S)


RESERVE_POLL_S = 15.0


def verify_jobs(host: HostSpec, jobs: int | None = None) -> int:
    """CARGO_BUILD_JOBS of a verify on `host` (half its cores, capped by `max_jobs`)."""
    return min(jobs or max(1, host.cores // 2), host.max_jobs or 1 << 16)


def verify_cost(host: HostSpec, jobs: int | None = None) -> host_budget.Cost:
    """T3837: the budget share of one verify on `host` (zero for an unbudgeted host)."""
    if getattr(host, "budget", None) is None:  # stand-in hosts of callers may lack the field
        return host_budget.Cost(0.0, 0)
    return host_budget.job_cost("verify", cargo_jobs=verify_jobs(host, jobs))


@contextlib.contextmanager
def _budget_lease(
    host: HostSpec,
    kind: str,
    cost: host_budget.Cost,
    reservation: host_budget.Reservation | None = None,
) -> Iterator[None]:
    """`host_budget.lease`, a full budget raised as RemoteExecBusy (requeue, never degrade)."""
    if reservation is not None:
        if not reservation.covers(host.name, kind, cost):
            raise RemoteExecBusy(f"{host.name}:budget_reservation_invalid")
        yield
        return
    with contextlib.ExitStack() as stack:
        try:
            budget = getattr(host, "budget", None)
            stack.enter_context(host_budget.lease(host.name, kind, cost, budget))
        except host_budget.BudgetFull as error:
            raise RemoteExecBusy(str(error)) from error
        yield


def budget_admits_verify(host: HostSpec) -> bool:
    """T3837 probe-time check: would a verify fit the host's budget now (no lease taken)."""
    return host_budget.admits(host.name, verify_cost(host), getattr(host, "budget", None))


def run_verify(
    host: HostSpec,
    root: Path,
    commit: str,
    commands: Sequence[str],
    *,
    command_timeout_s: int = 1800,
    jobs: int | None = None,
    ssh: SshRunner | None = None,
    copy: Copier | None = None,
    probe_result: Probe | None = None,
    nonblocking: bool = False,
    fetch: Sequence[str] = (),
    tail_lines: int = 60,
    progress: verify_progress.Progress | None = None,
    sync: Syncer | None = None,
    budget_reservation: host_budget.Reservation | None = None,
) -> VerifyResult:
    """Sync `commit`'s tree to `host`'s slot exec dir, run `commands` there, return exit code
    and logs (T3760: rsync delta, no host git).

    Every step prints `start` and `done <s>` lines on stderr and is recorded in a per-process
    progress file (T3678, tools.wt.verify_progress)."""
    progress = progress or verify_progress.Progress(host.name)
    try:
        # T3837: the host's one budget (verify, build and capture share it) before any slot.
        reserved = {"reservation": budget_reservation} if budget_reservation is not None else {}
        with _budget_lease(host, "verify", verify_cost(host, jobs), **reserved):
            return _run_verify(
                host,
                root,
                commit,
                commands,
                progress,
                command_timeout_s=command_timeout_s,
                jobs=jobs,
                ssh=ssh,
                copy=copy,
                probe_result=probe_result,
                nonblocking=nonblocking,
                fetch=fetch,
                tail_lines=tail_lines,
                sync=sync,
            )
    finally:
        progress.close()


def _run_verify(
    host: HostSpec,
    root: Path,
    commit: str,
    commands: Sequence[str],
    progress: verify_progress.Progress,
    *,
    command_timeout_s: int,
    jobs: int | None,
    ssh: SshRunner | None,
    copy: Copier | None,
    probe_result: Probe | None,
    nonblocking: bool,
    fetch: Sequence[str],
    tail_lines: int,
    sync: Syncer | None = None,
) -> VerifyResult:
    if not commands:
        raise RemoteExecError("no commands to run")
    if "verify" not in host.capabilities:
        raise RemoteExecError(f"{host.name} lacks the verify capability")
    streaming = ssh is None
    ssh = ssh or remote_replay._ssh
    fixed_cap = os.environ.get("TSFP_REMOTE_TRANSFER_CAP_S")
    caps: dict[str, int] = {}

    def default_copy(worker: remote_replay.Worker, src: Path, dst: str) -> int | None:
        caps["cap"] = int(fixed_cap) if fixed_cap else staging_timeout(src.stat().st_size)
        return remote_replay._copy_input(worker, src, dst, timeout=caps["cap"])

    copy = copy or default_copy
    main = _main_of(root)
    if probe_result:
        checked = probe_result
    else:
        with progress.step("probe"):
            checked = probe(host, ssh=ssh)
    if not checked.ok:
        raise RemoteExecError(f"{host.name} refused: {', '.join(checked.reasons)}")
    # T3760: the slot is always reserved before the source sync, because the slot names the
    # exec dir the tree goes to (a blocking caller polls for a free slot up to LOCK_WAIT_S).
    reservation = secrets.token_hex(8)
    cleanup = ssh(host.worker(), _cleanup_stale_reservation_dirs_command(host), timeout=15)
    if cleanup.returncode:
        raise RemoteExecError(f"cannot clean stale slot reservations on {host.name}")
    with progress.step("slot-reserve"):
        reserved_slot = _reserve_waiting(host, ssh, reservation, 0 if nonblocking else LOCK_WAIT_S)
    if host.canon_root and host.sccache != "none":
        # T3472 phase 2: repair the store route first; a failure or hang only costs the cache
        with progress.step("sccache-store") as note:
            limit = float(os.environ.get("TSFP_STORE_ENSURE_TIMEOUT_S", STORE_ENSURE_TIMEOUT_S))
            if not _bounded(
                lambda: sccache_hosts.ensure_host_store(host, ssh, hosts=load_registry()), limit
            ):
                note(f"gave up after {limit:g} s, continuing without the repair")
    tree = tree_of(root, commit)
    run_id = f"{tree[:12]}-{secrets.token_hex(4)}"
    worker = host.worker()
    started = time.monotonic()
    source_dir = exec_dir(host, reserved_slot)
    try:  # T3760: the exact tree into the slot's exec dir by rsync delta, no host git
        with progress.step("source-sync") as note:
            synced = (sync or default_sync)(host, root, tree, _commit_id(root, commit), source_dir)
            note(f"{synced.mode} {synced.files_transferred} files {synced.bytes_sent} B")
    except exec_sync.ExecSyncBusy as error:
        # T3798: a job the reservation could not see still holds the slot's exec dir (its
        # caller stopped, the job runs on until its current step ends). That slot is busy, the
        # host is fine: requeue like a slot race instead of benching the whole host 5 min.
        _release_slot(host, ssh, reservation)
        raise RemoteExecBusy(f"{host.name}:exec_dir_in_use slot={reserved_slot}") from error
    except (exec_sync.ExecSyncError, *TRANSPORT_ERRORS) as error:
        _release_slot(host, ssh, reservation)
        reason = f"staging: {type(error).__name__} {error}"[:160]
        mark_degraded(main, host.name, reason, minutes=STAGING_BENCH_MIN)
        raise RemoteExecError(f"staging on {host.name} failed: {error}"[:400]) from error
    source_mode = synced.mode
    bytes_sent = synced.bytes_sent
    bundles = cdiff_bundle.ids_in(commands)
    if bundles:
        try:  # T3501: the lifted c-diff bundles the commands name, once per host and id
            with progress.step("cdiff-bundle"):
                cdiff_bundle.ship(
                    bundles,
                    main=main,
                    workdir=host.workdir,
                    run=lambda command, limit: ssh(worker, command, timeout=limit),
                    copy=lambda source, dest: copy(worker, source, dest),
                    rpath=rpath,
                )
        except (cdiff_bundle.BundleError, RuntimeError, *TRANSPORT_ERRORS) as error:
            _release_slot(host, ssh, reservation)
            raise RemoteExecError(f"c-diff bundle on {host.name}: {error}"[:300]) from error
    transfer = round(time.monotonic() - started, 2)
    slow = int(os.environ.get("TSFP_REMOTE_TRANSFER_SLOW_S", TRANSFER_SLOW_S))
    if transfer > max(slow, caps.get("cap", 0)):
        mark_degraded(main, host.name, f"source transfer took {transfer} s")
    overall = (
        (0 if nonblocking else LOCK_WAIT_S)
        + 900 * len(fetch)
        + command_timeout_s * len(commands)
        + 120
    )
    script = verify_script(
        host,
        run_id=run_id,
        source_dir=source_dir,
        commands=commands,
        command_timeout_s=command_timeout_s,
        jobs=verify_jobs(host, jobs),
        nonblocking=nonblocking,
        fetch=fetch,
        tail_lines=tail_lines,
        wrapper_text=SCCACHE_WRAPPER.read_text() if host.canon_root else "",
        reserved_slot=reserved_slot,
        tree=tree,
        commit=synced.commit,
        deadline_s=overall + 60,
    )
    try:
        with progress.step("remote-verify") as note:
            note(f"limit {overall} s")
            remote_command = f"bash -c {shlex.quote(script)}"
            if streaming:
                result = stream_ssh(worker, remote_command, overall, progress.remote)
            else:
                result = ssh(worker, remote_command, timeout=overall)
    except subprocess.TimeoutExpired as error:
        _release_slot(host, ssh, reservation)
        mark_degraded(
            main,
            host.name,
            "remote run exceeded its transport timeout",
            RUN_TIMEOUT_DEGRADED_MIN,
        )
        raise RemoteExecError(f"remote run on {host.name} exceeded {overall}s") from error
    except TRANSPORT_ERRORS as error:
        _release_slot(host, ssh, reservation)
        if host_at_fault(error):
            mark_degraded(main, host.name, f"run: {type(error).__name__}")
        raise RemoteExecError(f"remote run on {host.name} lost: {type(error).__name__}") from error
    _release_slot(host, ssh, reservation)
    stats = re.search(r"^@@STATS ([0-9]+) ([0-9]+) ([0-9]+) ([0-9]+)$", result.stdout, re.MULTILINE)
    if stats:
        from tools.cache_telemetry import record_counts

        record_counts("sccache-rustc", int(stats.group(1)), int(stats.group(2)), host=host.name)
        record_counts("sccache-c-cmake", int(stats.group(3)), int(stats.group(4)), host=host.name)
    parsed = parse_results(result.stdout, commands)
    if result.returncode == 124:
        # A timed-out command says nothing about the candidate. Cool the host briefly so
        # concurrently starting workers do not immediately pile onto the same slow runner.
        # claude-w1: name the command; a repeat on one command is a hung test, not a slow host.
        timed = next((item for item in parsed if item.returncode == 124), None)
        reason = "remote command timed out"
        if timed is not None:
            reason += f": {' '.join(timed.command.split())[-80:]}"
            _record_timeout(main, host.name, timed)
        mark_degraded(main, host.name, reason, RUN_TIMEOUT_DEGRADED_MIN)
    verdict = VerifyResult(
        host=host.name,
        tree=tree,
        returncode=result.returncode,
        seconds=round(time.monotonic() - started, 2),
        transfer_seconds=transfer,
        commands=parsed,
        log=(result.stdout + ("\n" + result.stderr if result.stderr else ""))[-60000:],
        source_mode=source_mode,
        bytes_sent=bytes_sent,
        cache_hits=int(synced.files_transferred == 0),
        step_seconds={str(step["step"]): float(step["seconds"]) for step in progress.steps},
    )
    from tools.dispatch import remote_evidence

    try:
        verdict.diagnostic_path = remote_evidence.record_result(
            host, root, list(commands), verdict, infra=infrastructural(verdict)
        )
    except (OSError, ValueError) as error:
        progress.say(f"diagnostic receipt unavailable: {type(error).__name__}")
    return verdict


def infrastructural(result: VerifyResult) -> bool:
    """True when a non-zero remote run says nothing about the candidate (ssh, busy slot,
    staging, fetch, missing tool or toolchain): callers then verify locally instead."""
    if result.returncode == 0:
        return False
    if result.returncode in INFRA_EXITS:
        return True
    return bool(INFRA_LOG.search(result.log))


def pick_host(
    capability: str,
    *,
    hosts: Sequence[HostSpec] | None = None,
    ssh: SshRunner | None = None,
    need_owner_inputs: bool = False,
    root: Path | None = None,
) -> tuple[HostSpec, Probe] | None:
    """The admissible host with the most free memory for `capability`, else None."""
    ranked = ranked_hosts(
        capability, hosts=hosts, ssh=ssh, need_owner_inputs=need_owner_inputs, root=root
    )
    return ranked[0] if ranked else None


def host_disabled(root: Path, name: str) -> bool:
    """True while the owner has reclaimed the machine (`tmp/codexapi/<name>.off` in the main
    checkout or any parent, the same switch remote_prebuilt honours)."""
    return name in retired_hosts.retired_names() or any(
        (checkout / "tmp" / "codexapi" / f"{name}.off").exists()
        for checkout in (root, *root.parents)
    )


def ranked_hosts(
    capability: str,
    *,
    hosts: Sequence[HostSpec] | None = None,
    ssh: SshRunner | None = None,
    need_owner_inputs: bool = False,
    root: Path | None = None,
    skipped: list[str] | None = None,
    waiting: list[tuple[HostSpec, Probe]] | None = None,
) -> list[tuple[HostSpec, Probe]]:
    """Admissible hosts for `capability`, ordered by headroom; disabled hosts skipped.

    T3461: `skipped` collects "<host>:<reason>" for every refused host (21 of 44 remote
    attempts ended as a bare no_eligible_host). T3516: a host refused ONLY by its host-health
    guard is also appended to `waiting`: its transport is fine and the caller re-probes it in
    its wait window (it is never marked degraded)."""
    root = root or _main_root()
    found_hosts: list[tuple[HostSpec, Probe]] = []
    refused = skipped if skipped is not None else []
    candidates = list(hosts if hosts is not None else load_registry())
    spared = spare_least_recently_failed(
        _main_of(root),
        [
            h.name
            for h in candidates
            if capability in h.capabilities and not host_disabled(root, h.name)
        ],
    )
    for host in candidates:
        if capability not in host.capabilities:
            refused.append(f"{host.name}:lacks_{capability}_capability")
            continue
        if host_disabled(root, host.name):
            refused.append(f"{host.name}:disabled")
            continue
        degraded = "" if host.name == spared else degraded_reason(_main_of(root), host.name)
        if degraded:  # T3472 p3: slow or failing transport, retry later
            refused.append(f"{host.name}:degraded({degraded})"[:120])
            continue
        if need_owner_inputs and not host.has_owner_inputs():
            continue
        found = probe(host, ssh=ssh, root=_main_of(root))
        if found.ok:
            found_hosts.append((host, found))
        else:
            refused.append(f"{host.name}:{','.join(found.reasons)}"[:120])
            if waiting is not None and found.reachable and host_health.is_refusal(found.reasons):
                waiting.append((host, found))
    # T3748: the shared headroom score (load per core, then free memory), as replay and builds.
    from tools.fleet.headroom import score

    return sorted(
        found_hosts,
        key=lambda pair: score(pair[1].load1, pair[1].cores, pair[1].mem_available_mb / 1024),
    )


def load_bucket(probe: Probe) -> int:
    """0 below 1x cores, 1 below 2x, 2 above: a loaded host (capturing, other verifies) goes last.

    2026-10-25: verifies kept landing on the laptop (load 42/16) and deck (26/8) while both
    droplets idled, because the order was free memory only."""
    if probe.load1 is None or probe.cores <= 0:
        return 1
    ratio = probe.load1 / probe.cores
    return 0 if ratio < 1 else 1 if ratio < 2 else 2


def gate_replay_hosts(
    hosts: Sequence[HostSpec] | None = None, *, ssh: SshRunner | None = None
) -> tuple[str, list[str]]:
    """TSFP_REPLAY_REMOTE_HOSTS JSON for available replay-capable workers.

    The `replay` capability authorizes the existing private, hash-verified staging flow;
    owner_files records residency and avoids transfers where possible. Return workers in
    free-memory order so the best eligible remote host is attempted first.
    """
    selected: list[dict[str, object]] = []
    skipped: list[str] = []
    for host in hosts if hosts is not None else load_registry():
        if not host.replay_repo:
            skipped.append(f"{host.name}:no_replay_repo")
            continue
        if "replay" not in host.capabilities:
            skipped.append(f"{host.name}:no_replay_capability")
            continue
        found = probe(host, ssh=ssh)
        if not found.ok:
            skipped.append(f"{host.name}:{','.join(found.reasons)}")
            continue
        selected.append(
            (
                found.mem_available_mb,
                {
                    "name": host.name,
                    "target": host.target,
                    "port": host.port,
                    "repo": host.replay_repo,
                    "max_slots": host.slots,
                },
            )
        )
    selected.sort(key=lambda item: -item[0])
    return (json.dumps([entry for _mem, entry in selected]) if selected else ""), skipped


def apply_gate_environment(environ: dict[str, str] | None = None) -> dict[str, object]:
    """Opt-in hook for tools.gate: route replay captures to admissible remote hosts."""
    env = os.environ if environ is None else environ
    if not enabled(env):
        return {"enabled": False}
    if env.get("TSFP_REPLAY_REMOTE_HOSTS", "").strip():
        return {"enabled": True, "hosts": "preset", "skipped": []}
    value, skipped = gate_replay_hosts()
    if value:
        env["TSFP_REPLAY_REMOTE_HOSTS"] = value
    return {
        "enabled": True,
        "hosts": [entry["name"] for entry in json.loads(value)] if value else [],
        "skipped": skipped,
    }


def clean(host: HostSpec, *, ssh: SshRunner | None = None, target: bool = False) -> int:
    """Remove leftover runs/archives (and the cargo target cache with --target)."""
    ssh = ssh or remote_replay._ssh
    paths = [host.workdir + "/runs", host.workdir + "/archives"]
    if target:
        paths += [host.cargo_target] + [f"{base}/target*" for base in host.legacy_workdirs]
        if host.slot_targets:
            paths += [f"{host.cargo_target}-src"]
            paths += [f"{host.cargo_target}-src-{slot}" for slot in range(host.slots)]
        if host.canon_root:  # T3472 phase 2: the fixed source dirs (cargo home stays)
            paths += [f"{host.canon_root}/src"]
            paths += [f"{host.canon_root}/src-{slot}" for slot in range(host.slots)]
    command = (
        f"{all_slots_command(host.slots, 600)} && rm -rf {' '.join(rpath(path) for path in paths)}"
    )
    return ssh(host.worker(), command, timeout=600).returncode


def _main_root() -> Path:
    return Path(__file__).resolve().parents[2]


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(prog="python3 -m tools.wt.remote_exec", description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    sub.add_parser("hosts", help="print the host registry")
    probe_parser = sub.add_parser("probe", help="health/capacity probe")
    probe_parser.add_argument("--host", action="append", help="host name (default: all)")
    verify = sub.add_parser("verify", help="run a command set on a candidate tree remotely")
    verify.add_argument("--host", default="auto", help="host name or auto (most free memory)")
    verify.add_argument("--commit", default="HEAD", help="commit or tree-ish to archive")
    verify.add_argument("--repo", type=Path, default=Path.cwd(), help="local repository")
    verify.add_argument("--cmd", action="append", required=True, help="command (repeatable)")
    verify.add_argument(
        "--timeout", type=int, default=DEFAULT_COMMAND_TIMEOUT_S, help="per-command timeout seconds"
    )
    verify.add_argument(
        "--expect-tests",
        action="store_true",
        help="return infrastructure status 3 when all commands report zero tests",
    )
    verify.add_argument("--jobs", type=int, help="CARGO_BUILD_JOBS (default cores/2)")
    sub.add_parser("gate-env", help="print TSFP_REPLAY_REMOTE_HOSTS for remote gate replays")
    clean_parser = sub.add_parser("clean", help="remove leftover run outputs on a host")
    clean_parser.add_argument("--host", required=True)
    clean_parser.add_argument("--target", action="store_true", help="also drop the cargo target")
    args = parser.parse_args(argv)

    if args.command == "hosts":
        print(json.dumps([asdict(host) for host in load_registry()], indent=2))
        return 0
    if args.command == "probe":
        hosts = load_registry()
        names = set(args.host or [host.name for host in hosts])
        results = [asdict(probe(host)) | {"ok": None} for host in hosts if host.name in names]
        for result in results:
            result["ok"] = result["reachable"] and not result["reasons"]
        print(json.dumps(results, indent=2))
        return 0 if all(result["ok"] for result in results) else 1
    if args.command == "gate-env":
        value, skipped = gate_replay_hosts()
        print(json.dumps({"TSFP_REPLAY_REMOTE_HOSTS": value, "skipped": skipped}, indent=2))
        return 0 if value else 1
    if args.command == "clean":
        return clean(find_host(args.host), target=args.target)
    if args.host == "auto":
        picked = pick_host("verify", root=_main_root())
        if picked is None:
            print(json.dumps({"error": "no admissible verify host"}))
            return 3
        host, checked = picked
    else:
        host, checked = find_host(args.host), None
    try:
        result = run_verify(
            host,
            args.repo,
            args.commit,
            args.cmd,
            command_timeout_s=args.timeout,
            jobs=args.jobs,
            probe_result=checked,
        )
    except RemoteExecError as error:
        print(json.dumps({"error": str(error)}))
        return 3
    sys.stderr.write(result.log[-4000:] + "\n")
    summary = asdict(result)
    summary.pop("log")
    print(json.dumps(summary, indent=2))
    if (
        args.expect_tests
        and result.returncode == 0
        and not any((command.passed_tests or 0) > 0 for command in result.commands)
    ):
        print("remote verify: no tests ran; no verdict", file=sys.stderr)
        return 3
    return 0 if result.returncode == 0 else 1


if __name__ == "__main__":
    raise SystemExit(main())
