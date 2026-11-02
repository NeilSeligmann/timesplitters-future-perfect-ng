"""Owned disposable merge/preflight trees; never an override for task retirement.

Creation records origin before bootstrap. Sealing records only known bootstrap links
and the generated environment, before any verification runs. Unknown data retains
the tree. Git removes reviewed trees without --force and does not follow symlinks.
"""

from __future__ import annotations

import fcntl
import hashlib
import json
import math
import os
import subprocess
import tempfile
import threading
import time
import uuid
from collections.abc import Iterator
from contextlib import contextmanager
from pathlib import Path

from tools.wt import core, safe_prune, safety

MARKER = ".tsfp-scratch"
MANIFEST = ".tsfp-scratch.json"
OWNER_MARKER = ".tsfp-scratch-owner.json"
ACTIVE_LOCK = ".tsfp-scratch-active.lock"
_ACTIVE_FDS: dict[Path, int] = {}
CACHE_NAMES = frozenset(
    {
        "target",
        "cargo-target",
        "__pycache__",
        ".pytest_cache",
        ".ruff_cache",
        ".mypy_cache",
        "build",
        "node_modules",
        ".venv",
        "venvs",
    }
)
KINDS = frozenset({"preflight", "merge-task"})
# T3807: the native library build of the test suite (tests/native_build.py reads it from here,
# tools never import tests); its directory and lock are run output, never reviewed data
NATIVE_TEST_BUILD = "tmp/native-test-build"
SCRATCH_TTL_HOURS = 6.0  # T2419: scratch is disposable; real worktrees keep 72 h


def process_identity(pid: int) -> str | None:
    """Read only the creator's start time, never foreign cwd links."""
    try:
        fields = Path(f"/proc/{pid}/stat").read_text().rsplit(")", 1)[1].split()
        return fields[19]  # field 22; fields starts with field 3 (state)
    except FileNotFoundError:
        return None
    except (OSError, IndexError) as exc:
        raise safety.Unsafe(f"cannot inspect scratch owner process {pid}") from exc


def boot_identity() -> str:
    return Path("/proc/sys/kernel/random/boot_id").read_text().strip()


def pid_namespace(pid: int) -> str | None:
    try:
        return os.readlink(f"/proc/{pid}/ns/pid")
    except FileNotFoundError:
        return None
    except OSError as exc:
        raise safety.Unsafe(f"cannot inspect scratch owner pid namespace {pid}") from exc


def write_owner_marker(candidate: Path, pid: int | None = None) -> None:
    """Record enough process identity to distinguish dead owners from recycled PIDs."""
    pid = os.getpid() if pid is None else pid
    start = process_identity(pid)
    namespace = pid_namespace(pid)
    if start is None or namespace is None:
        raise safety.Unsafe(f"cannot establish scratch owner identity for pid {pid}")
    candidate.mkdir(parents=True, exist_ok=True)
    (candidate / OWNER_MARKER).write_text(
        json.dumps(
            {
                "boot": boot_identity(),
                "pid": pid,
                "process_start": start,
                "pid_namespace": namespace,
            },
            sort_keys=True,
        )
        + "\n"
    )


def owner_marker_alive(candidate: Path) -> bool | None:
    """Return None for legacy candidates lacking identity; fail closed on malformed data."""
    path = candidate / OWNER_MARKER
    if not path.is_file():
        return None
    try:
        data = json.loads(path.read_text())
        pid = int(data["pid"])
        if data["boot"] != boot_identity():
            return False
        namespace = pid_namespace(pid)
        if namespace is None:
            return False
        if namespace != data["pid_namespace"]:
            return False
        return process_identity(pid) == str(data["process_start"])
    except (OSError, ValueError, KeyError, TypeError, json.JSONDecodeError) as exc:
        raise safety.Unsafe("invalid scratch owner marker") from exc


def owner_alive(data: dict) -> bool:
    if data["boot"] != boot_identity():
        return False
    pid = int(data["pid"])
    recorded_namespace = data.get("pid_namespace")
    if recorded_namespace is not None and pid_namespace(pid) != recorded_namespace:
        return False
    return process_identity(pid) == str(data["process_start"])


def refresh(candidate: Path, *, now: float | None = None) -> bool:
    """Refresh a live scratch container's timestamp for age-based host cleaners."""
    manifest = candidate.parent / MANIFEST
    if candidate.name != "candidate" or not manifest.is_file():
        return False
    stamp = None if now is None else (now, now)
    os.utime(manifest, stamp)
    return True


@contextmanager
def keepalive(candidate: Path, *, interval_s: float = 60.0) -> Iterator[None]:
    """Refresh an active scratch manifest while a long verification uses its worktree.

    Some host cleanup jobs only see filesystem activity and process cwd, not the creator PID.
    Updating the manifest keeps the candidate recent during scheduler waits between commands.
    """
    manifest = candidate.parent / MANIFEST
    if candidate.name != "candidate" or not manifest.is_file():
        yield
        return
    stop = threading.Event()

    def pulse() -> None:
        while not stop.wait(interval_s):
            try:
                if not refresh(candidate):
                    return
            except OSError:
                return

    refresh(candidate)
    worker = threading.Thread(target=pulse, name="scratch-keepalive", daemon=True)
    worker.start()
    try:
        yield
    finally:
        stop.set()
        worker.join()


def write_manifest(container: Path, data: dict) -> None:
    # Atomic replacement; an interrupted update leaves an unsealed retained tree.
    pending = container / (MANIFEST + ".new")
    pending.write_text(json.dumps(data, indent=2) + "\n")
    pending.replace(container / MANIFEST)


def create(primary: Path, kind: str, ref: str) -> Path:
    if kind not in KINDS:
        raise safety.Unsafe("unknown scratch producer")
    primary = core.main_root(core.worktree_root(primary))
    tmp = primary / "tmp"
    tmp.mkdir(exist_ok=True)
    if tmp.resolve() != tmp or tmp.is_symlink():
        raise safety.Unsafe("scratch tmp must be local, not a symlink")
    from tools.wt import hygiene

    hygiene.ensure_free(primary)  # T2419: refuse (after hygiene) when the disk is nearly full
    container = Path(tempfile.mkdtemp(prefix=kind + "-", dir=tmp))
    try:
        candidate = _create_in(container, primary, kind, ref)
        lock_path = container / ACTIVE_LOCK
        fd = os.open(lock_path, os.O_CREAT | os.O_RDWR | os.O_NOFOLLOW, 0o600)
        try:
            fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BaseException:
            os.close(fd)
            raise
        _ACTIVE_FDS[candidate] = fd
        return candidate
    except BaseException:
        # T2419: a failed creation must not leak its container (marker, manifest, worktree).
        _discard_partial(container, primary)
        raise


def _discard_partial(container: Path, primary: Path) -> None:
    candidate = container / "candidate"
    try:
        if candidate.exists():
            safety.git(primary, "worktree", "remove", "--force", str(candidate))
    except (safety.Unsafe, OSError):
        pass
    for name in (MARKER, MANIFEST, MANIFEST + ".new", ACTIVE_LOCK):
        (container / name).unlink(missing_ok=True)
    try:
        container.rmdir()
    except OSError:
        pass


@contextmanager
def owned(primary: Path, kind: str, ref: str) -> Iterator[Path]:
    """Create a scratch candidate and remove it on every exit path (T2419).

    Unsealed or dirty trees are still stripped of rebuildable caches so a refusal to
    remove (unknown data) never keeps gigabytes of build output. SIGKILL cannot run
    this; `gc` (see below) reaps those orphans within minutes.
    """
    candidate = create(primary, kind, ref)
    try:
        yield candidate
    finally:
        try:
            remove(candidate)
        except (safety.Unsafe, OSError, SystemExit) as exc:
            print(f"scratch retained at {candidate}: {exc}")
            strip_caches(candidate)


def strip_caches(candidate: Path) -> int:
    """Delete rebuildable directories of a dead scratch candidate; returns bytes freed."""
    from tools.wt import hygiene

    freed = 0
    try:
        for path in hygiene._rebuildable_dirs(candidate):
            size = hygiene.tree_bytes(path)
            safety._remove_cache(path)
            freed += size
    except (safety.Unsafe, OSError):
        pass
    return freed


def _create_in(container: Path, primary: Path, kind: str, ref: str) -> Path:
    candidate = container / "candidate"
    token = uuid.uuid4().hex
    (container / MARKER).write_text(token + "\n")
    data = {
        "version": 1,
        "token": token,
        "kind": kind,
        "main": str(primary),
        "container": str(container),
        "candidate": str(candidate),
        "common": safety.git(
            primary, "rev-parse", "--path-format=absolute", "--git-common-dir"
        ).strip(),
        "head": safety.git(primary, "rev-parse", "--verify", ref + "^{commit}").strip(),
        "created_at": time.time(),
        "pid": os.getpid(),
        "process_start": process_identity(os.getpid()),
        "pid_namespace": pid_namespace(os.getpid()),
        "boot": boot_identity(),
        "sealed": False,
        "links": {},
        "files": {},
    }
    write_manifest(container, data)
    # The full tracked tree can take longer than the default safety.git timeout
    # to check out in the large shared repository.
    safety.git(
        primary,
        "worktree",
        "add",
        "--detach",
        str(candidate),
        data["head"],
        timeout=600,
    )
    sparse = safety.git(
        candidate, "config", "--bool", "--default", "false", "core.sparseCheckout"
    ).strip()
    if sparse == "true":
        # Scratch candidates run repository tools that expect the full tracked tree.
        # A shared Git config can enable sparse checkout without the per-worktree
        # patterns needed by these disposable candidates, silently omitting inputs.
        safety.git(candidate, "sparse-checkout", "disable", timeout=600)
    data["gitdir"] = safety.git(candidate, "rev-parse", "--absolute-git-dir").strip()
    write_manifest(container, data)
    return candidate


def read_manifest(candidate: Path) -> dict:
    container = candidate.parent
    if candidate.name != "candidate" or candidate.resolve() != candidate:
        raise safety.Unsafe("scratch path must be an exact local candidate")
    for name in (MARKER, MANIFEST):
        path = container / name
        if path.is_symlink() or not path.is_file():
            raise safety.Unsafe("unmarked scratch; preserve legacy directories")
    try:
        data = json.loads((container / MANIFEST).read_text())
        main = Path(data["main"])
        valid = (
            data["version"] == 1
            and data["kind"] in KINDS
            and container.name.startswith(data["kind"] + "-")
            and container.parent == main / "tmp"
            and data["container"] == str(container)
            and data["candidate"] == str(candidate)
            and (container / MARKER).read_text().strip() == data["token"]
            and len(data["token"]) == 32
            and core.main_root(candidate) == main
            and safety.git(main, "rev-parse", "--path-format=absolute", "--git-common-dir").strip()
            == data["common"]
            and safety.git(candidate, "rev-parse", "--absolute-git-dir").strip() == data["gitdir"]
        )
        if not valid:
            raise safety.Unsafe("scratch provenance mismatch")
        if {p.name for p in container.iterdir()} not in (
            {MARKER, MANIFEST, "candidate"},
            {MARKER, MANIFEST, ACTIVE_LOCK, "candidate"},
        ):
            raise safety.Unsafe("unexpected scratch container data")
        return data
    except (ValueError, KeyError, TypeError) as exc:
        raise safety.Unsafe("invalid scratch manifest") from exc


def seal(candidate: Path, family: str = "codex") -> None:
    """Approve links created by bootstrap, never arbitrary ignored files."""
    data = read_manifest(candidate)
    if data["sealed"] or data["pid"] != os.getpid() or not owner_alive(data):
        raise safety.Unsafe("scratch can only be sealed once by its creator")
    main = Path(data["main"])
    links = {}
    for spec, target, source in core.plan(candidate, main, None):
        if source is None:
            continue
        pairs = (
            [(target / child.name, child) for child in source.iterdir()]
            if spec.kind == core.ENTRIES
            else [(target, source)]
        )
        for target, source in pairs:
            if target.is_symlink() and os.readlink(target) == str(source):
                links[str(target.relative_to(candidate))] = str(source)
    lock = candidate / "tmp/merge.lock"
    if lock.is_symlink() and os.readlink(lock) == str(main / "tmp/merge.lock"):
        links["tmp/merge.lock"] = str(main / "tmp/merge.lock")
    env = candidate / core.ENV_REL
    if env.is_file() and not env.is_symlink():
        content = env.read_bytes()
        if content != core.env_script(candidate, main, family).encode():
            raise safety.Unsafe("unexpected bootstrap environment")
        data["files"][core.ENV_REL] = hashlib.sha256(content).hexdigest()
    data["links"] = links
    data["sealed"] = True
    write_manifest(candidate.parent, data)


def inspect(candidate: Path, data: dict) -> None:
    if not data["sealed"]:
        raise safety.Unsafe("scratch bootstrap not sealed")
    main = Path(data["main"])
    record = next((r for r in safety.registered(main) if r["worktree"] == str(candidate)), None)
    if not record or "locked" in record or "detached" not in record:
        raise safety.Unsafe("scratch must be registered, unlocked and detached")
    if safety.git(candidate, "status", "--porcelain", "--untracked-files=all").strip():
        raise safety.Unsafe("uncommitted/untracked scratch changes")
    # Scratch commits may be unpublished, but must descend from the recorded base.
    safety.git(candidate, "merge-base", "--is-ancestor", data["head"], "HEAD")
    blockers = safety.claim_blockers(main, candidate, "")
    if blockers:
        raise safety.Unsafe("; ".join(blockers))
    names = safety.git(candidate, "ls-files", "--others", "--ignored", "--exclude-standard", "-z")
    for name in filter(None, names.split("\0")):
        path = candidate / name
        if path.is_symlink():
            if data["links"].get(name) == os.readlink(path):
                continue
        elif path.is_file():
            if name in data["files"]:
                if hashlib.sha256(path.read_bytes()).hexdigest() == data["files"][name]:
                    continue
            elif any(part in CACHE_NAMES for part in Path(name).parts[:-1]):
                continue
            elif Path(name).name == "Cargo.lock" and name != "Cargo.lock":
                continue  # T2419: crate lockfiles are regenerated by cargo builds
            elif is_run_output(candidate, name):
                continue  # T3807: known verify run output, see run_outputs
        raise safety.Unsafe(f"unreviewed ignored scratch data: {name}")


def run_outputs() -> frozenset[str]:
    """Files verification tooling always writes into the tree it runs in, named by their writers
    (rule 45). T3807: they retained 26 of 36 scratch trees on 2026-10-26; any other ignored file
    still retains the tree for inspection."""
    from tools.agents import suite_health
    from tools.wt import rustup_health

    return frozenset(
        {
            suite_health.RECORD.as_posix(),
            rustup_health.STATE_REL.as_posix(),
            NATIVE_TEST_BUILD + ".lock",
        }
    )


def is_run_output(root: Path, name: str) -> bool:
    """Recognize owned output paths through their producing tool's contract."""
    if name in run_outputs() or name.startswith(NATIVE_TEST_BUILD + "/"):
        return True
    from tools.play import configure_generators

    return configure_generators.owns_generated_build_output(root, name)


def _remove(candidate: Path, data: dict) -> None:
    if candidate == Path.cwd() or candidate in Path.cwd().parents:
        raise safety.Unsafe("process cwd inside scratch; run teardown from outside")
    inspect(candidate, data)
    safety.git(Path(data["main"]), "worktree", "remove", str(candidate))
    (candidate.parent / MARKER).unlink()
    (candidate.parent / MANIFEST).unlink()
    (candidate.parent / ACTIVE_LOCK).unlink(missing_ok=True)
    candidate.parent.rmdir()
    print(f"removed owned scratch {candidate}")


def active(candidate: Path) -> bool:
    """Return true while another process holds this candidate's owner lease."""
    lock_path = candidate.parent / ACTIVE_LOCK
    if not lock_path.exists() and not lock_path.is_symlink():
        return False
    if lock_path.is_symlink() or not lock_path.is_file():
        return True
    try:
        fd = os.open(lock_path, os.O_RDONLY | os.O_NOFOLLOW)
    except FileNotFoundError:
        return False
    except OSError:
        return True
    try:
        try:
            fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            return True
        else:
            fcntl.flock(fd, fcntl.LOCK_UN)
            return False
    finally:
        os.close(fd)


def remove(candidate: Path) -> None:
    """Immediate teardown is available only to this scratch creator."""
    data = read_manifest(candidate)
    if data["pid"] != os.getpid() or not owner_alive(data):
        raise safety.Unsafe("only the scratch creator may perform immediate teardown")
    _remove(candidate, data)
    fd = _ACTIVE_FDS.pop(candidate, None)
    if fd is not None:
        os.close(fd)


def gc(root: Path, *, ttl_hours: float = SCRATCH_TTL_HOURS, prune: bool = False) -> int:
    """List or collect old marked scratch only; no legacy adoption or task scanning."""
    if not math.isfinite(ttl_hours) or ttl_hours < 0:
        raise safety.Unsafe("hours must be finite and non-negative")
    main = core.main_root(core.worktree_root(root))
    tmp = main / "tmp"
    if tmp.is_symlink() or tmp.resolve() != tmp:
        raise safety.Unsafe("scratch tmp must be local, not a symlink")
    if prune:
        try:
            safe_prune.prune(main)  # drop vanished candidates, never another mount's (T3784)
        except (safety.Unsafe, safe_prune.PruneError, OSError, subprocess.SubprocessError):
            pass
    for container in sorted(tmp.glob("*")):
        if not any(container.name.startswith(kind + "-") for kind in KINDS):
            continue
        candidate = container / "candidate"
        if _reap_husk(container, main, ttl_hours, prune):
            continue
        try:
            data = read_manifest(candidate)
            if Path(data["main"]) != main:
                raise safety.Unsafe("scratch belongs to another primary")
            if active(candidate):
                raise safety.Unsafe("scratch owner lease is active")
            if owner_alive(data):
                raise safety.Unsafe("scratch creator still alive")
            # A verified-dead owner is quarantined immediately. Identity checks
            # include boot id, pid namespace and process start ticks.
            if prune:
                strip_caches(candidate)  # T2419: free the bytes even if removal is refused
            try:
                inspect(candidate, data)
            except (safety.Unsafe, OSError):
                if prune:
                    strip_caches(candidate)
                raise
            if prune:
                _remove(candidate, data)
            else:
                print(f"would remove owned scratch {candidate}")
        except (safety.Unsafe, OSError, ValueError, KeyError, TypeError) as exc:
            if not candidate.exists() and container.is_dir():
                # The candidate vanished (Errno 2): nothing to inspect, reap or report once.
                if _reap_husk(container, main, ttl_hours, prune):
                    continue
                print(f"retain {container}: {exc} (candidate gone)")
                continue
            print(f"retain {container}: {exc}")
    return 0


def _reap_husk(container: Path, main: Path, ttl_hours: float, prune: bool) -> bool:
    """T2419: drop containers whose candidate worktree is already gone (marker/manifest only
    or empty). They hold no data; `_remove` crashed or was killed between its steps."""
    if (container / "candidate").exists() or container.is_symlink() or not container.is_dir():
        return False
    names = {p.name for p in container.iterdir()}
    if not names <= {MARKER, MANIFEST, MANIFEST + ".new"}:
        return False
    if time.time() - container.stat().st_mtime < ttl_hours * 3600:
        return True
    if prune:
        for name in names:
            (container / name).unlink()
        container.rmdir()
        print(f"removed empty scratch container {container}")
    else:
        print(f"would remove empty scratch container {container}")
    return True
