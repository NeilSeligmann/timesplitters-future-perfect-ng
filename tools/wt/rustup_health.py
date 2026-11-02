"""Detect accidental rustup use or mutation of the host-default toolchain home."""

from __future__ import annotations

import hashlib
import json
import os
from dataclasses import dataclass
from pathlib import Path

STATE_REL = Path("tmp/wt-cache/rustup-health.json")  # T3807: scratch.run_outputs reads it
STATE_VERSION = 2  # T3834: names plus rustc hash; an older state never fires a change
RUST_EXECUTABLES = frozenset({"cargo", "rustc", "rustup", "rustdoc"})
PARENT_DEPTH = 4
COMMAND_CHARS = 160


def _rustc_digest(rustc: Path, cache: dict[str, list], name: str) -> str:
    """sha256 of a toolchain's bin/rustc, reused while its (mtime, size, inode) is unchanged."""
    try:
        stat = rustc.stat()
    except OSError:
        return "missing"
    key = [stat.st_mtime_ns, stat.st_size, stat.st_ino]
    cached = cache.get(name)
    if isinstance(cached, list) and len(cached) == 4 and cached[:3] == key:
        return str(cached[3])
    digest = hashlib.sha256()
    try:
        with rustc.open("rb") as stream:
            for block in iter(lambda: stream.read(1 << 20), b""):
                digest.update(block)
    except OSError:
        return "unreadable"
    cache[name] = [*key, digest.hexdigest()]
    return digest.hexdigest()


def _fingerprint(toolchains: Path, cache: dict[str, list] | None = None) -> str:
    """T3834: toolchain directory names and each toolchain's rustc hash, nothing else.

    The old fingerprint hashed every entry's mtime and size, so rustup's own bookkeeping
    (lock files, update-hash touches) raised `rustup-default-toolchains-changed`."""
    cache = {} if cache is None else cache
    try:
        names = sorted(p.name for p in toolchains.iterdir() if p.is_dir())
    except OSError:
        names = []
    rows = [
        (name, _rustc_digest(toolchains / name / "bin" / "rustc", cache, name)) for name in names
    ]
    for stale in set(cache) - set(names):
        cache.pop(stale, None)
    return hashlib.sha256(json.dumps(rows, separators=(",", ":")).encode()).hexdigest()


@dataclass(frozen=True)
class DefaultHomeUse:
    pid: str
    command: str
    parents: tuple[str, ...]

    def describe(self) -> str:
        chain = " <- ".join(self.parents) if self.parents else "no parent"
        return f"pid {self.pid} `{self.command}` parents: {chain}"


def _read(path: Path, limit: int) -> bytes:
    with path.open("rb") as stream:
        return stream.read(limit)


def _argv(pid_dir: Path) -> list[str]:
    raw = _read(pid_dir / "cmdline", 8192).decode(errors="ignore")
    return [arg for arg in raw.split("\0") if arg]


def _command_text(pid_dir: Path) -> str:
    try:
        text = " ".join(_argv(pid_dir))
    except OSError:
        text = ""
    return (text or "?")[:COMMAND_CHARS]


def _is_rust_executable(pid_dir: Path, argv: list[str]) -> bool:
    """T3834: argv[0] or /proc/<pid>/exe basename is a rust tool; a `bash -c '... cargo'`
    wrapper only names cargo inside its command and never matches."""
    if argv and Path(argv[0]).name in RUST_EXECUTABLES:
        return True
    try:
        return Path(os.readlink(pid_dir / "exe")).name in RUST_EXECUTABLES
    except OSError:
        return False


def _parent_pid(pid_dir: Path) -> str | None:
    try:
        status = _read(pid_dir / "status", 4096).decode(errors="ignore")
    except OSError:
        return None
    for line in status.splitlines():
        if line.startswith("PPid:"):
            value = line.partition(":")[2].strip()
            return value if value.isdigit() and value != "0" else None
    return None


def _parent_chain(proc_root: Path, pid_dir: Path) -> tuple[str, ...]:
    chain: list[str] = []
    current = pid_dir
    for _ in range(PARENT_DEPTH):
        parent = _parent_pid(current)
        if parent is None:
            break
        current = proc_root / parent
        chain.append(f"{parent} `{_command_text(current)}`")
    return tuple(chain)


def _default_home_in_use(proc_root: Path, default_home: Path) -> DefaultHomeUse | None:
    expected = str(default_home.resolve())
    try:
        pid_dirs = sorted(
            (p for p in proc_root.iterdir() if p.name.isdigit()), key=lambda p: int(p.name)
        )
    except OSError:
        return None
    for pid_dir in pid_dirs:
        try:
            argv = _argv(pid_dir)
            if not _is_rust_executable(pid_dir, argv):
                continue
            environ = _read(pid_dir / "environ", 65536).decode(errors="ignore").split("\0")
        except (OSError, ProcessLookupError, PermissionError):
            continue
        setting = next((x.partition("=")[2] for x in environ if x.startswith("RUSTUP_HOME=")), "")
        if (str(Path(setting).resolve()) if setting else expected) == expected:
            return DefaultHomeUse(
                pid_dir.name, _command_text(pid_dir), _parent_chain(proc_root, pid_dir)
            )
    return None


def check(
    main: Path,
    home: Path | None = None,
    proc_root: Path = Path("/proc"),
) -> list[str]:
    """Emit HEALTH records for default-home use and changes to ~/.rustup/toolchains."""
    default_home = (home or Path.home()) / ".rustup"
    messages: list[str] = []
    use = _default_home_in_use(proc_root, default_home)
    if use is not None:
        messages.append(
            f"HEALTH rustup-default-home-used RUSTUP_HOME resolves to ~/.rustup: {use.describe()}"
        )
    state = main / STATE_REL
    try:
        saved = json.loads(state.read_text())
        saved = saved if isinstance(saved, dict) else {}
    except (OSError, ValueError):
        saved = {}
    same_version = saved.get("version") == STATE_VERSION
    cache = saved.get("rustc", {}) if same_version and isinstance(saved.get("rustc"), dict) else {}
    current = _fingerprint(default_home / "toolchains", cache)
    previous = saved.get("toolchains") if same_version else None
    if previous is not None and previous != current:
        messages.append(
            "HEALTH rustup-default-toolchains-changed ~/.rustup/toolchains changed "
            "(a toolchain added, removed or its rustc replaced)"
        )
    try:
        state.parent.mkdir(parents=True, exist_ok=True)
        temp = state.with_name(f"{state.name}.{os.getpid()}.tmp")
        body = {"version": STATE_VERSION, "toolchains": current, "rustc": cache}
        temp.write_text(json.dumps(body) + "\n")
        temp.replace(state)
    except OSError:
        pass
    # I-T3745-main-red: HEALTH lines go to the fleet health log, never to the caller's stderr
    # (gate.main and other tools assert their exact stderr).
    if messages and os.environ.get("TSFP_RUSTUP_HEALTH_LOG", "1") != "0":
        log = Path(os.environ.get("TSFP_HEALTH_LOG") or main / "tmp/codexapi/health.log")
        try:
            stamp = __import__("time").strftime("%H:%M:%S", __import__("time").gmtime())
            with log.open("a") as handle:
                handle.writelines(f"{stamp} {message}\n" for message in messages)
        except OSError:
            pass
    return messages
