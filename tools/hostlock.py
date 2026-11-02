"""Machine-wide FIFO N-slot host admission and PID-safe stale recovery (T2223).

T3500: tickets carry the holder's PID namespace and boot id and a heartbeat refreshed while
waiting and while admitted (every tools.wt.lease.HEARTBEAT_S). A ticket of another PID
namespace (a container sharing the state directory) is kept while its heartbeat is younger
than tools.wt.lease.TTL_S instead of being dropped as "exe gone"; same-namespace tickets keep
the pid, zombie and start-tick checks.

Usage: python3 -m tools.hostlock [--wait-timeout SECONDS] -- COMMAND ...
State is shared across clones, under TSFP_HOSTLOCK_DIR or /tmp/tsfp-hostlock-UID.
"""

from __future__ import annotations

import argparse
import contextlib
import fcntl
import functools
import json
import os
import socket
import subprocess
import sys
import tempfile
import threading
import time
import uuid
from collections.abc import Callable, Iterator
from pathlib import Path
from typing import ParamSpec, TypeVar

from tools import hostcaps

P = ParamSpec("P")
R = TypeVar("R")


def dead_reason(pid: int, proc: Path = Path("/proc")) -> str | None:
    """A zombie or vanished executable cannot retain an admission slot."""
    try:
        fields = (proc / str(pid) / "stat").read_text().rsplit(")", 1)[1].split()
        if fields[0] in ("Z", "X"):
            return "zombie" if fields[0] == "Z" else "dead"
        os.readlink(proc / str(pid) / "exe")
    except (FileNotFoundError, ProcessLookupError):
        return "exe gone"
    except (OSError, IndexError):
        # Permission denial is not evidence that a live process has exited.
        return None
    return None


def kernel_pid(proc: Path = Path("/proc")) -> int:
    """The kernel PID of this process, also inside a DMTCP computation (T2479).

    MEASURED: under DMTCP `os.getpid()` is a virtual PID (40000, 41000, ...) that every
    concurrent snapshot capture shares and that other sessions see as dead, so they dropped
    its ticket ("released stale holder pid 40000") and admission failed. The first field of
    /proc/self/stat is the kernel's PID.
    """
    if os.environ.get("DMTCP_VIRTUAL_PID") is None:
        return os.getpid()
    try:
        return int((proc / "self" / "stat").read_text().split(None, 1)[0])
    except (OSError, ValueError):
        return os.getpid()


def identity(pid: int, proc: Path = Path("/proc")) -> str | None:
    """Linux start ticks distinguish PID reuse; zombies are already dead."""
    try:
        fields = (proc / str(pid) / "stat").read_text().rsplit(")", 1)[1].split()
        return fields[19] if fields[0] not in ("Z", "X") else None
    except (OSError, IndexError):
        return None


def live_hosts(proc: Path = Path("/proc")) -> list[int]:
    result = []
    for entry in proc.iterdir():
        if (
            not entry.name.isdigit()
            or identity(int(entry.name), proc) is None
            or dead_reason(int(entry.name), proc)
        ):
            continue
        try:
            name = (entry / "comm").read_text().strip()
        except OSError:
            name = ""
        try:
            executable = Path(os.readlink(entry / "exe")).name
        except OSError:
            executable = ""
        if name in ("tsfp_host", "tsfp-bevy") or executable in ("tsfp_host", "tsfp-bevy"):
            result.append(int(entry.name))
    return result


def describe(pid: int, proc: Path = Path("/proc")) -> str:
    """`pid N (command line)` for a status line; the command line is cut to 120 characters."""
    try:
        text = (proc / str(pid) / "cmdline").read_text().replace("\0", " ").strip()
    except OSError:
        text = ""
    return f"pid {pid} ({text[:120] or 'command line unreadable'})"


MEM_RESERVE_MB = 12 * 1024
"""T3390: MemAvailable left after a new job's estimated RSS (TSFP_HOST_MEM_RESERVE_MB)."""
JOB_RSS_MB = 1024
"""T3390: per-job RSS estimate (TSFP_HOST_JOB_RSS_MB). MEASURED host peaks: 197 MB headless
story-level1, 326 MB GPU arcade; the rest covers rust_trace's collector (INFERRED)."""
RAMP_SECONDS = 60.0
"""A job admitted this recently may not have reached its RSS yet, so its estimate is still held."""


def mem_available_mb(proc: Path = Path("/proc")) -> int | None:
    """MemAvailable in MB under the container's cgroup ceiling (T3844, tools.hostcaps),
    None when neither /proc/meminfo nor the cgroup has it."""
    available = hostcaps.compute_mem_available_bytes(proc=proc)
    return None if available is None else available // 2**20


def env_mb(name: str, default: int) -> int:
    value = os.environ.get(name)
    if value is None:
        return default
    try:
        number = int(value)
    except ValueError as error:
        raise ValueError(f"{name} must be a non-negative integer (MB)") from error
    if number < 0:
        raise ValueError(f"{name} must be a non-negative integer (MB)")
    return number


def memory_shortfall(available_mb: int | None, job_mb: int, ramping: int, reserve_mb: int) -> int:
    """MB missing to admit one more job (0 = admit): the new job and the jobs still ramping up
    are charged their estimate against MemAvailable, the reserve must remain."""
    if available_mb is None:
        return 0
    return max(0, reserve_mb + job_mb * (1 + ramping) - available_mb)


def disabled() -> bool:
    """TSFP_HOSTLOCK=off skips this queue; the local game slot (T3823) still applies."""
    return os.environ.get("TSFP_HOSTLOCK", "").lower() in ("off", "0", "no")


@contextlib.contextmanager
def admission(enabled: bool = True) -> Iterator[None]:
    """Admission for one host run; none for an interactive play (T2233) or TSFP_HOSTLOCK=off."""
    from tools.hostresources import launch_scope

    submitted = time.monotonic()
    if not enabled or disabled():
        admitted = time.monotonic()
        try:
            with launch_scope():
                yield
        finally:
            _write_capture_timing(submitted, admitted, time.monotonic())
    else:
        with HostLock() as lock:
            admitted = time.monotonic()
            try:
                with launch_scope(lock.slot):
                    yield
            finally:
                _write_capture_timing(submitted, admitted, time.monotonic())


def _write_capture_timing(submitted: float, admitted: float, finished: float) -> None:
    """Persist the host-slot wait and admitted run duration for replay receipts."""
    target = os.environ.get("TSFP_CAPTURE_TIMING_FILE")
    if not target:
        return
    try:
        path = Path(target)
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(
            json.dumps(
                {
                    "queue_wait_seconds": round(max(0.0, admitted - submitted), 3),
                    "run_seconds": round(max(0.0, finished - admitted), 3),
                    "host_name": socket.gethostname(),
                    "timing_source": "hostlock admission",
                },
                sort_keys=True,
            )
            + "\n"
        )
    except OSError:
        # Diagnostics must never make a successful capture fail.
        return


class HostLock:
    """One admission ticket, retained through child cleanup; never kills another agent."""

    def __init__(
        self,
        directory: Path | None = None,
        timeout: float | None = None,
        proc: Path = Path("/proc"),
        poll: float = 0.2,
        announce: Callable[[str], None] | None = None,
        announce_after: float = 2.0,
        announce_every: float = 15.0,
        slots: int | None = None,
        serial: bool = False,
        clock: Callable[[], float] = time.monotonic,
        sleep: Callable[[float], None] = time.sleep,
    ) -> None:
        self.slots = int(os.environ.get("TSFP_HOST_SLOTS", "1")) if slots is None else slots
        if type(self.slots) is not int or self.slots < 1:
            raise ValueError("TSFP_HOST_SLOTS must be a positive integer")
        self.serial = serial or os.environ.get("TSFP_HOST_SERIAL", "") == "1"
        self.clock = clock
        self.sleep = sleep
        self.slot: int | None = None
        self.announce = announce or (lambda line: print(line, file=sys.stderr, flush=True))
        self.announce_after = announce_after
        self.announce_every = announce_every
        self.directory = directory or Path(
            os.environ.get("TSFP_HOSTLOCK_DIR")
            or Path(tempfile.gettempdir()) / f"tsfp-hostlock-{os.getuid()}"
        )
        self.timeout = timeout
        self.proc = proc
        self.job_mb = env_mb("TSFP_HOST_JOB_RSS_MB", JOB_RSS_MB)
        self.reserve_mb = env_mb("TSFP_HOST_MEM_RESERVE_MB", MEM_RESERVE_MB)
        self.memory_note = ""
        self.poll = poll
        owner = kernel_pid(proc)
        from tools.wt import lease

        where = lease.namespace()
        self._beat_stop = threading.Event()
        self._beat_thread: threading.Thread | None = None
        self.ticket = {
            "id": uuid.uuid4().hex,
            "pid": owner,
            "start": identity(owner, proc),
            "serial": self.serial,
            "slots": self.slots,
            "admitted_at": time.time(),
            "pidns": where["pidns"],
            "boot": where["boot"],
            "host": where["host"],
            "heartbeat": time.time(),
            "task": next(
                (
                    part
                    for part in Path.cwd().parts
                    if part.startswith("T") and part[1:].split("-")[0].isdigit()
                ),
                "unknown",
            ),
        }

    @contextlib.contextmanager
    def state(self) -> Iterator[list[dict]]:
        self.directory.mkdir(mode=0o700, parents=True, exist_ok=True)
        with (self.directory / "guard").open("a+") as guard:
            fcntl.flock(guard, fcntl.LOCK_EX)
            path = self.directory / "queue.json"
            # Corrupt state fails closed rather than admitting overlapping runs.
            queue = json.loads(path.read_text()) if path.exists() else []
            if not isinstance(queue, list) or any(
                not isinstance(t, dict)
                or not isinstance(t.get("id"), str)
                or type(t.get("pid")) is not int
                or t["pid"] < 1
                or not (t.get("start") is None or isinstance(t["start"], str))
                or ("slot" in t and (type(t["slot"]) is not int or t["slot"] < 0))
                or ("slots" in t and (type(t["slots"]) is not int or t["slots"] < 1))
                or ("serial" in t and type(t["serial"]) is not bool)
                for t in queue
            ):
                raise ValueError("invalid host admission state")
            if len({t["id"] for t in queue}) != len(queue):
                raise ValueError("duplicate host admission tickets")
            occupied = [t["slot"] for t in queue if "slot" in t]
            if len(set(occupied)) != len(occupied):
                raise ValueError("duplicate host admission slots")
            retained = []
            from tools.wt import lease

            now = time.time()
            for ticket in queue:
                if ticket.get("pidns") and not lease.is_local(ticket):
                    # T3500: a pid of another PID namespace is invisible here, so only the
                    # heartbeat can tell; a fresh foreign ticket is kept.
                    beat = ticket.get("heartbeat")
                    if type(beat) in (int, float) and now - beat <= lease.TTL_S:
                        retained.append(ticket)
                    else:
                        self.announce(
                            f"hostlock: released stale foreign holder pid {ticket['pid']} "
                            f"({ticket.get('pidns')}, no heartbeat for over {lease.TTL_S:.0f} s)"
                        )
                    continue
                reason = dead_reason(ticket["pid"], self.proc)
                if reason is None and (
                    ticket["start"] is None or identity(ticket["pid"], self.proc) != ticket["start"]
                ):
                    reason = "dead or reused PID"
                if reason is not None:
                    self.announce(f"hostlock: released stale holder pid {ticket['pid']} ({reason})")
                else:
                    retained.append(ticket)
            queue = retained
            try:
                yield queue
            finally:
                temporary = self.directory / "queue.new"
                temporary.write_text(json.dumps(queue) + "\n")
                temporary.replace(path)

    def __enter__(self) -> HostLock:
        started = self.clock()
        next_notice = self.announce_after
        if self.ticket["start"] is None or dead_reason(self.ticket["pid"], self.proc):
            raise RuntimeError("cannot identify host admission owner in /proc")
        with self.state() as queue:
            queue.append(self.ticket)
        try:
            while True:
                with self.state() as queue:
                    if not any(t["id"] == self.ticket["id"] for t in queue):
                        # Dropped by another session's stale sweep: rejoin, never crash.
                        queue.append(dict(self.ticket))
                    for ticket in queue:
                        if ticket["id"] == self.ticket["id"]:
                            ticket["heartbeat"] = time.time()
                    active = [t for t in queue if "slot" in t]
                    waiting = [t for t in queue if "slot" not in t]
                    hosts = [pid for pid in live_hosts(self.proc) if not self.managed(pid, active)]
                    ahead = waiting[
                        : next(
                            (i for i, t in enumerate(waiting) if t["id"] == self.ticket["id"]),
                            len(waiting),
                        )
                    ]
                    used = {t["slot"] for t in active}
                    exclusive = self.serial or any(t.get("serial", False) for t in active)
                    capacity = min([self.slots, *(t.get("slots", 1) for t in queue)])
                    available = next((n for n in range(capacity) if n not in used), None)
                    now = time.time()
                    ramping = sum(
                        1
                        for t in active
                        if type(t.get("admitted_at")) in (int, float)
                        and now - t["admitted_at"] < RAMP_SECONDS
                    )
                    shortfall = memory_shortfall(
                        mem_available_mb(self.proc), self.job_mb, ramping, self.reserve_mb
                    )
                    self.memory_note = (
                        f"memory: {shortfall} MB short of the {self.reserve_mb} MB reserve "
                        f"after a {self.job_mb} MB job"
                        if shortfall
                        else ""
                    )
                    if (
                        not ahead
                        and not hosts
                        and available is not None
                        and len(active) < capacity
                        and not (exclusive and active)
                        and not shortfall
                    ):
                        ticket = next(t for t in queue if t["id"] == self.ticket["id"])
                        self.slot = ticket["slot"] = available
                        ticket["admitted_at"] = time.time()
                        self._start_heartbeat()
                        return self
                waited = self.clock() - started
                if waited >= next_notice:
                    next_notice = waited + self.announce_every
                    self.announce(self.waiting_line(waited, len(ahead), hosts, len(active), active))
                if self.timeout is not None and self.clock() - started >= self.timeout:
                    raise TimeoutError("timed out waiting for host admission")
                self.sleep(self.poll)
        except BaseException:
            self.__exit__()
            raise

    def managed(self, pid: int, active: list[dict]) -> bool:
        """A live admitted owner or descendant; walk through timeout/nice wrappers."""
        owners = {t["pid"]: t["start"] for t in active}
        seen = set()
        while pid > 1 and pid not in seen:
            if pid in owners and identity(pid, self.proc) == owners[pid]:
                return True
            seen.add(pid)
            try:
                fields = (self.proc / str(pid) / "stat").read_text().rsplit(")", 1)[1].split()
                pid = int(fields[1])
            except (OSError, IndexError, ValueError):
                break
        return False

    def waiting_line(
        self,
        waited: float,
        ahead: int,
        hosts: list[int],
        occupied: int = 0,
        active: list[dict] | None = None,
    ) -> str:
        """The one status line of a wait: who blocks and how to skip (T2233)."""
        reasons = []
        if occupied:
            reasons.append(f"{occupied} slot(s) occupied")
            for ticket in (active or [])[:3]:
                timestamp = ticket.get("admitted_at")
                age = (
                    f"{max(0, time.time() - timestamp):.0f} s"
                    if type(timestamp) in (int, float)
                    else "unknown"
                )
                reasons.append(
                    f"holder {describe(ticket['pid'], self.proc)}, age {age}, "
                    f"task {ticket.get('task', 'unknown')}"
                )
        if ahead:
            reasons.append(f"{ahead} run(s) queued ahead")
        if getattr(self, "memory_note", ""):
            reasons.append(self.memory_note)
        if hosts:
            reasons.append(
                "host process running: " + "; ".join(describe(pid, self.proc) for pid in hosts[:3])
            )
        return (
            f"hostlock: waiting {waited:.0f} s for host admission "
            f"({', '.join(reasons) or 'queue turn'}); "
            "TSFP_HOSTLOCK=off skips this queue (never the T3823 local game slot), state in "
            + str(self.directory)
        )

    def _start_heartbeat(self) -> None:
        """T3500: refresh this ticket's heartbeat while admitted (foreign sweepers read it)."""
        from tools.wt import lease

        def run() -> None:
            while not self._beat_stop.wait(lease.HEARTBEAT_S):
                try:
                    with self.state() as queue:
                        for ticket in queue:
                            if ticket["id"] == self.ticket["id"]:
                                ticket["heartbeat"] = time.time()
                except (OSError, ValueError):
                    continue

        self._beat_thread = threading.Thread(target=run, name="hostlock-heartbeat", daemon=True)
        self._beat_thread.start()

    def __exit__(self, *exc: object) -> None:
        self._beat_stop.set()
        if self._beat_thread is not None:
            self._beat_thread.join(timeout=5)
        with self.state() as queue:
            queue[:] = [t for t in queue if t["id"] != self.ticket["id"]]


def reserve_display(x_dir: Path = Path("/tmp"), sock_dir: Path | None = None) -> int:
    """Reserve before spawn; stale X files are swept under the number's flock."""
    from tools.wt.display import reserve_number

    return reserve_number(x_dir=x_dir, sock_dir=sock_dir)


def serialized(function: Callable[P, R]) -> Callable[P, R]:
    """Wrap an actual host launch, not an outer pipeline that calls another launcher."""

    @functools.wraps(function)
    def wrapped(*args: P.args, **kwargs: P.kwargs) -> R:
        with admission():
            return function(*args, **kwargs)

    return wrapped


def run(command: list[str], timeout: float | None = None, serial: bool = False) -> int:
    from tools.hostresources import launch_scope, prepare_headless

    context = (
        contextlib.nullcontext(None)
        if disabled() and not serial
        else HostLock(timeout=timeout, serial=serial)
    )
    with context as lock, launch_scope(lock.slot if lock else None):
        directory = (
            lock.directory
            if lock
            else Path(
                os.environ.get("TSFP_HOSTLOCK_DIR")
                or Path(tempfile.gettempdir()) / f"tsfp-hostlock-{os.getuid()}"
            )
        )
        parent = directory / "raw-runs"
        parent.mkdir(parents=True, exist_ok=True)
        out = Path(tempfile.mkdtemp(prefix="run-", dir=parent))
        command, env = prepare_headless(command, dict(os.environ), out)
        child = subprocess.Popen(command, env=env, start_new_session=True)
        try:
            return child.wait()
        finally:
            if child.poll() is None:
                import signal

                os.killpg(child.pid, signal.SIGTERM)
                try:
                    child.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    os.killpg(child.pid, signal.SIGKILL)
                    child.wait()


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--wait-timeout", type=float)
    parser.add_argument(
        "--serial", action="store_true", help="exclusive admission across all slots"
    )
    parser.add_argument("command", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    command = args.command[1:] if args.command[:1] == ["--"] else args.command
    if not command:
        parser.error("a command is required")
    raise SystemExit(run(command, args.wait_timeout, args.serial))


if __name__ == "__main__":
    main()
