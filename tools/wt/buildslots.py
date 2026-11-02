"""T3411: memory-aware admission for heavy cargo builds across all worktrees.

A pool of flock slot files under tmp/wt-cache/build-slots/ on the main checkout. A heavy
command (cargo build/check/test/clippy/run/bench/doc, tools/dev/build.sh) takes one slot
before it starts and holds it until it exits: the locked fd is inherited across exec, so the
kernel frees the slot when the process dies, even on SIGKILL or OOM. Admission also waits
while MemAvailable is below the reserve plus one build's estimate, unless no other heavy
build holds a slot (waiting could not free memory then).

Knobs: TSFP_BUILD_SLOTS (pool size, default from MemTotal), TSFP_BUILD_RSS_MB (per-build
estimate), TSFP_BUILD_MEM_RESERVE_MB, TSFP_BUILD_ADMISSION=off (skip), TSFP_BUILD_WAIT_S.

T3446: a second, reserved pool (merge-slot-N.lock, size from TSFP_MERGE_SLOTS or the live
MERGECOUNT file, default 1) is taken only by merge_task verifies (`acquire(merge=True)`), which
prefer it and fall back to the shared pool. Worker builds never probe merge slots, so they
cannot starve a merge. Merge-slot holders still count as heavy builds for the memory check.

T3500: every taken slot gets a lease record (`slot-N.lock.lease`, tools.wt.lease) with the
holder's pid, PID namespace, host and start time, beaten by a sidecar process that survives
the exec into cargo and stops when the holder pid no longer holds the slot's flock. The
per-namespace lock watchdog reports a slot held past its max (TSFP_LEASE_MAX_S_BUILD_SLOT,
default 3600 s), signals the local holders and releases the slot of a foreign holder that is
past max or no longer heartbeating.
"""

from __future__ import annotations

import fcntl
import hashlib
import os
import shlex
import sys
import time
from collections.abc import Callable
from pathlib import Path

from tools import hostcaps
from tools.agents import jobevents
from tools.hostlock import env_mb, mem_available_mb
from tools.wt import lease

SLOTS_REL = Path("tmp/wt-cache/build-slots")
BUILD_RSS_MB = 6 * 1024
"""MEASURED peak RSS of one cold `cargo check` of tsfp-ports at 6 jobs: 5.5 to 6.4 GB (T3411)."""
MEM_RESERVE_MB = 4 * 1024
MERGE_SLOTS_DEFAULT = 1
SLOT_MEM_MB = 16 * 1024
"""Default pool size: one slot per 16 GB of MemTotal, between 1 and 4."""
HEAVY_CARGO = {"build", "b", "check", "c", "test", "t", "clippy", "run", "r", "bench", "doc"}


SEPARATORS = frozenset({"&&", "||", ";", "|", "&", "|&", "(", ")", "\n"})
WRAPPERS = frozenset(
    {"env", "nice", "ionice", "time", "command", "exec", "sudo", "nohup", "stdbuf"}
)
SHELLS = frozenset({"sh", "bash", "zsh"})
PYTEST_WORKERS = 4
"""Mirrors tools.dispatch.enforce.PYTEST_WORKERS (not imported: enforce imports this layer)."""


def _segments(command: str) -> list[list[str]]:
    """Simple commands of a shell line (split on | && || ; &), each as argv."""
    lexer = shlex.shlex(command, posix=True, punctuation_chars="|&;()\n")
    # A shell newline ends a simple command just like `;`. Keep it out of shlex's
    # default whitespace set so the segment loop can classify commands after it.
    lexer.whitespace = lexer.whitespace.replace("\n", "")
    lexer.whitespace_split = True
    segments: list[list[str]] = [[]]
    for token in lexer:
        if token in SEPARATORS:
            segments.append([])
        else:
            segments[-1].append(token)
    return [segment for segment in segments if segment]


def _head(argv: list[str]) -> list[str]:
    """argv without leading VAR=value words and wrapper commands (env, nice -n N, time...)."""
    index = 0
    while index < len(argv):
        word = argv[index]
        if "=" in word and not word.startswith(("-", "/")) and word.split("=", 1)[0].isidentifier():
            index += 1
        elif Path(word).name in WRAPPERS:
            index += 1
            while index < len(argv) and (argv[index].startswith("-") or argv[index].isdigit()):
                index += 1
        elif Path(word).name == "timeout" and index + 1 < len(argv):
            index += 2
            while index < len(argv) and argv[index].startswith("-"):
                index += 1
        else:
            break
    return argv[index:]


def _pytest_workers(argv: list[str]) -> int:
    for index, word in enumerate(argv):
        value = ""
        if word in ("-n", "--numprocesses") and index + 1 < len(argv):
            value = argv[index + 1]
        elif word.startswith("--numprocesses="):
            value = word.split("=", 1)[1]
        elif word.startswith("-n") and word[2:].isdigit():
            value = word[2:]
        if value.isdigit():
            return int(value)
    return 0


def _simple_heavy(argv: list[str]) -> bool:
    argv = _head(argv)
    if not argv:
        return False
    name = Path(argv[0]).name
    rest = argv[1:]
    if name == "build.sh":
        return True
    if name in SHELLS:
        if len(rest) >= 2 and rest[0] == "-c":
            return is_heavy(["sh", "-c", rest[1]])
        script = [word for word in rest if not word.startswith("-")][:1]
        return bool(script) and Path(script[0]).name == "build.sh"
    if name == "cargo":
        rest = [w for w in rest if not w.startswith("+")]
        return bool(rest) and rest[0] in HEAVY_CARGO
    if name == "cmake":
        # `cmake -E echo --build` and `cmake -P script.cmake --build` are
        # utility/script invocations. Treat --build as an option only in the
        # normal configure/build mode, before an option terminator.
        options = rest[: rest.index("--")] if "--" in rest else rest
        if any(word in {"-E", "-P", "--help", "--version"} for word in options):
            return False
        return "--build" in options
    if name == "ninja":
        return not any(word in rest for word in ("-t", "-h", "--help"))
    if name == "pytest" or (name.startswith("python") and rest[:2] == ["-m", "pytest"]):
        return _pytest_workers(rest) > PYTEST_WORKERS
    return False


def is_heavy(cmd: list[str]) -> bool:
    """True when the command actually compiles/tests heavily (T3885 classifier).

    Only the head word of each pipeline segment counts: `grep build.sh x | wc` or
    `echo cargo test` are light. Heavy: cargo build/test/..., build.sh, cmake --build, ninja,
    pytest with more than PYTEST_WORKERS xdist workers.
    """
    if any(word == "tools.wt" for word in cmd) and "run" in cmd:
        return False  # the inner tools.wt run owns the slot and profile lock
    if len(cmd) == 3 and Path(cmd[0]).name in SHELLS and cmd[1] == "-c":
        try:
            segments = _segments(cmd[2])
        except ValueError:
            return False
        return any(_simple_heavy(segment) for segment in segments)
    return _simple_heavy(cmd)


def mem_total_mb(proc: Path = Path("/proc")) -> int | None:
    """min(MemTotal, cgroup memory.max) in MB (T3844, tools.hostcaps)."""
    total = hostcaps.compute_mem_total_bytes(proc=proc)
    return None if total is None else total // 2**20


def slot_count(total_mb: int | None, directory: Path | None = None) -> int:
    value = os.environ.get("TSFP_BUILD_SLOTS")
    if value:
        return max(1, int(value))
    if directory is not None:
        # Host-wide override a supervisor can tune live, without restarting every worker.
        try:
            return max(1, int((directory / "COUNT").read_text().strip()))
        except (OSError, ValueError):
            pass
    if total_mb is None:
        return 2
    return max(1, min(4, total_mb // SLOT_MEM_MB))


def merge_slot_count(directory: Path) -> int:
    """Reserved merge pool size: TSFP_MERGE_SLOTS, else the live MERGECOUNT file, else 1.
    0 disables the reserved pool (merges then use the shared pool only)."""
    value = os.environ.get("TSFP_MERGE_SLOTS")
    if value:
        return max(0, int(value))
    try:
        return max(0, int((directory / "MERGECOUNT").read_text().strip()))
    except (OSError, ValueError):
        return MERGE_SLOTS_DEFAULT


def env_int(name: str, default: int) -> int:
    try:
        return max(0, int(os.environ.get(name, default)))
    except ValueError:
        return default


def _slot_paths(directory: Path, count: int, prefix: str = "slot") -> list[Path]:
    return [directory / f"{prefix}-{index}.lock" for index in range(count)]


def _try_lock(path: Path) -> int | None:
    fd = os.open(path, os.O_RDWR | os.O_CREAT, 0o666)
    try:
        fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except BlockingIOError:
        os.close(fd)
        return None
    return fd


def profile_lock_path(main: Path, profile: Path) -> Path:
    """Stable lock outside the removable profile directory, keyed by its resolved path."""
    key = hashlib.sha256(os.fsencode(profile.resolve(strict=False))).hexdigest()
    return main / SLOTS_REL / "profiles" / f"{key}.lock"


def acquire_profile_lock(
    main: Path,
    profile: Path,
    *,
    shared: bool,
    nonblocking: bool = False,
) -> int | None:
    """Hold one target profile against pruning (build=shared, prune=exclusive).

    Lock files live outside cargo-target so pruning a profile cannot unlink the lock inode.
    Build locks are inheritable and survive exec into cargo/rustc; pruning never waits.
    """
    lock_path = profile_lock_path(main, profile)
    lock_path.parent.mkdir(parents=True, exist_ok=True)
    fd = os.open(lock_path, os.O_RDWR | os.O_CREAT, 0o666)
    operation = fcntl.LOCK_SH if shared else fcntl.LOCK_EX
    if nonblocking:
        operation |= fcntl.LOCK_NB
    try:
        fcntl.flock(fd, operation)
    except BlockingIOError:
        os.close(fd)
        return None
    os.set_inheritable(fd, True)
    return fd


def held_slots(directory: Path, count: int, prefix: str = "slot") -> int:
    """How many slots another process holds right now (probe, never keeps a lock)."""
    held = 0
    for path in _slot_paths(directory, count, prefix):
        fd = _try_lock(path)
        if fd is None:
            held += 1
        else:
            os.close(fd)
    return held


WAIT_LOG_S = 60.0


def slot_holders(paths: list[Path], proc: Path = Path("/proc")) -> str:
    """`slot-0 pid 123 (agent, 95 s, alive)` for every held slot, from its lease record.

    T3720 (claude-w1): a worker_check waited 60 min for a slot with one log line and no
    holder named; the wait now says who holds what, and whether the holder looks alive."""
    parts = []
    for path in paths:
        fd = _try_lock(path)
        if fd is not None:
            os.close(fd)
            continue
        record = lease.read_record(lease.record_path(path))
        if record is None:
            parts.append(f"{path.stem} (no lease record)")
            continue
        alive, why = lease.holder_alive(record, proc=proc)
        started = record.get("start")
        age = f", {time.time() - float(started):.0f} s" if isinstance(started, int | float) else ""
        who = record.get("agent") or record.get("host") or ""
        state = "alive" if alive else f"STALE: {why}"
        parts.append(f"{path.stem} pid {record.get('pid')} ({who}{age}, {state})")
    return "; ".join(parts) or "none held"


def acquire(
    main: Path,
    proc: Path = Path("/proc"),
    sleep: float = 2.0,
    wait_s: float | None = None,
    log: Callable[[str], object] | None = None,
    merge: bool = False,
) -> int | None:
    """Block until a slot is free and memory allows; return the locked, inheritable fd.

    merge=True (merge_task verifies only) tries the reserved merge pool first, then the
    shared pool. Callers without merge=True never see the merge pool.

    None when admission is off or the wait limit passed (the build then runs unadmitted
    with a warning, it never fails because of the pool).
    """
    if os.environ.get("TSFP_BUILD_ADMISSION", "").lower() in ("off", "0", "no"):
        return None
    if os.environ.get("TSFP_BUILD_SLOT_HELD"):
        return None  # an outer merge_task verify already holds a slot for this process tree
    log = log or (lambda text: print(text, file=sys.stderr, flush=True))
    directory = main / SLOTS_REL
    directory.mkdir(parents=True, exist_ok=True)
    count = slot_count(mem_total_mb(proc), directory)
    merge_count = merge_slot_count(directory)
    candidates = _slot_paths(directory, count)
    if merge:
        # T3461: merge verifies held both shared slots and worker builds waited 20-30 min.
        # A merge verify uses the merge pool and all shared slots but the last, which stays
        # for worker builds (TSFP_WORKER_RESERVED_SLOTS, default 1, never the only slot).
        reserved = min(env_int("TSFP_WORKER_RESERVED_SLOTS", 1), count - 1)
        shared = candidates[: count - reserved] if reserved > 0 else candidates
        candidates = _slot_paths(directory, merge_count, "merge-slot") + shared
    job_mb = env_mb("TSFP_BUILD_RSS_MB", BUILD_RSS_MB)
    reserve_mb = env_mb("TSFP_BUILD_MEM_RESERVE_MB", MEM_RESERVE_MB)
    if wait_s is None:
        wait_s = float(os.environ.get("TSFP_BUILD_WAIT_S", "3600"))
    start = time.monotonic()
    said = ""
    logged = start
    while True:
        held = held_slots(directory, count) + held_slots(directory, merge_count, "merge-slot")
        available = mem_available_mb(proc)
        memory_ok = held == 0 or available is None or available >= reserve_mb + job_mb
        if memory_ok:
            for path in candidates:
                fd = _try_lock(path)
                if fd is not None:
                    os.set_inheritable(fd, True)
                    waited = time.monotonic() - start
                    log(f"tools.wt: build slot acquired after {waited:.3f} s")
                    jobevents.emit(
                        "slot-wait", start=time.time() - waited, result="acquired", merge=merge
                    )
                    lease.attach(fd, path, "build-slot", sidecar=True)
                    return fd
        reason = (
            f"all {len(candidates)} build slots busy"
            if memory_ok
            else f"MemAvailable {available} MB < {reserve_mb + job_mb} MB"
        )
        now = time.monotonic()
        if reason != said or now - logged >= WAIT_LOG_S:
            holders = slot_holders(
                candidates + [p for p in _slot_paths(directory, count) if p not in candidates]
            )
            log(
                f"tools.wt: waiting for a build slot {now - start:.0f} s of {wait_s:.0f} s "
                f"({reason}; holders: {holders})"
            )
            said, logged = reason, now
        if time.monotonic() - start > wait_s:
            log(f"tools.wt: build slot wait exceeded {wait_s:.0f} s, running unadmitted")
            jobevents.emit(
                "slot-wait",
                start=time.time() - (time.monotonic() - start),
                result="unadmitted",
                cause="slot-wait",
                merge=merge,
            )
            return None
        time.sleep(sleep)
