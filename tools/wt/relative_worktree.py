"""Service scratch worktrees that survive a different mount path of the same repo (T3784).

Containers mount the one repo at different roots (/workspace, <private>). A
worktree made by `git worktree add` stores ABSOLUTE links both ways, so a tree made in one
container does not resolve in another, and `git worktree prune` run in another container sees
the admin dir's gitdir as missing and deletes it (I-T3784-1). git here is 2.47 (no
`--relative-paths`, that is 2.48+), so this module:

* writes `<tree>/.git` as `gitdir: <path relative to the tree>` (resolves from any mount);
* writes `<admin>/gitdir` relative to the repo top level, the path 2.47 resolves against when
  prune runs at the top level (MEASURED: an admin-relative path is pruned there);
* LOCKS the worktree, because 2.47 resolves a relative admin gitdir against the cwd, so a prune
  run from inside another worktree would still see it as missing. Locked trees are never pruned.

`ensure` validates a tree every call and rebuilds it if broken, removing only its OWN admin dir
(never a global prune: run in one container it would prune every worktree made in another).
"""

from __future__ import annotations

import os
import shutil
import subprocess
from pathlib import Path

LOCK_REASON = "service scratch tree with relative links (tools.wt.relative_worktree, T3784)"


def _git(cwd: Path, *args: str, timeout: float = 900) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ["git", *args], cwd=cwd, capture_output=True, text=True, timeout=timeout, check=False
    )


def common_dir(repo: Path) -> Path:
    done = _git(repo, "rev-parse", "--path-format=absolute", "--git-common-dir", timeout=60)
    if done.returncode != 0:
        raise RuntimeError(f"{repo}: not a git repository: {done.stderr.strip()}")
    return Path(done.stdout.strip())


def admin_dir(tree: Path) -> Path | None:
    """The admin dir `<tree>/.git` names (relative or absolute), or None if unreadable."""
    try:
        text = (tree / ".git").read_text().strip()
    except OSError:
        return None
    if not text.startswith("gitdir:"):
        return None
    return Path(os.path.normpath(tree / text.split(":", 1)[1].strip()))


def is_valid(tree: Path) -> bool:
    admin = admin_dir(tree)
    if admin is None or not (admin / "HEAD").is_file() or not (admin / "commondir").is_file():
        return False
    return _git(tree, "rev-parse", "--git-dir", timeout=60).returncode == 0


def _own_admin_dirs(common: Path, tree: Path) -> list[Path]:
    """Admin dirs registered for `tree`, by the name its .git file gives or by its gitdir path."""
    found = []
    named = admin_dir(tree)
    if named is not None and named.parent == common / "worktrees" and named.is_dir():
        found.append(named)
    suffix = os.path.relpath(tree / ".git", common.parent)  # same under any mount prefix
    for admin in sorted((common / "worktrees").glob("*")):
        try:
            text = (admin / "gitdir").read_text().strip()
        except OSError:
            continue
        if admin not in found and (text == suffix or text.endswith(os.sep + suffix)):
            found.append(admin)
    return found


def relativize(tree: Path) -> Path:
    """Rewrite both links of an existing worktree to relative paths and lock it."""
    admin = admin_dir(tree)
    if admin is None or not admin.is_dir():
        raise RuntimeError(f"{tree}: no admin dir to relativize")
    top = admin.parent.parent.parent  # <top>/.git/worktrees/<name>
    (tree / ".git").write_text(f"gitdir: {os.path.relpath(admin, tree)}\n")
    (admin / "gitdir").write_text(os.path.relpath(tree / ".git", top) + "\n")
    (admin / "locked").write_text(LOCK_REASON + "\n")
    return admin


def prepare_removal(tree: Path) -> None:
    """Unlock and write absolute links for THIS mount: `git worktree remove` needs both."""
    admin = admin_dir(tree)
    if admin is None or not admin.is_dir():
        return
    (admin / "locked").unlink(missing_ok=True)
    (admin / "gitdir").write_text(os.path.abspath(tree / ".git") + "\n")
    (tree / ".git").write_text(f"gitdir: {admin}\n")


def ensure(repo: Path, tree: Path, sha: str, *, scratch_root: Path) -> bool:
    """Make `tree` a valid relative, locked worktree; True if it had to be (re)created.

    `scratch_root` bounds the deletion: a broken tree is removed only if it lies under it.
    """
    if is_valid(tree):
        relativize(tree)  # idempotent: also converts a tree made before T3784
        return False
    tree_abs = Path(os.path.abspath(tree))
    root_abs = Path(os.path.abspath(scratch_root))
    if tree_abs == root_abs or root_abs not in tree_abs.parents:
        raise RuntimeError(f"refusing to heal {tree}: not under scratch root {scratch_root}")
    common = common_dir(repo)
    for admin in _own_admin_dirs(common, tree_abs):
        shutil.rmtree(admin)
    if tree_abs.exists() or tree_abs.is_symlink():
        shutil.rmtree(tree_abs)
    tree_abs.parent.mkdir(parents=True, exist_ok=True)
    done = _git(repo, "worktree", "add", "--quiet", "--detach", str(tree_abs), sha)
    if done.returncode != 0:
        raise RuntimeError(f"git worktree add {tree}: {done.stderr.strip()[-400:]}")
    relativize(tree_abs)
    return True
