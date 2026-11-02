"""T3472: one shared sccache backend for the LAN hosts (local container, laptop, Deck, server).

The backend is a WebDAV store (`rclone serve webdav`, pinned) on the server, bound to the
server container's 127.0.0.1 only and protected by a password (htpasswd). The server
container exposes nothing but sshd, so clients reach it through an SSH local forward
(127.0.0.1:PORT -> server 127.0.0.1:PORT): the network gate is SSH key auth, the WebDAV
password is the second factor. The Deck uses a dedicated key that the server restricts to
this one forward (`restrict,port-forwarding,permitopen=...`). The droplet (public cloud,
no LAN route) keeps its own local sccache.

Builds use the shared store through a SEPARATE sccache daemon (SCCACHE_SERVER_UDS socket),
so the old local-disk daemon keeps serving builds that started before; the password is read
from a host-local file by tools/wt/sccache-rustc.sh at compile time and is never part of an
env script, a command line or a log. When the tunnel cannot be opened the build silently
uses the local-disk sccache (no build ever fails because the cache is away).

Retention: sccache does not evict remote stores, so `supervise` on the server prunes the
store oldest-mtime first to CAP_GB (owner 2026-10-21: up to ~60 GB inside the server's
100 GB tsfp cap; capguard counts but never deletes it).

This file runs standalone with python3 on the server (stdlib only, no tools.* imports at
module level):

    python3 -m tools.wt.sccache_shared status          # local: tunnel, endpoint, store size
    python3 -m tools.wt.sccache_shared tunnel          # local/laptop/Deck: open the forward
    python3 sccache_shared.py supervise --dir DIR      # server: rclone + prune loop
    python3 sccache_shared.py prune --dir DIR --cap-gb 60 [--apply]
"""

from __future__ import annotations

import argparse
import json
import math
import os
import socket
import subprocess
import sys
import time
from collections.abc import Callable
from pathlib import Path
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from tools.wt.remote_exec import HostSpec

PORT = 4230  # default for registries predating per-store sccache_port
USER = "tsfp"
KEY_PREFIX = "tsfp-v1"
CAP_GB = 60.0
LOW_WATER = 0.9
SERVER_DIR = "~/rea-codex2/sccache-shared"  # REA: counted by capguard, never deleted by it
RCLONE_VERSION = "v1.68.2"
PASS_NAME = "sccache-webdav.pass"
HOST_PASS = Path("~/.config/tsfp") / PASS_NAME
TUNNEL_KEY = Path("~/.ssh/tsfp-sccache")
DISABLE_ENV = "TSFP_SCCACHE_SHARED"
STORE_PORT_ENV = "TSFP_SCCACHE_STORE_PORT"
# Debian's sccache 0.10.0 is built without the WebDAV backend (MEASURED: it silently falls back
# to local disk), so the shared store needs the upstream musl release, pinned here.
SCCACHE_VERSION = "v0.10.0"
HOST_SCCACHE = Path("~/.local/lib/tsfp/sccache")


def pass_file(main: Path | None, env: dict[str, str] | None = None) -> Path | None:
    """The host-local password file: env override, main/.secrets, then ~/.config/tsfp."""
    env = os.environ if env is None else env
    candidates = []
    if env.get("TSFP_SCCACHE_PASS_FILE"):
        candidates.append(Path(env["TSFP_SCCACHE_PASS_FILE"]))
    if main is not None:
        candidates.append(main / ".secrets" / PASS_NAME)
    candidates.append(HOST_PASS.expanduser())
    for path in candidates:
        try:
            if path.is_file() and path.stat().st_size > 0:
                return path
        except OSError:
            continue
    return None


def sccache_bin(main: Path | None, env: dict[str, str] | None = None) -> Path | None:
    """The WebDAV-capable sccache: env override, main/tmp/rust/sccache, ~/.local/lib/tsfp."""
    env = os.environ if env is None else env
    candidates = [Path(env["TSFP_SCCACHE_BIN"])] if env.get("TSFP_SCCACHE_BIN") else []
    if main is not None:
        candidates.append(main / "tmp/rust/sccache/sccache")
    candidates.append(HOST_SCCACHE.expanduser())
    for path in candidates:
        if path.is_file() and os.access(path, os.X_OK):
            return path
    return None


def port_open(port: int = PORT, host: str = "127.0.0.1", timeout: float = 0.5) -> bool:
    try:
        with socket.create_connection((host, port), timeout=timeout):
            return True
    except OSError:
        return False


def webdav_responding(
    port: int | None = None,
    host: str = "127.0.0.1",
    timeout: float = 0.5,
    *,
    connect: Callable[..., socket.socket] = socket.create_connection,
) -> bool:
    """True only when the listener speaks HTTP; a stale SSH forward is not a store."""
    try:
        selected_port = store_port() if port is None else port
        with connect((host, selected_port), timeout=timeout) as connection:
            connection.sendall(b"GET / HTTP/1.0\r\nHost: localhost\r\n\r\n")
            return connection.recv(64).startswith(b"HTTP/")
    except OSError:
        return False


def store_host() -> HostSpec:
    """Resolve the single store through the fleet registry, never a copied SSH route."""
    from tools.wt import remote_exec

    hosts = [host for host in remote_exec.load_registry() if host.sccache == "server"]
    if len(hosts) != 1:
        raise ValueError("shared sccache requires exactly one registry store host")
    return hosts[0]


def store_cap_gib(host: HostSpec | None = None) -> float:
    """Resolve the active shared-store cap from its registry entry; default for older hosts."""
    selected = store_host() if host is None else host
    raw = getattr(selected, "sccache_store_cap_gib", None)
    cap = CAP_GB if raw is None else raw
    if (
        isinstance(cap, bool)
        or not isinstance(cap, (int, float))
        or not math.isfinite(cap)
        or cap <= 0
    ):
        raise ValueError(f"invalid sccache store cap for host {selected.name!r}")
    return float(cap)


def store_port(host: HostSpec | None = None) -> int:
    """Resolve the WebDAV endpoint port from the unique registry store host."""
    selected = store_host() if host is None else host
    raw = getattr(selected, "sccache_port", None)
    port = PORT if raw is None else raw
    if isinstance(port, bool) or not isinstance(port, int) or not 1 <= port <= 65535:
        raise ValueError(f"invalid sccache store port for host {selected.name!r}")
    return port


def tunnel_command(key: Path | None = None) -> list[str]:
    server = store_host()
    cmd = [
        "ssh",
        "-f",
        "-N",
        "-o",
        "BatchMode=yes",
        "-o",
        "ExitOnForwardFailure=yes",
        "-o",
        "ServerAliveInterval=30",
        "-o",
        "ServerAliveCountMax=3",
        "-o",
        "ConnectTimeout=5",
        "-o",
        "ControlMaster=no",
        "-o",
        "ControlPath=none",
    ]
    if key is not None:
        cmd += ["-i", str(key), "-o", "IdentitiesOnly=yes"]
    cmd += [
        "-L",
        f"127.0.0.1:{store_port()}:127.0.0.1:{store_port()}",
        "-p",
        str(server.port),
        f"{server.user}@{server.address}",
    ]
    return cmd


def reverse_command(
    address: str,
    port: int = 2222,
    user: str = "claude-user",
    store_port_value: int | None = None,
) -> list[str]:
    """From this host (which holds the forward), give `address` the store on its loopback.

    The Deck has no key on the server (its authorized_keys is owner-managed, not writable by
    agents), so the hub pushes the port to it: Deck 127.0.0.1:PORT -> here -> server. The same
    route works for public build hosts when this hub can SSH to their registered address."""
    selected_store_port = store_port() if store_port_value is None else store_port_value
    return [
        "ssh", "-f", "-N", "-o", "BatchMode=yes", "-o", "ExitOnForwardFailure=yes",
        "-o", "ServerAliveInterval=30", "-o", "ServerAliveCountMax=3", "-o", "ConnectTimeout=5",
        "-o", "ControlMaster=no", "-o", "ControlPath=none",
        "-o", "StrictHostKeyChecking=accept-new",
        "-R",
        f"127.0.0.1:{selected_store_port}:127.0.0.1:{selected_store_port}",
        "-p",
        str(port),
        f"{user}@{address}",
    ]  # fmt: skip


def ensure_tunnel(
    run: Callable[..., object] = subprocess.run, opened: Callable[[], bool] = webdav_responding
) -> bool:
    """Open the SSH forward unless the port already answers (also true on the server)."""
    from tools.wt import remote_replay

    try:
        if store_host().name in remote_replay.switched_off():
            return False
    except (OSError, ValueError):
        return False
    if opened():
        return True
    key = TUNNEL_KEY.expanduser()
    try:
        run(
            tunnel_command(key if key.is_file() else None),
            timeout=15,
            check=False,
            # T3684: `ssh -f` leaves a daemon with our cwd; from a worktree that pinned the tree
            # and `tools.wt remove` refused it (claude-1/s2, 2026-10-24).
            cwd="/",
            stdin=subprocess.DEVNULL,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
    except (OSError, subprocess.SubprocessError):
        return False
    for _ in range(10):
        if opened():
            return True
        time.sleep(0.2)
    return False


def client_env(
    main: Path,
    env: dict[str, str] | None = None,
    *,
    ensure: Callable[[], bool] = ensure_tunnel,
    check_pause: bool = True,
) -> dict[str, str]:
    """sccache settings for the shared store, or {} (local-disk sccache) when unavailable.

    Never contains the password, only the path of the file holding it."""
    env = os.environ if env is None else env
    if env.get(DISABLE_ENV, "1").lower() in ("0", "off", "no", "false"):
        return {}
    if check_pause:
        from tools.wt import remote_prebuilt

        if remote_prebuilt.worker_disabled(main, store_host().name):
            return {}  # An owner-paused worker must not be needed even as a cache service.
    secret = pass_file(main, env)
    binary = sccache_bin(main, env)
    if secret is None or binary is None or not ensure():
        return {}
    endpoint_port = env.get(STORE_PORT_ENV)
    port = int(endpoint_port) if endpoint_port is not None else store_port()
    if not 1 <= port <= 65535:
        return {}
    return {
        "TSFP_SCCACHE": str(binary),
        "SCCACHE_WEBDAV_ENDPOINT": f"http://127.0.0.1:{port}/",
        "SCCACHE_WEBDAV_USERNAME": USER,
        "SCCACHE_WEBDAV_KEY_PREFIX": KEY_PREFIX,
        "SCCACHE_SERVER_UDS": str(main / "tmp/wt-cache/sccache-shared.sock"),
        "TSFP_SCCACHE_PASS_FILE": str(secret),
    }


def start_server(env: dict[str, str], *, runner: Callable[..., object] = subprocess.run) -> bool:
    """T3472 phase 3: start the sccache daemon for `env` with NO inherited descriptors.

    A daemon the first rustc starts lazily inherits every open fd of the build, including the
    canonical-tree lock and the build-slot lock (the Deck's fd 9 bug, and locally a pool member
    that looked busy until the daemon idled out). The build starts it here first, through the
    wrapper (which reads the password file), as its own session, with close_fds. True when the
    wrapper is the shared-store one and the command ran."""
    wrapper = env.get("RUSTC_WRAPPER", "")
    if not wrapper.endswith("sccache-rustc.sh"):
        return False
    try:
        runner(
            ["sh", wrapper, "--start-server"],
            env=env,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            close_fds=True,
            start_new_session=True,
            cwd="/",  # T3684: a daemon must not pin the build worktree
            timeout=30,
            check=False,
        )
    except (OSError, subprocess.SubprocessError):
        return False
    return True


def store_files(root: Path) -> list[tuple[float, int, Path]]:
    rows = []
    for base, _dirs, files in os.walk(root):
        for name in files:
            path = Path(base) / name
            try:
                st = path.stat()
            except OSError:
                continue
            rows.append((st.st_mtime, st.st_size, path))
    return rows


def prune(root: Path, cap_gb: float = CAP_GB, apply: bool = False) -> dict[str, float]:
    """Delete oldest entries until the store is under LOW_WATER * cap (only when over cap)."""
    rows = store_files(root)
    total = sum(size for _mtime, size, _path in rows)
    cap = int(cap_gb * 1024**3)
    stats = {"files": len(rows), "gb": round(total / 1024**3, 2), "deleted": 0, "freed_gb": 0.0}
    if total <= cap:
        return stats
    goal = int(cap * LOW_WATER)
    freed = 0
    for _mtime, size, path in sorted(rows):
        if total - freed <= goal:
            break
        if apply:
            try:
                path.unlink()
            except OSError:
                continue
        freed += size
        stats["deleted"] += 1
    stats["freed_gb"] = round(freed / 1024**3, 2)
    return stats


def rclone_command(base: Path, port: int = PORT) -> list[str]:
    return [
        str(base / "bin/rclone"),
        "serve",
        "webdav",
        "--addr",
        f"127.0.0.1:{port}",
        "--htpasswd",
        str(base / "htpasswd"),
        "--log-file",
        str(base / "rclone.log"),
        "--log-level",
        "NOTICE",
        str(base / "data"),
    ]


def rclone_priority_command(
    base: Path,
    nice: int = 15,
    ionice_class: int = 2,
    ionice_level: int = 7,
    port: int = PORT,
) -> list[str]:
    """Wrap WebDAV with low CPU and best-effort I/O scheduling priority."""
    return [
        "ionice",
        "-c",
        str(ionice_class),
        "-n",
        str(ionice_level),
        "nice",
        "-n",
        str(nice),
        *rclone_command(base, port),
    ]


def supervise(
    base: Path,
    cap_gb: float,
    interval: int,
    nice: int = 15,
    ionice_class: int = 2,
    ionice_level: int = 7,
    port: int = PORT,
) -> int:
    """Server loop: keep rclone up, prune every interval, write a status file."""
    (base / "data").mkdir(parents=True, exist_ok=True)
    import fcntl

    # T3472 phase 2: one supervisor per base dir (the hub relaunches it after a container
    # restart without knowing whether one still lives); the lock dies with the process.
    lock = open(base / "supervisor.lock", "w")  # noqa: SIM115 - held for the process lifetime
    try:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except OSError:
        return 0
    child: subprocess.Popen | None = None
    while True:
        if child is None or child.poll() is not None:
            if webdav_responding(port):
                child = None  # another supervisor serves it
            else:
                child = subprocess.Popen(
                    rclone_priority_command(base, nice, ionice_class, ionice_level, port),
                    stdin=subprocess.DEVNULL,
                    start_new_session=True,
                )
        stats = prune(base / "data", cap_gb, apply=True)
        stats["at"] = time.strftime("%Y-%m-%dT%H:%M:%S")
        (base / "status.json").write_text(json.dumps(stats) + "\n")
        time.sleep(interval)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(prog="sccache_shared", description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    sub.add_parser("tunnel", help="open the SSH forward to the server's WebDAV store")
    sub.add_parser("status", help="tunnel, password file and client env (no secrets)")
    item = sub.add_parser("reverse", help="give a LAN host (the Deck) the store via this host")
    item.add_argument("address")
    item.add_argument("--port", type=int, default=2222)
    item = sub.add_parser("prune", help="server: prune the store to the cap")
    item.add_argument("--dir", type=Path, required=True, help="store data dir")
    item.add_argument("--cap-gb", type=float, default=CAP_GB)
    item.add_argument("--apply", action="store_true")
    item = sub.add_parser("supervise", help="server: run rclone and the prune loop")
    item.add_argument("--dir", type=Path, required=True, help="backend base dir")
    item.add_argument("--cap-gb", type=float, default=CAP_GB)
    item.add_argument("--interval", type=int, default=600)
    item.add_argument("--port", type=int, default=PORT)
    item.add_argument("--nice", type=int, default=15)
    item.add_argument("--ionice-class", type=int, choices=(1, 2, 3), default=2)
    item.add_argument("--ionice-level", type=int, choices=range(8), default=7)
    args = parser.parse_args(argv)
    if args.command == "tunnel":
        ok = ensure_tunnel()
        print("tunnel " + ("up" if ok else "unavailable"))
        return 0 if ok else 1
    if args.command == "reverse":
        if not ensure_tunnel():
            print("tunnel unavailable")
            return 1
        done = subprocess.run(
            reverse_command(args.address, args.port),
            timeout=20,
            check=False,
            stdin=subprocess.DEVNULL,
        )
        print(f"reverse forward to {args.address}: rc={done.returncode}")
        return done.returncode
    if args.command == "prune":
        print(json.dumps(prune(args.dir.expanduser(), args.cap_gb, args.apply)))
        return 0
    if args.command == "supervise":
        return supervise(
            args.dir.expanduser(),
            args.cap_gb,
            args.interval,
            args.nice,
            args.ionice_class,
            args.ionice_level,
            port=args.port,
        )
    from tools.wt import core

    main_dir = core.main_root(core.worktree_root(Path.cwd()))
    env = client_env(main_dir)
    secret = pass_file(main_dir)
    print(
        json.dumps(
            {
                "tunnel": webdav_responding(),
                "pass_file": str(secret) if secret else None,
                "shared": bool(env),
                "env": env,
            },
            indent=1,
        )
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
