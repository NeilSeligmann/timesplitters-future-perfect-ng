"""Compiler-cache selection and evidence from the actual build environment (T3863)."""

from __future__ import annotations

import json
import shutil
import subprocess
from collections.abc import Callable
from pathlib import Path

from tools.wt import sccache_shared


def configure(root: Path, main: Path, env: dict[str, str]) -> bool:
    """Wire C and Rust to the same cache, including a binary installed outside PATH.

    This selects the local backend first. Shared-store routing remains owned by
    core.apply_shared_sccache, including its wrapper and password-file contract.
    """
    if env.get("TSFP_NO_CCACHE") == "1":
        for key in ("RUSTC_WRAPPER", "CMAKE_C_COMPILER_LAUNCHER", "TSFP_SCCACHE"):
            env.pop(key, None)
        return False
    binary = env.get("TSFP_SCCACHE") or shutil.which("sccache", path=env.get("PATH"))
    if not binary:
        installed = sccache_shared.sccache_bin(main, env)
        binary = str(installed) if installed else None
    if not binary:
        return False
    wrapper = root / "tools/wt/sccache-rustc.sh"
    launcher = str(wrapper) if wrapper.is_file() else binary
    env.setdefault("TSFP_SCCACHE", binary)
    env.setdefault("RUSTC_WRAPPER", launcher)
    env.setdefault("CMAKE_C_COMPILER_LAUNCHER", launcher)
    env.setdefault("SCCACHE_DIR", str(main / "tmp/wt-cache/sccache"))
    env.setdefault("SCCACHE_CACHE_SIZE", "40G")
    return True


def snapshot(
    env: dict[str, str], *, runner: Callable[..., subprocess.CompletedProcess] = subprocess.run
) -> dict:
    """Read counters without resetting a daemon shared by concurrent builds.

    Only allowlisted counters and backend kind leave this function; diagnostics
    and cache URLs may contain authentication data and are never persisted.
    """
    if env.get("TSFP_NO_CCACHE") == "1":
        return {"available": False, "reason": "disabled"}
    wrapper = env.get("RUSTC_WRAPPER", "")
    binary = env.get("TSFP_SCCACHE")
    if wrapper.endswith("sccache-rustc.sh"):
        command = ["sh", wrapper]
    elif binary:
        command = [binary]
    else:
        return {"available": False, "reason": "not_configured"}
    try:
        done = runner(
            [*command, "--show-stats", "--stats-format", "json"],
            env={**env, "CARGO_INCREMENTAL": "0"},
            capture_output=True,
            text=True,
            timeout=15,
            close_fds=True,
            cwd="/",
            check=False,
        )
        if done.returncode:
            return {"available": False, "reason": "query_failed", "exit": done.returncode}
        data = json.loads(done.stdout)
        stats = data["stats"]
        requests = stats["compile_requests"]
        if isinstance(requests, bool) or not isinstance(requests, int) or requests < 0:
            raise ValueError("invalid requests")
        counters = {"requests": requests}
        for field in ("cache_hits", "cache_misses", "cache_errors"):
            counts = stats[field]["counts"]
            if not isinstance(counts, dict) or any(
                isinstance(value, bool) or not isinstance(value, int) or value < 0
                for value in counts.values()
            ):
                raise ValueError("invalid counters")
            counters[field] = {key: counts.get(key, 0) for key in ("C/C++", "Rust")}
        location = str(data.get("cache_location", "")).lower()
        backend = (
            "webdav" if "webdav" in location else "local" if "local" in location else "unknown"
        )
        return {"available": True, "backend": backend, **counters}
    except (OSError, subprocess.SubprocessError, ValueError, KeyError, TypeError):
        return {"available": False, "reason": "invalid_or_unavailable_stats"}


def interval(before: dict, after: dict) -> dict:
    """Daemon-wide interval evidence; concurrent clients may contribute counters."""
    evidence = {"scope": "daemon_interval", "before": before, "after": after}
    if not before.get("available") or not after.get("available"):
        return {**evidence, "available": False, "reason": "missing_snapshot"}
    if before.get("backend") != after.get("backend"):
        return {**evidence, "available": False, "reason": "backend_changed"}
    delta = {"requests": after["requests"] - before["requests"]}
    for field in ("cache_hits", "cache_misses", "cache_errors"):
        delta[field] = {
            language: after[field][language] - before[field][language]
            for language in ("C/C++", "Rust")
        }
    values = [
        delta["requests"],
        *(value for field in list(delta)[1:] for value in delta[field].values()),
    ]
    if any(value < 0 for value in values):
        return {**evidence, "available": False, "reason": "counters_reset"}
    return {**evidence, "available": True, "backend": after["backend"], "delta": delta}


def summary(evidence: dict) -> dict:
    """Small log-safe projection of recorded cache evidence."""
    return {
        key: evidence[key]
        for key in ("available", "backend", "reason", "scope", "delta")
        if key in evidence
    }
