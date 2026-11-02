# SPDX-License-Identifier: GPL-3.0-or-later
"""T3587 standing regression replay of main.

    python3 -m tools.regress run                      # newest origin/main, all standing routes
    python3 -m tools.regress run --sha 5d1df27546 --route story-hud-weapons --no-bisect
    python3 -m tools.regress status                   # print tmp/regress/status.json
    python3 -m tools.regress bisect --route story-hud-weapons --good <sha> --bad <sha>
    python3 -m tools.regress unblock --reason "fixed by T35xx"
    python3 -m tools.regress publish                  # copy receipts into tracked docs/data/regress

Replays each standing owner route (full Story Level 1, Story level 1, Map Maker Assault, full
Arcade) headless through the replay queue (fast-forward, remote-first) on the prebuilt host of a
main sha, with the settings `sh tools/dev/play.sh` uses by default (`--rust-ports=all,player`, the
adopted REA AI host default, `--rust-fileio`, the default-variant TSFP_* environment). A route
passes only when it is fault-free all the way to the recording's route end: no host fault, no panic,
no early exit, the route-end poll reached. Per route and sha it writes a receipt (sha, route,
verdict, first fault poll and message, digest), plus `tmp/regress/status.json` for the supervisor.

On a FAULT it bisects the first-parent main landings since the route's last good sha (only the
failing route is replayed, once per distinct build-input hash), posts the first bad sha over coord,
and writes the adoption block marker (tools.regress_block) that makes `tools.retire` and
`tools.agents.merge_task` refuse default-on adoptions until the fault is fixed. The marker is
removed by the next run in which every route passes. Raw run data is deleted after the verdict
(T3467); the receipts keep the digests. See docs/t3587-regression-replay.md.

Exit codes: 0 all PASS, 1 a FAULT, 2 an ERROR (no verdict: host or inputs unavailable), 3 another
run is active, 4 TIMEOUT only.
"""

from __future__ import annotations

import argparse
import concurrent.futures
import contextlib
import fcntl
import gzip
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import threading
import time
from collections.abc import Callable, Iterator
from dataclasses import dataclass
from pathlib import Path

from tools import regress_block, verification_store
from tools.agents import primary_fetch
from tools.wt import prebuilt

ROOT = Path(__file__).resolve().parents[1]
SCHEMA = 1
# Standing routes and their expected paks are owned by tools/data/regress_routes.json (rule 45,
# T3819: only routes the desync sweep measured in sync count as gameplay coverage). The recordings
# and their bound saves live in the main checkout's tmp/owner/recordings and are never tracked.
ROUTES_FILE = ROOT / "tools" / "data" / "regress_routes.json"


def load_routes(path: Path = ROUTES_FILE) -> tuple[dict[str, str], dict[str, str | None]]:
    """(name -> recording stem, name -> expected pak regex or None) from the route config."""
    data = json.loads(path.read_text())
    routes = {name: entry["stem"] for name, entry in data["routes"].items()}
    expect = {name: entry.get("expect_pak") for name, entry in data["routes"].items()}
    expect.update(data.get("expect_pak_extra") or {})
    if not routes:
        raise ValueError(f"{path}: no standing routes")
    return routes, expect


ROUTES, EXPECT_PAK = load_routes()
# Proof that the replay followed the owner's path and not just ran to the recording's last poll
# (T3592: a route can reach its route-end poll without the gameplay level ever loading, a
# fidelity bug of the replay and not a pass). A regex over the disc paks the host opened
# (`pak/story/l_2_st.pak`); None = no expectation yet, the opened paks are still recorded.
STORY_PAK = r"pak/story/l_\d+_st\.pak"


def expected_pak(route_name: str) -> str | None:
    """The pak a route must open, by name; any other `story-*` recording expects a Story level."""
    if route_name in EXPECT_PAK:
        return EXPECT_PAK[route_name]
    return STORY_PAK if route_name.startswith("story-") else None


PAK_PATH = re.compile(r"\\pak\\[^\"\s]*?\.pak", re.I)
# What `sh tools/dev/play.sh` (variant default) selects, as far as a headless replay can honour it
# (tests/test_regress.py compares this with the `default)` line of play.sh, so drift is caught).
# rust-rea-ai is the host default since the T2385 adoption: the replay queue otherwise passes
# --no-rust-rea-ai. The wgpu/window arguments do not apply with --render none. rust_trace adds the
# native input arguments (--input-router --mouse-keyboard --raw-look) and TSFP_UI=rust itself.
# -0x153270: T3398 Map Maker crash mitigation (play.sh blocked_roots)
DEFAULT_PORTS = ("all", "player", "-0x153270", "rust-rea-ai")
DEFAULT_HOST_ARGS = ("--rust-fileio",)
DEFAULT_ENV = (
    "TSFP_RUST_KMEM=1",
    "TSFP_UI_DISPLAY_RATE=1",
    "TSFP_NATIVE_OBJECT_TRANSFORM=1",
    "TSFP_HUD_NATIVE=core",
)
# Experiments only (`run --env TSFP_X=1`): extra host environment (cache key and receipt).
EXTRA_ENV: tuple[str, ...] = ()
# `run --gpu`: a paced replay with the GPU renderer (local, through the same queue) instead of the
# headless fast-forward one; it reaches the render path the headless replay skips (T3592).
GPU_REPLAY = False
KEEP_LOGS = False  # --keep-log: pack the full run.log of every verdict, not only a FAULT's
REGIONS = "tools/data/rust_trace_regions.json"
MAX_IDLE = 30000  # tools.gate WINDOW_MAX_IDLE: idle dumps are the samples, the default cap is 800
MAX_DUMPS = 2000
# rust_trace turns the animation shadow comparison on (a verification mode that panics on a
# mismatch, T1986); play does not run it, so the standing replay runs the plain ports.
ANIM_SHADOW = "off"
SECONDS = 1800
ROUTE_MARGIN_S = (
    900  # per-route wall limit = SECONDS budget + this (env TSFP_REGRESS_ROUTE_MARGIN_S)
)
RUN_MARGIN_S = 1800  # whole-run limit = route waves + this (env TSFP_REGRESS_RUN_MARGIN_S)
BISECT_PROBES = 6
HOST_ATTEMPTS = 4  # remote builder busy or absent: retry before giving up with an ERROR
HOST_WAIT_S = 90
HOST_BUSY = "HOST-BUSY"  # every remote builder busy for the whole wait: retried next loop, no alert
UNBUILDABLE = "UNBUILDABLE"
EXIT = {
    "PASS": 0,
    "FAULT": 1,
    "ERROR": 2,
    "TIMEOUT": 4,
    "REPLAY-FIDELITY": 5,
    HOST_BUSY: 6,
    UNBUILDABLE: 7,
}

# Every guest thread end prints a "stopped:" line, most of them orderly (a thread leaving through
# PsTerminateSystemThread, the host closed at the route end). Only a reason naming a fault counts.
FAULT_STOP = re.compile(
    r"^guest thread (0x[0-9A-Fa-f]+) \(entered (0x[0-9A-Fa-f]+)\) (?:stopped|STOPPED): (.+)$", re.M
)
BAD_STOP_REASON = re.compile(
    r"fault|no implementation|unimplemented|unhandled|abort|illegal|exception|crash", re.I
)
FAULT_FIELD = re.compile(r"^\s+(signal|fault address|fault rip|detail)\s+(.+?)\s*$", re.M)
HOST_CRASH = re.compile(r"^=== HOST CRASH[^\n]*===\n([^\n]*)", re.M)
PANIC = re.compile(r"panicked at [^\n]*(?:\n[^\n]*)?")
FAULT_OTHER = re.compile(
    r"(thread '[^']*' panicked at [^\n]*|non-unwinding panic[^\n]*"
    r"|STOPPED: XDK function with no implementation[^\n]*"
    r"|Segmentation fault[^\n]*|stack overflow[^\n]*|SIGSEGV[^\n]*)"
)


# ------------------------------------------------------------------ paths and state


def main_checkout() -> Path:
    return regress_block.main_checkout()


def work_dir() -> Path:
    configured = os.environ.get("TSFP_REGRESS_DIR")
    return Path(configured) if configured else main_checkout() / "tmp/regress"


def read_json(path: Path, default: object) -> object:
    try:
        return json.loads(path.read_text())
    except (OSError, json.JSONDecodeError):
        return default


def write_json(path: Path, value: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_suffix(path.suffix + ".tmp")
    temporary.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")
    temporary.replace(path)


def now() -> str:
    return time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())


_HELD: dict[str, object] = {}


def _pid_alive(pid: int) -> bool:
    try:
        os.kill(pid, 0)
    except ProcessLookupError:
        return False
    except OSError:
        return True
    return True


def _holder_dead(directory: Path) -> bool:
    """True when run.pid names a process that no longer exists (a lock held by an orphan)."""
    try:
        return not _pid_alive(int((directory / "run.pid").read_text().strip()))
    except (OSError, ValueError):
        return False


def _try_lock(directory: Path):  # noqa: ANN202
    handle = (directory / "run.lock").open("a")
    try:
        fcntl.flock(handle, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except OSError:
        handle.close()
        return None
    return handle


def release_lock() -> None:
    """Drop run.lock and run.pid now (also called by the wall-timeout watchdog)."""
    handle = _HELD.pop("handle", None)
    directory = _HELD.pop("directory", None)
    if directory is not None:
        with contextlib.suppress(OSError):
            (Path(str(directory)) / "run.pid").unlink()
    if handle is not None:
        with contextlib.suppress(OSError, ValueError):
            fcntl.flock(handle, fcntl.LOCK_UN)  # type: ignore[arg-type]
            handle.close()  # type: ignore[attr-defined]


@contextlib.contextmanager
def exclusive_run(directory: Path) -> Iterator[bool]:
    """Single-flight lock: yields False when another live run is active.

    The holder pid is kept in run.pid. A lock whose recorded holder is dead (an orphaned child
    kept the descriptor) is reclaimed: the lock file is replaced and locked afresh (I-T3678)."""
    directory.mkdir(parents=True, exist_ok=True)
    handle = _try_lock(directory)
    if handle is None and _holder_dead(directory):
        with contextlib.suppress(OSError):
            (directory / "run.lock").unlink()
        handle = _try_lock(directory)
    if handle is None:
        yield False
        return
    (directory / "run.pid").write_text(f"{os.getpid()}\n")
    _HELD.update(handle=handle, directory=directory)
    try:
        yield True
    finally:
        release_lock()


# ------------------------------------------------------------------ wall-timeout watchdog


def route_wall_s(seconds: int) -> int:
    """Wall limit of one route replay: its budget plus a margin (build, queue, remote copy)."""
    margin = int(os.environ.get("TSFP_REGRESS_ROUTE_MARGIN_S", ROUTE_MARGIN_S))
    return int(os.environ.get("TSFP_REGRESS_ROUTE_WALL_S", seconds + margin))


def run_wall_s(seconds: int, routes: int, jobs: int) -> int:
    """Wall limit of a whole run: the route waves plus a margin for fetch, build and bisect."""
    margin = int(os.environ.get("TSFP_REGRESS_RUN_MARGIN_S", RUN_MARGIN_S))
    waves = -(-max(1, routes) // max(1, jobs))
    return int(os.environ.get("TSFP_REGRESS_RUN_WALL_S", waves * route_wall_s(seconds) + margin))


def descendants(pid: int) -> list[int]:
    """Live descendant pids of `pid` (from /proc, children before their parents)."""
    children: dict[int, list[int]] = {}
    for entry in Path("/proc").iterdir():
        if not entry.name.isdigit():
            continue
        try:
            fields = (entry / "stat").read_text().rsplit(")", 1)[1].split()
            children.setdefault(int(fields[1]), []).append(int(entry.name))
        except (OSError, IndexError, ValueError):
            continue
    found: list[int] = []
    stack = [pid]
    while stack:
        for child in children.get(stack.pop(), []):
            found.append(child)
            stack.append(child)
    return found[::-1]


def kill_run_tree() -> None:
    """SIGKILL every descendant of this run (replay, ssh, cargo) and their process groups."""
    own_group = os.getpgrp()
    for pid in descendants(os.getpid()):
        with contextlib.suppress(OSError):
            group = os.getpgid(pid)
            if group != own_group:
                os.killpg(group, 9)
        with contextlib.suppress(OSError):
            os.kill(pid, 9)


class Watchdog:
    """Named wall deadlines; the first to expire records TIMEOUT, kills the run, releases the lock.

    `clock`, `kill`, `exit_fn` are injectable so tests run on a fake clock without threads."""

    def __init__(
        self,
        directory: Path,
        sha: str,
        *,
        clock: Callable[[], float] = time.monotonic,
        kill: Callable[[], None] = kill_run_tree,
        exit_fn: Callable[[int], None] = os._exit,
    ) -> None:
        self.directory, self.sha = directory, sha
        self.clock, self.kill, self.exit_fn = clock, kill, exit_fn
        self.deadlines: dict[str, tuple[float, str, str]] = {}
        self.lock = threading.Lock()
        self.fired: dict | None = None
        self.stopped = False

    def arm(self, key: str, seconds: float, route: str, step: str) -> None:
        with self.lock:
            self.deadlines[key] = (self.clock() + seconds, route, step)

    def disarm(self, key: str) -> None:
        with self.lock:
            self.deadlines.pop(key, None)

    def check(self) -> bool:
        """Fire once if any deadline has passed. True when it fired."""
        with self.lock:
            now_ = self.clock()
            late = [(d, k, r, s) for k, (d, r, s) in self.deadlines.items() if d <= now_]
            if not late or self.fired:
                return bool(self.fired)
            _, key, route, step = min(late)
            self.fired = {"key": key, "route": route, "step": step}
        write_json(
            self.directory / "status.json",
            {
                "schema": SCHEMA,
                "updated": now(),
                "state": "TIMEOUT",
                "verdict": "TIMEOUT",
                "tested_sha": self.sha,
                "route": route,
                "step": step,
                "message": f"wall timeout in {step} (route {route or '-'})",
                "routes": {},
            },
        )
        print(f"regress: TIMEOUT route {route or '-'} step {step}", file=sys.stderr, flush=True)
        self.kill()
        release_lock()
        self.exit_fn(EXIT["TIMEOUT"])
        return True

    def start(self, interval: float = 5.0) -> None:
        def loop() -> None:
            while not self.stopped and not self.check():
                time.sleep(interval)

        threading.Thread(target=loop, daemon=True, name="regress-watchdog").start()


_WATCHDOG: Watchdog | None = None


@contextlib.contextmanager
def wall_guard(key: str, seconds: float, route: str, step: str) -> Iterator[None]:
    """Arm the active watchdog for one step (a no-op outside `run`)."""
    dog = _WATCHDOG
    if dog is None:
        yield
        return
    dog.arm(key, seconds, route, step)
    try:
        yield
    finally:
        dog.disarm(key)


# ------------------------------------------------------------------ verdict classification


def scan_log(text: str) -> dict | None:
    """The first fault in a replay's run.log: a stopped guest thread, a panic or a guest stop."""
    stop = next(
        (found for found in FAULT_STOP.finditer(text) if BAD_STOP_REASON.search(found.group(3))),
        None,
    )
    if stop:
        fields: dict[str, str] = {}
        for found in FAULT_FIELD.finditer(text, stop.end(), stop.end() + 4000):
            fields.setdefault(found.group(1), found.group(2))
        parts = [f"guest thread {stop.group(1)} (entered {stop.group(2)}) stopped: {stop.group(3)}"]
        parts += [
            f"{key}={fields[key]}"
            for key in ("signal", "fault address", "fault rip", "detail")
            if key in fields
        ]
        return {"kind": "host-fault", "message": "; ".join(parts)[:600], "offset": stop.start()}
    crash = HOST_CRASH.search(text)
    if crash:  # the host's own crash handler (T2145): signal line plus the first panic message
        # (the first one is the cause, a later "panic in a function that cannot unwind" is not)
        panic = PANIC.search(text, 0, crash.start())
        parts = [crash.group(1).strip()]
        if panic:
            parts.append(" ".join(panic.group(0).split()))
        return {"kind": "host-crash", "message": "; ".join(parts)[:600], "offset": crash.start()}
    other = FAULT_OTHER.search(text)
    if other:
        return {"kind": "panic-or-stop", "message": other.group(1)[:600], "offset": other.start()}
    return None


def last_traced_poll(trace: dict | None) -> int | None:
    polls = [
        s.get("poll") for s in (trace or {}).get("samples", []) if isinstance(s.get("poll"), int)
    ]
    return max(polls) if polls else None


def opened_paks(log_text: str) -> list[str]:
    """The disc paks the replay opened, as `pak/story/l_2_st.pak` (sorted, deduplicated)."""
    return sorted(
        {
            found.group(0).lower().replace("\\", "/").lstrip("/")
            for found in PAK_PATH.finditer(log_text)
        }
    )


def classify(
    receipt: dict | None,
    trace: dict | None,
    log_text: str,
    *,
    budget_seconds: int = SECONDS,
    expect_pak: str | None = None,
) -> dict:
    """PASS / FAULT / TIMEOUT / ERROR for one replay.

    PASS needs: a receipt with exit code 0, a trace, the route-end poll reached and no fault in
    the log, and, when `expect_pak` is given, a pak matching it opened (else REPLAY-FIDELITY: the
    replay ran to the end without following the owner's path, which says nothing about the build
    and never blocks adoptions). A fault (log signature, non-zero exit, output cap, or a clean
    exit before the route end) is a FAULT with the last traced poll before it. A run cut by the
    wall budget without a fault is a TIMEOUT (slow or hung, not a crash). No receipt or trace is
    an ERROR (no verdict).
    """
    if receipt is None:
        return {"verdict": "ERROR", "message": "the replay queue returned no receipt"}
    meta = (trace or {}).get("meta", {})
    last_poll = last_traced_poll(trace)
    route_end = meta.get("route_end_poll")
    base = {
        "exit_code": receipt.get("exit_code"),
        "route_end_poll": route_end,
        "recorded_route_end_poll": meta.get("recorded_route_end_poll"),
        "last_traced_poll": last_poll,
        "host_exit": meta.get("host_exit"),
        "replay_seconds": meta.get("seconds"),
    }
    fault = scan_log(log_text)
    if fault:
        return {
            **base,
            "verdict": "FAULT",
            "first_fault_poll": last_poll,
            "message": f"{fault['kind']}: {fault['message']}",
        }
    if trace is None:
        tail = " | ".join(str(line)[-200:] for line in receipt.get("capture_output_tail", [])[-3:])
        if receipt.get("exit_code"):
            return {
                **base,
                "verdict": "FAULT",
                "first_fault_poll": None,
                "message": f"replay exited {receipt.get('exit_code')} without a trace: {tail}",
            }
        return {**base, "verdict": "ERROR", "message": "the replay left no trace.json"}
    if meta.get("output_cap_error"):
        return {
            **base,
            "verdict": "FAULT",
            "first_fault_poll": last_poll,
            "message": f"output cap exceeded: {meta['output_cap_error']}"[:300],
        }
    if receipt.get("exit_code"):
        return {
            **base,
            "verdict": "FAULT",
            "first_fault_poll": last_poll,
            "message": f"replay exited {receipt['exit_code']} (host exit {meta.get('host_exit')}) "
            f"before the route end, no fault signature in the log",
        }
    if not isinstance(route_end, int) or route_end <= 0:
        seconds = meta.get("seconds")
        if isinstance(seconds, (int, float)) and seconds >= 0.97 * budget_seconds:
            return {
                **base,
                "verdict": "TIMEOUT",
                "first_fault_poll": None,
                "message": f"wall budget {budget_seconds} s ended the replay at poll {last_poll}"
                " before the route end (slow or hung)",
            }
        return {
            **base,
            "verdict": "FAULT",
            "first_fault_poll": last_poll,
            "message": f"early exit: the host stopped (exit {meta.get('host_exit')}, no fault "
            f"signature in the log) at poll {last_poll} before the route-end poll "
            f"({meta.get('recorded_route_end_poll')})",
        }
    paks = opened_paks(log_text)
    base["paks_opened"] = paks[:60]
    if expect_pak and not any(re.search(expect_pak, pak) for pak in paks):
        return {
            **base,
            "verdict": "REPLAY-FIDELITY",
            "first_fault_poll": None,
            "message": f"route end reached but no pak matching {expect_pak} was opened "
            f"({len(paks)} paks): the replay did not reach the gameplay level",
        }
    desync = meta.get("desync_poll")
    base["desync_poll"] = desync if isinstance(desync, int) else None
    known = None
    if base["desync_poll"] is not None:
        from tools import replay_desync

        route = Path(str(receipt.get("route") or meta.get("route") or "")).stem
        known = replay_desync.known_desync(route, {**(meta.get("desync") or {}), **base})
        base["fidelity_baseline"] = "known" if known else "new"
    if base["desync_poll"] is not None and known is None:
        first = (meta.get("desync") or {}).get("first_divergence")
        return {
            **base,
            "verdict": "REPLAY-FIDELITY",
            "first_fault_poll": None,
            "message": f"route end reached but the replay desynced from the owner's session at "
            f"poll {base['desync_poll']} (T3819 {first})"[:300],
        }
    message = "route end reached"
    if known is not None:
        message += f" (known replay desync at poll {base['desync_poll']}, fidelity baseline T3819)"
    return {**base, "verdict": "PASS", "first_fault_poll": None, "message": message}


def digest_of(result: dict, recording_sha256: str, input_hash: str, log_sha256: str | None) -> str:
    """Stable digest of the verdict inputs (what a rerun must reproduce)."""
    material = {
        "verdict": result["verdict"],
        "message": result.get("message"),
        "route_end_poll": result.get("route_end_poll"),
        "last_traced_poll": result.get("last_traced_poll"),
        "recording_sha256": recording_sha256,
        "input_hash": input_hash,
        # the run log itself carries host paths and timings, so it is not part of the digest
    }
    return hashlib.sha256(json.dumps(material, sort_keys=True).encode()).hexdigest()


# ------------------------------------------------------------------ bisection


def first_bad(candidates: list[str], is_good: Callable[[str], bool]) -> str:
    """The first bad commit of `candidates` (oldest to newest, the last one known bad)."""
    low, high = 0, len(candidates) - 1
    while low < high:
        middle = (low + high) // 2
        if is_good(candidates[middle]):
            low = middle + 1
        else:
            high = middle
    return candidates[low]


def collapse_by_input(shas: list[str], input_hash: Callable[[str], str]) -> list[str]:
    """Keep the first landing of each run of equal build-input hashes (equal host, equal replay)."""
    kept: list[str] = []
    previous = None
    for sha in shas:
        current = input_hash(sha)
        if current != previous:
            kept.append(sha)
            previous = current
    return kept


# ------------------------------------------------------------------ backend (git, prebuilt, queue)


@dataclass(frozen=True)
class Route:
    name: str
    stem: str
    recording: Path
    save_dir: Path | None
    recording_sha256: str


class HostBusy(RuntimeError):
    """Every remote builder stayed busy through the bounded wait; not a failure, retried later."""


class LeaseLost(Exception):
    """The prebuilt host vanished (retention) between ensure_host and its lease."""


class Backend:
    """Everything that touches git, the prebuilt cache or the replay queue; tests replace it."""

    def __init__(self, allow_local_build: bool = False) -> None:
        self.allow_local_build = allow_local_build
        self.owner_root = main_checkout()

    def git(self, *args: str) -> str:
        result = subprocess.run(
            ["git", *args], cwd=ROOT, check=False, text=True, capture_output=True, timeout=300
        )
        if result.returncode:
            raise RuntimeError(f"git {' '.join(args)}: {result.stderr.strip()[-300:]}")
        return result.stdout.strip()

    def fetch(self) -> None:
        result = primary_fetch.fetch("fetch", "origin", "main", cwd=self.owner_root)
        if result.returncode:
            raise RuntimeError(f"git fetch origin main: {(result.stderr or '').strip()[-300:]}")

    def resolve(self, ref: str) -> str:
        return self.git("rev-parse", "--verify", f"{ref}^{{commit}}")

    def landings(self, good: str, bad: str) -> list[str]:
        """First-parent main landings in (good, bad], oldest first."""
        out = self.git("rev-list", "--first-parent", "--reverse", f"{good}..{bad}")
        return out.split()

    def is_ancestor(self, older: str, newer: str) -> bool:
        return (
            subprocess.run(
                ["git", "merge-base", "--is-ancestor", older, newer], cwd=ROOT, check=False
            ).returncode
            == 0
        )

    def subject(self, sha: str) -> str:
        return self.git("log", "-1", "--format=%s", sha)

    def input_hash(self, sha: str) -> str:
        from tools import prebuilt_select

        return prebuilt_select.build_input_hash(ROOT, sha)

    def ensure_host(self, sha: str, input_hash: str) -> Path:
        """The prebuilt host for the sha's build inputs: reuse, else a remote build.

        A busy remote builder is retried (HOST_ATTEMPTS times, HOST_WAIT_S apart), re-checking
        the cache each time: the supervisor's warm-main build may publish the host meanwhile.
        """
        from tools import prebuilt_select
        from tools.wt import prebuilt

        error: Exception | None = None
        for attempt in range(HOST_ATTEMPTS):
            host = prebuilt_select.find_host(ROOT, commit=sha, host_input_hash=input_hash)
            if prebuilt_select.available(host):
                return host
            try:
                return prebuilt.build(ROOT, sha=sha, remote_only=not self.allow_local_build)
            except prebuilt.PrebuiltError as caught:
                if isinstance(caught, prebuilt.BuildFailed):
                    raise
                error = caught
                if attempt + 1 < HOST_ATTEMPTS:
                    time.sleep(HOST_WAIT_S)
        if isinstance(error, prebuilt.RemoteUnavailable):
            raise HostBusy(f"remote builders busy for {sha[:12]}: {error}") from None
        raise RuntimeError(f"no prebuilt host for {sha[:12]}: {error}") from None

    @contextlib.contextmanager
    def lease(self, host: Path) -> Iterator[Path]:
        """Hold the host's artifact lease so prebuilt retention cannot delete it mid-replay."""
        from tools.wt import prebuilt

        try:
            held = prebuilt.lease(host)
            leased = held.__enter__()
        except prebuilt.PrebuiltError as error:
            raise LeaseLost(str(error)) from None
        try:
            yield leased
        finally:
            held.__exit__(None, None, None)

    def route(self, name: str) -> Route:
        from tools import adoption_gate, gate, trace_store

        stem = ROUTES.get(name, name)
        recording = gate.resolve_route(stem, self.owner_root)
        try:
            saves = adoption_gate.route_save_dir(recording)
        except (ValueError, OSError) as error:
            raise RuntimeError(f"{recording.name}: bound saves unusable: {error}") from None
        return Route(name, recording.stem, recording, saves, trace_store.sha256(recording))

    def disc(self) -> Path:
        from tools import owner_files

        disc = owner_files.disc_image(ROOT)
        if not disc.is_file():
            raise RuntimeError(f"owner disc not found at {disc} (set TSFP_DISC)")
        return disc.resolve()

    def replay(self, route: Route, sha: str, input_hash: str, budget: int) -> dict:
        """One headless fast-forward replay through the queue; returns the verdict fields."""
        from tools import adoption_gate as ag

        started = time.monotonic()
        cwd_switches = (*DEFAULT_PORTS, *DEFAULT_ENV, *EXTRA_ENV)
        try:
            receipt = ag.queue_submit(
                self.owner_root,
                str(route.recording),
                cwd_switches,
                sha,
                regions=ROOT / REGIONS,
                seconds=budget,
                no_cache=False,
                allow_capture_failure=True,
                save_dir=route.save_dir,
                host_args=DEFAULT_HOST_ARGS,
                stop_at_route_end=True,
                headless=not GPU_REPLAY,
                paced=GPU_REPLAY,
                render="gpu" if GPU_REPLAY else None,
                max_idle=MAX_IDLE,
                max_dumps=MAX_DUMPS,
                host_input_hash=input_hash,
                disc=self.disc(),
                anim_shadow=ANIM_SHADOW,
            )
        except RuntimeError as error:
            return {
                "verdict": "ERROR",
                "message": f"replay queue failed: {str(error)[-500:]}",
                "wall_seconds": round(time.monotonic() - started, 1),
            }
        run_dir = Path(receipt["run_dir"])
        trace = read_json(run_dir / "trace.json", None)
        log_path = run_dir / "run.log"
        log_text = log_path.read_text(errors="replace") if log_path.is_file() else ""
        result = classify(
            receipt,
            trace if isinstance(trace, dict) else None,
            log_text,
            budget_seconds=budget,
            expect_pak=expected_pak(route.name),
        )
        result["wall_seconds"] = round(time.monotonic() - started, 1)
        result["cache_hit"] = bool(receipt.get("cache_hit"))
        result["remote_host"] = receipt.get("remote_host")
        result["replay_key"] = receipt.get("key")
        result["run_log_sha256"] = (
            hashlib.sha256(log_text.encode(errors="replace")).hexdigest() if log_text else None
        )
        if (result["verdict"] == "FAULT" or KEEP_LOGS) and log_text:
            keep = work_dir() / "faults"
            keep.mkdir(parents=True, exist_ok=True)
            evidence = keep / f"{sha[:12]}-{route.name}.log"
            evidence.write_text(fault_excerpt(log_text))
            with gzip.open(keep / f"{sha[:12]}-{route.name}.run.log.gz", "wt") as packed:
                packed.write(log_text)  # the full log, packed: a fault needs its whole context
            result["fault_excerpt_file"] = str(evidence)
        shutil.rmtree(run_dir, ignore_errors=True)  # T3467: raw run data goes after the verdict
        return result

    def say(self, text: str) -> None:
        subprocess.run(
            [
                sys.executable,
                "-m",
                "tools.agents.coord",
                "say",
                "--owner",
                "claude-regress",
                "--to",
                "all",
                text,
            ],
            cwd=ROOT,
            check=False,
            timeout=120,
            capture_output=True,
        )


def fault_excerpt(log_text: str, before: int = 12, after: int = 40) -> str:
    """The fault block of a run.log with context (the full log is raw data and is deleted)."""
    fault = scan_log(log_text)
    if fault is None:
        return "\n".join(log_text.splitlines()[-60:]) + "\n"
    lines = log_text.splitlines()
    index = log_text.count("\n", 0, fault["offset"])
    return "\n".join(lines[max(0, index - before) : index + after]) + "\n"


# ------------------------------------------------------------------ the run


def config_key() -> str:
    """Identity of the replay settings: a changed default re-tests every cached verdict."""
    material = json.dumps(
        [
            DEFAULT_PORTS,
            DEFAULT_HOST_ARGS,
            DEFAULT_ENV,
            EXTRA_ENV,
            REGIONS,
            ANIM_SHADOW,
            GPU_REPLAY,
            SCHEMA,
        ],
        sort_keys=True,
    )
    return hashlib.sha256(material.encode()).hexdigest()[:12]


def state_key(input_hash: str, route: Route, seconds: int = SECONDS) -> str:
    return f"{input_hash}:{route.stem}:{route.recording_sha256[:12]}:{seconds}:{config_key()}"


def receipt_for(sha: str, input_hash: str, route: Route, result: dict, seconds: int) -> dict:
    return {
        "schema": SCHEMA,
        "sha": sha,
        "input_hash": input_hash,
        "route": route.name,
        "recording": route.stem,
        "recording_sha256": route.recording_sha256,
        "seconds": seconds,
        "settings": {
            "ports": ",".join(DEFAULT_PORTS),
            "host_args": list(DEFAULT_HOST_ARGS),
            "env": [*DEFAULT_ENV, *EXTRA_ENV],
            "anim_shadow": ANIM_SHADOW,
            "mode": "paced gpu, stop at route end"
            if GPU_REPLAY
            else "headless fast-forward, stop at route end",
        },
        "tested_at": now(),
        **result,
        "digest": digest_of(
            result, route.recording_sha256, input_hash, result.get("run_log_sha256")
        ),
    }


def receipt_variant() -> str:
    """File-name suffix of a non-default replay (gpu, extra switches), so variants never collide."""
    parts = ["gpu"] if GPU_REPLAY else []
    if EXTRA_ENV:
        parts.append("x" + hashlib.sha256("|".join(EXTRA_ENV).encode()).hexdigest()[:8])
    return "".join(f"-{part}" for part in parts)


def save_receipt(directory: Path, receipt: dict) -> Path:
    seconds = receipt.get("seconds", SECONDS)
    path = directory / (
        f"{receipt['sha'][:12]}-{receipt['route']}-s{seconds}{receipt_variant()}.json"
    )
    write_json(path, receipt)
    return path


def test_route(
    backend: Backend, route: Route, sha: str, input_hash: str, budget: int, state: dict, force: bool
) -> dict:
    """One route on one sha, reusing a verdict of an equal build-input hash and settings.

    A replay that must actually run first ensures the sha's prebuilt host (reuse, else build):
    a bisect or probe midpoint has no host of its own, and the replay queue refuses one without
    ("shared prebuilt host missing", T3587 incident). A host that cannot be had is an ERROR.
    """
    key = state_key(input_hash, route, budget)
    cached = state.get("tested", {}).get(key)
    if cached and not force and cached.get("verdict") in ("PASS", "FAULT"):
        return {**cached, "reused_from_sha": cached.get("sha"), "wall_seconds": 0.0}
    store_key, request = verification_store.request_key(
        build_input_hash=input_hash,
        profile="regress",
        route=route.recording_sha256,
        start=0,
        seconds=budget,
        switches=(*DEFAULT_PORTS, *DEFAULT_HOST_ARGS, *DEFAULT_ENV, *EXTRA_ENV),
        extra={"route_name": route.name, "config": config_key(), "gpu": GPU_REPLAY},
    )
    store_root = getattr(backend, "owner_root", None)
    if not force and store_root is not None:
        with verification_store.request_lock(store_root, store_key):
            shared = verification_store.get(store_root, store_key, request)
            if shared and shared.get("verdict") in ("PASS", "FAULT"):
                return {**shared, "reused_from_sha": shared.get("sha"), "wall_seconds": 0.0}
            result = host_then_replay(backend, route, sha, input_hash, budget)
            if result.get("verdict") in ("PASS", "FAULT"):
                verification_store.put(store_root, store_key, request, {**result, "sha": sha})
            return result
    result = host_then_replay(backend, route, sha, input_hash, budget)
    if result.get("verdict") in ("PASS", "FAULT") and store_root is not None:
        verification_store.put(
            store_root, store_key, request, {**result, "sha": sha}, replace=force
        )
    return result


def host_then_replay(
    backend: Backend, route: Route, sha: str, input_hash: str, budget: int
) -> dict:
    """Ensure the sha's prebuilt host, then replay; a missing host is an ERROR verdict."""
    started = time.monotonic()
    error: Exception | None = None
    # Retention may delete a just-restored host before the lease is taken: re-ensure once.
    for _attempt in range(2):
        try:
            host = backend.ensure_host(sha, input_hash)
            with backend.lease(host):
                with wall_guard(f"route:{route.name}", route_wall_s(budget), route.name, "replay"):
                    return backend.replay(route, sha, input_hash, budget)
        except LeaseLost as lost:
            error = lost
        except RuntimeError as caught:
            error = caught
            break
    return {
        "verdict": HOST_BUSY
        if isinstance(error, HostBusy)
        else UNBUILDABLE
        if isinstance(error, prebuilt.BuildFailed)
        else "ERROR",
        "message": f"prebuilt host unavailable: {str(error)[-500:]}",
        "wall_seconds": round(time.monotonic() - started, 1),
    }


def update_state(
    state: dict, sha: str, input_hash: str, route: Route, receipt: dict, seconds: int
) -> None:
    summary = {
        key: receipt.get(key)
        for key in (
            "verdict",
            "message",
            "first_fault_poll",
            "route_end_poll",
            "digest",
            "wall_seconds",
        )
    }
    summary["sha"] = sha
    if receipt.get("verdict") in ("PASS", "FAULT"):
        state.setdefault("tested", {})[state_key(input_hash, route, seconds)] = summary
    if receipt.get("verdict") == "PASS":
        state.setdefault("last_good", {})[route.name] = {
            "sha": sha,
            "input_hash": input_hash,
            "at": now(),
        }


def bisect_route(
    backend: Backend,
    route: Route,
    good: str,
    bad: str,
    budget: int,
    state: dict,
    log: Callable[[str], None],
) -> dict:
    """First bad landing for `route` between a known-good and a known-bad sha."""
    landings = backend.landings(good, bad)
    candidates = collapse_by_input(landings, backend.input_hash)
    if not candidates:
        return {"first_bad_sha": bad, "candidates": 0, "note": "no landings between good and bad"}
    if backend.input_hash(candidates[-1]) != backend.input_hash(bad):
        candidates.append(bad)
    log(f"bisect {route.name}: {len(landings)} landings, {len(candidates)} distinct host builds")
    tested: dict[str, str] = {}

    def verdict(sha: str) -> str:
        input_hash = backend.input_hash(sha)
        outcome = test_route(backend, route, sha, input_hash, budget, state, force=False)
        tested[sha] = outcome["verdict"]
        log(f"bisect {route.name}: {sha[:12]} -> {outcome['verdict']}")
        if outcome["verdict"] in ("ERROR", "REPLAY-FIDELITY", HOST_BUSY):
            raise RuntimeError(f"bisect inconclusive at {sha[:12]}: {outcome.get('message')}")
        return outcome["verdict"]

    # Search remaining untested builds between the known-good and known-bad boundaries.
    # A skipped midpoint carries no ordering information; try its nearest neighbor.
    low, high = -1, len(candidates) - 1
    skipped: set[int] = set()
    while True:
        available = [i for i in range(low + 1, high) if i not in skipped]
        if not available:
            break
        midpoint = (low + high) // 2
        index = min(available, key=lambda i: (abs(i - midpoint), i))
        result = verdict(candidates[index])
        if result == UNBUILDABLE:
            skipped.add(index)
        elif result in ("PASS", "TIMEOUT"):
            low = index
        else:
            high = index
    bordering = sorted(i for i in skipped if low < i < high)
    answer = candidates[high]
    group = [sha for sha in landings if backend.input_hash(sha) == backend.input_hash(answer)]
    return {
        "first_bad_sha": None if bordering else answer,
        "first_bad_subject": None if bordering else backend.subject(answer),
        "skipped": [candidates[i] for i in sorted(skipped)],
        "range": {"last_good": good if low < 0 else candidates[low], "first_bad": answer}
        if bordering
        else None,
        "same_host_landings": [sha[:12] for sha in group],
        "candidates": len(candidates),
        "replays": len(tested),
        "tested": {sha[:12]: verdict for sha, verdict in tested.items()},
    }


def probe_for_good(
    backend: Backend, route: Route, bad: str, budget: int, state: dict, log: Callable[[str], None]
) -> str | None:
    """No known-good sha for a failing route: walk back through distinct host builds."""
    history = backend.landings(f"{bad}~400", bad)
    builds = collapse_by_input(history, backend.input_hash)
    step = 1
    tried = 0
    index = len(builds) - 1
    while tried < BISECT_PROBES and index - step >= 0:
        index -= step
        step *= 2
        sha = builds[index]
        outcome = test_route(backend, route, sha, backend.input_hash(sha), budget, state, False)
        tried += 1
        log(f"probe {route.name}: {sha[:12]} -> {outcome['verdict']}")
        if outcome["verdict"] == "PASS":
            return sha
    return None


def error_reason(receipts: list[dict]) -> str:
    """The distinct ERROR messages of a run, route-tagged, for status and the alert."""
    reasons: dict[str, list[str]] = {}
    for receipt in receipts:
        if receipt["verdict"] == "ERROR":
            message = str(receipt.get("message") or "no message")
            reasons.setdefault(message, []).append(receipt["route"])
    return "; ".join(f"{', '.join(names)}: {message}" for message, names in reasons.items())[:600]


def build_status(
    sha: str,
    receipts: list[dict],
    bisections: dict[str, dict],
    state: dict,
    blocked: dict | None,
) -> dict:
    verdicts = {r["verdict"] for r in receipts}
    overall = (
        "FAULT"
        if "FAULT" in verdicts
        else "ERROR"
        if "ERROR" in verdicts
        else "REPLAY-FIDELITY"
        if "REPLAY-FIDELITY" in verdicts
        else HOST_BUSY
        if HOST_BUSY in verdicts
        else UNBUILDABLE
        if UNBUILDABLE in verdicts
        else "TIMEOUT"
        if "TIMEOUT" in verdicts
        else "PASS"
    )
    good_all = state.get("last_good_all")
    return {
        "schema": SCHEMA,
        "updated": now(),
        "state": overall,
        "tested_sha": sha,
        "last_good_sha": (good_all or {}).get("sha"),
        "last_good_at": (good_all or {}).get("at"),
        "routes": {
            r["route"]: {
                key: r.get(key)
                for key in (
                    "verdict",
                    "wall_seconds",
                    "first_fault_poll",
                    "message",
                    "route_end_poll",
                    "digest",
                    "cache_hit",
                    "remote_host",
                    "reused_from_sha",
                )
            }
            for r in receipts
        },
        "bisections": bisections,
        "adoption_blocked": blocked is not None,
        "block": blocked,
        **({"error": error_reason(receipts)} if overall == "ERROR" else {}),
    }


def run(args: argparse.Namespace, backend: Backend | None = None) -> int:
    """One regression run under the wall-timeout watchdog (verdict TIMEOUT, exit 4, on expiry)."""
    global _WATCHDOG
    dog = Watchdog(work_dir(), "")
    _WATCHDOG = dog
    dog.start()
    try:
        return _run_locked(args, backend, dog)
    finally:
        dog.stopped = True
        _WATCHDOG = None


def _run_locked(args: argparse.Namespace, backend: Backend | None, dog: Watchdog) -> int:
    backend = backend or Backend(allow_local_build=args.allow_local_build)
    global EXTRA_ENV, GPU_REPLAY, KEEP_LOGS
    GPU_REPLAY = bool(getattr(args, "gpu", False))
    KEEP_LOGS = bool(getattr(args, "keep_log", False))
    EXTRA_ENV = (*(getattr(args, "env", None) or ()), *(getattr(args, "switch", None) or ()))
    directory = work_dir()
    log = lambda text: print(f"regress: {text}", file=sys.stderr, flush=True)  # noqa: E731
    if args.sha == "origin/main" and not args.no_fetch:
        backend.fetch()
    sha = backend.resolve(args.sha)
    if not backend.is_ancestor(sha, backend.resolve("origin/main")):
        # A lane branch probe keeps its own state: writing the standing state made main read
        # FAULT on every route for a retire-branch merge that was never on main (I-T3879).
        # It also takes its own lock, so branch probes no longer queue behind each other or
        # behind main's run (five waited on one slot, 2026-10-29); replays still go through the
        # replay queue's per-host slots.
        directory = directory / "branches" / sha[:12]
        directory.mkdir(parents=True, exist_ok=True)
        log(f"{sha[:12]} is not on origin/main: state in {directory}")
    with exclusive_run(directory) as free:
        if not free:
            log("another regression run is active; nothing to do")
            return 3
        state = read_json(directory / "state.json", {})
        if not isinstance(state, dict):
            state = {}
        names = args.route or list(ROUTES)
        dog.sha = sha
        dog.arm("run", run_wall_s(args.seconds, len(names), args.jobs), "", "run")
        if not args.force and not args.route and state.get("last_run_sha") == sha:
            log(f"{sha[:12]} was already tested ({state.get('last_run_state')}); use --force")
            return 0
        write_json(
            directory / "status.json",
            {
                "schema": SCHEMA,
                "updated": now(),
                "state": "RUNNING",
                "tested_sha": sha,
                "pid": os.getpid(),
                "routes": {},
                "adoption_blocked": regress_block.read() is not None,
            },
        )
        input_hash = backend.input_hash(sha)
        routes: list[Route] = []
        receipts: list[dict] = []
        for name in names:
            try:
                routes.append(backend.route(name))
            except (RuntimeError, ValueError, OSError) as error:
                receipts.append(
                    {
                        "schema": SCHEMA,
                        "sha": sha,
                        "input_hash": input_hash,
                        "route": name,
                        "seconds": args.seconds,
                        "verdict": "ERROR",
                        "message": f"route unavailable: {error}"[:400],
                        "tested_at": now(),
                    }
                )
        host_error = None
        host_busy = False
        host_unbuildable = False
        if routes:
            try:
                backend.ensure_host(sha, input_hash)
            except RuntimeError as error:
                host_error = str(error)[:500]
                host_busy = isinstance(error, HostBusy)
                host_unbuildable = isinstance(error, prebuilt.BuildFailed)
        receipt_dir = Path(args.receipt_dir) if args.receipt_dir else directory / "receipts"
        if host_error:
            for route in routes:
                receipts.append(
                    {
                        "schema": SCHEMA,
                        "sha": sha,
                        "input_hash": input_hash,
                        "route": route.name,
                        "seconds": args.seconds,
                        "verdict": HOST_BUSY
                        if host_busy
                        else UNBUILDABLE
                        if host_unbuildable
                        else "ERROR",
                        "message": host_error,
                        "tested_at": now(),
                    }
                )
        elif routes:
            budget = args.seconds
            with concurrent.futures.ThreadPoolExecutor(max_workers=max(1, args.jobs)) as pool:
                futures = {
                    pool.submit(
                        test_route, backend, route, sha, input_hash, budget, state, args.force
                    ): route
                    for route in routes
                }
                for future in concurrent.futures.as_completed(futures):
                    route = futures[future]
                    result = future.result()
                    receipt = receipt_for(sha, input_hash, route, result, budget)
                    receipts.append(receipt)
                    update_state(state, sha, input_hash, route, receipt, budget)
                    log(
                        f"{route.name}: {receipt['verdict']} in {receipt.get('wall_seconds')} s"
                        f" {receipt.get('message', '')[:160]}"
                    )
        for receipt in receipts:
            save_receipt(receipt_dir, receipt)
        faults = [r for r in receipts if r["verdict"] == "FAULT"]
        # Only the main tip is allowed to block adoptions or announce: a probe of an older sha
        # (a proof run, a bisect step by hand) reports its verdict and nothing else.
        try:
            is_tip = sha == backend.resolve("origin/main")
        except RuntimeError:
            is_tip = False
        publish_effects = is_tip and not args.no_block
        bisections: dict[str, dict] = {}
        blocked = regress_block.read()
        route_by_name = {route.name: route for route in routes}
        for receipt in faults:
            name = receipt["route"]
            route = route_by_name[name]
            good = (state.get("last_good", {}).get(name) or {}).get("sha")
            if not args.no_bisect:
                try:
                    if good is None or not backend.is_ancestor(good, sha):
                        good = probe_for_good(backend, route, sha, args.seconds, state, log)
                    bisections[name] = (
                        bisect_route(backend, route, good, sha, args.seconds, state, log)
                        if good
                        else {
                            "first_bad_sha": None,
                            "note": "no passing sha within the probe horizon",
                        }
                    )
                except RuntimeError as error:
                    bisections[name] = {"first_bad_sha": None, "note": str(error)[:300]}
            found = bisections.get(name, {}).get("first_bad_sha")
            record = {
                "route": name,
                "sha": sha,
                "first_bad_sha": found or sha,
                "first_fault_poll": receipt.get("first_fault_poll"),
                "message": receipt.get("message"),
                "bisected": bool(found),
                "last_good_sha": good,
                "receipt": f"{sha[:12]}-{name}.json",
            }
            if not publish_effects:
                continue
            if blocked is None or blocked.get("route") == name:
                regress_block.write(record)
                blocked = regress_block.read()
            boundary = bisections.get(name, {}).get("range")
            culprit = (
                f"first bad between {boundary['last_good'][:12]} and {boundary['first_bad'][:12]} "
                "(unbuildable commits skipped)"
                if boundary
                else f"first bad sha {found[:12]}"
                if found
                else "first bad sha unknown"
            )
            backend.say(
                f"REGRESS FAULT route {name}: {culprit} "
                f"(tested tip {sha[:12]}, last good {(good or 'unknown')[:12]}), poll "
                f"{receipt.get('first_fault_poll')}: {str(receipt.get('message'))[:300]}. "
                "Default-on adoptions and retirements are blocked until fixed "
                "(python3 -m tools.regress status)."
            )

        def descends_from(mark: object) -> bool:
            try:
                return isinstance(mark, str) and bool(mark) and backend.is_ancestor(mark, sha)
            except RuntimeError:
                return False

        # T3778: main moves while a run lasts, so "the tip" is checked against the sha it ends
        # on. A full all-PASS run of a sha at or after the previous good one is the newest good
        # run whether or not main moved meanwhile (the stale detector reads last_good_all).
        good_all = state.get("last_good_all") or {}
        if (
            not args.no_block
            and not args.route
            and receipts
            and all(r["verdict"] == "PASS" for r in receipts)
            and (publish_effects or not good_all.get("sha") or descends_from(good_all.get("sha")))
        ):
            state["last_good_all"] = {"sha": sha, "at": now()}
        if not faults and not args.route and not args.no_block:
            # The marker goes once the faulting route passes again and no route faults, at the
            # tip or at a later sha than the blocking one; a REPLAY-FIDELITY or TIMEOUT of
            # another route says nothing about the crash.
            marked_route = (blocked or {}).get("route")
            marked_passes = any(
                r["route"] == marked_route and r["verdict"] == "PASS" for r in receipts
            )
            later = publish_effects or descends_from((blocked or {}).get("sha"))
            if blocked and later and marked_passes and regress_block.clear():
                log(f"{marked_route} passes again and no route faults: adoption block cleared")
                blocked = None
        overall = build_status(sha, receipts, bisections, state, blocked)["state"]
        if not args.route and overall not in ("ERROR", HOST_BUSY, UNBUILDABLE):  # no verdict
            state["last_run_sha"] = sha
            state["last_run_state"] = overall
            state["last_verdict_at"] = now()
        if not args.route and overall == "ERROR":
            # T3778: a run without a verdict is recorded and announced, never dropped silently;
            # the regress-error HEALTH detector reads last_error against last_verdict_at.
            reason = error_reason(receipts)
            previous = state.get("last_error") or {}
            state["last_error"] = {"sha": sha, "at": now(), "reason": reason}
            if not args.no_block and previous.get("reason") != reason:
                backend.say(
                    f"REGRESS ERROR at {sha[:12]}: no verdict ({reason[:300]}); retried next "
                    "loop (python3 -m tools.regress status)"
                )
        write_json(directory / "state.json", state)
        status = build_status(sha, receipts, bisections, state, blocked)
        write_json(directory / "status.json", status)
        print(json.dumps(status, indent=2, sort_keys=True))
        states = [r["verdict"] for r in receipts]
        for verdict in ("FAULT", "ERROR", "REPLAY-FIDELITY", HOST_BUSY, UNBUILDABLE, "TIMEOUT"):
            if verdict in states:
                return EXIT[verdict]
        return 0


# ------------------------------------------------------------------ other subcommands


def live_status(directory: Path) -> dict:
    """status.json, with a RUNNING record whose process died shown as INTERRUPTED."""
    status = read_json(directory / "status.json", {"state": "NEVER-RUN"})
    if not isinstance(status, dict):
        return {"state": "NEVER-RUN"}
    pid = status.get("pid")
    if status.get("state") == "RUNNING" and isinstance(pid, int):
        try:
            os.kill(pid, 0)
        except ProcessLookupError:
            return {**status, "state": "INTERRUPTED"}
        except PermissionError:
            pass
    return status


def cmd_status(_args: argparse.Namespace) -> int:
    print(json.dumps(live_status(work_dir()), indent=2, sort_keys=True))
    return 0


def cmd_bisect(args: argparse.Namespace) -> int:
    backend = Backend(allow_local_build=args.allow_local_build)
    route = backend.route(args.route)
    state = read_json(work_dir() / "state.json", {})
    if not isinstance(state, dict):
        state = {}
    outcome = bisect_route(
        backend,
        route,
        backend.resolve(args.good),
        backend.resolve(args.bad),
        args.seconds,
        state,
        lambda text: print(f"regress: {text}", file=sys.stderr, flush=True),
    )
    write_json(work_dir() / "state.json", state)
    print(json.dumps(outcome, indent=2, sort_keys=True))
    return 0


def cmd_unblock(args: argparse.Namespace) -> int:
    record = regress_block.read()
    if record is None:
        print("regress: no adoption block marker")
        return 0
    regress_block.clear()
    history = work_dir() / "unblocked.jsonl"
    history.parent.mkdir(parents=True, exist_ok=True)
    with history.open("a") as handle:
        handle.write(
            json.dumps({"at": now(), "reason": args.reason, "was": record}, sort_keys=True) + "\n"
        )
    print(f"regress: adoption block cleared ({args.reason})")
    return 0


def cmd_publish(args: argparse.Namespace) -> int:
    """Copy untracked receipts into docs/data/regress/ of the current checkout (to commit)."""
    source = Path(args.receipt_dir) if args.receipt_dir else work_dir() / "receipts"
    destination = ROOT / "docs/data/regress"
    destination.mkdir(parents=True, exist_ok=True)
    copied = 0
    for path in sorted(source.glob("*.json")):
        if args.sha and not path.name.startswith(args.sha[:12]):
            continue
        shutil.copy2(path, destination / path.name)
        copied += 1
    print(f"regress: published {copied} receipt(s) to {destination}")
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="python3 -m tools.regress",
        description=__doc__,
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    sub = parser.add_subparsers(dest="command")

    def add_run_options(p: argparse.ArgumentParser) -> None:
        p.add_argument(
            "--env",
            action="append",
            default=[],
            metavar="TSFP_X=V",
            help="extra host environment for an experiment (changes the settings key)",
        )
        p.add_argument(
            "--keep-log",
            action="store_true",
            help="keep every verdict's packed run.log in tmp/regress/faults (default: FAULT only)",
        )
        p.add_argument(
            "--gpu",
            action="store_true",
            help="paced replay with the GPU renderer, local (default: headless fast-forward)",
        )
        p.add_argument(
            "--switch",
            action="append",
            default=[],
            metavar="-0xVA",
            help="extra port switch for an experiment, e.g. -0x216230 removes that root from "
            "`all` (changes the settings key)",
        )
        p.add_argument(
            "--sha", default="origin/main", help="commit or ref to test (default origin/main)"
        )
        p.add_argument(
            "--route",
            action="append",
            help=f"standing route, repeatable: {', '.join(ROUTES)} or a stem (default all)",
        )
        p.add_argument(
            "--no-fetch", action="store_true", help="do not `git fetch origin main` first"
        )
        p.add_argument(
            "--no-bisect", action="store_true", help="report a FAULT without bisecting it"
        )
        p.add_argument(
            "--no-block",
            action="store_true",
            help="no block marker or coord announcement (implied off the origin/main tip)",
        )
        p.add_argument(
            "--force",
            action="store_true",
            help="re-test even when this sha or host build was tested",
        )
        p.add_argument(
            "--jobs",
            type=int,
            default=4,
            help="routes replayed at once (the queue and remote workers limit it further)",
        )
        p.add_argument("--seconds", type=int, default=SECONDS, help="wall budget per route replay")
        p.add_argument(
            "--receipt-dir",
            help="receipt directory (default tmp/regress/receipts, see `publish`)",
        )
        p.add_argument(
            "--allow-local-build",
            action="store_true",
            help="build a missing prebuilt here when no remote worker can (default: remote only)",
        )

    run_parser = sub.add_parser("run", help="test a main sha (the default command)")
    add_run_options(run_parser)
    sub.add_parser("status", help="print tmp/regress/status.json")
    bisect_parser = sub.add_parser("bisect", help="first bad landing of one route between two shas")
    bisect_parser.add_argument("--route", required=True)
    bisect_parser.add_argument("--good", required=True)
    bisect_parser.add_argument("--bad", required=True)
    bisect_parser.add_argument("--seconds", type=int, default=SECONDS)
    bisect_parser.add_argument("--allow-local-build", action="store_true")
    unblock_parser = sub.add_parser("unblock", help="clear the adoption block marker by hand")
    unblock_parser.add_argument("--reason", required=True)
    publish_parser = sub.add_parser("publish", help="copy receipts into tracked docs/data/regress")
    publish_parser.add_argument("--receipt-dir")
    publish_parser.add_argument("--sha", help="only receipts of this sha")
    return parser


REPLAYING_COMMANDS = frozenset({"run", "bisect"})


def apply_remote_hosts() -> None:
    """Remote-first replay: use the registry file the supervisor maintains unless one is preset."""
    if os.environ.get("TSFP_REPLAY_REMOTE_HOSTS", "").strip():
        return
    candidate = Path(
        os.environ.get(
            "TSFP_REGRESS_HOSTS_FILE", main_checkout() / "tmp/codexapi/replay-hosts.json"
        )
    )
    with contextlib.suppress(OSError):
        text = candidate.read_text().strip()
        if text:
            os.environ["TSFP_REPLAY_REMOTE_HOSTS"] = text
    # At most two replays at once on the owner's PC when no worker is admissible.
    if not os.environ.get("TSFP_REPLAY_HEADLESS_SLOTS"):
        os.environ["TSFP_REPLAY_HEADLESS_SLOTS"] = "2"


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    tokens = list(sys.argv[1:] if argv is None else argv)
    commands = {"run", "status", "bisect", "unblock", "publish"}
    if not any(token in commands for token in tokens[:1]) and not {"-h", "--help"} & set(tokens):
        tokens.insert(0, "run")  # `python3 -m tools.regress --sha X` means `run`
    args = parser.parse_args(tokens)
    if args.command in REPLAYING_COMMANDS:  # status, unblock and publish touch no replay queue
        apply_remote_hosts()
    handler = {
        "run": run,
        "status": cmd_status,
        "bisect": cmd_bisect,
        "unblock": cmd_unblock,
        "publish": cmd_publish,
    }[args.command]
    return handler(args)


if __name__ == "__main__":
    raise SystemExit(main())
