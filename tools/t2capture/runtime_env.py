# SPDX-License-Identifier: GPL-3.0-or-later
"""Run-time switch of the T3740 capture runtime (src/host/t2_capture.c).

Shared by rust_trace and the replay queue. A capture host (`TSFP_BUILD_CAPTURE=1 sh
tools/dev/build.sh`) records only when `TSFP_T2_CAPTURE` names the spool; the header fields of the
spool (tools/t2corpus/spool.py) come from the environment below. The lifted tree hash is compiled
into the host (CMake, the lift key), so it is not set here.
"""

from __future__ import annotations

import hashlib
import json
import mmap
import re
from pathlib import Path

SPOOL_NAME = "capture.bin"
STATS_SUFFIX = ".stats.json"
# T3767: what src/host/t2_capture.c compiles into a capture host, found in the binary itself.
_TOOL_VERSION = re.compile(rb"t2_capture\.c/(\d+)")
_SEGMENT_WRITER = b".seg-%06u.gz"  # T3745 gzip segments, the only form the compactor ingests
_RECORD_FORMAT = b'"record_format"'  # T3740 record format 2 header field


def capture_host_problem(host: Path) -> str | None:
    """None when `host` is a capture host the pipeline can use, else why not (T3767, REA 04:07).

    Probed before a capture starts, from the strings src/host/t2_capture.c compiles in: a host
    without the T3745 segment writer writes a plain spool the live compactor never ingests, one
    without the format 2 header or older than the store's current capture version records what
    the checker misjudges; each only showed after a whole capture."""
    from tools.t2corpus.store import CAPTURE_VERSION_CURRENT

    try:
        with (
            Path(host).open("rb") as source,
            mmap.mmap(source.fileno(), 0, access=mmap.ACCESS_READ) as image,
        ):
            versions = [int(match.group(1)) for match in _TOOL_VERSION.finditer(image)]
            segments = image.find(_SEGMENT_WRITER) >= 0
            record_format = image.find(_RECORD_FORMAT) >= 0
    except (OSError, ValueError) as exc:
        return f"capture host unreadable: {host}: {exc}"
    if not versions:
        return f"{host} is not a capture host (no t2_capture.c runtime): TSFP_BUILD_CAPTURE=1"
    version = max(versions)
    if not segments:
        return f"{host}: capture runtime t2_capture.c/{version} writes no gzip segments (pre-T3745)"
    if not record_format:
        return f"{host}: capture runtime t2_capture.c/{version} writes no record_format 2 header"
    if version < CAPTURE_VERSION_CURRENT:
        return (
            f"{host}: capture runtime t2_capture.c/{version} is older than the store's current "
            f"version {CAPTURE_VERSION_CURRENT}; rebuild the capture host from main"
        )
    return None


def check_capture_host(host: Path) -> None:
    """Refuse a capture with a clear message before the replay starts (ValueError)."""
    problem = capture_host_problem(host)
    if problem is not None:
        raise ValueError(f"--t2-capture: {problem}")


def file_sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with Path(path).open("rb") as source:
        for block in iter(lambda: source.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def capture_env(
    out: Path, recording: Path, build_input_hash: str | None, budget_seed: Path | None = None
) -> dict[str, str]:
    """Environment that turns recording on for one replay run, spool under its run dir.

    T3745: refuses a run dir on the root disk or over the disk budget, and hands the runtime its
    limits (byte cap, free floor, segment size, data disk anchor) from
    tools/data/disk_budget.json. T3803: `budget_seed` (tools/t2capture/budget_seed.py) seeds the
    per-VA capture budget with the corpus coverage; a named but missing seed is an error."""
    from tools import datadisk
    from tools.t2capture.budget_seed import ENV as SEED_ENV
    from tools.t2capture.budget_seed import directives_env

    datadisk.check("t2 capture", paths=[out])
    env = {
        **datadisk.capture_env(),
        "TSFP_T2_CAPTURE": str(Path(out) / SPOOL_NAME),
        "TSFP_T2_CAPTURE_RECORDING_SHA256": file_sha256(recording),
        "TSFP_T2_CAPTURE_BUILD_INPUT_HASH": f"{build_input_hash or 'unpinned'}+capture",
    }
    if budget_seed is not None:
        seed = Path(budget_seed).resolve()
        if not seed.is_file():
            raise ValueError(f"--t2-budget-seed: {seed} not found")
        env[SEED_ENV] = str(seed)
        env.update(directives_env(seed.read_text()))  # T3836: caps and allow-only of the run
    return env


def preserve_all_calls(env: dict[str, str]) -> bool:
    """All-calls runs cannot go through the sampling corpus compactor."""
    return env.get("TSFP_T2_CAPTURE_ALL_CALLS", "0") not in ("", "0")


def capture_meta(out: Path) -> dict[str, object]:
    """trace.json meta of a capture run: spool size, the runtime's counters, the compaction ledger.

    Fails loudly when the host recorded nothing: no plain spool, no segment, no compacted segment
    (a non-capture host ignores the variable)."""
    from tools.t2corpus import compact
    from tools.t2corpus.spool import segment_paths

    spool = Path(out) / SPOOL_NAME
    stats_path = spool.with_name(spool.name + STATS_SUFFIX)
    try:
        stats = json.loads(stats_path.read_text())
    except (OSError, ValueError):
        stats = {}  # killed before the exit summary: the spool itself is still valid
    ledger = compact.load_ledger(spool)
    segments = segment_paths(spool, include_open=True)
    if not spool.is_file() and not segments and not ledger.segments:
        raise ValueError(
            f"--t2-capture: no {SPOOL_NAME} or segments in {out}; is the host a "
            "TSFP_BUILD_CAPTURE=1 build? (a T3745 refusal is in run.log)"
        )
    raw = spool.stat().st_size if spool.is_file() else 0
    raw += sum(path.stat().st_size for path in segments)
    return {
        "spool": SPOOL_NAME,
        "spool_bytes": raw,
        "stats": stats,
        "truncated": bool(stats.get("truncated", False)),
        "stop_reason": stats.get("stop_reason"),
        "compaction": compact.asdict(ledger),
    }
