"""Share of main commits with a ready prebuilt host within N minutes (T3448).

A commit is *ready* when a published host exists for its build-input hash (T3411/T3412: the C,
Rust and cmake inputs). Readiness time comes from the durable ledger `.ready-log.jsonl` that
`tools.wt prebuilt` appends on every publish, and from the cache manifests (GC erases manifests,
the ledger survives). The *commit-keyed* column keeps the pre-T3448 semantics, where only a
host published under the commit's own SHA counted.

    python3 -m tools.prebuilt_readiness --hours 24 --minutes 10
    python3 -m tools.prebuilt_readiness --hours 24 --loop-log tmp/codexapi/sup-loop.log --json

Limits: commit time is the committer time of the first-parent (merge) commit, which is the
push time to within the guarded-merge latency. Hosts whose manifest and ledger rows were both
removed before the ledger existed are unknown, not "never built".
"""

from __future__ import annotations

import argparse
import json
import re
import statistics
import subprocess
import time
from datetime import datetime
from pathlib import Path

from tools.wt import prebuilt

TICK = re.compile(r"^(\d\d):(\d\d):(\d\d) laptop=")
FAIL = re.compile(r"prebuilt path failed: .*/prebuilt/([0-9a-f]{40})/")
STARTED = "prebuilt build started"


def _run(root: Path, argv: list[str], stdin: str | None = None) -> str:
    done = subprocess.run(
        argv, cwd=root, input=stdin, capture_output=True, text=True, check=True, timeout=300
    )
    return done.stdout


def main_commits(root: Path, ref: str, hours: float, now: float) -> list[tuple[str, float]]:
    """First-parent commits of `ref` in the window, oldest first, with committer epoch."""
    since = int(now - hours * 3600)
    out = _run(root, ["git", "log", "--first-parent", f"--since={since}", "--format=%H %ct", ref])
    rows = [line.split() for line in out.splitlines() if line.strip()]
    return [(sha, float(epoch)) for sha, epoch in reversed(rows)]


def input_hashes(root: Path, commits: list[str]) -> dict[str, str]:
    """Build-input hash per commit, computing the real hash once per distinct input tree."""
    paths = [item.rstrip("/") for item in prebuilt._BUILD_INPUTS]
    specs = "".join(f"{sha}:{path}\n" for sha in commits for path in paths)
    ids = _run(root, ["git", "cat-file", "--batch-check=%(objectname)"], specs).splitlines()
    width = len(paths)
    groups: dict[tuple[str, ...], str] = {}
    result: dict[str, str] = {}
    for index, sha in enumerate(commits):
        signature = tuple(ids[index * width : (index + 1) * width])
        if signature not in groups:
            groups[signature] = prebuilt.input_hash(root, sha)
        result[sha] = groups[signature]
    return result


def ready_times(cache: Path) -> tuple[dict[str, float], dict[str, float]]:
    """(first ready epoch per input hash, ready epoch per commit key) from ledger and cache."""
    by_hash: dict[str, float] = {}
    by_commit: dict[str, float] = {}

    def note(sha: str, wanted: str | None, epoch: float) -> None:
        by_commit[sha] = min(epoch, by_commit.get(sha, epoch))
        if wanted:
            by_hash[wanted] = min(epoch, by_hash.get(wanted, epoch))

    ledger = cache / prebuilt.READY_LOG
    if ledger.is_file():
        for line in ledger.read_text().splitlines():
            try:
                row = json.loads(line)
                note(row["sha"], row.get("input_hash"), float(row["ready_at"]))
            except (ValueError, KeyError, TypeError):
                continue
    if cache.is_dir():
        for directory in cache.iterdir():
            manifest = prebuilt._manifest(directory) if directory.is_dir() else None
            if not manifest:
                continue
            epoch = float(manifest["built_at_epoch"])
            reused = "reused_from" in manifest
            # A reuse manifest proves the host existed, but not since when: keep it keyed only.
            note(manifest["sha"], None if reused else manifest.get("input_hash"), epoch)
    return by_hash, by_commit


def measure(
    commits: list[tuple[str, float]],
    hashes: dict[str, str],
    by_hash: dict[str, float],
    by_commit: dict[str, float],
    minutes: float,
) -> dict:
    limit = minutes * 60
    hash_lag: list[float] = []
    ready_hash = ready_keyed = unknown = 0
    for sha, committed in commits:
        known = by_hash.get(hashes[sha])
        if known is None:
            unknown += 1
        else:
            lag = max(0.0, known - committed)
            hash_lag.append(lag)
            ready_hash += lag <= limit
        keyed = by_commit.get(sha)
        ready_keyed += keyed is not None and keyed - committed <= limit
    total = len(commits)
    span_h = (commits[-1][1] - commits[0][1]) / 3600 if total > 1 else 0.0
    distinct = len(set(hashes[sha] for sha, _ in commits))
    return {
        "commits": total,
        "window_hours": round(span_h, 2),
        "distinct_input_hashes": distinct,
        "distinct_hashes_per_hour": round(distinct / span_h, 2) if span_h else None,
        "minutes": minutes,
        "ready_by_input_hash": ready_hash,
        "ready_by_input_hash_share": round(ready_hash / total, 4) if total else None,
        "ready_commit_keyed": ready_keyed,
        "ready_commit_keyed_share": round(ready_keyed / total, 4) if total else None,
        "no_record": unknown,
        "median_lag_s": round(statistics.median(hash_lag), 1) if hash_lag else None,
        "max_lag_s": round(max(hash_lag), 1) if hash_lag else None,
    }


def simulate(
    commits: list[tuple[str, float]],
    hashes: dict[str, str],
    *,
    workers: int,
    build_seconds: float,
    gap_seconds: float = 0.0,
    minutes: float = 10.0,
) -> dict:
    """INFERRED: replay the real commit stream through a newest-wins builder pool.

    Each worker builds one new input hash at a time and takes the newest pending hash when it
    frees up (older pending hashes are superseded). `gap_seconds` is the minimum spacing between
    build starts (the old supervisor stamp guard was 45 minutes). Commits that share a hash with
    a ready or pending build cost nothing, which is why docs- and tools-only merges are free.
    """
    ready: dict[str, float] = {}
    pending: dict[str, float] = {}  # hash -> arrival time of its newest commit
    free_at = [0.0] * workers
    last_start = float("-inf")
    started = 0

    def run_until(now: float) -> None:
        nonlocal last_start, started
        while pending:
            worker = min(range(workers), key=lambda index: free_at[index])
            wanted = max(pending, key=lambda key: pending[key])
            start = max(free_at[worker], last_start + gap_seconds, pending[wanted])
            if start > now:
                return
            pending.clear()  # everything older is superseded by the newest tip
            ready[wanted] = start + build_seconds
            free_at[worker] = start + build_seconds
            last_start = start
            started += 1

    for sha, committed in commits:
        run_until(committed)
        wanted = hashes[sha]
        if wanted not in ready:
            pending[wanted] = committed
    run_until(float("inf"))
    limit = minutes * 60
    within = sum(
        1 for sha, committed in commits if ready.get(hashes[sha], 1e18) - committed <= limit
    )
    total = len(commits)
    return {
        "workers": workers,
        "build_seconds": build_seconds,
        "gap_seconds": gap_seconds,
        "builds_started": started,
        "ready_within_minutes": within,
        "ready_within_share": round(within / total, 4) if total else None,
    }


def loop_ticks(log: Path, hours: float, now: float) -> dict:
    """Tick-level view of the supervisor loop log: how often the tip had no ready host."""
    seconds: list[int] = []
    failed: list[list[str]] = []
    builds: list[int] = []
    pending: list[str] = []
    started = 0
    for line in log.read_text(errors="replace").splitlines():
        tick = TICK.match(line)
        if tick:
            seconds.append(int(tick[1]) * 3600 + int(tick[2]) * 60 + int(tick[3]))
            failed.append(pending)
            builds.append(started)
            pending, started = [], 0
            continue
        match = FAIL.search(line)
        if match:
            pending.append(match[1])
        started += STARTED in line
    # The log carries clock times only: unroll midnight wraps backwards from the last tick.
    absolute = [0.0] * len(seconds)
    if seconds:
        anchor = log.stat().st_mtime
        day = datetime.fromtimestamp(anchor).replace(hour=0, minute=0, second=0).timestamp()
        offset = 0
        for index in range(len(seconds) - 1, -1, -1):
            if index < len(seconds) - 1 and seconds[index] > seconds[index + 1] + 3600:
                offset -= 86400
            absolute[index] = day + offset + seconds[index]
    keep = [i for i, epoch in enumerate(absolute) if epoch >= now - hours * 3600]
    ticks = len(keep)
    failing = sum(1 for i in keep if failed[i])
    tips = {sha for i in keep for sha in failed[i]}
    return {
        "ticks": ticks,
        "ticks_without_ready_host": failing,
        "share_ticks_ready": round(1 - failing / ticks, 4) if ticks else None,
        "distinct_tips_without_host": len(tips),
        "builds_started": sum(builds[i] for i in keep),
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--worktree", type=Path, default=Path.cwd())
    parser.add_argument("--ref", default="origin/main")
    parser.add_argument("--hours", type=float, default=24.0)
    parser.add_argument("--minutes", type=float, default=10.0)
    parser.add_argument("--cache", type=Path, default=None)
    parser.add_argument("--loop-log", type=Path, default=None)
    parser.add_argument(
        "--simulate",
        action="append",
        default=[],
        metavar="WORKERS:BUILD_S[:GAP_S]",
        help="INFERRED newest-wins builder pool over the same commit stream (repeatable)",
    )
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args()
    root = args.worktree.resolve()
    cache = (args.cache or prebuilt.cache_root(root)).resolve()
    now = time.time()
    commits = main_commits(root, args.ref, args.hours, now)
    report: dict = {"ref": args.ref, "cache": str(cache), "generated_at": int(now)}
    if commits:
        hashes = input_hashes(root, [sha for sha, _ in commits])
        by_hash, by_commit = ready_times(cache)
        report["commits"] = measure(commits, hashes, by_hash, by_commit, args.minutes)
        report["simulations"] = []
        for spec in args.simulate:
            fields = [float(item) for item in spec.split(":")]
            report["simulations"].append(
                simulate(
                    commits,
                    hashes,
                    workers=int(fields[0]),
                    build_seconds=fields[1],
                    gap_seconds=fields[2] if len(fields) > 2 else 0.0,
                    minutes=args.minutes,
                )
            )
    ledger = cache / prebuilt.READY_LOG
    report["ledger_rows"] = len(ledger.read_text().splitlines()) if ledger.is_file() else 0
    if args.loop_log:
        report["supervisor_loop"] = loop_ticks(args.loop_log, args.hours, now)
    print(json.dumps(report, indent=None if args.json else 2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
