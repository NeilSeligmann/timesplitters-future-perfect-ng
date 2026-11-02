"""Shared primitives of tools.fleet: findings, command runner, ssh helpers, flock liveness.

Everything with side effects goes through a `Runner` so tests and the controller's dry-run mode can
substitute a recorder. Liveness checks use flock on a lock file, never `kill -0` or `ps`: a PID is
meaningless across the PID namespaces of the several containers that share this disk (incident
register T3505, `docs/t3505-fleet-incident-register.md`).
"""

from __future__ import annotations

import fcntl
import os
import shlex
import subprocess
import time
from collections.abc import Callable, Sequence
from dataclasses import dataclass
from pathlib import Path


@dataclass(frozen=True)
class Finding:
    """One health finding. `ident` is the stable signature the rea-wake watcher keys on."""

    ident: str
    detail: str

    def line(self, stamp: str | None = None) -> str:
        return f"{stamp or time.strftime('%T')} HEALTH {self.ident} {self.detail}"


@dataclass(frozen=True)
class CmdResult:
    rc: int
    out: str = ""
    err: str = ""

    @property
    def ok(self) -> bool:
        return self.rc == 0


Runner = Callable[[Sequence[str], float], CmdResult]
"""`runner(argv, timeout_seconds) -> CmdResult`. Never raises: a timeout is rc 124."""


def run_local(argv: Sequence[str], timeout: float = 60, cwd: Path | str | None = None) -> CmdResult:
    """Run a command with a hard timeout (process group kill). Never raises."""
    try:
        proc = subprocess.run(
            list(argv),
            capture_output=True,
            text=True,
            timeout=timeout,
            cwd=cwd,
            stdin=subprocess.DEVNULL,
            check=False,
        )
    except subprocess.TimeoutExpired as error:
        return CmdResult(124, _text(error.stdout), _text(error.stderr) + "timeout")
    except OSError as error:
        return CmdResult(127, "", str(error))
    return CmdResult(proc.returncode, proc.stdout, proc.stderr)


def _text(value: bytes | str | None) -> str:
    if value is None:
        return ""
    return value.decode("utf-8", "replace") if isinstance(value, bytes) else value


def local_runner(cwd: Path | str | None = None) -> Runner:
    return lambda argv, timeout=60: run_local(argv, timeout, cwd)


def ssh_argv(target: str, port: int, remote_command: str, connect_timeout: int = 8) -> list[str]:
    """Argv for a non-interactive ssh to `target` (user@host or host) running `remote_command`."""
    return [
        "ssh",
        "-o",
        "BatchMode=yes",
        "-o",
        f"ConnectTimeout={connect_timeout}",
        "-p",
        str(port),
        target,
        remote_command,
    ]


def scp_argv(target: str, port: int, sources: Sequence[str | Path], destination: str) -> list[str]:
    return [
        "scp",
        "-q",
        "-o",
        "BatchMode=yes",
        "-P",
        str(port),
        *map(str, sources),
        f"{target}:{destination}",
    ]


def quote(value: str) -> str:
    return shlex.quote(value)


def flock_held(path: Path | str) -> bool:
    """True when another open file description holds an flock on `path` (the owner is alive).

    A missing file means nobody holds it. The probe takes and releases a non-blocking shared
    lock, so it never disturbs the holder. This works across containers sharing the disk.
    """
    try:
        fd = os.open(str(path), os.O_RDONLY)
    except OSError:
        return False
    try:
        try:
            fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except OSError:
            return True
        fcntl.flock(fd, fcntl.LOCK_UN)
        return False
    finally:
        os.close(fd)


class SingleInstance:
    """Hold an exclusive flock for the life of the process (single controller per state dir)."""

    def __init__(self, path: Path) -> None:
        self.path = path
        self._fd: int | None = None

    def acquire(self) -> bool:
        self.path.parent.mkdir(parents=True, exist_ok=True)
        fd = os.open(str(self.path), os.O_RDWR | os.O_CREAT, 0o644)
        try:
            fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except OSError:
            os.close(fd)
            return False
        os.ftruncate(fd, 0)
        os.write(fd, f"{os.getpid()}\n".encode())
        self._fd = fd
        return True

    def adopt(self, fd: int) -> bool:
        """Adopt this process's inherited lock after an exec-based controller restart."""
        try:
            held = os.fstat(fd)
            target = self.path.stat()
        except OSError:
            return False
        if (held.st_dev, held.st_ino) != (target.st_dev, target.st_ino):
            return False
        self._fd = fd
        os.ftruncate(fd, 0)
        os.write(fd, f"{os.getpid()}\n".encode())
        return True

    def prepare_exec(self) -> int:
        """Keep the lock open across exec and return the descriptor to pass to the child."""
        if self._fd is None:
            raise RuntimeError("cannot hand off an unheld singleton lock")
        os.set_inheritable(self._fd, True)
        return self._fd

    def release(self) -> None:
        if self._fd is not None:
            os.close(self._fd)
            self._fd = None


def age_seconds(path: Path | str, now: float | None = None) -> float | None:
    """Seconds since the file was modified, or None when it does not exist."""
    try:
        mtime = Path(path).stat().st_mtime
    except OSError:
        return None
    return (now if now is not None else time.time()) - mtime


def newest_mtime(paths: Sequence[Path]) -> float | None:
    stamps = []
    for path in paths:
        try:
            stamps.append(path.stat().st_mtime)
        except OSError:
            continue
    return max(stamps) if stamps else None
