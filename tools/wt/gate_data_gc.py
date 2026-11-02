"""T3467 retention for bulk gate data: DMTCP gate snapshots and gate trace bulk.

`python3 -m tools.wt gate-data-gc [--apply] [--idle-h 2] [--keep-hosts 2] [--bulk-age-h 24]`

Roots: the current `tmp/wt-cache/gate-data/<kind>` (tools/gate_data.py) and the legacy
`tmp/owner/<kind>` that gates wrote before T3467. Owner inputs (discs, saves, recordings,
routes, Xbox files) are never under these roots and are never touched.

Owner decision 2026-10-21 (docs/decisions/replay-data-retention.md): raw replay and gate
output is a reproducible cache, not evidence. Receipts (verdicts and digests) are the evidence;
only what current gates resume from, what a live process uses and explicit pins with a TTL stay.
Tracked-doc citations no longer pin bulk data.

Snapshots (`adoption-gate-snapshots/<route>/<host sha256>/<config key>`). A snapshot is a DMTCP
image of one gate configuration; `snapshot.manifest.verify` refuses it for any other host binary,
and gates look it up under the sha256 of their current host. One is KEPT when:

* `in-use`: its `resume.lock` flock is held, or a live process has a cwd or open file under it,
  or names it in its argv or environment (a take or a restored DMTCP process);
* `keep-file`: an unexpired pin in `<root>/KEEP` (`route/host[/key] [until=YYYY-MM-DDTHH:MM]`;
  a line without `until=` expires `PIN_DEFAULT_TTL_H` after the KEEP file was last written);
* `current`: its host binary is live (a prebuilt host, or the manifest's host path with that
  sha256) and the host is the newest of the `--keep-hosts` live hosts of that (route, poll);
* `young`: a live-host or incomplete snapshot with an entry newer than `--idle-h` (a take in
  progress, a gate between resumes).

Everything else is deleted: `dead-host` (the host binary is gone, the image can never resume),
`superseded` (a live host older than the newest `--keep-hosts`), `incomplete` (no manifest:
an interrupted or failed take) and `retired` (`*.stale-*` leftovers of
`adoption_gate.retire_snapshot`). A delete takes the snapshot's `resume.lock`, rechecks the
process index, renames the directory to a hidden sibling and removes it, so a concurrent gate
sees a missing snapshot and takes a new one.

Gate trace bulk (`adoption-gate`, `adoption-gate-segments`, `traces`). Receipts, reports and
manifests stay. A bulk file (`trace.json` or `*.trace` of at least 1 MB) older than
`--bulk-age-h`, not open or named by a live process and not pinned is removed after its sha256,
size and mtime are recorded in `.gc-removed.json` next to it. A trace-store baseline whose
trace.json is gone is ignored by `adoption_gate.stored_baseline`, which then recaptures, and a
window copy is rewritten by the next gate (fails clean, re-runs).

Dry-run is the default. `--apply` appends to `tmp/wt-cache/gate-data/gc.log`.
"""

from __future__ import annotations

import argparse
import ast
import fcntl
import hashlib
import json
import os
import re
import shutil
import time
import uuid
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path

from tools import gate_data
from tools.wt import hygiene, safety

HOST = re.compile(r"^[0-9a-f]{64}$")
DEFAULT_IDLE_H = 2.0
DEFAULT_KEEP_HOSTS = 1
DEFAULT_BULK_AGE_H = 2.0
PIN_DEFAULT_TTL_H = 72.0
BULK_MIN_BYTES = 1_000_000
BULK_KINDS = (gate_data.ADOPTION_GATE, gate_data.SEGMENTS, gate_data.TRACES)
REMOVED = ".gc-removed.json"
SNAPSHOT_REASONS = (
    "in-use",
    "keep-file",
    "current",
    "young",
    "dead-host",
    "superseded",
    "incomplete",
    "retired",
)
DELETE_REASONS = frozenset({"dead-host", "superseded", "incomplete", "retired"})


@dataclass
class Snapshot:
    root: Path
    route: str
    host: str
    key: str
    path: Path
    manifest: dict | None
    poll: int | None
    created: float
    host_path: str | None
    newest: float = 0.0
    size: int = 0
    reason: str | None = None

    @property
    def rel(self) -> tuple[str, str, str]:
        return (self.route, self.host, self.key)


def _tree(path: Path) -> tuple[int, float]:
    """Allocated bytes and newest mtime of a tree (symlinks not followed)."""
    total = 0
    try:
        newest = path.lstat().st_mtime
    except OSError:
        return 0, 0.0
    stack = [path]
    while stack:
        directory = stack.pop()
        try:
            with os.scandir(directory) as entries:
                for entry in entries:
                    try:
                        info = entry.stat(follow_symlinks=False)
                    except OSError:
                        continue
                    newest = max(newest, info.st_mtime)
                    if entry.is_dir(follow_symlinks=False):
                        stack.append(Path(entry.path))
                    else:
                        total += getattr(info, "st_blocks", 0) * 512 or info.st_size
        except OSError:
            continue
    return total, newest


class ProcessIndex:
    """One /proc pass: cwd and open-file paths, argv and environment text of every process.

    An uninspectable process raises safety.Unsafe (incomplete scans fail closed, T2250).
    """

    def __init__(self, proc: Path = Path("/proc")) -> None:
        self.paths: list[str] = []
        self.texts: list[str] = []
        if not proc.is_dir():
            raise safety.Unsafe("/proc unavailable: cannot prove gate data is unused")
        for entry in proc.iterdir():
            if not entry.name.isdecimal():
                continue
            try:
                self.paths.append(os.readlink(entry / "cwd"))
            except OSError:
                pass
            self.paths.extend(hygiene.fd_targets(entry))
            for name in ("cmdline", "environ"):
                try:
                    raw = (entry / name).read_bytes()[: 1 << 20]
                except OSError:
                    continue
                self.texts.append(raw.replace(b"\0", b"\n").decode("utf-8", "replace"))

    def mentions(self, path: Path) -> bool:
        root = str(path)
        prefix = root + os.sep
        if any(item == root or item.startswith(prefix) for item in self.paths):
            return True
        return any(root in text for text in self.texts)

    def mentioned_under(self, base: Path, depth: int) -> set[tuple[str, ...]]:
        """Relative component tuples (up to `depth`) of every mention of a path under base."""
        prefix = str(base) + os.sep
        found: set[tuple[str, ...]] = set()
        for text in [*self.paths, *self.texts]:
            offset = text.find(prefix)
            while offset >= 0:
                tail = text[offset + len(prefix) :]
                tail = re.split(r"[\s\"'=:,;]", tail, maxsplit=1)[0]
                parts = tuple(part for part in tail.split(os.sep) if part)[:depth]
                if parts:
                    found.add(parts)
                offset = text.find(prefix, offset + len(prefix))
        return found


def _host_record(value: object) -> dict:
    if isinstance(value, str):
        try:
            value = ast.literal_eval(value)
        except (ValueError, SyntaxError):
            return {}
    return value if isinstance(value, dict) else {}


def _created(manifest: dict | None, fallback: float) -> float:
    text = manifest.get("created") if manifest else None
    if isinstance(text, str):
        try:
            return datetime.strptime(text, "%Y-%m-%dT%H:%M:%S%z").timestamp()
        except ValueError:
            pass
    return fallback


def live_hosts(main: Path, extra_paths: set[str] | None = None) -> set[str]:
    """sha256 of every host binary that exists now (prebuilt manifests, then named paths)."""
    found: set[str] = set()
    prebuilt = Path(os.environ.get("TSFP_PREBUILT_ROOT", str(main / "tmp/wt-cache/prebuilt")))
    if prebuilt.is_dir():
        for directory in prebuilt.iterdir():
            manifest = hygiene._read_small_json(directory / "manifest.json")
            files = manifest.get("files") if manifest else None
            if isinstance(files, dict) and (directory / "tsfp-bevy").is_file():
                value = files.get("tsfp-bevy")
                if isinstance(value, str) and HOST.fullmatch(value):
                    found.add(value)
    for name in sorted(extra_paths or ()):
        path = Path(name)
        try:
            if path.is_file() and not path.is_symlink() and path.stat().st_size < 4 << 30:
                found.add(_sha256(path))
        except OSError:
            continue
    return found


def _sha256(path: Path) -> str:
    value = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1 << 20), b""):
            value.update(block)
    return value.hexdigest()


def snapshot_inventory(main: Path) -> list[Snapshot]:
    found: list[Snapshot] = []
    for root in gate_data.roots(main, gate_data.SNAPSHOTS):
        if root.is_symlink() or not root.is_dir():
            continue
        for route in sorted(root.iterdir()):
            if route.is_symlink() or not route.is_dir() or route.name.startswith("."):
                continue
            for host in sorted(route.iterdir()):
                if host.is_symlink() or not host.is_dir() or not HOST.fullmatch(host.name):
                    continue
                for snap in sorted(host.iterdir()):
                    if snap.is_symlink() or not snap.is_dir():
                        continue
                    manifest = hygiene._read_small_json(snap / "manifest.json", limit=16 << 20)
                    record = _host_record(manifest.get("host")) if manifest else {}
                    poll = manifest.get("poll") if manifest else None
                    found.append(
                        Snapshot(
                            root=root,
                            route=route.name,
                            host=host.name,
                            key=snap.name,
                            path=snap,
                            manifest=manifest,
                            poll=poll if isinstance(poll, int) else None,
                            created=_created(manifest, snap.stat().st_mtime),
                            host_path=record.get("path")
                            if isinstance(record.get("path"), str)
                            else None,
                        )
                    )
    return found


def pins(path: Path, now: float, depth: int) -> set[tuple[str, ...]]:
    """Unexpired pins of a KEEP file: `a/b[/c] [until=YYYY-MM-DD[THH:MM]]`, `#` comments.

    A line without `until=` expires PIN_DEFAULT_TTL_H after the file was last written, so a pin
    is always temporary (owner decision 2026-10-21)."""
    entries: set[tuple[str, ...]] = set()
    try:
        text = path.read_text(errors="replace")
        written = path.stat().st_mtime
    except OSError:
        return entries
    for line in text.splitlines():
        fields = line.split("#", 1)[0].split()
        if not fields:
            continue
        expiry = written + PIN_DEFAULT_TTL_H * 3600
        for field in fields[1:]:
            if field.startswith("until="):
                value = field.removeprefix("until=")
                for layout in ("%Y-%m-%dT%H:%M", "%Y-%m-%d"):
                    try:
                        expiry = datetime.strptime(value, layout).timestamp()
                        if layout == "%Y-%m-%d":
                            expiry += 86400
                        break
                    except ValueError:
                        continue
        if expiry > now:
            entries.add(tuple(part for part in fields[0].strip("/").split("/") if part)[:depth])
    return entries


def classify_snapshots(
    main: Path,
    snapshots: list[Snapshot],
    *,
    idle_h: float,
    keep_hosts: int,
    now: float,
    processes: ProcessIndex,
    live: set[str],
) -> None:
    cutoff = now - idle_h * 3600
    mentioned: dict[Path, set[tuple[str, ...]]] = {}
    keep: dict[Path, set[tuple[str, ...]]] = {}
    for snap in snapshots:
        if snap.root not in mentioned:
            mentioned[snap.root] = processes.mentioned_under(snap.root, 3)
            keep[snap.root] = pins(snap.root / "KEEP", now, 3)
    # Newest live hosts per (root, route, poll), by the newest manifest creation time.
    host_age: dict[tuple[Path, str, int | None], dict[str, float]] = {}
    for snap in snapshots:
        if snap.manifest and snap.host in live:
            ages = host_age.setdefault((snap.root, snap.route, snap.poll), {})
            ages[snap.host] = max(ages.get(snap.host, 0.0), snap.created)
    current: set[tuple[Path, str, int | None, str]] = set()
    for group, ages in host_age.items():
        for host in sorted(ages, key=lambda item: -ages[item])[:keep_hosts]:
            current.add((*group, host))
    for snap in snapshots:
        snap.size, snap.newest = _tree(snap.path)
        rel = snap.rel
        lock = snap.path / "resume.lock"
        if (
            any(rel[: len(item)] == item for item in mentioned[snap.root])
            or (lock.exists() and not hygiene._try_existing_lock(lock))
            or processes.mentions(snap.path)
        ):
            snap.reason = "in-use"
        elif any(rel[: len(item)] == item for item in keep[snap.root]):
            snap.reason = "keep-file"
        elif ".stale-" in snap.key or snap.key.startswith(".gc-"):
            snap.reason = "retired"
        elif snap.manifest is not None and snap.host not in live:
            snap.reason = "dead-host"
        elif (snap.root, snap.route, snap.poll, snap.host) in current:
            snap.reason = "current"
        elif snap.newest >= cutoff:
            snap.reason = "young"
        elif snap.manifest is None:
            snap.reason = "incomplete"
        else:
            snap.reason = "superseded"


def _log(main: Path, line: str) -> None:
    log = gate_data.path(main, gate_data.SNAPSHOTS).parent / "gc.log"
    log.parent.mkdir(parents=True, exist_ok=True)
    with log.open("a") as stream:
        stream.write(f"{time.strftime('%Y-%m-%dT%H:%M:%S')} {line}\n")


def open_by_other_process(path: Path, proc: Path = Path("/proc")) -> bool:
    """hygiene.open_by_process without this process (fails closed through hygiene.fd_targets)."""
    root = str(path.resolve())
    own = str(os.getpid())
    for entry in proc.iterdir():
        if not entry.name.isdecimal() or entry.name == own:
            continue
        try:
            targets = [os.readlink(entry / "cwd")]
        except OSError:
            targets = []
        for target in targets + hygiene.fd_targets(entry):
            if target == root or target.startswith(root + os.sep):
                return True
    return False


def _delete_snapshot(snap: Snapshot) -> bool:
    """Rename under the snapshot's resume.lock, recheck use, then remove. False when busy."""
    if snap.path.is_symlink() or not snap.path.is_dir():
        return False
    if snap.path.stat().st_dev != snap.root.stat().st_dev:
        return False
    lock = snap.path / "resume.lock"
    if lock.is_symlink():
        return False
    fd = os.open(lock, os.O_CREAT | os.O_RDWR, 0o600)
    try:
        try:
            fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            return False
        try:
            # The full argv/environment index was taken just before; here the lock plus a fresh
            # open-file and cwd scan is enough (a resume or take holds resume.lock or files).
            # This process holds resume.lock inside the snapshot, so it is excluded.
            if open_by_other_process(snap.path):
                return False
        except safety.Unsafe:
            return False
        hidden = snap.path.with_name(f".gc-{snap.key}-{os.getpid()}-{uuid.uuid4().hex[:8]}")
        snap.path.rename(hidden)
    finally:
        os.close(fd)
    shutil.rmtree(hidden)
    return True


def _prune_empty(directory: Path, stop: Path) -> None:
    while directory != stop and directory.is_dir() and not directory.is_symlink():
        try:
            directory.rmdir()
        except OSError:
            return
        directory = directory.parent


def snapshot_gc(
    main: Path,
    *,
    apply: bool = False,
    idle_h: float = DEFAULT_IDLE_H,
    keep_hosts: int = DEFAULT_KEEP_HOSTS,
    now: float | None = None,
    processes: ProcessIndex | None = None,
    live: set[str] | None = None,
    quiet: bool = False,
) -> dict:
    if idle_h < 0 or keep_hosts < 1:
        raise ValueError("idle hours must be nonnegative and keep-hosts positive")
    now = time.time() if now is None else now
    snapshots = snapshot_inventory(main)
    processes = ProcessIndex() if processes is None else processes
    if live is None:
        live = live_hosts(main, {snap.host_path for snap in snapshots if snap.host_path})
    classify_snapshots(
        main,
        snapshots,
        idle_h=idle_h,
        keep_hosts=keep_hosts,
        now=now,
        processes=processes,
        live=live,
    )
    stats = {reason: [0, 0] for reason in (*SNAPSHOT_REASONS, "deleted", "busy")}
    deleted: list[str] = []
    for snap in sorted(snapshots, key=lambda item: item.newest):
        assert snap.reason is not None
        stats[snap.reason][0] += 1
        stats[snap.reason][1] += snap.size
        if snap.reason not in DELETE_REASONS:
            continue
        label = "/".join(snap.rel)
        if apply:
            if not _delete_snapshot(snap):
                stats["busy"][0] += 1
                stats["busy"][1] += snap.size
                continue
            _log(main, f"snapshot deleted {snap.reason} {snap.root}/{label} bytes={snap.size}")
            _prune_empty(snap.path.parent, snap.root)
        stats["deleted"][0] += 1
        stats["deleted"][1] += snap.size
        deleted.append(label)
    if not quiet:
        mode = "applied" if apply else "dry-run"
        print(
            f"snapshot gc {mode}: snapshots={len(snapshots)} live_hosts={len(live)} "
            f"idle_h={idle_h:g} keep_hosts={keep_hosts}"
        )
        for reason, (count, size) in stats.items():
            verb = "would delete" if reason == "deleted" and not apply else reason
            print(f"  {verb:<14} {count:>6} snapshots {size / 1e9:>9.2f} GB")
    if apply:
        _log(main, f"snapshot summary deleted={len(deleted)} bytes={stats['deleted'][1]}")
    return {"stats": stats, "deleted": deleted}


def _is_bulk(name: str) -> bool:
    return name == "trace.json" or name.endswith(".trace")


def _record_removed(directory: Path, name: str, entry: dict) -> None:
    sidecar = directory / REMOVED
    data = hygiene._read_small_json(sidecar, limit=16 << 20) or {}
    data[name] = entry
    temporary = sidecar.with_name(f"{REMOVED}.{os.getpid()}.tmp")
    temporary.write_text(json.dumps(data, indent=1, sort_keys=True) + "\n")
    temporary.replace(sidecar)


def bulk_gc(
    main: Path,
    *,
    apply: bool = False,
    age_h: float = DEFAULT_BULK_AGE_H,
    now: float | None = None,
    processes: ProcessIndex | None = None,
    quiet: bool = False,
) -> dict:
    """Drop idle trace bulk under the gate data roots, keeping a digest record."""
    if age_h < 0:
        raise ValueError("bulk age must be nonnegative")
    now = time.time() if now is None else now
    cutoff = now - age_h * 3600
    processes = ProcessIndex() if processes is None else processes
    stats = {"kept-young": [0, 0], "kept-pinned": [0, 0], "in-use": [0, 0], "removed": [0, 0]}
    removed: list[str] = []
    for kind in BULK_KINDS:
        for root in gate_data.roots(main, kind):
            if root.is_symlink() or not root.is_dir():
                continue
            device = root.stat().st_dev
            pinned = pins(root / "KEEP", now, 8)
            for directory, dirnames, filenames in os.walk(root, followlinks=False):
                dirnames[:] = [
                    item
                    for item in dirnames
                    if not (Path(directory) / item).is_symlink() and not item.startswith(".gc-")
                ]
                for name in filenames:
                    if not _is_bulk(name):
                        continue
                    path = Path(directory) / name
                    try:
                        info = path.lstat()
                    except OSError:
                        continue
                    size = getattr(info, "st_blocks", 0) * 512 or info.st_size
                    if path.is_symlink() or info.st_size < BULK_MIN_BYTES:
                        continue
                    rel = path.relative_to(root).parts
                    if info.st_mtime >= cutoff:
                        bucket = "kept-young"
                    elif any(rel[: len(item)] == item for item in pinned):
                        bucket = "kept-pinned"
                    elif processes.mentions(path) or info.st_dev != device:
                        bucket = "in-use"
                    else:
                        bucket = "removed"
                    stats[bucket][0] += 1
                    stats[bucket][1] += size
                    if bucket != "removed":
                        continue
                    if apply:
                        entry = {
                            "sha256": _sha256(path),
                            "bytes": info.st_size,
                            "mtime": int(info.st_mtime),
                            "removed_at": int(time.time()),
                            "reason": f"T3467 idle gate trace bulk older than {age_h:g} h",
                        }
                        _record_removed(path.parent, name, entry)
                        path.unlink()
                        _log(main, f"bulk removed {path} bytes={size} sha256={entry['sha256']}")
                    removed.append(str(path))
    if not quiet:
        mode = "applied" if apply else "dry-run"
        print(f"gate bulk gc {mode}: age_h={age_h:g}")
        for bucket, (count, size) in stats.items():
            verb = "would remove" if bucket == "removed" and not apply else bucket
            print(f"  {verb:<14} {count:>6} files {size / 1e9:>9.2f} GB")
    if apply:
        _log(main, f"bulk summary removed={len(removed)} bytes={stats['removed'][1]}")
    return {"stats": stats, "removed": removed}


def run(
    main: Path,
    *,
    apply: bool = False,
    idle_h: float = DEFAULT_IDLE_H,
    keep_hosts: int = DEFAULT_KEEP_HOSTS,
    bulk_age_h: float = DEFAULT_BULK_AGE_H,
) -> dict:
    lock_path = gate_data.path(main, gate_data.SNAPSHOTS).parent / ".gc.lock"
    lock_path.parent.mkdir(parents=True, exist_ok=True)
    with lock_path.open("a") as lock:
        try:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            print("gate-data gc: another pass holds the lock; skipped")
            return {}
        return _run_locked(
            main, apply=apply, idle_h=idle_h, keep_hosts=keep_hosts, bulk_age_h=bulk_age_h
        )


def _run_locked(
    main: Path, *, apply: bool, idle_h: float, keep_hosts: int, bulk_age_h: float
) -> dict:
    roots = [
        root
        for kind in (gate_data.SNAPSHOTS, *BULK_KINDS)
        for root in gate_data.roots(main, kind)
        if root.is_dir() and not root.is_symlink()
    ]
    if not roots:
        # Nothing to retain (most remote workers): no process scan, so a host whose root-owned
        # processes cannot be inspected without sudo does not fail the command.
        print("gate-data gc: no gate data roots; nothing to do")
        return {"snapshots": {}, "bulk": {}}
    processes = ProcessIndex()
    snapshots = snapshot_inventory(main)
    live = live_hosts(main, {snap.host_path for snap in snapshots if snap.host_path})
    return {
        "snapshots": snapshot_gc(
            main,
            apply=apply,
            idle_h=idle_h,
            keep_hosts=keep_hosts,
            processes=processes,
            live=live,
        ),
        "bulk": bulk_gc(main, apply=apply, age_h=bulk_age_h, processes=processes),
    }


def add_arguments(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--apply", action="store_true", help="delete (default: dry-run)")
    parser.add_argument("--idle-h", type=float, default=DEFAULT_IDLE_H)
    parser.add_argument("--keep-hosts", type=int, default=DEFAULT_KEEP_HOSTS)
    parser.add_argument("--bulk-age-h", type=float, default=DEFAULT_BULK_AGE_H)


def run_cli(args: argparse.Namespace, main: Path) -> int:
    try:
        run(
            main,
            apply=args.apply,
            idle_h=args.idle_h,
            keep_hosts=args.keep_hosts,
            bulk_age_h=args.bulk_age_h,
        )
    except (OSError, ValueError, safety.Unsafe) as exc:
        print(f"gate-data-gc failed: {exc}")
        return 1
    return 0
