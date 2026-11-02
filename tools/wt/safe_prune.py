"""`git worktree prune` that never drops another mount's live worktree (T3784, I-T3784-2).

Plain prune deletes every admin dir whose gitdir path does not exist HERE. A worktree created
in a container that mounts the repo at another prefix (tools/data/repo_mounts.json) has an
absolute gitdir under that prefix, which never exists here, so plain prune from /workspace
deleted the host-side t2-continuous checker (and from the host it would delete every agent
worktree). `prune` asks git which entries it would remove (`--dry-run --verbose`, same
`--expire`), keeps every entry whose gitdir lies under a known mount prefix other than the
one `main` is under (treated as LIVE), and removes the rest itself, only inside
`<common dir>/worktrees`.
"""

from __future__ import annotations

import os
import re
import shutil
import subprocess
from pathlib import Path

from tools.agents import canonical_mount

REMOVING = re.compile(r"^Removing worktrees/([^:/]+):")


class PruneError(RuntimeError):
    pass


def _git(main: Path, *args: str, timeout: float) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ["git", *args], cwd=main, capture_output=True, text=True, timeout=timeout, check=False
    )


def foreign(gitdir_text: str, main: Path, mounts: canonical_mount.Mounts | None = None) -> bool:
    """True if this admin gitdir names a path under a known mount prefix other than main's."""
    if not os.path.isabs(gitdir_text):
        return False
    mounts = mounts or canonical_mount.load()
    theirs = canonical_mount.prefix_of(gitdir_text, mounts)
    return theirs is not None and theirs != canonical_mount.prefix_of(main, mounts)


def prune(
    main: Path,
    *,
    expire: str | None = None,
    timeout: float = 60,
    mounts: canonical_mount.Mounts | None = None,
) -> tuple[list[str], list[str]]:
    """Prune dead registrations; return (removed, kept-as-foreign) admin names."""
    extra = ["--expire", expire] if expire else []
    done = _git(main, "worktree", "prune", "--dry-run", "--verbose", *extra, timeout=timeout)
    if done.returncode:
        raise PruneError(f"worktree prune --dry-run failed: {done.stderr.strip()[-400:]}")
    common = _git(main, "rev-parse", "--path-format=absolute", "--git-common-dir", timeout=timeout)
    if common.returncode:
        raise PruneError(f"{main}: no git common dir: {common.stderr.strip()[-400:]}")
    admin_root = Path(common.stdout.strip()) / "worktrees"
    removed, kept = [], []
    for line in (done.stdout + done.stderr).splitlines():
        match = REMOVING.match(line.strip())
        if not match:
            continue
        admin = admin_root / match.group(1)
        try:
            text = (admin / "gitdir").read_text().strip()
        except OSError:
            text = ""
        if foreign(text, main, mounts):
            kept.append(admin.name)
            continue
        if admin.parent == admin_root and admin.is_dir():
            shutil.rmtree(admin)
            removed.append(admin.name)
    if not any(admin_root.iterdir()) if admin_root.is_dir() else False:
        admin_root.rmdir()  # git removes the empty dir too
    return removed, kept
