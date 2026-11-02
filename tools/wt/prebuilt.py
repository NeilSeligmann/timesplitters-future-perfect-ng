"""Shared immutable Bevy host builds keyed by the primary commit (T2359).

T3411: a commit whose build inputs hash like an existing host reuses it (hard links),
and the global builder lock single-flights concurrent builds.
"""

from __future__ import annotations

import contextlib
import fcntl
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import time
from collections.abc import Callable, Iterator
from datetime import UTC, datetime
from pathlib import Path

from tools.agents import jobevents
from tools.cache_telemetry import record_lookup
from tools.wt import (
    build_profile,
    cache_keys,
    remote_prebuilt,
    shared_artifacts,
    source_identity,
)

MANIFEST = "manifest.json"
READY_LOG = ".ready-log.jsonl"
LOCAL_SLOT_WAIT_S = 60.0
_SHA = re.compile(r"^[0-9a-f]{40}$")


def resolve_commit(root: Path, ref: str) -> str:
    """A full commit SHA for `ref`. Lanes kept passing abbreviated shas or origin/main, and every
    such job failed before building ("expected full 40-character commit SHA", 2026-10-30)."""
    if _SHA.fullmatch(ref):
        return ref
    result = subprocess.run(
        ["git", "-C", str(root), "rev-parse", "--verify", "--quiet", f"{ref}^{{commit}}"],
        capture_output=True,
        text=True,
        timeout=30,
        check=False,
    )
    sha = result.stdout.strip()
    if result.returncode != 0 or not _SHA.fullmatch(sha):
        raise PrebuiltError(f"cannot resolve {ref!r} to a commit")
    return sha


_BASE_BUILD_INPUTS = (
    "src/",
    "crates/",
    "cmake/",
    "third_party/xboxrecomp/",
    "tools/lift/",
    "tools/config/",
    "CMakeLists.txt",
    "tools/build_bevy.sh",
    "tools/bundle_bevy_libs.sh",
    "tools/dev/build.sh",
    "tools/dev/stamp.sh",
    "tools/private_host.py",
    "tools/data/shader_original_entries.json",
)
_BUILD_INPUTS = (
    *_BASE_BUILD_INPUTS,
    f"{build_profile.PROFILE_DIR}/{build_profile.PROFILE_PREFIX}{build_profile.GATE_PROFILE}.env",
)


def _build_inputs(profile: str | None = None) -> tuple[str, ...]:
    selected = profile or build_profile.GATE_PROFILE
    return (
        *_BASE_BUILD_INPUTS,
        build_profile.profile_path(Path("."), selected).as_posix(),
    )


_BUILD_RUN_NOTES = re.compile(
    r"(?ms)^# <!-- run-notes:build begin.*?^# <!-- run-notes:build end -->\s*\n?"
)


class PrebuiltError(RuntimeError):
    """An invalid cache request or unusable host artifact."""


def _close_fds() -> bool:
    """T3416: inside a DMTCP snapshot capture, close_fds closes DMTCP's PID-map fd and every
    git child exits 99, so main_root fell back to the worktree and host lookup by input hash
    failed in every snapshot take (MEASURED 2026-10-20)."""
    return not os.environ.get("DMTCP_VIRTUAL_PID")


def main_root(start: Path | None = None) -> Path:
    """Main checkout root (T2472): parent of the git common dir, never a fixed path.

    Works from the main checkout, any linked worktree, and a plain clone on the
    owner's host. Falls back to the start directory outside a repository.
    """
    start = (start or Path.cwd()).resolve()
    if source_identity.exec_root(start) is not None:
        # T3760/sub-lift: a git-less exec tree never asks git (the strict shim logged each call).
        return source_identity.main_override() or start
    done = subprocess.run(
        ["git", "rev-parse", "--path-format=absolute", "--git-common-dir"],
        cwd=start,
        capture_output=True,
        text=True,
        check=False,
        timeout=60,
        close_fds=_close_fds(),
    )
    common = done.stdout.strip()
    if done.returncode or not common:
        # T3760: a git-less exec tree on a remote host names its stand-in by TSFP_MAIN_ROOT.
        return source_identity.main_override() or start
    return Path(common).parent


def cache_root(start: Path | None = None) -> Path:
    override = os.environ.get("TSFP_PREBUILT_ROOT")
    if override:
        return Path(override).expanduser()
    return main_root(start) / "tmp" / "wt-cache" / "prebuilt"


def build_environment(
    root: Path, base: dict[str, str], profile: str | None = None
) -> dict[str, str]:
    """Environment for the prebuilt build, derived from the main checkout (T2472).

    Explicit variables win. Otherwise a shared directory under the main checkout
    is used only when it already exists, so a fresh owner checkout inherits its
    own ~/.cargo and network instead of an offline, missing cache.
    """
    main = main_root(root)
    env = dict(base)
    env["TSFP_BUILD_NAME"] = "prebuilt-build"
    if env.get("TSFP_NO_CCACHE") == "1":
        env.pop("CMAKE_C_COMPILER_LAUNCHER", None)
        env.pop("RUSTC_WRAPPER", None)
    # T3597: the host is built with exactly the gate profile, whatever the caller exported, so
    # the input hash (which names the profile) stays true. Fails closed on a missing profile.
    for switch in build_profile.CLASSIFICATION:
        env.pop(switch, None)
    # T3824: build.sh takes the live census default from tools/data/live_census.json, not a
    # prebuilt input: a shared host is census OFF unless its profile (an input) says ON.
    env.pop("TSFP_BUILD_LIVE_CENSUS", None)
    env.update(build_profile.environment(root, profile))
    env.setdefault("TSFP_BUILD_LIVE_CENSUS", "0")
    env.setdefault("CARGO_TARGET_DIR", str(main / "tmp/wt-cache/cargo-target/default"))
    shared_cargo = main / "tmp/rust/cargo"
    if "CARGO_HOME" not in env and shared_cargo.is_dir():
        env["CARGO_HOME"] = str(shared_cargo)
        env.setdefault("CARGO_NET_OFFLINE", "true")
    shared_rustup = main / "tmp/rust/rustup"
    if "RUSTUP_HOME" not in env and shared_rustup.is_dir():
        env["RUSTUP_HOME"] = str(shared_rustup)
    from tools.wt import cargo_home as cargo_homes

    bevy_home = cargo_homes.bevy_home(main, env)  # T3472
    if "TSFP_BEVY_CARGO_HOME" not in env and bevy_home.is_dir():
        env["TSFP_BEVY_CARGO_HOME"] = str(bevy_home)
    from tools.wt import compiler_cache

    if compiler_cache.configure(root, main, env):
        from tools.wt import core as wt_core

        wt_core.apply_shared_sccache(root, main, env)
    if env.get("CARGO_HOME"):
        env["PATH"] = str(Path(env["CARGO_HOME"]) / "bin") + os.pathsep + env.get("PATH", "")
    apply_incremental(env)
    return env


#: T3605: suffix of the cargo target dir of an incremental prebuilt build. build.sh derives the
#: `-rt-host` sibling target from it.
INCREMENTAL_TARGET_SUFFIX = "-inc"


def incremental_wanted(env: dict[str, str]) -> bool:
    """Opt in to incremental prebuilts; fleet builds default to smaller targets."""
    return (
        env.get("TSFP_PREBUILT_INCREMENTAL") == "1"
        and env.get("TSFP_CARGO_INCREMENTAL", "1") != "0"
    )


def apply_incremental(env: dict[str, str]) -> None:
    """Build the prebuilt's local Rust crates incrementally, in a target of their own (T3605).

    MEASURED on the droplet (8 vCPU, main ba3b80c7ce): a one-line edit in tsfp-ports rebuilt the
    host in 318 s with CARGO_INCREMENTAL=0 (a full non-incremental compile of the facade, 263 s)
    and in 43 s incremental in the same persisted target (docs/t3605-build-speed.md). The prebuilt
    target dirs persist per family. sccache-rustc.sh runs `-Cincremental` units directly, registry
    crates stay cached, and the release profile marks only the tsfp-* crates incremental. The
    target dir gets its own name because flipping the flag in one target would rebuild every
    local crate on each switch.
    """
    if not incremental_wanted(env):
        env["CARGO_INCREMENTAL"] = "0"
        env["TSFP_CARGO_INCREMENTAL"] = "0"
        return
    env["CARGO_INCREMENTAL"] = "1"
    env["TSFP_CARGO_INCREMENTAL"] = "1"
    target = env.get("CARGO_TARGET_DIR", "").rstrip("/")
    if target and not target.endswith(INCREMENTAL_TARGET_SUFFIX):
        env["CARGO_TARGET_DIR"] = target + INCREMENTAL_TARGET_SUFFIX


def _git(root: Path, *args: str, check: bool = True) -> str:
    done = subprocess.run(
        ["git", *args],
        cwd=root,
        capture_output=True,
        text=True,
        check=False,
        timeout=60,
        close_fds=_close_fds(),
    )
    if check and done.returncode:
        raise PrebuiltError(done.stderr.strip() or f"git {' '.join(args)} failed")
    return done.stdout.strip()


def _exec_identity(root: Path) -> source_identity.Identity | None:
    """T3760: the identity of a git-less exec tree (remote host), None in a git checkout."""
    try:
        return source_identity.load(root)
    except source_identity.SourceIdentityError as error:
        raise PrebuiltError(str(error)) from None


def head_commit(root: Path) -> str:
    """`git rev-parse HEAD`, or the synced commit of an exec tree (no git on the host)."""
    identity = _exec_identity(root)
    if identity is None:
        return _git(root, "rev-parse", "HEAD")
    try:
        return source_identity.head(identity)
    except source_identity.SourceIdentityError as error:
        raise PrebuiltError(str(error)) from None


def _same_commit(identity: source_identity.Identity, sha: str, what: str) -> None:
    if sha != identity.commit:
        raise PrebuiltError(
            f"{what} of {sha} needs git: the exec tree {identity.root} holds {identity.commit}"
        )


def primary_ref(root: Path) -> str:
    """Find the local primary branch, using origin/HEAD to identify its name."""
    remote_head = _git(root, "symbolic-ref", "--quiet", "refs/remotes/origin/HEAD", check=False)
    remote_primary = remote_head.removeprefix("refs/remotes/") if remote_head else ""
    current = _git(root, "branch", "--show-current", check=False)
    if current and current == remote_primary:
        return current
    # The shared artifact namespace follows the remote primary tip, not a stale
    # local main branch in a worker checkout.
    candidates = [remote_primary, "main", "master"]
    for candidate in candidates:
        if candidate and _git(root, "rev-parse", "--verify", candidate, check=False):
            return candidate
    if current:
        return current
    raise PrebuiltError("cannot determine the repository primary branch")


def primary_commit(root: Path, ref: str | None = None) -> str:
    identity = _exec_identity(root)
    if identity is not None:
        raise PrebuiltError(
            f"the exec tree {identity.root} has no primary branch; name the commit (--commit)"
        )
    ref = ref or primary_ref(root)
    sha = _git(root, "merge-base", "HEAD", ref, check=False)
    if not _SHA.fullmatch(sha):
        raise PrebuiltError(f"no merge base between HEAD and primary branch {ref}")
    return sha


def artifact_dir(
    root: Path, sha: str, cache: Path | None = None, profile: str | None = None
) -> Path:
    del root
    return (cache or cache_root()) / _artifact_name(sha, profile, capture_identity())


def _artifact_name(sha: str, profile: str | None, capture: dict) -> str:
    """Name every build variant independently; normal and legacy names stay stable."""
    selected = profile or build_profile.GATE_PROFILE
    if not isinstance(selected, str) or not re.fullmatch(r"[a-zA-Z0-9_-]+", selected):
        raise ValueError("invalid prebuilt profile")
    if not isinstance(capture, dict) or (
        capture
        and (
            capture.get("capture") != "1"
            or set(capture) - {"capture", "capture_kind"}
            or any(not isinstance(value, str) for value in capture.values())
        )
    ):
        raise ValueError("invalid capture variant")
    suffix = "" if selected == build_profile.GATE_PROFILE else f"-{selected}"
    if capture:
        variant = json.dumps(capture, sort_keys=True, separators=(",", ":"))
        suffix += "-capture-" + hashlib.sha256(variant.encode()).hexdigest()[:16]
    return f"{sha}{suffix}"


def expected_path(root: Path, cache: Path | None = None, profile: str | None = None) -> Path:
    return artifact_dir(root, primary_commit(root), cache, profile) / "tsfp-bevy"


def available(binary: Path) -> bool:
    binary = binary.resolve()
    return binary.name == "tsfp-bevy" and _manifest(binary.parent) is not None


def _manifest(path: Path) -> dict | None:
    try:
        data = json.loads((path / MANIFEST).read_text())
        binary = path / "tsfp-bevy"
        profile_data = data.get("build_profile")
        profile = (
            profile_data.get("name", build_profile.GATE_PROFILE)
            if isinstance(profile_data, dict)
            else build_profile.GATE_PROFILE
        )
        expected_dir = _artifact_name(
            str(data.get("sha", "")), profile, data.get("capture_variant", {})
        )
        if (
            data.get("schema") != 1
            or not _SHA.fullmatch(str(data.get("sha", "")))
            or path.name != expected_dir
            or not isinstance(data.get("lift_id"), str)
            or not isinstance(data.get("toolchain"), dict)
            or not isinstance(data.get("files"), dict)
            or not isinstance(data.get("size_bytes"), int)
            or data["size_bytes"] <= 0
            or not isinstance(data.get("build_time_seconds"), (int, float))
            or data["build_time_seconds"] < 0
            or not isinstance(data.get("built_at_epoch"), (int, float))
            or not binary.is_file()
            or not os.access(binary, os.X_OK)
        ):
            return None
        return data
    except (OSError, ValueError, KeyError, TypeError):
        return None


def _lock_path(cache: Path, name: str) -> Path:
    lock_dir = cache / ".locks"
    lock_dir.mkdir(parents=True, exist_ok=True)
    return lock_dir / name


@contextlib.contextmanager
def _locked(
    path: Path, operation: int, *, nonblocking: bool = False, purpose: str | None = None
) -> Iterator[bool]:
    """flock `path` for the block. `purpose` (T3500): record and heartbeat the hold as a
    tools.wt.lease so the lock watchdog can name its holder and bound the hold."""
    path.parent.mkdir(parents=True, exist_ok=True)
    fd = os.open(path, os.O_CREAT | os.O_RDWR, 0o600)
    held = None
    try:
        flags = operation | (fcntl.LOCK_NB if nonblocking else 0)
        try:
            fcntl.flock(fd, flags)
        except BlockingIOError:
            yield False
            return
        if purpose is not None:
            from tools.wt import lease

            held = lease.attach(fd, path, purpose)
        yield True
    finally:
        if held is not None:
            held.release()
        os.close(fd)


@contextlib.contextmanager
def lease(binary: Path, cache: Path | None = None) -> Iterator[Path]:
    """Hold a shared artifact lease while a launched host may use this binary."""
    cache = (cache or cache_root()).resolve()
    binary = binary.resolve()
    try:
        directory = binary.parent
        directory.relative_to(cache)
    except ValueError as exc:
        raise PrebuiltError(f"host is outside the shared prebuilt cache: {binary}") from exc
    manifest = _manifest(directory)
    if manifest is None or binary.name != "tsfp-bevy":
        raise PrebuiltError(f"not a valid shared prebuilt host: {binary}")
    coordination = _lock_path(cache, ".gc.lock")
    artifact_lock = _lock_path(cache, f"{manifest['sha']}.lock")
    artifact_guard = _locked(artifact_lock, fcntl.LOCK_SH)
    with _locked(coordination, fcntl.LOCK_SH) as acquired:
        if not acquired:
            raise PrebuiltError("could not acquire prebuilt cache read lock")
        if not artifact_guard.__enter__():
            raise PrebuiltError("could not acquire prebuilt host lease")
        if _manifest(directory) is None:
            artifact_guard.__exit__(None, None, None)
            raise PrebuiltError(f"prebuilt host disappeared while opening: {binary}")
    try:
        yield binary
    finally:
        artifact_guard.__exit__(None, None, None)


def _is_build_input(path: str, inputs: tuple[str, ...] | None = None) -> bool:
    return any(path == item or path.startswith(item) for item in (inputs or _BUILD_INPUTS))


def _without_generated_build_notes(text: str) -> str:
    """Remove the generated build notes block, which contains no build logic."""
    return _BUILD_RUN_NOTES.sub("", text).rstrip("\n")


def changed_build_inputs(
    root: Path, base: str | None = None, profile: str | None = None
) -> list[str]:
    """List changed tracked/untracked build or lift inputs relative to the primary base."""
    base = base or primary_commit(root)
    identity = _exec_identity(root)
    if identity is not None:  # T3760: compare the files with the synced tree's blob ids
        _same_commit(identity, base, "a build-input diff")
        try:
            return source_identity.modified(identity, _build_inputs(profile))
        except source_identity.SourceIdentityError as error:
            raise PrebuiltError(str(error)) from None
    changed = set(filter(None, _git(root, "diff", "--name-only", "-z", base, "--").split("\0")))
    untracked = _git(root, "ls-files", "--others", "--exclude-standard", "-z")
    changed.update(filter(None, untracked.split("\0")))
    selected_inputs = _build_inputs(profile)
    inputs = [path for path in changed if _is_build_input(path, selected_inputs)]
    if "tools/dev/build.sh" in inputs:
        original = _git(root, "show", f"{base}:tools/dev/build.sh", check=False)
        current_path = root / "tools/dev/build.sh"
        if original and current_path.is_file():
            current = current_path.read_text()
            if _without_generated_build_notes(current) == _without_generated_build_notes(original):
                inputs.remove("tools/dev/build.sh")
    return sorted(inputs)


def probe_env(root: Path) -> dict[str, str]:
    """The environment the input hash probes tool versions under (I-T3865-10).

    The build always runs with a project Rust toolchain (`tools.wt env`, or the host registry's
    `toolchain_dir` exported as RUSTUP_HOME). A caller without RUSTUP_HOME (the shell hook's
    background warm-main) used to probe the bare rustup proxy, which printed an install notice
    instead of a version and hashed to a key no builder could reproduce.
    """
    from tools.wt import core

    main = main_root(root)
    if os.environ.get("RUSTUP_HOME") or not core.shared_rustup_home(main).is_dir():
        return dict(os.environ)
    return core.toolchain_env(main)


def input_hash(root: Path, sha: str, profile: str | None = None) -> str:
    """T3411: hash of the build inputs at a commit (tracked trees only, generated notes ignored).

    Commits that only touch docs or tools outside _BUILD_INPUTS share one hash, so their
    hosts are reused instead of rebuilt.
    """
    return input_key(root, sha, profile).key


def input_components(root: Path, sha: str, profile: str | None = None) -> dict[str, object]:
    """The named components `input_hash` folds (tree, tool versions, profile): recorded in the
    manifest so a rejected artifact can name what differs (I-T3865-10)."""
    return input_key(root, sha, profile).components


def input_key(root: Path, sha: str, profile: str | None = None) -> cache_keys.KeyResult:
    selected_profile = profile or build_profile.GATE_PROFILE
    selected_inputs = _build_inputs(selected_profile)
    identity = _exec_identity(root)
    if identity is None:
        rows = _git(root, "ls-tree", "-r", "-z", sha, "--", *selected_inputs).split("\0")

        def show(path: str) -> str:
            return _git(root, "show", f"{sha}:{path}")

    else:  # T3760: a git-less exec tree answers from its verified listing, same bytes
        _same_commit(identity, sha, "the input hash")
        try:
            rows = source_identity.ls_tree_rows(identity, selected_inputs)
        except source_identity.SourceIdentityError as error:
            raise PrebuiltError(str(error)) from None

        def show(path: str) -> str:
            try:  # stripped like _git's output
                return source_identity.read_text(identity, path).strip()
            except source_identity.SourceIdentityError as error:
                raise PrebuiltError(str(error)) from None

    digest = hashlib.sha256()
    # T3597: every prebuilt is built with the gate profile; a host built without it must not
    # match, so the profile name is part of the identity (the list itself is a tracked input).
    digest.update(f"build-profile\t{selected_profile}\0".encode())
    for row in sorted(filter(None, rows)):
        meta, path = row.split("\t", 1)
        if path == "tools/dev/build.sh":
            text = _without_generated_build_notes(show(path))
            meta = "build.sh " + hashlib.sha256(text.encode()).hexdigest()
        digest.update(f"{meta}\t{path}\0".encode())
    tools = cache_keys.tool_versions(
        root, env=probe_env(root), names=("rustc", "cc", "cmake", "python3"), include_pin=False
    )
    broken = sorted(name for name, value in tools.items() if value == cache_keys.UNAVAILABLE)
    if broken:  # a key over a failed probe is one no builder can reproduce
        raise PrebuiltError(
            f"cannot hash the build inputs: {', '.join(broken)} version unavailable"
        )
    return cache_keys.make_key(
        "bevy-host-prebuilt",
        files={"build-input-tree": digest.hexdigest()},
        tools=tools,
        env={},
        extra={"profile": selected_profile, **capture_identity()},
    )


def capture_identity() -> dict[str, str]:
    """T3784: a TSFP_BUILD_CAPTURE=1 host is a different artifact (transformed lifted chunks,
    never a play or gate host), so the flag is part of the input hash: a capture build never
    reuses or restores a normal host and never publishes itself under a normal host's key.
    Empty for a normal build, so normal hashes are unchanged."""
    if os.environ.get("TSFP_BUILD_CAPTURE") != "1":
        return {}
    # T3833: the live targeted capture host (play.sh --capture-targets) keeps the normal Rust
    # defaults; its own key, so it never reuses or replaces the oracle capture host, and back.
    kind = os.environ.get("TSFP_CAPTURE_KIND", "")
    return {"capture": "1"} if kind in ("", "oracle") else {"capture": "1", "capture_kind": kind}


CAPTURE_MARKER = b"t2_capture.c/"  # tool_version string the capture runtime compiles in
CAPTURE_MISMATCH_LOG = ".capture-mismatch.jsonl"


def binary_is_capture(binary: Path) -> bool:
    """True when the host binary carries the capture runtime (TSFP_BUILD_CAPTURE=1 build)."""
    import mmap

    with binary.open("rb") as source, mmap.mmap(source.fileno(), 0, access=mmap.ACCESS_READ) as m:
        return m.find(CAPTURE_MARKER) >= 0


def capture_mismatch(cache: Path, binary: Path, origin: str, key: str) -> bool:
    """T3784 (I-T3784-3): True, logged for HEALTH prebuilt-capture-mismatch, when a host about
    to be restored or reused is a capture build and the request is not, or the reverse."""
    try:
        found = binary_is_capture(binary)
    except (OSError, ValueError):
        return False  # unreadable binaries are refused by the manifest checks
    wanted = bool(capture_identity())
    if found == wanted:
        return False
    row = {"at": time.time(), "origin": origin, "key": key, "found_capture": found}
    print(
        f"prebuilt: refusing {origin} host {key[:12]}: capture build {found}, request {wanted} "
        "(HEALTH prebuilt-capture-mismatch, I-T3784-3)",
        file=sys.stderr,
        flush=True,
    )
    with contextlib.suppress(OSError):
        cache.mkdir(parents=True, exist_ok=True)
        with (cache / CAPTURE_MISMATCH_LOG).open("a") as log:
            log.write(json.dumps(row, sort_keys=True) + "\n")
    return True


def capture_mismatch_findings(cache: Path, now: float, window_s: float = 86400) -> list[str]:
    """Refusals in the last `window_s` (HEALTH prebuilt-capture-mismatch)."""
    try:
        lines = (cache / CAPTURE_MISMATCH_LOG).read_text().splitlines()
    except OSError:
        return []
    rows = []
    for line in lines:
        with contextlib.suppress(ValueError, TypeError, KeyError):
            row = json.loads(line)
            if now - float(row["at"]) <= window_s:
                rows.append(
                    f"{row['origin']} {str(row['key'])[:12]} capture={row['found_capture']}"
                )
    return rows


def _reuse(cache: Path, sha: str, wanted: str, destination: Path) -> Path | None:
    """Publish an existing host with the same input hash under this commit (hard links)."""
    for candidate in sorted(cache.iterdir()):
        manifest = _manifest(candidate)
        if not manifest or manifest.get("input_hash") != wanted:
            continue
        if capture_mismatch(cache, candidate / "tsfp-bevy", "reuse", wanted):
            continue
        staging = cache / f".{destination.name}.staging-{os.getpid()}"
        staging.mkdir()
        try:
            for item in candidate.iterdir():
                if item.is_file() and item.name != MANIFEST:
                    try:
                        os.link(item, staging / item.name)
                    except OSError:
                        shutil.copy2(item, staging / item.name)
            manifest.update(
                sha=sha,
                capture_variant=capture_identity(),
                reused_from=candidate.name,
                built_at=datetime.now(UTC).isoformat(timespec="seconds"),
                built_at_epoch=time.time(),
            )
            (staging / MANIFEST).write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n")
            _readonly_tree(staging)
            with _locked(_lock_path(cache, ".gc.lock"), fcntl.LOCK_SH) as publish_allowed:
                if not publish_allowed or not _manifest(candidate):
                    raise PrebuiltError("reuse source vanished during publication")
                staging.rename(destination)
        except BaseException:
            _make_writable(staging)
            shutil.rmtree(staging, ignore_errors=True)
            raise
        print(f"prebuilt reused {candidate.name} for {sha} (same build inputs)")
        return destination / "tsfp-bevy"
    return None


def needs_build(root: Path, base: str | None = None) -> bool:
    return bool(changed_build_inputs(root, base))


def _capture(argv: list[str], cwd: Path) -> str:
    done = subprocess.run(argv, cwd=cwd, capture_output=True, text=True, check=False, timeout=30)
    return done.stdout.splitlines()[0].strip() if done.stdout else "unavailable"


def toolchain(root: Path) -> dict[str, str]:
    return {
        "rustc": _capture(["rustc", "--version"], root),
        "cargo": _capture(["cargo", "--version"], root),
        "clang": _capture(["clang", "--version"], root),
        "cmake": _capture(["cmake", "--version"], root),
        "ninja": _capture(["ninja", "--version"], root),
    }


def _host_libraries(binary: Path) -> list[Path]:
    done = subprocess.run(
        ["ldd", str(binary)], capture_output=True, text=True, check=False, timeout=30
    )
    if done.returncode:
        raise PrebuiltError(f"ldd failed for built host: {done.stderr.strip()}")
    libraries: list[Path] = []
    for line in done.stdout.splitlines():
        if "libtsfp_" not in line:
            continue
        match = re.search(r"=>\s+(\S+)", line)
        if not match or match.group(1) == "not":
            raise PrebuiltError(f"host has an unresolved game library: {line.strip()}")
        library = Path(match.group(1))
        if not library.is_file():
            raise PrebuiltError(f"linked game library is missing: {library}")
        libraries.append(library)
    if not libraries:
        raise PrebuiltError("built host has no linked libtsfp_* runtime libraries")
    return sorted(set(libraries))


def _readonly_tree(path: Path) -> None:
    for item in path.rglob("*"):
        executable = item.is_file() and bool(item.stat().st_mode & 0o111)
        item.chmod(0o555 if item.is_dir() or executable else 0o444)
    path.chmod(0o555)


def _make_writable(path: Path) -> None:
    if path.exists():
        for item in path.rglob("*"):
            if item.is_dir():
                item.chmod(0o755)
            elif item.stat().st_nlink == 1:
                # T3416: never chmod a hard-linked file: reused hosts share the inode with
                # the published source, and 0o644 stripped the exec bit of every reuse
                # (MEASURED 2026-10-20). Unlinking only needs the writable directory.
                item.chmod(0o644)
        path.chmod(0o755)


def _input_lock(cache: Path, wanted: str) -> Path:
    return _lock_path(cache, f"input-{wanted}.lock")


def _inflight_path(cache: Path, wanted: str) -> Path:
    directory = cache / ".inflight"
    directory.mkdir(parents=True, exist_ok=True)
    return directory / f"{wanted}.json"


def read_inflight(cache: Path, wanted: str) -> dict | None:
    """The live build flight for a build-input hash, or None (T3448)."""
    try:
        data = json.loads(_inflight_path(cache, wanted).read_text())
    except (OSError, ValueError, KeyError, TypeError):
        return None
    from tools.wt import lease

    if lease.holder_alive(data)[0]:
        return data
    # The flight's kernel lock is authoritative across PID namespaces. Older workers
    # cannot write heartbeats, so preserve their record while the lock is held.
    with _locked(_input_lock(cache, wanted), fcntl.LOCK_EX, nonblocking=True) as free:
        return data if not free else None


def _write_inflight(cache: Path, sha: str, wanted: str, mode: str) -> None:
    payload = {
        "sha": sha,
        "input_hash": wanted,
        "pid": os.getpid(),
        **__import__("tools.wt.lease", fromlist=["holder"]).holder(),
        "heartbeat": time.time(),
        "mode": mode,
        "started_at": time.time(),
    }
    path = _inflight_path(cache, wanted)
    temporary = path.with_suffix(f".{os.getpid()}.tmp")
    temporary.write_text(json.dumps(payload, sort_keys=True) + "\n")
    temporary.replace(path)


def _clear_inflight(cache: Path, wanted: str) -> None:
    with contextlib.suppress(OSError):
        _inflight_path(cache, wanted).unlink()


def record_ready(
    cache: Path, *, sha: str, wanted: str, source: str, seconds: float | None = None
) -> None:
    """Append one durable readiness record (T3448) so GC cannot erase the measurement.

    `tools.prebuilt_readiness` joins these records with the main commit stream. `source` is
    `local`, `remote:<worker>` or `reuse:<sha>`.
    """
    row = {
        "ready_at": round(time.time(), 3),
        "sha": sha,
        "input_hash": wanted,
        "source": source,
        "build_seconds": None if seconds is None else round(seconds, 3),
    }
    with contextlib.suppress(OSError), (cache / READY_LOG).open("a") as handle:
        handle.write(json.dumps(row, sort_keys=True) + "\n")


def lookup(
    root: Path,
    sha: str | None = None,
    cache: Path | None = None,
    profile: str | None = None,
) -> Path | None:
    """Published host whose build inputs equal this commit's (T3448), own key first.

    Docs- and tools-only commits share one input hash, so they resolve to the host of the
    last commit that changed the C, Rust or cmake inputs without any copy or rebuild.
    """
    from tools import prebuilt_select

    root = root.resolve()
    cache = (cache or cache_root(root)).resolve()
    sha = sha or primary_commit(root)
    own = artifact_dir(root, sha, cache, profile)
    if _manifest(own) and not capture_mismatch(cache, own / "tsfp-bevy", "lookup", sha):
        return own / "tsfp-bevy"
    return prebuilt_select.find_host(root, commit=sha, cache=cache, profile=profile)


def _publish_staging(staging: Path, destination: Path, cache: Path) -> None:
    _readonly_tree(staging)
    with _locked(_lock_path(cache, ".gc.lock"), fcntl.LOCK_SH) as publish_allowed:
        if not publish_allowed:
            raise PrebuiltError("could not acquire prebuilt cache publish lock")
        staging.rename(destination)


BUSY_BACKOFF_FIRST_S = 5.0
BUSY_BACKOFF_MAX_S = 120.0
_monotonic = time.monotonic
_sleep = time.sleep


class Superseded(PrebuiltError):
    """warm-main gave up because origin/main moved to a tip with other build inputs."""

    jobevent_result = "superseded"  # T3801: latest-wins by design, not a failed build
    jobevent_cause = "superseded"


class RemoteUnavailable(PrebuiltError):
    """remote_only build found no free remote builder; the caller retries on its next tick."""

    jobevent_result = (
        "skipped"  # T3801: 8 of 15 post-fix prebuilt "errors" were warm-main deferrals
    )
    jobevent_cause = "remote-unavailable"


class BuildFailed(PrebuiltError):
    """A remote builder actually ran the build and it failed."""


def _prebuilt_queue_wait_s() -> float:
    """Maximum time to retry busy remote builders before local fallback."""
    try:
        configured = float(os.environ.get("TSFP_PREBUILT_QUEUE_S", "1800"))
    except ValueError:
        configured = 1800.0
    return min(1800.0, max(0.0, configured))


def _build_remote(
    root: Path,
    sha: str,
    wanted: str,
    cache: Path,
    destination: Path,
    only: str | None = None,
    *,
    remote_only: bool = False,
    order: tuple[str, ...] | None = None,
    abort: Callable[[], bool] | None = None,
    profile: str | None = None,
) -> tuple[Path, float, str] | None:
    """Remote build published under the commit key, or None when no worker can help.

    Workers are tried in `remote_prebuilt.REMOTE_BUILD_ORDER` (the droplet, T3461); a busy,
    absent or failing worker passes the build to the next one. `only` names a single worker
    (`--remote droplet`).
    """
    started = _monotonic()
    order = order or remote_prebuilt.REMOTE_BUILD_ORDER
    if only in remote_prebuilt.REMOTE_BUILDABLE_WORKERS:
        order = (only,)
    else:  # T3748: least loaded builder first; toolchain parity is still checked per worker
        from tools.fleet.config import main_checkout
        from tools.fleet.headroom import order_names

        order = tuple(order_names(order, main_checkout(root), build=True))
    from tools.wt import retired_hosts

    gone = retired_hosts.retired_identities()  # T3753: a retired host is never tried or waited on
    order = tuple(worker for worker in order if worker not in gone)
    queue_s = _prebuilt_queue_wait_s()
    deadline = _monotonic() + queue_s
    attempts: dict[str, str] = {}
    build_failure: str | None = None
    delay = BUSY_BACKOFF_FIRST_S
    while order:
        busy_workers: set[str] = set()
        failed_workers: set[str] = set()
        for worker in order:
            if abort is not None and abort():
                raise Superseded("a newer origin/main tip needs a different build")
            staging = cache / f".{destination.name}.remote-staging-{os.getpid()}"
            try:
                fetched = remote_prebuilt.build_to_staging(
                    root,
                    sha=sha,
                    input_hash=wanted,
                    destination=staging,
                    worker_name=worker,
                    **({"profile": profile} if profile else {}),
                )
            except (
                OSError,
                subprocess.SubprocessError,
                remote_prebuilt.RemotePrebuiltError,
            ) as error:
                fetched = None
                if attempts.get(worker) != str(error):  # T3737: log a repeated busy once
                    print(
                        f"remote prebuilt failed on {worker}: {error}", file=sys.stderr, flush=True
                    )
                attempts[worker] = str(error)
                if isinstance(error, remote_prebuilt.RemoteBuildFailed):
                    build_failure = str(error)
                if isinstance(error, remote_prebuilt.RemoteBusyError):
                    busy_workers.add(worker)
                elif not isinstance(error, remote_prebuilt.RemoteBuildFailed):
                    failed_workers.add(worker)
            if fetched is None:
                if staging.exists():
                    _make_writable(staging)
                    shutil.rmtree(staging, ignore_errors=True)
                continue
            manifest, peak_rss_kb = fetched
            # The worker publishes read-only trees and rsync -a keeps the modes: reopen the
            # directory to add the local manifest (T3448 live finding: PermissionError on
            # manifest.json after a good build). _make_writable would strip the executable bit.
            staging.chmod(staging.stat().st_mode | 0o200)
            (staging / MANIFEST).unlink(missing_ok=True)
            (staging / MANIFEST).write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n")
            _publish_staging(staging, destination, cache)
            if _manifest(destination) is None:
                _make_writable(destination)
                shutil.rmtree(destination, ignore_errors=True)
                raise PrebuiltError("remote artifact failed local prebuilt manifest validation")
            from tools.wt import compiler_cache

            cache_summary = (
                compiler_cache.summary(manifest["compiler_cache"])
                if "compiler_cache" in manifest
                else manifest.get("remote_sccache", "unavailable")
            )
            print(
                f"prebuilt published remotely: {destination / 'tsfp-bevy'}; "
                f"worker={manifest.get('remote_worker')} "
                f"peak_rss={peak_rss_kb if peak_rss_kb is not None else 'unknown'} KiB "
                f"sccache={cache_summary}",
                flush=True,
            )
            return destination / "tsfp-bevy", _monotonic() - started, worker
        # T3737: a worker that is absent or failed hard drops out; only busy ones are waited for,
        # so a down LAN host does not end the wait for a busy droplet.
        # Rule 45 (remote-slot-contention): while any builder is busy the wait goes on, and a
        # builder that failed transiently (raised, not busy) is tried again. A host that is down
        # (build_to_staging returned None) stays excluded.
        order = tuple(name for name in order if name in busy_workers or name in failed_workers)
        if not busy_workers or _monotonic() >= deadline:
            break
        # A matching prebuilt may have appeared in the shared store while workers were busy.
        if not remote_only:
            found = _fetch_shared_prebuilt(cache, destination, wanted, sha)
            if found is not None:
                print(
                    f"prebuilt restored from shared artifact store while queued: {found}",
                    flush=True,
                )
                return found, time.monotonic() - started, "shared-store"
        print(
            f"remote builders busy ({', '.join(sorted(busy_workers))}); next try in {delay:.0f} s",
            flush=True,
        )
        _sleep(min(delay, max(0.0, deadline - _monotonic())))
        delay = min(delay * 2, BUSY_BACKOFF_MAX_S)  # T3737: bounded exponential backoff
    if build_failure is not None:
        raise BuildFailed(build_failure)
    return None


def _fetch_shared_prebuilt(cache: Path, destination: Path, wanted: str, sha: str) -> Path | None:
    """Restore a shared archive only when its embedded manifest matches this input hash."""
    with tempfile.TemporaryDirectory(prefix=".shared-prebuilt-", dir=cache) as temp:
        archive = Path(temp) / "prebuilt.tar.gz"
        if not shared_artifacts.fetch_file("prebuilt", wanted, archive):
            return None
        extracted = Path(temp) / "artifact"
        if not shared_artifacts.unpack_tree(archive, extracted):
            return None
        try:
            source_manifest = json.loads((extracted / MANIFEST).read_text())
            source_sha = source_manifest.get("sha")
            source_name = _artifact_name(
                source_sha,
                (source_manifest.get("build_profile") or {}).get("name"),
                source_manifest.get("capture_variant", {}),
            )
        except (OSError, ValueError, AttributeError):
            return None
        if not isinstance(source_sha, str) or not _SHA.fullmatch(source_sha):
            return None
        source = Path(temp) / source_name
        extracted.rename(source)
        manifest = _manifest(source)
        if (
            not manifest
            or manifest.get("input_hash") != wanted
            or not available(source / "tsfp-bevy")
            or capture_mismatch(cache, source / "tsfp-bevy", "shared-store", wanted)
        ):
            return None
        staging = cache / f".{destination.name}.shared-{os.getpid()}"
        if staging.exists():
            _make_writable(staging)
            shutil.rmtree(staging)
        shutil.copytree(source, staging)
        staged_manifest = json.loads((staging / MANIFEST).read_text())
        staged_manifest["sha"] = sha
        staged_manifest["input_hash"] = wanted
        staged_manifest["capture_variant"] = capture_identity()
        (staging / MANIFEST).write_text(
            json.dumps(staged_manifest, indent=2, sort_keys=True) + "\n"
        )
        _readonly_tree(staging)
        _publish_staging(staging, destination, cache)
        return destination / "tsfp-bevy" if _manifest(destination) else None


def _publish_shared_prebuilt(artifact: Path, wanted: str) -> None:
    """Best-effort cross-host publication after the normal local manifest validates."""
    if shared_artifacts.already_published("prebuilt", wanted):
        return
    with tempfile.TemporaryDirectory(prefix=".shared-prebuilt-publish-") as temp:
        archive = Path(temp) / "prebuilt.tar.gz"
        shared_artifacts.pack_tree(artifact, archive)
        shared_artifacts.publish_file("prebuilt", wanted, archive)


def _local_slot(root: Path, wait_s: float | None) -> tuple[bool, int | None]:
    """Take a T3411 build slot for the local compile.

    Returns (admitted, fd). admitted=False means the host stayed saturated for `wait_s`
    seconds (None waits the normal limit), so the caller can try a remote worker instead of
    queueing behind other builds.
    """
    from tools.wt import buildslots

    if os.environ.get("TSFP_BUILD_ADMISSION", "").lower() in ("off", "0", "no"):
        return True, None
    if os.environ.get("TSFP_BUILD_SLOT_HELD"):
        return True, None  # an outer verify or gate already holds a slot for this tree

    def log(text: str) -> None:
        if "running unadmitted" not in text:  # the timeout is reported by the caller
            print(text, file=sys.stderr, flush=True)

    fd = buildslots.acquire(main_root(root), wait_s=wait_s, log=log)
    return fd is not None, fd


def require_profile_modules(
    root: Path, build_name: str, profile_name: str | None = None
) -> dict[str, str]:
    """T3597: refuse a host whose CMake configuration lacks a profile module.

    Catches an old build.sh that ignores the profile and a build dir configured without it.
    Returns the profile switches that were verified.
    """
    profile_name = profile_name or build_profile.GATE_PROFILE
    profile = build_profile.load(root, profile_name)
    cache_file = root / "tmp" / build_name / "CMakeCache.txt"
    try:
        cmake_cache = cache_file.read_text()
    except OSError as error:
        raise PrebuiltError(f"cannot verify the build profile: {cache_file} unreadable") from error
    missing = build_profile.verify_cmake_cache(cmake_cache, profile)
    if missing:
        raise PrebuiltError(
            f"host was not built with the {build_profile.GATE_PROFILE} profile: "
            + "; ".join(missing)
        )
    return profile


@contextlib.contextmanager
def _remote_canonical_host(root: Path, env: dict[str, str], profile: str | None) -> Iterator[Path]:
    """Hold the fixed-source lease until the prebuilt artifacts are published."""
    if source_identity.load(root) is None:
        yield root
        return
    from tools.wt import canonical_build, core, host_family

    env["TSFP_CANON_BUILD"] = "1"
    env["TSFP_CARGO_INCREMENTAL"] = "0"
    env["CARGO_INCREMENTAL"] = "0"
    command = ["sh", "tools/dev/build.sh"]
    family = f"prebuilt-{profile or build_profile.GATE_PROFILE}"
    key = core.cargo_family_key(root, family, command)
    prepared = host_family.prepare(
        root, main_root(root), command, env, root, key, canonical_build._git
    )
    if prepared is None:
        raise PrebuiltError("remote prebuilt requires a canonical host source")
    _, mirror, fd = prepared
    try:
        yield mirror
    finally:
        os.close(fd)


def _build_local(
    root: Path,
    sha: str,
    wanted: str,
    cache: Path,
    destination: Path,
    slot_fd: int | None = None,
    profile: str | None = None,
) -> Path:
    """Compile on this host. The global builder lock keeps it to one compile at a time.

    `slot_fd` is the already taken T3411 slot; build.sh then runs with TSFP_BUILD_SLOT_HELD so
    its inner `tools.wt run` calls do not queue for a second slot.
    """
    build_name = "prebuilt-build"
    from tools import datadisk

    datadisk.check("prebuilt build", paths=[cache, destination])  # T3745 budget and root guard
    env = datadisk.tmp_env(build_environment(root, os.environ.copy(), profile))
    if slot_fd is not None:
        env["TSFP_BUILD_SLOT_HELD"] = "1"
    target_dir = Path(env["CARGO_TARGET_DIR"])
    if not target_dir.is_absolute():
        target_dir = (root / target_dir).resolve()
    env["CARGO_TARGET_DIR"] = str(target_dir)
    command = ["sh", "tools/dev/build.sh"]
    jobs = env.get("CARGO_BUILD_JOBS", "")
    if jobs:
        if not re.fullmatch(r"[1-9][0-9]*", jobs):
            raise PrebuiltError("CARGO_BUILD_JOBS must be a positive integer for a local prebuilt")
        command.extend(("--jobs", jobs))
    if env.get("TSFP_PREBUILT_RECONFIGURE") == "1":
        command.append("--reconfigure")
    with (
        _locked(
            _lock_path(cache, ".build.lock"), fcntl.LOCK_EX, purpose="prebuilt-build"
        ) as acquired,
        contextlib.ExitStack() as leases,
    ):
        if not acquired:
            raise PrebuiltError("could not acquire prebuilt builder lock")
        build_root = leases.enter_context(_remote_canonical_host(root, env, profile))
        target_dir = Path(env["CARGO_TARGET_DIR"])
        started = time.monotonic()
        print(
            f"prebuilt building {sha}: {' '.join(command)} (build dir tmp/{build_name})",
            flush=True,
        )
        from tools.wt import compiler_cache, prebuilt_stages

        cache_before = compiler_cache.snapshot(env)
        try:
            with prebuilt_stages.recording(root, env) as stage_evidence:
                result = subprocess.run(command, cwd=build_root, env=env, check=False)
        finally:
            if slot_fd is not None:
                os.close(slot_fd)
        if result.returncode:
            raise PrebuiltError(f"full host build failed with exit {result.returncode}")
        elapsed = time.monotonic() - started
        stage_evidence.update(
            source_commit=sha, build_input_hash=wanted, measured_at_epoch=time.time()
        )
        cache_evidence = compiler_cache.interval(cache_before, compiler_cache.snapshot(env))
        source_binary = target_dir / "release/tsfp-bevy"
        stamp = target_dir / "release/.tsfp-build-stamp"
        if not source_binary.is_file() or not stamp.is_file():
            raise PrebuiltError(f"build did not produce {source_binary} and its build stamp")
        stamp_words = stamp.read_text().split()
        if len(stamp_words) < 3 or stamp_words[0] != sha:
            raise PrebuiltError(
                f"build stamp does not identify requested commit {sha}: {stamp_words[:3]}"
            )

        profile_name = profile or build_profile.GATE_PROFILE
        profile_switches = require_profile_modules(root, build_name, profile_name)

        staging = cache / f".{destination.name}.staging-{os.getpid()}"
        staging.mkdir()
        try:
            shutil.copy2(source_binary, staging / "tsfp-bevy")
            for library in _host_libraries(source_binary):
                shutil.copy2(library, staging / library.name)
            shutil.copy2(stamp, staging / ".tsfp-build-stamp")
            files = sorted(item for item in staging.iterdir() if item.is_file())
            file_hashes = {
                item.name: hashlib.sha256(item.read_bytes()).hexdigest() for item in files
            }
            size = sum(item.stat().st_size for item in files)
            manifest = {
                "schema": 1,
                "sha": sha,
                "capture_variant": capture_identity(),
                "input_hash": wanted,
                "input_components": input_components(root, sha, profile),
                "lift_id": stamp_words[2],
                "toolchain": toolchain(root),
                "build_profile": {"name": profile_name, "switches": profile_switches},
                "built_at": datetime.now(UTC).isoformat(timespec="seconds"),
                "built_at_epoch": time.time(),
                "build_time_seconds": round(elapsed, 3),
                "compiler_cache": cache_evidence,
                "build_stages": stage_evidence,
                "size_bytes": size,
                "files": file_hashes,
            }
            (staging / MANIFEST).write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n")
            _publish_staging(staging, destination, cache)
        except BaseException:
            _make_writable(staging)
            shutil.rmtree(staging, ignore_errors=True)
            raise
        print(
            f"prebuilt published: {destination / 'tsfp-bevy'}; "
            f"build {elapsed:.1f}s; artifact {size} bytes; lift {stamp_words[2]}"
        )
    return destination / "tsfp-bevy"


@jobevents.timed("prebuilt")
def build(
    root: Path,
    *,
    sha: str | None = None,
    keep: int = 3,
    cache: Path | None = None,
    remote: str = "auto",
    remote_only: bool = False,
    profile: str | None = None,
    order: tuple[str, ...] | None = None,
    abort: Callable[[], bool] | None = None,
) -> Path:
    """Publish a host for the commit's build inputs: reuse, remote first, local fallback.

    T3448: the flight is single per build-input hash. Concurrent callers (supervisor loop,
    environment hook, gates) wait on one per-hash lock, then find the published host and
    return without building. Different hashes may overlap only as one remote plus one local
    compile (the laptop and the global builder lock each admit one).
    """
    root = root.resolve()
    cache = (cache or cache_root()).resolve()
    selected_profile = profile or build_profile.GATE_PROFILE
    sha = sha or primary_commit(root)
    try:
        build_profile.load(root, selected_profile)
    except build_profile.ProfileError as error:
        raise PrebuiltError(str(error)) from None
    sha = resolve_commit(root, sha)
    head = head_commit(root)
    if head != sha and remote == "local":
        raise PrebuiltError(
            f"local build requires a checkout at the requested commit: HEAD={head}, requested={sha}"
        )
    if keep < 1:
        raise PrebuiltError("--keep must be at least 1")
    destination = artifact_dir(root, sha, cache, selected_profile)
    if destination.exists():
        if _manifest(destination) and not capture_mismatch(
            cache, destination / "tsfp-bevy", "build", sha
        ):
            record_lookup("bevy-prebuilt", True)
            print(f"prebuilt already exists: {destination / 'tsfp-bevy'}")
            return destination / "tsfp-bevy"
        raise PrebuiltError(
            f"refusing to replace an invalid existing prebuilt directory: {destination}"
        )
    dirty = changed_build_inputs(root, sha, selected_profile) if head == sha else []
    if dirty:
        raise PrebuiltError(
            "build inputs differ from the primary commit ("
            + ", ".join(dirty)
            + "); commit them first or use needs-build to inspect them"
        )

    cache.mkdir(parents=True, exist_ok=True)
    wanted = input_hash(root, sha, selected_profile)
    flight = _input_lock(cache, wanted)
    with _locked(flight, fcntl.LOCK_EX, nonblocking=True) as free:
        pass
    if not free:
        other = read_inflight(cache, wanted) or {}
        print(
            f"prebuilt build of inputs {wanted[:12]} is in flight "
            f"(pid {other.get('pid', '?')}, {other.get('mode', '?')}); waiting for it",
            flush=True,
        )
    published: Path
    with _locked(flight, fcntl.LOCK_EX) as acquired:
        if not acquired:
            raise PrebuiltError("could not acquire prebuilt build-input lock")
        if destination.exists():
            if _manifest(destination):
                record_lookup("bevy-prebuilt", True)
                print(f"prebuilt already exists: {destination / 'tsfp-bevy'}")
                return destination / "tsfp-bevy"
            raise PrebuiltError(
                f"refusing to replace an invalid existing prebuilt directory: {destination}"
            )
        reused = _reuse(cache, sha, wanted, destination)
        if reused is not None:
            record_lookup("bevy-prebuilt", True)
            record_ready(cache, sha=sha, wanted=wanted, source=f"reuse:{reused.parent.name[:12]}")
            gc(root, keep=keep, cache=cache, protect=sha)
            return reused
        # `--remote-only` is an explicit request to exercise a healthy remote
        # builder and refuse if it cannot build. A shared cached binary must not
        # satisfy or mask that request (in particular, readiness provenance must
        # continue to say which worker produced it).
        if not remote_only:
            shared = _fetch_shared_prebuilt(cache, destination, wanted, sha)
            if shared is not None:
                record_lookup("bevy-prebuilt", True)
                record_ready(cache, sha=sha, wanted=wanted, source="shared-store")
                print(f"prebuilt restored from shared artifact store: {shared}", flush=True)
                gc(root, keep=keep, cache=cache, protect=sha)
                return shared
        record_lookup("bevy-prebuilt", False)
        # T3824: remote workers take an explicit profile name (remote_prebuilt --profile).
        use_remote = remote_prebuilt.should_build_remotely(remote)
        remote_profile = (
            None if selected_profile == build_profile.GATE_PROFILE else selected_profile
        )
        if remote_only and not use_remote:
            raise PrebuiltError("remote-only prebuild requested but remote mode is not selected")
        _write_inflight(cache, sha, wanted, "remote" if use_remote else "local")
        try:
            remote_result = (
                _build_remote(
                    root,
                    sha,
                    wanted,
                    cache,
                    destination,
                    remote,
                    remote_only=remote_only,
                    order=order,
                    abort=abort,
                    profile=remote_profile,
                )
                if use_remote
                else None
            )
            slot_fd: int | None = None
            if remote_result is None:
                if use_remote:
                    if remote_only:
                        raise RemoteUnavailable(
                            "remote prebuild unavailable; local fallback disabled"
                        )
                    print("remote prebuilt unavailable; falling back to local build", flush=True)
                if head != sha:
                    raise PrebuiltError(
                        "remote prebuild unavailable and local fallback cannot build a different "
                        f"HEAD: HEAD={head}, requested={sha}"
                    )
                # T3448: a saturated host (T3411 slots, memory admission) sends the build to the
                # remote worker after LOCAL_SLOT_WAIT_S instead of queueing for the slot.
                admitted, slot_fd = _local_slot(root, LOCAL_SLOT_WAIT_S if use_remote else None)
                if not admitted:
                    print(
                        f"local build slot busy for {LOCAL_SLOT_WAIT_S:.0f} s; "
                        "sending the build to the remote worker",
                        flush=True,
                    )
                    remote_result = _build_remote(
                        root,
                        sha,
                        wanted,
                        cache,
                        destination,
                        remote,
                        remote_only=remote_only,
                        profile=remote_profile,
                    )
                    if remote_result is None:
                        print("remote worker unavailable; waiting for a local slot", flush=True)
                        _admitted, slot_fd = _local_slot(root, None)
            if remote_result is not None:
                published, seconds, worker = remote_result
                record_ready(
                    cache,
                    sha=sha,
                    wanted=wanted,
                    source="shared-store" if worker == "shared-store" else f"remote:{worker}",
                    seconds=seconds,
                )
            else:
                _write_inflight(cache, sha, wanted, "local")
                started = time.monotonic()
                if selected_profile == build_profile.GATE_PROFILE:
                    published = _build_local(root, sha, wanted, cache, destination, slot_fd)
                else:
                    published = _build_local(
                        root, sha, wanted, cache, destination, slot_fd, selected_profile
                    )
                record_ready(
                    cache,
                    sha=sha,
                    wanted=wanted,
                    source="local",
                    seconds=time.monotonic() - started,
                )
            if _manifest(published.parent):
                _publish_shared_prebuilt(published.parent, wanted)
        finally:
            _clear_inflight(cache, wanted)
    gc(root, keep=keep, cache=cache, protect=sha)
    return published


def _origin_main(root: Path) -> str | None:
    sha = _git(root, "rev-parse", "--verify", "origin/main", check=False)
    commit = _git(root, "rev-parse", "--verify", f"{sha}^{{commit}}", check=False)
    return sha if _SHA.fullmatch(sha) and commit == sha else None


def warm_main(
    root: Path,
    *,
    cache: Path | None = None,
    remote_only: bool = False,
    rounds: int = 3,
) -> Path | None:
    """Run warm-main and heartbeat the scheduled record while its build is active."""
    import threading

    resolved_cache = (cache or cache_root(root)).resolve()
    state_path = resolved_cache / ".warm-main.json"
    stopped = threading.Event()

    def beat() -> None:
        while not stopped.wait(1):
            try:
                state = json.loads(state_path.read_text())
            except (OSError, ValueError):
                continue  # the scheduler may still be publishing the initial record
            if state.get("pid") != os.getpid() or state.get("status") != "scheduled":
                continue
            state["heartbeat"] = time.time()
            temporary = state_path.with_name(f".{state_path.name}.{os.getpid()}.tmp")
            temporary.write_text(json.dumps(state, sort_keys=True) + "\n")
            temporary.replace(state_path)

    thread = threading.Thread(target=beat, daemon=True)
    thread.start()
    try:
        return _warm_main_impl(root, cache=cache, remote_only=remote_only, rounds=rounds)
    finally:
        stopped.set()
        thread.join(timeout=2)


def _warm_main_impl(
    root: Path,
    *,
    cache: Path | None = None,
    remote_only: bool = False,
    rounds: int = 3,
) -> Path | None:
    """Pre-build the fetched origin/main when its build inputs have no published host.

    T3448: readiness is keyed by the input hash, so a docs- or tools-only tip returns the
    existing host at once. While a build runs the tip may move; the next round follows it
    (at most `rounds` builds per call) so stale intermediate hashes are never queued.
    """
    root = root.resolve()
    cache = (cache or cache_root(root)).resolve()
    result: Path | None = None
    failure: PrebuiltError | None = None
    for _ in range(rounds):
        sha = _origin_main(root)
        if sha is None:
            print("prebuilt warm-main skipped: origin/main commit is unavailable locally")
            return result
        wanted = input_hash(root, sha)
        host = lookup(root, sha, cache) if cache.is_dir() else None
        if host is not None:
            print(f"prebuilt warm-main skipped: input hash already cached ({wanted})")
            record_warm_stat(cache, sha=sha, wanted=wanted, outcome="cached")
            return host
        started = _monotonic()
        try:
            result = build(
                root,
                sha=sha,
                keep=12,
                cache=cache,
                remote="auto",
                remote_only=remote_only,
                order=remote_prebuilt.WARM_BUILD_ORDER,
                abort=_newer_tip_check(root, sha, wanted),
            )
        except Superseded as error:
            # T3737 latest-wins: do not build (or keep waiting for) a stale tip.
            print(f"prebuilt warm-main: {sha} given up, {error}", flush=True)
            record_warm_stat(cache, sha=sha, wanted=wanted, outcome="superseded")
            continue
        except PrebuiltError as error:
            # T3834: a deferral (RemoteUnavailable, jobevent_result "skipped") is not a build
            # error; detect_warm_main_failing counts it apart.
            skipped = getattr(error, "jobevent_result", "") == "skipped"
            record_warm_stat(
                cache,
                sha=sha,
                wanted=wanted,
                outcome="skipped" if skipped else "failed",
                seconds=_monotonic() - started,
                error=str(error),
                cause=getattr(error, "jobevent_cause", None),
            )
            failure = error
            if _origin_main(root) == sha:
                raise
            continue
        failure = None
        manifest = _manifest(result.parent) or {}
        record_warm_stat(
            cache,
            sha=sha,
            wanted=wanted,
            outcome="ready",
            seconds=_monotonic() - started,
            worker=str(manifest.get("remote_worker") or "local"),
        )
        if _origin_main(root) == sha:
            break
    if failure is not None and result is None:
        raise failure
    return result


WARM_STATS = ".warm-main-stats.jsonl"


def record_warm_stat(cache: Path, *, sha: str, wanted: str, outcome: str, **fields: object) -> None:
    """T3737: append one line per warm-main attempt (ready, cached, superseded, skipped, failed).

    `tools.fleet.detectors.detect_warm_main_failing` reads this file; before it, 19 failed
    runs in 8 h left nothing but per-tip logs.
    """
    row = {"at": time.time(), "sha": sha, "input_hash": wanted, "outcome": outcome, **fields}
    row["error"] = str(row["error"])[:300] if "error" in row else None
    try:
        cache.mkdir(parents=True, exist_ok=True)
        with (cache / WARM_STATS).open("a") as handle:
            handle.write(json.dumps({k: v for k, v in row.items() if v is not None}) + "\n")
    except OSError:
        pass  # statistics must never fail a build


def _newer_tip_check(root: Path, sha: str, wanted: str) -> Callable[[], bool]:
    """True once origin/main points at a tip whose build inputs differ from `wanted`."""
    judged: dict[str, bool] = {}

    def superseded() -> bool:
        tip = _origin_main(root)
        if tip is None or tip == sha:
            return False
        if tip not in judged:
            judged[tip] = input_hash(root, tip) != wanted
        return judged[tip]

    return superseded


def _state_pid_alive(state: dict) -> bool:
    from tools.wt import lease

    if not state.get("pidns"):
        # Legacy state has no namespace identity; keep same-container behavior until
        # retry_after expires rather than interpreting an invisible foreign pid as dead.
        pid = state.get("pid")
        try:
            if isinstance(pid, int) and pid > 0:
                os.kill(pid, 0)
                return True
        except PermissionError:
            return True
        except OSError:
            pass
        return False
    if state.get("start_ticks") is None and lease.is_local(state):
        pid = state.get("pid")
        try:
            if isinstance(pid, int) and pid > 0:
                os.kill(pid, 0)
                return True
        except PermissionError:
            return True
        except OSError:
            return False
    return lease.holder_alive(state)[0]


def schedule_main_warm(root: Path, *, cache: Path | None = None, retry_after_s: int = 900) -> bool:
    """Start one detached remote prebuild when origin/main's build inputs have no host.

    Cheap on the hot path: the environment hook calls this on every worker command, so a
    tip that was already judged returns from the state file without touching git trees.
    """
    root = root.resolve()
    cache = (cache or cache_root(root)).resolve()
    sha = _origin_main(root)
    if sha is None:
        return False
    cache.mkdir(parents=True, exist_ok=True)
    state_path = cache / ".warm-main.json"
    lock = _lock_path(cache, ".warm-main.lock")
    with _locked(lock, fcntl.LOCK_EX) as acquired:
        if not acquired:
            return False
        now = time.time()
        try:
            state = json.loads(state_path.read_text())
        except (OSError, ValueError):
            state = {}
        if state.get("sha") == sha:
            if state.get("status") in ("no-build-change", "ready"):
                return False
            if _state_pid_alive(state):
                return False
            if now - float(state.get("attempted_at", 0)) < retry_after_s:
                return False
        elif _state_pid_alive(state):
            # One warm flight at a time: it re-reads origin/main when its build finishes.
            return False
        wanted = input_hash(root, sha)
        if lookup(root, sha, cache) is not None or read_inflight(cache, wanted) is not None:
            state_path.write_text(
                json.dumps({"sha": sha, "input_hash": wanted, "status": "ready", "checked_at": now})
                + "\n"
            )
            return False
        command = [
            sys.executable,
            "-m",
            "tools.wt",
            "prebuilt",
            "warm-main",
            "--worktree",
            str(root),
            "--remote-only",
        ]
        log_path = cache / f"warm-main-{sha}.log"
        with log_path.open("ab") as log:
            process = subprocess.Popen(
                command,
                cwd=root,
                env=os.environ.copy(),
                stdin=subprocess.DEVNULL,
                stdout=log,
                stderr=subprocess.STDOUT,
                start_new_session=True,
                close_fds=True,
            )
        state_path.write_text(
            json.dumps(
                {
                    "sha": sha,
                    "input_hash": wanted,
                    "attempted_at": now,
                    "pid": process.pid,
                    **__import__("tools.wt.lease", fromlist=["holder"]).holder(process.pid),
                    "heartbeat": now,
                    "status": "scheduled",
                },
                sort_keys=True,
            )
            + "\n"
        )
        print(f"prebuilt warm-main scheduled: {sha} ({wanted})")
        return True


def gc(
    root: Path, *, keep: int = 3, cache: Path | None = None, protect: str | None = None
) -> list[str]:
    """Keep the newest N input-hash groups of complete hosts, skipping hosts under a lease.

    `protect` (a sha) is the host the caller just produced: a reused or restored host of an
    old commit keeps its old `built_at_epoch` and would otherwise be evicted at once (T3587
    incident: every bisect midpoint restore was deleted before its replay).

    T3448: hosts that share a build-input hash (a real build plus its hard-linked reuse
    aliases) count as one group, so a burst of docs-only commits cannot evict the host the
    next gate needs.
    """
    del root
    if keep < 1:
        raise PrebuiltError("--keep must be at least 1")
    cache = (cache or cache_root()).resolve()
    if not cache.is_dir():
        return []
    removed: list[str] = []
    with _locked(_lock_path(cache, ".gc.lock"), fcntl.LOCK_EX) as acquired:
        if not acquired:
            raise PrebuiltError("could not acquire prebuilt cache GC lock")
        entries: list[tuple[float, str, Path, str]] = []
        for directory in cache.iterdir():
            if not directory.is_dir():
                continue
            manifest = _manifest(directory)
            if manifest:
                group = str(manifest.get("input_hash") or directory.name)
                entries.append(
                    (
                        float(manifest.get("built_at_epoch", 0)),
                        group,
                        directory,
                        str(manifest["sha"]),
                    )
                )
        entries.sort(reverse=True, key=lambda item: item[0])
        kept_groups: list[str] = []
        for _built_at, group, directory, sha in entries:
            if group in kept_groups or (protect and protect in (sha, directory.name[:40])):
                continue
            if len(kept_groups) < keep:
                kept_groups.append(group)
                continue
            with _locked(
                _lock_path(cache, f"{sha}.lock"), fcntl.LOCK_EX, nonblocking=True
            ) as available:
                if not available:
                    print(f"prebuilt retained (in use): {directory}")
                    continue
                _make_writable(directory)
                shutil.rmtree(directory)
                removed.append(sha)
                print(f"prebuilt removed: {directory}")
    if not removed:
        count = sum(1 for path in cache.iterdir() if path.is_dir() and _manifest(path))
        print(f"prebuilt GC: kept {min(keep, count)} newest; removed 0")
    return removed


def resolve_host(configured: str | None, root: Path) -> Path:
    if configured:
        return Path(configured).expanduser().resolve()
    return expected_path(root)


def is_cached_host(binary: Path, cache: Path | None = None) -> bool:
    try:
        binary.resolve().relative_to((cache or cache_root()).resolve())
        return True
    except ValueError:
        return False


def _missing_message(
    root: Path, sha: str | None, cache: Path | None, profile: str | None = None
) -> str:
    cache = (cache or cache_root(root)).resolve()
    sha = sha or primary_commit(root)
    wanted = input_hash(root, sha, profile)
    flight = read_inflight(cache, wanted)
    state = (
        f"build in flight (pid {flight.get('pid')}, {flight.get('mode')}, "
        f"{int(time.time() - float(flight.get('started_at', 0)))}s)"
        if flight
        else "no build in flight; run `python3 -m tools.wt prebuilt build`"
    )
    return (
        f"prebuilt host is not available yet: {expected_path(root, cache, profile)} "
        f"(build inputs {wanted[:12]}; {state})"
    )


def cli(argv: list[str] | None = None) -> int:
    import argparse

    parser = argparse.ArgumentParser(prog="python3 -m tools.wt prebuilt")
    sub = parser.add_subparsers(dest="action", required=True)
    for action in ("path", "build", "gc", "needs-build", "run", "warm-main"):
        item = sub.add_parser(action)
        item.add_argument("--worktree", type=Path, default=Path.cwd())
        item.add_argument("--cache", type=Path, default=None)
        if action == "path":
            item.add_argument("--if-present", action="store_true")
            item.add_argument("--commit", help="commit to resolve (default: primary merge base)")
            item.add_argument("--profile", default=build_profile.GATE_PROFILE)
        elif action in ("build", "gc"):
            item.add_argument("--keep", type=int, default=3)
        if action == "build":
            item.add_argument("--commit")
            item.add_argument("--profile", default=build_profile.GATE_PROFILE)
            item.add_argument(
                "--remote",
                choices=("auto", *sorted(remote_prebuilt.REMOTE_BUILDABLE_WORKERS), "local"),
                default=os.environ.get("TSFP_PREBUILT_REMOTE", "auto"),
                help="build on a named worker, or let auto try the workers in order",
            )
            item.add_argument("--remote-only", action="store_true")
        if action == "warm-main":
            item.add_argument("--remote-only", action="store_true")
        if action == "needs-build":
            item.add_argument("--base")
        if action == "run":
            item.add_argument("--host", type=Path, required=True)
            item.add_argument("command", nargs=argparse.REMAINDER)
    args = parser.parse_args(argv)
    root = args.worktree.resolve()
    try:
        if args.action == "path":
            host = lookup(root, args.commit, args.cache, args.profile)
            if host is None:
                if args.if_present:
                    return 0
                raise PrebuiltError(_missing_message(root, args.commit, args.cache, args.profile))
            print(host)
        elif args.action == "build":
            build(
                root,
                sha=args.commit,
                keep=args.keep,
                cache=args.cache,
                remote=args.remote,
                remote_only=args.remote_only,
                profile=args.profile,
            )
        elif args.action == "warm-main":
            warm_main(root, cache=args.cache, remote_only=args.remote_only)
        elif args.action == "gc":
            gc(root, keep=args.keep, cache=args.cache)
        elif args.action == "run":
            command = args.command[1:] if args.command[:1] == ["--"] else args.command
            if not command:
                raise PrebuiltError("run requires a command after --")
            host = args.host.expanduser().resolve()
            with lease(host, args.cache):
                env = os.environ.copy()
                library_path = str(host.parent)
                if env.get("LD_LIBRARY_PATH"):
                    library_path += os.pathsep + env["LD_LIBRARY_PATH"]
                env["LD_LIBRARY_PATH"] = library_path
                return subprocess.run(command, cwd=root, env=env, check=False).returncode
        else:
            changed = changed_build_inputs(root, args.base)
            print("yes" if changed else "no")
            if changed:
                for path in changed:
                    print(f"  {path}")
        return 0
    except (OSError, PrebuiltError, subprocess.SubprocessError) as exc:
        print(f"prebuilt {args.action} failed: {exc}", file=sys.stderr)
        return 1
