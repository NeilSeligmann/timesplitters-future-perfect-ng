"""Content-addressed artifact store (T3621; re-homed on the owner PC by T3760).

Artifacts are staged locally, checksummed before publication, and copied to a temporary path
before their digest is trusted; no owner inputs are copied to the store.

T3760 (remote hosts are execution-only, docs/decisions/remote-hosts-execution-only.md): the store
lives on the owner PC's data disk, `tmp/wt-cache/shared-artifacts` of the main checkout (counted
in the tools.datadisk project budget; `TSFP_SHARED_ARTIFACTS_PATH` overrides the directory),
LRU-capped at `TSFP_SHARED_ARTIFACTS_CAP_GB` (default STORE_CAP_GB). Every owner-PC worktree and
every build the owner PC fetches back from a host (remote prebuilt) publishes into it and
restores from it. A remote exec tree (TSFP_SOURCE_TREE set) owns no store: the owner PC ships
what a job needs by rsync. `TSFP_SHARED_ARTIFACTS_SSH=<user@host>` plus the PATH still selects a
store on another machine over ssh; `TSFP_SHARED_ARTIFACTS=off` disables it.
"""

from __future__ import annotations

import contextlib
import fcntl
import hashlib
import os
import select
import shlex
import shutil
import subprocess
import sys
import tarfile
import tempfile
import time
from collections.abc import Iterator
from contextlib import contextmanager
from pathlib import Path

STORE_REL = "tmp/wt-cache/shared-artifacts"
STORE_CAP_GB = 80.0
LOCK_WAIT_S = 3600


def local_store() -> Path | None:
    """The owner-PC store directory (data disk), or None when the disk rules refuse it."""
    override = os.environ.get("TSFP_SHARED_ARTIFACTS_PATH", "").strip()
    if override:
        return Path(override)
    from tools import datadisk

    try:
        return datadisk.resolve(STORE_REL, what="shared artifact store")
    except (datadisk.DiskRefused, OSError) as error:
        print(f"shared artifacts: store unavailable: {error}", file=sys.stderr)
        return None


def _endpoint() -> tuple[str, str] | None:
    """`(ssh host, dir)` of the store; host "" is the local owner-PC store (T3760)."""
    if os.environ.get("TSFP_SHARED_ARTIFACTS", "").lower() in {"off", "0", "no"}:
        return None
    host = os.environ.get("TSFP_SHARED_ARTIFACTS_SSH", "").strip()
    if host:
        path = os.environ.get("TSFP_SHARED_ARTIFACTS_PATH", "").strip()
        return (host, path) if path else None
    if os.environ.get("TSFP_SOURCE_TREE"):
        return None  # a remote exec tree: hosts own no store, the owner PC ships artifacts
    store = local_store()
    return ("", str(store)) if store is not None else None


def _digest(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def _remote_path(base: str, kind: str, key: str, suffix: str) -> str:
    if kind not in {
        "lift",
        "prebuilt",
        "configure-generators",
        "test-select",
        "build-verdict",
    }:
        raise ValueError(f"unsupported shared artifact kind: {kind}")
    if not key or any(c not in "0123456789abcdef-" for c in key):
        raise ValueError("artifact key must be lowercase hex")
    return f"{base.rstrip('/')}/{kind}/{key}{suffix}"


@contextmanager
def build_verdict_lock(key: str) -> Iterator[bool]:
    """Single-flight a content-keyed build verdict through the shared artifact host.

    If the endpoint is disabled or cannot be reached, yield False and let the caller use its
    ordinary local/remote build path. A live SSH session holds the remote flock until the caller
    exits the context, so concurrent hosts wait for the first successful receipt.
    """
    endpoint = _endpoint()
    if endpoint is None:
        yield False
        return
    host, base = endpoint
    lock_path = _remote_path(base, "build-verdict", key, ".lock")
    parent = str(Path(lock_path).parent)
    if not host:
        with _local_lock(Path(lock_path)) as locked:
            yield locked
        return
    script = (
        f"set -eu; mkdir -p {shlex.quote(parent)}; exec 9>{shlex.quote(lock_path)}; "
        "flock -x 9; printf 'READY\\n'; cat >/dev/null"
    )
    try:
        process = subprocess.Popen(
            [
                "ssh",
                "-o",
                "BatchMode=yes",
                "-o",
                "ConnectTimeout=5",
                host,
                f"sh -c {shlex.quote(script)}",
            ],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
    except OSError:
        yield False
        return
    acquired = False
    deadline = time.monotonic() + 3600
    try:
        while time.monotonic() < deadline:
            if process.stdout is None:
                break
            readable, _, _ = select.select([process.stdout], [], [], 1)
            if readable:
                acquired = process.stdout.readline() == b"READY\n"
                break
            if process.poll() is not None:
                break
        yield acquired
    finally:
        if process.stdin is not None:
            try:
                process.stdin.close()
            except BrokenPipeError:
                pass
        try:
            process.wait(timeout=30)
        except subprocess.TimeoutExpired:
            process.terminate()
            process.wait(timeout=5)


def _ssh(host: str, *args: str) -> subprocess.CompletedProcess:
    return subprocess.run(
        ["ssh", "-o", "BatchMode=yes", "-o", "ConnectTimeout=5", host, *args],
        capture_output=True,
        text=True,
    )


def _rsync(source: str, destination: str) -> subprocess.CompletedProcess:
    return subprocess.run(
        ["rsync", "-a", "-e", "ssh -o BatchMode=yes -o ConnectTimeout=5", source, destination],
        capture_output=True,
    )


def fetch_file(kind: str, key: str, destination: Path) -> bool:
    """Fetch a file and verify its remote SHA-256 sidecar before publishing locally."""
    endpoint = _endpoint()
    if endpoint is None:
        return False
    host, base = endpoint
    remote = _remote_path(base, kind, key, ".tar")
    remote_hash = _remote_path(base, kind, key, ".sha256")
    destination.parent.mkdir(parents=True, exist_ok=True)
    if not host:
        return _fetch_local(Path(remote), Path(remote_hash), destination)
    with tempfile.TemporaryDirectory(prefix=".shared-fetch-", dir=destination.parent) as temp:
        archive = Path(temp) / Path(remote).name
        result = subprocess.run(
            [
                "rsync",
                "-a",
                "-e",
                "ssh -o BatchMode=yes -o ConnectTimeout=5",
                f"{host}:{remote}",
                f"{host}:{remote_hash}",
                f"{temp}/",
            ],
            capture_output=True,
        )
        if result.returncode:
            return False
        checksum_path = Path(temp) / Path(remote_hash).name
        if not checksum_path.is_file():
            return False
        expected = checksum_path.read_text().strip().split()[0]
        if len(expected) != 64 or _digest(archive) != expected:
            return False
        os.replace(archive, destination)
    return True


def already_published(kind: str, key: str) -> bool:
    endpoint = _endpoint()
    if endpoint is None:
        return False
    host, base = endpoint
    archive = _remote_path(base, kind, key, ".tar")
    checksum = _remote_path(base, kind, key, ".sha256")
    if not host:
        return _verified(Path(archive), Path(checksum))
    script = (
        f"test -s {shlex.quote(archive)} && test -s {shlex.quote(checksum)} && "
        f"test \"$(sha256sum {shlex.quote(archive)} | cut -d ' ' -f 1)\" = "
        f"\"$(cut -d ' ' -f 1 {shlex.quote(checksum)})\""
    )
    result = _ssh(host, "sh", "-c", shlex.quote(script))
    return result.returncode == 0


def publish_file(kind: str, key: str, source: Path) -> bool:
    """Publish an immutable file through remote temporary names and atomic rename."""
    endpoint = _endpoint()
    if endpoint is None:
        return False
    host, base = endpoint
    remote = _remote_path(base, kind, key, ".tar")
    checksum_path = _remote_path(base, kind, key, ".sha256")
    if not host:
        published = _publish_local(source, Path(remote), Path(checksum_path))
        if published:
            gc(Path(base))
        return published
    token = f"{os.getpid()}-{_digest(source)[:12]}"
    temp_remote = f"{remote}.tmp-{token}"
    digest = _digest(source)
    script = (
        "set -eu; "
        f"mkdir -p {shlex.quote(str(Path(remote).parent))}; "
        f"tmp={shlex.quote(temp_remote)}; "
        f"sumtmp={shlex.quote(checksum_path + '.tmp-' + token)}; "
        'trap \'rm -f "$tmp" "$sumtmp"\' EXIT; '
        'cat > "$tmp"; '
        f'test "$(sha256sum "$tmp" | cut -d \' \' -f 1)" = {shlex.quote(digest)}; '
        f"printf '%s\\n' {shlex.quote(digest)} > \"$sumtmp\"; "
        f'mv "$tmp" {shlex.quote(remote)}; '
        f'mv "$sumtmp" {shlex.quote(checksum_path)}; '
        "trap - EXIT"
    )
    with source.open("rb") as stream:
        result = subprocess.run(
            [
                "ssh",
                "-o",
                "BatchMode=yes",
                "-o",
                "ConnectTimeout=5",
                host,
                "sh",
                "-c",
                shlex.quote(script),
            ],
            stdin=stream,
            capture_output=True,
            timeout=3600,
        )
    return result.returncode == 0


def pack_tree(source: Path, archive: Path) -> None:
    with tarfile.open(archive, "w:gz") as stream:
        stream.add(source, arcname=".")


def unpack_tree(archive: Path, destination: Path) -> bool:
    """Extract only regular files/directories with paths contained in destination."""
    destination.mkdir(parents=True, exist_ok=True)
    root = destination.resolve()
    try:
        with tarfile.open(archive, "r:gz") as stream:
            members = stream.getmembers()
            for member in members:
                target = (destination / member.name).resolve()
                if target != root and root not in target.parents:
                    return False
                if not (member.isfile() or member.isdir()):
                    return False
            stream.extractall(destination, members=members, filter="data")
    except (OSError, tarfile.TarError, ValueError):
        return False
    return True


# T3760: the local owner-PC store -------------------------------------------------------------


def _verified(archive: Path, checksum: Path) -> bool:
    try:
        expected = checksum.read_text().strip().split()[0]
    except (OSError, IndexError):
        return False
    return len(expected) == 64 and archive.is_file() and _digest(archive) == expected


def _fetch_local(archive: Path, checksum: Path, destination: Path) -> bool:
    if not _verified(archive, checksum):
        return False
    with tempfile.TemporaryDirectory(prefix=".shared-fetch-", dir=destination.parent) as temp:
        copy = Path(temp) / archive.name
        shutil.copyfile(archive, copy)
        if not _verified(copy, checksum):
            return False
        os.replace(copy, destination)
    with contextlib.suppress(OSError):
        os.utime(archive)  # LRU: a restored artifact is recently used
    return True


def _publish_local(source: Path, archive: Path, checksum: Path) -> bool:
    """Atomic: the archive and its sidecar appear by rename, the sidecar last."""
    digest = _digest(source)
    archive.parent.mkdir(parents=True, exist_ok=True)
    token = f"{os.getpid()}-{digest[:12]}"
    temp_archive = archive.with_name(f"{archive.name}.tmp-{token}")
    temp_sum = checksum.with_name(f"{checksum.name}.tmp-{token}")
    try:
        shutil.copyfile(source, temp_archive)
        if _digest(temp_archive) != digest:
            return False
        temp_sum.write_text(f"{digest}\n")
        os.replace(temp_archive, archive)
        os.replace(temp_sum, checksum)
    except OSError:
        return False
    finally:
        temp_archive.unlink(missing_ok=True)
        temp_sum.unlink(missing_ok=True)
    return True


@contextmanager
def _local_lock(path: Path, wait_s: float = LOCK_WAIT_S) -> Iterator[bool]:
    try:
        path.parent.mkdir(parents=True, exist_ok=True)
        handle = path.open("a")
    except OSError:
        yield False
        return
    with handle:
        deadline = time.monotonic() + wait_s
        while True:
            try:
                fcntl.flock(handle, fcntl.LOCK_EX | fcntl.LOCK_NB)
                break
            except OSError:
                if time.monotonic() >= deadline:
                    yield False
                    return
                time.sleep(1.0)
        try:
            yield True
        finally:
            fcntl.flock(handle, fcntl.LOCK_UN)


def gc(base: Path, cap_gb: float | None = None) -> list[Path]:
    """Drop the least recently used archives (and sidecars) until the store fits its cap."""
    if cap_gb is None:
        cap_gb = float(os.environ.get("TSFP_SHARED_ARTIFACTS_CAP_GB", STORE_CAP_GB))
    archives = []
    for path in base.glob("*/*.tar"):
        with contextlib.suppress(OSError):
            stat = path.stat()
            archives.append((stat.st_mtime, stat.st_size, path))
    total = sum(size for _mtime, size, _path in archives)
    removed = []
    for _mtime, size, path in sorted(archives):
        if total <= cap_gb * 1024**3:
            break
        path.with_suffix(".sha256").unlink(missing_ok=True)  # sidecar first: never half-valid
        path.unlink(missing_ok=True)
        total -= size
        removed.append(path)
    return removed
