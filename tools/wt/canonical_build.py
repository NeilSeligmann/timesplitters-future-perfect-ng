"""T3432: build Rust in a fixed per-family source path so artifacts are reused across worktrees.

Cargo keys path crates by mtime and sccache keys them by the compile cwd, so neither can
reuse one worktree's tsfp-ports build in another (T3411), and a target dir shared across
worktrees (T3427) is unsound: a worktree whose edit is older than another worktree's last
build gets the other worktree's artifacts (MEASURED, docs/t3432-prebuilt-ports.md).

`tools.wt run -- cargo build|check|test|clippy ...` therefore runs cargo in
`tmp/wt-cache/canon-src/<family key>/` with `CARGO_TARGET_DIR=cargo-target/canon-<key>`. Before
cargo starts, the worktree's tracked and untracked-unignored files are synced in by content:
an unchanged file keeps its old mtime (cargo stays fresh), a changed file is rewritten with a
new mtime (cargo rebuilds it), a removed file is deleted. That is exactly the history a single
checkout would have, so cargo fingerprints stay correct, while the path, and with it the
sccache key, is the same for every worktree. Ignored top-level-of-ignore entries (build/,
generated/, tmp/, .venv, the XDK surface) are symlinks back into the current worktree, and
`.git` points at the worktree's git dir so build scripts see its HEAD. An exclusive flock per
key is held, through exec, for the whole build. Opt out with TSFP_CANON_BUILD=0.

T3472: each family key is a POOL of trees (`<key>`, `<key>-p1`, ... up to TSFP_CANON_POOL,
default min(build slots, 2)), each with its own target. One tree per family serialized every
worker of the family on its lock. A build takes, without waiting, the free member that last
built this worktree, else the least recently used free one; it waits only when every member
is busy (then on its own member). Each member is synced by content like before, so the
stale-artifact guarantee is per member and unchanged; sccache shares objects across members.

T3712: the shared root (`canon_root`, under /home) is container-local while the cargo targets
live on the shared /workspace disk, so two containers on one host had their own `src-1` tree,
no common lock and ONE target dir: cargo's mtime check then reused artifacts compiled from the
other tree (a committed test ran 0 tests). Each canon base now carries a random `.canon-id`
(created once with O_EXCL); shared-root targets are `canon-<key>-<member>-<id[:12]>`, and every
target records the base id in `.tsfp-canon-id`. A target claimed by another base is never reused:
prepare logs it and builds in the worktree instead.
"""

from __future__ import annotations

import fcntl
import hashlib
import json
import os
import shlex
import shutil
import subprocess
import sys
import time
from collections.abc import Callable
from pathlib import Path

from tools import host_registry

CANON_REL = Path("tmp/wt-cache/canon-src")
TARGET_PREFIX = "canon-"
CANON_SUBCOMMANDS = {"build", "b", "check", "c", "test", "t", "clippy"}
MANIFEST = ".tsfp-canon.json"
CHUNK = 1 << 20

SHARED_ENV = "TSFP_CANON_ROOT"
SHARED_POOL_ENV = "TSFP_CANON_SHARED_POOL"
SEED_MARKER = ".tsfp-t3472-seeded"
BASE_ID = ".canon-id"
TARGET_MARKER = ".tsfp-canon-id"

Git = Callable[[Path, list[str]], bytes]


def _git(root: Path, args: list[str]) -> bytes:
    return subprocess.run(
        ["git", *args], cwd=root, capture_output=True, timeout=120, check=True
    ).stdout


def enabled(env: dict[str, str]) -> bool:
    return env.get("TSFP_CANON_BUILD", "1").lower() not in ("0", "off", "no", "false")


def cargo_argv(cmd: list[str]) -> list[str] | None:
    """The cargo argv when cmd is one direct cargo build/check/test/clippy, else None.

    `sh -c "cargo ..."` with a single simple command counts too (the env hook form). Anything
    with `cd`, pipes or several commands, `cargo run` and `--target-dir` keeps the normal path.
    """
    words = list(cmd)
    if len(words) == 3 and Path(words[0]).name in {"sh", "bash", "zsh"} and words[1] == "-c":
        try:
            words = shlex.split(words[2])
        except ValueError:
            return None
        if any(word in {"&&", "||", ";", "|", "&"} or word.endswith(";") for word in words):
            return None
    if not words or Path(words[0]).name != "cargo":
        return None
    rest = [word for word in words[1:] if not word.startswith("+")]
    if not rest or rest[0] not in CANON_SUBCOMMANDS:
        return None
    if any(word == "--target-dir" or word.startswith("--target-dir=") for word in words):
        return None
    return words


def _files(root: Path, git: Git) -> list[str]:
    from tools.wt import source_identity

    identity = source_identity.load(root)
    if identity is not None:
        return sorted(entry.path for entry in source_identity.entries(identity))
    out = git(root, ["ls-files", "-z", "--cached", "--others", "--exclude-standard"])
    return sorted({item.decode() for item in out.split(b"\0") if item})


def _ignored(root: Path, git: Git) -> list[str]:
    from tools.wt import core, source_identity

    if source_identity.load(root) is not None:
        # Exec trees are exact tracked exports; their bootstrap-owned links and tmp
        # are the only extra inputs supported by the remote prebuilt contract.
        paths = {"tmp"}
        for spec in core.LINKS:
            if not spec.path.startswith("tmp/"):
                paths.update(str(path.relative_to(root)) for path in root.glob(spec.path))
        return sorted(path for path in paths if (root / path).exists())
    out = git(
        root, ["ls-files", "-z", "--others", "--ignored", "--exclude-standard", "--directory"]
    )
    keep = []
    for item in out.split(b"\0"):
        rel = item.decode().rstrip("/")
        if not rel or rel == ".git" or "target" in Path(rel).parts:
            continue
        keep.append(rel)
    return sorted(keep)


def _digest(path: Path) -> str:
    if path.is_symlink():
        return "link:" + os.readlink(path)
    digest = hashlib.blake2b(digest_size=20)
    with path.open("rb") as handle:
        while chunk := handle.read(CHUNK):
            digest.update(chunk)
    return digest.hexdigest()


def _stamp(path: Path) -> list[int]:
    st = path.lstat()
    return [st.st_size, st.st_mtime_ns, st.st_ino, st.st_mode]


def _install(source: Path, dest: Path) -> None:
    dest.parent.mkdir(parents=True, exist_ok=True)
    temp = dest.with_name(f".{dest.name}.t3432tmp")
    if temp.is_dir() and not temp.is_symlink():
        shutil.rmtree(temp)
    elif temp.is_symlink() or temp.exists():
        temp.unlink()
    if source.is_symlink():
        temp.symlink_to(os.readlink(source))
    else:
        shutil.copyfile(source, temp)  # copy_file_range: a reflink on btrfs
        os.chmod(temp, source.stat().st_mode & 0o7777)
    if dest.is_dir() and not dest.is_symlink():
        # The caller holds the canonical-tree lock. Build-generated directories
        # may occupy a path now supplied as a file or symlink by another tree.
        # rmtree removes only this mirror entry; it does not follow its symlinks.
        shutil.rmtree(dest)
    os.replace(temp, dest)


def sync(root: Path, canon: Path, git: Git = _git) -> dict[str, int]:
    """Mirror the worktree's source files into canon by content; return counters."""
    canon.mkdir(parents=True, exist_ok=True)
    manifest_path = canon / MANIFEST
    try:
        old = json.loads(manifest_path.read_text())
    except (OSError, ValueError):
        old = {}
    old_root = old.get("root")
    old_stamps: dict[str, list] = {
        rel: value for rel, value in old.get("stamps", {}).items() if len(value) == 3
    }
    cache_path = (
        canon.parent
        / f"{canon.name}.roots"
        / (hashlib.sha256(str(root).encode()).hexdigest()[:24] + ".json")
    )
    try:
        root_cache: dict[str, list] = json.loads(cache_path.read_text())
    except (OSError, ValueError):
        root_cache = {}
    files = _files(root, git)
    ignored = _ignored(root, git)
    stats = {"files": len(files), "copied": 0, "same": 0, "removed": 0, "links": 0}
    stamps: dict[str, list] = {}
    new_cache: dict[str, list] = {}
    # Ignored links from a previous worktree go first: a path ignored there may be tracked here.
    for rel in old.get("links", []):
        link = canon / rel
        if link.is_symlink():
            link.unlink()
    for rel in files:
        source, dest = root / rel, canon / rel
        if not source.is_symlink() and not source.is_file():
            continue  # deleted but still in the index, or a submodule dir
        src_stamp = _stamp(source)
        recorded = old_stamps.get(rel)
        try:
            dest_stamp = _stamp(dest) if dest.is_symlink() or dest.is_file() else None
        except OSError:
            dest_stamp = None
        if old_root == str(root) and recorded and recorded[:2] == [src_stamp, dest_stamp]:
            digest = recorded[2]
            stats["same"] += 1
        else:
            cached = root_cache.get(rel)
            digest = cached[1] if cached and cached[0] == src_stamp else _digest(source)
            if dest_stamp is None:
                current = None
            elif recorded and recorded[1] == dest_stamp:
                current = recorded[2]
            else:
                current = _digest(dest)
            if current == digest:
                stats["same"] += 1
            else:
                _install(source, dest)
                stats["copied"] += 1
        stamps[rel] = [src_stamp, _stamp(dest), digest]
        new_cache[rel] = [src_stamp, digest]
    wanted = set(stamps)
    for rel in old_stamps:
        if rel not in wanted:
            path = canon / rel
            if path.is_symlink() or path.is_file():
                path.unlink()
                stats["removed"] += 1
    links = []
    for rel in ignored:
        link = canon / rel
        if rel in wanted or link.exists() and not link.is_symlink():
            continue
        link.parent.mkdir(parents=True, exist_ok=True)
        if link.is_symlink():
            link.unlink()
        link.symlink_to(root / rel)
        links.append(rel)
    stats["links"] = len(links)
    from tools.wt import source_identity

    identity = source_identity.load(root)
    dot_git = canon / ".git"
    if dot_git.is_symlink() or dot_git.is_file():
        dot_git.unlink()
    if identity is None:
        for name in (source_identity.MARKER, source_identity.LISTING):
            (canon / name).unlink(missing_ok=True)
        gitdir = git(root, ["rev-parse", "--absolute-git-dir"]).decode().strip()
        dot_git.write_text(f"gitdir: {gitdir}\n")
    else:
        for name in (source_identity.MARKER, source_identity.LISTING):
            _install(root / name, canon / name)
    manifest_path.write_text(json.dumps({"root": str(root), "stamps": stamps, "links": links}))
    cache_path.parent.mkdir(parents=True, exist_ok=True)
    cache_path.write_text(json.dumps(new_cache))
    return stats


def _map_path(word: str, root: Path, canon: Path) -> str:
    prefix, sep, value = word.partition("=") if word.startswith("--") else ("", "", word)
    path = Path(value)
    if path.is_absolute():
        try:
            rel = path.relative_to(root)
        except ValueError:
            return word
        value = str(canon / rel)
    return f"{prefix}{sep}{value}"


# Owner 2026-10-21: the local host is the owner's main PC; two trees per family at most.
POOL_CAP = 2


def pool_size(main: Path, env: dict[str, str]) -> int:
    value = env.get("TSFP_CANON_POOL", "")
    if value.strip():
        try:
            return max(1, min(POOL_CAP * 2, int(value)))
        except ValueError:
            pass
    from tools.wt import buildslots

    directory = main / buildslots.SLOTS_REL
    count = buildslots.slot_count(buildslots.mem_total_mb(), directory)
    return max(1, min(POOL_CAP, count))


def members(key: str, size: int) -> list[str]:
    return [key] + [f"{key}-p{index}" for index in range(1, size)]


def _last_use(base: Path, member: str, root: Path) -> tuple[int, float]:
    """Sort key: this worktree's last tree first, then least recently used."""
    manifest = base / member / MANIFEST
    try:
        same = json.loads(manifest.read_text()).get("root") == str(root)
        return (0 if same else 1, manifest.stat().st_mtime)
    except (OSError, ValueError):
        return (1, 0.0)


def shared_root(main: Path, env: dict[str, str]) -> Path | None:
    """T3472 phase 3: the fixed canonical root shared with the remote hosts, or None.

    sccache keys a compile by the absolute source dir and CARGO_HOME, so a local build only hits
    what a remote built when the paths are physically equal (a symlink resolves to a different
    path). The root is `canon_root` of tools/data/remote_hosts.json (env TSFP_CANON_ROOT
    overrides, `0` opts out); the pool members are `<root>/src`, `<root>/src-1`, ... like the
    remote hosts' and CARGO_HOME is `<root>/cargo`. None when it cannot be created here."""
    value = env.get(SHARED_ENV, "")
    if value == "0":
        return None
    if not value:
        hosts = host_registry.hosts_or_empty(host_registry.path_in(main))  # T3754
        value = next((str(h["canon_root"]) for h in hosts if h.get("canon_root")), "")
    if not value:
        return None
    path = Path(value)
    try:
        path.mkdir(parents=True, exist_ok=True)
    except OSError:
        return None
    return path if os.access(path, os.W_OK) and not path.is_symlink() else None


def shared_members(size: int) -> list[str]:
    return ["src"] + [f"src-{index}" for index in range(1, size)]


def _cache_count(home: Path) -> int:
    total = 0
    for kind in ("cache", "src"):
        for registry in (home / "registry" / kind).glob("*"):
            try:
                total += len(os.listdir(registry))
            except OSError:
                continue
    return total


def seed_cargo_home(main: Path, shared: Path) -> Path | None:
    """`<shared>/cargo`: a real copy of the main cargo home (reflinked where the filesystem
    can), topped up with crates the main home gained later. None when it cannot be made."""
    from tools.wt import cargo_home

    source = cargo_home.unified(main)
    dest = shared / "cargo"
    if not (source / "registry").is_dir():
        return dest if (dest / "registry").is_dir() else None
    lock = os.open(shared / "cargo.seed.lock", os.O_RDWR | os.O_CREAT, 0o666)
    try:
        fcntl.flock(lock, fcntl.LOCK_EX)
        if not (dest / SEED_MARKER).is_file():
            temp = shared / "cargo.seeding"
            shutil.rmtree(temp, ignore_errors=True)
            subprocess.run(
                ["cp", "-a", "--reflink=auto", str(source), str(temp)], check=True, timeout=900
            )
            shutil.rmtree(dest, ignore_errors=True)
            os.replace(temp, dest)
            (dest / SEED_MARKER).write_text(str(source) + "\n")
        elif _cache_count(source) > _cache_count(dest):
            cargo_home.merge(source, dest, apply=True)
    except (OSError, subprocess.SubprocessError):
        return None
    finally:
        os.close(lock)
    return dest


def base_id(base: Path) -> str:
    """The physical identity of a canon base: a random uuid made once, then read back.

    The value is written to a private temp file and hard-linked into place, so a reader never
    sees a partial id and concurrent creators agree on the first link (O_EXCL semantics)."""
    import uuid

    path = base / BASE_ID
    try:
        value = path.read_text().strip()
    except FileNotFoundError:
        value = ""
    if value:
        return value
    temp = base / f".{BASE_ID}.{os.getpid()}.{uuid.uuid4().hex}"
    fd = os.open(temp, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o644)
    try:
        os.write(fd, (uuid.uuid4().hex + "\n").encode())
        os.fsync(fd)
    finally:
        os.close(fd)
    try:
        os.link(temp, path)
    except FileExistsError:
        pass
    finally:
        temp.unlink(missing_ok=True)
    value = path.read_text().strip()
    if not value:
        raise OSError(f"{path} is empty")
    return value


def target_name(key: str, member: str, shared: bool, ident: str) -> str:
    """The cargo target dir name; shared-root targets carry the base id (T3712)."""
    if shared:
        return f"{TARGET_PREFIX}{key}-{member}-{ident[:12]}"
    return f"{TARGET_PREFIX}{member}"


def claim_target(target: Path, ident: str) -> str | None:
    """Record ident in the target; return the foreign id when another base owns it, else None."""
    target.mkdir(parents=True, exist_ok=True)
    marker = target / TARGET_MARKER
    try:
        fd = os.open(marker, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o644)
    except FileExistsError:
        owner = marker.read_text().strip()
        return None if owner == ident else owner or "<empty>"
    try:
        os.write(fd, (ident + "\n").encode())
    finally:
        os.close(fd)
    return None


def acquire_member(
    base: Path,
    key: str,
    size: int,
    root: Path,
    log: Callable[[str], object],
    names: list[str] | None = None,
) -> tuple[str, int, float]:
    """Lock a pool member; return (member, fd, seconds waited)."""
    ordered = sorted(names or members(key, size), key=lambda m: _last_use(base, m, root))
    for member in ordered:
        fd = os.open(base / f"{member}.lock", os.O_RDWR | os.O_CREAT, 0o666)
        try:
            fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
            return member, fd, 0.0
        except BlockingIOError:
            os.close(fd)
    member = ordered[0]
    log(f"tools.wt: waiting for canonical build tree {member} (all {size} trees of {key} busy)")
    started = time.monotonic()
    fd = os.open(base / f"{member}.lock", os.O_RDWR | os.O_CREAT, 0o666)
    fcntl.flock(fd, fcntl.LOCK_EX)
    return member, fd, time.monotonic() - started


def prepare(
    root: Path,
    main: Path,
    cmd: list[str],
    env: dict[str, str],
    cwd: Path,
    key: str,
    git: Git = _git,
    log: Callable[[str], object] | None = None,
) -> tuple[list[str], Path, int] | None:
    """Sync and lock the canonical tree; return (argv, cwd, lock fd) or None to build as is.

    Updates env in place (CARGO_TARGET_DIR, PWD). The lock fd is inheritable so the lock
    lasts until the exec'd cargo exits.
    """
    argv = cargo_argv(cmd)
    if argv is None:
        from tools.wt import host_family

        return host_family.prepare(root, main, cmd, env, cwd, key, git, log)
    if not enabled(env):
        return None
    try:
        rel_cwd = cwd.resolve().relative_to(root.resolve())
    except ValueError:
        return None
    log = log or (lambda text: print(text, file=sys.stderr, flush=True))
    shared = shared_root(main, env)
    cargo_home = seed_cargo_home(main, shared) if shared else None
    if shared and cargo_home is None:
        shared = None
    if shared:
        base = shared
        if env.get(SHARED_POOL_ENV, "").strip().isdigit():
            size = max(1, min(POOL_CAP * 2, int(env[SHARED_POOL_ENV])))
        else:  # the verify default (TSFP_CANON_POOL=1) must not shrink the global pool
            size = pool_size(main, {k: v for k, v in env.items() if k != "TSFP_CANON_POOL"})
        names = shared_members(size)
    else:
        base = main / CANON_REL
        base.mkdir(parents=True, exist_ok=True)
        names = None
        size = pool_size(main, env)
    member, fd, waited = acquire_member(base, key, size, root, log, names)
    os.set_inheritable(fd, True)
    canon = base / member
    started = time.monotonic()
    try:
        ident = base_id(base)
        target = main / "tmp/wt-cache/cargo-target" / target_name(key, member, bool(shared), ident)
        stats = sync(root, canon, git)
        foreign = claim_target(target, ident)
    except Exception as error:  # a cache problem must never break the build
        os.close(fd)
        log(f"tools.wt: canonical build skipped ({error}), building in the worktree")
        return None
    if foreign is not None:
        os.close(fd)
        log(
            f"tools.wt: ERROR T3712 cargo target {target} belongs to canon base id {foreign}, "
            f"not {ident} ({base}); refusing to reuse artifacts from another tree, "
            "building in the worktree"
        )
        return None
    env["CARGO_TARGET_DIR"] = str(target)
    if shared and cargo_home is not None:
        env["CARGO_HOME"] = str(cargo_home)
    new_cwd = canon / rel_cwd
    env["PWD"] = str(new_cwd)
    log(
        f"tools.wt: canonical build in {canon} ({stats['copied']} changed, "
        f"{stats['removed']} removed of {stats['files']} files in "
        f"{time.monotonic() - started:.1f} s, tree wait {waited:.1f} s; "
        "TSFP_CANON_BUILD=0 opts out)"
    )
    return [_map_path(word, root, canon) for word in argv], new_cwd, fd
