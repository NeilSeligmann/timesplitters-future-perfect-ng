"""T3472 phase 2: keep the shared sccache store reachable from every remote build host.

The store (sccache_shared.py) lives on the registry's store host, which has no init system, so a
container restart silently kills its supervisor. Each host reaches the registry-selected store
port on its own loopback:

- `server`  the host itself (laptop: rclone serve webdav, relaunched here when dead);
- `forward` the host opens its own `ssh -L` to the store;
- `reverse` this hub holds `ssh -R` into the host because the host has no key on the server
  (Deck);
- `reverse-ssh` this hub holds `ssh -R` into a public host over its registered SSH route
  (droplets);
- `none`    no route (public droplet): local-disk sccache only.

`ensure_host_store` is called before every remote verify and by the hourly hygiene tick
(`hygiene_pass`): it checks that the port answers HTTP, relaunches the server supervisor when
the backend is dead, and re-opens the tunnel in the host's mode. Nothing here handles the
WebDAV password: hosts read it from their own `~/.config/tsfp/sccache-webdav.pass`.

    python3 -m tools.wt.sccache_hosts [--host NAME]     # check + repair, print JSON
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import shlex
import subprocess
import sys
from collections.abc import Callable, Sequence
from pathlib import Path
from typing import TYPE_CHECKING

from tools.wt import sccache_shared

if TYPE_CHECKING:
    from tools.wt.remote_exec import HostSpec

ALIVE_SH = (
    "( exec 3<>/dev/tcp/127.0.0.1/{port} && printf 'GET / HTTP/1.0\\r\\n\\r\\n' >&3 "
    '&& read -r -t 5 l <&3 && case "$l" in HTTP/*) exit 0;; esac; exit 1 ) 2>/dev/null'
)
"""Shell: succeeds when something answers HTTP on the loopback store port (401 counts)."""

SERVER_BASE = sccache_shared.SERVER_DIR.replace("~/", "$HOME/", 1)
SUPERVISOR_HELD_SH = f'cd "{SERVER_BASE}" 2>/dev/null && ! flock -n supervisor.lock -c true'
"""Shell on the server: succeeds while a supervisor holds its lock (rclone alone is not enough:
without the supervisor the store is never pruned and rclone is not restarted)."""


DEPLOYED_SH = (
    f'B="{SERVER_BASE}"; test -x "$B/bin/rclone" && test -s "$B/htpasswd" '
    '&& test -f "$B/sccache_shared.py" && test -d "$B/data"'
)
"""Shell on the server: succeeds while the store's own files (pinned rclone, htpasswd, the
supervisor script, data dir) exist. A disk cleanup that removes the dir makes this fail
(I-T3865-4)."""

RCLONE_INSTALL_SH = (
    f'B="{SERVER_BASE}"; set -e; A=amd64; [ "$(uname -m)" = aarch64 ] && A=arm64; '
    f"V={sccache_shared.RCLONE_VERSION}; N=rclone-$V-linux-$A; U=https://downloads.rclone.org/$V; "
    'T=$(mktemp -d); cd "$T"; curl -fsSL -m 120 -o $N.zip $U/$N.zip; '
    'curl -fsSL -m 60 -o SUMS $U/SHA256SUMS; grep " $N.zip$" SUMS | sha256sum -c - >/dev/null; '
    'python3 -c "import zipfile,sys;zipfile.ZipFile(sys.argv[1]).extractall()" $N.zip; '
    'install -m 755 $N/rclone "$B/bin/rclone"; cd /; rm -rf "$T"'
)
"""Shell on the server: install the pinned rclone, checked against the published SHA256SUMS."""

HTPASSWD_SH = (
    f'B="{SERVER_BASE}"; P="$HOME/.config/tsfp/{sccache_shared.PASS_NAME}"; set -e; umask 077; '
    f'printf "{sccache_shared.USER}:%s\\n" "$(openssl passwd -apr1 -stdin < "$P")" > "$B/htpasswd"'
)
"""Shell on the server: htpasswd (apr1) from the host-local password file; nothing is printed."""


def push_file(
    host: HostSpec,
    local: Path,
    remote: str,
    *,
    mode: str = "600",
    runner: Callable[..., subprocess.CompletedProcess] = subprocess.run,
) -> bool:
    """Copy one local file to `remote` over ssh stdin (file to file, content never displayed)."""
    from tools.wt import remote_replay

    command = (
        f"umask 077; cat > {remote}.new && chmod {mode} {remote}.new && mv {remote}.new {remote}"
    )
    with local.open("rb") as handle:
        done = runner(
            remote_replay.ssh_argv(host.worker(), f"bash -c {shlex.quote(command)}"),
            stdin=handle,
            capture_output=True,
            timeout=60,
            check=False,
        )
    return done.returncode == 0


def store_deployed(host: HostSpec, ssh: Callable[..., subprocess.CompletedProcess]) -> bool:
    digest = hashlib.sha256(Path(sccache_shared.__file__).read_bytes()).hexdigest()
    script_hash = (
        f'test "$(sha256sum "{SERVER_BASE}/sccache_shared.py" | cut -d " " -f 1)" '
        f"= {shlex.quote(digest)}"
    )
    done = _run_remote(host, ssh, f"{DEPLOYED_SH} && {script_hash}", 30)
    return bool(done and done.returncode == 0)


def deploy_store(
    host: HostSpec,
    ssh: Callable[..., subprocess.CompletedProcess],
    *,
    main: Path | None = None,
    pusher: Callable[..., bool] = push_file,
) -> bool:
    """Recreate a wiped store on its `server` host: dirs, supervisor script, password file
    (copied from this hub's file, or generated on the host), htpasswd, pinned rclone.
    Idempotent: existing pieces are kept. True when the store is complete afterwards."""
    from tools.wt import core

    def ok(command: str, timeout: int = 30) -> bool:
        done = _run_remote(host, ssh, command, timeout)
        return bool(done and done.returncode == 0)

    base = SERVER_BASE
    pass_dir = "$HOME/.config/tsfp"
    remote_pass = f"{pass_dir}/{sccache_shared.PASS_NAME}"
    if not ok(f'mkdir -p "{base}/bin" "{base}/data" "{pass_dir}"'):
        return False
    if not pusher(host, Path(sccache_shared.__file__), f'"{base}/sccache_shared.py"', mode="644"):
        return False
    if not ok(f'test -s "{remote_pass}"'):
        hub_pass = sccache_shared.pass_file(main or core.main_root(core.worktree_root(Path.cwd())))
        if hub_pass is not None:
            if not pusher(host, hub_pass, f'"{remote_pass}"'):
                return False
        elif not ok(f'umask 077; openssl rand -base64 32 | tr -d "\\n" > "{remote_pass}"'):
            return False  # generated on the host: clients must then be re-fed file to file
    if not ok(HTPASSWD_SH):
        return False
    if not ok(f'test -x "{base}/bin/rclone"') and not ok(RCLONE_INSTALL_SH, 240):
        return False
    return store_deployed(host, ssh)


def alive_command(mode: str = "") -> str:
    check = ALIVE_SH.format(port=sccache_shared.store_port())
    if mode != "server":
        return check
    return f"{check} && {supervisor_held_sh()}"


def supervisor_command(host: HostSpec | None = None) -> str:
    """Start the registry-selected store with its registry-defined retention cap."""
    cap = sccache_shared.store_cap_gib(host)
    port = sccache_shared.store_port(host)
    return (
        f'cd "{SERVER_BASE}" && '
        "setsid -f python3 sccache_shared.py supervise "
        f'--dir "{SERVER_BASE}" --cap-gb {cap:g} --port {port} '
        "--nice 19 --ionice-class 2 --ionice-level 7 "
        ">> supervise.log 2>&1 < /dev/null"
    )


def supervisor_held_sh() -> str:
    return f'cd "{SERVER_BASE}" 2>/dev/null && ! flock -n supervisor.lock -c true'


def tunnel_shell(mode: str) -> str | None:
    """Shell run ON the host to (re)open its own forward, for the `forward` mode only."""
    if mode != "forward":
        return None
    port = sccache_shared.store_port()
    server = sccache_shared.store_host()
    return (
        'K=; [ -f "$HOME/.ssh/tsfp-sccache" ] '
        '&& K="-i $HOME/.ssh/tsfp-sccache -o IdentitiesOnly=yes"; '
        "ssh -f -N -o BatchMode=yes -o ExitOnForwardFailure=yes -o ServerAliveInterval=30 "
        "-o ServerAliveCountMax=3 -o ConnectTimeout=5 -o ControlMaster=no -o ControlPath=none "
        f"$K -L 127.0.0.1:{port}:127.0.0.1:{port} -p {server.port} "
        f"{shlex.quote(server.user + '@' + server.address)}"
    )


def _run_remote(
    host: HostSpec, ssh: Callable[..., subprocess.CompletedProcess], command: str, timeout: int
) -> subprocess.CompletedProcess | None:
    try:
        return ssh(host.worker(), f"bash -c {shlex.quote(command)}", timeout=timeout)
    except (OSError, subprocess.SubprocessError):
        return None


def host_alive(host: HostSpec, ssh: Callable[..., subprocess.CompletedProcess]) -> bool:
    done = _run_remote(host, ssh, alive_command(getattr(host, "sccache", "")), 30)
    return bool(done and done.returncode == 0)


def relaunch_server(
    servers: Sequence[HostSpec], ssh: Callable[..., subprocess.CompletedProcess]
) -> bool:
    """Start the supervisor on the registry host in `server` mode; True when it was started."""
    for server in servers:
        if getattr(server, "sccache", "none") != "server":
            continue
        done = _run_remote(server, ssh, supervisor_command(server), 40)
        return bool(done and done.returncode == 0)
    return False


def store_switched_off(hosts: Sequence[HostSpec]) -> bool:
    """True when the store's own host (route `server`) is switched off by its fleet `off_file`
    (owner 2026-10-24: keep load off the git server's host). Every host then builds with its
    local-disk sccache only and nothing tries to reach or relaunch the store (I-T3735-1)."""
    from tools.wt import remote_replay

    off = remote_replay.switched_off()
    return any(getattr(host, "sccache", "none") == "server" and host.name in off for host in hosts)


def ensure_host_store(
    host: HostSpec,
    ssh: Callable[..., subprocess.CompletedProcess],
    *,
    hosts: Sequence[HostSpec] = (),
    local_runner: Callable[..., object] = subprocess.run,
    local_tunnel: Callable[[], bool] = sccache_shared.ensure_tunnel,
    sleep: Callable[[float], None] | None = None,
    store_off: Callable[[Sequence[HostSpec]], bool] | None = None,
    deploy: Callable[..., bool] = deploy_store,
) -> bool:
    """True when `host` reaches the store on its loopback, repairing what is down."""
    import time

    sleep = sleep or time.sleep
    mode = getattr(host, "sccache", "none")
    if mode == "none" or (store_off or store_switched_off)(hosts or [host]):
        return False
    if mode == "server" and not store_deployed(host, ssh):
        if not deploy(host, ssh) or not store_deployed(host, ssh):
            return False  # refresh wiped or outdated assets before trying to launch the service
    if host_alive(host, ssh):
        return True
    relaunch_server(hosts or [host], ssh)  # the backend may be the dead part
    sleep(3)
    if mode == "forward":
        command = tunnel_shell(mode)
        if command:
            _run_remote(host, ssh, command, 40)
    elif mode in {"reverse", "reverse-ssh"}:
        if local_tunnel():
            local_runner(
                sccache_shared.reverse_command(
                    host.address, host.port, host.user, sccache_shared.store_port()
                ),
                timeout=25,
                check=False,
                stdin=subprocess.DEVNULL,
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
            )
    sleep(1)
    return host_alive(host, ssh)


def ensure_host_cache(
    host: HostSpec,
    ssh: Callable[..., subprocess.CompletedProcess],
    *,
    hosts: Sequence[HostSpec],
    main: Path,
) -> bool:
    """A reachable WebDAV store is insufficient when its compiler client was wiped."""
    from tools.wt import sccache_client

    return ensure_host_store(host, ssh, hosts=hosts) and sccache_client.ensure(host, main)


def hygiene_pass(
    main: Path | None = None,
    *,
    hosts: Sequence[HostSpec] | None = None,
    ssh: Callable[..., subprocess.CompletedProcess] | None = None,
) -> dict[str, bool]:
    """Hourly: check and repair the store route of every enabled remote host."""
    from tools.wt import core, remote_exec, remote_replay

    ssh = ssh or remote_replay._ssh
    hosts = list(hosts if hosts is not None else remote_exec.load_registry())
    root = main or core.main_root(remote_exec._main_root())
    result: dict[str, bool] = {}
    if store_switched_off(hosts):
        return result
    ordered = sorted(hosts, key=lambda host: getattr(host, "sccache", "none") != "server")
    for host in ordered:
        if getattr(host, "sccache", "none") == "none" or remote_exec.host_disabled(root, host.name):
            continue
        result[host.name] = ensure_host_cache(host, ssh, hosts=hosts, main=root)
    return result


def ensure_local_server(main: Path) -> bool:
    """T3472 phase 3: start this host's own sccache daemon with a clean fd table (hygiene tick)."""
    from tools.wt import core

    env = {**os.environ, **core.env_vars(main, main, "default")}
    if not core.apply_shared_sccache(main, main, env):
        return False
    return sccache_shared.start_server(env)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(prog="python3 -m tools.wt.sccache_hosts", description=__doc__)
    parser.add_argument("--host", action="append", help="host name (default: all with a route)")
    args = parser.parse_args(argv)
    from tools.wt import core, remote_exec, remote_replay

    hosts = remote_exec.load_registry()
    if store_switched_off(hosts):
        print(json.dumps({"store": "switched-off", "route": "local-disk sccache only"}))
        return 0
    wanted = set(args.host or [host.name for host in hosts])
    root = core.main_root(remote_exec._main_root())
    result = {
        host.name: ensure_host_cache(host, remote_replay._ssh, hosts=hosts, main=root)
        for host in hosts
        if host.name in wanted and host.sccache != "none"
    }
    print(json.dumps(result))
    return 0 if all(result.values()) else 1


if __name__ == "__main__":
    sys.exit(main())
