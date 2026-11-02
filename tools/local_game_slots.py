"""Host-wide cap on LOCAL game-host launches (T3823, incident class local-host-fanout).

Incident 2026-10-27 (MEASURED): a subagent started 5-10 paced captures at once with
`TSFP_HOSTLOCK=off python -m tools.rust_trace capture --host .../tsfp-bevy`. Each host took about
5 cores (software GL on Xvfb); the owner PC reached load 136 with 21 GB of swap.

Hard rule (rules 26, 36, 45), one owner for "may this process start a game host here":

* every local launch of tsfp-bevy or the C host (`tsfp_host`) takes a flock slot
  `tmp/locks/local-game-slot-<k>` on the main checkout, `owner_pc_limit` slots
  (tools/data/local_game_hosts.json, default 1); flock is visible from every container that
  shares the disk and is freed by the kernel when the holder dies;
* `TSFP_HOSTLOCK=off` and `TSFP_HOST_SLOTS` only steer tools.hostlock's queue, never this cap;
* a waiter logs every `log_every_s` and names the holders; `python3 -m tools.wt replay submit`
  routes remote-first instead of waiting here;
* interactive owner play (`sh tools/dev/play.sh`, the T2233 window path) never waits: it takes
  a `local-game-play-<k>` lock instead, which counts as an occupied slot for everyone else;
* remote execution hosts (no git checkout, rule 47, or TSFP_DISK_PROFILE=remote_worker) are not
  the owner PC: their admission stays with tools.hostlock and the remote replay probe.

The shared spawn function tools.hostresources.prepare calls `launch_slot` for every host argv,
so a new runner built on it is covered without code of its own. Direct spawners (they start
their own Xvfb, see tests/test_local_game_slots.py) wrap their launch in `hold()`.
`python3 -m tools.local_game_slots status` prints holders and running hosts.
"""

from __future__ import annotations

import argparse
import contextlib
import contextvars
import fcntl
import functools
import json
import os
import re
import sys
import time
from collections.abc import Callable, Iterator
from pathlib import Path
from typing import ParamSpec, TypeVar

ROOT = Path(__file__).resolve().parents[1]
CONFIG = ROOT / "tools" / "data" / "local_game_hosts.json"
DEFAULTS: dict[str, object] = {
    "owner_pc_limit": 1,
    "lock_dir": "tmp/locks",
    "log_every_s": 30,
    "host_names": ["tsfp-bevy", "tsfp_host"],
    "not_a_game_args": ["--t2-offline"],
}
WRAPPERS = frozenset(
    ("timeout", "nice", "env", "setsid", "stdbuf", "ionice", "prlimit", "chrt", "dmtcp_launch")
)
DURATION = re.compile(r"^\d+(\.\d+)?[smhd]?$")
ASSIGNMENT = re.compile(r"^[A-Za-z_][A-Za-z0-9_]*=")
P = ParamSpec("P")
R = TypeVar("R")
_held: contextvars.ContextVar[int] = contextvars.ContextVar("local_game_slot_held", default=0)


def load_config(path: Path = CONFIG) -> dict[str, object]:
    """Tracked config over the defaults; a broken file degrades loudly to the defaults."""
    try:
        data = json.loads(path.read_text())
    except (OSError, ValueError) as error:
        print(f"local game slots: {path} unreadable ({error}), using defaults", file=sys.stderr)
        data = {}
    return {**DEFAULTS, **{k: v for k, v in data.items() if k in DEFAULTS}}


def limit(cfg: dict[str, object]) -> int:
    value = cfg["owner_pc_limit"]
    if type(value) is not int or value < 1:
        raise ValueError("local_game_hosts.json owner_pc_limit must be a positive integer")
    return value


def is_owner_pc(root: Path = ROOT, env: dict[str, str] | None = None) -> bool:
    """Only the owner PC holds the git checkout (rule 47); remote exec trees are rsync'd."""
    env = dict(os.environ) if env is None else env
    if env.get("TSFP_DISK_PROFILE") == "remote_worker":
        return False
    return any((parent / ".git").exists() for parent in (root, *root.parents))


def main_dir(root: Path = ROOT) -> Path:
    """The main checkout, shared by every worktree (tools.datadir owns the lookup)."""
    from tools.datadir import main_root

    return main_root(root)


def host_executable(command: list[str], cfg: dict[str, object] | None = None) -> str | None:
    """The game-host executable a command starts, after wrappers; None for any other command.

    Only the executable position counts: `python -m tools.rust_trace capture --host X/tsfp-bevy`
    is a launcher, not a host, so a wrapper never takes the slot its inner launch needs."""
    cfg = cfg or load_config()
    names = [str(name) for name in cfg["host_names"]]  # type: ignore[union-attr]
    skip = [str(arg) for arg in cfg["not_a_game_args"]]  # type: ignore[union-attr]
    for word in command:
        base = Path(word).name
        if base in WRAPPERS or word.startswith("-") or DURATION.match(word):
            continue
        if ASSIGNMENT.match(word):
            continue
        if any(base.startswith(name) for name in names):
            return None if any(arg in command for arg in skip) else base
        return None
    return None


def _try_lock(path: Path) -> int | None:
    fd = os.open(path, os.O_RDWR | os.O_CREAT, 0o666)
    try:
        fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except OSError:
        os.close(fd)
        return None
    return fd


def _busy(directory: Path, prefix: str, count: int) -> list[Path]:
    busy = []
    for k in range(count):
        path = directory / f"{prefix}-{k}"
        fd = _try_lock(path)
        if fd is None:
            busy.append(path)
        else:
            os.close(fd)
    return busy


def _plays(directory: Path) -> list[Path]:
    """Held interactive-play locks (any number of owner windows)."""
    busy = []
    for path in sorted(directory.glob("local-game-play-*")):
        fd = _try_lock(path)
        if fd is None:
            busy.append(path)
        else:
            os.close(fd)
    return busy


def _stamp(fd: int, what: str) -> None:
    """Holder facts for the waiters' log line (diagnostics only)."""
    record = {"pid": os.getpid(), "since": round(time.time()), "what": what[:160]}
    with contextlib.suppress(OSError):
        os.ftruncate(fd, 0)
        os.pwrite(fd, (json.dumps(record) + "\n").encode(), 0)


def _holder(path: Path) -> str:
    try:
        record = json.loads(path.read_text() or "{}")
    except (OSError, ValueError):
        return path.name
    age = max(0, round(time.time() - float(record.get("since", time.time()))))
    return f"{path.name} pid {record.get('pid')} {age} s ({record.get('what', '')})"


def acquire(
    directory: Path,
    count: int,
    *,
    what: str = "",
    interactive: bool = False,
    log: Callable[[str], object] | None = None,
    log_every_s: float = 30.0,
    clock: Callable[[], float] = time.monotonic,
    sleep: Callable[[float], object] = time.sleep,
    timeout: float | None = None,
) -> int:
    """Block until a local game slot is free; return the locked fd (close it to release).

    `interactive` (owner play) never waits: it locks a new play file, counted by every waiter."""
    log = log or (lambda text: print(text, file=sys.stderr, flush=True))
    directory.mkdir(parents=True, exist_ok=True)
    if interactive:
        k = 0
        while True:
            fd = _try_lock(directory / f"local-game-play-{k}")
            if fd is not None:
                _stamp(fd, what or "interactive play")
                return fd
            k += 1
    start = clock()
    last_said: float | None = None
    while True:
        for k in range(count):
            fd = _try_lock(directory / f"local-game-slot-{k}")
            if fd is None:
                continue
            # Our own slot shows as busy too (flock is per open file description).
            occupied = len(_plays(directory)) + len(_busy(directory, "local-game-slot", count))
            if occupied <= count:
                _stamp(fd, what)
                return fd
            os.close(fd)
            break
        now = clock()
        if timeout is not None and now - start >= timeout:
            raise TimeoutError(f"no local game slot within {timeout:g} s")
        if last_said is None or now - last_said >= log_every_s:
            holders = [
                _holder(p)
                for p in [*_busy(directory, "local-game-slot", count), *_plays(directory)]
            ]
            log(
                f"local game slot: waiting {now - start:.0f} s, owner-PC limit {count} "
                f"(tools/data/local_game_hosts.json), held by: {'; '.join(holders) or 'racing'}; "
                "TSFP_HOSTLOCK=off does not skip this cap, use `python3 -m tools.wt replay "
                "submit` to run remote-first"
            )
            last_said = now
        sleep(1.0)


@contextlib.contextmanager
def hold(
    what: str = "",
    *,
    interactive: bool = False,
    root: Path = ROOT,
    cfg: dict[str, object] | None = None,
    **kwargs: object,
) -> Iterator[None]:
    """Hold a local game slot for the block; re-entrant in one context, a no-op off the owner PC."""
    if _held.get() or not is_owner_pc(root):
        yield
        return
    cfg = cfg or load_config()
    directory = main_dir(root) / str(cfg["lock_dir"])
    with contextlib.ExitStack() as stack:
        # T3885: a game launch holds a dispatcher placement (interactive owner play: exempt)
        if not interactive:
            from tools.dispatch import enforce

            stack.enter_context(
                enforce.placement(
                    "game",
                    enforce.GAME_CORES,
                    label=f"game {what}"[:80],
                    reason="local game host launch",
                    root=root,
                )
            )
        with _slot(directory, cfg, what, interactive, kwargs):
            yield


@contextlib.contextmanager
def _slot(
    directory: Path,
    cfg: dict[str, object],
    what: str,
    interactive: bool,
    kwargs: dict[str, object],
) -> Iterator[None]:
    fd = acquire(
        directory,
        limit(cfg),
        what=what,
        interactive=interactive,
        log_every_s=float(cfg["log_every_s"]),  # type: ignore[arg-type]
        **kwargs,  # type: ignore[arg-type]
    )
    token = _held.set(_held.get() + 1)
    try:
        yield
    finally:
        _held.reset(token)
        os.close(fd)


def holding(what: str) -> Callable[[Callable[P, R]], Callable[P, R]]:
    """Decorator for a direct spawner: the whole function runs under `hold(what)`."""

    def wrap(function: Callable[P, R]) -> Callable[P, R]:
        @functools.wraps(function)
        def wrapped(*args: P.args, **kwargs: P.kwargs) -> R:
            with hold(what):
                return function(*args, **kwargs)

        return wrapped

    return wrap


def launch_slot(
    command: list[str], *, interactive: bool = False, root: Path = ROOT
) -> contextlib.AbstractContextManager[None]:
    """The slot for one spawn: a hold for a game host argv, nothing for any other command."""
    cfg = load_config()
    if host_executable(command, cfg) is None:
        return contextlib.nullcontext()
    return hold(" ".join(command)[:160], interactive=interactive, root=root, cfg=cfg)


def running_hosts(proc: Path = Path("/proc"), cfg: dict[str, object] | None = None) -> list[int]:
    """Live game hosts of this PID namespace (renamed copies too, offline Tier 2 runs excluded)."""
    cfg = cfg or load_config()
    names = [str(name) for name in cfg["host_names"]]  # type: ignore[union-attr]
    skip = [str(arg) for arg in cfg["not_a_game_args"]]  # type: ignore[union-attr]
    found = []
    for entry in proc.iterdir():
        if not entry.name.isdigit():
            continue
        try:
            state = (entry / "stat").read_text().rsplit(")", 1)[1].split()[0]
            argv = (entry / "cmdline").read_bytes().split(b"\0")
        except (OSError, IndexError):
            continue
        if state in ("Z", "X") or not argv or not argv[0]:
            continue
        words = [word.decode(errors="replace") for word in argv if word]
        base = Path(words[0]).name
        if any(base.startswith(name) for name in names) and not any(a in words for a in skip):
            found.append(int(entry.name))
    return sorted(found)


def over_limit(
    root: Path, proc: Path = Path("/proc"), cfg: dict[str, object] | None = None
) -> str | None:
    """HEALTH detail when more game hosts run than the limit plus the exempt owner plays."""
    cfg = cfg or load_config()
    if not is_owner_pc(root):
        return None
    hosts = running_hosts(proc, cfg)
    directory = root / str(cfg["lock_dir"])
    plays = len(_plays(directory)) if directory.is_dir() else 0
    allowed = limit(cfg) + plays
    if len(hosts) <= allowed:
        return None
    shown = ", ".join(str(pid) for pid in hosts[:8])
    return (
        f"{len(hosts)} > limit {allowed} (owner-PC limit {limit(cfg)} + {plays} interactive "
        f"play); pids {shown}; tools/local_game_slots.py, docs/incidents/I-T3823-1.md"
    )


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.split("\n", 1)[0])
    parser.add_argument("command", choices=["status"])
    parser.parse_args()
    cfg = load_config()
    directory = main_dir() / str(cfg["lock_dir"])
    directory.mkdir(parents=True, exist_ok=True)
    print(f"owner PC: {is_owner_pc()}, limit {limit(cfg)}, locks in {directory}")
    for path in [*_busy(directory, "local-game-slot", limit(cfg)), *_plays(directory)]:
        print(f"held: {_holder(path)}")
    print(f"running hosts: {running_hosts(cfg=cfg)}")
    print(f"detector: {over_limit(main_dir(), cfg=cfg) or 'ok'}")


if __name__ == "__main__":
    main()
