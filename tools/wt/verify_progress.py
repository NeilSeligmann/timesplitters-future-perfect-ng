"""Step progress of one `remote_exec.run_verify` (T3678, incident I-T3678-landing-stall).

Two remote verifies sat silent until SIGTERM at 7200 s because `merge_task` logged nothing
between `trying <host>` and the result. Every step now prints `start` and `done <s>` lines to
stderr and records its name and start time in `<progress dir>/<pid>.json`; the file is removed
when the verify ends. `tools.fleet.detectors.detect_verify_step_stuck` reads these files.
"""

from __future__ import annotations

import contextlib
import json
import os
import socket
import sys
import time
from collections.abc import Callable, Iterator
from pathlib import Path

ENV_DIR = "TSFP_VERIFY_PROGRESS_DIR"
STUCK_AFTER_S = 1200
"""A step running longer than this with a live owner is reported by the HEALTH detector."""


def pid_namespace(proc: Path = Path("/proc")) -> str | None:
    """Return this process's PID namespace identity when procfs is available."""
    try:
        return os.readlink(proc / "self" / "ns" / "pid")
    except OSError:
        return None


def progress_dir() -> Path:
    override = os.environ.get(ENV_DIR)
    if override:
        return Path(override)
    from tools import datadir

    return datadir.data_dir("remote-verify-progress")


def pid_alive(pid: int, namespace: str | None) -> bool | None:
    """Return liveness only when the record proves it shares this PID namespace.

    ``None`` means the namespace is unknown or foreign, so callers must use age.
    """
    current_namespace = pid_namespace()
    if not namespace or not current_namespace or namespace != current_namespace:
        return None
    try:
        os.kill(pid, 0)
    except ProcessLookupError:
        return False
    except OSError:
        return True
    return True


def read_all(directory: Path) -> list[dict]:
    """Every progress record under `directory` (unreadable files are skipped)."""
    records = []
    for path in sorted(directory.glob("*.json")) if directory.is_dir() else []:
        try:
            record = json.loads(path.read_text())
        except (OSError, ValueError):
            continue
        if isinstance(record, dict) and "pid" in record:
            records.append({**record, "file": str(path)})
    return records


class Progress:
    """Prints and records the steps of one verify on `host`."""

    def __init__(
        self,
        host: str,
        *,
        directory: Path | None = None,
        out: Callable[[str], None] | None = None,
        clock: Callable[[], float] = time.monotonic,
        wall: Callable[[], float] = time.time,
        pid: int | None = None,
        namespace: str | None = None,
        hostname: str | None = None,
    ) -> None:
        self.host = host
        self.out = out or self._stderr
        self.clock, self.wall = clock, wall
        self.pid = pid if pid is not None else os.getpid()
        self.namespace = namespace if namespace is not None else pid_namespace()
        self.hostname = hostname if hostname is not None else socket.gethostname()
        self.directory = directory
        self.step_name = ""
        self.steps: list[dict[str, float | str]] = []

    @staticmethod
    def _stderr(line: str) -> None:
        print(line, file=sys.stderr, flush=True)

    def _path(self) -> Path | None:
        if self.directory is None:
            try:
                self.directory = progress_dir()
            except Exception:  # noqa: BLE001 - progress is advisory, never fail a verify
                return None
        return self.directory / f"{self.pid}.json"

    def _write(self, step: str) -> None:
        path = self._path()
        if path is None:
            return
        record = {
            "pid": self.pid,
            "pid_namespace": self.namespace,
            "hostname": self.hostname,
            "host": self.host,
            "step": step,
            "started": self.wall(),
        }
        with contextlib.suppress(OSError):
            path.parent.mkdir(parents=True, exist_ok=True)
            temporary = path.with_suffix(".tmp")
            temporary.write_text(json.dumps(record))
            temporary.replace(path)

    def say(self, text: str) -> None:
        self.out(f"merge_task: remote verify: {self.host} {text}")

    @contextlib.contextmanager
    def step(self, name: str) -> Iterator[Callable[[str], None]]:
        """Announce `name`, record it, and announce its end (with the detail given to `note`)."""
        detail = [""]

        def note(text: str) -> None:
            detail[0] = f" ({text})"

        started = self.clock()
        self.step_name = name
        self._write(name)
        self.say(f"{name} start")
        try:
            yield note
        except BaseException as error:
            elapsed = round(self.clock() - started, 3)
            self.steps.append({"step": name, "seconds": elapsed, "status": "failed"})
            self.say(f"{name} failed {elapsed:.1f} s ({type(error).__name__})")
            raise
        elapsed = round(self.clock() - started, 3)
        self.steps.append({"step": name, "seconds": elapsed, "status": "done"})
        self.say(f"{name} done {elapsed:.1f} s{detail[0]}")

    def remote(self, line: str) -> None:
        """A `@@STEP` marker echoed by the remote script, streamed while it runs."""
        self.say(f"remote: {line.removeprefix('@@STEP ').strip()}")

    def close(self) -> None:
        path = self._path()
        if path is not None:
            with contextlib.suppress(OSError):
                path.unlink()
