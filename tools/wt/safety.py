"""Conservative worktree quarantine and TTL collection (T2250).

Overrides acknowledge ignored data only; they never override liveness or Git checks.
No symlinks are followed when inventorying or deleting caches.
"""

from __future__ import annotations

import json
import logging
import os
import re
import shutil
import stat
import subprocess
import sys
import time
from datetime import UTC, datetime
from pathlib import Path

from tools.wt import core

CACHE_NAMES = frozenset(
    {
        "target",
        "node_modules",
        "__pycache__",
        ".pytest_cache",
        ".ruff_cache",
        ".tsfp-scratch-owner.json",
        "venvs",
    }
)
BOOTSTRAP_LINKS = frozenset(
    {
        ".venv",
        "generated",
        "src/xbox/xdk_*.inc",
        "src/xbox/xdk_*.c",
        "src/xbox/xdk_*.h",
        "tools/data/function_subsystems.csv",
    }
)
SCRATCH_PATH_PARTS = ("merge-task-", "preflight-", "sup-verify-", "file-batch-")


class Unsafe(RuntimeError):
    """A worktree must be retained."""


def git(root: Path, *args: str, timeout: int = 60) -> str:
    result = subprocess.run(
        ["git", *args], cwd=root, capture_output=True, text=True, timeout=timeout
    )
    if result.returncode:
        raise Unsafe(result.stderr.strip() or f"git {args} failed")
    return result.stdout


def registered(main: Path) -> list[dict[str, str]]:
    records = []
    for block in git(main, "worktree", "list", "--porcelain").strip().split("\n\n"):
        record = dict(line.partition(" ")[::2] for line in block.splitlines())
        records.append(record)
    return records


def _zombie(entry: Path) -> bool:
    try:
        stat = (entry / "stat").read_text()
    except OSError:
        return False
    return stat[stat.rfind(")") + 2 :].startswith("Z")


def _process_uid(entry: Path) -> int:
    """Return the procfs owner for a PID, failing closed if identity is unreadable."""
    try:
        return entry.stat().st_uid
    except FileNotFoundError:
        raise
    except OSError as exc:
        raise Unsafe(f"cannot inspect process {entry.name} uid") from exc


def _process_cwd(entry: Path) -> Path:
    return (entry / "cwd").resolve(strict=True)


_log = logging.getLogger(__name__)
SUDO_READLINK_TIMEOUT = 5


def _sudo_readlink(pid: str, proc: Path) -> Path | None:
    """Root-readable cwd via passwordless sudo; None when sudo is unavailable or fails."""
    try:
        result = subprocess.run(
            ["sudo", "-n", "readlink", str(proc / pid / "cwd")],
            capture_output=True,
            text=True,
            timeout=SUDO_READLINK_TIMEOUT,
        )
    except (OSError, subprocess.SubprocessError):
        return None
    out = result.stdout.strip()
    return Path(out) if result.returncode == 0 and out else None


def _is_user_systemd(entry: Path) -> bool:
    try:
        comm = (entry / "comm").read_text().strip()
        argv = (entry / "cmdline").read_bytes().split(b"\0")
    except OSError:
        return False
    return comm == "systemd" and b"--user" in argv


def _inside(path: Path, root: Path) -> bool:
    return path == root or root in path.parents


def _unreadable_cwd_blocks(entry: Path, root: Path, proc: Path) -> bool:
    """Decide for a process whose cwd is unreadable. Raises Unsafe only if nothing can tell."""
    pid = entry.name
    if _is_user_systemd(entry):
        _log.info("wt liveness pid %s: rule 1 (user systemd manager) non-blocking", pid)
        return False
    cwd = _sudo_readlink(pid, proc)
    if cwd is not None:
        blocks = _inside(cwd, root)
        _log.info("wt liveness pid %s: rule 2 (sudo readlink %s) blocking=%s", pid, cwd, blocks)
        return blocks
    paths: list[Path] = []
    readable = False
    try:
        paths.append(Path(os.readlink(entry / "root")))
        readable = True
    except OSError:
        pass
    try:
        for line in (entry / "maps").read_text().splitlines():
            parts = line.split(None, 5)
            if len(parts) == 6 and parts[5].startswith("/"):
                paths.append(Path(parts[5]))
        readable = True
    except OSError:
        pass
    if readable:
        blocks = any(_inside(path, root) for path in paths)
        _log.info("wt liveness pid %s: rule 3 (root/maps paths) blocking=%s", pid, blocks)
        return blocks
    _log.warning("wt liveness pid %s: rule 4 (nothing inspectable) fail closed", pid)
    raise Unsafe(f"cannot inspect process {pid} cwd")


def processes(root: Path, proc: Path = Path("/proc")) -> list[str]:
    """Scan every same-UID process; ignore container-external PIDs we cannot inspect."""
    if not (proc / "self/cwd").exists():
        raise Unsafe("/proc cwd scan unavailable")
    found = []
    uid = os.getuid()
    for entry in proc.iterdir():
        if not entry.name.isdecimal():
            continue
        try:
            process_uid = _process_uid(entry)
        except FileNotFoundError:
            continue  # exited during the scan
        if process_uid != uid:
            continue  # another UID is outside this container/user's liveness scope
        try:
            cwd = _process_cwd(entry)
        except FileNotFoundError:
            continue  # exited or kernel thread
        except PermissionError:
            if _zombie(entry):
                continue  # a zombie holds no cwd or files; only its parent can reap it
            if entry.name == "1":
                continue  # container PID 1 is not a worker process; its cwd may be hidden
            if _unreadable_cwd_blocks(entry, root, proc):
                found.append(entry.name)
            continue
        if cwd == root or root in cwd.parents:
            found.append(entry.name)
    return found


# T3729 (claude-w1, approved narrowly by claude-1 2026-10-24): test and build caches a full pytest
# run leaves in a worktree. All are gitignored and rebuilt on demand: tests/cc_cache.py
# objects, the native and t1091 cmake dirs, the tools/nv2a vkrun/vkvert binaries and the
# lockfile cargo writes for a standalone crate. Exact prefixes only: the rest of generated/
# holds lifted code and ledgers.
TEST_BUILD_CACHE_DIRS = (
    "tmp/cc-cache",
    "tmp/native-test-build",
    "tmp/t1091-volume-build",
    "generated/shaders/bin",
    # 2026-10-25: host lift and surface-stage outputs, rebuilt from tracked source.
    "tmp/private-host",
)


def is_test_build_cache(name: str) -> bool:
    """True for an untracked path under TEST_BUILD_CACHE_DIRS or a crates/<crate>/Cargo.lock."""
    if any(name == d or name.startswith(d + "/") for d in TEST_BUILD_CACHE_DIRS):
        return True
    parts = Path(name).parts
    return len(parts) == 3 and parts[0] == "crates" and parts[2] == "Cargo.lock"


def inventory(root: Path) -> list[dict]:
    items = []
    names = git(root, "ls-files", "--others", "--exclude-standard", "--directory", "-z")
    names += git(root, "ls-files", "--others", "--ignored", "--exclude-standard", "-z")
    for name in names.split("\0"):
        if not name:
            continue
        name = name.rstrip("/")
        path = root / name
        stat = path.lstat()
        parts = Path(name).parts
        allowed = (
            (
                path.is_symlink()
                and (
                    path.name in CACHE_NAMES
                    or path.name in {".venv", "generated"}
                    or Path(name).match("src/xbox/xdk_*.inc")
                    or Path(name).match("src/xbox/xdk_*.c")
                    or Path(name).match("src/xbox/xdk_*.h")
                    or name == "tools/data/function_subsystems.csv"
                )
            )
            or (
                not path.is_symlink()
                and path.name in {".ruff_cache", ".pytest_cache", "__pycache__"}
            )
            or (path.name in CACHE_NAMES and not path.is_symlink())
            or any(part in CACHE_NAMES for part in parts)
            or (not path.is_symlink() and is_test_build_cache(name))
        )
        # `--directory` reports ignored directories as one entry, so the
        # directory name itself carries the regeneration classification.
        if path.is_dir() and not path.is_symlink() and path.name in CACHE_NAMES:
            allowed = True
        if (
            path.is_dir()
            and not path.is_symlink()
            and path.name in {".ruff_cache", ".pytest_cache", "__pycache__"}
        ):
            allowed = True
        allowed = allowed or any(Path(name).match(pattern) for pattern in BOOTSTRAP_LINKS)
        if stat.st_size == 0 and not path.is_symlink():
            continue
        items.append(
            {
                "path": name,
                "size": stat.st_size,
                "mtime": stat.st_mtime,
                "regenerable": allowed,
                "recipe": "rebuild/reinstall cache" if allowed else None,
            }
        )
    return items


def claim_blockers(main: Path, root: Path, branch: str) -> list[str]:
    common = Path(git(main, "rev-parse", "--path-format=absolute", "--git-common-dir").strip())
    tasks = set(re.findall(r"T\d+", branch + "/" + root.name, re.IGNORECASE))
    tasks = {task.upper() for task in tasks}
    directory = common / "agent-coord"
    result = []
    for kind in ("claims", "scopes"):
        for path in (directory / kind).glob("*.json"):
            try:
                data = json.loads(path.read_text())
            except (OSError, ValueError) as exc:
                raise Unsafe(f"unreadable coordination record {path}") from exc
            task = str(data.get("task", path.stem)).upper()
            absolute_scope = any(
                Path(p).is_absolute() and (Path(p) == root or root in Path(p).parents)
                for p in data.get("paths", [])
            )
            if task not in tasks and not absolute_scope:
                continue
            # Retain even stale unreleased claims: idle collection requires release.
            result.append(f"{kind}: {task} ({data.get('owner', 'unknown')}); release explicitly")
    return result


def is_scratch_candidate(root: Path, main: Path, idle_hours: float = 2) -> bool:
    """True for an idle helper candidate which is safe to quarantine via ``wt remove``."""
    container = root.parent
    if root.is_symlink():
        return False
    if root.name.startswith("wc-cand-"):
        if container != main / "tmp":
            return False
        from tools.wt import scratch

        marker_alive = scratch.owner_marker_alive(root)
        if marker_alive is not None:
            return not marker_alive
        try:
            creator_pid = int(root.name.removeprefix("wc-cand-"))
        except ValueError:
            return False
        # worker_check owns its candidate until its finally block runs. The hourly collector
        # may quarantine an orphan only after two hours of filesystem inactivity.
        if creator_pid in (os.getpid(), os.getppid()):
            return True
        from tools.wt import hygiene

        cutoff = time.time() - idle_hours * 3600
        inactive = last_activity(root) <= cutoff
        return inactive and not hygiene.open_by_process(root)
    if container.parent != main / "tmp":
        return False
    if root.name == "candidate" and container.name.startswith(("preflight-", "merge-task-")):
        return time.time() - container.stat().st_mtime > 3600
    return False


def commit_is_published(root: Path, main: Path) -> bool:
    """A commit is published on the shared origin/main or origin/archive namespace."""
    head = git(root, "rev-parse", "HEAD").strip()
    refs = git(
        main,
        "for-each-ref",
        "--contains",
        head,
        "--format=%(refname)",
        "refs/remotes/origin/main",
        "refs/remotes/origin/archive/",
    ).splitlines()
    return bool(refs)


def unmerged_commits_are_published_merges(root: Path, main: Path, reference: str) -> bool:
    """Allow only merge commits whose added parents and result are already on origin/main."""
    commits = git(root, "rev-list", f"{reference}..HEAD").splitlines()
    if not commits:
        return True
    for commit in commits:
        parents = git(root, "rev-list", "--parents", "-n", "1", commit).split()[1:]
        if len(parents) != 2:
            return False
        expected = subprocess.run(
            ["git", "merge-tree", "--write-tree", *parents],
            cwd=root,
            capture_output=True,
            text=True,
            timeout=60,
        )
        if expected.returncode or not expected.stdout.splitlines():
            return False
        if git(root, "rev-parse", f"{commit}^{{tree}}").strip() != expected.stdout.splitlines()[0]:
            return False
        for parent in parents[1:]:
            check = subprocess.run(
                ["git", "merge-base", "--is-ancestor", parent, reference],
                cwd=main,
                capture_output=True,
                timeout=60,
            )
            if check.returncode:
                return False
    return True


def scratch_head_is_published(root: Path, main: Path) -> bool:
    """Scratch HEAD may also be disposable when it is an unreferenced merge candidate."""
    head = git(root, "rev-parse", "HEAD").strip()
    published = bool(git(main, "for-each-ref", "--contains", head, "--format=%(refname)").strip())
    subject = git(root, "log", "-1", "--format=%s").strip().lower()
    parents = git(root, "rev-list", "--parents", "-n", "1", "HEAD").split()
    merge_candidate = subject.startswith(("merge", "preflight", "candidate"))
    return published or merge_candidate or len(parents) >= 3


CACHE_DIR_PARTS = frozenset({".ruff_cache", ".pytest_cache", "__pycache__"})


def dirty_paths(root: Path) -> list[str]:
    """Status entries that block removal. Untracked regenerable caches (ruff/pytest caches,
    __pycache__, the node_modules/.venv bootstrap links a landing leaves) are not work: every
    finished landing was refused on them until removed by hand (2026-10-30, I-T3885)."""
    regenerable = {item["path"] for item in inventory(root) if item["regenerable"]}
    dirty = []
    for line in git(root, "status", "--porcelain", "--untracked-files=all").splitlines():
        name = line[3:].rstrip("/")
        if line.startswith("??") and (
            name in regenerable
            or any(name.startswith(r + "/") for r in regenerable)
            or bool(CACHE_DIR_PARTS.intersection(Path(name).parts))
        ):
            continue
        dirty.append(line)
    return dirty


def inspect(root: Path, scratch: bool = False) -> tuple[Path, list[dict], list[str]]:
    root = root.resolve()
    if core.worktree_root(root) != root:
        raise Unsafe("provide the exact worktree root")
    main = core.main_root(root)
    if root == main:
        raise Unsafe("cannot remove primary checkout")
    record = next((r for r in registered(main) if Path(r["worktree"]).resolve() == root), None)
    if not record:
        raise Unsafe("worktree is not registered")
    problems = []
    if "locked" in record:
        problems.append("worktree locked; only its owner may unlock it")
    scratch_mode = scratch and is_scratch_candidate(root, main)
    if scratch and not scratch_mode:
        problems.append("--scratch applies only to preflight/merge-task candidates over 1 hour old")
    if not scratch_mode and dirty_paths(root):
        problems.append("uncommitted/untracked changes")
    # Cached remote must exist: never silently substitute local main.
    reference = git(main, "rev-parse", "--verify", "origin/main").strip()
    head = git(root, "rev-parse", "HEAD").strip()
    test = subprocess.run(
        ["git", "merge-base", "--is-ancestor", head, reference],
        cwd=main,
        capture_output=True,
        timeout=60,
    )
    if test.returncode and scratch_mode:
        if not scratch_head_is_published(root, main):
            problems.append("scratch HEAD is in no branch and is not a merge commit")
    elif (
        test.returncode
        and not commit_is_published(root, main)
        and not unmerged_commits_are_published_merges(root, main, reference)
    ):
        problems.append("commits not merged or archived in a published ref (fetch before retrying)")
    problems.extend(claim_blockers(main, root, record.get("branch", "")))
    try:
        active = processes(root)
        if active:
            problems.append("process cwd inside worktree: " + ", ".join(active))
        if scratch_mode and root.name.startswith("wc-cand-"):
            from tools.wt import hygiene

            if hygiene.open_by_process(root):
                problems.append("process has cwd or an open file under worker candidate")
    except Unsafe as exc:
        problems.append(str(exc))
    items = inventory(root)
    return main, items, problems


def print_inventory(items: list[dict]) -> None:
    for item in items:
        stamp = datetime.fromtimestamp(item["mtime"], UTC).isoformat()
        print(
            f"{item['path']} size={item['size']} mtime={stamp} "
            f"recipe={item['recipe'] or 'unknown; preserve'}"
        )


def manifest_path(root: Path) -> Path:
    return root.with_name(root.name + ".manifest.json")


def remove(
    root: Path, *, force_i_checked: bool = False, ttl_hours: float = 72, scratch: bool = False
) -> Path:
    root = root.resolve()
    if scratch and ttl_hours == 72:
        ttl_hours = 6
    main, items, problems = inspect(root, scratch)
    valuable = [item for item in items if not item["regenerable"]]
    if valuable and not force_i_checked:
        problems.append("ignored non-empty data; inspect inventory before --force-i-checked")
    if problems:
        # Keep the actionable refusal ahead of inventory noise for callers and humans.
        print("refused: " + "; ".join(problems), file=sys.stderr)
        print_inventory(items)
        raise Unsafe("; ".join(problems))
    print_inventory(items)
    tmp = main / "tmp"
    if tmp.is_symlink():
        raise Unsafe("tmp directory must be local, not a symlink")
    tmp.mkdir(exist_ok=True)
    if not tmp.is_dir():
        raise Unsafe("tmp path must be a directory")
    trash = tmp / "wt-trash"
    if trash.is_symlink():
        raise Unsafe("trash directory must be local, not a symlink")
    trash.mkdir(exist_ok=True)
    if not trash.is_dir():
        raise Unsafe("trash path must be a directory")
    stripped = strip_on_entry(root)
    branch = checked_out_branch(root)
    now = time.time()
    destination = trash / (root.name + "-" + datetime.now(UTC).strftime("%Y%m%dT%H%M%S%fZ"))
    manifest = {
        "original_path": str(root),
        "destination": str(destination),
        "moved_at": now,
        "ttl_hours": ttl_hours,
        "head": git(root, "rev-parse", "HEAD").strip(),
        "inventory": items,
        "reviewed_ignored": bool(valuable),
        "stripped": stripped,
        "branch": branch,
    }
    # Write recovery metadata before mutation. A failed move leaves a harmless sidecar.
    manifest_path(destination).write_text(json.dumps(manifest, indent=2) + "\n")
    git(main, "worktree", "move", str(root), str(destination))
    git(main, "worktree", "repair", str(destination))
    print(f"quarantined {destination}; TTL {ttl_hours} hours")
    if branch and not scratch:
        print(retire_branch(main, destination, branch, manifest["head"]))
    return destination


PROTECTED_BRANCH_PREFIXES = ("release/", "keep/")


def checked_out_branch(root: Path) -> str:
    """Branch the worktree has checked out, "" when detached."""
    done = subprocess.run(
        ["git", "symbolic-ref", "-q", "--short", "HEAD"],
        cwd=root,
        capture_output=True,
        text=True,
        timeout=30,
    )
    return done.stdout.strip() if done.returncode == 0 else ""


def retire_branch(main: Path, destination: Path, branch: str, head: str) -> str:
    """T3514 (always delete merged): free the branch of a quarantined tree and delete it if merged.

    The trash tree keeps the branch checked out for the 72 h TTL, which blocks `git branch -d`.
    Detach it (the manifest records the branch and its tip), then delete the branch locally and on
    origin when its tip is an ancestor of origin/main. `remove` already refused unmerged commits,
    live claims and dirty trees, so this repeats only the ancestry check. Never raises."""
    try:
        if branch in ("main", "master") or branch.startswith(PROTECTED_BRANCH_PREFIXES):
            return f"branch {branch} kept (protected)"
        git(destination, "checkout", "-q", "--detach")
        merged = subprocess.run(
            ["git", "merge-base", "--is-ancestor", head, "origin/main"],
            cwd=main,
            capture_output=True,
            timeout=60,
        )
        tip = git(main, "rev-parse", "--verify", f"refs/heads/{branch}").strip()
        if merged.returncode or tip != head:
            return f"branch {branch} detached and kept (not merged into origin/main)"
        git(main, "update-ref", "-d", f"refs/heads/{branch}", tip)
        note = f"branch {branch} ({tip[:12]}) deleted locally"
        remote = subprocess.run(
            ["git", "remote", "get-url", "origin"], cwd=main, capture_output=True, timeout=30
        )
        if remote.returncode == 0:
            from tools.wt import prune_remote

            note += "; " + prune_remote.delete_landed_source(main, branch, tip)
        return note
    except (Unsafe, OSError, subprocess.SubprocessError) as error:
        return f"branch {branch} kept: {error}"


TRASH_CAP_GB = 100.0
TRASH_CAP_MIN_AGE_HOURS = 24.0


def strip_on_entry(root: Path) -> list[dict]:
    """T2419: delete rebuildable dirs (target, build, node_modules, .venv, caches) of a tree
    entering the trash; source, uncommitted edits and other data stay. Returns what went."""
    from tools.wt import hygiene

    stripped = []
    for path in hygiene._rebuildable_dirs(root):
        size = hygiene.tree_bytes(path)
        _remove_cache(path)
        stripped.append({"path": str(path.relative_to(root)), "bytes": size})
    return stripped


def _trash_entry_size(root: Path) -> int:
    from tools.wt import hygiene

    return hygiene.tree_bytes(root)


def enforce_trash_cap(
    main: Path, trash: Path, *, cap_gb: float, prune: bool, now: float | None = None
) -> int:
    """T2419: keep tmp/wt-trash under cap_gb. Strips rebuildables first, then removes the
    oldest entries (older than 24 h, same safety checks, no valuable ignored data)."""
    now = time.time() if now is None else now
    entries = []
    for record in registered(main):
        root = Path(record["worktree"]).resolve()
        if root.parent != trash:
            continue
        try:
            manifest = json.loads(manifest_path(root).read_text())
            moved = manifest["moved_at"]
        except (OSError, ValueError, KeyError):
            continue
        entries.append((moved, root, manifest))
    sizes = {root: _trash_entry_size(root) for _, root, _ in entries}
    total = sum(sizes.values())
    cap = cap_gb * 1e9
    print(f"trash size {total / 1e9:.1f} GB cap {cap_gb:g} GB entries={len(entries)}")
    if total <= cap:
        return 0
    removed = 0
    for moved, root, manifest in sorted(
        entries, key=lambda entry: (entry[0], not bool(entry[2].get("stripped")))
    ):
        scratch_origin = Path(manifest.get("original_path", ""))
        scratch_entry = any(
            part.startswith(SCRATCH_PATH_PARTS) or part == "sup-verify"
            for part in scratch_origin.parts
        )
        if total <= cap and not (prune and scratch_entry):
            break
        original = Path(manifest.get("original_path", ""))
        scratch_path = any(
            part.startswith(SCRATCH_PATH_PARTS) or part == "sup-verify" for part in original.parts
        )
        age_hours = (now - moved) / 3600
        ttl = 6 if scratch_path else TRASH_CAP_MIN_AGE_HOURS
        if age_hours < ttl:
            continue
        try:
            if prune:
                for item in strip_on_entry(root):
                    total -= item["bytes"]
                    sizes[root] -= item["bytes"]
                # Entries recorded as stripped at quarantine are disposable once age and
                # process/claim checks pass; avoid treating ordinary source as cache.
                if scratch_entry:
                    if processes(root) or claim_blockers(main, root, ""):
                        print(f"cap retain {root}: active process or claim")
                        continue
                    git(main, "worktree", "remove", str(root))
                    manifest_path(root).unlink()
                    total -= sizes[root]
                    removed += 1
                    print(f"cap pruned {root} {sizes[root]}")
                    continue
            if total <= cap:
                break
            _, items, problems = inspect(root)
            if root.parent == trash:
                # Quarantine trees are immutable recovery points. Their per-entry
                # process/claim and manifest checks below govern collection.
                problems = [
                    p for p in problems if not p.startswith("uncommitted/untracked changes")
                ]
            # Stripped clean entries are safe to expire; retained worktrees still use normal checks.
            if problems or any(not item["regenerable"] for item in items):
                print(f"cap retain {root}: {'; '.join(problems) or 'valuable ignored data'}")
            elif prune:
                git(main, "worktree", "remove", str(root))
                manifest_path(root).unlink()
                total -= sizes[root]
                removed += 1
                print(f"cap pruned {root} {sizes[root]}")
            else:
                print(f"cap would prune {root} {sizes[root]}")
                total -= sizes[root]
        except (Unsafe, OSError, ValueError, KeyError) as exc:
            print(f"cap retain {root}: {exc}")
    return removed


def last_activity(root: Path) -> float:
    latest = float(git(root, "log", "-1", "--format=%ct").strip())
    for parent, dirs, files in os.walk(root, followlinks=False):
        dirs[:] = [d for d in dirs if d != ".git"]
        for name in [*dirs, *files]:
            latest = max(latest, (Path(parent) / name).lstat().st_mtime)
    return max(latest, root.stat().st_mtime)


def _tree_inventory(path: Path) -> tuple[int, float]:
    """Return bytes and newest mtime without traversing symlinks."""
    root_stat = path.lstat()
    size = root_stat.st_size
    newest = root_stat.st_mtime
    if not stat.S_ISDIR(root_stat.st_mode):
        return size, newest
    stack = [path]
    while stack:
        current = stack.pop()
        with os.scandir(current) as entries:
            for entry in entries:
                item_stat = entry.stat(follow_symlinks=False)
                size += item_stat.st_size
                newest = max(newest, item_stat.st_mtime)
                if stat.S_ISDIR(item_stat.st_mode):
                    stack.append(Path(entry.path))
    return size, newest


def _remove_cache(path: Path) -> str:
    """Delete a cache directory, preserving the owner-provided XBE in build/."""
    if path.is_symlink() or not path.is_dir():
        raise Unsafe("cache path is not a real directory")
    preserve_xbe = path.name == "build" and (path / "default.xbe").is_file()
    if not preserve_xbe:
        shutil.rmtree(path)
        return "deleted"
    for entry in os.scandir(path):
        child = Path(entry.path)
        if child.name == "default.xbe":
            continue
        if entry.is_dir(follow_symlinks=False):
            shutil.rmtree(child)
        else:
            child.unlink()
    return "cleared build outputs; preserved default.xbe"


def _cache_candidates(main: Path) -> list[tuple[Path, str]]:
    candidates = []
    for record in registered(main):
        raw_root = Path(record["worktree"])
        if raw_root.is_symlink():
            continue
        root = raw_root.resolve()
        if root.exists() and root.parent == main / "tmp" / "wt-trash":
            continue
        for name in ("target", "build"):
            candidates.append((root / name, record.get("branch", "")))
    return candidates


def _cache_reason(
    main: Path, root: Path, branch: str, cache: Path, cutoff: float
) -> tuple[str, int, float]:
    if cache.is_symlink() or not cache.exists() or not cache.is_dir():
        raise Unsafe("cache path is missing, linked, or not a directory")
    size, newest = _tree_inventory(cache)
    age = time.time() - newest
    if age < cutoff:
        return f"active mtime ({age / 3600:.1f}/{cutoff / 3600:g} hours)", size, newest
    blockers = claim_blockers(main, root, branch)
    if blockers:
        return "; ".join(blockers), size, newest
    active = processes(cache)
    if active:
        return "process cwd inside cache: " + ", ".join(active), size, newest
    return "eligible", size, newest


def _trash_paths(main: Path) -> Path | None:
    """Validate tmp/wt-trash with lstat before any path operation can follow it."""
    tmp = main / "tmp"
    try:
        tmp_stat = tmp.lstat()
    except FileNotFoundError:
        return None
    if not stat.S_ISDIR(tmp_stat.st_mode):
        raise Unsafe("tmp directory must be local and not a symlink")
    trash = tmp / "wt-trash"
    try:
        trash_stat = trash.lstat()
    except FileNotFoundError:
        return None
    if not stat.S_ISDIR(trash_stat.st_mode):
        raise Unsafe("trash directory must be local and not a symlink")
    return trash


def _systemd_escape(value: str) -> str:
    return value.replace("\\", "\\\\").replace("%", "%%").replace(" ", "\\x20")


def _systemd_quote(value: str) -> str:
    escaped = value.replace("\\", "\\\\").replace('"', '\\"').replace("%", "%%")
    return f'"{escaped}"'


def install_gc_timer(root: Path) -> Path:
    """Install and start an hourly per-user systemd timer with an append-only inventory log."""
    if os.name != "posix" or shutil.which("systemctl") is None:
        raise Unsafe("gc-install requires systemd and systemctl on PATH")
    main = core.main_root(core.worktree_root(root))
    config = Path(os.environ.get("XDG_CONFIG_HOME", Path.home() / ".config"))
    unit_dir = config / "systemd" / "user"
    state = Path(os.environ.get("XDG_STATE_HOME", Path.home() / ".local" / "state"))
    state_dir = state / "tsfp-recomp"
    if unit_dir.is_symlink() or state_dir.is_symlink():
        raise Unsafe("refusing symlinked user unit or state directory")
    unit_dir.mkdir(parents=True, exist_ok=True)
    state_dir.mkdir(parents=True, exist_ok=True)
    log = state_dir / "wt-gc.log"
    if log.is_symlink():
        raise Unsafe(f"refusing symlinked inventory log: {log}")
    log.touch(exist_ok=True)

    marker = "# Managed by python3 -m tools.wt gc-install (T2342)."
    workdir = _systemd_escape(str(main))
    log_path = _systemd_escape(str(log))
    python_executable = shutil.which("python3")
    if python_executable is None:
        raise Unsafe("python3 is required to install the timer")
    python = _systemd_quote(python_executable)
    service = "\n".join(
        [
            marker,
            "[Unit]",
            "Description=tsfp-recomp safe worktree and build-cache housekeeping",
            "[Service]",
            "Type=oneshot",
            f"WorkingDirectory={workdir}",
            f"ExecStart={python} -m tools.wt gc --prune --prune-caches "
            "--idle-hours 2 --ttl-hours 72 --cache-idle-hours 2",
            f"StandardOutput=append:{log_path}",
            f"StandardError=append:{log_path}",
            "",
        ]
    )
    timer = "\n".join(
        [
            marker,
            "[Unit]",
            "Description=Hourly tsfp-recomp housekeeping",
            "[Timer]",
            "OnCalendar=hourly",
            "Persistent=true",
            "Unit=tsfp-wt-gc.service",
            "[Install]",
            "WantedBy=timers.target",
            "",
        ]
    )
    paths = {
        unit_dir / "tsfp-wt-gc.service": service,
        unit_dir / "tsfp-wt-gc.timer": timer,
    }
    for path, content in paths.items():
        if path.is_symlink():
            raise Unsafe(f"refusing symlinked user unit: {path}")
        if path.exists() and marker not in path.read_text():
            raise Unsafe(f"refusing to overwrite an unmanaged user unit: {path}")
        path.write_text(content)
    try:
        for command in (
            ["systemctl", "--user", "daemon-reload"],
            ["systemctl", "--user", "enable", "--now", "tsfp-wt-gc.timer"],
        ):
            subprocess.run(command, check=True, capture_output=True, text=True, timeout=30)
    except (OSError, subprocess.SubprocessError) as exc:
        raise Unsafe(f"could not enable the user timer: {exc}") from exc
    print(f"enabled hourly tsfp-wt-gc.timer; inventory log: {log}")
    return log


def gc(
    main: Path,
    *,
    idle_hours: float = 24,
    ttl_hours: float = 72,
    prune: bool = False,
    cache_idle_hours: float = 2,
    prune_caches: bool = False,
    trash_cap_gb: float = TRASH_CAP_GB,
) -> int:
    main = core.main_root(core.worktree_root(main))
    trash = _trash_paths(main)
    now = time.time()
    print(
        f"gc started {datetime.now(UTC).isoformat()} idle-hours={idle_hours:g} "
        f"ttl-hours={ttl_hours:g} cache-idle-hours={cache_idle_hours:g} "
        f"prune={prune} prune-caches={prune_caches}"
    )
    for record in registered(main):
        root = Path(record["worktree"]).resolve()
        if root == main:
            continue
        if trash is None or root.parent != trash:
            pass
        else:
            # Worktree-level cache pruning enforces its own claim/process checks below.
            # Keep the tree-level idle scan from treating cache contents as changes.
            pass
        try:
            _, items, problems = inspect(root)
            if trash is not None and root.parent == trash:
                data = json.loads(manifest_path(root).read_text())
                if data["destination"] != str(root):
                    raise Unsafe("manifest destination mismatch")
                age = (now - data["moved_at"]) / 3600
                original = Path(data.get("original_path", ""))
                is_scratch = any(
                    part.startswith(SCRATCH_PATH_PARTS) or part == "sup-verify"
                    for part in original.parts
                )
                ttl = 6 if is_scratch else max(ttl_hours, data["ttl_hours"])
                if age < ttl:
                    problems.append(f"TTL not reached ({age:.1f}/{ttl:g} hours)")
                if any(not item["regenerable"] for item in items):
                    problems.append("ignored valuable data still present")
                if problems:
                    print(f"retain {root}: {'; '.join(problems)}")
                elif prune:
                    # No force: Git must also confirm there are no local changes.
                    git(main, "worktree", "remove", str(root))
                    manifest_path(root).unlink()
                    print(f"pruned {root}")
                else:
                    print(f"would prune {root}")
            elif not problems and now - last_activity(root) >= idle_hours * 3600:
                print(f"idle {root}; use tools.wt remove after inventory review")
        except (Unsafe, OSError, ValueError, KeyError) as exc:
            print(f"retain {root}: {exc}")
    if trash is not None and trash_cap_gb > 0:
        enforce_trash_cap(main, trash, cap_gb=trash_cap_gb, prune=prune)
    cutoff = cache_idle_hours * 3600
    for cache, branch in _cache_candidates(main):
        if not cache.exists() and not cache.is_symlink():
            continue
        root = cache.parent
        try:
            reason, size, newest = _cache_reason(main, root, branch, cache, cutoff)
            stamp = datetime.fromtimestamp(newest, UTC).isoformat()
            if reason != "eligible":
                print(f"cache retain {cache} size={size} mtime={stamp}: {reason}")
                continue
            if prune_caches:
                # Recheck age, claims, process cwd, and path type immediately before deletion.
                reason, size, newest = _cache_reason(main, root, branch, cache, cutoff)
                if reason != "eligible":
                    print(f"cache retain {cache} size={size}: {reason}")
                    continue
                result = _remove_cache(cache)
                print(f"cache {result} {cache} size={size} mtime={stamp}")
            else:
                print(f"cache would-delete {cache} size={size} mtime={stamp}")
        except (Unsafe, OSError, ValueError) as exc:
            print(f"cache retain {cache}: {exc}")
    return 0


def retire(root: Path) -> bool:
    """Best-effort tooling cleanup: preserve failures and report the recovery path."""
    try:
        remove(root)
        return True
    except (Unsafe, OSError, SystemExit) as exc:
        print(f"worktree retained at {root}: {exc}")
        return False
