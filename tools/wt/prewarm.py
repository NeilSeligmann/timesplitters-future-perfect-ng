"""T3953: prebuilt hosts for every new origin/main tip, built before a lane asks for them.

Lanes stalled on "prebuilt for capture commit missing on laptop": a capture or gate at a fresh
commit found no host and either failed or rebuilt locally. `python3 -m tools.wt.prewarm --loop`
(supervised, tools/data/loops.json) builds each profile of `tools/data/prewarm_profiles.json`
for the tip through the existing remote machinery (`tools.wt prebuilt build --remote auto
--remote-only`: LAN hosts first, per-commit retention `--keep`). One build per (build-input hash,
profile): a docs-only tip, a published host, a live flight or a recent failure is skipped, and a
tip that moves while a profile builds drops the remaining profiles for the old tip (latest wins).

`wait_for_host` is the consumer side: a gate whose exact inputs have a build in flight (ours or
a lane's) waits for it instead of failing or compiling again.
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
import time
from collections.abc import Callable, Sequence
from dataclasses import dataclass, field
from pathlib import Path

PROFILES_FILE = Path(__file__).resolve().parents[2] / "tools" / "data" / "prewarm_profiles.json"
LOOP_NAME = "prebuilt-prewarm"
STATE_NAME = ".prewarm.json"
RETRY_FAILED_S = 1800.0
BUILD_TIMEOUT_S = 3600.0
KEEP = 12
STATE_KEEP = 200


def profiles(path: Path = PROFILES_FILE) -> list[str]:
    names = json.loads(path.read_text()).get("profiles", [])
    if not names or not all(isinstance(n, str) for n in names):
        raise ValueError(f"{path}: `profiles` must be a non-empty list of names")
    return list(dict.fromkeys(names))


@dataclass
class Ports:
    """Everything a tick touches outside itself; tests pass fakes (rule 39)."""

    tip: Callable[[], str | None]
    wanted: Callable[[str, str], str]  # (sha, profile) -> input hash
    present: Callable[[str, str], bool]  # (sha, profile) -> published host exists
    inflight: Callable[[str], bool]  # input hash -> live flight
    build: Callable[[str, str], int]  # (sha, profile) -> exit code
    now: Callable[[], float] = time.time
    state: dict = field(default_factory=dict)


def load_state(path: Path) -> dict:
    try:
        data = json.loads(path.read_text())
    except (OSError, ValueError):
        return {}
    return data if isinstance(data, dict) else {}


def save_state(path: Path, state: dict) -> None:
    rows = sorted(state.items(), key=lambda kv: kv[1].get("at", 0))[-STATE_KEEP:]
    temporary = path.with_name(f".{path.name}.tmp")
    temporary.write_text(json.dumps(dict(rows), sort_keys=True) + "\n")
    temporary.replace(path)


def tick(names: Sequence[str], ports: Ports) -> list[str]:
    """Build what the current tip lacks. Returns one log line per decision."""
    out: list[str] = []
    sha = ports.tip()
    if sha is None:
        return ["skip: origin/main unavailable locally"]
    for name in names:
        if ports.tip() != sha:
            out.append(f"{sha[:12]}: tip moved, dropping remaining profiles")
            break
        wanted = ports.wanted(sha, name)
        key = f"{wanted}:{name}"
        if ports.present(sha, name):
            out.append(f"{sha[:12]} {name}: cached ({wanted[:12]})")
            continue
        if ports.inflight(wanted):
            out.append(f"{sha[:12]} {name}: build in flight ({wanted[:12]})")
            continue
        prior = ports.state.get(key)
        if prior and prior.get("status") == "failed":
            if ports.now() - prior.get("at", 0) < RETRY_FAILED_S:
                out.append(f"{sha[:12]} {name}: failed recently, retry later")
                continue
        ports.state[key] = {"status": "building", "sha": sha, "at": ports.now()}
        code = ports.build(sha, name)
        status = "ready" if code == 0 else "failed"
        ports.state[key] = {"status": status, "sha": sha, "at": ports.now(), "exit": code}
        out.append(f"{sha[:12]} {name}: {status} (exit {code})")
    return out


def wait_for_host(
    find: Callable[[], Path | None],
    inflight: Callable[[], bool],
    *,
    timeout_s: float = BUILD_TIMEOUT_S,
    poll_s: float = 15.0,
    sleep: Callable[[float], None] = time.sleep,
    now: Callable[[], float] = time.monotonic,
) -> Path | None:
    """Host for these exact inputs, waiting while a build of them is in flight; None otherwise."""
    deadline = now() + timeout_s
    while True:
        host = find()
        if host is not None:
            return host
        if not inflight() or now() >= deadline:
            return None
        sleep(poll_s)


def real_ports(root: Path, cache: Path) -> Ports:
    from tools.wt import prebuilt

    state = load_state(cache / STATE_NAME)

    def build(sha: str, name: str) -> int:
        argv = [sys.executable, "-m", "tools.wt", "prebuilt", "build", "--worktree", str(root)]
        argv += ["--commit", sha, "--profile", name, "--keep", str(KEEP)]
        argv += ["--remote", "auto", "--remote-only"]
        try:
            return subprocess.run(argv, cwd=root, timeout=BUILD_TIMEOUT_S, check=False).returncode
        except subprocess.TimeoutExpired:
            return 124

    return Ports(
        tip=lambda: prebuilt._origin_main(root),
        wanted=lambda sha, name: prebuilt.input_hash(root, sha, name),
        present=lambda sha, name: prebuilt.lookup(root, sha, cache, name) is not None,
        inflight=lambda wanted: prebuilt.read_inflight(cache, wanted) is not None,
        build=build,
        state=state,
    )


def loop_alive(root: Path) -> bool:
    from tools.fleet.core import flock_held

    return flock_held(root / "tmp" / "prewarm" / "prewarm.lock")


def kill_switch(root: Path) -> Path:
    return root / "tmp" / "prewarm" / "prewarm.off"


def loop_stopped(root: Path) -> bool:
    return kill_switch(root).exists()


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--loop", action="store_true")
    parser.add_argument("--interval", type=float, default=120)
    parser.add_argument("--worktree", type=Path, default=Path.cwd())
    args = parser.parse_args(argv)
    from tools.agents import loops
    from tools.fleet.core import SingleInstance
    from tools.wt import prebuilt

    root = args.worktree.resolve()
    main_dir = prebuilt.main_root(root)
    cache = prebuilt.cache_root(root)
    names = profiles()
    lock = SingleInstance(main_dir / "tmp" / "prewarm" / "prewarm.lock")
    if args.loop and not lock.acquire():
        print("prewarm already running", file=sys.stderr)
        return 1
    if args.loop:
        loops.install_term_handler()
    while True:
        if kill_switch(main_dir).exists():
            return 0
        ports = real_ports(root, cache)
        for line in tick(names, ports):
            print(line, flush=True)
        cache.mkdir(parents=True, exist_ok=True)
        save_state(cache / STATE_NAME, ports.state)
        if args.loop:
            loops.tick(LOOP_NAME, main_dir)
            time.sleep(args.interval)
        else:
            return 0


if __name__ == "__main__":
    raise SystemExit(main())
