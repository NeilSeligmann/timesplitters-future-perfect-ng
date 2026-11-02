# SPDX-License-Identifier: GPL-3.0-or-later
"""T3500: one lease helper and watchdog for every shared lock and slot.

At least two containers (the REA container and claude-sup's) share /workspace in different
PID namespaces, so a holder's pid is often invisible to the other side. The kernel flock
stays the mutual exclusion (it works across containers on one kernel and is released when
the last fd holding it closes, also on SIGKILL or OOM). A lease adds what the flock lacks:

* who holds it: a record `<lock>.lease` next to the lock with pid, PID namespace
  (`readlink /proc/self/ns/pid`), boot id, host, container, pid start ticks, start time,
  heartbeat, purpose and a per-acquisition token;
* liveness that works across namespaces: a heartbeat (a thread in the holder, or a sidecar
  process for holders that exec into cargo) rewrites the record every HEARTBEAT_S. A
  same-namespace holder is checked by pid plus start ticks (zombies count as dead); a foreign
  one is alive while its heartbeat is younger than TTL_S;
* a maximum hold per purpose (POLICIES), enforced by the per-namespace watch loop
  (`watch_once`, `run_watch_loop`): it reports every lock held past its warn or max time to
  tmp/lock-watch.jsonl with the holder record and, for purposes with a `terminate` or `break`
  action, sends SIGTERM (then SIGKILL after GRACE_S) to the processes of OUR namespace whose
  open file description holds the flock (from /proc/<pid>/fdinfo, so lock waiters are never
  signalled). A `break` purpose (build slots, pure admission control) is also released when
  its holder is foreign and past max or not heartbeating: the slot file is renamed away so new
  acquirers lock a fresh inode. Mutexes that guard shared state (merge.lock, canon trees) are
  never broken, only their local holders signalled; the owning container's own watch loop
  handles a foreign holder.

Measured stuck cases (claude-sup audit 2026-10-21) were all ALIVE holders, which a dead-pid
check cannot catch: an orphaned merge_task (merge.lock 2 h 48 min), an orphaned cargo
(canon src.lock 2.5 h) and a runaway test (build slot 88 min). The kernel already frees the
flock of a dead process; fds inherited by daemonised children show up here as `leaked`.

    python3 -m tools.wt.lease status            table of every watched lock
    python3 -m tools.wt.lease watch --once      one watchdog pass (add --report-only)
    python3 -m tools.wt.lease watch-loop        per-namespace singleton loop (ensure starts it)
    python3 -m tools.wt.lease ensure            start the loop if this namespace has none

See docs/t3500-lease-locks.md.
"""

from __future__ import annotations

import argparse
import contextlib
import fcntl
import functools
import json
import os
import signal
import socket
import subprocess
import sys
import threading
import time
import uuid
from collections.abc import Callable
from dataclasses import dataclass
from pathlib import Path

LEASE_SUFFIX = ".lease"
HEARTBEAT_S = 10.0
TTL_S = 45.0
"""A heartbeat older than this (4.5 x HEARTBEAT_S) is stale."""
PROGRESS_STALL_S = 600.0
"""A holder that reports progress and then reports none for this long stops heartbeating."""
GRACE_S = 60.0
"""SIGTERM to SIGKILL."""
WATCH_INTERVAL_S = 60
REPORT_EVERY_S = 600.0
"""A persisting condition is reported again after this."""
WATCH_LOG_REL = Path("tmp/lock-watch.jsonl")
WATCH_LOG_MAX_BYTES = 20 * 1024 * 1024
STATE_REL = Path("tmp/wt-cache/lock-watch")
VERSION = 1


@dataclass(frozen=True)
class Policy:
    warn_s: float
    max_s: float
    action: str
    """report: log only; terminate: signal local holders past max; break: terminate, and
    release the slot (rename the lock file) when the holder is foreign or not heartbeating."""


POLICIES: dict[str, Policy] = {
    # merge_task under the lock: hold p50 7.6 s, p90 22 s; the locked last-round verify
    # fallback is bounded by its step timeout (1800 s). SIGTERM unwinds merge_task cleanly.
    "merge": Policy(900, 3600, "terminate"),
    "merge-push": Policy(300, 1500, "terminate"),  # PUSH_TIMEOUT_S is 1200 s for all rounds
    # A cold tsfp-ports check is ~10 min, the longest verify step timeout is 1800 s.
    "build-slot": Policy(1800, 3600, "break"),
    "canon-src": Policy(1800, 3600, "terminate"),
    "prebuilt-build": Policy(2700, 5400, "terminate"),
    "hostlock": Policy(1800, 7200, "report"),
    "unknown": Policy(3600, 4 * 3600, "report"),
}


def policy(purpose: str, env: dict[str, str] | None = None) -> Policy:
    """POLICIES[purpose] with TSFP_LEASE_WARN_S_<P>, TSFP_LEASE_MAX_S_<P> and
    TSFP_LEASE_ACTION_<P> overrides (P = purpose upper-cased, '-' as '_')."""
    env = os.environ if env is None else env
    base = POLICIES.get(purpose, POLICIES["unknown"])
    key = purpose.upper().replace("-", "_")

    def number(name: str, default: float) -> float:
        try:
            return float(env[name])
        except (KeyError, ValueError):
            return default

    action = env.get(f"TSFP_LEASE_ACTION_{key}", base.action)
    if action not in ("report", "terminate", "break"):
        action = base.action
    return Policy(
        number(f"TSFP_LEASE_WARN_S_{key}", base.warn_s),
        number(f"TSFP_LEASE_MAX_S_{key}", base.max_s),
        action,
    )


# ---- identity ----------------------------------------------------------------------------


@functools.lru_cache(maxsize=1)
def namespace() -> dict[str, str]:
    """Who 'we' are: PID namespace, boot id, host and container of this process."""
    try:
        pidns = os.readlink("/proc/self/ns/pid")
    except OSError:
        pidns = ""
    try:
        boot = Path("/proc/sys/kernel/random/boot_id").read_text().strip()
    except OSError:
        boot = ""
    host = socket.gethostname()
    return {
        "pidns": pidns,
        "boot": boot,
        "host": host,
        "container": os.environ.get("TSFP_CONTAINER_ID") or f"{host}/{pidns}",
    }


def proc_stat(pid: int, proc: Path = Path("/proc")) -> tuple[str, str] | None:
    """(state, start ticks) of `pid`, None when it does not exist here."""
    try:
        fields = (proc / str(pid) / "stat").read_text().rsplit(")", 1)[1].split()
        return fields[0], fields[19]
    except (OSError, IndexError):
        return None


def pid_alive(pid: int, start: str | None, proc: Path = Path("/proc")) -> bool:
    """Same-namespace liveness: exists, not a zombie, and the same process (start ticks)."""
    stat = proc_stat(pid, proc)
    if stat is None or stat[0] in ("Z", "X"):
        return False
    return start is None or stat[1] == start


def cmdline(pid: int, proc: Path = Path("/proc"), limit: int = 300) -> str:
    try:
        text = (proc / str(pid) / "cmdline").read_bytes().replace(b"\0", b" ")
    except OSError:
        return ""
    return text.decode(errors="replace").strip()[:limit]


def holder(pid: int | None = None, proc: Path = Path("/proc")) -> dict[str, object]:
    """Identity fields of a holder process (default: this one)."""
    pid = os.getpid() if pid is None else pid
    stat = proc_stat(pid, proc)
    return {
        "pid": pid,
        "start_ticks": stat[1] if stat else None,
        **namespace(),
        "uid": os.getuid(),
        "argv": cmdline(pid, proc),
        "agent": os.environ.get("TSFP_AGENT_ID") or os.environ.get("CLAUDE_AGENT_ID", ""),
    }


def is_local(record: dict, me: dict[str, str] | None = None) -> bool:
    me = me or namespace()
    return bool(record.get("pidns")) and (
        record.get("pidns") == me["pidns"] and record.get("boot") == me["boot"]
    )


def holder_alive(
    record: dict,
    now: float | None = None,
    proc: Path = Path("/proc"),
    me: dict[str, str] | None = None,
) -> tuple[bool, str]:
    """(alive, why) for a lease or ticket record, in any namespace.

    Same namespace: the pid with its start ticks decides (a fresh heartbeat of a dead pid
    does not keep it alive). Foreign namespace or no namespace recorded: the heartbeat decides.
    """
    now = time.time() if now is None else now
    heartbeat = record.get("heartbeat")
    age = now - heartbeat if isinstance(heartbeat, (int, float)) else None
    ttl = float(record.get("ttl") or TTL_S)
    if is_local(record, me):
        pid = record.get("pid")
        if type(pid) is not int:
            return False, "no pid"
        if pid_alive(pid, record.get("start_ticks"), proc):
            return True, "pid alive"
        return False, "pid dead"
    if age is None:
        return False, "no heartbeat"
    if age <= ttl:
        return True, f"foreign, heartbeat {age:.0f}s"
    return False, f"foreign, heartbeat stale {age:.0f}s"


# ---- records ------------------------------------------------------------------------------


def record_path(lock: Path) -> Path:
    return lock.with_name(lock.name + LEASE_SUFFIX)


def read_record(path: Path) -> dict | None:
    try:
        data = json.loads(path.read_text())
    except (OSError, ValueError):
        return None
    return data if isinstance(data, dict) else None


def write_record(path: Path, record: dict) -> None:
    temporary = path.with_name(f".{path.name}.{os.getpid()}.{threading.get_ident()}.tmp")
    temporary.write_text(json.dumps(record, sort_keys=True) + "\n")
    os.replace(temporary, path)


def remove_record(path: Path, token: str) -> bool:
    """Unlink the record only while it is still this acquisition's (token)."""
    current = read_record(path)
    if current is None or current.get("token") != token:
        return False
    with contextlib.suppress(FileNotFoundError):
        path.unlink()
    return True


class Lease:
    """The record of one held lock, beaten every `interval` seconds.

    mode "thread": a daemon thread in the holder beats while `held()` is true (merge_task's
    locks, which never exec). mode "sidecar": a small child process beats while the holder pid
    holds the flock on the lock inode (seen in /proc/<pid>/fdinfo), so the heartbeat survives
    an exec into cargo and stops as soon as the fd is closed."""

    def __init__(
        self,
        lock: Path,
        purpose: str,
        *,
        held: Callable[[], bool] | None = None,
        pid: int | None = None,
        detail: dict | None = None,
        interval: float | None = None,
        ttl: float = TTL_S,
        proc: Path = Path("/proc"),
    ) -> None:
        self.lock = Path(lock)
        self.path = record_path(self.lock)
        self.purpose = purpose
        self.held = held or (lambda: True)
        if interval is None:
            try:
                interval = float(os.environ.get("TSFP_LEASE_HEARTBEAT_S", HEARTBEAT_S))
            except ValueError:
                interval = HEARTBEAT_S
        self.interval = interval
        self.token = uuid.uuid4().hex
        self.sidecar_pid: int | None = None
        now = time.time()
        self.record: dict[str, object] = {
            "version": VERSION,
            "lock": str(self.lock),
            "purpose": purpose,
            "token": self.token,
            "start": now,
            "heartbeat": now,
            "interval": interval,
            "ttl": ttl,
            **holder(pid, proc),
            "detail": detail or {},
        }
        self._stop = threading.Event()
        self._thread: threading.Thread | None = None
        self._service = None  # T3518 shadow mirror, thread mode only (tools.agents.coordsvc)

    def _shadow(self, action: str) -> None:
        """Mirror acquire/beat/release to the coordination service when TSFP_COORDSVC_URL is set."""
        if not os.environ.get("TSFP_COORDSVC_URL"):
            return
        try:
            if self._service is None:
                if action != "acquire":
                    return
                from tools.agents import coordsvc

                self._service = coordsvc.LeaseShadow(
                    self.lock, self.purpose, float(self.record["ttl"]), _main_root()
                )
            getattr(self._service, action)()
        except Exception:  # noqa: BLE001 - shadow mode never fails the lock holder
            pass

    def write(self) -> None:
        try:
            write_record(self.path, self.record)
        except OSError:
            pass  # a lease is metadata, it must never fail the lock holder

    def progress(self, stage: str) -> None:
        """The holder reports work (a verify step started or finished). Once a holder reports,
        the heartbeat follows progress: no report for PROGRESS_STALL_S stops it (I-T3594-5)."""
        self.record["progress"] = {"ts": time.time(), "stage": stage}

    def beat(self) -> bool:
        """Refresh the heartbeat. False when another acquisition now owns the record."""
        current = read_record(self.path)
        if current is not None and current.get("token") != self.token:
            return False
        reported = self.record.get("progress")
        if isinstance(reported, dict) and time.time() - float(reported["ts"]) > PROGRESS_STALL_S:
            self.record["stalled"] = True  # alive but doing nothing: the heartbeat goes stale
            self.write()
            return True
        self.record.pop("stalled", None)
        self.record["heartbeat"] = time.time()
        self.write()
        self._shadow("beat")
        return True

    def start(self) -> Lease:
        self.write()
        self._shadow("acquire")
        self._thread = threading.Thread(target=self._run, name=f"lease-{self.purpose}", daemon=True)
        self._thread.start()
        return self

    def _run(self) -> None:
        next_beat = time.monotonic() + self.interval
        while not self._stop.wait(min(1.0, self.interval)):
            try:
                if not self.held():
                    break
            except Exception:  # noqa: BLE001 - a broken predicate ends the lease
                break
            if self._service is not None and self._service.pending is not None:
                self._shadow("retry")  # T3593: a refused mirror acquisition waits out the grace
            if time.monotonic() >= next_beat:
                if not self.beat():
                    self._shadow("release")  # T3548: the file record moved on, so does the mirror
                    return
                next_beat = time.monotonic() + self.interval
        self._remove()

    def _remove(self) -> None:
        with contextlib.suppress(OSError):
            remove_record(self.path, self.token)
        self._shadow("release")

    def start_sidecar(self, env: dict[str, str] | None = None) -> Lease:
        self.write()
        self.sidecar_pid = spawn_sidecar(self, env)
        return self

    def release(self) -> None:
        self._stop.set()
        if self._thread is not None and self._thread is not threading.current_thread():
            self._thread.join(timeout=2)
        self._remove()

    def __enter__(self) -> Lease:
        return self.start()

    def __exit__(self, *exc: object) -> None:
        self.release()


def fd_held(fd: int, lock: Path) -> Callable[[], bool]:
    """Predicate: `fd` is still open on the inode `lock` had when it was taken."""
    try:
        inode = os.fstat(fd).st_ino
    except OSError:
        inode = -1

    def held() -> bool:
        try:
            return os.fstat(fd).st_ino == inode
        except OSError:
            return False

    return held


def attach(
    handle: object,
    lock: Path,
    purpose: str,
    *,
    detail: dict | None = None,
    sidecar: bool = False,
) -> Lease | None:
    """Record and heartbeat a lock just taken on `handle` (an fd or a file object).

    sidecar=False: a thread, the lease ends when the handle is closed. sidecar=True: for
    holders that may exec (tools.wt run into cargo), the lease ends when the pid no longer
    holds the flock. Never raises: TSFP_LEASE=0 or any error leaves the lock unrecorded."""
    if os.environ.get("TSFP_LEASE", "1") == "0":
        return None
    try:
        if isinstance(handle, int):
            held = fd_held(handle, lock)
        else:
            file = handle
            inner = fd_held(file.fileno(), lock)  # type: ignore[attr-defined]

            def held() -> bool:
                return not file.closed and inner()  # type: ignore[attr-defined]

        lease = Lease(lock, purpose, held=held, detail=detail)
        return lease.start_sidecar() if sidecar else lease.start()
    except Exception:  # noqa: BLE001 - leases are metadata, never fail the holder
        return None


def spawn_sidecar(lease: Lease, env: dict[str, str] | None = None) -> int | None:
    """Start the heartbeat process of `lease`. It inherits no fd (close_fds, so never the
    lock), runs in its own session and exits when the holder stops holding the flock."""
    root = str(Path(__file__).resolve().parents[2])
    argv = [
        sys.executable,
        "-m",
        "tools.wt.lease",
        "beat",
        "--pid",
        str(lease.record["pid"]),
        "--lock",
        str(lease.lock),
        "--token",
        lease.token,
        "--interval",
        str(lease.interval),
    ]
    base = dict(os.environ if env is None else env)
    base["PYTHONPATH"] = root
    try:
        child = subprocess.Popen(
            argv,
            cwd=root,
            env=base,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            close_fds=True,
            start_new_session=True,
        )
    except OSError:
        return None
    return child.pid


def pid_holds_flock(pid: int, lock: Path, proc: Path = Path("/proc")) -> bool:
    """True when `pid` has an fd on `lock`'s inode whose fdinfo lists a flock."""
    try:
        target = os.stat(lock)
    except OSError:
        return False
    try:
        fds = list((proc / str(pid) / "fd").iterdir())
    except OSError:
        return False
    for fd in fds:
        try:
            st = os.stat(fd)
        except OSError:
            continue
        if (st.st_dev, st.st_ino) != (target.st_dev, target.st_ino):
            continue
        try:
            info = (proc / str(pid) / "fdinfo" / fd.name).read_text()
        except OSError:
            continue
        if any(line.startswith("lock:") and "FLOCK" in line for line in info.splitlines()):
            return True
    return False


def sidecar(
    pid: int,
    lock: Path,
    token: str,
    interval: float = HEARTBEAT_S,
    proc: Path = Path("/proc"),
    sleep: Callable[[float], None] = time.sleep,
    max_beats: int | None = None,
) -> int:
    """Beat the record of `lock` while `pid` lives and holds its flock; remove the record
    when it stops. Returns the number of beats."""
    path = record_path(lock)
    stat = proc_stat(pid, proc)
    start = stat[1] if stat else None
    beats = 0
    while max_beats is None or beats < max_beats:
        if not pid_alive(pid, start, proc) or not pid_holds_flock(pid, lock, proc):
            with contextlib.suppress(OSError):
                remove_record(path, token)
            return beats
        current = read_record(path)
        if current is None or current.get("token") != token:
            return beats  # released, or a newer acquisition owns the lock now
        current["heartbeat"] = time.time()
        with contextlib.suppress(OSError):
            write_record(path, current)
        beats += 1
        sleep(interval)
    return beats


# ---- observation ----------------------------------------------------------------------------


def lock_held(lock: Path) -> bool | None:
    """True when some process (any container) holds an exclusive flock on `lock`; None when
    the file does not exist. Probes with a shared non-blocking flock, released at once."""
    try:
        fd = os.open(lock, os.O_RDONLY | os.O_NOFOLLOW)
    except FileNotFoundError:
        return None
    except OSError:
        return None
    try:
        fcntl.flock(fd, fcntl.LOCK_SH | fcntl.LOCK_NB)
    except BlockingIOError:
        return True
    except OSError:
        return None
    finally:
        os.close(fd)
    return False


def flock_holders(locks: list[Path], proc: Path = Path("/proc")) -> dict[Path, list[int]]:
    """Pids in this PID namespace whose open file description holds a flock on each lock
    (from /proc/<pid>/fdinfo `lock:` lines), in one /proc scan. Processes that merely have
    the file open, such as blocked waiters, are not holders."""
    wanted: dict[tuple[int, int], Path] = {}
    for lock in locks:
        try:
            st = os.stat(lock)
        except OSError:
            continue
        wanted[(st.st_dev, st.st_ino)] = lock
    found: dict[Path, list[int]] = {lock: [] for lock in locks}
    if not wanted:
        return found
    try:
        entries = [entry for entry in proc.iterdir() if entry.name.isdigit()]
    except OSError:
        return found
    for entry in entries:
        try:
            fds = list((entry / "fd").iterdir())
        except OSError:
            continue
        for fd in fds:
            try:
                st = os.stat(fd)
            except OSError:
                continue
            lock = wanted.get((st.st_dev, st.st_ino))
            if lock is None:
                continue
            try:
                info = (entry / "fdinfo" / fd.name).read_text()
            except OSError:
                continue
            pid = int(entry.name)
            if pid not in found[lock] and any(
                line.startswith("lock:") and "FLOCK" in line for line in info.splitlines()
            ):
                found[lock].append(pid)
    return {lock: sorted(pids) for lock, pids in found.items()}


def fd_holders(lock: Path, proc: Path = Path("/proc")) -> list[int]:
    return flock_holders([lock], proc)[lock]


@dataclass
class Observation:
    lock: Path
    purpose: str
    held: bool | None
    record: dict | None
    alive: bool
    why: str
    hold_s: float | None
    local_pids: list[int]
    key: str = ""
    basis: str = ""
    """How hold_s was measured: `lease` (recorded start), `first-seen` (same holder pids
    and start ticks), `held-span` (foreign unrecorded: the lock stayed held, possibly by
    several holders in turn, so hold_s is an upper bound for any one of them)."""

    def holder_summary(self) -> dict[str, object]:
        record = self.record or {}
        keys = ("pid", "pidns", "host", "container", "start", "heartbeat", "argv", "agent")
        summary: dict[str, object] = {key: record.get(key) for key in keys if key in record}
        if record.get("detail"):
            summary["detail"] = record["detail"]
        return summary


def holder_key(lock: Path, record: dict | None, local: list[int], proc: Path) -> str:
    """Identity of one hold: the lease token, else the local holder pids with start ticks
    (an unrecorded holder from an older tools copy), else `foreign` (cannot be told apart)."""
    if record and record.get("token"):
        return f"{lock}|{record['token']}"
    if local:
        ticks = [f"{pid}:{(proc_stat(pid, proc) or ('', ''))[1]}" for pid in local]
        return f"{lock}|pids={','.join(ticks)}"
    return f"{lock}|foreign"


def observe(
    lock: Path,
    purpose: str,
    now: float,
    first_seen: dict[str, list] | None = None,
    proc: Path = Path("/proc"),
    me: dict[str, str] | None = None,
    local: list[int] | None = None,
) -> Observation:
    """The state of one lock. `first_seen` (key -> [time]) gives unrecorded holds an age."""
    held = lock_held(lock)
    record = read_record(record_path(lock))
    alive, why = holder_alive(record, now, proc, me) if record else (False, "no record")
    if not held:
        return Observation(lock, purpose, held, record, alive, why, None, [])
    local = fd_holders(lock, proc) if local is None else local
    key = holder_key(lock, record, local, proc)
    start = record.get("start") if record else None
    if record and alive and isinstance(start, (int, float)):
        hold_s: float | None = now - start
    elif first_seen is not None:
        hold_s = now - first_seen.setdefault(key, [now])[0]
    else:
        hold_s = None
    if record is None and local:
        why = "unrecorded local holder"
    elif record is None:
        why = "unrecorded, holder not in this PID namespace"
    if record and alive and isinstance(start, (int, float)):
        basis = "lease"
    elif key.endswith("|foreign"):
        basis = "held-span"
    else:
        basis = "first-seen"
    return Observation(lock, purpose, held, record, alive, why, hold_s, local, key, basis)


# ---- what is watched ------------------------------------------------------------------------


def targets(main: Path, env: dict[str, str] | None = None) -> list[tuple[Path, str]]:
    """Every shared lock the watchdog checks: (path, purpose)."""
    env = dict(os.environ if env is None else env)
    tmp = main / "tmp"
    found: list[tuple[Path, str]] = [
        (tmp / "merge.lock", "merge"),
        (tmp / "merge-push.lock", "merge-push"),
    ]
    slots = main / "tmp/wt-cache/build-slots"
    for pattern in ("slot-*.lock", "merge-slot-*.lock"):
        found += [(path, "build-slot") for path in sorted(slots.glob(pattern))]
    canon_dirs = [main / "tmp/wt-cache/canon-src"]
    try:
        from tools.wt import canonical_build

        shared = canonical_build.shared_root(main, env)
        if shared is not None:
            canon_dirs.append(shared)
    except Exception:  # noqa: BLE001 - an unreadable host map must not stop the watchdog
        pass
    for directory in canon_dirs:
        found += [
            (path, "canon-src")
            for path in sorted(directory.glob("*.lock"))
            if path.name != "cargo.seed.lock"
        ]
    prebuilt = Path(env["TSFP_PREBUILT_ROOT"]) if env.get("TSFP_PREBUILT_ROOT") else None
    prebuilt = (prebuilt or main / "tmp/wt-cache/prebuilt") / ".locks" / ".build.lock"
    found.append((prebuilt, "prebuilt-build"))
    return [(path, purpose) for path, purpose in dict.fromkeys(found) if path.exists()]


def hostlock_queues(main: Path, env: dict[str, str] | None = None) -> list[Path]:
    env = dict(os.environ if env is None else env)
    queues = sorted((main / "tmp/wt-cache/replays").glob("hostlock-*/queue.json"))
    queues.append(Path(f"/tmp/tsfp-hostlock-{os.getuid()}/queue.json"))
    if env.get("TSFP_HOSTLOCK_DIR"):
        queues.append(Path(env["TSFP_HOSTLOCK_DIR"]) / "queue.json")
    return [q for q in dict.fromkeys(queues) if q.exists()]


# ---- watchdog -------------------------------------------------------------------------------


def _ns_tag(me: dict[str, str]) -> str:
    return "".join(ch for ch in me["pidns"] if ch.isdigit()) or "unknown"


def _state_path(main: Path, me: dict[str, str]) -> Path:
    return main / STATE_REL / f"state-{_ns_tag(me)}.json"


def _load_state(path: Path) -> dict:
    data = read_record(path) or {}
    for key in ("first_seen", "reported", "signalled"):
        if not isinstance(data.get(key), dict):
            data[key] = {}
    return data


def _emit(sink: Path, event: dict) -> None:
    sink.parent.mkdir(parents=True, exist_ok=True)
    try:
        if sink.stat().st_size > WATCH_LOG_MAX_BYTES:
            os.replace(sink, sink.with_name(sink.name + ".1"))
    except OSError:
        pass
    with sink.open("a") as out:
        out.write(json.dumps(event, sort_keys=True) + "\n")


def _break(lock: Path, now: float) -> Path | None:
    """Release a slot whose holder cannot be signalled: move the lock file away, so new
    acquirers lock a fresh inode while the stuck holder keeps the old one."""
    moved = lock.with_name(f"{lock.name}.broken-{int(now)}")
    try:
        os.rename(lock, moved)
    except OSError:
        return None
    with contextlib.suppress(OSError):
        record_path(lock).unlink()
    return moved


def watch_once(
    main: Path,
    *,
    now: float | None = None,
    enforce: bool = True,
    proc: Path = Path("/proc"),
    me: dict[str, str] | None = None,
    kill: Callable[[int, int], None] = os.kill,
    lock_targets: list[tuple[Path, str]] | None = None,
    env: dict[str, str] | None = None,
) -> list[dict]:
    """One watchdog pass over every target. Returns the events appended to
    tmp/lock-watch.jsonl. Signals go only to processes of this PID namespace that hold the
    flock; a foreign holder is reported (and a `break` slot released), never signalled."""
    now = time.time() if now is None else now
    env = dict(os.environ if env is None else env)
    me = me or namespace()
    enforce = enforce and env.get("TSFP_LEASE_ENFORCE", "1") != "0"
    sink = main / WATCH_LOG_REL
    state_file = _state_path(main, me)
    state = _load_state(state_file)
    first_seen: dict[str, list] = state["first_seen"]
    reported: dict[str, float] = state["reported"]
    signalled: dict[str, list] = state["signalled"]
    events: list[dict] = []
    live_keys: set[str] = set()
    watcher = {"pid": os.getpid(), **me}
    chosen = targets(main, env) if lock_targets is None else lock_targets

    def emit(kind: str, obs: Observation, key: str, repeat: bool = True, **extra: object) -> None:
        report_key = f"{kind}:{key}"
        last = reported.get(report_key)
        if last is not None and (not repeat or now - last < REPORT_EVERY_S):
            return
        reported[report_key] = now
        event = {
            "ts": round(now, 3),
            "event": kind,
            "lock": str(obs.lock),
            "purpose": obs.purpose,
            "hold_s": None if obs.hold_s is None else round(obs.hold_s, 1),
            "hold_basis": obs.basis,
            "holder": obs.holder_summary(),
            "holder_alive": obs.alive,
            "why": obs.why,
            "local_pids": obs.local_pids,
            "local_argv": {str(pid): cmdline(pid, proc, 160) for pid in obs.local_pids[:6]},
            "watcher": watcher,
            **extra,
        }
        events.append(event)
        _emit(sink, event)

    def release_slot(obs: Observation) -> None:
        moved = _break(obs.lock, now)
        if moved is not None:
            emit("break", obs, obs.key, repeat=False, moved_to=str(moved))

    held_now = [(lock, purpose) for lock, purpose in chosen if lock_held(lock)]
    holders = flock_holders([lock for lock, _purpose in held_now], proc) if held_now else {}
    for lock, purpose in chosen:
        if (lock, purpose) in held_now:
            continue
        # Free: the flock is the authority; a leftover record is dropped once it is stale.
        record = read_record(record_path(lock))
        if record is not None:
            alive, _why = holder_alive(record, now, proc, me)
            beat = record.get("heartbeat")
            if not alive and (not isinstance(beat, (int, float)) or now - beat > TTL_S):
                with contextlib.suppress(OSError):
                    remove_record(record_path(lock), str(record.get("token")))
    for lock, purpose in held_now:
        rule = policy(purpose, env)
        obs = observe(lock, purpose, now, first_seen, proc, me, holders.get(lock, []))
        if not obs.held:
            continue  # released between the probe and now
        live_keys.add(obs.key)
        local = [pid for pid in obs.local_pids if pid != os.getpid()]
        if obs.record is None:
            emit("unrecorded", obs, obs.key, repeat=False)
        elif not obs.alive:
            # Recorded holder dead (local) or silent (foreign) while the flock is still held:
            # an fd inherited by another process, or a stopped or frozen holder.
            emit("leaked" if is_local(obs.record, me) else "stale-heartbeat", obs, obs.key)
            if enforce and rule.action == "break" and not local:
                release_slot(obs)
                continue
        hold = obs.hold_s or 0.0
        if hold < rule.warn_s:
            continue
        if hold < rule.max_s:
            emit("over-warn", obs, obs.key, warn_s=rule.warn_s, max_s=rule.max_s)
            continue
        emit("over-max", obs, obs.key, max_s=rule.max_s, action=rule.action)
        if not enforce or rule.action == "report":
            continue
        if local:
            for pid in local:
                sent = signalled.get(f"{obs.key}|{pid}")
                if sent is None:
                    signum = signal.SIGTERM
                elif now - sent[0] >= GRACE_S and sent[1] == int(signal.SIGTERM):
                    signum = signal.SIGKILL
                else:
                    continue
                with contextlib.suppress(OSError):
                    kill(pid, signum)
                signalled[f"{obs.key}|{pid}"] = [now, int(signum)]
                emit(
                    "signal",
                    obs,
                    f"{obs.key}|{pid}|{int(signum)}",
                    repeat=False,
                    pid=pid,
                    signal=signal.Signals(signum).name,
                )
        elif rule.action == "break" and obs.record is not None:
            release_slot(obs)  # a foreign runaway: its own container signals it
    for directory in {lock.parent for lock, _purpose in chosen}:
        for broken in directory.glob("*.broken-*"):
            if lock_held(broken) is False:
                with contextlib.suppress(OSError):
                    broken.unlink()
    events += _watch_hostlock(main, now, me, proc, env, sink, watcher, reported)
    state["first_seen"] = {k: v for k, v in first_seen.items() if k in live_keys}
    state["signalled"] = {k: v for k, v in signalled.items() if k.rsplit("|", 1)[0] in live_keys}
    state["reported"] = {k: v for k, v in reported.items() if now - v < 6 * 3600}
    state_file.parent.mkdir(parents=True, exist_ok=True)
    write_record(state_file, state)
    return events


def _watch_hostlock(
    main: Path,
    now: float,
    me: dict[str, str],
    proc: Path,
    env: dict[str, str],
    sink: Path,
    watcher: dict,
    reported: dict[str, float],
) -> list[dict]:
    """Report hostlock (replay) admissions held past warn or max. Report only: the queue
    sweeps dead holders itself, and every capture has its own timeout."""
    rule = policy("hostlock", env)
    events: list[dict] = []
    for queue in hostlock_queues(main, env):
        try:
            tickets = json.loads(queue.read_text())
        except (OSError, ValueError):
            continue
        for ticket in tickets if isinstance(tickets, list) else []:
            if not isinstance(ticket, dict) or "slot" not in ticket:
                continue
            admitted = ticket.get("admitted_at")
            if not isinstance(admitted, (int, float)) or now - admitted < rule.warn_s:
                continue
            kind = "over-max" if now - admitted >= rule.max_s else "over-warn"
            key = f"{kind}:{queue}|{ticket.get('id')}"
            if now - reported.get(key, 0.0) < REPORT_EVERY_S:
                continue
            reported[key] = now
            view = {**ticket, "start_ticks": ticket.get("start")}
            if "pidns" not in ticket:  # pre-T3500 ticket: written by a same-namespace peer
                view.update(pidns=me["pidns"], boot=me["boot"])
            alive, why = holder_alive(view, now, proc, me)
            event = {
                "ts": round(now, 3),
                "event": kind,
                "lock": str(queue),
                "purpose": "hostlock",
                "hold_s": round(now - admitted, 1),
                "holder": {k: ticket.get(k) for k in ("pid", "pidns", "task", "slot", "id")},
                "holder_alive": alive,
                "why": why,
                "watcher": watcher,
                "max_s": rule.max_s,
                "action": "report",
            }
            events.append(event)
            _emit(sink, event)
    return events


def status(main: Path, now: float | None = None) -> list[dict]:
    now = time.time() if now is None else now
    rows = []
    for lock, purpose in targets(main):
        obs = observe(lock, purpose, now, proc=Path("/proc"))
        rows.append(
            {
                "lock": str(lock.relative_to(main)) if lock.is_relative_to(main) else str(lock),
                "purpose": purpose,
                "held": obs.held,
                "hold_s": None if obs.hold_s is None else round(obs.hold_s),
                "alive": obs.alive,
                "why": obs.why,
                "holder": obs.holder_summary(),
                "local_pids": obs.local_pids,
            }
        )
    return rows


# ---- per-namespace loop -----------------------------------------------------------------------


def _loop_lock(main: Path) -> Path:
    path = main / STATE_REL / f"loop-{_ns_tag(namespace())}.lock"
    path.parent.mkdir(parents=True, exist_ok=True)
    return path


def _try_loop_lock(main: Path) -> int | None:
    fd = os.open(_loop_lock(main), os.O_RDWR | os.O_CREAT, 0o666)
    try:
        fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except BlockingIOError:
        os.close(fd)
        return None
    return fd


def loop_alive(main: Path) -> bool:
    fd = _try_loop_lock(main)
    if fd is None:
        return True
    os.close(fd)
    return False


LOOP_NAME = "lease-watch"  # tools/data/loops.json


def run_watch_loop(main: Path, interval: float = WATCH_INTERVAL_S) -> int:
    """Singleton per PID namespace (each container signals only its own processes). Exits
    when this module's file changes (a merge updated it), the next ensure starts the new one."""
    fd = _try_loop_lock(main)
    if fd is None:
        return 0
    os.ftruncate(fd, 0)
    os.write(fd, f"{os.getpid()}\n".encode())
    with contextlib.suppress(OSError):
        os.nice(10)
    source = Path(__file__)
    version = source.stat().st_mtime_ns
    from tools.agents import canonical_mount, loops

    canonical_mount.require(LOOP_NAME, main)  # T3784: exits 0 off /workspace
    loops.install_term_handler()  # T3763
    while True:
        if not (main / ".git").exists():
            print(f"lock watch: {main} is gone, exiting", flush=True)  # leaked test loops
            return 0
        loops.tick(LOOP_NAME, main)
        try:
            for event in watch_once(main):
                print(json.dumps(event, sort_keys=True), flush=True)
        except Exception as exc:  # noqa: BLE001 - the watchdog must outlive one bad pass
            print(f"lock watch pass failed: {exc}", flush=True)
        time.sleep(interval)
        try:
            if source.stat().st_mtime_ns != version:
                print("lock watch: tools/wt/lease.py changed, exiting for a restart", flush=True)
                return 0
        except OSError:
            return 0


def ensure_watch_loop(main: Path) -> str:
    """Start the detached per-namespace watch loop unless one runs. Safe to call often."""
    if os.environ.get("TSFP_LEASE_WATCH", "1") == "0":
        return "disabled"
    if loop_alive(main):
        return "running"
    if os.environ.get("PYTEST_CURRENT_TEST"):
        return "disabled under pytest"  # T3763: tests never spawn real daemons
    log = main / STATE_REL / "loop.log"
    log.parent.mkdir(parents=True, exist_ok=True)
    with open(log, "ab") as out:
        subprocess.Popen(
            [sys.executable, "-m", "tools.wt.lease", "watch-loop", "--main", str(main)],
            cwd=main,
            stdin=subprocess.DEVNULL,
            stdout=out,
            stderr=out,
            close_fds=True,
            start_new_session=True,
        )
    return "started"


def _main_root() -> Path:
    done = subprocess.run(
        ["git", "rev-parse", "--path-format=absolute", "--git-common-dir"],
        capture_output=True,
        text=True,
        check=False,
        timeout=30,
    )
    if done.returncode:
        from tools.wt import source_identity

        override = source_identity.main_override()  # T3760: git-less remote exec tree
        if override is not None:
            return override
        done.check_returncode()
    return Path(done.stdout.strip()).parent


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    sub = parser.add_subparsers(dest="command", required=True)
    beat = sub.add_parser("beat", help="sidecar: heartbeat a lease while --pid holds --lock")
    beat.add_argument("--pid", type=int, required=True)
    beat.add_argument("--lock", type=Path, required=True)
    beat.add_argument("--token", required=True)
    beat.add_argument("--interval", type=float, default=HEARTBEAT_S)
    for name, text in (
        ("status", "print every watched lock as JSON lines"),
        ("watch", "run one watchdog pass (with --once) and print its events"),
        ("watch-loop", "per-namespace singleton watchdog loop"),
        ("ensure", "start the watch loop if this namespace has none"),
    ):
        command = sub.add_parser(name, help=text)
        command.add_argument("--main", type=Path, help="primary checkout (default: from git)")
        if name == "watch":
            command.add_argument("--once", action="store_true", help="one pass (default)")
            command.add_argument("--report-only", action="store_true", help="never signal")
        if name == "watch-loop":
            command.add_argument("--interval", type=float, default=WATCH_INTERVAL_S)
    args = parser.parse_args(argv)
    if args.command == "beat":
        sidecar(args.pid, args.lock, args.token, args.interval)
        return 0
    root = args.main or _main_root()
    if args.command == "status":
        for row in status(root):
            print(json.dumps(row, sort_keys=True))
        return 0
    if args.command == "watch":
        for event in watch_once(root, enforce=not args.report_only):
            print(json.dumps(event, sort_keys=True))
        return 0
    if args.command == "watch-loop":
        return run_watch_loop(root, args.interval)
    print(ensure_watch_loop(root))
    return 0


if __name__ == "__main__":
    sys.exit(main())
