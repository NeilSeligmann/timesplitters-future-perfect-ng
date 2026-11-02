"""T2360: bounded file output monitoring for live host runs."""

from __future__ import annotations

import subprocess
import time
from pathlib import Path


class OutputCapExceeded(RuntimeError):
    pass


def directory_size(paths: list[Path]) -> tuple[int, int]:
    count = size = 0
    for root in paths:
        if root.exists():
            for path in root.rglob("*"):
                try:
                    if path.is_file():
                        count += 1
                        size += path.stat().st_size
                except OSError:
                    continue
    return count, size


def check(paths: list[Path], max_files: int, max_bytes: int) -> tuple[int, int]:
    files, size = directory_size(paths)
    if (max_files and files > max_files) or (max_bytes and size > max_bytes):
        raise OutputCapExceeded(
            f"output cap exceeded: {files} files, {size} bytes "
            f"(limits {max_files} files, {max_bytes} bytes)"
        )
    return files, size


def stop_after_cap(host: subprocess.Popen, grace_seconds: float = 5.0) -> int:
    """Stop this selected process immediately, with bounded TERM/KILL waits."""
    host.terminate()
    try:
        return host.wait(timeout=grace_seconds)
    except subprocess.TimeoutExpired:
        host.kill()
        return host.wait(timeout=grace_seconds)


def monitor(
    host: subprocess.Popen,
    paths: list[Path],
    max_files: int,
    max_bytes: int,
    interval: float = 0.2,
) -> bool:
    """Wait for host; terminate it once aggregate output limits are exceeded."""
    while host.poll() is None:
        try:
            check(paths, max_files, max_bytes)
        except OutputCapExceeded as cap:
            status = stop_after_cap(host)
            raise OutputCapExceeded(f"{cap}; host stopped (exit {status})") from cap
        time.sleep(interval)
    return False
