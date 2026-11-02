"""T2424: keep the shared venv interpreter alive across container restarts.

uv-managed Pythons live under `~/.local/share/uv/python` by default and vanish when the
container is rebuilt. The interpreter is pinned to `<main>/.uv-python` (gitignored) instead,
and `ensure` reinstalls it and repoints `.venv` when the symlink dangles.
"""

from __future__ import annotations

import os
import re
import shutil
import subprocess
import tempfile
from collections.abc import Callable
from pathlib import Path
from typing import Any

PIN_DIR = ".uv-python"


def pinned_version(main: Path) -> str | None:
    """Return the repository's exact Python pin, if present."""
    try:
        value = (main / ".python-version").read_text().strip()
    except OSError:
        return None
    return value or None


def venv_version(venv: Path) -> str | None:
    """Return the running interpreter's full version, falling back to pyvenv.cfg metadata."""
    python = venv / "bin" / "python"
    try:
        done = subprocess.run(
            [str(python), "-c", "import platform; print(platform.python_version())"],
            capture_output=True,
            text=True,
            timeout=5,
            check=True,
        )
        value = done.stdout.strip()
        if value:
            return value
    except (OSError, subprocess.SubprocessError):
        pass
    try:
        text = (venv / "pyvenv.cfg").read_text()
    except OSError:
        return None
    match = re.search(r"^version_info\s*=\s*([\d.]+)", text, re.MULTILINE)
    return match.group(1) if match else None


def healthy(venv: Path) -> bool:
    return (venv / "bin" / "python").exists()  # follows the symlink: False when it dangles


def editable_pth_files(venv: Path) -> list[Path]:
    """The pip/uv editable-install path files of the venv (one line: the source directory)."""
    return sorted(venv.glob("lib/python*/site-packages/_editable_impl_*.pth"))


def repoint_editable(venv: Path, main: Path) -> list[str]:
    """T3596: make the venv's editable `tools` install point at the main checkout.

    `uv pip install -e .` run from a worker tree leaves the SHARED venv importing `tools` from
    that tree. When it is removed Python prints "In ...pth: <dir> does not exist" on every start
    (tests that assert on stderr then fail), and while it exists, a subprocess started in another
    cwd imports a foreign tree. Resolve relative to the real interpreter prefix so shared
    mount namespaces do not keep overwriting each other's absolute checkout path.
    Unknown import hooks are left alone; returns the files that were repointed.
    """
    relative = os.path.relpath(main.resolve(), venv.resolve())
    line = (
        "import os, sys; _tsfp_editable_root = os.path.normpath(os.path.join("
        f"os.path.realpath(sys.prefix), {relative!r})); "
        "sys.path.append(_tsfp_editable_root) if "
        "os.path.isdir(os.path.join(_tsfp_editable_root, 'tools')) and "
        "_tsfp_editable_root not in sys.path else None"
    )
    repointed = []
    for pth in editable_pth_files(venv):
        try:
            target = pth.read_text().strip()
        except OSError:
            continue
        if target.startswith("import ") and "_tsfp_editable_root" not in target:
            continue
        if target and target != line:
            with tempfile.NamedTemporaryFile(mode="w", dir=pth.parent, delete=False) as output:
                staged = Path(output.name)
                output.write(line + "\n")
            try:
                staged.chmod(pth.stat().st_mode & 0o777)
                staged.replace(pth)
            finally:
                staged.unlink(missing_ok=True)
            repointed.append(pth.name)
    return repointed


def ensure(
    main: Path,
    run: Callable[..., Any] = subprocess.run,
    timeout: int = 600,
    expected_version: str | None = None,
) -> str:
    """Keep the main venv on the repository pin; never raises."""
    venv = main / ".venv"
    if not venv.is_dir():
        return "no-venv"
    expected = expected_version or pinned_version(main)
    actual = venv_version(venv)
    if healthy(venv) and (expected is None or actual == expected):
        fixed = repoint_editable(venv, main)
        return f"ok (editable install repointed to {main}: {', '.join(fixed)})" if fixed else "ok"
    version = expected or actual or "3"
    env = dict(os.environ, UV_PYTHON_INSTALL_DIR=str(main / PIN_DIR))
    try:
        run(
            ["uv", "python", "install", version],
            env=env,
            check=True,
            timeout=timeout,
            capture_output=True,
        )
        run(
            ["uv", "venv", "--clear", "--python", version, str(venv)],
            env=env,
            check=True,
            timeout=timeout,
            capture_output=True,
        )
    except (OSError, subprocess.SubprocessError) as exc:
        return f"failed: {exc}"
    actual = venv_version(venv)
    return (
        "healed"
        if healthy(venv) and (expected is None or actual == expected)
        else "failed: interpreter mismatch"
    )


def ensure_worktree(root: Path, main: Path) -> str:
    """Rebuild a stale private worktree venv on the pinned interpreter."""
    target = root / ".venv"
    if root.resolve() == main.resolve() or not target.exists() and not target.is_symlink():
        return "skip"
    expected = pinned_version(root) or pinned_version(main) or venv_version(main / ".venv")
    actual = venv_version(target)
    if expected is None or actual == expected:
        return "ok"
    same_lock = all(
        (root / name).exists() == (main / name).exists()
        and (not (root / name).exists() or (root / name).read_bytes() == (main / name).read_bytes())
        for name in ("uv.lock", "pyproject.toml")
    )
    try:
        if target.is_symlink() or target.is_file():
            target.unlink()
        else:
            shutil.rmtree(target)
        if same_lock:
            target.symlink_to(main / ".venv", target_is_directory=True)
            return f"relinked ({actual or 'unknown'} -> {expected})"
        env = dict(
            os.environ,
            UV_PYTHON_INSTALL_DIR=str(main / PIN_DIR),
            UV_PROJECT_ENVIRONMENT=str(target),
        )
        subprocess.run(
            ["uv", "python", "install", expected],
            env=env,
            check=True,
            capture_output=True,
            timeout=600,
        )
        subprocess.run(
            ["uv", "sync", "--frozen", "--offline"],
            cwd=root,
            env=env,
            check=True,
            capture_output=True,
            timeout=600,
        )
    except OSError as exc:
        return f"failed: {exc}"
    except subprocess.SubprocessError as exc:
        return f"failed: {exc}"
    return f"rebuilt ({actual or 'unknown'} -> {expected})"
