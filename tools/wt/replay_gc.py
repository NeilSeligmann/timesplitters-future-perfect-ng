"""Replay-run retention GC (T3431, policy replaced by the owner decision of 2026-10-21, T3467).

`python3 -m tools.wt replay gc [--apply] [--max-age-h 2] [--cap-gb 0] [--triage-h 24]`

Raw replay output under `tmp/wt-cache/replays/runs/<key>` is a reproducible cache, not evidence
(docs/decisions/replay-data-retention.md). The receipt is the evidence, and a receipt-cache hit
whose run is gone re-runs the capture (`replay.cached_run_problem`). A run directory is KEPT when:

* `in-use`: its `replay-<key>.lock` flock is held (running submit), a process has a file or
  cwd inside it, a live process argv or hostlock `queue.json` names it, or a `*.pid` guard file
  inside it names a live process.
* `keep-file`: an unexpired pin in `tmp/wt-cache/replays/KEEP` (`<key> [until=YYYY-MM-DDTHH:MM]`,
  `#` comments; a line without `until=` expires 72 h after the file was written).
* `young`: newer than `--max-age-h` (max mtime of the dir, its receipt and its recipe). This
  covers a gate between its baseline and candidate captures; immediate deletion after a verdict
  would race other gates sharing the same cached baseline.
* `triage`: within `--triage-h`, the capture failed (receipt exit code not 0) or a gate receipt
  or report with a verdict other than EQUAL/PASS names the run (DIVERGED or INVALID raw data is
  kept 24 h for triage, then removed).
* `newest`: with `newest_per_group`, the newest runs per (route, switches) younger than
  `--triage-h` (the T3417 `hygiene-cache --replay-keep` rule).

Tracked-doc citations, trace-store and snapshot references and other kept runs no longer pin a
run (owner override). Everything else is deleted oldest first while the run total exceeds
`--cap-gb` (default 0: every eligible run). Before a delete the receipt is archived to
`tmp/wt-cache/receipts/replays/<key>.json` with the sha256 and size of `trace.json` and
`call-args.trace`; then the run dir, receipt, recipe and lock go. `--apply` logs to
`tmp/wt-cache/replays/gc.log`. Never touches tmp/owner.
"""

from __future__ import annotations

import argparse
import fcntl
import hashlib
import json
import os
import re
import shutil
import time
from collections.abc import Iterable
from dataclasses import dataclass, field
from pathlib import Path

from tools import gate_data
from tools.wt import hygiene, safety

KEY = re.compile(r"^[0-9a-f]{64}$")
KEY_TOKEN = re.compile(r"(?<![0-9a-f])[0-9a-f]{64}(?![0-9a-f])")
GATE_RECEIPT_ROOTS = (
    "tmp/owner/gate-receipts",
    "tmp/owner/gate-reports",
    "tmp/owner/adoption-gate",
    "tmp/owner/verify-all",
    f"{gate_data.GATE_DATA}/{gate_data.ADOPTION_GATE}",
)
PASSING_VERDICTS = frozenset({"EQUAL", "PASS", "ADOPT", "ADOPTED", "OK"})
TEXT_SUFFIXES = (".json", ".md", ".txt")
TEXT_LIMIT = 4 * 1024 * 1024
DEFAULT_MAX_AGE_H = 2.0
DEFAULT_CAP_GB = 0.0
DEFAULT_TRIAGE_H = 24.0
HYGIENE_FREE_GB = 150.0
DIGEST_FILES = ("trace.json", "call-args.trace")
REASONS = ("in-use", "keep-file", "young", "triage", "newest")


@dataclass
class Run:
    key: str
    path: Path
    mtime: float
    size: int
    reason: str | None = None
    extra: list[Path] = field(default_factory=list)


def _tokens(text: str, keys: set[str]) -> set[str]:
    return {token for token in KEY_TOKEN.findall(text) if token in keys}


def _read_text(path: Path) -> str:
    try:
        if path.is_symlink() or path.stat().st_size > TEXT_LIMIT:
            return ""
        return path.read_text(errors="replace")
    except OSError:
        return ""


def cache_root(main: Path) -> Path:
    return Path(os.environ.get("TSFP_REPLAY_CACHE", str(main / "tmp/wt-cache/replays")))


def inventory(cache: Path) -> dict[str, Run]:
    runs_dir = cache / "runs"
    found: dict[str, Run] = {}
    if not runs_dir.is_dir():
        return found
    for entry in os.scandir(runs_dir):
        if not (KEY.fullmatch(entry.name) and entry.is_dir(follow_symlinks=False)):
            continue
        path = Path(entry.path)
        extra = [
            item
            for item in (cache / f"{entry.name}.json", runs_dir / f"{entry.name}.recipe.json")
            if item.is_file() and not item.is_symlink()
        ]
        try:
            mtimes = [path.stat().st_mtime, *(item.stat().st_mtime for item in extra)]
        except OSError:
            continue
        found[entry.name] = Run(entry.name, path, max(mtimes), 0, extra=extra)
    return found


def keep_file_keys(cache: Path, now: float | None = None) -> set[str]:
    from tools.wt import gate_data_gc

    now = time.time() if now is None else now
    return {item[0] for item in gate_data_gc.pins(cache / "KEEP", now, 1) if KEY.fullmatch(item[0])}


def triage_keys(main: Path, cache: Path, runs: dict[str, Run], cutoff: float) -> set[str]:
    """Runs of failed captures and runs named by recent non-passing gate receipts."""
    keys = set(runs)
    found: set[str] = set()
    for key in keys:
        record = hygiene._read_small_json(cache / f"{key}.json")
        if record and record.get("exit_code") not in (0, None) and runs[key].mtime >= cutoff:
            found.add(key)
    for name in GATE_RECEIPT_ROOTS:
        root = main / name
        if not root.is_dir():
            continue
        for directory, dirnames, filenames in os.walk(root, followlinks=False):
            dirnames[:] = [item for item in dirnames if not (Path(directory) / item).is_symlink()]
            for filename in filenames:
                if not filename.endswith(".json"):
                    continue
                path = Path(directory) / filename
                try:
                    if path.stat().st_mtime < cutoff:
                        continue
                except OSError:
                    continue
                data = hygiene._read_small_json(path, limit=TEXT_LIMIT)
                verdict = data.get("verdict") if data else None
                if isinstance(verdict, str) and verdict.upper() not in PASSING_VERDICTS:
                    found |= _tokens(_read_text(path), keys)
    return found


def _lock_held(lock: Path) -> bool:
    return not hygiene._try_existing_lock(lock)


def _pid_alive(pid: int) -> bool:
    try:
        os.kill(pid, 0)
    except ProcessLookupError:
        return False
    except PermissionError:
        return True
    return True


def in_use_keys(cache: Path, runs: dict[str, Run]) -> set[str]:
    keys = set(runs)
    found = hygiene._live_replay_keys(cache / "runs", keys) if keys else set()
    for queue in cache.glob("hostlock-*/queue.json"):
        found |= _tokens(_read_text(queue), keys)
    for key, run in runs.items():
        if key in found:
            continue
        if _lock_held(cache / f"replay-{key}.lock"):
            found.add(key)
            continue
        for pid_file in run.path.glob("*.pid"):
            text = _read_text(pid_file).strip()
            if text.isdecimal() and _pid_alive(int(text)):
                found.add(key)
                break
    return found


def newest_keys(cache: Path, runs: dict[str, Run], keep: int, cutoff: float) -> set[str]:
    """Newest `keep` runs per (route, switches) group among runs newer than cutoff."""
    if keep < 1:
        return set()
    grouped: dict[tuple[str, tuple[str, ...]], list[tuple[float, str]]] = {}
    for key, run in runs.items():
        if run.mtime < cutoff:
            continue
        record = hygiene._read_small_json(cache / f"{key}.json") or {}
        route, switches = record.get("route"), record.get("switches")
        if isinstance(route, str) and isinstance(switches, list):
            group = (route, tuple(sorted(str(item) for item in switches)))
            grouped.setdefault(group, []).append((run.mtime, key))
    found: set[str] = set()
    for values in grouped.values():
        found.update(key for _mtime, key in sorted(values, reverse=True)[:keep])
    return found


def classify(
    main: Path,
    cache: Path,
    runs: dict[str, Run],
    max_age_h: float,
    now: float,
    *,
    triage_h: float = DEFAULT_TRIAGE_H,
    newest_per_group: int = 0,
    tracked: Iterable[str] | None = None,
) -> None:
    """Assign the first keep reason to every kept run. `tracked` is accepted and ignored: tracked
    docs no longer pin raw runs (owner decision 2026-10-21)."""
    del tracked
    cutoff = now - max_age_h * 3600
    triage_cutoff = now - triage_h * 3600
    for reason, members in (
        ("in-use", in_use_keys(cache, runs)),
        ("keep-file", keep_file_keys(cache, now) & set(runs)),
        ("young", {key for key, run in runs.items() if run.mtime >= cutoff}),
        ("triage", triage_keys(main, cache, runs, triage_cutoff)),
        ("newest", newest_keys(cache, runs, newest_per_group, triage_cutoff)),
    ):
        for key in members:
            if runs[key].reason is None:
                runs[key].reason = reason


def _log(cache: Path, line: str) -> None:
    with (cache / "gc.log").open("a") as stream:
        stream.write(f"{time.strftime('%Y-%m-%dT%H:%M:%S')} {line}\n")


def _digest(path: Path) -> dict | None:
    try:
        if path.is_symlink() or not path.is_file():
            return None
        value = hashlib.sha256()
        with path.open("rb") as stream:
            for block in iter(lambda: stream.read(1 << 20), b""):
                value.update(block)
        return {"sha256": value.hexdigest(), "bytes": path.stat().st_size}
    except OSError:
        return None


def archive_receipt(main: Path, cache: Path, run: Run, *, digest: bool = True) -> Path | None:
    """Copy the run's receipt to the compact receipt store with digests of its trace files."""
    record = cache / f"{run.key}.json"
    data = hygiene._read_small_json(record)
    if data is None:
        return None
    files = {name: _digest(run.path / name) for name in DIGEST_FILES} if digest else {}
    entry = {
        **data,
        "raw_removed": {
            "at": int(time.time()),
            "by": "T3467 replay gc",
            "files": {name: value for name, value in files.items() if value},
        },
    }
    store = gate_data.receipt_store(main, "replays")
    store.mkdir(parents=True, exist_ok=True)
    target = store / f"{run.key}.json"
    temporary = target.with_name(f"{target.name}.{os.getpid()}.tmp")
    temporary.write_text(json.dumps(entry, indent=1, sort_keys=True) + "\n")
    temporary.replace(target)
    return target


def _delete(main: Path, cache: Path, run: Run, *, digest: bool = True) -> bool:
    """Archive the receipt, then delete one run under its replay lock; False when it turned busy."""
    device = cache.stat().st_dev
    if run.path.is_symlink() or run.path.stat().st_dev != device:
        return False
    lock_path = cache / f"replay-{run.key}.lock"
    if lock_path.is_symlink():
        return False
    fd = os.open(lock_path, os.O_CREAT | os.O_RDWR, 0o600)
    try:
        try:
            fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            return False
        try:
            if hygiene.open_by_process(run.path):
                return False
        except safety.Unsafe:
            return False
        archive_receipt(main, cache, run, digest=digest)
        # Receipt first: a cache hit can never see a receipt whose run is half deleted.
        for item in run.extra:
            item.unlink(missing_ok=True)
        shutil.rmtree(run.path)
        lock_path.unlink(missing_ok=True)
        return True
    finally:
        os.close(fd)


def gc(
    main: Path,
    *,
    apply: bool = False,
    max_age_h: float = DEFAULT_MAX_AGE_H,
    cap_gb: float = DEFAULT_CAP_GB,
    triage_h: float = DEFAULT_TRIAGE_H,
    newest_per_group: int = 0,
    now: float | None = None,
    tracked: Iterable[str] | None = None,
    digest: bool = True,
    quiet: bool = False,
) -> dict:
    if max_age_h < 0 or cap_gb < 0 or triage_h < 0 or newest_per_group < 0:
        raise ValueError("max age, triage hours, cap and keep counts must be nonnegative")
    cache = cache_root(main)
    for path in (cache, cache / "runs"):
        if path.is_symlink():
            raise safety.Unsafe(f"replay cache path is a symlink: {path}")
    now = time.time() if now is None else now
    runs = inventory(cache)
    for run in runs.values():
        run.size = hygiene._walk_allocated(run.path) + sum(
            item.stat().st_size for item in run.extra if item.exists()
        )
    classify(
        main,
        cache,
        runs,
        max_age_h,
        now,
        triage_h=triage_h,
        newest_per_group=newest_per_group,
        tracked=tracked,
    )
    total = sum(run.size for run in runs.values())
    cap = int(cap_gb * 1e9)
    stats = {reason: [0, 0] for reason in (*REASONS, "deleted", "candidate-kept-under-cap")}
    for run in runs.values():
        if run.reason:
            stats[run.reason][0] += 1
            stats[run.reason][1] += run.size
    remaining = total
    deleted: list[str] = []
    for run in sorted((run for run in runs.values() if not run.reason), key=lambda r: r.mtime):
        if remaining <= cap:
            stats["candidate-kept-under-cap"][0] += 1
            stats["candidate-kept-under-cap"][1] += run.size
            continue
        if apply:
            if not _delete(main, cache, run, digest=digest):
                stats["in-use"][0] += 1
                stats["in-use"][1] += run.size
                continue
            _log(cache, f"deleted {run.key} bytes={run.size} mtime={int(run.mtime)}")
        remaining -= run.size
        deleted.append(run.key)
        stats["deleted"][0] += 1
        stats["deleted"][1] += run.size
    mode = "applied" if apply else "dry-run"
    if apply and runs:
        _log(
            cache,
            f"summary runs={len(deleted)} freed_gb={stats['deleted'][1] / 1e9:.2f} "
            f"max_age_h={max_age_h:g} cap_gb={cap_gb:g}",
        )
    if not quiet:
        print(
            f"replay gc {mode}: runs={len(runs)} total_gb={total / 1e9:.2f} "
            f"after_gb={remaining / 1e9:.2f} cap_gb={cap_gb:g} max_age_h={max_age_h:g}"
        )
        for reason, (count, size) in stats.items():
            verb = "would delete" if reason == "deleted" and not apply else reason
            print(f"  {verb:<26} {count:>6} runs {size / 1e9:>9.2f} GB")
    return {"stats": stats, "deleted": deleted, "total": total, "after": remaining}


def remote_staging_root() -> Path:
    """Worker-side staging of `remote_replay.run_capture` (~/.cache/tsfp-replay-remote/<run>)."""
    default = Path.home() / ".cache" / "tsfp-replay-remote"
    return Path(os.environ.get("TSFP_REPLAY_REMOTE_STAGING", str(default)))


def remote_staging_gc(
    *,
    apply: bool = False,
    triage_h: float = DEFAULT_TRIAGE_H,
    now: float | None = None,
    root: Path | None = None,
    quiet: bool = False,
) -> dict:
    """Remove worker staging dirs idle for `triage_h`: a successful remote capture deletes its
    own; a failed one is kept for triage (remote_replay) and was never removed before T3467."""
    from tools.wt import gate_data_gc

    root = remote_staging_root() if root is None else root
    now = time.time() if now is None else now
    stats = {"kept-young": [0, 0], "in-use": [0, 0], "removed": [0, 0]}
    if root.is_symlink() or not root.is_dir():
        return stats
    for entry in sorted(root.iterdir()):
        if entry.is_symlink() or not entry.is_dir():
            continue
        size, newest = gate_data_gc._tree(entry)
        if newest >= now - triage_h * 3600:
            bucket = "kept-young"
        else:
            try:
                busy = hygiene.open_by_process(entry)
            except safety.Unsafe:
                busy = True
            bucket = "in-use" if busy else "removed"
        stats[bucket][0] += 1
        stats[bucket][1] += size
        if bucket == "removed" and apply:
            shutil.rmtree(entry)
    if not quiet:
        mode = "applied" if apply else "dry-run"
        print(f"remote staging gc {mode}: {root} triage_h={triage_h:g}")
        for bucket, (count, size) in stats.items():
            verb = "would remove" if bucket == "removed" and not apply else bucket
            print(f"  {verb:<14} {count:>6} dirs {size / 1e9:>9.2f} GB")
    return stats


def hygiene_pass(main: Path) -> dict | None:
    """Hourly hook: remove every idle unpinned run (owner decision 2026-10-21)."""
    cap = float(os.environ.get("TSFP_REPLAY_GC_CAP_GB", DEFAULT_CAP_GB))
    cache = cache_root(main)
    if not (cache / "runs").is_dir():
        return None
    return gc(main, apply=True, cap_gb=cap)


def add_parser(sub: argparse._SubParsersAction) -> None:
    parser = sub.add_parser(
        "gc", help="retention GC for replay runs (T3431, T3467), dry-run default"
    )
    parser.add_argument("--apply", action="store_true", help="delete (default: dry-run)")
    parser.add_argument("--max-age-h", type=float, default=DEFAULT_MAX_AGE_H)
    parser.add_argument("--cap-gb", type=float, default=DEFAULT_CAP_GB)
    parser.add_argument("--triage-h", type=float, default=DEFAULT_TRIAGE_H)
    parser.add_argument(
        "--no-digest", action="store_true", help="archive receipts without trace digests"
    )


def run_cli(args: argparse.Namespace) -> int:
    from tools.wt import core

    main = core.main_root(core.worktree_root(Path.cwd()))
    try:
        gc(
            main,
            apply=args.apply,
            max_age_h=args.max_age_h,
            cap_gb=args.cap_gb,
            triage_h=args.triage_h,
            digest=not args.no_digest,
        )
    except (OSError, ValueError, safety.Unsafe) as exc:
        print(f"replay gc failed: {exc}")
        return 1
    return 0
