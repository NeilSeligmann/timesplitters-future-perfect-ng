"""Remote Bevy prebuilt builds (T3428).

The local prebuilt cache remains authoritative. T3760 stage 4 (owner decision 2026-10-26, remote
hosts are execution-only): the commit's exact tree is synced by `exec_sync` (rsync delta) into a
stable exec dir on the worker (`<canon_root>/prebuilt`, sccache keys on that absolute path), the
build holds the dir's lease and gets the commit as data (TSFP_SOURCE_COMMIT / TSFP_SOURCE_TREE,
`.tsfp-source.json`, the tree listing read by `tools.wt.source_identity`); no git runs on the
host. The worker publishes through its normal prebuilt cache; the whole artifact dir is pulled
back under the artifact's `<sha>.lock` lease (the host GC skips it) and verified file by file
before the local atomic publication.
"""

from __future__ import annotations

import contextlib
import json
import os
import re
import shlex
import subprocess
import sys
from collections.abc import Mapping
from pathlib import Path

from tools import host_registry, hostcaps
from tools.wt import (
    build_container,
    cache_keys,
    exec_sync,
    git_compat,
    host_budget,
    host_health,
    lift_ship,
    prebuilt_diagnostics,
    remote_replay,
    remote_toolchain,
    retired_hosts,
)
from tools.wt.remote_exec import SLOT_LOCK, rpath, slot_paths

REMOTE_BUILD_JOBS = 8
REMOTE_BUILD_RESERVE_MB = 12_000
REMOTE_BUILD_RSS_MB = 8_000
REMOTE_BUILD_ORDER = ("laptop", "deck", "droplet", "droplet2")
WARM_BUILD_ORDER = REMOTE_BUILD_ORDER
"""T3737: LAN hosts first (no upload to the droplet, no shared droplet slot), droplets last. One
order for warm-main, gates and the regression loop (it tried droplet, droplet2, laptop and failed on
busy, hash and a laptop publish error). Hosts that are switched off or lack the `build` capability
and a `data_root` are skipped. See the history below."""
"""Workers tried in order for a prebuilt build, then the local build. I-T3505-9 (2026-10-22):
the laptop is back as the second builder. Its image was resized to 148 GB (data cap 110 GB), so
the space reason below no longer holds, and with the droplet as the only builder one droplet build
in flight made every other prebuilt (gates, the T3587 regression loop) fail as "busy".
Before that, T3461 (coordinator 2026-10-21): the laptop is out. Its 59 GB worker
volume (45 GB cap) cannot hold remote-prebuilt
(about 10 GB), its cargo target (9 GB) and sccache (5 GB) beside the light workers and kept going
over cap, which paused them; it keeps the replay role. The droplet (388 GB free, about 19 min
cold builds) is first and only. The droplet (159.223.192.217, 8 vCPU, 15 GB,
Ubuntu 24.04) builds since T3459 (docs/t-droplet-host.md): toolchain, owner inputs and XBE are in
place. It stays inert until `tools/data/remote_hosts.json` lists it with the `build` capability
and a `data_root` (or TSFP_REPLAY_REMOTE_HOSTS names it)."""
WORKER_LIMITS = {
    "droplet": {"jobs": 6, "reserve_mb": 3_000, "rss_mb": 7_000, "slots": 2},
    # droplet2 (owner 2026-10-22, temporary): 8 vCPU / 23 GB shared with Codex workers (memguard).
    "droplet2": {"jobs": 6, "reserve_mb": 3_000, "rss_mb": 7_000, "slots": 2},
    # deck (T3737): the owner's Steam Deck, 14 GB, one build at a time.
    "deck": {"jobs": 4, "reserve_mb": 2_000, "rss_mb": 7_000},
}
"""Per-worker overrides of the build parallelism and the admission memory model. The droplet
shares 15 GB with Codex workers (memguard) so it builds with six jobs and a smaller reserve.
`slots` is the T3411 build-slot pool size the remote build sees: the droplet's default pool is
one slot (MemTotal // 16 GB), which the workers' `cargo test` runs hold for tens of minutes, so
the build looked at slot indexes 0 and 1 (the workers keep using index 0) and waited up to
TSFP_BUILD_WAIT_S for nothing (measured 2026-10-21)."""
REMOTE_BUILDABLE_WORKERS = {*REMOTE_BUILD_ORDER, *WARM_BUILD_ORDER}
REMOTE_BUSY_EXIT = 75
REMOTE_LOW_DISK_EXIT = 74
REMOTE_MIN_FREE_KB = 12_000_000
PULL_LEASE_WAIT_S = 60
"""How long an artifact pull waits for its `<sha>.lock` lease (only a GC holds it exclusively)."""
PULL_TIMEOUT_S = 1800
BUILD_TIMEOUT_S = 3600
"""Transport limit of one remote build; its exec dir lease ends 60 s after it (`REAP_SH`)."""
HOST_GIT_CALLS = re.compile(r"^REMOTE_PREBUILT_HOST_GIT_CALLS=(\d+)$", re.MULTILINE)
REGISTRY = host_registry.REGISTRY
_SHA = re.compile(r"^[0-9a-f]{40}$")


class RemotePrebuiltError(RuntimeError):
    """A remote build failed or returned an artifact that did not verify."""


class RemoteBusyError(RemotePrebuiltError):
    """The worker is already running a prebuilt build; the caller may build locally."""


class RemoteBuildFailed(RemotePrebuiltError):
    """The remote build command ran and exited unsuccessfully."""


def registry_workers(path: Path = REGISTRY) -> dict[str, remote_replay.Worker]:
    """Build-capable workers from the T3435 host registry (`data_root` names the plain data
    dir, cx-execlayout: never a git clone).

    T3448: the SSH worker list used to come only from TSFP_REPLAY_REMOTE_HOSTS, which the
    supervisor loop and the environment hook never set, so every ahead-of-main remote build
    ended in "remote prebuild unavailable". The registry is tracked and always present.
    """
    try:
        entries = json.loads(path.read_text()).get("hosts", [])
    except (OSError, ValueError):
        return {}
    found: dict[str, remote_replay.Worker] = {}
    for entry in retired_hosts.active(entries):  # T3753: retired hosts build nothing
        name, repo = entry.get("name"), entry.get("data_root")
        buildable = "build" in entry.get("capabilities", ())
        if name not in REMOTE_BUILDABLE_WORKERS or not repo or not buildable:
            continue
        target = f"{entry['user']}@{entry['address']}"
        found[name] = remote_replay.Worker(name, target, int(entry["port"]), repo, 1)
    return found


def canon_root_of(name: str, path: Path | None = None) -> str:
    """T3472 phase 3: the fixed canon root of a registry host that has a shared sccache route,
    else "". sccache keys a compile by source dir and CARGO_HOME, so the prebuilt build runs in
    `<canon_root>/prebuilt` with `<canon_root>/cargo` and its hits are equal on every such host."""
    for entry in host_registry.hosts_or_empty(path or REGISTRY):  # T3754
        if entry.get("name") == name and entry.get("sccache", "none") != "none":
            return str(entry.get("canon_root", ""))
    return ""


def host_layout_of(name: str, path: Path | None = None) -> dict[str, str]:
    """cx-execlayout: the exec-only layout keys (`host_venv`, `toolchain_dir`,
    `replay_cache_dir`) of a registry host, {} for a host missing there."""
    for entry in host_registry.hosts_or_empty(path or REGISTRY):
        if entry.get("name") == name:
            return {key: str(entry.get(key) or "") for key in host_registry.HOST_LAYOUT_KEYS}
    return {}


def home_path(path: str, home: str) -> str:
    """A registry `~/` path made absolute with the worker's probed $HOME: the build script
    quotes paths and writes the interpreter into the git shim, so a tilde never expands."""
    return f"{home.rstrip('/')}/{path[2:]}" if path.startswith("~/") and home else path


def workers() -> dict[str, remote_replay.Worker]:
    """Workers permitted to build host artifacts: environment entries win over the registry."""
    configured = {
        worker.name: worker
        for worker in remote_replay.configured_workers()
        if worker.name in REMOTE_BUILDABLE_WORKERS
    }
    return {**registry_workers(), **configured}


def worker_disabled(root: Path, name: str) -> bool:
    """True while the owner has reclaimed the machine (`tmp/codexapi/<name>.off`).

    T3737: also true for a host the fleet config switched off (its `off_file`, e.g. the
    server), so a prebuild never lands on a machine taken out of the fleet.
    """
    if name in remote_replay.switched_off():
        return True
    return any(
        (checkout / "tmp" / "codexapi" / f"{name}.off").exists()
        for checkout in (root, *root.parents)
    )


def local_memory_available_mb(proc: Path = Path("/proc")) -> int | None:
    """Local MemAvailable under the container's cgroup ceiling (T3844, tools.hostcaps)."""
    available = hostcaps.compute_mem_available_bytes(proc=proc)
    return None if available is None else available // 2**20


def should_build_remotely(
    requested: str, *, available_mb: int | None = None, threshold_mb: int | None = None
) -> bool:
    """Resolve local/auto/worker policy.

    T3448: `auto` is remote first with the local build as fallback (an absent, busy or
    unhealthy worker falls through). Setting TSFP_PREBUILT_REMOTE_THRESHOLD_MB restores the
    old rule of offloading only while local MemAvailable is below the threshold.
    """
    if requested == "local":
        return False
    if requested in REMOTE_BUILDABLE_WORKERS:
        return True
    if requested != "auto":
        raise ValueError(f"unknown remote prebuilt mode: {requested}")
    if threshold_mb is None:
        configured = os.environ.get("TSFP_PREBUILT_REMOTE_THRESHOLD_MB")
        if not configured:
            return True
        threshold_mb = int(configured)
    if available_mb is None:
        available_mb = local_memory_available_mb()
    return available_mb is not None and available_mb < threshold_mb


def _is_local_checkout(root: Path, remote_repo_path: str) -> bool:
    """Whether the worker's clone belongs to the checkout family this process runs in."""
    try:
        local = {str(path.resolve()) for path in (root, *root.parents) if (path / ".git").exists()}
        return str(Path(remote_repo_path).resolve()) in local
    except OSError:
        return False


def remote_root_of(repo_path: str) -> str:
    """The worker's prebuilt state dir: logs, build lock and the artifact cache (`artifacts`)."""
    return f"{repo_path}/tmp/wt-cache/remote-prebuilt"


def source_dir_of(name: str, repo_path: str, sha: str = "") -> str:
    """T3760: the exec dir the commit's tree is synced into (a git worktree before stage 4).

    T3885: one dir per commit (`<base>-<sha12>`), so successive or concurrent prebuilds of
    different commits never share a CMake dir or incremental cargo target (a later commit's
    objects linked into an earlier build). sccache and the cargo registry stay shared."""
    canon_root = canon_root_of(name)
    base = f"{canon_root}/prebuilt" if canon_root else f"{remote_root_of(repo_path)}/worktree"
    return f"{base}-{sha[:12]}" if sha else base


KEEP_COMMIT_DIRS = 2
"""Per-commit exec dirs kept besides the one being built (newest first)."""


def prune_plan(
    dirs: list[tuple[str, float]],
    current: str,
    *,
    free_kb: int,
    floor_kb: int,
    dir_kb: int,
    keep: int = KEEP_COMMIT_DIRS,
) -> list[str]:
    """Per-commit exec dirs to delete: all but the `keep` newest others; while free space is
    under `floor_kb`, further oldest ones (assuming each frees `dir_kb`). Never `current`."""
    others = sorted((d for d in dirs if d[0] != current), key=lambda d: d[1], reverse=True)
    doomed = [name for name, _ in others[keep:]]
    spare = [name for name, _ in others[:keep]]
    free = free_kb + dir_kb * len(doomed)
    while spare and free < floor_kb:
        doomed.append(spare.pop())
        free += dir_kb
    return doomed


def prune_command(name: str, repo_path: str, sha: str) -> str:
    """Shell run on the worker before a sync: prune old per-commit exec dirs (never leased,
    synced or used within `exec_sync.LEASE_FRESH_S`, nor the current one)."""
    q = shlex.quote
    base = source_dir_of(name, repo_path)
    current = source_dir_of(name, repo_path, sha)
    fresh_min = exec_sync.LEASE_FRESH_S // 60
    return (
        f"{exec_sync.LEASED_SH}; n=0; "
        f"for d in $(ls -dt {base}-* 2>/dev/null); do "
        f'[ "$d" = {q(current)} ] && continue; case "$d" in *.lock|*.holders) continue;; esac; '
        f"n=$((n+1)); free=$(df -Pk {q(os.path.dirname(base))} | awk 'NR==2 {{print $4}}'); "
        f'[ "$n" -le {KEEP_COMMIT_DIRS} ] && [ "$free" -ge {REMOTE_MIN_FREE_KB} ] && continue; '
        'tsfp_leased "$d" && continue; '
        '[ -n "$(find "$d" -maxdepth 0 -mmin -' + str(fresh_min) + ')" ] '
        '&& [ "$free" -ge ' + str(REMOTE_MIN_FREE_KB) + " ] && continue; "
        'rm -rf "$d" "$d.lock" "$d.holders"; done; true'
    )


def protected_paths() -> tuple[str, ...]:
    """Untracked state of the exec dir a sync keeps: build dirs and caches under tmp/, and
    every link `tools.wt bootstrap` makes (derived from `core.LINKS`, rule 45)."""
    from tools.wt import core

    links = (f"/{spec.path}" for spec in core.LINKS if not spec.path.startswith("tmp/"))
    return ("/tmp/", *links)


def sync_source(
    worker: remote_replay.Worker, root: Path, tree: str, commit: str, dest: str
) -> exec_sync.SyncResult:
    """Place `tree` exactly at `dest` on the worker (rsync delta; tests replace this)."""
    endpoint = exec_sync.Endpoint(
        worker.name, worker.target, worker.port, dest, remote_replay._ssh_transport(worker)
    )
    return exec_sync.sync(endpoint, root, tree, commit=commit, protect=protected_paths())


def busy_probe_command(remote_root: str) -> str:
    """Ask for the build lock before syncing: a running build holds the exec dir's lease, and
    a sync would wait on it for minutes instead of moving to the next worker."""
    q = remote_replay._remote_path
    lock = f"{remote_root}/build.lock"
    return (
        f"mkdir -p {q(remote_root)} && {{ flock -n {q(lock)} true || echo REMOTE_PREBUILT_BUSY; }}"
    )


def pull_argv(
    worker: remote_replay.Worker,
    artifact_rel: str,
    destination: Path,
    *,
    wait_s: int = PULL_LEASE_WAIT_S,
) -> list[str]:
    """rsync of the WHOLE artifact dir (binary, libraries, stamp, manifest) back to staging.

    The worker side runs under a shared flock of the artifact's `<cache>/.locks/<sha>.lock`, the
    lease `prebuilt.lease` takes and `prebuilt.gc` skips, so no host GC removes the dir while it
    is read (I-T3760-3). No include/exclude filter: the replay-result filter of
    `remote_replay._pull_results` (only `*.json`/`*.trace`) dropped every file but the manifest."""
    cache, sha = artifact_rel.rstrip("/").rsplit("/", 1)
    lock_dir = f"{cache}/.locks"
    lease = exec_sync.lease_wrap(remote_replay._remote_path(f"{lock_dir}/{sha}"), wait_s)
    return [
        "rsync",
        "-a",
        "--compress-choice=zstd",
        "--compress-level=3",
        f"--timeout={exec_sync.IO_TIMEOUT_S}",
        "-e",
        remote_replay._ssh_transport(worker),
        f"--rsync-path=mkdir -p {remote_replay._remote_path(lock_dir)} && {lease} rsync",
        f"{worker.target}:{artifact_rel}/",
        str(destination) + "/",
    ]


def pull_artifact(
    worker: remote_replay.Worker,
    artifact_rel: str,
    destination: Path,
    *,
    wait_s: int = PULL_LEASE_WAIT_S,
) -> None:
    destination.mkdir(parents=True, exist_ok=True)
    done = subprocess.run(
        pull_argv(worker, artifact_rel, destination, wait_s=wait_s),
        text=True,
        capture_output=True,
        timeout=PULL_TIMEOUT_S,
        check=False,
    )
    if done.returncode:
        raise RemotePrebuiltError(
            f"artifact pull from {worker.name} failed (exit {done.returncode}): "
            f"{done.stderr.strip()[-400:]}"
        )


# T3448 live finding: a toolchain left half-installed (disk-full era) under the repo-local
# RUSTUP_HOME made `rustup which` fail and `rustup toolchain install` die on a leftover
# share/doc dir, so build.sh exited 1 before compiling. Prefer a RUSTUP_HOME whose pinned
# toolchain resolves (repo-local first, then ~/.rustup) and clear a broken local install so
# build.sh can reinstall it. Always succeeds: build.sh stays the judge of the toolchain.
TOOLCHAIN_DOCTOR = (
    "{ pin=$(sed -n 's/.*STREQUAL \"rustc \\([^ ]*\\) .*/\\1/p' "
    "cmake/rust-ports.cmake | head -n 1); ok=; "
    'for h in "${RUSTUP_HOME:-}" "$HOME/.rustup"; do '
    '[ -n "$h" ] && RUSTUP_HOME=$h rustup which rustc --toolchain "$pin" >/dev/null 2>&1 '
    "&& { export RUSTUP_HOME=$h; ok=1; break; }; done; "
    '[ -n "$ok" ] || rustup toolchain uninstall "$pin" >/dev/null 2>&1; true; }'
)


def worker_limit(name: str, key: str, default: int) -> int:
    """Per-worker override (WORKER_LIMITS) of a build parameter, else the global default."""
    return WORKER_LIMITS.get(name, {}).get(key, default)


def build_slots_export(name: str) -> str:
    """` TSFP_BUILD_SLOTS=N` for workers whose pool needs widening, else an empty string."""
    slots = WORKER_LIMITS.get(name, {}).get("slots")
    return f" TSFP_BUILD_SLOTS={slots}" if slots else ""


def capture_exports() -> str:
    """Carry the requested variant; a worker's inherited capture flags are not inputs."""
    from tools.wt import prebuilt

    variant = prebuilt.capture_identity()
    reset = "unset TSFP_BUILD_CAPTURE TSFP_CAPTURE_KIND"
    if not variant:
        return reset
    kind = shlex.quote(variant.get("capture_kind", "oracle"))
    return f"{reset}; export TSFP_BUILD_CAPTURE=1 TSFP_CAPTURE_KIND={kind}"


def _remote_build_command(
    worker: remote_replay.Worker,
    *,
    repo_path: str,
    home: str,
    sha: str,
    input_hash: str,
    tree: str,
    git_mode: str = "strict",
    lift_install: str = "",
    profile: str | None = None,
) -> str:
    """Build the commit synced into the worker's exec dir, behind the single-build lock.

    `profile` (T3824): a build profile other than the gate one is passed on, so the worker
    publishes it under artifacts/<sha>-<profile> exactly as a local profile build would.

    T3760: no host git. The exec dir must carry the marker of `tree` (a sync for another build
    in between makes the build exit busy, the caller retries), the build holds the dir's lease
    (a sync waits for it, host cleaners skip it) and gets the commit as data. TSFP_MAIN_ROOT
    names the worker dir that stands in for the main checkout (venv, owner inputs, cargo and
    rust homes, caches), which the git common dir used to name.

    `git_mode` (`git_compat.mode_for`, computed on the owner PC): a commit from before the
    git-less tools runs its own tools, which ask git; the `git_compat` shim, first on PATH,
    answers them from the shipped listing (`answer`) or fails them like a host without git
    (`strict`), and logs every call (`REMOTE_PREBUILT_HOST_GIT_CALLS`). The disk budget is the
    worker's own (`TSFP_DISK_PROFILE=remote_worker`), not the owner PC's floors.

    `lift_install` (`lift_ship.ship`): installs the lift and key inputs the owner PC shipped
    into the data root before bootstrap links them, so build.sh never relifts on a host."""
    del input_hash  # the build derives it from the synced tree; the caller verifies the match
    q = remote_replay._remote_path
    guard, host_cores, host_slots = host_health.facts_for_host(worker.name)
    remote_root = remote_root_of(repo_path)
    artifact_cache = f"{remote_root}/artifacts"
    canon_root = canon_root_of(worker.name)
    worktree = source_dir_of(worker.name, repo_path, sha)
    lock = f"{remote_root}/build.lock"
    log = f"{remote_root}/logs/{sha}.log"
    time_log = f"{remote_root}/logs/{sha}.time"
    git_log = f"{remote_root}/logs/{sha}.git-calls"
    layout = {key: home_path(value, home) for key, value in host_layout_of(worker.name).items()}
    # T3760: provisioning (`python-deps`) keeps the exec venv beside the exec dirs; the legacy
    # repo venv (`host_venv`) went missing with a disk cleanup and every warm-main failed for 6 h
    # (I-T3865-7). The registry venv is only the fallback for hosts without a canon root.
    python = (
        f"{canon_root}/venv/bin/python"
        if canon_root
        else f"{layout.get('host_venv') or repo_path + '/.venv'}/bin/python"
    )
    toolchain = layout.get("toolchain_dir", "")
    repo_cd = remote_replay._repo_cd(worker.repo)
    python_q = shlex.quote(python)
    shim_base = canon_root or remote_root
    owner_dir = q(repo_path + "/tmp/owner")
    marker = f"{q(worktree)}/{exec_sync.MARKER}"
    git_calls = (
        "printf 'REMOTE_PREBUILT_HOST_GIT_CALLS=%s\\n' "
        f'"$(cat {q(git_log)} 2>/dev/null | wc -l)"; '
        f"head -n 5 {q(git_log)} 2>/dev/null || true"
    )
    needle = shlex.quote(f'"tree": "{tree}"')
    # T3803: the build leaves the host's capture-reserved CPUs to capture replays.
    pin = host_health.build_pin_assign(host_health.capture_reserved_cores(worker.name))
    build_command = (
        f"{pin}; ${host_health.PIN_VAR} /usr/bin/time -v {python_q} -m tools.wt prebuilt build "
        f"--worktree {q(worktree)} --commit {shlex.quote(sha)} --keep 12 --remote local "
        + (f"--profile {shlex.quote(profile)} " if profile else "")
        + f"> {q(log)} 2> {q(time_log)} || {{ rc=$?; "
        "echo REMOTE_PREBUILT_BUILD_FAILED; "
        f"tail -n 24 {q(log)}; "
        # T3737: the stderr traceback precedes `time -v`'s statistics (head -n 3 cut it off).
        f"sed '/^Command exited/,$d' {q(time_log)} | tail -n 15; "
        f"{git_calls}; "
        f'exit "$rc"; }}'
    )
    # Keep the actual prebuilt's cache evidence in this command's output. A follow-up
    # `remote_exec verify` zeros the shared daemon's counters before every command, so
    # querying stats in a second SSH session loses the build's hit/miss interval.
    stats_awk = (
        "/^Compile requests +[0-9]+$/ {r=$3} /^Cache hits +[0-9]+$/ {h=$3} "
        "/^Cache misses +[0-9]+$/ {m=$3} "
        'END {printf "REMOTE_PREBUILT_SCCACHE_STATS requests=%d hits=%d misses=%d\\n", r, h, m}'
    )
    sccache_stats = (
        'if [ -n "${TSFP_SCCACHE:-}" ]; then '
        f'"$TSFP_SCCACHE" --show-stats 2>/dev/null | awk {shlex.quote(stats_awk)}; '
        'else echo "REMOTE_PREBUILT_SCCACHE_STATS unavailable"; fi; true'
    )
    script = " && ".join(
        (
            f"mkdir -p {q(remote_root + '/logs')}",
            f"cd {repo_cd}",
            f"{{ test \"$(df -Pk . | awk 'NR==2 {{print $4}}')\" -ge {REMOTE_MIN_FREE_KB} || "
            f"{{ echo REMOTE_PREBUILT_LOW_DISK; exit {REMOTE_LOW_DISK_EXIT}; }}; }}",
            # Leftovers of the archive/bundle transport before T3760 stage 4.
            f"rm -rf {q(remote_root + '/sources')} {q(remote_root + '/worktrees')}",
            f"exec 9>{q(lock)}",
            "{ flock -n 9 || { echo REMOTE_PREBUILT_BUSY build-lock; "
            f"exit {REMOTE_BUSY_EXIT}; }}; }}",
            # T3459: a host-wide slot shared with remote_exec verify runs (fd 8). Any free one
            # (T3775: the laptop's verify reservations kept slot 0 and starved every prebuild).
            f"mkdir -p $(dirname {rpath(SLOT_LOCK)})",
            "slot=; for s in "
            + " ".join(rpath(path) for path in slot_paths(host_slots))
            + '; do exec 8>"$s"; flock -n 8 && { slot=$s; break; }; done; '
            '[ -n "$slot" ] || { echo REMOTE_PREBUILT_BUSY execution-slot; '
            f"exit {REMOTE_BUSY_EXIT}; }}",
            # T3516: host-health gate with the slot held (inert for hosts without `host_health`)
            host_health.gate_command(
                guard,
                cores=host_cores,
                slot_files=[rpath(path) for path in slot_paths(host_slots)],
            ),
            # T3760: the synced tree must still be ours (another caller's sync in between: busy).
            f"{{ grep -qF {needle} {marker} || "
            f"{{ echo REMOTE_PREBUILT_BUSY source changed; exit {REMOTE_BUSY_EXIT}; }}; }}",
            exec_sync.lease_hold(q(worktree), fd=7, deadline_s=BUILD_TIMEOUT_S + 60, attached=True),
            f"cd {q(worktree)}",
            # Remote prebuilds are per-commit snapshots, not interactive edit loops.  In
            # particular, never allow a caller's incremental setting to multiply their
            # already large target families (T3947).
            "export TSFP_CARGO_FAMILY=prebuilt-remote TSFP_WT_FAMILY=prebuilt-remote "
            "TSFP_CARGO_INCREMENTAL=0 TSFP_PREBUILT_INCREMENTAL=0 CARGO_INCREMENTAL=0",
            exec_sync.identity_exports(sha, tree),
            # The worker's disk floors (remote_hosts.json), not the owner PC's 100 GB root floor
            # that refused every remote build from 04:17 on (laptop 98 GB, deck 34 GB free).
            f"export TSFP_DISK_PROFILE=remote_worker TSFP_DISK_HOST={shlex.quote(worker.name)}",
            # Every git call inside the tree goes to the shim (answered or refused, logged).
            git_compat.install_command(shim_base, python),
            f"rm -f {q(git_log)}",
            git_compat.exports(shim_base, git_mode, git_log),
            # build.rs stamps fall back to TSFP_GIT_SHA (git rev-parse --short=12 HEAD before).
            f"export TSFP_MAIN_ROOT={q(repo_path)} TSFP_GIT_SHA={shlex.quote(sha[:12])}",
            # A stray git call must fail, never find a repository above the exec dir.
            f"export GIT_CEILING_DIRECTORIES={q(worktree.rsplit('/', 1)[0])}",
            # sub-lift: the owner PC's lift for this tree (hosts have no XBE to relift with).
            *((f"{{ {lift_install}; }}",) if lift_install else ()),
            f"{{ {capture_exports()}; }}",
            f"{python_q} -m tools.wt bootstrap --worktree {q(worktree)} --files-dir {owner_dir}",
            f'eval "$({python_q} -m tools.wt env --worktree {q(worktree)})"',
            f"export GIT_CEILING_DIRECTORIES={q(worktree.rsplit('/', 1)[0])}",
            # cx-execlayout: the registry `toolchain_dir` names the rustup home explicitly.
            *((f"export RUSTUP_HOME={q(toolchain + '/rustup')}",) if toolchain else ()),
            *(
                (
                    f"mkdir -p {q(canon_root + '/cargo')}",
                    f"export CARGO_HOME={q(canon_root + '/cargo')} "
                    f"TSFP_BEVY_CARGO_HOME={q(canon_root + '/cargo')}",
                )
                if canon_root
                else ()
            ),
            TOOLCHAIN_DOCTOR,
            # build.sh refuses a cargo home without registry/ (exit 5): a wiped host refills it
            # here, network allowed, instead of failing every build (I-T3865-9).
            *(
                (
                    f"{{ [ -d {q(canon_root + '/cargo/registry')} ] || cargo fetch --locked "
                    f"--manifest-path {q(worktree + '/crates/tsfp-bevy/Cargo.toml')}; }}",
                )
                if canon_root
                else ()
            ),
            # The repo-local cargo homes on a worker can exist but be empty, which the
            # builder's offline default would turn into "no matching package named libc".
            f"export TSFP_PREBUILT_REMOTE=local TSFP_CLOSE_LOCK_FDS=1 "
            f"TSFP_PREBUILT_ROOT={q(artifact_cache)} "
            f"CARGO_BUILD_JOBS={worker_limit(worker.name, 'jobs', REMOTE_BUILD_JOBS)} "
            "CARGO_NET_OFFLINE=false" + build_slots_export(worker.name),
            build_command,
            sccache_stats,
            git_calls,
            f"peak=$(awk '/Maximum resident set size/ {{print $6}}' {q(time_log)})",
            "printf 'REMOTE_PREBUILT_PEAK_RSS_KB=%s\\n' \"$peak\"",
            f"tail -n 24 {q(log)}",
        )
    )
    if not build_container.enabled(worker.name):
        return script
    # T3737: Ubuntu builders run the same script inside the Debian 13 image (owner PC toolchain).
    return build_container.wrap(script, home=home, paths=[repo_path, canon_root])


BUILD_STAMP_INPUTS = ("src", "crates", "cmake", "CMakeLists.txt", "tools/build_bevy.sh")


def stamp_fingerprint(root: Path, sha: str) -> str | None:
    """The build-input fingerprint `tools/dev/stamp.sh` writes as stamp word 1 for `sha`:
    sha256 of `git ls-files -s` over the build inputs, rebuilt from the commit's tree.
    None when the tree cannot be read."""
    import hashlib

    result = subprocess.run(
        ["git", "ls-tree", "-r", "-z", sha, "--", *BUILD_STAMP_INPUTS],
        cwd=root,
        capture_output=True,
        check=False,
        timeout=60,
    )
    if result.returncode:
        return None
    lines = []
    for entry in result.stdout.split(b"\0"):
        if not entry:
            continue
        meta, _, name = entry.partition(b"\t")
        mode, _kind, oid = meta.split()
        lines.append(mode + b" " + oid + b" 0\t" + name + b"\n")
    return hashlib.sha256(b"".join(lines)).hexdigest()


def commit_lift_key(root: Path, sha: str) -> str | None:
    """The lift key of `sha` as `lift_ship.plan` derives it (commit blobs plus the main
    checkout's gitignored key inputs and XBE digest): the key the shipped lift and the host's
    stamp carry. None when the owner PC cannot name it (the fingerprint still decides).

    I-T3791-1: the owner-side `owner_lift_key()` hashed the RUNNING checkout's working files
    (a worktree lacks the gitignored XDK surface, an older sha has other lifter files), so
    every remote stamp mismatched."""
    try:
        from tools.private_host import main_checkout

        lift = lift_ship.plan(root, sha, main_checkout(root, None))
    except Exception as error:  # noqa: BLE001 - no lift key known here: the fingerprint decides
        print(f"remote build: lift key of {sha[:12]} unknown ({error})", file=sys.stderr)
        return None
    return lift.key if lift is not None else None


def _local_components(root: Path, sha: str, profile: str | None) -> dict[str, object] | None:
    from tools.wt import prebuilt  # lazy: prebuilt imports this module

    try:
        return prebuilt.input_components(root, sha, profile)
    except Exception as error:  # noqa: BLE001 - diagnostics only, the hash check already ran
        print(f"remote build: local components unavailable ({error})", file=sys.stderr)
        return None


def _verify_artifact(
    path: Path,
    *,
    sha: str,
    input_hash: str,
    fingerprint: str | None = None,
    lift_key: str | None = None,
    components: Mapping[str, object] | None = None,
) -> dict:
    """Check a pulled artifact against the commit and its file hashes.

    The build-input hash folds in the builder's tool versions (rustc, cc, cmake, python3) and
    must match: I-T3737-3 reverted T3737's commit-only identity after droplet-built hosts
    (another C compiler) aborted every regress route. Builders need the caller's toolchain.
    """
    try:
        manifest = json.loads((path / "manifest.json").read_text())
    except (OSError, ValueError) as error:
        raise RemotePrebuiltError(f"remote artifact manifest is unreadable: {error}") from error
    if manifest.get("sha") != sha or not isinstance(manifest.get("input_hash"), str):
        raise RemotePrebuiltError("remote artifact commit does not match the requested commit")
    files = manifest.get("files")
    if not isinstance(files, dict) or not files:
        raise RemotePrebuiltError("remote artifact manifest has no file hashes")
    total = 0
    for name, expected in files.items():
        if not isinstance(name, str) or Path(name).name != name or not isinstance(expected, str):
            raise RemotePrebuiltError("remote artifact manifest contains an invalid file entry")
        file_path = path / name
        if not file_path.is_file() or remote_replay.hash_file(file_path) != expected:
            raise RemotePrebuiltError(f"remote artifact SHA-256 mismatch: {name}")
        total += file_path.stat().st_size
    binary = path / "tsfp-bevy"
    stamp = path / ".tsfp-build-stamp"
    if not binary.is_file() or not os.access(binary, os.X_OK):
        raise RemotePrebuiltError("remote artifact has no executable tsfp-bevy")
    stamp_words = stamp.read_text().split() if stamp.is_file() else []
    if len(stamp_words) < 3:
        raise RemotePrebuiltError("remote artifact build stamp is incomplete")
    if fingerprint:
        # T3775: an input-hash reuse (hard-linked from an earlier commit with the same build
        # inputs) keeps the stamp of the commit that built it. What matters is the inputs: the
        # stamp fingerprint must be this commit's, not the sha (I-T3775-5).
        if stamp_words[1] != fingerprint:
            raise RemotePrebuiltError(
                "remote artifact build stamp fingerprint does not match the requested commit"
            )
        if lift_key and lift_key != "unavailable" and stamp_words[2] != lift_key:
            raise RemotePrebuiltError("remote artifact build stamp lift key does not match")
    elif stamp_words[0] != sha:
        raise RemotePrebuiltError(
            "remote artifact build stamp does not identify the requested commit"
        )
    if manifest.get("size_bytes") != total:
        raise RemotePrebuiltError("remote artifact size metadata does not match its files")
    if manifest["input_hash"] != input_hash:
        # I-T3737-3: a build from another toolchain is NOT the same artifact. The droplet-built
        # hosts (different C compiler) accepted this way aborted every regress route at poll 434.
        raise RemotePrebuiltError(
            "remote artifact build-input hash does not match "
            f"(differing components: {differing_components(components, manifest)})"
        )
    return manifest


def differing_components(local: Mapping[str, object] | None, manifest: Mapping) -> str:
    """Name the input-hash components (tree, tools.*, extra.*) the builder computed differently,
    each as `name: local=... remote=...` (I-T3865-10: the message used to blame the toolchain
    whatever differed, and the bare-rustup probe noise that really differed went unseen)."""
    remote = manifest.get("input_components")
    if not isinstance(remote, Mapping):
        return "the builder recorded no components, update its tree"
    if local is None:
        return "this machine could not compute its components"
    names = cache_keys.explain_difference(local, remote)

    def pick(tree: Mapping | object, dotted: str) -> object:
        for part in dotted.split("."):
            tree = tree.get(part) if isinstance(tree, Mapping) else None
        return tree

    shown = [f"{n}: local={pick(local, n)!r} remote={pick(remote, n)!r}" for n in names]
    return "; ".join(shown) or "components equal, the hash model differs"


def ship_lift(
    worker: remote_replay.Worker, root: Path, sha: str, health: remote_replay.Health
) -> str:
    """sub-lift (T3775): ship the commit's private lift through the host's private input cache
    and return the install shell (tests replace this)."""
    layout = {
        key: home_path(value, health.home) for key, value in host_layout_of(worker.name).items()
    }
    staging = remote_replay.staging_root(
        health.home, health.repo_path, layout.get("replay_cache_dir", "")
    )
    try:
        return lift_ship.ship(
            worker,
            root,
            sha,
            repo_path=health.repo_path,
            remote_root=remote_root_of(health.repo_path),
            cache_root=f"{staging}/input-cache",
        )
    except (RuntimeError, OSError, subprocess.SubprocessError) as error:
        raise RemotePrebuiltError(f"could not ship the lift to {worker.name}: {error}") from error


def toolchain_skip(root: Path, worker: remote_replay.Worker) -> str | None:
    """Why `worker` cannot build a matching artifact, else None (and the skip is logged).

    I-T3737-3 / T3737: the build-input hash folds in cc, cmake and python3 versions, so a
    host with another toolchain builds an artifact `_verify_artifact` must reject. Compare
    the host's fingerprint with this machine's first; an unreadable fingerprint does not
    skip (the rejection stays as the backstop).
    """
    local = cache_keys.tool_versions(root, names=remote_toolchain.COMPARED_TOOLS, include_pin=False)
    remote = remote_toolchain.host_fingerprint(worker, root=root)
    if remote is None:
        return None
    difference = remote_toolchain.first_difference(local, remote)
    if difference is not None:
        print(
            f"remote build: skipped {worker.name}: toolchain differs ({difference})",
            file=sys.stderr,
            flush=True,
        )
    return difference


def build_to_staging(
    root: Path,
    *,
    sha: str,
    input_hash: str,
    destination: Path,
    worker_name: str = "droplet",
    profile: str | None = None,
) -> tuple[dict, int | None] | None:
    """Build remotely and copy a verified artifact to an empty local staging directory.

    Returns None when the worker is absent or unhealthy, allowing the caller to use
    the normal local builder. Transport, command, or artifact errors raise so the
    caller can log the failure before falling back.
    """
    if worker_name not in REMOTE_BUILDABLE_WORKERS:
        return None
    worker = workers().get(worker_name)
    if worker is None:
        return None
    if worker_disabled(root, worker.name):
        return None
    health = remote_replay.health(
        worker,
        reserve_mb=worker_limit(worker.name, "reserve_mb", REMOTE_BUILD_RESERVE_MB),
        job_rss_mb=worker_limit(worker.name, "rss_mb", REMOTE_BUILD_RSS_MB),
    )
    if health is None:
        return None
    if _is_local_checkout(root, health.repo_path):
        return None  # running on the worker itself: an ssh to ourselves only adds a hop
    if toolchain_skip(root, worker) is not None:
        return None
    if destination.exists() and any(destination.iterdir()):
        raise RemotePrebuiltError(f"remote staging directory is not empty: {destination}")
    destination.mkdir(parents=True, exist_ok=True)
    remote_root = remote_root_of(health.repo_path)
    try:
        tree = exec_sync.tree_of(root, sha)
    except exec_sync.ExecSyncError as error:
        raise RemotePrebuiltError(f"cannot resolve the tree of {sha}: {error}") from error
    probe = remote_replay._ssh(worker, busy_probe_command(remote_root), timeout=30)
    if "REMOTE_PREBUILT_BUSY" in probe.stdout:
        raise RemoteBusyError(f"remote builder on {worker.name} is busy with another build")
    # T3837: a prebuilt build takes a verify slot of the host, so it also takes its share of the
    # host's ONE budget (verify, build, capture); no room = busy, the caller builds locally.
    budget = host_budget.for_host(worker.name)
    jobs = worker_limit(worker.name, "jobs", REMOTE_BUILD_JOBS)
    cost = host_budget.job_cost("build", cargo_jobs=jobs) if budget else host_budget.Cost(0, 0)
    try:
        with host_budget.lease(worker.name, "build", cost, budget):
            return _build_synced(
                root,
                worker,
                health,
                sha=sha,
                tree=tree,
                input_hash=input_hash,
                destination=destination,
                remote_root=remote_root,
                profile=profile,
            )
    except host_budget.BudgetFull as error:
        raise RemoteBusyError(str(error)) from error


def _build_synced(
    root: Path,
    worker: remote_replay.Worker,
    health: remote_replay.Health,
    *,
    sha: str,
    tree: str,
    input_hash: str,
    destination: Path,
    remote_root: str,
    profile: str | None,
) -> tuple[dict, int | None]:
    """`build_to_staging` after admission: sync, build, pull and verify the artifact."""
    try:  # T3760: the exact tree by rsync delta, no host git
        dest = source_dir_of(worker.name, health.repo_path, sha)
        with contextlib.suppress(OSError, subprocess.SubprocessError):  # retention is best effort
            remote_replay._ssh(
                worker, prune_command(worker.name, health.repo_path, sha), timeout=120
            )
        synced = sync_source(worker, root, tree, sha, dest)
    except (exec_sync.ExecSyncError, OSError, subprocess.SubprocessError) as error:
        raise RemotePrebuiltError(f"could not sync the source to {worker.name}: {error}") from error
    if any(error.startswith("marker") for error in synced.errors):
        raise RemotePrebuiltError(f"source marker on {worker.name} not written: {synced.errors}")
    lift_install = ship_lift(worker, root, sha, health)
    command = _remote_build_command(
        worker,
        repo_path=health.repo_path,
        home=health.home,
        sha=sha,
        input_hash=input_hash,
        tree=tree,
        git_mode=git_compat.mode_for(root, sha),
        lift_install=lift_install,
        profile=profile,
    )

    def retain(result: subprocess.CompletedProcess | subprocess.TimeoutExpired) -> Path:
        return prebuilt_diagnostics.save_failure(
            root,
            worker=worker.name,
            sha=sha,
            profile=profile,
            remote_root=remote_root,
            result=result,
        )

    try:
        result = remote_replay._ssh(worker, command, timeout=BUILD_TIMEOUT_S)
    except subprocess.TimeoutExpired as error:
        diagnostic = retain(error)
        raise RemotePrebuiltError(
            f"remote build on {worker.name} profile={profile} timed out after "
            f"{error.timeout}s; complete returned output and remote log paths: {diagnostic}"
        ) from error
    diagnostic = None
    if result.returncode or "REMOTE_PREBUILT_BUSY" in result.stdout:
        diagnostic = retain(result)
    if result.returncode == host_health.HEALTH_EXIT:
        reasons = ",".join(host_health.refusal_in(result.stdout)) or "unreadable"
        host_health.note_refusal(worker.name)  # T3588: shared back-off, no probe for a while
        raise RemoteBusyError(
            f"{worker.name} refused by its host-health guard: {reasons}; diagnostics: {diagnostic}"
        )
    if result.returncode == REMOTE_BUSY_EXIT or "REMOTE_PREBUILT_BUSY" in result.stdout:
        reason = next(
            (
                line
                for line in result.stdout.splitlines()
                if line.startswith("REMOTE_PREBUILT_BUSY")
            ),
            "busy exit without admission marker",
        )
        raise RemoteBusyError(
            f"{worker.name}: {reason}; profile={profile}; diagnostics: {diagnostic}"
        )
    if result.returncode:
        detail = (result.stdout + "\n" + result.stderr)[-1200:]
        error_type = (
            RemoteBuildFailed
            if "REMOTE_PREBUILT_BUILD_FAILED" in result.stdout
            else RemotePrebuiltError
        )
        raise error_type(
            f"remote build on {worker.name} profile={profile} failed: {detail}; "
            f"complete returned output and remote log paths: {diagnostic}"
        )
    match = re.search(r"REMOTE_PREBUILT_PEAK_RSS_KB=(\d+)", result.stdout)
    peak_rss_kb = int(match.group(1)) if match else None
    stats = re.search(
        r"REMOTE_PREBUILT_SCCACHE_STATS requests=(\d+) hits=(\d+) misses=(\d+)",
        result.stdout,
    )
    from tools.wt import prebuilt

    artifact_name = prebuilt.artifact_dir(root, sha, profile=profile).name
    pull_artifact(worker, f"{remote_root}/artifacts/{artifact_name}", destination)
    manifest = _verify_artifact(
        destination,
        sha=sha,
        input_hash=input_hash,
        fingerprint=stamp_fingerprint(root, sha),
        lift_key=commit_lift_key(root, sha),
        components=_local_components(root, sha, profile),
    )
    manifest["remote_worker"] = worker.name
    manifest["remote_peak_rss_kb"] = peak_rss_kb
    calls = HOST_GIT_CALLS.search(result.stdout)
    manifest["remote_host_git_calls"] = int(calls.group(1)) if calls else None
    if stats:
        manifest["remote_sccache"] = {
            "requests": int(stats.group(1)),
            "hits": int(stats.group(2)),
            "misses": int(stats.group(3)),
        }
    return manifest, peak_rss_kb
