"""Private mutable launch resources for cooperative host runners (T2253)."""

from __future__ import annotations

import contextlib
import contextvars
import fcntl
import os
import shutil
import signal
import socket
import struct
import subprocess
import tempfile
import time
from collections.abc import Iterator
from pathlib import Path

_slot: contextvars.ContextVar[int | None] = contextvars.ContextVar("host_slot", default=None)

_scope: contextvars.ContextVar[contextlib.ExitStack] = contextvars.ContextVar("host_resources")


@contextlib.contextmanager
def launch_scope(slot: int | None = None) -> Iterator[None]:
    with contextlib.ExitStack() as stack:
        token = _scope.set(stack)
        slot_token = _slot.set(_slot.get() if slot is None else slot)
        try:
            yield
        finally:
            _slot.reset(slot_token)
            _scope.reset(token)


def _copy_file(source: str, destination: str) -> str:
    """Linux reflink CoW where supported, otherwise a private byte copy."""
    try:
        with open(source, "rb") as original, open(destination, "wb") as private:
            fcntl.ioctl(private.fileno(), 0x40049409, original.fileno())  # FICLONE
    except OSError:
        shutil.copyfile(source, destination)
    shutil.copystat(source, destination)
    return destination


def _copy(source: Path, destination: Path) -> None:
    # No hardlinks: translators may truncate an existing module. Reflink or full
    # copies never write back to the seed; symlinks are dereferenced as well.
    if source.exists():
        shutil.copytree(source, destination, copy_function=_copy_file)
    else:
        destination.mkdir()


SHARED_GPU_REL = "tmp/wt-cache/gpu-shared/modules"
SHARED_GPU_MAX_BYTES = 512 * 1024 * 1024


def shared_gpu_cache(base: Path) -> Path | None:
    """The one translated-shader and pipeline-cache folder shared by every run and worktree.

    It lives under the primary checkout's tmp/wt-cache (the git common dir's parent), so linked
    worktrees and repeated play runs all warm each other. Before 2026-11-02 each run copied a
    small seed and threw its translated modules and VkPipelineCache away, so every run began cold
    (shader translation 148 ms each, `VkPipelineCache loaded 0 bytes`, the over-100 ms hitches)."""
    override = os.environ.get("TSFP_GPU_SHARED_CACHE")
    if override is not None:  # "" turns the shared cache off (tests, offline captures)
        return Path(override) if override else None
    result = subprocess.run(
        ["git", "-C", str(base), "rev-parse", "--git-common-dir"],
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode:
        return None
    return (base / result.stdout.strip()).resolve().parent / SHARED_GPU_REL


def _overlay(shared: Path, target: Path) -> None:
    """Seed a private modules copy with every shared file it does not already have."""
    for item in shared.rglob("*"):
        if item.is_file():
            destination = target / item.relative_to(shared)
            if not destination.exists():
                destination.parent.mkdir(parents=True, exist_ok=True)
                _copy_file(str(item), str(destination))


def _publish(target: Path, shared: Path) -> None:
    """Merge a finished run's new modules into the shared cache, atomically and size-bounded.

    Module names embed a content hash, so a name is written once. Pipeline-cache blobs are
    replaced only by a larger one (the driver validates the header, so a stale blob is harmless).
    Every write is temp file + rename: concurrent runs cannot corrupt a file."""
    try:
        total = sum(f.stat().st_size for f in shared.rglob("*") if f.is_file())
    except OSError:
        total = 0
    for item in sorted(target.rglob("*")):
        if not item.is_file():
            continue
        relative = item.relative_to(target)
        destination = shared / relative
        is_blob = relative.parts[0] == "pipeline-cache"
        try:
            size = item.stat().st_size
            if destination.exists() and not (is_blob and size > destination.stat().st_size):
                continue
            if total + size > SHARED_GPU_MAX_BYTES:
                continue
            destination.parent.mkdir(parents=True, exist_ok=True)
            staging = destination.with_name(f".{destination.name}.{os.getpid()}.tmp")
            _copy_file(str(item), str(staging))
            os.replace(staging, destination)
            total += size
        except OSError:
            continue


def prepare(
    command: list[str],
    env: dict[str, str],
    out: Path,
    *,
    display: int | None = None,
    headless: bool = True,
    cwd: Path | None = None,
) -> tuple[list[str], dict[str, str]]:
    """Lease output, snapshot capture inputs, and return a child-only environment.

    Call within launch_scope through the entire child/X server cleanup. Snapshots
    stay under out/.host-runs for diagnosis; never remove caller data on exit.
    """
    out = out.resolve()
    out.mkdir(parents=True, exist_ok=True)
    guard = _scope.get().enter_context((out / ".host-run.guard").open("a+"))
    try:
        fcntl.flock(guard, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except BlockingIOError as error:
        raise RuntimeError(f"host output already in use: {out}") from error
    # T3823: every local game-host launch holds a host-wide local game slot until the scope
    # closes (after child cleanup); interactive play (headless=False) is exempt but counted.
    from tools import local_game_slots

    _scope.get().enter_context(local_game_slots.launch_slot(command, interactive=not headless))
    parent = out / ".host-runs"
    parent.mkdir(exist_ok=True)
    root = Path(tempfile.mkdtemp(prefix="run-", dir=parent))
    result = list(command)
    base = cwd or Path.cwd()
    # T2319: interactive play writes the persistent HDD. Captures still snapshot it.
    # Shader modules remain private mutable inputs for both kinds of run.
    for flag, name in (("--hdd", "hdd"), ("--gpu-replay", "modules")):
        occurrences = [
            i for i, word in enumerate(result) if word == flag or word.startswith(flag + "=")
        ]
        for ordinal, index in enumerate(occurrences):
            inline = result[index].startswith(flag + "=")
            if not inline and index + 1 >= len(result):
                raise ValueError(f"missing value for {flag}")
            source = Path(result[index].split("=", 1)[1] if inline else result[index + 1])
            source = source if source.is_absolute() else base / source
            if flag == "--hdd" and not headless:
                source = source.resolve()
                guard_path = (
                    (source / ".tsfp-storage.guard")
                    if source.is_dir()
                    else source.with_name(source.name + ".tsfp-storage.guard")
                )
                storage_guard = _scope.get().enter_context(guard_path.open("a+"))
                try:
                    fcntl.flock(storage_guard, fcntl.LOCK_EX | fcntl.LOCK_NB)
                except BlockingIOError as error:
                    raise RuntimeError(f"PC save storage already in use: {source}") from error
                if inline:
                    result[index] = flag + "=" + str(source)
                else:
                    result[index + 1] = str(source)
                continue
            target = root / (name if ordinal == 0 else f"{name}-{ordinal}")
            _copy(source, target)
            if flag == "--gpu-replay" and (shared := shared_gpu_cache(base)) is not None:
                shared.mkdir(parents=True, exist_ok=True)
                _overlay(shared, target)
                _scope.get().callback(_publish, target, shared)
            if inline:
                result[index] = flag + "=" + str(target)
            else:
                result[index + 1] = str(target)
    child_env = dict(env)
    slot = _slot.get()
    if slot is not None:
        child_env["TSFP_HOST_SLOT"] = str(slot)
        devices = env.get("TSFP_HOST_GPU_DEVICES", "").split(",")
        if devices != [""]:
            if slot >= len(devices) or not devices[slot].strip():
                raise ValueError("TSFP_HOST_GPU_DEVICES must provide a device for every slot")
            child_env["DRI_PRIME"] = devices[slot].strip()

    runtime, cache = root / "runtime", root / "cache"
    runtime.mkdir(mode=0o700)
    cache.mkdir(mode=0o700)
    child_env.update(
        XDG_RUNTIME_DIR=str(runtime) if headless else env.get("XDG_RUNTIME_DIR", str(runtime)),
        XDG_CACHE_HOME=str(cache),
        MESA_SHADER_CACHE_DIR=str(cache / "mesa"),
        __GL_SHADER_DISK_CACHE_PATH=str(cache / "nvidia"),
    )
    if headless:
        child_env["SDL_AUDIODRIVER"] = "dummy"
        # Bevy/rodio uses ALSA rather than SDL on this Linux build. An empty
        # private configuration exposes no default PCM; the existing Bevy
        # no-device fallback drains its ring on the discard clock.
        alsa = root / "alsa.conf"
        alsa.write_text("# T2253 headless: no ALSA PCM devices\n")
        alsa.chmod(0o600)
        child_env["ALSA_CONFIG_PATH"] = str(alsa)
        # Independent Vulkan/wgpu contexts are created by separate processes;
        # inherited device/ICD selection stays explicit, never reassigned silently.
        # DMTCP strips launcher-only DMTCP_COORD_PORT before starting the
        # captured process. DMTCP_VIRTUAL_PID is its in-process marker; an
        # intervening `nice` exec drops the host's DMTCP attachment.
        if not env.get("DMTCP_VIRTUAL_PID"):
            result = ["nice", "-n", "10", *result]
    if display is not None:
        authority = root / "Xauthority"
        # Xauthority FamilyWild record. Private cookie and file, no global env mutation.
        # T2257: python-xlib (rust_trace send_close) matches only FamilyLocal (256) by
        # hostname, not FamilyWild: write the cookie under both families or the close
        # request fails and the host is SIGKILLed.
        cookie = os.urandom(16)
        records = (
            (256, socket.gethostname().encode()),
            (65535, b""),
        )
        authority.write_bytes(
            b"".join(
                struct.pack("!H", family)
                + b"".join(
                    struct.pack("!H", len(field)) + field
                    for field in (address, str(display).encode(), b"MIT-MAGIC-COOKIE-1", cookie)
                )
                for family, address in records
            )
        )
        authority.chmod(0o600)
        child_env.update(DISPLAY=f":{display}", XAUTHORITY=str(authority))
    return result, child_env


def prepare_headless(
    command: list[str],
    env: dict[str, str],
    out: Path,
    *,
    cwd: Path | None = None,
) -> tuple[list[str], dict[str, str]]:
    """Give a headless window its own X server; non-window hosts need no display."""
    window = any(
        word == "--present=window"
        or (word == "--present" and i + 1 < len(command) and command[i + 1] == "window")
        for i, word in enumerate(command)
    )
    window = window or any(Path(word).name == "tsfp-bevy" for word in command)
    if not window:
        return prepare(command, env, out, cwd=cwd)
    from tools.t982_acceptance import pick_display, stop_xvfb
    from tools.wt.display import release_number

    number = pick_display()
    # If preparation/spawn fails, release the number; a successful spawn adds
    # reaping first (LIFO), so admission outlives the server and its files.
    _scope.get().callback(release_number, number)
    command, env = prepare(command, env, out, display=number, cwd=cwd)
    server = subprocess.Popen(
        [
            "Xvfb",
            f":{number}",
            "-auth",
            env["XAUTHORITY"],
            "-nolisten",
            "tcp",
            "-screen",
            "0",
            "1280x720x24",
        ],
        env=env,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    _scope.get().callback(stop_xvfb, server, number)
    time.sleep(2)
    if server.poll() is not None:
        raise RuntimeError("private Xvfb exited before host launch")
    return command, env


def track_process(process: subprocess.Popen) -> subprocess.Popen:
    """Reap an owned session leader even if logging/stream setup raises."""

    def reap() -> None:
        if process.poll() is None:
            os.killpg(process.pid, signal.SIGTERM)
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                os.killpg(process.pid, signal.SIGKILL)
                process.wait()

    _scope.get().callback(reap)
    return process
