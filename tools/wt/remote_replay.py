"""Remote headless replay workers (T3407).

Only immutable prebuilt hosts and explicit capture inputs are sent to workers. Trace
outputs are copied back; owner files are not mirrored.

T3760 (remote hosts are execution-only): a job's source tree is synced by `exec_sync` into a
stable per-slot exec dir (`remote_exec.replay_exec_dir`, `<canon_root>/replay-<slot>`), the
job holds that dir's lease for its whole run and gets its identity as data
(TSFP_SOURCE_COMMIT / TSFP_SOURCE_TREE). No host git is used.
"""

from __future__ import annotations

import contextlib
import contextvars
import dataclasses
import hashlib
import io
import json
import os
import re
import secrets
import shlex
import shutil
import subprocess
import sys
import tarfile
import tempfile
import threading
import time
from collections.abc import Callable, Mapping
from concurrent.futures import ThreadPoolExecutor
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from tools import owner_files
from tools.fleet import replay_registry
from tools.hostlock import JOB_RSS_MB, MEM_RESERVE_MB
from tools.wt import exec_sync, host_budget, host_health, replay_input_cache

REMOTE_HOST_CAPS = replay_registry.remote_host_caps()
MIN_REMOTE_STAGING_FREE_MB = 12 * 1024
WORKER_MEMORY_MB = replay_registry.worker_memory_mb(JOB_RSS_MB)
"""(reserve, per-job RSS) in MB for workers that cannot meet the 12 GiB local reserve. The
droplet has 15 GB shared with Codex workers (T3462): a 12 GiB reserve left no admissible job.
T3794: the Steam Deck (14 GB, about 11 GB available idle) had the same problem: every capture was
refused as "below resource reserve" (I-T3794-2)."""


REPLAY_DATA_ROOT = "~/rea-codex2/repo"
"""T3760: a replay worker's data root when its record names none: per-job staging and the
private input cache (`<root>/tmp/wt-cache/remote-replay`, capguard evicts it there). Owner
inputs exist on the worker ONLY in that cache and as hard links staged from it (rule 1,
HEALTH owner-file-outside-cache); there is no `<root>/tmp/owner`. Plain directories: replays
never run git there and take their source from the exec dir; stage 5 moves the data root out of
the old repo dir."""
REPLAY_SOURCE_MISSING_EXIT = 70
"""The exec dir's marker does not name the job's tree (remote_exec.STAGE_EXIT)."""
REPLAY_LEASE_EXIT = 75
"""The exec dir's lease could not be taken (a sync or cleaner holds it)."""


@dataclass(frozen=True)
class Worker:
    name: str
    target: str
    port: int
    repo: str
    """The worker's data root (staging, input cache); not a git repo
    for replays (T3760). remote_prebuilt still builds in it until stage 4."""
    max_slots: int


@dataclass(frozen=True)
class Health:
    cores: int
    available_mb: int
    slots: int
    home: str
    repo_path: str
    """The resolved data root of the worker (`Worker.repo`)."""
    available_disk_mb: int = 0


def configured_workers(value: str | None = None) -> list[Worker]:
    """Parse the current replay worker list as a JSON array.

    Fleet runners set ``TSFP_REPLAY_HOSTS_FILE`` to their autoscaler output. Read it at each
    dispatch so a long-lived replay process sees capacity changes without restarting. Explicit
    values and standalone callers continue to use ``TSFP_REPLAY_REMOTE_HOSTS``.
    """
    if value is not None:
        raw = value
    else:
        hosts_file = os.environ.get("TSFP_REPLAY_HOSTS_FILE", "").strip()
        if hosts_file:
            try:
                raw = Path(hosts_file).read_text(encoding="utf-8")
            except FileNotFoundError:
                raw = "[]"
        else:
            raw = os.environ.get("TSFP_REPLAY_REMOTE_HOSTS", "")
    if not raw.strip():
        return []
    try:
        entries = json.loads(raw)
    except json.JSONDecodeError as error:
        raise ValueError(f"TSFP_REPLAY_REMOTE_HOSTS must be a JSON array: {error}") from error
    if not isinstance(entries, list):
        raise ValueError("TSFP_REPLAY_REMOTE_HOSTS must be a JSON array")
    result: list[Worker] = []
    seen: set[str] = set()
    endpoints: set[tuple[str, int]] = set()
    for entry in entries:
        if not isinstance(entry, dict):
            raise ValueError("each TSFP_REPLAY_REMOTE_HOSTS entry must be an object")
        # T3760: `repo` is optional, the data root (REPLAY_DATA_ROOT); the source tree goes
        # to the host's exec dir from tools/data/remote_hosts.json.
        name, target = entry.get("name"), entry.get("target")
        repo = entry.get("repo", REPLAY_DATA_ROOT)
        port, max_slots = entry.get("port", 22), entry.get("max_slots")
        if not all(isinstance(item, str) and item for item in (name, target, repo)):
            raise ValueError("remote worker requires non-empty name, target, and repo strings")
        if name in seen:
            raise ValueError(f"duplicate remote replay worker name: {name}")
        if not isinstance(port, int) or isinstance(port, bool) or not 1 <= port <= 65535:
            raise ValueError(f"invalid SSH port for remote replay worker {name}")
        if not isinstance(max_slots, int) or isinstance(max_slots, bool) or max_slots < 1:
            raise ValueError(f"remote replay worker {name} requires positive max_slots")
        if any(char in target + repo for char in "\n\r\0"):
            raise ValueError(f"invalid newline or NUL in remote replay worker {name}")
        address = target.rsplit("@", 1)[-1]
        endpoint = (address, port)
        if endpoint in endpoints:
            raise ValueError(f"duplicate remote replay worker endpoint: {address}:{port}")
        seen.add(name)
        endpoints.add(endpoint)
        max_slots = min(max_slots, REMOTE_HOST_CAPS.get(address, max_slots))
        result.append(Worker(name, target, port, repo, max_slots))
    off = switched_off()
    kept = [
        worker for worker in result if not ({worker.name, worker.target.rsplit("@", 1)[-1]} & off)
    ]
    return prefer_lan(by_headroom(kept))


def by_headroom(workers: list[Worker], root: Path | None = None) -> list[Worker]:
    """T3748: least loaded per core first (tools/fleet/headroom.py cache); prefer_lan then keeps
    LAN hosts ahead of WAN ones as a hard rule (stable sort)."""
    from tools.fleet.config import main_checkout
    from tools.fleet.headroom import order_names

    order = order_names([worker.name for worker in workers], root or main_checkout())
    return sorted(workers, key=lambda worker: order.index(worker.name))


def lan_addresses() -> set[str]:
    """Addresses of remote hosts marked `"lan": true` in tools/data/remote_hosts.json."""
    path = Path(__file__).resolve().parents[1] / "data" / "remote_hosts.json"
    try:
        hosts = json.loads(path.read_text()).get("hosts", [])
    except (OSError, ValueError):
        return set()
    return {
        address
        for host in hosts
        if host.get("lan") is True
        for address in (host.get("address"), host.get("replay_target"))
        if address
    }


def prefer_lan(workers: list[Worker], lan: set[str] | None = None) -> list[Worker]:
    """Order workers LAN hosts first (no WAN uplink), keeping the given order inside each group.

    The queue tries workers in order and skips one without a free slot or health, so a droplet
    is used only when every LAN host is busy or down (T3678).
    """
    lan = lan_addresses() if lan is None else lan
    return sorted(workers, key=lambda worker: worker.target.rsplit("@", 1)[-1] not in lan)


def switched_off() -> set[str]:
    """Names and addresses of fleet hosts whose `off_file` exists (I-T3407-1, e.g. laptop.off).

    Read on every call so a host handed back to the owner stops receiving replays at once, even
    when TSFP_REPLAY_REMOTE_HOSTS was baked into a long-running worker's environment.
    """
    from tools.fleet.config import ConfigError, load_config
    from tools.wt import retired_hosts

    # T3753: retired hosts (the deleted droplets) stay off even if a worker's environment baked them
    off: set[str] = set(retired_hosts.retired_identities())
    try:
        config = load_config()
    except ConfigError:
        return off
    for host in config.remote_hosts():
        if host.off_file and (config.codexapi_dir / host.off_file).exists():
            off |= {
                name
                for name in (host.name, host.replay_name, host.address, host.replay_target)
                if name
            }
    return off


def staging_root(home: str, repo_path: str, cache_dir: str = "") -> str:
    """Per-job staging parent on the worker: TSFP_REMOTE_STAGING_ROOT, else the registry
    `replay_cache_dir` (cx-execlayout), else the data root's tmp/wt-cache.

    T3462: staging used to live in `~/.cache/tsfp-replay-remote`. On the laptop `$HOME` is the
    host's BTRFS overlay while the repo is on the capped /workspace volume, and each job stages
    the 4 GB disc plus a 640 MB source archive: failed or interrupted jobs filled the overlay
    with 57 GB.
    """
    configured = os.environ.get("TSFP_REMOTE_STAGING_ROOT", "").strip() or cache_dir.strip()
    if configured:
        # T3794 (I-T3794-3): every remote path is shell-quoted, so `~` was never expanded: the
        # Deck staged under a literal `$HOME/~/...` and the capture's `cd` to the exec dir broke
        # its log redirect (exit 1, no output). Expand against the probed home, refuse relative.
        if configured == "~" or configured.startswith("~/"):
            configured = home.rstrip("/") + configured[1:]
        if not configured.startswith("/"):
            raise ValueError(f"remote staging root must be absolute: {configured!r}")
        return configured.rstrip("/")
    return (
        f"{repo_path.rstrip('/')}/tmp/wt-cache/remote-replay"
        if repo_path
        else (str(Path(home) / ".cache" / "tsfp-replay-remote"))
    )


def owner_files_dir(root: Path) -> Path:
    """Find the private owner-data directory, shared by a linked worktree's main checkout."""
    return owner_files.resolve_path(root, "tmp/owner", kind="dir")


def calculate_slots(
    cores: int,
    available_mb: int,
    max_slots: int,
    reserve_mb: int = MEM_RESERVE_MB,
    job_rss_mb: int = JOB_RSS_MB,
) -> int:
    """Bound configured capacity by one slot per four cores and free-memory headroom."""
    if min(cores, available_mb, max_slots, job_rss_mb) < 1 or reserve_mb < 0:
        return 0
    memory_slots = max(0, available_mb - reserve_mb) // job_rss_mb
    return min(max_slots, max(1, cores // 4), memory_slots)


_CONTROL_PATH: contextvars.ContextVar[str | None] = contextvars.ContextVar(
    "tsfp_replay_ssh_control_path", default=None
)
CONTROL_PERSIST_S = 60
"""One ssh ControlMaster per replay job (the handshake costs 2.5 s per connection on the WAN)."""
STREAMS = 4
"""Concurrent rsync streams per job: one WAN stream is latency-bound (8.5 MB/s), four 29 MB/s."""
_STREAM_SLOTS = threading.BoundedSemaphore(STREAMS)
_stream_stats = {"active": 0, "peak": 0}
_stream_lock = threading.Lock()
_cache_probe_lock = threading.Lock()
_cache_probe_results: dict[tuple[str, str], bool] = {}


_CONTROL_UP: contextvars.ContextVar[bool] = contextvars.ContextVar(
    "tsfp_replay_ssh_control_up", default=False
)


def _control_options() -> list[str]:
    """I-T3678: calls only ever USE a master (ControlMaster=no); `open_control_master` starts it.

    A captured `ControlMaster=auto` call can become the master and hold its pipe until
    ControlPersist expires. With no live master, calls go direct (no ControlPath)."""
    path = _CONTROL_PATH.get()
    if not path or not _CONTROL_UP.get():
        return ["-o", "ControlMaster=no"] if path else []
    return ["-o", "ControlMaster=no", "-o", f"ControlPath={path}"]


def _ssh_transport(worker: Worker) -> str:
    """The `ssh ...` command rsync runs: same options as `_ssh`, so it shares the master."""
    return shlex.join(
        ["ssh", "-p", str(worker.port), "-o", "BatchMode=yes", "-o", "ConnectTimeout=5"]
        + _control_options()
    )


def master_argv(worker: Worker, path: str) -> list[str]:
    """Explicit master start: backgrounded after auth, owns no pipe of the caller."""
    return [
        "ssh",
        "-fN",
        "-p",
        str(worker.port),
        "-o",
        "BatchMode=yes",
        "-o",
        "ConnectTimeout=5",
        "-o",
        "ControlMaster=yes",
        "-o",
        f"ControlPath={path}",
        "-o",
        f"ControlPersist={CONTROL_PERSIST_S}",
        worker.target,
    ]


def open_control_master(worker: Worker) -> None:
    """Start the job's master before parallel staging (failure is not fatal: calls go direct)."""
    path = _CONTROL_PATH.get()
    if not path:
        return
    with contextlib.suppress(Exception):
        started = subprocess.run(
            master_argv(worker, path),
            stdin=subprocess.DEVNULL,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            timeout=30,
            check=False,
        )
        _CONTROL_UP.set(started.returncode == 0)


def close_control_master(worker: Worker) -> None:
    """`ssh -O exit` on the job's master; safe when none is running."""
    path = _CONTROL_PATH.get()
    if not path:
        return
    with contextlib.suppress(Exception):
        subprocess.run(
            [
                "ssh",
                "-p",
                str(worker.port),
                "-o",
                f"ControlPath={path}",
                "-O",
                "exit",
                worker.target,
            ],
            text=True,
            capture_output=True,
            timeout=10,
            check=False,
        )


def ssh_argv(worker: Worker, remote_command: str) -> list[str]:
    return [
        "ssh",
        "-p",
        str(worker.port),
        "-o",
        "BatchMode=yes",
        "-o",
        "ConnectTimeout=5",
        *_control_options(),
        worker.target,
        remote_command,
    ]


def _ssh(
    worker: Worker, remote_command: str, timeout: int = 15
) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ssh_argv(worker, remote_command),
        text=True,
        capture_output=True,
        timeout=timeout,
        check=False,
    )


def _repo_cd(repo: str) -> str:
    # Preserve shell expansion for the documented ~/repo form.
    if repo.startswith("~/"):
        return '"$HOME"/' + shlex.quote(repo[2:])
    return shlex.quote(repo)


def memory_model(worker: Worker) -> tuple[int, int]:
    """(reserve_mb, job_rss_mb) admission model for a worker."""
    return WORKER_MEMORY_MB.get(worker.name, (MEM_RESERVE_MB, JOB_RSS_MB))


def health(
    worker: Worker,
    reserve_mb: int | None = None,
    job_rss_mb: int | None = None,
    expected_commit: str | None = None,
) -> Health | None:
    """Probe SSH, Linux CPU/memory, and the worker's data root in one round trip.

    T3760: no host git (the data root is created when missing; the source comes by exec_sync).
    T3588: a host that refused its host-health guard is not probed again for the shared
    back-off (host_health.backoff_remaining); it answers None without any ssh."""
    guard = host_health.facts_for_host(worker.name)[0]
    if guard is not None and host_health.backoff_remaining(worker.name) > 0:
        return None
    data_root = _repo_cd(worker.repo)
    command = (
        f"mkdir -p {data_root} && cd {data_root} && "
        "command -v rsync >/dev/null && command -v flock >/dev/null && "
        "printf '%s\\n' \"$(nproc)\" && "
        "awk '/^MemAvailable:/ {print int($2/1024)}' /proc/meminfo && "
        'printf "%s\\n%s\\n" "$HOME" "$(pwd -P)" && '
        "cut -d' ' -f1 /proc/loadavg && "  # T3516: host load for the host-health guard
        "df -Pm . | awk 'NR==2 {print $4}'"
    )
    try:
        result = _ssh(worker, command)
        if result.returncode:
            return None
        fields = result.stdout.splitlines()
        cores, available = int(fields[0]), int(fields[1])
        home, repo_path = fields[2], fields[3]
        available_disk_mb = int(fields[5])
        if not home.startswith("/") or not repo_path.startswith("/"):
            return None
        # Remote replay stages a source snapshot, host, route and often a multi-GB disc
        # under the repository cache. Leave enough room for cache and per-run copies before
        # trying a worker; ENOSPC was otherwise discovered after serial input staging.
        disk_reserve_mb = staging_disk_reserve_mb(worker)
        if available_disk_mb < disk_reserve_mb:
            return None
    except (OSError, ValueError, IndexError, subprocess.TimeoutExpired):
        return None
    if guard is not None:  # T3516: replays and remote prebuilt builds respect the host guard too
        try:
            load1 = float(fields[4])
        except (IndexError, ValueError):
            load1 = None
        if host_health.evaluate(guard, mem_mb=available, load1=load1, cores=cores, builds=None):
            host_health.note_refusal(worker.name)
            return None
        host_health.note_admission(worker.name)
    default_reserve, default_job = memory_model(worker)
    slots = calculate_slots(
        cores,
        available,
        worker.max_slots,
        default_reserve if reserve_mb is None else reserve_mb,
        default_job if job_rss_mb is None else job_rss_mb,
    )
    return Health(cores, available, slots, home, repo_path, available_disk_mb) if slots else None


def hash_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def hash_tree(path: Path) -> str:
    digest = hashlib.sha256()
    for item in sorted(path.rglob("*")):
        if item.is_file():
            digest.update(item.relative_to(path).as_posix().encode())
            digest.update(bytes.fromhex(hash_file(item)))
    return digest.hexdigest()


def transfer_timeout(source: Path, floor_s: int = 300, min_bytes_per_s: int = 1_500_000) -> int:
    """Seconds allowed for one rsync: 300 s plus the time the payload needs at 1.5 MB/s.

    T3462: the fixed 300 s killed the 4 GB disc transfer to the droplet (about 7 MB/s over the
    WAN, 570 s) and the job silently fell back to the local queue.
    """
    try:
        if source.is_dir():
            size = sum(item.stat().st_size for item in source.rglob("*") if item.is_file())
        else:
            size = source.stat().st_size
    except OSError:
        size = 0
    return floor_s + size // min_bytes_per_s


def _rsync(
    worker: Worker,
    source: Path,
    destination: str,
    *,
    directory: bool = False,
    timeout: int | None = None,
) -> int | None:
    """Copy `source` to the worker; return the bytes rsync sent (from --stats) when known."""
    ssh_transport = _ssh_transport(worker)
    source_arg = str(source) + ("/" if directory else "")
    with _STREAM_SLOTS:
        with _stream_lock:
            _stream_stats["active"] += 1
            _stream_stats["peak"] = max(_stream_stats["peak"], _stream_stats["active"])
        try:
            result = _run_rsync(worker, source, source_arg, destination, ssh_transport, timeout)
        finally:
            with _stream_lock:
                _stream_stats["active"] -= 1
    if result.returncode:
        raise RuntimeError(f"rsync to {worker.name} failed: {result.stderr[-500:]}")
    return _bytes_sent(getattr(result, "stdout", None))


def _run_rsync(
    worker: Worker,
    source: Path,
    source_arg: str,
    destination: str,
    ssh_transport: str,
    timeout: int | None,
) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [
            "rsync",
            "-a",
            "--protect-args",
            "--partial",
            "--stats",
            "-e",
            ssh_transport,
            source_arg,
            f"{worker.target}:{destination}",
        ],
        text=True,
        capture_output=True,
        timeout=timeout or transfer_timeout(source),
        check=False,
    )


def _bytes_sent(stats: object) -> int | None:
    """`Total bytes sent` from `rsync --stats` output, or None when absent."""
    match = re.search(r"Total bytes sent:\s*([\d,]+)", stats) if isinstance(stats, str) else None
    return int(match.group(1).replace(",", "")) if match else None


def verify_timeout(source: Path) -> int:
    """Seconds for the remote sha256sum of a copied file: 60 s plus the payload at 5 MB/s.

    T3472 phase 3: the fixed 15 s of `_ssh` expired on a loaded Deck hashing a 600 MB tree
    archive and the TimeoutExpired escaped as a failed check."""
    try:
        size = source.stat().st_size
    except OSError:
        size = 0
    return 60 + size // 5_000_000


def _verify_remote(
    worker: Worker,
    relative_path: str,
    expected_hash: str,
    *,
    directory: bool = False,
    timeout: int = 15,
) -> None:
    if directory:
        # Directory content is copied below a dedicated path; check every file against a
        # local manifest rather than relying on rsync's transport checksum alone.
        raise NotImplementedError("directory manifests are verified by verify_tree")
    # Callers pass absolute paths (T3760: no cd into the worker's data root).
    command = (
        f"test \"$(sha256sum {_remote_path(relative_path)} | cut -d' ' -f1)\" = "
        f"{shlex.quote(expected_hash)}"
    )
    result = _ssh(worker, command, timeout=timeout)
    if result.returncode:
        raise RuntimeError(f"SHA-256 verification failed for {relative_path} on {worker.name}")


def _remote_path(path: str) -> str:
    return shlex.quote(path)


def _remove_remote(worker: Worker, *paths: str) -> None:
    """Delete staging paths on the worker without ever failing the job (T3794, I-T3794-4).

    A finished 20-minute capture was reported failed because `rm -rf` of its multi-GB staging dir
    outlasted the 15 s ssh timeout on a loaded laptop. Each path is renamed aside (instant on one
    filesystem) and removed by a detached background `rm`; a slow or failed cleanup is logged
    and left to the remote staging GC."""
    quoted = " ".join(_remote_path(path) for path in paths)
    command = (
        f'for p in {quoted}; do [ -e "$p" ] || continue; t="$p.trash.$$"; '
        'mv "$p" "$t" 2>/dev/null || t="$p"; '
        'nohup rm -rf "$t" >/dev/null 2>&1 </dev/null & done'
    )
    try:
        _ssh(worker, command, timeout=30)
    except (OSError, subprocess.TimeoutExpired) as exc:
        print(
            f"remote_replay: cleanup of {quoted} on {worker.name} deferred: {exc}", file=sys.stderr
        )


def verify_tree(worker: Worker, remote_root: str, local_root: Path) -> None:
    entries = [item for item in sorted(local_root.rglob("*")) if item.is_file()]
    for item in entries:
        relative = f"{remote_root}/{item.relative_to(local_root).as_posix()}"
        _verify_remote(worker, relative, hash_file(item))


def _copy_input(
    worker: Worker,
    source: Path,
    dest_rel: str,
    *,
    directory: bool = False,
    timeout: int | None = None,
) -> int | None:
    parent = str(Path(dest_rel).parent).replace("\\", "/")
    prepared = _ssh(worker, f"mkdir -p {_remote_path(parent)} && test -d {_remote_path(parent)}")
    if prepared.returncode:
        raise RuntimeError(
            f"could not prepare destination directory on {worker.name}: {prepared.stderr[-500:]}"
        )
    # rsync receives argv directly here; shell quotes would become literal filename
    # characters on the remote side. --protect-args carries paths with spaces safely.
    sent = _rsync(worker, source, dest_rel, directory=directory, timeout=timeout)
    if directory:
        verify_tree(worker, dest_rel, source)
    else:
        _verify_remote(worker, dest_rel, hash_file(source), timeout=verify_timeout(source))
    return sent


def input_cache_cap_gb(worker: Worker) -> float:
    """Per-host input cache cap: `replay_input_cache_gb` of the matching remote host data."""
    address = worker.target.rsplit("@", 1)[-1]
    path = Path(__file__).resolve().parents[1] / "data" / "remote_hosts.json"
    try:
        hosts = json.loads(path.read_text()).get("hosts", [])
    except (OSError, ValueError):
        hosts = []
    entry = next((h for h in hosts if address in (h.get("address"), h.get("replay_target"))), None)
    return replay_input_cache.cap_gb_for(entry)


def staging_disk_reserve_mb(worker: Worker) -> int:
    """Honor the fleet's per-host free-disk floor before admitting replay staging."""
    address = worker.target.rsplit("@", 1)[-1]
    path = Path(__file__).resolve().parents[1] / "data" / "remote_hosts.json"
    try:
        hosts = json.loads(path.read_text()).get("hosts", [])
    except (OSError, ValueError):
        hosts = []
    entry = next(
        (
            host
            for host in hosts
            if worker.name in (host.get("name"), host.get("replay_name"))
            or address in (host.get("address"), host.get("replay_target"))
        ),
        None,
    )
    floor_gb = entry.get("disk_floor_gb") if isinstance(entry, dict) else None
    try:
        configured_floor = max(0, int(float(floor_gb) * 1024))
    except (TypeError, ValueError):
        configured_floor = 0
    return max(MIN_REMOTE_STAGING_FREE_MB, configured_floor)


def evict_input_cache(worker: Worker, source_dir: str, cache_root: str) -> None:
    """Enforce the LRU cap on the worker before staging; failure must not block the replay.

    T3760: runs the evictor from the job's synced exec dir, not from a host repo."""
    command = replay_input_cache.evict_command(
        _remote_path(source_dir), cache_root, input_cache_cap_gb(worker)
    )
    _ssh(worker, command, timeout=300)


def pack_tree(source: Path, archive: Path) -> str:
    """Pack a directory into a deterministic tar and return its SHA-256 (the tree identity).

    Entries are sorted, with fixed mtime, owner and group, and the file mode kept, so equal
    trees give equal bytes on any machine and a changed file, name or mode changes the digest.
    """
    with tarfile.open(archive, "w", format=tarfile.PAX_FORMAT) as bundle:
        for item in sorted(source.rglob("*")):
            info = bundle.gettarinfo(str(item), arcname=item.relative_to(source).as_posix())
            info.mtime, info.uid, info.gid, info.uname, info.gname = 0, 0, 0, "", ""
            if item.is_file():
                with item.open("rb") as stream:
                    bundle.addfile(info, stream)
            else:
                bundle.addfile(info)
    return hash_file(archive)


def newest_cached_basis(worker: Worker, cache_root: str, name: str) -> str | None:
    """Newest cached entry with the same file name: the rsync delta basis for a changed build."""
    found = _ssh(
        worker,
        f"find {_remote_path(cache_root)} -mindepth 2 -maxdepth 2 -type f "
        f"-name {shlex.quote('*-' + name)} -printf '%T@ %p\\n' 2>/dev/null | sort -rn | head -n 1",
        timeout=60,
    )
    line = (found.stdout or "").strip() if found.returncode == 0 else ""
    return line.split(" ", 1)[1] if " " in line else None


def _prune_input_cache(worker: Worker, cache_root: str) -> None:
    # Rebuildable artifacts follow the wt-cache three-day retention window. Pruning is
    # restricted to this dedicated content-addressed cache and never touches run inputs.
    pruned = _ssh(worker, f"find {_remote_path(cache_root)} -type f -mmin +4320 -delete")
    if pruned.returncode:
        raise RuntimeError(f"could not prune replay input cache on {worker.name}")


def _record(report: list[dict[str, Any]] | None, label: str, **fields: Any) -> None:
    if report is not None:
        report.append({"input": label, **fields})


def _copy_cached_input(
    worker: Worker,
    source: Path,
    dest_rel: str,
    cache_root: str,
    *,
    report: list[dict[str, Any]] | None = None,
    label: str | None = None,
    delta: bool = False,
    private: bool = False,
    prune: bool = True,
    cache_hit: bool | None = None,
    link: bool = False,
) -> bool:
    """Reuse a verified immutable artifact on the worker; return whether it hit.

    A miss is promoted into the content-addressed cache (`<cache>/<2>/<sha256>-<name>`). With
    `delta`, the newest cached file of the same name is copied to the incoming path first so
    rsync sends only the changed blocks (no --inplace). `private` entries (owner inputs) are
    mode 0600 in a 0700 directory. `report` collects hit/miss and bytes sent per input. With
    `link` the staged path is a HARD LINK of the cache entry (never a second copy of a 4 GB disc,
    rule 1); it falls back to a reflink copy across filesystems.
    """
    digest = hash_file(source)
    cache_rel = f"{cache_root.rstrip('/')}/{digest[:2]}/{digest}-{source.name}"
    cache_parent = str(Path(cache_rel).parent).replace("\\", "/")
    if cache_hit is None:
        with _cache_probe_lock:
            cache_hit = _cache_probe_results.pop((worker.target, cache_rel), None)
    if cache_hit is None:
        checked = _ssh(
            worker,
            f"test -f {_remote_path(cache_rel)} && "
            f"test \"$(sha256sum {_remote_path(cache_rel)} | cut -d' ' -f1)\" = {digest} && "
            f"touch {_remote_path(cache_rel)}",
            timeout=300,
        )
        cache_hit = checked.returncode == 0
    sent: int | None = None
    basis: str | None = None
    if not cache_hit:
        incoming_rel = f"{cache_root.rstrip('/')}/incoming/{secrets.token_hex(8)}-{source.name}"
        if delta:
            basis = newest_cached_basis(worker, cache_root, source.name)
            if basis:
                _ssh(
                    worker,
                    f"mkdir -p {_remote_path(str(Path(incoming_rel).parent))} && "
                    f"cp --reflink=auto {_remote_path(basis)} {_remote_path(incoming_rel)}",
                    timeout=600,
                )
        sent = _copy_input(worker, source, incoming_rel)
        privacy = (
            f"chmod 700 {_remote_path(cache_parent)} && chmod 600 {_remote_path(incoming_rel)} && "
            if private
            else ""
        )
        promoted = _ssh(
            worker,
            f"mkdir -p {_remote_path(cache_parent)} && {privacy}"
            f"mv -f {_remote_path(incoming_rel)} {_remote_path(cache_rel)} && "
            f"touch {_remote_path(cache_rel)}",
        )
        if promoted.returncode:
            raise RuntimeError(f"could not promote cached replay input on {worker.name}")
    destination_parent = str(Path(dest_rel).parent).replace("\\", "/")
    materialized = _ssh(
        worker,
        f"mkdir -p {_remote_path(destination_parent)} && "
        + replay_input_cache.use_lock_command(
            cache_rel,
            (
                f"rm -f {_remote_path(dest_rel)} && ln {_remote_path(cache_rel)} "
                f"{_remote_path(dest_rel)} || "
                if link
                else ""
            )
            + f"cp --reflink=auto --preserve=mode,timestamps "
            f"{_remote_path(cache_rel)} {_remote_path(dest_rel)}",
        ),
        timeout=verify_timeout(source),
    )
    if materialized.returncode:
        raise RuntimeError(f"could not materialize cached replay input on {worker.name}")
    if prune:
        _prune_input_cache(worker, cache_root)
    _record(
        report,
        label or source.name,
        cache="hit" if cache_hit else "miss",
        bytes_sent=sent,
        delta_basis=bool(basis),
    )
    return cache_hit


def _copy_cached_tree(
    worker: Worker,
    source: Path,
    dest_rel: str,
    cache_root: str,
    *,
    report: list[dict[str, Any]] | None = None,
    label: str | None = None,
    prune: bool = True,
) -> bool:
    """Cache a private directory (saves) by the SHA-256 of its deterministic tar; unpack it."""
    archive_rel = f"{dest_rel.rstrip('/')}.tree.tar"
    with tempfile.TemporaryDirectory(prefix="tsfp-remote-tree-") as temp_dir:
        archive = Path(temp_dir) / f"{source.name}.tree.tar"
        pack_tree(source, archive)
        hit = _copy_cached_input(
            worker,
            archive,
            archive_rel,
            cache_root,
            report=report,
            label=label or source.name,
            private=True,
            prune=prune,
        )
    unpacked = _ssh(
        worker,
        f"mkdir -p {_remote_path(dest_rel)} && "
        f"tar -xpf {_remote_path(archive_rel)} -C {_remote_path(dest_rel)} && "
        f"rm -f {_remote_path(archive_rel)}",
        timeout=300,
    )
    if unpacked.returncode:
        raise RuntimeError(f"could not unpack cached tree {source.name} on {worker.name}")
    return hit


def _copy_owner_input(
    worker: Worker,
    source: Path,
    dest_rel: str,
    cache_root: str | None,
    *,
    report: list[dict[str, Any]] | None = None,
    label: str | None = None,
) -> None:
    """Stage a private file or directory: through the content cache when one is given."""
    if cache_root is None:
        _copy_input(worker, source, dest_rel, directory=source.is_dir())
    elif source.is_dir():
        _copy_cached_tree(
            worker, source, dest_rel, cache_root, report=report, label=label, prune=False
        )
    else:
        _copy_cached_input(
            worker,
            source,
            dest_rel,
            cache_root,
            report=report,
            label=label,
            private=True,
            prune=False,
            link=True,
        )


def _run_parallel(tasks: list[Callable[[], Any]]) -> list[Any]:
    """Run staging tasks on threads (each keeps the job's ssh master); raise the first error.

    Concurrent rsync streams are bounded by `_STREAMS`, not by the pool size.
    """
    if len(tasks) == 1:
        return [tasks[0]()]
    with ThreadPoolExecutor(max_workers=len(tasks)) as pool:
        futures = [pool.submit(contextvars.copy_context().run, task) for task in tasks]
        outcomes = [future.exception() for future in futures]
    for error in outcomes:
        if error is not None:
            raise error
    return [future.result() for future in futures]


def host_libraries(host: Path) -> tuple[Path, ...]:
    """The host's libtsfp_* runtime libraries: its neighbours plus every one `ldd` resolves.

    T3740: a local TSFP_BUILD_CAPTURE=1 host keeps libtsfp_kernel_clock_capi.so in its build
    dir (RUNPATH), not next to the binary; staging only neighbours left the worker unable to
    load it."""
    found = {lib.name: lib for lib in host.parent.glob("libtsfp_*.so")}
    try:
        listed = subprocess.run(
            ["ldd", str(host)], capture_output=True, text=True, timeout=60, check=False
        ).stdout
    except (OSError, subprocess.TimeoutExpired):
        listed = ""
    for line in listed.splitlines():
        name, arrow, rest = line.strip().partition(" => ")
        if arrow and name.startswith("libtsfp_") and rest.startswith("/"):
            found.setdefault(name, Path(rest.split(" (")[0]))
    return tuple(sorted(found.values(), key=lambda lib: lib.name))


def _stage_host(
    worker: Worker,
    host: Path,
    host_rel: str,
    *,
    cache_root: str | None = None,
    report: list[dict[str, Any]] | None = None,
) -> tuple[Path, ...]:
    """Copy a prebuilt host and its runtime libraries by hash, in parallel, delta-sent."""
    libraries = host_libraries(host)
    host_directory = str(Path(host_rel).parent).replace("\\", "/")

    def copy_input(source: Path, dest: str) -> None:
        if cache_root is None:
            _copy_input(worker, source, dest)
        else:
            _copy_cached_input(
                worker, source, dest, cache_root, report=report, delta=True, prune=False
            )

    _run_parallel(
        [
            lambda source=source, dest=dest: copy_input(source, dest)
            for source, dest in (
                (host, host_rel),
                *((lib, f"{host_directory}/{lib.name}") for lib in libraries),
            )
        ]
    )
    return libraries


def _batch_cached_input_checks(
    worker: Worker, sources: tuple[Path, ...], cache_root: str
) -> dict[Path, bool]:
    """Verify and LRU-touch cache entries in one SSH round trip."""
    entries = []
    commands = []
    for index, source in enumerate(sources):
        digest = hash_file(source)
        cache_rel = f"{cache_root.rstrip('/')}/{digest[:2]}/{digest}-{source.name}"
        path = _remote_path(cache_rel)
        commands.append(
            f"if test -f {path} && test \"$(sha256sum {path} | cut -d' ' -f1)\" = {digest}; "
            f"then touch {path} && echo {index}:hit; else echo {index}:miss; fi"
        )
        entries.append((source, cache_rel))
    checked = _ssh(worker, "; ".join(commands), timeout=300)
    if checked.returncode:
        raise RuntimeError(f"batched replay cache check failed on {worker.name}")
    outcomes: dict[int, bool] = {}
    for line in checked.stdout.splitlines():
        index, separator, outcome = line.partition(":")
        if separator and index.isdigit() and int(index) < len(entries):
            outcomes[int(index)] = outcome == "hit"
    if len(outcomes) != len(entries):
        raise RuntimeError(f"incomplete batched replay cache check on {worker.name}")
    result = {source: outcomes[index] for index, (source, _) in enumerate(entries)}
    with _cache_probe_lock:
        for index, (_, cache_rel) in enumerate(entries):
            _cache_probe_results[(worker.target, cache_rel)] = outcomes[index]
    return result


def _forget_cached_input_checks(worker: Worker) -> None:
    with _cache_probe_lock:
        for key in [key for key in _cache_probe_results if key[0] == worker.target]:
            _cache_probe_results.pop(key, None)


def _stage_default_xbe(
    worker: Worker,
    xbe: Path,
    source_root: str,
    *,
    cache_root: str | None = None,
    report: list[dict[str, Any]] | None = None,
) -> str:
    """Stage the private runtime XBE where rust_trace resolves build/default.xbe."""
    destination = f"{source_root}/build/default.xbe"
    _copy_owner_input(
        worker, xbe.resolve(strict=True), destination, cache_root, report=report, label="xbe"
    )
    return destination


def _stage_route_saves(
    worker: Worker,
    route: Path,
    route_rel: str,
    private_root: Path,
    *,
    cache_root: str | None = None,
    report: list[dict[str, Any]] | None = None,
) -> str | None:
    """Stage a recording's hash-stamped HDD snapshot directory beside its remote copy."""
    from tools.play import rec_saves

    stamped = rec_saves.header_saves(route)
    if stamped is None:
        return None
    source = _private_owner_input(rec_saves.locate(route, stamped[0]), private_root)
    destination = f"{Path(route_rel).parent.as_posix()}/{source.name}"
    _copy_owner_input(worker, source, destination, cache_root, report=report, label="route-saves")
    return destination


def _use_staged_host(args: list[str]) -> list[str]:
    """Use an explicitly staged host instead of resolving a worker cache manifest."""
    values = list(args)
    index = 0
    while index < len(values):
        if values[index] == "--host-input-hash":
            del values[index : index + 2]
            continue
        index += 1
    return values


_RESULT_REQUIRED_SUFFIXES = (".json", ".trace")


def result_file_is_required(relative_path: str) -> bool:
    """Whether a capture artifact is consumed by receipts or trace comparisons.

    Gate comparisons use trace.json plus the call-args streams; receipt reconstruction uses
    the JSON sidecars (including digests and capture timing). Raw logs and intermediates stay
    on the worker and can be fetched explicitly with ``fetch_results``.
    """
    name = Path(relative_path).name
    return name.endswith(_RESULT_REQUIRED_SUFFIXES)


def _pull_results(worker: Worker, source_rel: str, destination: Path) -> None:
    """Stream only receipt/comparison artifacts through zstd; raw outputs remain remote."""
    destination.mkdir(parents=True, exist_ok=True)
    extracted = subprocess.run(
        [
            "rsync",
            "-a",
            "--compress-choice=zstd",
            "--compress-level=3",
            "--protect-args",
            "--partial",
            "--include=*/",
            "--include=*.json",
            "--include=*.trace",
            # T3818: --shot-at frames of a remote visual capture (small PNGs)
            "--include=shot-*.png",
            "--include=shot-*.error",
            "--exclude=*",
            "-e",
            _ssh_transport(worker),
            f"{worker.target}:{source_rel}/",
            str(destination) + "/",
        ],
        text=True,
        capture_output=True,
        timeout=1800,
        check=False,
    )
    if extracted.returncode:
        raise RuntimeError(
            f"trace result transfer from {worker.name} failed: {extracted.stderr[-500:]}"
        )


RESULT_LOGS = ("run.log", "host.out", "host.err", "stop.txt", "route.log")
"""T3760 (REA 2026-10-26): the logs regress classifies faults from; always pulled back, the
`.json`/`.trace` filter of `_pull_results` dropped them and receipts had run_log_sha256=null."""
RESULT_LOG_CAP_BYTES = 8 * 1024 * 1024
"""Per log: a bigger one comes back as its last RESULT_LOG_CAP_BYTES behind a one-line note."""
_LOG_SUMS = ".tsfp-log-sums"


def logs_command(out_rel: str, cap: int | None = None) -> str:
    """Remote shell: a tar on stdout of every RESULT_LOGS file up to two levels under
    `out_rel`, each capped to its tail, plus `.tsfp-log-sums` (path, sha256 and size of the
    original file). An absent dir or log is not an error."""
    cap = RESULT_LOG_CAP_BYTES if cap is None else cap
    names = " -o ".join(f"-name {shlex.quote(name)}" for name in RESULT_LOGS)
    note = shlex.quote(f"[T3760: %s was %s bytes; only the last {cap} bytes were pulled]\n")
    sums = shlex.quote(_LOG_SUMS)
    line = shlex.quote("%s %s %s\n")
    body = "\n".join(
        (
            f"cd {_remote_path(out_rel)} 2>/dev/null || exit 0",
            "T=$(mktemp -d) || exit 1",
            """trap 'rm -rf "$T"' EXIT""",
            f': > "$T"/{sums}',
            rf"find . -maxdepth 2 -type f \( {names} \) | while IFS= read -r f; do",
            '  mkdir -p "$T/$(dirname "$f")"; s=$(stat -c %s "$f")',
            f'  if [ "$s" -gt {cap} ]; then',
            f'    {{ printf {note} "$f" "$s"; tail -c {cap} "$f"; }} > "$T/$f"',
            '  else cp "$f" "$T/$f"; fi',
            f'  printf {line} "$(sha256sum < "$f" | cut -d" " -f1)" "$s" "$f" >> "$T"/{sums}',
            "done",
            'tar -C "$T" -cf - .',
        )
    )
    return f"bash -c {shlex.quote(body)}"


def _pull_logs(
    worker: Worker,
    out_rel: str,
    destination: Path,
    *,
    run: Callable[..., subprocess.CompletedProcess] = subprocess.run,
) -> dict[str, dict[str, Any]]:
    """Pull the run's logs (capped) into `destination`; returns {relative path: {sha256 of
    the full remote file, bytes, truncated}} for the receipt. Never raises: a missing log is
    recorded as absent by the caller's classification, not as a transport failure."""
    destination.mkdir(parents=True, exist_ok=True)
    try:
        pulled = run(
            ssh_argv(worker, logs_command(out_rel)),
            capture_output=True,
            timeout=300,
            check=False,
        )
    except (subprocess.SubprocessError, OSError):
        return {}
    if pulled.returncode or not pulled.stdout:
        return {}
    digests: dict[str, dict[str, Any]] = {}
    root = destination.resolve()
    try:
        with tarfile.open(fileobj=io.BytesIO(pulled.stdout)) as bundle:
            for member in bundle.getmembers():
                target = (destination / member.name).resolve()
                if not member.isfile() or (target != root and root not in target.parents):
                    continue
                data = bundle.extractfile(member)
                if data is None:
                    continue
                payload = data.read()
                name = Path(member.name).as_posix().removeprefix("./")
                if name == _LOG_SUMS:
                    for line in payload.decode(errors="replace").splitlines():
                        parts = line.split(" ", 2)
                        if len(parts) == 3 and parts[1].isdigit():
                            path = parts[2].removeprefix("./")
                            digests[path] = {
                                "sha256": parts[0],
                                "bytes": int(parts[1]),
                                "truncated": int(parts[1]) > RESULT_LOG_CAP_BYTES,
                            }
                    continue
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_bytes(payload)
    except (tarfile.TarError, OSError):
        return digests
    return digests


T2_BUNDLE_DIR = "t2bundle"


def _pull_t2_all_calls(worker: Worker, source_rel: str, output: Path) -> dict[str, Any]:
    """Retain ordered raw segments; the normal corpus bundle applies sampling caps."""
    output.mkdir(parents=True, exist_ok=True)
    result = subprocess.run(
        [
            "rsync",
            "-a",
            "--protect-args",
            "--partial",
            "--include=*/",
            "--include=capture.bin",
            "--include=capture.bin.seg-*.gz",
            "--include=capture.bin.seg-*.gz.part",
            "--include=capture.bin.stats.json",
            "--exclude=*",
            "-e",
            _ssh_transport(worker),
            f"{worker.target}:{source_rel}/",
            str(output) + "/",
        ],
        text=True,
        capture_output=True,
        timeout=1800,
        check=False,
    )
    if result.returncode:
        raise RuntimeError(f"all-calls transfer from {worker.name} failed: {result.stderr[-500:]}")
    segments = sorted(output.glob("capture.bin.seg-*.gz"))
    plain = output / "capture.bin"
    if not segments and not plain.is_file():
        raise RuntimeError(f"all-calls transfer from {worker.name} returned no capture spool")
    if list(output.glob("capture.bin.seg-*.gz.part")):
        raise RuntimeError(f"all-calls capture on {worker.name} has an unfinished segment")
    return {"mode": "all_calls", "segments": len(segments), "plain": plain.is_file()}


def _pull_t2_bundle(
    worker: Worker, *, prelude: str, root: str, run_id: str, output: Path
) -> dict[str, Any]:
    """T3740: carry a remote --t2-capture into the shared corpus.

    The worker's run compacts its spool segments into a run-local corpus (`<root>/data`, deleted
    with the staging dir). Its entries are bundled into spools on the worker, pulled, ingested
    into the local shared corpus (`data_dir("t2corpus")`, under the store's ingest lock) and the
    bundle files deleted. Returns the ingest ledger (also written as `t2corpus-ingest.json`)."""
    bundle_rel = f"{root}/{T2_BUNDLE_DIR}"
    # `prelude`: exec_prelude + python_select (T3760: the exec dir under its lease).
    command = (
        f'{prelude}cd "$SRC" && '
        f'"$python_bin" -m tools.t2corpus --store {_remote_path(root + "/data/t2corpus")} '
        f"bundle --out {_remote_path(bundle_rel)} --tag {shlex.quote(run_id)}"
    )
    made = _ssh(worker, command, timeout=1800)
    if made.returncode:
        raise RuntimeError(f"t2 bundle on {worker.name} failed: {(made.stderr or '')[-500:]}")
    local = output / T2_BUNDLE_DIR
    fetch_results(worker, bundle_rel, local)
    return ingest_t2_bundle(local, output)


def ingest_t2_bundle(bundle_dir: Path, output: Path, store: Path | None = None) -> dict[str, Any]:
    """Ingest pulled bundle spools into the shared corpus, then delete them."""
    from tools.datadir import data_dir
    from tools.t2corpus.spool import read_spool
    from tools.t2corpus.store import Corpus

    ledger: dict[str, Any] = {"bundles": 0, "records": 0, "kept": 0, "duplicates": 0}
    # TSFP_T2_CORPUS_NAME picks a fresh store (format 2 must not share caps with format 1)
    corpus = Corpus(store or data_dir(os.environ.get("TSFP_T2_CORPUS_NAME") or "t2corpus"))
    try:
        for path in sorted(Path(bundle_dir).glob("bundle-*.spool")):
            report = corpus.ingest(read_spool(path))
            ledger["bundles"] += 1
            ledger["records"] += report.records
            ledger["kept"] += report.kept
            ledger["duplicates"] += report.duplicates
            path.unlink()
    finally:
        corpus.close()
    shutil.rmtree(bundle_dir, ignore_errors=True)
    (Path(output) / "t2corpus-ingest.json").write_text(json.dumps(ledger, sort_keys=True) + "\n")
    return ledger


def fetch_results(worker: Worker, source_rel: str, destination: Path) -> None:
    """Fetch the full retained remote run directory on demand (for diagnosis)."""
    result = subprocess.run(
        [
            "rsync",
            "-a",
            "--compress-choice=zstd",
            "--compress-level=3",
            "--protect-args",
            "--partial",
            "-e",
            _ssh_transport(worker),
            f"{worker.target}:{source_rel}/",
            str(destination) + "/",
        ],
        text=True,
        capture_output=True,
        timeout=1800,
        check=False,
    )
    if result.returncode:
        raise RuntimeError(f"full result fetch from {worker.name} failed: {result.stderr[-500:]}")


def _private_owner_input(path: Path, private_root: Path) -> Path:
    resolved = path.expanduser().resolve(strict=True)
    owner_root = private_root.expanduser().resolve(strict=True)
    try:
        resolved.relative_to(owner_root)
    except ValueError as error:
        raise ValueError(f"owner input must resolve under {owner_root}: {resolved}") from error
    return resolved


def exec_host(worker: Worker) -> Any:
    """The registry entry (`remote_exec.HostSpec`) of a replay worker, by name or address.

    T3760: the exec dir and venv come from tools/data/remote_hosts.json (rule 45); a worker
    missing there has no exec dir, so the replay refuses it loudly (the caller falls back)."""
    from tools.wt import remote_exec

    address = worker.target.rsplit("@", 1)[-1]
    for host in remote_exec.load_registry():
        if host.name == worker.name or host.address == address:
            return host
    raise RuntimeError(f"replay worker {worker.name} is not in tools/data/remote_hosts.json")


def exec_slot_dir() -> Path:
    """Owner-PC lock files naming which replay exec dir of a host a job owns."""
    from tools.datadir import data_dir

    return data_dir("wt-cache/replay-exec-slots")


@contextlib.contextmanager
def exec_slot(worker: Worker, slots: int, base: Path | None = None) -> Any:
    """Hold a free replay slot of `worker` (flock on the owner PC) for the whole job.

    The slot names the exec dir, so two jobs never sync different trees into one dir; a crash
    releases the lock with the process. All slots busy raises (the caller tries the next
    worker). Dispatch happens only on the owner PC (remote hosts are execution-only)."""
    import fcntl

    base = base or exec_slot_dir()
    base.mkdir(parents=True, exist_ok=True)
    for slot in range(max(1, slots)):
        handle = (base / f"{worker.name}-{slot}.lock").open("a")
        try:
            fcntl.flock(handle, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except OSError:
            handle.close()
            continue
        try:
            yield slot
        finally:
            fcntl.flock(handle, fcntl.LOCK_UN)
            handle.close()
        return
    raise RuntimeError(f"all {max(1, slots)} replay exec dirs on {worker.name} are busy")


CAPTURE_BUDGET_WAIT_S = 300.0
"""T3837: how long a capture waits for room in its host's shared budget (a verify takes 2-10
min) before it gives the worker up like an all-busy replay slot."""
CAPTURE_TIMEOUT_S = 3600
"""Floor of the transport limit of one remote capture (rule 45: it grows with the recording)."""
CAPTURE_SECONDS_PER_BYTE = 7.0e-4
"""Measured 5.9e-4 s per recording byte (story-l2-full-boss-20261026: 6.56 MB, 22 min of play,
about 3,900 s on the laptop), plus about 18 percent headroom."""
CAPTURE_TIMEOUT_MARGIN_S = 600
CAPTURE_TIMEOUT_CEILING_ENV = "TSFP_CAPTURE_TIMEOUT_CEILING_S"
CAPTURE_TIMEOUT_CEILING_S = 14400


def capture_timeout_s(route: Path, environ: Mapping[str, str] | None = None) -> int:
    """Transport limit for capturing `route`: size times the measured factor plus a margin,
    never under CAPTURE_TIMEOUT_S, never over the ceiling (`TSFP_CAPTURE_TIMEOUT_CEILING_S`)."""
    env = os.environ if environ is None else environ
    try:
        ceiling = int(env.get(CAPTURE_TIMEOUT_CEILING_ENV, "") or CAPTURE_TIMEOUT_CEILING_S)
    except ValueError:
        ceiling = CAPTURE_TIMEOUT_CEILING_S
    try:
        size = route.stat().st_size
    except OSError:
        size = 0
    wanted = int(size * CAPTURE_SECONDS_PER_BYTE) + CAPTURE_TIMEOUT_MARGIN_S
    return max(CAPTURE_TIMEOUT_S, min(wanted, max(ceiling, CAPTURE_TIMEOUT_S)))


def exec_prelude(source_dir: str, tree: str, timeout_s: int = CAPTURE_TIMEOUT_S) -> str:
    """Shell: enter the job's exec dir under its shared lease, after checking its marker names
    `tree`; exports the source identity (T3760, same contract as remote_exec.verify_script).
    The lease ends with the ssh session or after `timeout_s` (`exec_sync.REAP_SH`)."""
    needle = shlex.quote(f'"tree": "{tree}"')
    lease = exec_sync.lease_hold('"$SRC"', fd=7, deadline_s=timeout_s + 60, attached=True)
    return (
        f"SRC={_remote_path(source_dir)}; "
        f"{{ {lease}; }} "
        f'|| {{ echo "@@LEASE_TIMEOUT $SRC"; exit {REPLAY_LEASE_EXIT}; }}; '
        f'grep -qF {needle} "$SRC"/{exec_sync.MARKER} '
        f'|| {{ echo "@@SOURCE_MISSING $SRC"; exit {REPLAY_SOURCE_MISSING_EXIT}; }}; '
    )


def python_select(exec_spec: Any, repo_path: str) -> str:
    """Shell: `python_bin` = the exec venv's python, else the legacy data-root venv (hosts not
    yet provisioned by T3760), else the system python3."""
    from tools.wt import remote_exec

    candidates = (
        f"{remote_exec.exec_venv(exec_spec)}/bin/python",
        str(Path(repo_path) / ".venv" / "bin" / "python"),
    )
    return (
        "python_bin=python3; for candidate in "
        + " ".join(_repo_cd(path) for path in candidates)
        + '; do [ -x "$candidate" ] && python_bin=$candidate && break; done; '
    )


def _sync_source(
    exec_spec: Any,
    *,
    source_commit: str,
    source_dir: str,
    breakdown: dict[str, Any],
    report: list[dict[str, Any]],
) -> str:
    """Sync `source_commit`'s tree into the job's exec dir by rsync delta; return the tree.

    T3760: replaces the per-commit host tree cache, `source_push` and the archive upload; the
    tree stays in the dir after the job, so the next sync of a nearby commit is a delta."""
    from tools.wt import remote_exec

    started = time.monotonic()
    root = Path(__file__).resolve().parents[2]
    tree = exec_sync.tree_of(root, source_commit)
    synced = remote_exec.default_sync(exec_spec, root, tree, source_commit, source_dir)
    breakdown.update(
        source_mode=synced.mode,
        source_exec_dir=source_dir,
        source_files_transferred=synced.files_transferred,
        source_archive_seconds=0.0,
        source_transfer_seconds=round(time.monotonic() - started, 3),
        source_materialize_seconds=0.0,
    )
    _record(report, "source", cache=synced.mode, bytes_sent=synced.bytes_sent)
    return tree


def run_capture(worker: Worker, **kwargs: Any) -> subprocess.CompletedProcess[str]:
    """Stage, run and pull one capture over one shared ssh master; clean up on any error.

    T3462: an rsync timeout used to leave a multi-GB half-staged directory on the worker.
    T3678: all ssh and rsync calls of the job share one ControlMaster (private short socket
    dir, closed with `ssh -O exit` on success and on error).

    T3760: the job owns one replay slot of the worker (`exec_slot`), whose exec dir
    (`remote_exec.replay_exec_dir`) receives the source tree; the error cleanup removes the
    per-job staging dir only, never the synced tree.
    """
    run_id = f"{kwargs['key']}-{secrets.token_hex(8)}"
    exec_spec = exec_host(worker)
    cache_dir = getattr(exec_spec, "replay_cache_dir", "")
    root = staging_root(kwargs["home"], kwargs["repo_path"], cache_dir) + "/" + run_id
    from tools.wt import remote_exec

    control_dir = tempfile.mkdtemp(prefix="tsfp-cm-", dir="/tmp")
    token = _CONTROL_PATH.set(f"{control_dir}/m")
    up_token = _CONTROL_UP.set(False)
    try:
        # T3837: the host's ONE budget (shared with verify and prebuilt builds) before the slot;
        # a capture may wait CAPTURE_BUDGET_WAIT_S for a verify to finish (BudgetFull after it).
        budget = getattr(exec_spec, "budget", None)
        with (
            host_budget.lease(
                getattr(exec_spec, "name", worker.name),  # the registry name verify uses
                "capture",
                host_budget.job_cost("capture") if budget else host_budget.Cost(0, 0),
                budget,
                wait_s=CAPTURE_BUDGET_WAIT_S,
            ),
            exec_slot(worker, worker.max_slots) as slot,
        ):
            open_control_master(worker)
            return _run_capture(
                worker,
                run_id=run_id,
                exec_spec=exec_spec,
                source_dir=remote_exec.replay_exec_dir(exec_spec, slot),
                **kwargs,
            )
    except BaseException:
        _remove_remote(worker, root)
        raise
    finally:
        _forget_cached_input_checks(worker)
        close_control_master(worker)
        _CONTROL_UP.reset(up_token)
        _CONTROL_PATH.reset(token)
        shutil.rmtree(control_dir, ignore_errors=True)


def _run_capture(
    worker: Worker,
    *,
    run_id: str,
    key: str,
    host: Path,
    route: Path,
    output: Path,
    switches: str,
    capture_options: tuple[str, ...],
    host_args: tuple[str, ...],
    save_dir: Path | None,
    env_values: tuple[str, ...],
    priority: str,
    slots: int,
    home: str,
    repo_path: str,
    private_root: Path,
    source_commit: str,
    exec_spec: Any,
    source_dir: str,
) -> subprocess.CompletedProcess[str]:
    """Sync the source into `source_dir`, stage inputs by content hash, execute behind remote
    hostlock under the exec dir's lease, and pull trace results only."""
    staging_breakdown: dict[str, Any] = {}
    report: list[dict[str, Any]] = []
    with _stream_lock:
        _stream_stats["peak"] = 0
    staging_started = time.monotonic()
    cache_dir = getattr(exec_spec, "replay_cache_dir", "")
    root = staging_root(home, repo_path, cache_dir) + "/" + run_id
    source_root = source_dir  # T3760: the synced exec dir, kept after the job
    host_rel, route_rel = f"{root}/inputs/host/tsfp-bevy", f"{root}/inputs/route/{route.name}"
    out_rel, log_rel = f"{root}/result", f"{root}/capture.log"
    mkdir_command = "mkdir -p " + " ".join(
        _remote_path(path)
        for path in (
            root + "/inputs/host",
            root + "/inputs/route",
            root + "/data",
            out_rel,
        )
    )
    cache_root = f"{staging_root(home, repo_path, cache_dir)}/input-cache"
    prepared = _ssh(worker, mkdir_command)
    if prepared.returncode:
        raise RuntimeError(f"could not prepare staging directories on {worker.name}")
    # The tree first (a delta of seconds): the evictor and the capture run from it.
    tree = _sync_source(
        exec_spec,
        source_commit=source_commit,
        source_dir=source_root,
        breakdown=staging_breakdown,
        report=report,
    )
    evict_input_cache(worker, source_root, cache_root)
    route = _private_owner_input(route, private_root)
    args = [
        "python3",
        "-m",
        "tools.rust_trace",
        "capture",
        "--route",
        route_rel,
        f"--rust-ports={switches}",
        "--host",
        host_rel,
        "--out-dir",
        out_rel,
        *capture_options,
    ]
    # Rebase the supported file-valued capture options into the private staged input tree.
    # The queue pins the hash locally; on the worker, --host names the verified staged
    # binary directly, because rust_trace's hash selector searches published manifests.
    values = _use_staged_host(args)
    from tools.rust_trace import default_xbe

    def stage_xbe() -> None:
        # The source tree omits this ignored runtime input. rust_trace replaces its default
        # build/default.xbe argument with this path; transfer it privately and verify its
        # bytes just like the disc and route inputs (the capture removes it when done).
        started = time.monotonic()
        _stage_default_xbe(
            worker, Path(default_xbe()), source_root, cache_root=cache_root, report=report
        )
        staging_breakdown["xbe_transfer_seconds"] = round(time.monotonic() - started, 3)

    def stage_host() -> tuple[Path, ...]:
        started = time.monotonic()
        libraries = _stage_host(worker, host, host_rel, cache_root=cache_root, report=report)
        staging_breakdown["host_transfer_seconds"] = round(time.monotonic() - started, 3)
        return libraries

    def stage_route() -> None:
        started = time.monotonic()
        _copy_owner_input(worker, route, route_rel, cache_root, report=report, label="route")
        _stage_route_saves(
            worker, route, route_rel, private_root, cache_root=cache_root, report=report
        )
        staging_breakdown["route_transfer_seconds"] = round(time.monotonic() - started, 3)

    def stage_extra_inputs() -> None:
        nonlocal save_dir
        started = time.monotonic()
        for flag in ("--disc", "--regions", "--t2-budget-seed", "--t2-capture-targets"):
            if flag in values:
                index = values.index(flag) + 1
                source = Path(values[index]).expanduser().resolve()
                if not source.is_file():
                    raise ValueError(f"remote capture input not found: {source}")
                if flag == "--disc":
                    source = _private_owner_input(source, private_root)
                dest = f"{root}/inputs/{flag[2:]}/{source.name}"
                _copy_owner_input(worker, source, dest, cache_root, report=report, label=flag[2:])
                values[index] = dest
        if save_dir is not None:
            save_dir = _private_owner_input(save_dir, private_root)
            if not save_dir.is_dir():
                raise ValueError(f"save directory not found: {save_dir}")
            saves_rel = f"{root}/inputs/saves"
            _copy_owner_input(worker, save_dir, saves_rel, cache_root, report=report, label="saves")
            values.extend(["--save-dir", saves_rel])
        staging_breakdown["extra_input_transfer_seconds"] = round(time.monotonic() - started, 3)

    # Hash every immutable replay input up front and ask the remote cache about the full set
    # in one ControlMaster round trip. Later staging workers consume these prechecked results.
    cache_sources: list[Path] = [
        host,
        *host_libraries(host),
        route,
        Path(default_xbe()),
    ]
    for flag in ("--disc", "--regions", "--t2-budget-seed", "--t2-capture-targets"):
        if flag in values:
            candidate = Path(values[values.index(flag) + 1]).expanduser().resolve()
            if candidate.is_file():
                cache_sources.append(candidate)
    if save_dir is not None:
        save_dir = _private_owner_input(save_dir, private_root)
        if not save_dir.is_dir():
            raise ValueError(f"save directory not found: {save_dir}")
    _forget_cached_input_checks(worker)
    with tempfile.TemporaryDirectory(prefix="tsfp-cache-probe-") as temp_dir:
        tree_sources = []
        from tools.play import rec_saves

        stamped_saves = rec_saves.header_saves(route)
        if stamped_saves is not None:
            tree_sources.append(rec_saves.locate(route, stamped_saves[0]))
        if save_dir is not None and save_dir.is_dir():
            tree_sources.append(save_dir)
        for index, tree_source in enumerate(tree_sources):
            archive = Path(temp_dir) / str(index) / f"{tree_source.name}.tree.tar"
            archive.parent.mkdir(parents=True, exist_ok=True)
            pack_tree(tree_source, archive)
            cache_sources.append(archive)
        unique_sources = tuple(dict.fromkeys(cache_sources))
        _batch_cached_input_checks(worker, unique_sources, cache_root)

    # The four groups are independent: they share the job's ssh master and at most STREAMS
    # concurrent rsync streams, so a WAN link that is latency-bound per stream is filled.
    try:
        staged = _run_parallel([stage_xbe, stage_host, stage_route, stage_extra_inputs])
        host_runtime_libs = staged[1]
    finally:
        _forget_cached_input_checks(worker)
    _prune_input_cache(worker, cache_root)
    staging_breakdown["staging_wall_seconds"] = round(time.monotonic() - staging_started, 3)
    staging_breakdown["rsync_streams_peak"] = _stream_stats["peak"]
    staging_breakdown["rsync_streams_max"] = STREAMS
    staging_breakdown["bytes_sent_total"] = sum(int(item.get("bytes_sent") or 0) for item in report)
    staging_breakdown["inputs"] = report
    values.extend(f"--host-arg={argument}" for argument in host_args)
    assignments: dict[str, str] = {}
    from tools.wt.capture_artifacts import resolve_capture_env

    for item in env_values:
        name, value = resolve_capture_env(item, out_rel)
        assignments[name] = value
    # Remote worker admission remains bounded by the live probe even when caller env
    # overrides select other TSFP behavior.
    assignments.update(
        {
            "TSFP_HOST_SLOTS": str(slots),
            "TSFP_HOSTLOCK_DIR": str(Path(home) / ".cache" / f"tsfp-replay-hostlock-{worker.name}"),
            "TSFP_HOST_MEM_RESERVE_MB": str(memory_model(worker)[0]),
            "TSFP_HOST_JOB_RSS_MB": str(memory_model(worker)[1]),
            "TSFP_CAPTURE_TIMING_FILE": out_rel + "/capture-timing.json",
            # T3760: the exec dir has no .git; derived data and recipe metadata come as data.
            "TSFP_DATA_DIR": root + "/data",
            "TSFP_RECIPE_GIT_HEAD": source_commit,
            # T3740: worker disks are small and capguarded; the owner-PC floors refuse them.
            "TSFP_DISK_PROFILE": "remote_worker",
            "TSFP_DISK_HOST": worker.name,
        }
    )
    if host_runtime_libs:
        assignments["LD_LIBRARY_PATH"] = root + "/inputs/host"
    env_command = " ".join(f"{name}={shlex.quote(value)}" for name, value in assignments.items())
    if priority == "high":
        priority_dir = Path(home) / ".cache" / f"tsfp-replay-priority-{worker.name}"
        marker_path = str(priority_dir / f"{run_id}.pid")
        priority_setup = (
            f"mkdir -p {_remote_path(str(priority_dir))} && "
            f"printf '%s\\n' \"$$\" > {_remote_path(marker_path)} && "
        )
        priority_cleanup = f"rm -f {_remote_path(marker_path)}; "
    else:
        marker_dir = _remote_path(
            str(Path(home) / ".cache" / ("tsfp-replay-priority-" + worker.name))
        )
        priority_setup = (
            "while :; do active=0; "
            f"for marker in {marker_dir}/*.pid; do "
            '[ -f "$marker" ] || continue; pid=$(cat "$marker" 2>/dev/null || true); '
            'if [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null; '
            'then active=1; else rm -f "$marker"; fi; done; '
            '[ "$active" -eq 0 ] && break; sleep 2; done; '
        )
        priority_cleanup = ""
    # T3462: no outer `tools.hostlock --` wrapper. rust_trace admits its own host launch
    # (@serialized), and the nested second admission deadlocked a one-slot worker: the outer
    # lock held the only slot while the inner one waited for it (measured on the droplet).
    # T3516: re-check the host's health right before the capture starts (staging took minutes).
    guard, host_cores, _slots = host_health.facts_for_host(worker.name)
    gate = host_health.gate_command(
        dataclasses.replace(guard, max_builds=0) if guard is not None else None,
        cores=host_cores,
        slot_files=[],
    )
    capture_timeout = capture_timeout_s(route)
    command = (
        f"{gate}; "
        f"{exec_prelude(source_root, tree, capture_timeout)}"
        f"{python_select(exec_spec, repo_path)}"
        f"{exec_sync.identity_exports(source_commit, tree)}; "
        f'cd "$SRC" && {priority_setup}'
        f'{env_command} "$python_bin" -m tools.rust_trace '
        f"{shlex.join(values[3:])} "
        f"> {_remote_path(log_rel)} 2>&1; rc=$?; {priority_cleanup}"
        'rm -f "$SRC"/build/default.xbe; '
        f"tail -n 30 {_remote_path(log_rel)}; exit $rc"
    )
    phase_started = time.monotonic()
    result = _ssh(worker, command, timeout=capture_timeout)
    remote_command_seconds = time.monotonic() - phase_started
    # T3760: the logs come back whatever the outcome (regress classifies faults from run.log;
    # the receipt records their sha256 and size).
    result.remote_logs = _pull_logs(worker, out_rel, output)
    if result.returncode == 0:
        phase_started = time.monotonic()
        _pull_results(worker, out_rel, output)
        timing_result = _ssh(
            worker,
            f"cat {_remote_path(out_rel + '/capture-timing.json')} 2>/dev/null || true",
            timeout=15,
        )
        result.capture_timing = None
        if timing_result.returncode == 0 and timing_result.stdout.strip():
            try:
                result.capture_timing = json.loads(timing_result.stdout)
            except json.JSONDecodeError:
                result.capture_timing = None
        from tools.t2capture.runtime_env import preserve_all_calls

        if "--t2-capture" in values and preserve_all_calls(assignments):
            result.t2_ingest = _pull_t2_all_calls(worker, out_rel, output)
        elif "--t2-capture" in values:
            result.t2_ingest = _pull_t2_bundle(
                worker,
                prelude=exec_prelude(source_root, tree, capture_timeout)
                + python_select(exec_spec, repo_path),
                root=root,
                run_id=run_id,
                output=output,
            )
        staging_breakdown["result_transfer_seconds"] = round(time.monotonic() - phase_started, 3)
        child_seconds = float((result.capture_timing or {}).get("run_seconds") or 0.0)
        queue_seconds = float((result.capture_timing or {}).get("queue_wait_seconds") or 0.0)
        staging_breakdown["remote_setup_seconds"] = round(
            max(0.0, remote_command_seconds - child_seconds - queue_seconds), 3
        )
        result.remote_staging_breakdown = staging_breakdown
    else:
        diagnostic_command = (
            f"printf '%s\\n' '--- staged input files (path and size) ---'; "
            f"find {_remote_path(root + '/inputs')} -maxdepth 4 -type f "
            "-printf '%p %s bytes\\n' 2>&1 | head -n 30; "
            f"printf '%s\\n' '--- result files (path and size) ---'; "
            f"find {_remote_path(out_rel)} -maxdepth 2 -type f "
            "-printf '%p %s bytes\\n' 2>&1 | head -n 30; "
            f"printf '%s\\n' '--- host linkage ---'; "
            f"ldd {_remote_path(host_rel)} 2>&1 | "
            "grep -E 'not found|not a dynamic executable' || true; "
            f"for log in {_remote_path(out_rel + '/run.log')} {_remote_path(log_rel)}; do "
            'if [ -f "$log" ]; then '
            'printf \'%s\\n\' "--- $log (first 40 lines) ---"; head -n 40 "$log"; '
            'printf \'%s\\n\' "--- $log (last 60 lines) ---"; tail -n 60 "$log"; '
            "fi; done"
        )
        diagnostics = _ssh(worker, diagnostic_command, timeout=15)
        detail_parts = [
            part.strip()
            for part in (diagnostics.stdout, diagnostics.stderr)
            if part and part.strip()
        ]
        detail = "\n".join(detail_parts)
        if detail:
            diagnostic_text = "\nRemote launch diagnostics:\n" + detail
            # replay.submit prints the tails of both streams on failure. Append the
            # remote inspection to stdout too, so it remains visible even when SSH
            # diagnostics themselves arrive on stderr and rust_trace has noisy output.
            result.stdout = (result.stdout or "") + diagnostic_text
            result.stderr = (result.stderr or "") + diagnostic_text
    # Successful transfers can be discarded immediately. Failed captures retain their
    # private staging directory so the bounded diagnostics above can be inspected after
    # the SSH session; retrying the queue must not erase the only remote failure evidence.
    if result.returncode == 0:
        _remove_remote(worker, root)
    else:
        # T3462: keep the logs and the partial result, drop the multi-GB inputs (disc, host)
        # that the diagnostics above have already listed. T3760: the synced tree lives in the
        # exec dir, not here, and is never removed.
        _remove_remote(worker, *(f"{root}/{name}" for name in ("inputs", "data")))
        note = f"\nRemote failure logs retained at {root} (inputs and data removed)"
        result.stdout = (result.stdout or "") + note
        result.stderr = (result.stderr or "") + note
    return result


__all__ = [
    "Health",
    "REMOTE_HOST_CAPS",
    "Worker",
    "calculate_slots",
    "configured_workers",
    "health",
    "run_capture",
]
