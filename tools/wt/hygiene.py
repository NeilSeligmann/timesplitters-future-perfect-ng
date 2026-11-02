"""T2419 disk hygiene: rebuildable-artifact stripping and ephemeral tmp retention.

Policy: `tmp/` is ephemeral. Anything worth keeping is copied to a tracked or owner
location (`tmp/owner/`) first. Every function here is dry-run unless `apply` is true,
never follows symlinks, and skips anything claimed, in use, or referenced by tracked docs.
"""

from __future__ import annotations

import fcntl
import hashlib
import json
import math
import os
import re
import shutil
import subprocess
import sys
import time
from collections.abc import Callable, Iterator
from pathlib import Path
from typing import IO

from tools import host_registry
from tools.wt import safety

REBUILDABLE = frozenset(
    {
        "target",
        "build",
        "node_modules",
        ".venv",
        "__pycache__",
        ".pytest_cache",
        ".ruff_cache",
        ".mypy_cache",
        "sccache",
        "ccache",
        "cargo-target",
        "prebuilt-build",
    }
)
# T3947: rebuildable dirs git lists only through their ignored parent (`tmp/`): each idle worktree
# kept its own 2-3 GB CMake tree (ten on the owner PC, 24 GB).
# T3947: every per-worktree cache/build dir tools write (tests/test_cache_paths_guard.py checks
# the list against the tree); idle worktrees lose them, the shared stores under tmp/wt-cache stay.
REBUILDABLE_PATHS = (
    "tmp/prebuilt-build",
    "tmp/bevy-target",
    "tmp/bevy-build",
    "tmp/hybrid-build",
    "tmp/native-test-build",
    "tmp/game-target",
    "tmp/cc-cache",
    "tmp/ui-wgpu-target",
    "tmp/harness-build",
)
# Top-level tmp entries the ephemeral sweep never touches.
TMP_PROTECTED = frozenset(
    {
        "owner",
        "wt",
        "wt-cache",
        "wt-trash",
        "play",
        "live-census",  # T3824: durable live hit census store (tools.live_census.store)
        "xemu",
        "ui-tree",
        "merge.lock",
        "lock-watch.jsonl",  # T3500: lock watchdog reports
        "Xbox-Emulator-Files",
        "Xbox-Emulator-Files.zip",
        "oxm discs",
        "ps2 demodiscs",
        "ps2 demo disc 53",
        "xbox-sdks",
        "xbox-sdk-ref",
        "routes",
        "visual-studio",
        "spikes-cargo-home",
        "uv-cache",
        "cc-cache",
        "llm-cache",
        "rust",
    }
)
TMP_PROTECTED_PREFIXES = ("preflight-", "merge-task-", "bevy-target", "bevy-build")
PRIVATE_TARGET_IDLE_HOURS = 2  # T2424
CARGO_IDLE_DAYS = 0.25  # T2419: 6 h, was 2 days
DEFAULT_CARGO_CAP_GB = 150.0  # T3422
CARGO_HYGIENE_REGISTRY = host_registry.REGISTRY
MIN_FREE_GB = 50.0
OWNER_SUFFIXES = (".rec", ".iso", ".rvz", ".7z", ".zip", ".rar")
CACHE_RETENTION_HOURS = 48
DEFAULT_REPLAY_CAP_GB = 150.0
DEFAULT_REPLAY_KEEP = 3
DEFAULT_PREBUILT_KEEP = 12
_CACHE_KEY = re.compile(r"^[0-9a-f]{64}$")
_HOST_KEY = re.compile(r"^[0-9a-f]{40}$")


def default_cargo_cap_gb(host_name: str | None = None) -> float:
    """Resolve the tracked Cargo cleanup cap using the repository's host identity."""
    policy = host_registry.load(CARGO_HYGIENE_REGISTRY).section("cargo_hygiene")  # T3754
    if host_name is None:
        host_name = os.environ.get("TSFP_REMOTE_EXEC_HOST") or policy.get("local_host")
    if not isinstance(host_name, str) or not host_name:
        raise ValueError("cargo hygiene host identity is missing")
    by_host = policy.get("cap_gb_by_host", {})
    if not isinstance(by_host, dict):
        raise ValueError("cargo_hygiene.cap_gb_by_host must be an object")
    raw_cap = by_host.get(host_name, policy.get("default_cap_gb", DEFAULT_CARGO_CAP_GB))
    if isinstance(raw_cap, bool) or not isinstance(raw_cap, int | float):
        raise ValueError(f"invalid Cargo cleanup cap for host {host_name!r}")
    try:
        cap = float(raw_cap)
    except (TypeError, ValueError) as exc:
        raise ValueError(f"invalid Cargo cleanup cap for host {host_name!r}") from exc
    if cap < 0 or not math.isfinite(cap):
        raise ValueError(f"invalid Cargo cleanup cap for host {host_name!r}")
    return cap


def _walk_allocated(root: Path) -> int:
    """Count allocated bytes without opening file contents or following symlinks."""
    if not root.exists():
        return 0
    total = 0
    stack = [root]
    while stack:
        directory = stack.pop()
        try:
            with os.scandir(directory) as entries:
                for entry in entries:
                    try:
                        info = entry.stat(follow_symlinks=False)
                    except FileNotFoundError:
                        continue
                    if entry.is_dir(follow_symlinks=False):
                        stack.append(Path(entry.path))
                    else:
                        total += getattr(info, "st_blocks", 0) * 512 or info.st_size
        except FileNotFoundError:
            continue
    return total


def _read_small_json(path: Path, limit: int = 1024 * 1024) -> dict | None:
    try:
        if path.is_symlink():
            return None
        if path.stat().st_size > limit:
            return None
        value = json.loads(path.read_text(errors="replace"))
    except (OSError, ValueError):
        return None
    return value if isinstance(value, dict) else None


def _try_existing_lock(path: Path) -> bool:
    """Check an existing flock without creating a lock file (used by dry-run)."""
    if not path.exists():
        return True
    try:
        fd = os.open(path, os.O_RDONLY)
    except OSError:
        return False
    try:
        try:
            fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
            return True
        except BlockingIOError:
            return False
    finally:
        os.close(fd)


def _json_strings(value: object) -> Iterator[str]:
    if isinstance(value, str):
        yield value
    elif isinstance(value, dict):
        for child in value.values():
            yield from _json_strings(child)
    elif isinstance(value, list):
        for child in value:
            yield from _json_strings(child)


def _recent_receipt_references(
    main: Path, cache: Path, run_keys: set[str], cutoff: float
) -> tuple[set[str], set[str]]:
    """Return recent run keys and prebuilt identifiers from known receipt roots."""
    run_refs: set[str] = set()
    host_refs: set[str] = set()
    sources = [cache]
    owner = main / "tmp/owner"
    sources.extend((owner / "adoption-gate", owner / "adoption-gate-snapshots", owner / "traces"))
    temp = Path(os.environ.get("TMPDIR", "/tmp"))
    try:
        for entry in os.scandir(temp):
            if entry.name.startswith("verify_all_") and entry.is_dir(follow_symlinks=False):
                sources.append(Path(entry.path))
    except OSError:
        pass
    for source in sources:
        if not source.is_dir():
            continue
        stack = [source]
        while stack:
            directory = stack.pop()
            # Replay cache JSON receipts are only the immediate <key>.json files.
            # Never walk or open JSON produced inside runs/<key>.
            direct_receipts = source == cache
            try:
                with os.scandir(directory) as entries:
                    for entry in entries:
                        try:
                            if entry.is_dir(follow_symlinks=False):
                                if not direct_receipts:
                                    stack.append(Path(entry.path))
                                continue
                            if not entry.is_file(follow_symlinks=False) or not entry.name.endswith(
                                ".json"
                            ):
                                continue
                            if entry.stat(follow_symlinks=False).st_mtime < cutoff:
                                continue
                            data = _read_small_json(Path(entry.path))
                            if not data:
                                continue
                            for value in _json_strings(data):
                                path_key = Path(value).name
                                if path_key in run_keys and ("runs" in value or value == path_key):
                                    run_refs.add(path_key)
                                for key in run_keys:
                                    if key in value:
                                        run_refs.add(key)
                                host_refs.update(
                                    re.findall(r"(?<![0-9a-f])[0-9a-f]{40,64}(?![0-9a-f])", value)
                                )
                        except OSError:
                            continue
            except OSError:
                continue
    return run_refs, host_refs


def _live_replay_keys(runs: Path, run_keys: set[str]) -> set[str]:
    """Detect live capture commands whose argv names a run output directory."""
    found: set[str] = set()
    prefix = str(runs.resolve()) + os.sep
    proc = Path("/proc")
    try:
        entries = list(os.scandir(proc))
    except OSError as exc:
        raise safety.Unsafe("cannot inspect processes before replay cleanup") from exc
    for entry in entries:
        if not entry.name.isdecimal():
            continue
        try:
            raw = (Path(entry.path) / "cmdline").read_bytes()[:65536]
        except (FileNotFoundError, ProcessLookupError):
            continue
        except PermissionError:
            # The per-key replay flock still protects normal submissions; skip this PID.
            continue
        command = raw.decode("utf-8", "replace")
        offset = command.find(prefix)
        while offset >= 0:
            key = command[offset + len(prefix) :].split(os.sep, 1)[0]
            if key in run_keys:
                found.add(key)
            offset = command.find(prefix, offset + len(prefix))
    return found


def prune_replay_cache(
    main: Path,
    *,
    cap_gb: float = DEFAULT_REPLAY_CAP_GB,
    replay_keep: int = DEFAULT_REPLAY_KEEP,
    prebuilt_keep: int = DEFAULT_PREBUILT_KEEP,
    apply: bool = False,
) -> dict[str, int]:
    """Dry-run-first retention for shared replay runs and immutable prebuilt hosts."""
    if cap_gb < 0 or replay_keep < 1 or prebuilt_keep < 1:
        raise ValueError("cache cap must be nonnegative and keep counts must be positive")
    cache_root = main / "tmp/wt-cache"
    replay_cache = Path(os.environ.get("TSFP_REPLAY_CACHE", str(cache_root / "replays")))
    runs = replay_cache / "runs"
    prebuilt_cache = Path(os.environ.get("TSFP_PREBUILT_ROOT", str(cache_root / "prebuilt")))
    for label, path in (
        ("cache", cache_root),
        ("replay cache", replay_cache),
        ("replay runs", runs),
        ("prebuilt cache", prebuilt_cache),
    ):
        if path.is_symlink():
            raise safety.Unsafe(f"{label} path is a symlink: {path}")
    now = time.time()
    cutoff = now - CACHE_RETENTION_HOURS * 3600
    keys = (
        {
            entry.name
            for entry in os.scandir(runs)
            if entry.is_dir(follow_symlinks=False) and _CACHE_KEY.fullmatch(entry.name)
        }
        if runs.is_dir()
        else set()
    )
    # Prebuilt hosts named by recent receipts stay (T3412). Replay runs follow the single
    # replay_gc policy (T3467, owner decision 2026-10-21): raw runs are a reproducible cache, so
    # doc, trace-store and receipt references no longer pin them and the cap no longer gates it.
    _recent_run_refs, recent_host_refs = _recent_receipt_references(
        main, replay_cache, keys, cutoff
    )
    total_bytes = _walk_allocated(cache_root)
    from tools.wt import replay_gc

    replay = replay_gc.gc(main, apply=apply, newest_per_group=replay_keep, quiet=True)
    replay_freed = replay["stats"]["deleted"][1]
    deleted_runs = replay["deleted"]
    protected_runs = sum(replay["stats"][reason][0] for reason in replay_gc.REASONS)
    live_runs = replay["stats"]["in-use"][0]

    # T3412 host leases are advisory read locks under .locks/<commit>.lock.
    prebuilt_removed: list[str] = []
    prebuilt_freed = 0
    try:
        from tools.wt import prebuilt

        if prebuilt_cache.is_dir():
            entries = []
            for directory in prebuilt_cache.iterdir():
                if (
                    directory.is_symlink()
                    or not directory.is_dir()
                    or not _HOST_KEY.fullmatch(directory.name)
                ):
                    continue
                manifest = prebuilt._manifest(directory)
                if manifest:
                    entries.append((float(manifest.get("built_at_epoch", 0)), directory, manifest))
            entries.sort(reverse=True, key=lambda item: item[0])
            keep_ids = {directory.name for _time, directory, _manifest in entries[:prebuilt_keep]}
            gc_lock = (
                prebuilt._lock_path(prebuilt_cache, ".gc.lock")
                if apply
                else prebuilt_cache / ".locks" / ".gc.lock"
            )
            if gc_lock.is_symlink():
                raise safety.Unsafe(f"prebuilt GC lock is a symlink: {gc_lock}")
            gc_available = True if apply else _try_existing_lock(gc_lock)
            if apply:
                gc_context = prebuilt._locked(gc_lock, fcntl.LOCK_EX)
                acquired = gc_context.__enter__()
                gc_available = acquired
            try:
                for _built_at, directory, manifest in entries[prebuilt_keep:]:
                    identities = {directory.name, str(manifest.get("input_hash", ""))}
                    if (
                        directory.name in recent_host_refs
                        or identities & recent_host_refs
                        or directory.name in keep_ids
                    ):
                        continue
                    host_lock = prebuilt_cache / ".locks" / f"{directory.name}.lock"
                    if host_lock.is_symlink():
                        continue
                    if not gc_available:
                        continue
                    if apply:
                        available_context = prebuilt._locked(
                            prebuilt._lock_path(prebuilt_cache, f"{directory.name}.lock"),
                            fcntl.LOCK_EX,
                            nonblocking=True,
                        )
                        available = available_context.__enter__()
                    else:
                        available_context = None
                        available = _try_existing_lock(host_lock)
                    try:
                        if not available:
                            continue
                        size = _walk_allocated(directory)
                        if apply:
                            prebuilt._make_writable(directory)
                            shutil.rmtree(directory)
                        prebuilt_freed += size
                        prebuilt_removed.append(directory.name)
                    finally:
                        if available_context is not None:
                            available_context.__exit__(None, None, None)
            finally:
                if apply:
                    gc_context.__exit__(None, None, None)
    except (OSError, ValueError) as exc:
        raise safety.Unsafe(f"prebuilt retention failed safely: {exc}") from exc

    result = {
        "replay_runs": len(deleted_runs),
        "replay_bytes": replay_freed,
        "prebuilt_hosts": len(prebuilt_removed),
        "prebuilt_bytes": prebuilt_freed,
        "cache_bytes_before": total_bytes,
        "cache_bytes_after_estimate": max(0, total_bytes - replay_freed - prebuilt_freed),
    }
    mode = "applied" if apply else "dry-run"
    print(
        f"cache retention {mode}: replay_runs={result['replay_runs']} "
        f"replay_gb={replay_freed / 1e9:.2f} prebuilt_hosts={result['prebuilt_hosts']} "
        f"prebuilt_gb={prebuilt_freed / 1e9:.2f} "
        f"cache_gb={total_bytes / 1e9:.2f}->{result['cache_bytes_after_estimate'] / 1e9:.2f} "
        f"cap_gb={cap_gb:g} protected_runs={protected_runs} live_runs={live_runs}"
    )
    return result


def newer_than(path: Path, cutoff: float, skip: frozenset[str] = frozenset()) -> bool:
    """True when any entry under path (symlinks not followed) is newer than cutoff."""
    try:
        if path.lstat().st_mtime > cutoff:
            return True
    except OSError:
        return True
    for parent, dirs, files in os.walk(path, followlinks=False):
        dirs[:] = [d for d in dirs if d not in skip and d != ".git"]
        for name in [*dirs, *files]:
            try:
                if (Path(parent) / name).lstat().st_mtime > cutoff:
                    return True
            except OSError:
                return True
    return False


def tree_bytes(path: Path) -> int:
    total = 0
    for parent, _dirs, files in os.walk(path, followlinks=False):
        for name in files:
            try:
                total += (Path(parent) / name).lstat().st_size
            except OSError:
                pass
    return total


def contains_owner_data(path: Path) -> bool:
    for _parent, _dirs, files in os.walk(path, followlinks=False):
        if any(name.endswith(".rec") for name in files):
            return True
    return False


def doc_reference_blob(main: Path) -> str:
    parts = []
    for name in ("context.md", "AGENTS.md"):
        file = main / name
        if file.is_file():
            parts.append(file.read_text(errors="replace"))
    for file in (main / "docs").rglob("*.md"):
        try:
            parts.append(file.read_text(errors="replace"))
        except OSError:
            pass
    return "\n".join(parts)


def _rebuildable_dirs(root: Path) -> list[Path]:
    """Ignored, untracked, non-symlink rebuildable directories (any depth)."""
    out = safety.git(
        root, "ls-files", "--others", "--ignored", "--exclude-standard", "--directory", "-z"
    )
    found = []
    for name in filter(None, out.split("\0")):
        path = root / name.rstrip("/")
        if path.name in REBUILDABLE and path.is_dir() and not path.is_symlink():
            found.append(path)
    found.extend(
        root / rel
        for rel in REBUILDABLE_PATHS
        if (root / rel).is_dir() and not (root / rel).is_symlink()
    )
    # Nested rebuildable dirs (for example crates/x/target) only appear when git lists
    # the parent; also scan one level below top-level ignored directories.
    return sorted(set(found))


def strip_rebuildable(
    main: Path, root: Path, branch: str, idle_hours: float = 24, apply: bool = False
) -> tuple[int, str]:
    """Delete rebuildable artifacts of an old, unclaimed, unused tree; keep all else.

    Returns (bytes, reason); reason is "stripped"/"would strip" or why it was skipped.
    """
    if safety.claim_blockers(main, root, branch):
        return 0, "claimed"
    if safety.processes(root):
        return 0, "in use"
    if newer_than(root, time.time() - idle_hours * 3600, REBUILDABLE):
        return 0, "active"
    freed = 0
    for path in _rebuildable_dirs(root):
        size = tree_bytes(path)
        if apply:
            safety._remove_cache(path)
        freed += size
    return freed, "stripped" if apply else "would strip"


def fd_targets(entry: Path) -> list[str]:
    """Open-fd link targets of one /proc/<pid> entry (the one shared fd resolver, T2441).

    A vanished fd or process is skipped and the scan continues. An unreadable process
    (PermissionError, no sudo fallback) raises safety.Unsafe so the scan fails closed.
    """
    try:
        targets = []
        for link in list((entry / "fd").iterdir()):
            try:
                targets.append(os.readlink(link))
            except (FileNotFoundError, ProcessLookupError):
                continue  # fd closed during the scan (for example the scan's own dir fd)
        return targets
    except FileNotFoundError:
        return []
    except PermissionError:
        if safety._zombie(entry):
            return []
        out = subprocess.run(
            ["sudo", "-n", "find", str(entry / "fd"), "-mindepth", "1", "-printf", "%l\n"],
            capture_output=True,
            text=True,
            timeout=10,
        )
        if out.returncode:
            raise safety.Unsafe(f"cannot inspect process {entry.name} open files") from None
        return out.stdout.splitlines()
    except OSError:
        return []  # process exited during the scan


def open_by_process(path: Path, proc: Path = Path("/proc")) -> bool:
    """True when any process has its cwd or an open fd under path.

    Raises safety.Unsafe when a process cannot be inspected (fail closed).
    """
    root = str(path.resolve())
    for entry in proc.iterdir():
        if not entry.name.isdecimal():
            continue
        try:
            targets = [os.readlink(entry / "cwd")]
        except OSError:
            targets = []
        for target in targets + fd_targets(entry):
            if target == root or target.startswith(root + os.sep):
                return True
    return False


def prune_private_targets(main: Path, idle_hours: float = 2, apply: bool = False) -> int:
    """T2424: delete private `target/` and `crates/*/target` dirs of every worktree.

    Builds belong in the shared tmp/wt-cache/cargo-target/<family>; a private dir idle for
    idle_hours (and not open by any process) is stale. Symlinks are never followed.
    """
    cutoff = time.time() - idle_hours * 3600
    freed = 0
    for record in safety.registered(main):
        root = Path(record["worktree"])
        if root.is_symlink() or not root.is_dir():
            continue
        if root.resolve().parent == main / "tmp" / "wt-trash":
            continue
        for path in [root / "target", *sorted(root.glob("crates/*/target"))]:
            if path.is_symlink() or not path.is_dir():
                continue
            if newer_than(path, cutoff):
                continue
            try:
                if open_by_process(path):
                    continue
            except safety.Unsafe:
                continue  # cannot inspect every process: treat as in use
            size = tree_bytes(path)
            if apply:
                shutil.rmtree(path, ignore_errors=True)
            freed += size
            print(f"private-target {'pruned' if apply else 'would prune'} {path} {size}")
    return freed


def prune_cargo_target(
    main: Path,
    idle_hours: float = 6,
    cap_gb: float | None = None,
    apply: bool = False,
) -> int:
    """Retain whole Cargo target profiles using non-blocking, profile-scoped locks."""
    cap_gb = default_cargo_cap_gb() if cap_gb is None else cap_gb
    base = main / "tmp" / "wt-cache" / "cargo-target"
    if base.is_symlink() or not base.is_dir():
        return 0
    if idle_hours < 0 or cap_gb < 0:
        raise ValueError("cargo retention hours and cap must be non-negative")
    cutoff = time.time() - idle_hours * 3600
    profiles: list[Path] = []
    for family in sorted(base.iterdir()):
        if family.is_symlink() or not family.is_dir():
            continue
        children = [p for p in family.iterdir() if p.is_dir() and not p.is_symlink()]
        if not children:
            profiles.append(family)
            continue
        for child in children:
            nested = [p for p in child.iterdir() if p.is_dir() and not p.is_symlink()]
            cargo_profiles = [p for p in nested if p.name in {"debug", "release"}]
            if cargo_profiles:  # target triples place the Cargo profile one level deeper
                profiles.extend(cargo_profiles)
            elif child.name in {"debug", "release"} or (child / "deps").is_dir():
                profiles.append(child)
            else:
                profiles.append(child)  # legacy family/profile layout

    def profile_size(profile: Path) -> int:
        return tree_bytes(profile)

    cap_bytes = int(cap_gb * 1_000_000_000)

    from tools.wt import buildslots

    def profile_available(profile: Path) -> bool:
        """Read-only lock probe for dry-run; actual deletion holds the lock exclusively."""
        lock_path = buildslots.profile_lock_path(main, profile)
        return not lock_path.exists() or _try_existing_lock(lock_path)

    def inventory() -> tuple[dict[Path, int], int, list[tuple[float, int, Path, bool]]]:
        sizes = {profile: profile_size(profile) for profile in profiles if profile.exists()}
        candidates: list[tuple[float, int, Path, bool]] = []
        for profile in sizes:
            try:
                if open_by_process(profile) or not profile_available(profile):
                    continue
            except safety.Unsafe:
                continue
            try:
                stat = profile.stat()
            except OSError:
                continue
            candidates.append(
                (stat.st_mtime, sizes[profile], profile, not newer_than(profile, cutoff))
            )
        return sizes, sum(sizes.values()), candidates

    sizes, total, candidates = inventory()

    # Largest first; ties prefer the oldest. Age-only pruning also follows this order.
    candidates.sort(key=lambda item: (-item[1], item[0], str(item[2])))
    freed = 0
    # First enforce age retention. Only then delete recent profiles if the shared cache
    # remains over cap; otherwise a large recent profile could be deleted before a set
    # of stale profiles that would have brought the total below the cap.
    for age_only in (True, False):
        for _modified, size, profile, age_idle in candidates:
            if age_idle != age_only or (not age_idle and total <= cap_bytes):
                continue
            lock_fd = None
            if apply:
                lock_fd = buildslots.acquire_profile_lock(
                    main, profile, shared=False, nonblocking=True
                )
                if lock_fd is None:
                    continue
            try:
                # Lock first, then recheck process ownership immediately before removal.
                try:
                    if (
                        not profile.exists()
                        or open_by_process(profile)
                        or safety.processes(profile)
                    ):
                        continue
                except safety.Unsafe:
                    continue
                size = profile_size(profile)
                if apply:
                    shutil.rmtree(profile)
                freed += size
                total -= size
                reason = "idle" if age_idle else "cap"
                print(
                    f"cargo-profile {'pruned' if apply else 'would prune'}-{reason} "
                    f"{profile} {size}"
                )
            finally:
                if lock_fd is not None:
                    os.close(lock_fd)
    return freed


def prune_play(main: Path, keep: int = 20, min_age_days: float = 3, apply: bool = False) -> int:
    """Keep the newest `keep` runs per variant; protect owner recordings and doc references."""
    play = main / "tmp" / "play"
    if play.is_symlink() or not play.is_dir():
        return 0
    blob = doc_reference_blob(main)
    variants: dict[str, list[Path]] = {}
    for run in play.iterdir():
        if run.is_symlink() or not run.is_dir():
            continue
        variant = run.name.rsplit("-", 2)[0] if run.name.count("-") >= 2 else run.name
        variants.setdefault(variant, []).append(run)
    cutoff = time.time() - min_age_days * 86400
    freed = 0
    for runs in variants.values():
        runs.sort(key=lambda r: r.stat().st_mtime, reverse=True)
        for run in runs[keep:]:
            if f"tmp/play/{run.name}" in blob or contains_owner_data(run):
                continue
            if newer_than(run, cutoff) or safety.processes(run):
                continue
            size = tree_bytes(run)
            if apply:
                shutil.rmtree(run)
            freed += size
    return freed


def prune_ephemeral_tmp(main: Path, days: float = 3, apply: bool = False) -> int:
    """Delete top-level tmp run data older than `days` unless protected or doc-referenced."""
    tmp = main / "tmp"
    if tmp.is_symlink() or not tmp.is_dir():
        return 0
    blob = doc_reference_blob(main)
    cutoff = time.time() - days * 86400
    freed = 0
    for entry in sorted(tmp.iterdir()):
        name = entry.name
        if entry.is_symlink() or name in TMP_PROTECTED or name.startswith(TMP_PROTECTED_PREFIXES):
            continue
        if name.endswith(OWNER_SUFFIXES) and entry.is_file():
            continue  # disc images, archives and recordings may be owner data
        if f"tmp/{name}" in blob:
            continue
        if entry.is_dir():
            if contains_owner_data(entry) or newer_than(entry, cutoff) or safety.processes(entry):
                continue
            size = tree_bytes(entry)
            if apply:
                shutil.rmtree(entry)
        else:
            if entry.lstat().st_mtime > cutoff:
                continue
            size = entry.lstat().st_size
            if apply:
                entry.unlink()
        freed += size
    return freed


RUSTUP_TMP_MAX_AGE_HOURS = 1.0  # T3834


def rustup_tmp_dirs(main: Path, home: Path | None = None) -> list[Path]:
    """`~/.rustup/tmp`, `$RUSTUP_HOME/tmp` and the project RUSTUP_HOME's tmp, deduplicated."""
    from tools.wt import core

    homes = [(home or Path.home()) / ".rustup", core.shared_rustup_home(main)]
    if os.environ.get("RUSTUP_HOME"):
        homes.append(Path(os.environ["RUSTUP_HOME"]))
    found: dict[str, Path] = {}
    for rustup_home in homes:
        tmp = rustup_home / "tmp"
        if tmp.is_dir() and not tmp.is_symlink():
            found.setdefault(str(tmp.resolve()), tmp)
    return list(found.values())


def sweep_rustup_tmp(
    main: Path,
    *,
    home: Path | None = None,
    max_age_hours: float = RUSTUP_TMP_MAX_AGE_HOURS,
    now: float | None = None,
    proc: Path = Path("/proc"),
    apply: bool = True,
) -> int:
    """T3834: delete rustup download/unpack leftovers older than `max_age_hours`.

    rustup leaves interrupted downloads in `<RUSTUP_HOME>/tmp`; claude-1 found 23 GB there on
    2026-10-26. An entry with anything newer than the cutoff, or held open by a process, stays.
    Returns the bytes freed (or that would be freed when `apply` is false)."""
    cutoff = (time.time() if now is None else now) - max_age_hours * 3600
    freed = 0
    for tmp in rustup_tmp_dirs(main, home):
        for entry in sorted(tmp.iterdir()):
            try:
                if newer_than(entry, cutoff) or open_by_process(entry, proc):
                    continue
                if entry.is_dir() and not entry.is_symlink():
                    size = tree_bytes(entry)
                    if apply:
                        shutil.rmtree(entry)
                else:
                    size = entry.lstat().st_size
                    if apply:
                        entry.unlink()
            except (OSError, safety.Unsafe) as exc:
                print(f"rustup tmp sweep kept {entry}: {exc}", flush=True)
                continue
            freed += size
    return freed


def retire_task_worktree(main: Path, branch: str) -> str:
    """After a successful merge: quarantine the task worktree through tools.wt, never rm."""
    if branch in ("", "main", "master"):
        return "no task branch"
    for record in safety.registered(main):
        if record.get("branch") != branch or Path(record["worktree"]).resolve() == main:
            continue
        result = subprocess.run(
            ["python3", "-m", "tools.wt", "remove", record["worktree"]],
            cwd=main,
            capture_output=True,
            text=True,
            timeout=900,
        )
        tail = (result.stderr or result.stdout).strip().splitlines()[-1:] or [""]
        return "quarantined" if result.returncode == 0 else f"retained: {tail[0][:120]}"
    return "no worktree"


def run_all(
    main: Path,
    *,
    apply: bool,
    strip_idle_hours: float = 24,
    cargo_idle_hours: float = 6,
    cargo_cap_gb: float | None = None,
) -> dict[str, int]:
    """Everything the hourly timer does beyond worktree trash: returns bytes per category."""
    freed = {"strip": 0, "private-target": 0, "cargo-target": 0, "play": 0, "tmp-ephemeral": 0}
    for record in safety.registered(main):
        root = Path(record["worktree"])
        if root.is_symlink() or not root.is_dir() or root.resolve() == main:
            continue
        if root.resolve().parent == main / "tmp" / "wt-trash":
            continue
        try:
            size, reason = strip_rebuildable(
                main, root.resolve(), record.get("branch", ""), strip_idle_hours, apply
            )
        except (safety.Unsafe, OSError) as exc:
            print(f"strip retain {root}: {exc}")
            continue
        if size:
            print(f"strip {reason} {root} {size}")
        freed["strip"] += size
    freed["private-target"] = prune_private_targets(main, PRIVATE_TARGET_IDLE_HOURS, apply)
    freed["cargo-target"] = prune_cargo_target(main, cargo_idle_hours, cargo_cap_gb, apply)
    freed["play"] = prune_play(main, 20, 3, apply)
    freed["tmp-ephemeral"] = prune_ephemeral_tmp(main, 3, apply)
    cache = prune_replay_cache(main, apply=apply)
    freed["cache-replay"] = cache["replay_bytes"]
    freed["cache-prebuilt"] = cache["prebuilt_bytes"]
    return freed


def refresh_disk_usage_if_stale(main: Path, now: float | None = None) -> str:
    """T3745: start the project-area du behind tools.datadisk (detached, niced) when stale."""
    from tools import datadisk

    now = time.time() if now is None else now
    budget = datadisk.load_budget()
    usage = datadisk.usage_file(main)
    stamp = usage.with_name("measure.started")
    try:
        if now - usage.stat().st_mtime < budget.usage_max_age_hours * 3600 / 2:
            return "fresh"
    except OSError:
        pass
    try:
        if now - stamp.stat().st_mtime < 3600:
            return "measuring"
    except OSError:
        pass
    stamp.parent.mkdir(parents=True, exist_ok=True)
    stamp.write_text(f"{now}\n")
    subprocess.Popen(
        [sys.executable, "-m", "tools.datadisk", "measure"],
        cwd=main,
        stdin=subprocess.DEVNULL,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        start_new_session=True,
    )
    return "started"


def enforce_budget(main: Path) -> dict[str, int]:
    """T3745: over the project budget or under a free floor, prune caches by retention, tightest
    retention first (the hourly limits halved). Returns bytes freed per category."""
    from tools import datadisk

    found = datadisk.status(main)
    if not found.problems:
        return {}
    print(f"disk budget: {'; '.join(found.problems)}; pruning caches", file=sys.stderr, flush=True)

    # I-T3885-47: budget pruning is best effort. One unremovable entry (a read-only cargo registry
    # copy under tmp/sub-reg2) aborted the whole prune and so every landing's preflight; contain the
    # error to its category, report it, and keep going.
    def attempt(name: str, prune: Callable[[], int]) -> int:
        try:
            return prune()
        except OSError as error:
            print(f"disk budget: {name} prune skipped: {error}", file=sys.stderr, flush=True)
            return 0

    freed = {
        "tmp-ephemeral": attempt("tmp-ephemeral", lambda: prune_ephemeral_tmp(main, 1, apply=True)),
        "play": attempt("play", lambda: prune_play(main, 5, 1, apply=True)),
        "cargo-target": attempt(
            "cargo-target", lambda: prune_cargo_target(main, 1, None, apply=True)
        ),
    }
    cap = float(os.environ.get("TSFP_REPLAY_CAP_GB", DEFAULT_REPLAY_CAP_GB)) / 2
    cache: dict = {"replay_bytes": 0, "prebuilt_bytes": 0}
    try:
        cache = prune_replay_cache(main, cap_gb=cap, apply=True)
    except OSError as error:
        print(f"disk budget: replay cache prune skipped: {error}", file=sys.stderr, flush=True)
    freed["cache-replay"] = cache["replay_bytes"]
    freed["cache-prebuilt"] = cache["prebuilt_bytes"]
    print(f"disk budget: freed {freed}", file=sys.stderr, flush=True)
    return freed


def free_gb(path: Path) -> float:
    usage = shutil.disk_usage(path)
    return usage.free / 1e9


def min_free_gb() -> float:
    try:
        return float(os.environ.get("TSFP_MIN_FREE_GB", MIN_FREE_GB))
    except ValueError:
        return MIN_FREE_GB


def prune_worker_candidates(main: Path, idle_hours: float = 2, apply: bool = False) -> int:
    """Quarantine abandoned worker_check worktrees through the guarded wt remove command."""
    if not math.isfinite(idle_hours) or idle_hours < 0:
        raise ValueError("idle_hours must be finite and non-negative")
    tmp = main / "tmp"
    if tmp.is_symlink() or not tmp.is_dir():
        return 0
    removed = 0
    for candidate in sorted(tmp.glob("wc-cand-*")):
        if candidate.is_symlink() or not candidate.is_dir():
            continue
        try:
            if not safety.is_scratch_candidate(candidate, main, idle_hours):
                continue
        except (OSError, safety.Unsafe):
            continue  # inability to establish inactivity must never authorize cleanup
        if not apply:
            print(f"would quarantine idle worker candidate {candidate}")
            removed += 1
            continue
        command = [
            sys.executable,
            "-m",
            "tools.wt",
            "remove",
            "--scratch",
            "--force-i-checked",
            str(candidate),
        ]
        env = {**os.environ}
        source_root = str(Path(__file__).resolve().parents[2])
        env["PYTHONPATH"] = os.pathsep.join(
            part for part in (source_root, env.get("PYTHONPATH", "")) if part
        )
        try:
            result = subprocess.run(
                command, cwd=main, env=env, capture_output=True, text=True, timeout=300
            )
        except (OSError, subprocess.SubprocessError) as exc:
            print(f"worker candidate remove failed for {candidate}: {exc}", flush=True)
            continue
        if result.returncode:
            print(
                f"worker candidate remove refused for {candidate}: "
                f"{(result.stderr or result.stdout).strip()[-500:]}",
                flush=True,
            )
            continue
        removed += 1
    return removed


def prune_dead_worktree_registrations(main: Path) -> None:
    """Prune only registrations whose checkout directories are missing, keeping every entry
    under another mount prefix of the repo (T3784: those are live in another container)."""
    from tools.wt import safe_prune

    try:
        _removed, kept = safe_prune.prune(main, expire="now")
    except (safe_prune.PruneError, OSError, subprocess.SubprocessError) as error:
        raise safety.Unsafe(f"worktree registration prune failed: {error}") from error
    if kept:
        print(f"hygiene: kept {len(kept)} other-mount worktree registrations", flush=True)


def tick(
    main: Path,
    scratch_ttl_hours: float | None = None,
    *,
    cargo_idle_hours: float = 6,
    cargo_cap_gb: float | None = None,
) -> dict[str, int]:
    """One hygiene pass (T2419): reap dead scratch (6 h TTL), expire trash by TTL, enforce the
    100 GB trash cap, strip caches, prune cargo targets. safety.gc runs run_all itself."""
    from tools.wt import scratch, scratch_sweep

    ttl = scratch.SCRATCH_TTL_HOURS if scratch_ttl_hours is None else scratch_ttl_hours
    try:  # leaked merge_task scratch (killed runs): idle 2 h, nobody inside, then delete
        scratch_sweep.sweep(main, hours=min(ttl, scratch_sweep.DEFAULT_HOURS), apply=True)
    except (scratch_sweep.Unreadable, OSError, subprocess.SubprocessError) as exc:
        print(f"scratch sweep skipped: {exc}", flush=True)
    scratch.gc(main, ttl_hours=ttl, prune=True)
    # Prune missing checkout registrations; this never removes present trees.
    try:
        prune_dead_worktree_registrations(main)
    except safety.Unsafe as exc:
        print(exc)
    safety.gc(main, idle_hours=2, ttl_hours=72, prune=True, cache_idle_hours=2, prune_caches=True)
    worker_candidates = prune_worker_candidates(main, idle_hours=2, apply=True)
    from tools.wt import archive

    try:  # T2438: claim expiry, archive-then-delete (opt-in apply), low-disk alarm
        archive.hourly(main)
    except (safety.Unsafe, archive.ArchiveError, OSError) as exc:
        print(f"archive pass failed: {exc}", flush=True)
    prune_cargo_target(main, cargo_idle_hours, cargo_cap_gb, apply=True)
    try:  # T3834: stale rustup downloads (23 GB on 2026-10-26), never from a test's fake main
        if (main / ".git").is_dir():
            print(f"rustup tmp sweep: freed {sweep_rustup_tmp(main)} bytes", flush=True)
    except OSError as exc:
        print(f"rustup tmp sweep failed: {exc}", flush=True)
    prune_replay_cache(
        main,
        cap_gb=float(os.environ.get("TSFP_REPLAY_CAP_GB", DEFAULT_REPLAY_CAP_GB)),
        apply=True,
    )
    if (main / ".git").exists():  # T3745: project disk budget (tools/data/disk_budget.json)
        try:
            print(f"disk usage measure: {refresh_disk_usage_if_stale(main)}", flush=True)
            enforce_budget(main)
        except (OSError, ValueError, KeyError) as exc:
            print(f"disk budget pass failed: {exc}", flush=True)
    try:  # T3662: keep the primary-ref guard hook installed (I-T3662-3)
        from tools.wt import core

        if (main / ".git").is_dir():  # the primary checkout only, never a test's fake main
            print(f"primary guard: {core.install_primary_guard(main)}", flush=True)
    except (OSError, subprocess.SubprocessError) as exc:
        print(f"primary guard install failed: {exc}", flush=True)
    try:  # T3431/T3467: replay-run retention GC, every idle unpinned run
        from tools.wt import replay_gc

        replay_gc.hygiene_pass(main)
    except (safety.Unsafe, OSError, ValueError) as exc:
        print(f"replay gc failed: {exc}", flush=True)
    try:  # T3467: gate snapshots and gate trace bulk (owner decision 2026-10-21)
        from tools.wt import gate_data_gc

        gate_data_gc.run(main, apply=True)
    except (safety.Unsafe, OSError, ValueError) as exc:
        print(f"gate data gc failed: {exc}", flush=True)
    try:  # T3472: canonical build tree pool and verdict cache retention
        from tools.wt import canon_gc

        canon_gc.run(main, apply=True)
    except (OSError, ValueError) as exc:
        print(f"canon gc failed: {exc}", flush=True)
    if os.environ.get("TSFP_SCCACHE_ENSURE", "1") != "0":
        try:  # T3472: shared sccache store route (server supervisor, laptop forward, Deck reverse)
            from tools.wt import sccache_hosts

            print(f"sccache store: {sccache_hosts.hygiene_pass(main)}", flush=True)
            print(f"sccache local daemon: {sccache_hosts.ensure_local_server(main)}", flush=True)
        except Exception as exc:  # noqa: BLE001 - hosts away must not stop the hygiene tick
            print(f"sccache store ensure failed: {exc}", flush=True)
    try:  # T3644 (rule 36): per-crate warm-rebuild budget, detached and niced when stale
        from tools import build_budget

        print(
            f"build budget: {build_budget.hygiene(main, 24.0, build_budget.DEFAULT_TOP)}",
            flush=True,
        )
    except Exception as exc:  # noqa: BLE001 - a measurement problem must not stop the hygiene tick
        print(f"build budget failed: {exc}", flush=True)
    try:
        from tools import build_health_schedule

        print(f"build health: {build_health_schedule.schedule(main)}", flush=True)
    except Exception as exc:  # noqa: BLE001 - the read-only landing gate fails closed separately
        print(f"build health schedule failed: {type(exc).__name__}", flush=True)
    return {"worker-candidates": worker_candidates}


def refresh_ledger_if_stale(main: Path, timeout: int = 900) -> str:
    """T3449: keep the shared generated/ledger snapshot current with the tier inputs.

    The loop runs in the main checkout, so the refresh reads main's own registry, verification
    tiers and Tier 2 evidence (a worktree behind main can never write an older snapshot). It is a
    no-op while the snapshot is newer than every input, so a merged verify-all credit reaches
    `tools.ledger breadth` within one hygiene interval without any merge-time hook."""
    subsystems = main / "tools/data/function_subsystems.csv"
    if not subsystems.is_file():
        return "skipped: placement table missing"
    try:
        done = subprocess.run(
            [sys.executable, "-m", "tools.ledger", "refresh", "--if-stale", "--subsystems"]
            + [str(subsystems)],
            cwd=main,
            capture_output=True,
            text=True,
            timeout=timeout,
            check=False,
        )
    except (OSError, subprocess.SubprocessError) as exc:
        return f"failed: {exc}"
    lines = (done.stdout or done.stderr).strip().splitlines()
    return ("ok: " if done.returncode == 0 else f"exit {done.returncode}: ") + (
        lines[-1] if lines else ""
    )


def request_progress_publish(main: Path, timeout: int = 60) -> str:
    """T3736: the tick is only a safety net for the progress publisher (heatmap and badges go to
    the `progress` branch, never main). `request` is cheap, never blocks and retries a failed
    last run; merge_task requests one after every landing."""
    python = main / ".venv/bin/python"
    try:
        done = subprocess.run(
            [
                str(python) if python.exists() else sys.executable,
                "-m",
                "tools.progress_publish",
                "request",
            ],
            cwd=main,
            capture_output=True,
            text=True,
            timeout=timeout,
            check=False,
        )
    except (OSError, subprocess.SubprocessError) as exc:
        return f"failed: {exc}"
    lines = (done.stdout or done.stderr).strip().splitlines()
    return ("ok: " if done.returncode == 0 else f"exit {done.returncode}: ") + (
        lines[-1] if lines else ""
    )


def ensure_free(main: Path, min_gb: float | None = None) -> None:
    """Disk guard (T2419): below the floor run hygiene once, then refuse if still short.

    Floor: `TSFP_MIN_FREE_GB` (default 50, 0 disables). Raises safety.Unsafe.
    """
    floor = min_free_gb() if min_gb is None else min_gb
    if (main / "tmp").is_dir() and (main / ".git").exists():  # T3745: the project budget too
        from tools import datadisk

        try:
            datadisk.check("tools.wt run / scratch", main=main)
        except datadisk.DiskRefused as exc:
            enforce_budget(main)
            try:
                datadisk.check("tools.wt run / scratch", main=main)
            except datadisk.DiskRefused:
                raise safety.Unsafe(str(exc)) from exc
    if floor <= 0 or not (main / "tmp").is_dir() or free_gb(main) >= floor:
        return
    print(
        f"disk guard: {free_gb(main):.0f} GB free < {floor:g} GB, running hygiene",
        file=sys.stderr,
    )
    try:
        tick(main, scratch_ttl_hours=0.5)  # emergency: shorter scratch hold
    except (safety.Unsafe, OSError) as exc:
        print(f"disk guard: hygiene failed: {exc}", file=sys.stderr)
    left = free_gb(main)
    if left < floor:
        raise safety.Unsafe(
            f"only {left:.0f} GB free on the workspace (< {floor:g} GB) after hygiene; "
            "free space (python3 -m tools.wt gc --prune --prune-caches --scratch) "
            "or set TSFP_MIN_FREE_GB to override"
        )


# ---- background loop (T2419): survives without systemd/cron, re-armed by SessionStart ----


def _loop_pidfile(main: Path) -> Path:
    return main / "tmp" / ".hygiene-loop.pid"


def _pid_start(pid: int) -> str | None:
    try:
        return Path(f"/proc/{pid}/stat").read_text().rsplit(")", 1)[1].split()[19]
    except (OSError, IndexError):
        return None


def _lock_path(main: Path) -> Path:
    path = main / "tmp" / "wt-cache" / "hygiene-loop.lock"
    path.parent.mkdir(parents=True, exist_ok=True)
    return path


def _try_lock(main: Path) -> IO[str] | None:
    """Non-blocking exclusive flock; returns the open file (keep it open) or None if held."""
    import fcntl

    handle = open(_lock_path(main), "a+")  # noqa: SIM115 - held for the process lifetime
    try:
        fcntl.flock(handle, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except OSError:
        handle.close()
        return None
    return handle


def loop_alive(main: Path) -> bool:
    """True when another process holds the singleton lock (the flock is the authority)."""
    handle = _try_lock(main)
    if handle is None:
        return True
    handle.close()
    return False


def _stamp(main: Path) -> Path:
    return main / "tmp" / "wt-cache" / "hygiene-last-pass"


def pass_is_fresh(main: Path, interval: int) -> bool:
    """True when the previous pass finished less than `interval` seconds ago."""
    try:
        return time.time() - _stamp(main).stat().st_mtime < interval
    except OSError:
        return False


def _lower_priority() -> None:
    try:
        os.nice(19)
    except OSError:
        pass


def _hygiene_code_hash(main: Path) -> str:
    """Hash the Python tool modules a long-lived loop may execute or import."""
    digest = hashlib.sha256()
    for package in (main / "tools" / "wt", main / "tools" / "agents"):
        for path in sorted(package.glob("*.py")):
            digest.update(path.relative_to(main).as_posix().encode())
            digest.update(b"\0")
            try:
                with path.open("rb") as source:
                    for chunk in iter(lambda: source.read(1024 * 1024), b""):
                        digest.update(chunk)
            except OSError:
                # A transient replacement during a deploy gets another check next pass.
                digest.update(b"<unreadable>")
            digest.update(b"\0")
    return digest.hexdigest()


def run_scheduled_remote_prune(main: Path, *, now: float | None = None) -> str:
    """T3514: delete merged branches on origin at most every two hours per clone (dry run on
    TSFP_PRUNE_REMOTE=dry). The sweep carries its own safety rules and forge health gate."""
    from tools.wt import prune_remote

    if os.environ.get("TSFP_PRUNE_REMOTE") == "off":
        return "disabled (TSFP_PRUNE_REMOTE=off)"
    cache = main / "tmp" / "wt-cache"
    cache.mkdir(parents=True, exist_ok=True)
    stamp = cache / "remote-prune-last-run"
    current = time.time() if now is None else now
    try:
        if current - stamp.stat().st_mtime < 2 * 60 * 60:
            return "skipped (recent)"
    except OSError:
        pass
    if now is None:
        stamp.touch()
    report = prune_remote.sweep(
        main,
        apply=os.environ.get("TSFP_PRUNE_REMOTE") != "dry",
        backup_file=cache / "origin-prune-backup.log",
        log=lambda line: print(f"  {line}", flush=True),
    )
    return "ok" if report.ok else f"failed: {report.stopped[:200]}"


def run_scheduled_git_maintenance(main: Path, *, now: float | None = None) -> str:
    """Run Git's costly maintenance at most every six hours, serialized per clone."""
    cache = main / "tmp" / "wt-cache"
    cache.mkdir(parents=True, exist_ok=True)
    lock_path = cache / "git-maintenance.lock"
    stamp = cache / "git-maintenance-last-run"
    with lock_path.open("a+") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        current = time.time() if now is None else now
        try:
            if current - stamp.stat().st_mtime < 6 * 60 * 60:
                return "skipped (recent)"
        except OSError:
            pass
        result = subprocess.run(
            [
                "git",
                "-C",
                str(main),
                "maintenance",
                "run",
                "--task=gc",
                "--task=loose-objects",
                "--task=commit-graph",
            ],
            capture_output=True,
            text=True,
            timeout=3600,
            check=False,
        )
        if result.returncode:
            return f"failed ({result.returncode}): {result.stderr[-500:]}"
        if now is None:
            stamp.touch()
        else:
            stamp.touch()
            os.utime(stamp, (current, current))
        return "ran"
    try:
        subprocess.run(
            ["ionice", "-c", "3", "-p", str(os.getpid())],
            timeout=5,
            check=False,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
    except (OSError, subprocess.SubprocessError):
        pass


LOOP_NAME = "hygiene"  # tools/data/loops.json


def ensure_loop(
    main: Path,
    interval: int = 600,
    *,
    cargo_idle_hours: float = 6,
    cargo_cap_gb: float | None = None,
) -> str:
    """Start the detached hygiene loop unless already running. Safe to call every session.

    T3500: also the per-PID-namespace lock watchdog (tools.wt.lease). The hygiene loop is one
    per host (its flock is shared by every container), the watchdog one per container,
    because each may signal only its own processes."""
    try:
        from tools.wt import lease

        lease.ensure_watch_loop(main)
    except OSError as exc:
        print(f"lock watch loop not started: {exc}", file=sys.stderr)
    if loop_alive(main):
        return "running"
    if os.environ.get("PYTEST_CURRENT_TEST"):
        # T3763: tests (and the subprocesses they start) never spawn real daemons; 20 watch and
        # hygiene loops leaked from tests/test_fleet_server_admission.py runs on 2026-10-26
        return "disabled under pytest"
    log = main / "tmp" / "hygiene-loop.log"
    command = [
        sys.executable,
        "-m",
        "tools.wt",
        "hygiene-loop",
        "--interval",
        str(interval),
        "--cargo-idle-hours",
        str(cargo_idle_hours),
    ]
    if cargo_cap_gb is not None:
        command.extend(("--cargo-cap-gb", str(cargo_cap_gb)))
    with open(log, "ab") as out:
        subprocess.Popen(
            command,
            cwd=main,
            stdin=subprocess.DEVNULL,
            stdout=out,
            stderr=out,
            start_new_session=True,
        )
    return "started"


def run_loop(
    main: Path,
    interval: int,
    once: bool = False,
    *,
    cargo_idle_hours: float = 6,
    cargo_cap_gb: float | None = None,
) -> int:
    """Singleton loop: a second start exits at once; fresh passes are skipped."""
    cargo_cap_gb = default_cargo_cap_gb() if cargo_cap_gb is None else cargo_cap_gb
    handle = None
    if not once:
        handle = _try_lock(main)
        if handle is None:
            return 0
        handle.truncate(0)
        handle.write(f"{os.getpid()}\n")
        handle.flush()
    _lower_priority()
    code_hash = _hygiene_code_hash(main)
    if not once:
        from tools.agents import loops

        loops.install_term_handler()  # T3763
    while True:
        if not once and not (main / ".git").exists():
            print(f"hygiene-loop: {main} is gone, exiting", flush=True)  # leaked test loops
            return 0
        if once or not pass_is_fresh(main, interval):
            if _hygiene_code_hash(main) != code_hash:
                print("hygiene-loop: code changed, re-exec", flush=True)
                if Path(sys.argv[0]).name == "__main__.py":
                    command = [sys.executable, "-m", "tools.wt", *sys.argv[1:]]
                else:
                    command = [sys.executable, *sys.argv]
                os.execv(sys.executable, command)
            try:
                print(
                    f"hygiene tick {time.strftime('%FT%T')} free={free_gb(main):.0f}GB", flush=True
                )
                print(
                    tick(main, cargo_idle_hours=cargo_idle_hours, cargo_cap_gb=cargo_cap_gb),
                    flush=True,
                )
                print(f"git maintenance: {run_scheduled_git_maintenance(main)}", flush=True)
                print(f"ledger refresh: {refresh_ledger_if_stale(main)}", flush=True)
                print(f"progress publish: {request_progress_publish(main)}", flush=True)
                print(f"origin branch prune: {run_scheduled_remote_prune(main)}", flush=True)
            except Exception as exc:  # noqa: BLE001 - the loop must outlive any single failure
                print(f"hygiene tick failed: {exc}", flush=True)
            _stamp(main).parent.mkdir(parents=True, exist_ok=True)
            _stamp(main).touch()
        if once:
            return 0
        loops.tick(LOOP_NAME, main)
        time.sleep(interval)
