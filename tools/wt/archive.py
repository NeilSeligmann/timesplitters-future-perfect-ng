"""T2438 archive-then-delete hygiene: rescue everything that cannot be regenerated, verify, delete.

Owner goal: never lose data, but save space. For each idle worktree (or orphan dir under
tmp/wt, tmp/wt-trash, .claude/worktrees):

1. HEAD is pinned by the local ref ``refs/archive/<name>`` (never pushed: no policy allows
   pushing archive refs to origin, so ``push_refs`` stays off by default).
2. Tracked edits and untracked (non-ignored) files become one binary patch
   (``git diff --binary HEAD`` over a throwaway index, the tree's own index is untouched).
3. Ignored files that are not regenerable go into a content-addressed, zstd-compressed store
   ``tmp/owner/archive/objects/<sha[:2]>/<sha>.zst`` deduplicated across all trees. Rebuildable
   dirs (``hygiene.REBUILDABLE``: target, build, .venv, node_modules, caches) are skipped and
   only listed. Symlinks are recorded as links, never followed.
4. A manifest ``tmp/owner/archive/items/<name>.json`` describes the item.
5. Every object is re-read and hash-checked, the patch is recomputed and compared, liveness is
   re-checked, then the tree is deleted (no trash TTL) and ``git worktree prune`` runs.

``restore`` recreates the tree from ref + patch + objects. Guards fail closed: an incomplete
process scan, a live claim, recent activity, a lock, or anything under tmp/owner retains a tree.
"""

from __future__ import annotations

import hashlib
import json
import os
import re
import shutil
import stat
import subprocess
import sys
import tempfile
import time
from datetime import UTC, datetime
from pathlib import Path

from tools.wt import hygiene, safe_prune, safety

IDLE_HOURS = 24.0
ALARM_IDLE_HOURS = 6.0
CLAIM_EXPIRE_HOURS = 6.0
ALARM_FREE_GB = 150.0
ALARM_MESSAGE_EVERY_S = 6 * 3600
ORPHAN_PARENTS = ("tmp/wt", "tmp/wt-trash", ".claude/worktrees")
CHUNK = 1 << 20


class ArchiveError(RuntimeError):
    """Archiving or verification failed; the tree is retained."""


# ---- paths -------------------------------------------------------------------------------


def store(main: Path) -> Path:
    return main / "tmp" / "owner" / "archive"


def object_path(main: Path, digest: str) -> Path:
    return store(main) / "objects" / digest[:2] / f"{digest}.zst"


def item_manifest(main: Path, name: str) -> Path:
    return store(main) / "items" / f"{name}.json"


def safe_name(text: str) -> str:
    name = re.sub(r"[^A-Za-z0-9._-]+", "-", text).strip(".-") or "tree"
    return name.replace("..", "-")


# ---- content-addressed store ---------------------------------------------------------------


def _hash_file(path: Path) -> str:
    digest = hashlib.sha256()
    with open(path, "rb") as handle:
        for block in iter(lambda: handle.read(CHUNK), b""):
            digest.update(block)
    return digest.hexdigest()


def _hash_object(path: Path) -> str:
    """sha256 of the decompressed content of a stored object."""
    digest = hashlib.sha256()
    with subprocess.Popen(["zstd", "-dcq", str(path)], stdout=subprocess.PIPE) as proc:
        assert proc.stdout is not None
        for block in iter(lambda: proc.stdout.read(CHUNK), b""):
            digest.update(block)
    if proc.returncode:
        raise ArchiveError(f"zstd could not read {path}")
    return digest.hexdigest()


def put_file(main: Path, source: Path) -> str:
    """Store one regular file; identical content is stored once. Returns its sha256."""
    digest = _hash_file(source)
    target = object_path(main, digest)
    if target.is_file():
        return digest
    target.parent.mkdir(parents=True, exist_ok=True)
    fd, temp = tempfile.mkstemp(dir=target.parent, prefix=".put-")
    os.close(fd)
    try:
        subprocess.run(
            ["zstd", "-q", "-f", "-T0", "-3", str(source), "-o", temp],
            check=True,
            capture_output=True,
            timeout=3600,
        )
        if _hash_object(Path(temp)) != digest:
            raise ArchiveError(f"{source} changed while archiving")
        os.replace(temp, target)
    finally:
        if os.path.exists(temp):
            os.unlink(temp)
    return digest


def put_bytes(main: Path, data: bytes) -> str:
    with tempfile.NamedTemporaryFile(dir=store(main), prefix=".blob-", delete=False) as handle:
        handle.write(data)
    try:
        return put_file(main, Path(handle.name))
    finally:
        os.unlink(handle.name)


def get_bytes(main: Path, digest: str) -> bytes:
    data = subprocess.run(
        ["zstd", "-dcq", str(object_path(main, digest))], check=True, capture_output=True
    ).stdout
    if hashlib.sha256(data).hexdigest() != digest:
        raise ArchiveError(f"object {digest} is corrupt")
    return data


def extract_to(main: Path, digest: str, dest: Path) -> None:
    dest.parent.mkdir(parents=True, exist_ok=True)
    with open(dest, "wb") as out:
        subprocess.run(["zstd", "-dcq", str(object_path(main, digest))], check=True, stdout=out)
    if _hash_file(dest) != digest:
        raise ArchiveError(f"restored {dest} does not match {digest}")


# ---- inventory -----------------------------------------------------------------------------


def _record_entry(root: Path, path: Path, files: list, links: list, skipped: list) -> None:
    """Walk one ignored path: rebuildable dirs are skipped, symlinks recorded, files kept."""
    rel = str(path.relative_to(root))
    if path.is_symlink():
        links.append({"path": rel, "target": os.readlink(path)})
        return
    if path.is_dir():
        if path.name in hygiene.REBUILDABLE:
            skipped.append({"path": rel, "bytes": hygiene.tree_bytes(path)})
            return
        for parent, dirs, names in os.walk(path, followlinks=False):
            here = Path(parent)
            keep = []
            for name in dirs:
                child = here / name
                if child.is_symlink():
                    links.append(
                        {"path": str(child.relative_to(root)), "target": os.readlink(child)}
                    )
                elif name in hygiene.REBUILDABLE:
                    skipped.append(
                        {"path": str(child.relative_to(root)), "bytes": hygiene.tree_bytes(child)}
                    )
                else:
                    keep.append(name)
            dirs[:] = keep
            for name in names:
                _record_entry(root, here / name, files, links, skipped)
        return
    info = path.lstat()
    if stat.S_ISREG(info.st_mode):
        files.append(
            {
                "path": rel,
                "size": info.st_size,
                "mode": stat.S_IMODE(info.st_mode),
                "mtime": info.st_mtime,
            }
        )


def ignored_inventory(root: Path, is_git: bool) -> tuple[list, list, list]:
    """(files, symlinks, skipped regenerable dirs) for everything git ignores (or, for a
    non-git dir, everything)."""
    files: list = []
    links: list = []
    skipped: list = []
    if is_git:
        out = safety.git(
            root, "ls-files", "--others", "--ignored", "--exclude-standard", "--directory", "-z"
        )
        entries = [root / name.rstrip("/") for name in filter(None, out.split("\0"))]
    else:
        entries = sorted(root.iterdir())
    for path in entries:
        if any(part in hygiene.REBUILDABLE for part in path.relative_to(root).parts[:-1]):
            continue
        _record_entry(root, path, files, links, skipped)
    return files, links, skipped


def worktree_patch(root: Path) -> bytes:
    """Binary diff of tracked edits plus untracked non-ignored files against HEAD, made with
    a throwaway copy of the index so the tree itself is not modified."""
    index = Path(
        safety.git(root, "rev-parse", "--path-format=absolute", "--git-path", "index").strip()
    )
    with tempfile.TemporaryDirectory(prefix="wt-archive-index-") as scratch:
        temp_index = Path(scratch) / "index"
        if index.exists():
            shutil.copy2(index, temp_index)
        env = {**os.environ, "GIT_INDEX_FILE": str(temp_index)}
        subprocess.run(
            ["git", "add", "-A"], cwd=root, env=env, check=True, capture_output=True, timeout=600
        )
        return subprocess.run(
            ["git", "diff", "--cached", "--binary", "--no-ext-diff", "--no-renames", "HEAD"],
            cwd=root,
            env=env,
            check=True,
            capture_output=True,
            timeout=600,
        ).stdout


# ---- guards --------------------------------------------------------------------------------


def open_files(root: Path, proc: Path = Path("/proc")) -> list[str]:
    """PIDs with an open fd under root. Fails closed when a process cannot be inspected."""
    if not (proc / "self/fd").exists():
        raise safety.Unsafe("/proc fd scan unavailable")
    prefix = str(root) + os.sep
    found = []
    for entry in proc.iterdir():
        if entry.name.isdecimal() and any(
            t == str(root) or t.startswith(prefix) for t in hygiene.fd_targets(entry)
        ):
            found.append(entry.name)
    return found


def _admin_newest(root: Path) -> float:
    """Newest mtime of the worktree's git admin files (HEAD, index, logs/HEAD)."""
    try:
        gitdir = Path(safety.git(root, "rev-parse", "--absolute-git-dir").strip())
    except safety.Unsafe:
        return 0.0
    newest = 0.0
    for name in ("HEAD", "index", "logs/HEAD"):
        try:
            newest = max(newest, (gitdir / name).lstat().st_mtime)
        except OSError:
            pass
    return newest


def retain_reasons(
    main: Path,
    root: Path,
    branch: str,
    idle_hours: float,
    *,
    locked: bool = False,
    is_git: bool = True,
) -> list[str]:
    """Why a tree must stay. Empty list means it may be archived and deleted."""
    reasons = []
    owner = (main / "tmp" / "owner").resolve()
    real = root.resolve()
    if real == main.resolve():
        reasons.append("primary checkout")
    if real == owner or owner in real.parents:
        reasons.append("under tmp/owner")
    if locked:
        reasons.append("worktree locked")
    try:
        reasons.extend(safety.claim_blockers(main, real, branch))
    except safety.Unsafe as exc:
        reasons.append(str(exc))
    cutoff = time.time() - idle_hours * 3600
    admin = _admin_newest(real) if is_git else 0.0
    if hygiene.newer_than(real, cutoff, hygiene.REBUILDABLE) or admin > cutoff:
        reasons.append(f"active within {idle_hours:g} h")
    try:
        if safety.processes(real):
            reasons.append("process cwd inside")
        if open_files(real):
            reasons.append("open files inside")
    except safety.Unsafe as exc:
        reasons.append(f"process scan incomplete: {exc}")
    return reasons


# ---- archive / verify / delete -------------------------------------------------------------


def _unique_name(main: Path, base: str) -> str:
    name = safe_name(base)
    if not item_manifest(main, name).exists():
        return name
    return f"{name}-{datetime.now(UTC).strftime('%Y%m%dT%H%M%SZ')}"


def archive_tree(
    main: Path, root: Path, *, branch: str = "", is_git: bool = True, push_refs: bool = False
) -> dict:
    """Rescue one tree into the store and write its manifest. Does not delete anything."""
    root = root.resolve()
    (store(main) / "items").mkdir(parents=True, exist_ok=True)
    name = _unique_name(main, root.name)
    manifest: dict = {
        "name": name,
        "kind": "worktree" if is_git else "dir",
        "original_path": str(root),
        "branch": branch,
        "archived_at": time.time(),
        "head": None,
        "archive_ref": None,
        "ref_pushed": False,
        "patch": None,
    }
    if is_git:
        head = safety.git(root, "rev-parse", "HEAD").strip()
        ref = f"refs/archive/{name}"
        safety.git(main, "update-ref", ref, head)
        manifest.update(head=head, archive_ref=ref)
        if push_refs:
            safety.git(main, "push", "origin", f"{ref}:{ref}", timeout=600)
            manifest["ref_pushed"] = True
        patch = worktree_patch(root)
        if patch:
            manifest["patch"] = {"sha": put_bytes(main, patch), "bytes": len(patch)}
    files, links, skipped = ignored_inventory(root, is_git)
    for item in files:
        item["sha"] = put_file(main, root / item["path"])
    manifest.update(files=files, symlinks=links, skipped_regenerable=skipped)
    sidecar = safety.manifest_path(root)
    if root.parent.name == "wt-trash" and sidecar.is_file():
        manifest["trash_manifest"] = json.loads(sidecar.read_text())
    item_manifest(main, name).write_text(json.dumps(manifest, indent=2) + "\n")
    return manifest


def verify(main: Path, manifest: dict, root: Path | None = None) -> None:
    """Re-read every object and hash-check it; check the ref; recompute the patch."""
    checked: set[str] = set()
    digests = [item["sha"] for item in manifest["files"]]
    if manifest.get("patch"):
        digests.append(manifest["patch"]["sha"])
    for digest in digests:
        if digest in checked:
            continue
        if _hash_object(object_path(main, digest)) != digest:
            raise ArchiveError(f"object {digest} failed verification")
        checked.add(digest)
    if manifest["kind"] == "worktree":
        ref_head = safety.git(main, "rev-parse", "--verify", manifest["archive_ref"]).strip()
        if ref_head != manifest["head"]:
            raise ArchiveError("archive ref does not point at the archived HEAD")
        safety.git(main, "cat-file", "-e", f"{ref_head}^{{commit}}")
        if root is not None:
            if safety.git(root, "rev-parse", "HEAD").strip() != manifest["head"]:
                raise ArchiveError("HEAD moved while archiving")
            patch = worktree_patch(root)
            expected = manifest["patch"]["sha"] if manifest.get("patch") else None
            actual = hashlib.sha256(patch).hexdigest() if patch else None
            if actual != expected:
                raise ArchiveError("working tree changed while archiving")
    if root is not None:
        for item in manifest["files"]:
            path = root / item["path"]
            if not path.is_file() or path.lstat().st_size != item["size"]:
                raise ArchiveError(f"ignored file {item['path']} changed while archiving")


def delete_tree(main: Path, root: Path, manifest: dict) -> None:
    shutil.rmtree(root)
    sidecar = safety.manifest_path(root)
    if sidecar.is_file():
        sidecar.unlink()
    safe_prune.prune(main)  # T3784: never drops another mount's live worktree
    manifest["deleted_at"] = time.time()
    item_manifest(main, manifest["name"]).write_text(json.dumps(manifest, indent=2) + "\n")


def candidates(main: Path) -> list[dict]:
    """Registered linked worktrees plus orphan dirs under the worktree parents."""
    found = []
    registered = set()
    for record in safety.registered(main):
        root = Path(record["worktree"])
        registered.add(root.resolve())
        if root.resolve() == main.resolve() or root.is_symlink() or not root.is_dir():
            continue
        found.append(
            {
                "root": root.resolve(),
                "branch": record.get("branch", "").removeprefix("refs/heads/"),
                "locked": "locked" in record,
                "git": True,
            }
        )
    for parent in ORPHAN_PARENTS:
        base = main / parent
        if base.is_symlink() or not base.is_dir():
            continue
        for child in sorted(base.iterdir()):
            if child.is_symlink() or not child.is_dir() or child.resolve() in registered:
                continue
            if any(child.resolve() in r.parents for r in registered):
                continue  # holds a registered worktree (for example tmp/wt/x/candidate)
            found.append({"root": child.resolve(), "branch": "", "locked": False, "git": False})
    return found


def process_one(
    main: Path, cand: dict, idle_hours: float, apply: bool, push_refs: bool = False
) -> tuple[str, int]:
    root = cand["root"]
    reasons = retain_reasons(
        main, root, cand["branch"], idle_hours, locked=cand["locked"], is_git=cand["git"]
    )
    if reasons:
        return "retain: " + "; ".join(reasons), 0
    size = hygiene.tree_bytes(root)
    if not apply:
        return "would archive and delete", size
    manifest = archive_tree(
        main, root, branch=cand["branch"], is_git=cand["git"], push_refs=push_refs
    )
    verify(main, manifest, root)
    # Liveness can change while archiving: check again right before deletion.
    reasons = retain_reasons(
        main, root, cand["branch"], idle_hours, locked=cand["locked"], is_git=cand["git"]
    )
    if reasons:
        return "archived, retained: " + "; ".join(reasons), 0
    delete_tree(main, root, manifest)
    return f"archived as {manifest['name']} and deleted", size


def run(
    main: Path,
    *,
    idle_hours: float = IDLE_HOURS,
    apply: bool = False,
    only: list[Path] | None = None,
    push_refs: bool = False,
) -> int:
    """One archive pass. Returns bytes freed (or that would be freed)."""
    if shutil.which("zstd") is None:
        raise safety.Unsafe("zstd is required (sudo apt-get install zstd)")
    wanted = {p.resolve() for p in only} if only else None
    freed = 0
    for cand in candidates(main):
        if wanted is not None and cand["root"] not in wanted:
            continue
        try:
            verdict, size = process_one(main, cand, idle_hours, apply, push_refs)
        except (safety.Unsafe, ArchiveError, OSError, subprocess.SubprocessError) as exc:
            verdict, size = f"retain: archive failed: {exc}", 0
        freed += size
        print(f"archive {cand['root']} {size}: {verdict}", flush=True)
    if apply:
        safe_prune.prune(main)  # T3784: never drops another mount's live worktree
    print(f"archive {'freed' if apply else 'would free'} {freed}", flush=True)
    return freed


# ---- restore -------------------------------------------------------------------------------


def restore(main: Path, name: str, dest: Path | None = None) -> Path:
    path = item_manifest(main, safe_name(name))
    if not path.is_file():
        raise ArchiveError(f"no archive item {name}")
    manifest = json.loads(path.read_text())
    dest = (dest or Path(manifest["original_path"])).resolve()
    if dest.exists():
        raise ArchiveError(f"{dest} exists; pass --path")
    if manifest["kind"] == "worktree":
        head = manifest["head"]
        branch = manifest.get("branch") or ""
        tip = subprocess.run(
            ["git", "rev-parse", "--verify", "-q", f"refs/heads/{branch}"],
            cwd=main,
            capture_output=True,
            text=True,
        ).stdout.strip()
        busy = any(r.get("branch") == f"refs/heads/{branch}" for r in safety.registered(main))
        if branch and tip == head and not busy:
            safety.git(main, "worktree", "add", str(dest), branch)
        else:
            safety.git(main, "worktree", "add", "--detach", str(dest), manifest["archive_ref"])
        if manifest.get("patch"):
            patch = get_bytes(main, manifest["patch"]["sha"])
            subprocess.run(
                ["git", "apply", "--binary", "--whitespace=nowarn", "-"],
                cwd=dest,
                input=patch,
                check=True,
                capture_output=True,
            )
    else:
        dest.mkdir(parents=True)
    for item in manifest["files"]:
        target = dest / item["path"]
        extract_to(main, item["sha"], target)
        os.chmod(target, item["mode"])
        os.utime(target, (item["mtime"], item["mtime"]))
    for link in manifest["symlinks"]:
        target = dest / link["path"]
        target.parent.mkdir(parents=True, exist_ok=True)
        if not target.is_symlink():
            os.symlink(link["target"], target)
    print(f"restored {manifest['name']} at {dest}")
    return dest


# ---- claims, alarm, hourly wiring ----------------------------------------------------------


def coord(main: Path, *args: str) -> subprocess.CompletedProcess:
    repo = Path(__file__).resolve().parents[2]
    env = {**os.environ, "PYTHONPATH": str(repo)}
    return subprocess.run(
        [sys.executable, "-m", "tools.agents.coord", *args],
        cwd=main,
        env=env,
        capture_output=True,
        text=True,
        timeout=120,
    )


def expired_claims(main: Path, hours: float = CLAIM_EXPIRE_HOURS) -> list[dict]:
    common = Path(
        safety.git(main, "rev-parse", "--path-format=absolute", "--git-common-dir").strip()
    )
    out = []
    for path in sorted((common / "agent-coord" / "claims").glob("*.json")):
        try:
            data = json.loads(path.read_text())
            silent = time.time() - path.stat().st_mtime
        except (OSError, ValueError):
            continue
        if silent > hours * 3600:
            out.append(
                {
                    "task": data.get("task", path.stem),
                    "owner": data.get("owner"),
                    "silent_h": round(silent / 3600, 1),
                }
            )
    return out


def expire_claims(main: Path, hours: float = CLAIM_EXPIRE_HOURS, apply: bool = False) -> list:
    """Release claims silent for more than `hours` through `coord reap` (logs a liberate
    event per claim, also reaps expired leases and scopes)."""
    stale = expired_claims(main, hours)
    for held in stale:
        print(f"claim {'expire' if apply else 'would expire'} {held}", flush=True)
    if apply and stale:
        result = coord(main, "--stale", str(int(hours * 3600)), "reap")
        print(result.stdout.strip() or result.stderr.strip(), flush=True)
    return stale


def alarm_floor() -> float:
    try:
        return float(os.environ.get("TSFP_ALARM_FREE_GB", ALARM_FREE_GB))
    except ValueError:
        return ALARM_FREE_GB


def apply_enabled(main: Path) -> bool:
    """Real deletion is opt-in: env TSFP_WT_ARCHIVE_APPLY=1 or tmp/wt-cache/archive-apply."""
    return (
        os.environ.get("TSFP_WT_ARCHIVE_APPLY") == "1"
        or (main / "tmp" / "wt-cache" / "archive-apply").is_file()
    )


def post_alarm(main: Path, text: str) -> bool:
    stamp = main / "tmp" / "wt-cache" / "low-disk-alarm"
    try:
        if time.time() - stamp.stat().st_mtime < ALARM_MESSAGE_EVERY_S:
            return False
    except OSError:
        pass
    coord(main, "say", "--owner", "wt-hygiene", "--to", "all", text)
    stamp.parent.mkdir(parents=True, exist_ok=True)
    stamp.touch()
    return True


def hourly(main: Path, free_gb: float | None = None) -> int:
    """Hygiene-loop step: claim expiry, archive pass, low-disk alarm."""
    apply = apply_enabled(main)
    free = hygiene.free_gb(main) if free_gb is None else free_gb
    alarm = free < alarm_floor()
    idle = ALARM_IDLE_HOURS if alarm else IDLE_HOURS
    expire_claims(main, CLAIM_EXPIRE_HOURS, apply)
    freed = run(main, idle_hours=idle, apply=apply)
    if alarm:
        post_alarm(
            main,
            f"LOW DISK: {free:.0f} GB free on /workspace (< {alarm_floor():g} GB); "
            f"wt-hygiene archive pass idle={idle:g}h apply={apply} "
            f"{'freed' if apply else 'would free'} {freed / 1e9:.1f} GB",
        )
    return freed
