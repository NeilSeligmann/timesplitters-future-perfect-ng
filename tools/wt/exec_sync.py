"""Ship an exact source tree to a stateless remote exec dir by rsync delta (T3760).

Owner decision 2026-10-26 (docs/decisions/remote-hosts-execution-only.md): the owner PC holds
the only git repository; remote hosts build, run and test and keep no commits. Every remote job
gets its exact tree like this:

1. `export_tree`: the tree is checked out into a local export mirror (a pool under
   `tmp/wt-cache/exec-export/<n>`, one per concurrent sync, each with its own private index) by
   `git read-tree --reset -u`, which rewrites only the files that differ from the mirror's last
   tree. Stray, deleted or modified mirror files are repaired, so the mirror is exactly the tree.
2. `sync`: `rsync -rlpc --delete` from the mirror into a stable per-slot exec dir on the host.
   Checksum mode without `-t`: unchanged files are not touched (their host mtimes stay, so the
   cargo fingerprints and the shared sccache stay warm), changed files get a fresh mtime and are
   rebuilt. The host-side rsync runs under `timeout -k`, the local side under a timeout scaled by
   the bytes that changed since the dir's last synced tree (from its marker), with one retry,
   then a gzip tar stream of the whole tree into the dir as the last resort.
3. The dir's marker `.tsfp-source.json` (commit, tree, time) is deleted before a transfer and
   written after it, so a half-synced dir never claims a tree. Jobs get the identity as data
   (`identity_exports`: TSFP_SOURCE_COMMIT, TSFP_SOURCE_TREE), never from host git; the tree's
   listing `.tsfp-source-files` travels inside it (`tools.wt.source_identity` reads blob ids
   from it, T3760 stage 4).

No host git is involved at any step. Every sync appends one row to
`tmp/wt-cache/exec-sync.jsonl` (`tools.wt.exec_sync stats`, HEALTH `exec-sync-failing`).
The caller owns the exec dir for the job (slot lock or reservation); two syncs into one dir at
once are prevented on the host by the dir's lock file `<dir>.lock`: a sync holds it exclusively,
a job holds it shared for its whole run (`lease_hold`), and host cleaners skip a dir whose lock
they cannot take (`LEASED_SH`, `leased`). It is the same file the T3472 verify script locked
(`<canon_root>/src.lock`), so tools from before T3760 and after it never share a dir.
"""

from __future__ import annotations

import argparse
import contextlib
import fcntl
import json
import os
import re
import shlex
import shutil
import subprocess
import sys
import time
from collections.abc import Callable, Iterator, Sequence
from dataclasses import asdict, dataclass, field
from pathlib import Path
from typing import Any

from tools.wt.source_identity import LISTING, MARKER

CACHE_PROTECT = (
    "__pycache__/",
    ".pytest_cache/",
    ".ruff_cache/",
    ".mypy_cache/",
    ".hypothesis/",
)
"""Host-side caches inside an exec dir that `--delete` keeps (they never change results)."""
MIN_BYTES_PER_S = 1_500_000
"""Slowest link still waited for (same floor as remote_replay.transfer_timeout)."""
BASE_TIMEOUT_S = 120
"""Fixed part of a sync timeout: ssh setup plus checksumming ~28k files on both sides."""
IO_TIMEOUT_S = 60
LOCK_WAIT_IN_TRANSFER_S = 20
"""How long the host side of a transfer may still wait for the dir's lock. Must stay well
under IO_TIMEOUT_S: rsync's I/O clock runs while the host side waits (I-T3760-9). The long
wait for a running job's lease happens first, in `wait_free_argv`, before any transfer."""
"""rsync `--timeout`: a transfer with no I/O for this long is dead, whatever its total limit."""
POOL_SIZE = 3
POOL_WAIT_S = 600
LOG_NAME = "exec-sync.jsonl"
ENV_POOL = "TSFP_EXEC_EXPORT_SLOTS"
ENV_LOG = "TSFP_EXEC_SYNC_LOG"
ENV_EXPORT_DIR = "TSFP_EXEC_EXPORT_DIR"
_TREE = re.compile(r"[0-9a-f]{40}([0-9a-f]{24})?")
_GIT_SAFE = (
    "-c",
    "core.sparseCheckout=false",
    "-c",
    "core.autocrlf=false",
    "-c",
    "core.symlinks=true",
)

Runner = Callable[..., subprocess.CompletedProcess]
"""`run(argv, *, timeout, input=None)`: the local command runner (fakes replace it in tests)."""


class ExecSyncError(RuntimeError):
    """The tree could not be placed on the host by rsync nor by the tar fallback."""


class ExecSyncBusy(ExecSyncError):
    """T3798: the dir's lease stayed held (another job still runs in it). Nothing about the
    host's transport: callers treat it as a busy slot, never as a degraded host."""


@dataclass(frozen=True)
class Endpoint:
    """Where a tree goes: `user@host`, ssh port, and the exec dir on the host."""

    name: str
    target: str
    port: int
    dest: str
    transport: str = ""
    """The `ssh ...` command rsync uses (default: BatchMode on `port`)."""

    def ssh_command(self) -> str:
        return self.transport or shlex.join(
            ["ssh", "-p", str(self.port), "-o", "BatchMode=yes", "-o", "ConnectTimeout=10"]
        )

    def remote_dest(self) -> str:
        """`dest` as rsync's remote path (home-relative for `~/x`)."""
        return self.dest[2:] if self.dest.startswith("~/") else self.dest

    def shell_dest(self) -> str:
        """`dest` quoted for a remote shell, keeping `~/` expansion."""
        if self.dest.startswith("~/"):
            return '"$HOME"/' + shlex.quote(self.dest[2:])
        return shlex.quote(self.dest)


@dataclass
class SyncResult:
    host: str
    dest: str
    tree: str
    commit: str
    mode: str
    """`rsync`, `rsync-retry` or `tar`."""
    seconds: float
    timeout_s: int
    delta_bytes: int
    bytes_sent: int | None = None
    files_transferred: int | None = None
    errors: list[str] = field(default_factory=list)


def _run(
    argv: Sequence[str],
    *,
    timeout: float,
    input: str | None = None,
    env: dict[str, str] | None = None,
) -> subprocess.CompletedProcess:
    """Run in its own process group; on timeout the whole group dies (rsync's ssh child too)."""
    process = subprocess.Popen(
        list(argv),
        stdin=subprocess.PIPE if input is not None else subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        env=env,
        start_new_session=True,
    )
    try:
        out, err = process.communicate(input, timeout=timeout)
    except subprocess.TimeoutExpired:
        with contextlib.suppress(OSError):
            os.killpg(process.pid, 9)
        process.communicate()
        raise
    return subprocess.CompletedProcess(list(argv), process.returncode, out, err)


def _git(
    root: Path,
    *args: str,
    env: dict[str, str] | None = None,
    input: str | None = None,
    timeout: float = 300,
) -> subprocess.CompletedProcess:
    return subprocess.run(
        ["git", *_GIT_SAFE, "-C", str(root), *args],
        capture_output=True,
        text=True,
        timeout=timeout,
        check=False,
        env=env,
        input=input,
    )


def export_dir() -> Path:
    override = os.environ.get(ENV_EXPORT_DIR)
    if override:
        return Path(override)
    from tools import datadir

    return datadir.data_dir("wt-cache/exec-export")


def log_path() -> Path:
    override = os.environ.get(ENV_LOG)
    if override:
        return Path(override)
    from tools import datadir

    return datadir.data_dir(f"wt-cache/{LOG_NAME}")


@contextlib.contextmanager
def mirror(base: Path | None = None, *, wait_s: float = POOL_WAIT_S) -> Iterator[Path]:
    """One free export mirror of the pool, held (flock) until the block ends."""
    base = base or export_dir()
    base.mkdir(parents=True, exist_ok=True)
    size = max(1, int(os.environ.get(ENV_POOL, POOL_SIZE)))
    deadline = time.monotonic() + wait_s
    while True:
        for index in range(size):
            handle = (base / f"{index}.lock").open("a")
            try:
                fcntl.flock(handle, fcntl.LOCK_EX | fcntl.LOCK_NB)
            except OSError:
                handle.close()
                continue
            try:
                yield base / str(index)
            finally:
                fcntl.flock(handle, fcntl.LOCK_UN)
                handle.close()
            return
        if time.monotonic() > deadline:
            raise ExecSyncError(f"no free export mirror in {base} after {wait_s:.0f} s")
        time.sleep(0.5)


def export_tree(root: Path, tree: str, directory: Path) -> None:
    """Make `directory` exactly `tree` (files, modes, symlinks), touching only what differs."""
    if not _TREE.fullmatch(tree):
        raise ExecSyncError(f"not a tree id: {tree!r}")
    directory.mkdir(parents=True, exist_ok=True)
    env = {
        **os.environ,
        "GIT_INDEX_FILE": str(directory.with_name(directory.name + ".index")),
        "GIT_WORK_TREE": str(directory),
    }
    reset = ["read-tree", "--reset", "-u", tree]
    done = _git(root, *reset, env=env)
    if done.returncode:
        # A damaged private index: start it over (every file is compared again).
        Path(env["GIT_INDEX_FILE"]).unlink(missing_ok=True)
        done = _git(root, *reset, env=env)
    if done.returncode:
        raise ExecSyncError(f"git read-tree into the export mirror failed: {done.stderr[-300:]}")
    # `--reset -u` also rewrites mirror files that were deleted or edited (stat-checked); only
    # files git does not track there are left, and they go.
    others = _git(root, "ls-files", "-z", "--others", "--directory", env=env)
    for name in filter(None, others.stdout.split("\0")):
        if name == LISTING:
            continue  # rewritten below only when the tree changed
        path = directory / name.rstrip("/")
        if path.is_dir() and not path.is_symlink():
            shutil.rmtree(path, ignore_errors=True)
        else:
            path.unlink(missing_ok=True)
    listing = subprocess.run(
        ["git", *_GIT_SAFE, "-C", str(root), "ls-tree", "-r", "-z", "--full-tree", tree],
        capture_output=True,
        timeout=300,
        check=False,
    )
    if listing.returncode:
        raise ExecSyncError(f"git ls-tree {tree} failed: {listing.stderr.decode()[-300:]}")
    _write_if_changed(directory / LISTING, listing.stdout)


def _write_if_changed(path: Path, data: bytes) -> None:
    """Keep the mtime of an unchanged file (rsync -c skips it either way, cargo never sees it)."""
    with contextlib.suppress(OSError):
        if path.read_bytes() == data:
            return
    path.write_bytes(data)


def tree_bytes(root: Path, tree: str) -> int:
    listed = _git(root, "ls-tree", "-r", "-l", "-z", tree)
    total = 0
    for entry in filter(None, listed.stdout.split("\0")):
        size = entry.split("\t", 1)[0].split()[-1]
        total += int(size) if size.isdigit() else 0
    return total


def delta_bytes(root: Path, old_tree: str | None, new_tree: str) -> int:
    """Bytes of the files that differ between the dir's last tree and `new_tree` (the whole
    tree when the old one is unknown or not in the local repo)."""
    if not old_tree or not _TREE.fullmatch(old_tree):
        return tree_bytes(root, new_tree)
    diff = _git(root, "diff-tree", "-r", "-z", "--no-renames", old_tree, new_tree)
    if diff.returncode:
        return tree_bytes(root, new_tree)
    fields = diff.stdout.split("\0")
    blobs = []
    for index in range(0, len(fields) - 1, 2):
        meta = fields[index].split()
        if len(meta) >= 5 and set(meta[3]) != {"0"}:
            blobs.append(meta[3])
    if not blobs:
        return 0
    sizes = _git(root, "cat-file", "--batch-check=%(objectsize)", input="\n".join(blobs) + "\n")
    return sum(int(line) for line in sizes.stdout.split() if line.isdigit())


def sync_timeout(size: int) -> int:
    """Seconds for one transfer of `size` changed bytes at the slowest link we wait for."""
    return BASE_TIMEOUT_S + int(size / MIN_BYTES_PER_S)


def read_marker(endpoint: Endpoint, run: Runner = _run) -> dict[str, Any] | None:
    argv = [
        *shlex.split(endpoint.ssh_command()),
        endpoint.target,
        f"cat {endpoint.shell_dest()}/{MARKER} 2>/dev/null || true",
    ]
    try:
        done = run(argv, timeout=30)
    except (subprocess.SubprocessError, OSError):
        return None
    try:
        data = json.loads(done.stdout or "null")
    except ValueError:
        return None
    return data if isinstance(data, dict) else None


def rsync_argv(
    endpoint: Endpoint, source: Path, timeout_s: int, protect: Sequence[str] = ()
) -> list[str]:
    dest = endpoint.shell_dest()
    # Host side: create the dir, wait for its lock (a running job holds it shared), drop the
    # marker, run rsync under its own timeout so a lost connection never leaves a remote rsync
    # behind (I-T3745-7). rsync appends its server arguments, which `"$@"` passes on.
    inner = f'rm -f {dest}/{MARKER} && exec timeout -k 5 {timeout_s} rsync "$@"'
    wait = min(timeout_s, LOCK_WAIT_IN_TRANSFER_S)
    remote = (
        f"mkdir -p {dest} && flock -w {wait} {dest}{LEASE_SUFFIX} sh -c {shlex.quote(inner)} sh"
    )
    filters = [f"--filter=P {item}" for item in (*CACHE_PROTECT, *protect)]
    return [
        "rsync",
        "-rlpc",
        "--delete",
        "--compress",
        "--info=stats2",
        f"--timeout={IO_TIMEOUT_S}",
        *filters,
        "-e",
        endpoint.ssh_command(),
        f"--rsync-path={remote}",
        f"{source}/",
        f"{endpoint.target}:{endpoint.remote_dest()}/",
    ]


def tar_argv(endpoint: Endpoint, source: Path, timeout_s: int) -> list[str]:
    """`sh -c` pipeline streaming the whole tree as gzip tar into a fresh dir, then swapping."""
    dest = endpoint.shell_dest()
    swap = (
        f"rm -rf {dest}.new && mkdir -p {dest}.new && gzip -dc | tar -xf - -C {dest}.new "
        f"&& rm -rf {dest} && mv {dest}.new {dest}"
    )
    remote = (
        f"mkdir -p {dest} && flock -w {min(timeout_s, LOCK_WAIT_IN_TRANSFER_S)} "
        f"{dest}{LEASE_SUFFIX} timeout -k 5 {timeout_s} sh -c {shlex.quote(swap)}"
    )
    ssh = shlex.join([*shlex.split(endpoint.ssh_command()), endpoint.target, remote])
    return [
        "bash",
        "-o",
        "pipefail",
        "-c",
        f"tar -C {shlex.quote(str(source))} -cf - . | gzip -1 | {ssh}",
    ]


def wait_free_argv(endpoint: Endpoint, wait_s: int) -> list[str]:
    """ssh: break a dead job's lease (`REAP_SH`), then wait up to `wait_s` for the dir's lock to
    be free (a running job holds it shared)."""
    dest = endpoint.shell_dest()
    return [
        *shlex.split(endpoint.ssh_command()),
        endpoint.target,
        f"mkdir -p {dest} && {{ {REAP_SH} tsfp_reap {dest}; }} "
        f"&& flock -w {wait_s} {dest}{LEASE_SUFFIX} true",
    ]


def write_marker_argv(endpoint: Endpoint, commit: str, tree: str) -> list[str]:
    body = json.dumps({"commit": commit, "tree": tree, "synced_at": round(time.time())})
    return [
        *shlex.split(endpoint.ssh_command()),
        endpoint.target,
        f"printf '%s\\n' {shlex.quote(body)} > {endpoint.shell_dest()}/{MARKER}",
    ]


LEASE_SUFFIX = ".lock"
LEASE_FRESH_S = 6 * 3600
"""A dir synced (marker) or used (lease file touched) within this window is never pruned."""
LEASED_SH = (
    "tsfp_leased() { "
    '[ -e "$1' + LEASE_SUFFIX + '" ] || return 1; '
    'flock -n -x "$1' + LEASE_SUFFIX + '" true 2>/dev/null && return 1; return 0; }'
)
"""Shell function for host cleaners: `tsfp_leased <dir> && skip` (a job or pull holds it)."""


HOLDERS_SUFFIX = ".holders"
"""`<dir>.holders/<pid>`: one line per lease holder,
`pid start anchor anchor_start pgid deadline ns`. start is the /proc starttime (a reused pid never
matches), anchor the holder's nearest `sshd*` ancestor (0: none), deadline 0 when unbounded,
ns the pid namespace."""
REAP_GRACE_S = 10
"""SIGTERM to SIGKILL grace when a dead job's lease is broken."""
_STAT = "sed 's/.*) //' /proc/{pid}/stat 2>/dev/null"
"""/proc/<pid>/stat fields after the command name: 1 state, 2 ppid, 3 pgrp, 20 starttime."""
REAP_SH = (
    "tsfp_reap() { dir=$1; d=$dir" + HOLDERS_SUFFIX + '; [ -d "$d" ] || return 0; '
    'lock=$(readlink -f "$dir' + LEASE_SUFFIX + '") || return 0; '
    "now=$(date +%s); ns=$(readlink /proc/self/ns/pid); "
    'for f in "$d"/*; do [ -f "$f" ] || continue; '
    'read -r pid start anchor astart pgid deadline fns < "$f" || { rm -f "$f"; continue; }; '
    '[ "$fns" = "$ns" ] || continue; '
    "set -- $(" + _STAT.format(pid='"$anchor"') + "); a20=${20:-}; "
    "set -- $(" + _STAT.format(pid='"$pid"') + "); "
    'why=; if [ "${20:-}" != "$start" ]; then why=holder-gone; '
    'elif [ "$anchor" != 0 ] && [ "$a20" != "$astart" ]; then why=orphaned; '
    'elif [ "$deadline" -gt 0 ] && [ "$now" -gt "$deadline" ]; then why=overdue; fi; '
    '[ -n "$why" ] || continue; victims=; '
    "for p in /proc/[0-9]*; do "
    '[ "$(' + _STAT.format(pid='"${p#/proc/}"') + ' | cut -d" " -f3)" = "$pgid" ] || continue; '
    'for l in "$p"/fd/*; do [ "$(readlink "$l" 2>/dev/null)" = "$lock" ] '
    '&& { victims="$victims ${p#/proc/}"; break; }; done; done; '
    'if [ -n "$victims" ]; then '
    'echo "@@LEASE_REAPED $dir $why pid=$pid victims=$victims" >&2; '
    "kill -TERM $victims 2>/dev/null; i=0; while [ $i -lt " + str(REAP_GRACE_S) + " ]; do "
    "alive=; for v in $victims; do "  # a zombie (Z) or dead (X) victim is gone
    'case "$(' + _STAT.format(pid="$v") + ' | cut -d" " -f1)" in ""|Z|X) ;; *) alive=1;; esac; '
    "done; "
    '[ -n "$alive" ] || break; sleep 1; i=$((i + 1)); done; '
    "kill -KILL $victims 2>/dev/null; fi; "
    'rm -f "$f"; done; return 0; }; '
)
"""Shell function `tsfp_reap <dir>`: break the lease of a job that can no longer finish.

A holder record in this pid namespace whose holder is gone (leftover children still hold the
inherited fd), orphaned (its ssh session, the `sshd*` ancestor, is gone: the caller gave up) or
past its deadline: every process of the holder's process group with the lock file open gets
SIGTERM, SIGKILL after REAP_GRACE_S, and the record goes. A job within its deadline whose
session lives, or a holder in another pid namespace (a build container), is left alone."""
LEASE_ANCHOR = "sshd"
"""Command-name prefix of the ancestor whose exit orphans an attached job."""


def lease_hold(
    path: str, fd: int = 6, wait_s: int = 60, *, deadline_s: int = 0, attached: bool = False
) -> str:
    """Shell: hold a shared lease on `path` (already shell-quoted) until the script exits.

    Every child inherits the fd, so the lease lasts as long as the job; host cleaners take the
    lease exclusively and non-blocking (`LEASED_SH`, `leased`) and skip the path while held.
    `deadline_s` (the job's own limit) and `attached` (the job ends with its ssh session) go
    into a holder record, so the next sync breaks a lease its job can no longer use
    (`REAP_SH`; deck 2026-10-26: a verify left running after its caller gave up blocked the
    next syncs)."""
    lock = f"{path}{LEASE_SUFFIX}"
    holders = f"{path}{HOLDERS_SUFFIX}"
    anchor = (
        'a=$PPID; anchor=0; while [ "$a" -gt 1 ]; do '
        f'case "$(cat /proc/$a/comm 2>/dev/null)" in {LEASE_ANCHOR}*) anchor=$a; break;; esac; '
        "set -- $(" + _STAT.format(pid="$a") + "); a=${2:-0}; done; "
        if attached
        else "anchor=0; "
    )
    note = (
        f"mkdir -p {holders} && ( {anchor}"
        "set -- $(" + _STAT.format(pid="$anchor") + "); astart=${20:-0}; "
        "set -- $(" + _STAT.format(pid="$$") + "); "
        'printf \'%s %s %s %s %s %s %s\\n\' $$ "${20}" $anchor $astart "$3" '
        f'"$(( {deadline_s} > 0 ? $(date +%s) + {deadline_s} : 0 ))" '
        f'"$(readlink /proc/self/ns/pid)" > {holders}/$$ )'
    )
    return f"exec {fd}>{lock} && flock -s -w {wait_s} {fd} && touch {lock} && {note}"


def lease_wrap(path: str, wait_s: int = 60) -> str:
    """Shell prefix holding a shared lease on `path` for ONE command (a pull's rsync-path)."""
    return f"flock -s -w {wait_s} {path}{LEASE_SUFFIX}"


def leased(path: Path, now: float | None = None) -> bool:
    """True while a job holds `path`'s lease, or its lease or marker is younger than
    LEASE_FRESH_S (the gap between a sync and the job taking the lease)."""
    now = time.time() if now is None else now
    lease = path.with_name(path.name + LEASE_SUFFIX)
    for fresh in (lease, path / MARKER):
        with contextlib.suppress(OSError):
            if now - fresh.stat().st_mtime < LEASE_FRESH_S:
                return True
    if not lease.exists():
        return False
    try:
        with lease.open("a") as handle:
            fcntl.flock(handle, fcntl.LOCK_EX | fcntl.LOCK_NB)
            fcntl.flock(handle, fcntl.LOCK_UN)
    except OSError:
        return True
    return False


def identity_exports(commit: str, tree: str) -> str:
    """Shell prefix giving a remote job its source identity without host git."""
    return f"export TSFP_SOURCE_COMMIT={shlex.quote(commit)} TSFP_SOURCE_TREE={shlex.quote(tree)}"


def _stat(pattern: str, text: str) -> int | None:
    found = re.search(pattern + r":\s+([\d,]+)", text)
    return int(found.group(1).replace(",", "")) if found else None


def record(result: SyncResult, ok: bool, path: Path | None = None) -> None:
    path = path or log_path()
    with contextlib.suppress(OSError):
        path.parent.mkdir(parents=True, exist_ok=True)
        row = {"ts": round(time.time(), 3), "ok": ok, **asdict(result)}
        row["errors"] = [error[-200:] for error in result.errors]
        with path.open("a") as stream:
            stream.write(json.dumps(row) + "\n")


def sync(
    endpoint: Endpoint,
    root: Path,
    tree: str,
    *,
    commit: str = "",
    protect: Sequence[str] = (),
    run: Runner = _run,
    mirror_base: Path | None = None,
    log: Path | None = None,
) -> SyncResult:
    """Place `tree` exactly at `endpoint.dest`; raise ExecSyncError when every way failed."""
    started = time.monotonic()
    marker = read_marker(endpoint, run)
    old_tree = marker.get("tree") if marker else None
    changed = delta_bytes(root, str(old_tree) if old_tree else None, tree)
    limit = sync_timeout(changed)
    result = SyncResult(endpoint.name, endpoint.dest, tree, commit, "rsync", 0.0, limit, changed)
    try:  # a job still running in the dir holds its lease: wait here, not inside rsync
        free = run(wait_free_argv(endpoint, limit), timeout=limit + 30)
        busy = f"exit {free.returncode}" if free.returncode else ""
    except (subprocess.SubprocessError, OSError) as error:
        busy = type(error).__name__
    if busy:
        result.errors.append(f"lease: the dir stayed in use ({busy})")
        result.seconds = round(time.monotonic() - started, 2)
        record(result, False, log)
        # flock -w gives up with exit 1; 255 is ssh itself (transport), a timeout is local
        error_type = ExecSyncBusy if busy == "exit 1" else ExecSyncError
        raise error_type(f"sync to {endpoint.name}: {result.errors[-1]}")
    with mirror(mirror_base) as source:
        export_tree(root, tree, source)
        for attempt in ("rsync", "rsync-retry"):
            try:
                done = run(rsync_argv(endpoint, source, limit, protect), timeout=limit + 30)
            except (subprocess.SubprocessError, OSError) as error:
                result.errors.append(f"{attempt}: {type(error).__name__}")
                continue
            if done.returncode == 0:
                result.mode = attempt
                result.bytes_sent = _stat("Total bytes sent", done.stdout)
                result.files_transferred = _stat("Number of regular files transferred", done.stdout)
                break
            result.errors.append(f"{attempt}: exit {done.returncode} {done.stderr.strip()[-160:]}")
        else:
            whole = sync_timeout(tree_bytes(root, tree))
            try:
                done = run(tar_argv(endpoint, source, whole), timeout=whole + 30)
                if done.returncode:
                    result.errors.append(
                        f"tar: exit {done.returncode} {done.stderr.strip()[-160:]}"
                    )
                else:
                    result.mode = "tar"
            except (subprocess.SubprocessError, OSError) as error:
                result.errors.append(f"tar: {type(error).__name__}")
            if result.mode != "tar":
                result.seconds = round(time.monotonic() - started, 2)
                record(result, False, log)
                raise ExecSyncError(f"sync to {endpoint.name} failed: {'; '.join(result.errors)}")
    try:
        marked = run(write_marker_argv(endpoint, commit, tree), timeout=30)
        if marked.returncode:
            result.errors.append(f"marker: exit {marked.returncode}")
    except (subprocess.SubprocessError, OSError) as error:
        result.errors.append(f"marker: {type(error).__name__}")
    result.seconds = round(time.monotonic() - started, 2)
    record(result, True, log)
    return result


def tree_of(root: Path, rev: str) -> str:
    done = _git(root, "rev-parse", "--verify", f"{rev}^{{tree}}", timeout=30)
    tree = done.stdout.strip()
    if done.returncode or not _TREE.fullmatch(tree):
        raise ExecSyncError(f"cannot resolve the tree of {rev!r}")
    return tree


def read_log(path: Path | None = None, since: float = 0.0) -> list[dict[str, Any]]:
    rows = []
    try:
        lines = (path or log_path()).read_text(errors="replace").splitlines()
    except OSError:
        return []
    for line in lines:
        with contextlib.suppress(ValueError):
            row = json.loads(line)
            if isinstance(row, dict) and float(row.get("ts", 0)) >= since:
                rows.append(row)
    return rows


def stats(rows: Sequence[dict[str, Any]]) -> dict[str, Any]:
    """Per host: syncs, failures, retries, tar fallbacks, p50/p90/max seconds."""
    out: dict[str, Any] = {}
    for host in sorted({str(row.get("host")) for row in rows}):
        mine = [row for row in rows if str(row.get("host")) == host]
        seconds = sorted(float(row.get("seconds", 0)) for row in mine if row.get("ok"))
        out[host] = {
            "syncs": len(mine),
            "failed": sum(1 for row in mine if not row.get("ok")),
            "retried": sum(1 for row in mine if row.get("mode") == "rsync-retry"),
            "tar": sum(1 for row in mine if row.get("mode") == "tar" and row.get("ok")),
            "p50_s": seconds[len(seconds) // 2] if seconds else None,
            "p90_s": seconds[int(len(seconds) * 0.9)] if seconds else None,
            "max_s": seconds[-1] if seconds else None,
        }
    return out


def failing_hosts(rows: Sequence[dict[str, Any]], *, min_failures: int = 2) -> dict[str, int]:
    """Hosts whose most recent syncs failed `min_failures` times in a row (newest first)."""
    streaks: dict[str, int] = {}
    finished: set[str] = set()
    for row in sorted(rows, key=lambda item: float(item.get("ts", 0)), reverse=True):
        host = str(row.get("host"))
        if host in finished:
            continue
        if row.get("ok"):
            finished.add(host)
        else:
            streaks[host] = streaks.get(host, 0) + 1
    return {host: count for host, count in streaks.items() if count >= min_failures}


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        prog="python3 -m tools.wt.exec_sync", description=__doc__.split("\n")[0]
    )
    sub = parser.add_subparsers(dest="command", required=True)
    push = sub.add_parser("push", help="sync a rev's tree to a host exec dir and time it")
    push.add_argument("host")
    push.add_argument("--rev", default="HEAD")
    push.add_argument("--dest", help="exec dir on the host (default <canon_root>/exec/manual)")
    report = sub.add_parser("stats", help="per-host sync stats from the log")
    report.add_argument("--hours", type=float, default=24.0)
    args = parser.parse_args(argv)
    if args.command == "stats":
        rows = read_log(since=time.time() - args.hours * 3600)
        print(json.dumps(stats(rows), indent=1))
        return 0
    from tools.wt import remote_exec, remote_replay

    host = remote_exec.find_host(args.host)
    root = Path.cwd()
    dest = args.dest or f"{host.canon_root or host.workdir}/exec/manual"
    worker = host.worker()
    endpoint = Endpoint(
        host.name, host.target, host.port, dest, remote_replay._ssh_transport(worker)
    )
    commit = _git(root, "rev-parse", "--verify", f"{args.rev}^{{commit}}").stdout.strip()
    result = sync(endpoint, root, tree_of(root, args.rev), commit=commit)
    print(json.dumps(asdict(result)))
    return 0


if __name__ == "__main__":
    sys.exit(main())
