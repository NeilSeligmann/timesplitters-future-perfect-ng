"""Bounded, host-attributed cache lookup events for rolling health checks."""

from __future__ import annotations

import fcntl
import json
import os
import re
import socket
import time
from datetime import UTC, datetime, timedelta
from pathlib import Path
from typing import Any

from tools import datadir

DAY_SECONDS = 24 * 60 * 60


def telemetry_root(root: Path | None = None) -> Path | None:
    """Where events go; None under pytest without an explicit directory (T3758: tests faking
    replays wrote cache misses into the real telemetry the cache-hit-rate HEALTH check reads)."""
    if root is not None:
        return root
    configured = os.environ.get("TSFP_CACHE_TELEMETRY_DIR")
    if configured:
        return Path(configured)
    if os.environ.get("PYTEST_CURRENT_TEST") and not os.environ.get("TSFP_DATA_DIR"):
        return None
    return datadir.data_dir("wt-cache/cache-telemetry")


def record_lookup(
    cache: str,
    hit: bool,
    *,
    host: str | None = None,
    root: Path | None = None,
    now: float | None = None,
) -> bool:
    """Append one cache lookup without allowing telemetry failures to fail the caller."""
    if not re.fullmatch(r"[a-z0-9][a-z0-9_-]{0,63}", cache) or not isinstance(hit, bool):
        return False
    host = host or os.environ.get("TSFP_REMOTE_EXEC_HOST") or socket.gethostname()
    if not host or len(host) > 255:
        return False
    stamp = time.time() if now is None else now
    date = datetime.fromtimestamp(stamp, UTC).strftime("%Y%m%d")
    safe_host = re.sub(r"[^A-Za-z0-9._-]+", "-", host)[:80] or "unknown"
    directory = telemetry_root(root)
    if directory is None:
        return False
    path = directory / f"{safe_host}-{date}.jsonl"
    record = json.dumps({"ts": stamp, "host": host, "cache": cache, "hit": hit}, sort_keys=True)
    try:
        path.parent.mkdir(parents=True, exist_ok=True)
        descriptor = os.open(path, os.O_CREAT | os.O_WRONLY | os.O_APPEND, 0o600)
        try:
            fcntl.flock(descriptor, fcntl.LOCK_EX)
            os.write(descriptor, (record + "\n").encode())
        finally:
            os.close(descriptor)
    except OSError:
        return False
    return True


def record_counts(
    cache: str,
    hits: int,
    misses: int,
    *,
    host: str | None = None,
    root: Path | None = None,
    now: float | None = None,
) -> bool:
    """Append one aggregate counter sample, for tools that expose per-run totals."""
    if (
        not re.fullmatch(r"[a-z0-9][a-z0-9_-]{0,63}", cache)
        or isinstance(hits, bool)
        or isinstance(misses, bool)
        or not isinstance(hits, int)
        or not isinstance(misses, int)
        or hits < 0
        or misses < 0
        or hits + misses == 0
    ):
        return False
    host = host or os.environ.get("TSFP_REMOTE_EXEC_HOST") or socket.gethostname()
    if not host or len(host) > 255:
        return False
    stamp = time.time() if now is None else now
    date = datetime.fromtimestamp(stamp, UTC).strftime("%Y%m%d")
    safe_host = re.sub(r"[^A-Za-z0-9._-]+", "-", host)[:80] or "unknown"
    directory = telemetry_root(root)
    if directory is None:
        return False
    path = directory / f"{safe_host}-{date}.jsonl"
    record = json.dumps(
        {"ts": stamp, "host": host, "cache": cache, "hits": hits, "misses": misses},
        sort_keys=True,
    )
    try:
        path.parent.mkdir(parents=True, exist_ok=True)
        descriptor = os.open(path, os.O_CREAT | os.O_WRONLY | os.O_APPEND, 0o600)
        try:
            fcntl.flock(descriptor, fcntl.LOCK_EX)
            os.write(descriptor, (record + "\n").encode())
        finally:
            os.close(descriptor)
    except OSError:
        return False
    return True


def rolling_counts(
    root: Path | None = None, *, now: float | None = None
) -> dict[tuple[str, str], dict[str, dict[str, int]]]:
    """Count previous and current 24-hour windows from at most three UTC day files."""
    end = time.time() if now is None else now
    current_start = end - DAY_SECONDS
    baseline_start = current_start - DAY_SECONDS
    base = telemetry_root(root)
    counts: dict[tuple[str, str], dict[str, dict[str, int]]] = {}
    if base is None:
        return counts
    days = {
        (datetime.fromtimestamp(end, UTC) - timedelta(days=offset)).strftime("%Y%m%d")
        for offset in range(3)
    }
    for day in days:
        for path in base.glob(f"*-{day}.jsonl"):
            if path.is_symlink():
                continue
            try:
                stream = path.open(encoding="utf-8")
            except OSError:
                continue
            with stream:
                for line in stream:
                    try:
                        event: Any = json.loads(line)
                    except json.JSONDecodeError:
                        continue
                    if not isinstance(event, dict):
                        continue
                    stamp, host, cache, hit, hits, misses = (
                        event.get("ts"),
                        event.get("host"),
                        event.get("cache"),
                        event.get("hit"),
                        event.get("hits"),
                        event.get("misses"),
                    )
                    if (
                        isinstance(stamp, bool)
                        or not isinstance(stamp, (int, float))
                        or not isinstance(host, str)
                        or not isinstance(cache, str)
                        or not (
                            isinstance(hit, bool)
                            or (
                                isinstance(hits, int)
                                and not isinstance(hits, bool)
                                and hits >= 0
                                and isinstance(misses, int)
                                and not isinstance(misses, bool)
                                and misses >= 0
                                and hits + misses > 0
                            )
                        )
                        or not baseline_start <= stamp < end
                    ):
                        continue
                    window = "baseline" if stamp < current_start else "observed"
                    pair = (host, cache)
                    data = counts.setdefault(
                        pair,
                        {
                            "baseline": {"hits": 0, "misses": 0},
                            "observed": {"hits": 0, "misses": 0},
                        },
                    )
                    if isinstance(hit, bool):
                        data[window]["hits" if hit else "misses"] += 1
                    else:
                        data[window]["hits"] += hits
                        data[window]["misses"] += misses
    return counts
