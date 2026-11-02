# SPDX-License-Identifier: GPL-3.0-or-later
"""Parse the host's stop report (T741): stop names, addresses, call census, last guest calls.

The host prints `stopped: <reason>` for the main thread and `guest thread 0x.. (entered 0x..)
stopped: <reason>` for each guest thread, each with an optional `guest address`/`icall target`
line, then `HLE calls reached ... (N recorded, T total: K kernel ordinal, X XDK address, M
monitor notify)`.  A thread that returned or called PsTerminateSystemThread is a normal end, not
a stop.  Parsing is by line shape only and returns what it found; nothing is guessed.
"""

import re
from dataclasses import dataclass, field

NORMAL_ENDS = ("guest entry point returned", "guest thread exited via PsTerminateSystemThread")
_THREAD = re.compile(r"^guest thread (0x[0-9a-fA-F]+) \(entered (0x[0-9A-Fa-f]+)\) stopped: (.+)$")
_MAIN = re.compile(r"^main thread \(entry point (0x[0-9A-Fa-f]+)\) stopped:$")
_REASON = re.compile(r"^stopped: (.+)$")
_ADDRESS = re.compile(r"^\s+(?:guest address|icall target)\s+(0x[0-9A-Fa-f]+)")
_DETAIL = re.compile(r"^\s+detail\s+(.*)$")
_CENSUS = re.compile(
    r"^HLE calls reached.*\((\d+) recorded, (\d+) total: (\d+) kernel ordinal, "
    r"(\d+) XDK address, (\d+) monitor notify\)"
)
_RING = re.compile(r"^\s+((?:[0-9A-F]{8} )+)\s*$")
# T3786: the host's file report counts relative opens whose root handle it could not resolve.
_RELATIVE_REFUSED = re.compile(r"^refused, relative\s+(\d+)\b")
# T3799: native page commands waited on a page not ready for input (the UI froze for the player).
_INPUT_STALLED = re.compile(r"native page input stalled: .*$")
_INPUT_UNHANDLED = re.compile(r"native input unhandled: .*$")


@dataclass
class ThreadStop:
    thread: str
    reason: str
    address: str = ""
    detail: str = ""

    @property
    def is_stop(self) -> bool:
        return self.reason not in NORMAL_ENDS


@dataclass
class Census:
    recorded: int
    total: int
    kernel: int
    xdk: int
    monitor: int


@dataclass
class StopReport:
    threads: list[ThreadStop] = field(default_factory=list)
    census: Census | None = None
    recent_indirect_targets: list[str] = field(default_factory=list)
    watchdog: str = ""
    # sup-l1crash: where a host fault really was (a Rust port leaf is not "lifted code")
    fault_lines: list[str] = field(default_factory=list)
    rust_roots: list[str] = field(default_factory=list)
    relative_refused: int = 0

    @property
    def stops(self) -> list[ThreadStop]:
        return [thread for thread in self.threads if thread.is_stop]


def parse(text: str) -> StopReport:
    report = StopReport()
    current: ThreadStop | None = None
    in_ring = False
    in_frames = False
    for line in text.splitlines():
        main = _MAIN.match(line)
        thread = _THREAD.match(line)
        if thread:
            current = ThreadStop(thread[1], thread[3])
            report.threads.append(current)
            in_ring = False
            continue
        if main:
            current = None
            in_ring = False
            report.threads.append(ThreadStop("main", ""))
            continue
        reason = _REASON.match(line)
        if reason and report.threads and report.threads[-1].thread == "main":
            report.threads[-1].reason = reason[1]
            current = report.threads[-1]
            continue
        if current is not None:
            address = _ADDRESS.match(line)
            detail = _DETAIL.match(line)
            if address and not current.address:
                current.address = address[1]
            elif detail and not current.detail:
                current.detail = detail[1]
        stripped = line.strip()
        if stripped.startswith(("fault address ", "fault rip ")):
            report.fault_lines.append(stripped)
            in_frames = stripped.startswith("fault rip ")
            continue
        if stripped.startswith("host frames "):
            in_frames = True
            continue
        if in_frames and stripped.startswith("#") and len(report.fault_lines) < 12:
            report.fault_lines.append("  " + stripped)
            continue
        if stripped and not stripped.startswith("#"):
            in_frames = False
        if stripped.startswith("last Rust-port roots entered (oldest first):"):
            report.rust_roots = stripped.split(":", 1)[1].split()
            continue
        if "recent indirect targets" in line:
            in_ring = True
            continue
        ring = _RING.match(line) if in_ring else None
        if ring:
            report.recent_indirect_targets = ring[1].split()
            in_ring = False
        census = _CENSUS.match(line)
        if census:
            report.census = Census(*(int(group) for group in census.groups()))
        if line.startswith("WATCHDOG:"):
            report.watchdog = line
        relative = _RELATIVE_REFUSED.match(line)
        if relative:
            report.relative_refused = int(relative[1])
    return report


_CRASH_BLOCKS = (
    ("=== HOST CRASH (T2145) ===", "=== END HOST CRASH ==="),
    ("=== RUST PANIC (T2145) ===", "=== END RUST PANIC ==="),
)


def crash_blocks(err_text: str) -> list[str]:
    """T2145: the host crash blocks (abort handler) found in stderr, whole, in order."""
    found: list[str] = []
    current: list[str] | None = None
    end = ""
    for line in err_text.splitlines():
        if current is None:
            for begin, finish in _CRASH_BLOCKS:
                if line.strip() == begin:
                    current, end = [line], finish
                    break
        else:
            current.append(line)
            if line.strip() == end:
                found.extend(current)
                current = None
    if current:
        found.extend(current)  # cut off mid-block: keep what was written
    return found


def input_stalls(err_text: str) -> list[str]:
    """T3799 (I-T3799-1): the native page input stall lines in stderr, in order."""
    return [match[0] for match in map(_INPUT_STALLED.search, err_text.splitlines()) if match]


def input_unhandled(err_text: str) -> list[str]:
    """T3809 (I-T3809-1): activations that reached a row kind nothing handles, in order."""
    return [match[0] for match in map(_INPUT_UNHANDLED.search, err_text.splitlines()) if match]


def render(report: StopReport, last_calls: int = 12, err_text: str = "") -> list[str]:
    lines = ["", "=== play: stop report ==="]
    crash = crash_blocks(err_text)
    if not report.threads:
        if crash:
            return [
                *lines,
                "no stop report: the host crashed first, details from host.err follow",
                *crash,
            ]
        return [*lines, "no stop report found: the host did not reach its report (see host.err)"]
    stops = report.stops
    for stop in stops:
        where = f" at {stop.address}" if stop.address else ""
        lines.append(f"STOP  thread {stop.thread}: {stop.reason}{where}")
        if stop.detail:
            lines.append(f"      detail: {stop.detail}")
    if not stops:
        lines.append("no stop: every thread ended normally")
    if report.watchdog:
        lines.append(report.watchdog)
    if report.census is not None:
        c = report.census
        lines.append(
            f"CALL CENSUS  {c.total} total ({c.kernel} kernel, {c.xdk} XDK, {c.monitor} monitor)"
        )
    if report.recent_indirect_targets:
        tail = report.recent_indirect_targets[-last_calls:]
        lines.append(f"LAST GUEST CALLS (indirect targets, oldest first): {' '.join(tail)}")
    if report.rust_roots:
        roots = report.rust_roots[-last_calls:]
        lines.append(f"LAST RUST-PORT ROOTS (oldest first): {' '.join(roots)}")
    for fault_line in report.fault_lines:
        lines.append(f"FAULT {fault_line}")
    if report.relative_refused:
        lines.append(
            f"WARN  {report.relative_refused} directory-relative file open(s) REFUSED: a save "
            "delete or content removal did nothing (I-T3786-1, host.err 'outside bounded "
            "drive-root policy')"
        )
    for stall in input_stalls(err_text):
        lines.append(f"WARN  UI input froze: {stall}")
    for unhandled in input_unhandled(err_text):
        lines.append(f"WARN  UI input ignored: {unhandled}")
    return lines


def primary_stop(report: StopReport) -> ThreadStop | None:
    """The first real stop that names an address, else the first real stop, else None."""
    stops = report.stops
    return next((s for s in stops if s.address), stops[0] if stops else None)
