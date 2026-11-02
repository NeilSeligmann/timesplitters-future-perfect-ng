"""Debian 13 build container for Ubuntu builder hosts (T3737, docs/t-toolchain-parity.md).

The prebuilt input hash folds in the builder's cc, cmake and python3 version lines, so a host
only builds accepted artifacts under the owner PC toolchain (Debian clang 19.1.7 (3+b1), cmake
3.31.6, python 3.13.5). The droplets run Ubuntu 24.04, which cannot report those lines, so a
fleet.json host with `build_container: true` runs its remote build, and the toolchain probe,
INSIDE the pinned image (`tools/fleet/remote/builder-image/Dockerfile`). The image ID is pinned
in `tools/data/expected_toolchain.json` (`build_image`).

The container sees the host home at the same path (repo, canon root, cargo, rustup, sccache,
uv, ssh key), runs as the same uid and gid, and shares the host network, so the build command
is byte-identical to the one a plain host runs. Docker is allowed on the droplets only: never
on the owner PC or the laptop.
"""

from __future__ import annotations

import json
import shlex
from collections.abc import Iterable
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
EXPECTED_TOOLCHAIN = REPO_ROOT / "tools" / "data" / "expected_toolchain.json"
DOCKERFILE = REPO_ROOT / "tools" / "fleet" / "remote" / "builder-image" / "Dockerfile"
# The ssh login has no docker group (joining it needs a new login), passwordless sudo exists.
DOCKER = "sudo -n docker"
# Host files the image deliberately does not carry: user database (git and ssh need the uid to
# resolve) and nothing else. Binaries (sccache, uv, cargo) live in the home mount.
READ_ONLY_MOUNTS = ("/etc/passwd", "/etc/group")


def build_image(path: Path = EXPECTED_TOOLCHAIN) -> dict[str, str]:
    """The pinned image: `{"name": <tag>, "id": "sha256:<image id>"}` from the tracked config."""
    entry = json.loads(path.read_text()).get("build_image") or {}
    name, image_id = str(entry.get("name", "")), str(entry.get("id", ""))
    if not name or not image_id.startswith("sha256:"):
        raise ValueError(f"{path.name} has no pinned build_image (name and sha256: id)")
    return {"name": name, "id": image_id}


def enabled(host_name: str) -> bool:
    """True when the fleet config runs `host_name` builds in the container."""
    from tools.fleet.config import ConfigError, load_config

    try:
        config = load_config()
    except ConfigError:
        return False
    return any(host.build_container for host in config.remote_hosts() if host.name == host_name)


# The static sccache the fleet installs in the home (the host /usr/bin one is not on the image).
STATIC_SCCACHE = (
    '$(f="$HOME/.local/lib/tsfp/sccache"; [ -x "$f" ] && printf -- "-e TSFP_SCCACHE=%s" "$f")'
)
OUTSIDE_MOUNTS = (
    '$(for l in "$HOME"/* "$HOME"/.[!.]* @PATHS@; do '
    '[ -d "$l" ] && readlink -f "$l"; done | sort -u | while read -r r; do '
    'case "$r" in "$HOME"|"$HOME"/*) ;; *) printf -- "-v %s:%s " "$r" "$r";; esac; done)'
)


def mounts(home: str, paths: Iterable[str]) -> list[str]:
    """Bind mounts at identical paths: the home, then every real directory outside it.

    The persistent volume hangs off the home as a symlink on some droplets (`~/rea-codex2` ->
    `/mnt/volume_*/rea-codex2`): its target must exist in the container under the same path or
    git and the build cannot create their files there. The remote shell resolves the symlinks
    under the home plus the explicit `paths` (the repo and canon root) to real paths, drops
    those inside the home and emits one `-v` per distinct remaining directory.
    """
    home_mount = '"$HOME:$HOME"' if home == "$HOME" else shlex.quote(f"{home}:{home}")
    explicit = " ".join(shlex.quote(path) for path in sorted(set(paths)) if path.startswith("/"))
    return [f"-v {home_mount}", OUTSIDE_MOUNTS.replace("@PATHS@", explicit).replace("  ", " ")]


def run_prefix(*, home: str = "$HOME", paths: Iterable[str] = (), image: str | None = None) -> str:
    """`docker run` words up to (excluding) the command.

    Without a known `home` the remote login shell expands `$HOME`, as it does for the plain
    build; the home mount is always the same path inside and out.
    """
    image_id = image or build_image()["id"]
    words = [
        DOCKER,
        "run --rm --init --network host",
        '-u "$(id -u):$(id -g)" -e HOME="$HOME" -w "$HOME"',
        *(f"-v {path}:{path}:ro" for path in READ_ONLY_MOUNTS),
        STATIC_SCCACHE,
        *mounts(home, paths),
        shlex.quote(image_id),
    ]
    return " ".join(words)


def wrap(
    script: str, *, home: str = "$HOME", paths: Iterable[str] = (), image: str | None = None
) -> str:
    """Run `script` (a bash script) inside the build container, exit status preserved."""
    prefix = run_prefix(home=home, paths=paths, image=image)
    return f"{prefix} bash -c {shlex.quote(script)}"
