"""T2424: shared cargo target, sccache and venv for every worktree, via a PreToolUse Bash hook.

Harness worktrees (`.claude/worktrees/agent-*`) never run `tools.wt env`, so cargo built into
private `crates/*/target` dirs. The hook prefixes each Bash command with the exports of
`core.env_vars` (shared `tmp/wt-cache/cargo-target/<family>`) and links a missing `.venv`.
Every path is computed from the git common dir, nothing is hardcoded.
"""

from __future__ import annotations

import hashlib
import json
import os
import shlex
import subprocess
import sys
from pathlib import Path

from tools import owner_files
from tools.wt import buildslots, core

DEFAULT_FAMILY = "default"
SHARED_PARTS = ("tmp", "wt-cache", "cargo-target")


def family_name(env: dict[str, str] | None = None) -> str:
    """Family keys incompatible profiles apart; TSFP_CARGO_FAMILY selects one."""
    return (env if env is not None else os.environ).get("TSFP_CARGO_FAMILY") or DEFAULT_FAMILY


def worktree_of(cwd: Path) -> tuple[Path, Path] | None:
    """(worktree root, main checkout) for a cwd inside any git tree, else None."""
    try:
        out = subprocess.run(
            ["git", "-C", str(cwd), "rev-parse", "--show-toplevel", "--git-common-dir"],
            capture_output=True,
            text=True,
            timeout=10,
            check=True,
        ).stdout.split("\n")
    except (OSError, subprocess.SubprocessError):
        return None
    if len(out) < 2 or not out[0]:
        return None
    root = Path(out[0])
    common = (Path(cwd) / out[1]).resolve()
    main = common.parent if common.name == ".git" else root
    return root, main


LOCK_FILES = ("uv.lock", "pyproject.toml")


def lock_key(root: Path) -> str:
    """Hash of the files that decide the venv contents (T2439); empty when none exist."""
    digest = hashlib.sha256()
    found = False
    for name in LOCK_FILES:
        path = root / name
        if path.is_file():
            digest.update(name.encode() + b"\0" + path.read_bytes())
            found = True
    return digest.hexdigest() if found else ""


def private_venv(root: Path) -> str:
    """Private .venv synced from this worktree's own lockfile (only when it differs)."""
    marker = root / ".venv-sync-failed"
    if marker.exists():
        return "lock-differs"
    try:
        subprocess.run(
            ["uv", "sync", "--frozen", "--offline"],
            cwd=root,
            capture_output=True,
            timeout=600,
            check=True,
            env=dict(os.environ, UV_PROJECT_ENVIRONMENT=str(root / ".venv")),
        )
    except (OSError, subprocess.SubprocessError):
        marker.write_text("uv sync --frozen --offline failed; run it by hand\n")
        return "lock-differs"
    return "private"


def link_venv(root: Path, main: Path) -> str:
    """Share main's venv while the lockfile hash matches, else a private one (T2439)."""
    target, source = root / ".venv", main / ".venv"
    if root.resolve() == main.resolve():
        return "skip"
    from tools.wt import pyheal

    main_status = pyheal.ensure(main, expected_version=pyheal.pinned_version(root))
    if main_status.startswith("failed"):
        return main_status
    worktree_status = pyheal.ensure_worktree(root, main)
    if worktree_status.startswith("failed"):
        return worktree_status
    if target.exists() or target.is_symlink():
        return worktree_status if worktree_status != "ok" else "skip"
    if not source.is_dir():
        return "no-source"
    if lock_key(root) != lock_key(main):
        return private_venv(root)
    try:
        target.symlink_to(source)
    except OSError:
        return "error"
    return "linked"


DISC_NAME = owner_files.DISC_NAME


def main_checkout_paths(root: Path, env: dict[str, str] | None = None) -> dict[str, str]:
    """TSFP_BEVY_CARGO_HOME and TSFP_DISC from the main checkout when unset (T2424).

    build.sh needs the offline Bevy cargo home, play.sh the disc image. Harness worktrees
    have neither, so point both at the main checkout (git common dir), tmp/owner included.
    """
    env = os.environ if env is None else env
    found: dict[str, str] = {}
    from tools.wt import cargo_home as cargo_homes

    main = core.main_root(root)
    cargo_home = cargo_homes.bevy_home(main, env)  # T3472: the unified home once merged
    if not env.get("TSFP_BEVY_CARGO_HOME") and (cargo_home / "registry").is_dir():
        found["TSFP_BEVY_CARGO_HOME"] = str(cargo_home)
    if not env.get("TSFP_DISC"):
        disc = owner_files.disc_image(root, env)
        if disc.is_file():
            found["TSFP_DISC"] = str(disc)
    if not env.get("TSFP_FILES_DIR"):
        found["TSFP_FILES_DIR"] = str(owner_files.xbox_files_dir(root, env))
    return found


def exports(root: Path, main: Path, family: str, command: list[str] | None = None) -> str:
    env = core.env_vars(root, main, family, command)
    env["UV_PYTHON_INSTALL_DIR"] = str(main / ".uv-python")
    env.update(main_checkout_paths(root))
    return "".join(f"export {k}={shlex.quote(v)}; " for k, v in env.items())


def rewrite(command: str, cwd: Path, env: dict[str, str] | None = None) -> str | None:
    """The command with shared-cache exports prefixed, or None to leave it alone."""
    found = worktree_of(cwd)
    if found is None:
        return None
    root, main = found
    link_venv(root, main)
    try:
        from tools.wt import prebuilt

        prebuilt.schedule_main_warm(main)
    except Exception as error:  # a build-cache check must never break the shell hook
        print(f"prebuilt warm-main check skipped: {error}", file=sys.stderr)
    family = family_name(env)
    shell_command = ["sh", "-c", command]
    effective = core.cargo_family_key(root, family, shell_command)
    main.joinpath(*SHARED_PARTS, effective).mkdir(parents=True, exist_ok=True)
    prefix = exports(root, main, family, shell_command)
    # The hook runs before the host shell starts, so route direct cargo invocations through
    # tools.wt run to keep the profile's shared flock alive across cargo and rustc execs.
    if buildslots.is_heavy(shell_command):
        routed = (
            "python3 -m tools.wt run --family "
            + shlex.quote(family)
            + " -- sh -c "
            + shlex.quote(command)
        )
        return prefix + routed
    return prefix + command


def hook(stdin_text: str) -> str:
    """PreToolUse JSON in, hook decision JSON out (empty string = no opinion)."""
    try:
        payload = json.loads(stdin_text)
        command = payload["tool_input"]["command"]
        cwd = Path(payload.get("cwd") or os.getcwd())
    except (ValueError, KeyError, TypeError):
        return ""
    new = rewrite(command, cwd)
    if new is None:
        return ""
    return json.dumps(
        {
            "hookSpecificOutput": {
                "hookEventName": "PreToolUse",
                "permissionDecision": "allow",
                "updatedInput": dict(payload["tool_input"], command=new),
            }
        }
    )


def cli() -> int:
    out = hook(sys.stdin.read())
    if out:
        print(out)
    return 0
