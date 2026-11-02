"""Restore a missing execution-host cache client from the pinned local installation.

Only the installed executable is streamed; no repository or credentials are copied.
The destination is published after architecture, digest and version checks.
"""

from __future__ import annotations

import hashlib
import re
import shlex
import subprocess
from collections.abc import Callable
from pathlib import Path
from typing import TYPE_CHECKING

from tools.wt import remote_replay, sccache_shared

if TYPE_CHECKING:
    from tools.wt.remote_exec import HostSpec


def target() -> str:
    return remote_replay._repo_cd(str(sccache_shared.HOST_SCCACHE))


def expected_version() -> str:
    return "sccache " + sccache_shared.SCCACHE_VERSION.removeprefix("v")


def check_command() -> str:
    path = target()
    return (
        f"test -x {path} && "
        f'test "$(timeout 10s nice -n 19 {path} --version 2>/dev/null)" '
        f"= {shlex.quote(expected_version())}"
    )


def source_identity(path: Path) -> tuple[str, str]:
    """Bounded, streaming identity of the supported static ELF client."""
    with path.open("rb") as stream:
        header = stream.read(20)
        if len(header) != 20 or header[:6] != b"\x7fELF\x02\x01":
            raise ValueError("cache client must be a 64-bit little-endian ELF")
        machine = int.from_bytes(header[18:20], "little")
        arch = {62: "x86_64", 183: "aarch64"}.get(machine)
        if arch is None:
            raise ValueError("unsupported cache client architecture")
        digest = hashlib.sha256(header)
        while chunk := stream.read(1024 * 1024):
            digest.update(chunk)
    return arch, digest.hexdigest()


def install_command(arch: str, digest: str) -> str:
    if arch not in {"x86_64", "aarch64"} or not re.fullmatch(r"[0-9a-f]{64}", digest):
        raise ValueError("invalid cache client identity")
    # A private temporary file avoids clobbering a live executable or another
    # installer's upload. Even concurrent repairs only publish verified bytes.
    return (
        f'set -eu; test "$(uname -m)" = {shlex.quote(arch)}; '
        f'client={target()}; mkdir -p "$(dirname "$client")"; '
        'umask 077; staged=$(mktemp "$client.XXXXXX"); '
        'trap \'rm -f "$staged"\' EXIT; cat > "$staged"; '
        f"printf '%s  %s\\n' {digest} \"$staged\" | sha256sum -c - >/dev/null; "
        'chmod 755 "$staged"; '
        'test "$(timeout 10s nice -n 19 "$staged" --version 2>/dev/null)" '
        f"= {shlex.quote(expected_version())}; "
        'mv -f "$staged" "$client"'
    )


def ensure(
    host: HostSpec,
    main: Path,
    *,
    runner: Callable[..., subprocess.CompletedProcess] = subprocess.run,
) -> bool:
    """Check first; restore only from the local pinned binary, fail closed on errors."""

    def remote(command: str, **kwargs: object) -> subprocess.CompletedProcess:
        return runner(
            remote_replay.ssh_argv(
                host.worker(), f"timeout 60s nice -n 19 sh -c {shlex.quote(command)}"
            ),
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            timeout=75,
            check=False,
            **kwargs,
        )

    try:
        if remote(check_command()).returncode == 0:
            return True
        source = sccache_shared.sccache_bin(main)
        if source is None:
            return False
        arch, digest = source_identity(source)
        with source.open("rb") as stream:
            if remote(install_command(arch, digest), stdin=stream).returncode:
                return False
        return remote(check_command()).returncode == 0
    except (OSError, ValueError, subprocess.SubprocessError):
        return False
