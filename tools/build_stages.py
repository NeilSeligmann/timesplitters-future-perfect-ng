"""Record build-stage wall time and only newly appended Ninja edges (T3863).

Receipts contain timing metadata, never command arguments, compiler output or sources.
Edge sums include parallel work and must not be added to stage wall times.
"""

from __future__ import annotations

import argparse
import json
import os
import signal
import subprocess
import time
from dataclasses import dataclass
from pathlib import Path
from types import FrameType


class StageInterrupted(Exception):
    def __init__(self, signum: int) -> None:
        self.signum = signum


def interrupt(signum: int, _frame: FrameType | None) -> None:
    raise StageInterrupted(signum)


@dataclass(frozen=True)
class Cursor:
    inode: int
    offset: int
    anchor: bytes


def cursor(path: Path) -> Cursor | None:
    try:
        with path.open("rb") as stream:
            stat = os.fstat(stream.fileno())
            stream.seek(max(0, stat.st_size - 256))
            return Cursor(stat.st_ino, stat.st_size, stream.read(256))
    except FileNotFoundError:
        return None


def edge_kind(output: str) -> str:
    name = Path(output).name
    if name.startswith("recomp_") and name.endswith(".c.o"):
        return "lifted_c"
    if name.endswith(".rlib") or name == "libtsfp_ports.a":
        return "rust_archive"
    if name.endswith((".c.o", ".cc.o", ".cpp.o")):
        return "native_c"
    if name.endswith((".a", ".so")) or name.startswith("tsfp_host"):
        return "link_or_archive"
    return "other"


def ninja_delta(path: Path, before: Cursor | None) -> dict:
    """Stream the appended suffix; a rewritten log is unknown, never a cold build."""
    groups: dict[str, dict] = {}
    try:
        with path.open("rb") as stream:
            stat = os.fstat(stream.fileno())
            if before is not None:
                if stat.st_ino != before.inode or stat.st_size < before.offset:
                    raise ValueError("Ninja log replaced or truncated")
                stream.seek(max(0, before.offset - 256))
                if stream.read(len(before.anchor)) != before.anchor:
                    raise ValueError("Ninja log compacted or rewritten")
                stream.seek(before.offset)
            # Bounded reads also reject partial lines from an interrupted writer.
            while line := stream.readline(65537):
                if len(line) > 65536 or not line.endswith(b"\n"):
                    raise ValueError("oversized or incomplete Ninja row")
                if line.startswith(b"# ninja log v"):
                    continue
                fields = line.decode("utf-8").rstrip("\n").split("\t")
                if len(fields) != 5:
                    raise ValueError("invalid Ninja row")
                start, end = int(fields[0]), int(fields[1])
                if start < 0 or end < start:
                    raise ValueError("invalid Ninja interval")
                kind = edge_kind(fields[3])
                group = groups.setdefault(
                    kind, {"outputs": 0, "output_seconds": 0.0, "first_ms": start, "last_ms": end}
                )
                group["outputs"] += 1
                group["output_seconds"] += (end - start) / 1000
                group["first_ms"] = min(group["first_ms"], start)
                group["last_ms"] = max(group["last_ms"], end)
    except FileNotFoundError:
        return {"available": False, "reason": "Ninja log absent"}
    except (OSError, UnicodeError, ValueError) as exc:
        return {"available": False, "reason": str(exc)}
    for group in groups.values():
        group["output_seconds"] = round(group["output_seconds"], 6)
    return {
        "available": True,
        "groups": groups,
        "scope": "new output rows; multi-output edges may appear more than once",
    }


def run_stage(
    command: list[str], receipt: Path, stage: str, ninja_log: Path | None, timeout: float
) -> int:
    # Resolve and open before invoking a costly command; unwritable evidence fails early.
    receipt.parent.mkdir(parents=True, exist_ok=True)
    before = cursor(ninja_log) if ninja_log else None
    with receipt.open("a", encoding="utf-8") as stream:
        started = time.monotonic()
        error = None
        child = None
        previous = {sig: signal.signal(sig, interrupt) for sig in (signal.SIGTERM, signal.SIGINT)}
        try:
            child = subprocess.Popen(command, start_new_session=True)
            rc = child.wait(timeout=timeout)
        except (subprocess.TimeoutExpired, StageInterrupted) as exc:
            if child is not None:
                try:
                    os.killpg(child.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                child.wait()
            if isinstance(exc, StageInterrupted):
                rc, error = -exc.signum, "stage interrupted"
            else:
                rc, error = 124, "stage timeout"
        except OSError:
            rc, error = 127, "stage command could not start"
        finally:
            for sig, handler in previous.items():
                signal.signal(sig, handler)
        record = {
            "schema": 1,
            "stage": stage,
            "wall_seconds": round(time.monotonic() - started, 6),
            "exit": rc,
            "error": error,
        }
        if ninja_log:
            record["ninja"] = ninja_delta(ninja_log, before)
        stream.write(json.dumps(record, sort_keys=True) + "\n")
    return rc if rc >= 0 else 128 - rc


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--receipt", required=True, type=Path)
    parser.add_argument("--stage", required=True, choices=("configure", "host", "cargo", "bundle"))
    parser.add_argument("--ninja-log", type=Path)
    parser.add_argument("--timeout", type=float, default=3600)
    parser.add_argument("command", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    command = args.command[1:] if args.command[:1] == ["--"] else args.command
    if not command or not 0 < args.timeout <= 7200:
        parser.error("a command and finite timeout in (0, 7200] are required")
    return run_stage(command, args.receipt, args.stage, args.ninja_log, args.timeout)


if __name__ == "__main__":
    raise SystemExit(main())
