"""Owner inputs that tests need inside a remote exec tree (cx-ownerxbe, rule 1 2026-10-28).

The retail XBE and the XDK surface are gitignored, so `exec_sync` trees never carry them and
tests that need them SKIP on the host. Provisioning (`tools.fleet provision`, step owner-inputs)
copies them once into `<data_root>/tmp/owner/links/<relative path>`; `verify_script` links them
into the exec tree like `tools.wt bootstrap` does locally. Never committed, never printed."""

from __future__ import annotations

import shlex
from pathlib import Path

from tools.wt import core

STORE_REL = "tmp/owner/links"
"""Where a host keeps the files, relative to its `data_root`."""
EXTRA = ("tmp/oxm-extract/retail/default.xbe",)
"""Test inputs reached through a directory link locally (LINKS links `tmp/oxm-extract`)."""


def link_paths() -> list[str]:
    """Relative paths of the gitignored owner file links (derived from `core.LINKS`)."""
    paths = [
        spec.path
        for spec in core.LINKS
        if spec.kind == core.FILE and spec.required and not spec.path.startswith("tools/")
    ]
    return paths + [path for path in EXTRA if path not in paths]


def sources(root: Path) -> list[tuple[str, Path]]:
    """(relative path, local file) of every link source the owner PC holds."""
    found = []
    for rel in link_paths():
        path = (root / rel).resolve()
        if path.is_file():
            found.append((rel, path))
    return found


def store_dir(data_root: str) -> str:
    """Remote store path, `$HOME`-relative when `data_root` starts with `~/`."""
    return f"{data_root.removeprefix('~/')}/{STORE_REL}"


def link_lines(data_root: str) -> list[str]:
    """Shell lines (run with `$SRC` set) linking the stored files into the exec tree."""
    if not data_root:
        return []
    store = (
        f'"$HOME"/{shlex.quote(store_dir(data_root))}'
        if data_root.startswith("~/")
        else (shlex.quote(f"{data_root}/{STORE_REL}"))
    )
    lines = []
    for rel in link_paths():
        lines.append(
            f"if [ -f {store}/{shlex.quote(rel)} ]; then "
            f'mkdir -p "$SRC"/{shlex.quote(str(Path(rel).parent))}'
            f' && ln -sfn {store}/{shlex.quote(rel)} "$SRC"/{shlex.quote(rel)}; fi'
        )
    return lines
