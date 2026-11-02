"""Canonical full-host builds: one shared source tree and target per input family.

The source and lock live on the same shared filesystem as the target. A
container-local source root must never share these artifacts (T3712). Unlike
arbitrary shell commands, the repository's build entrypoint has a known cwd and
output contract, so it can safely participate in content-based synchronization.
"""

from __future__ import annotations

import os
import re
import shlex
import sys
from collections.abc import Callable
from pathlib import Path

ENTRY = Path("tools/dev/build.sh")
PREFIX = "host-"


def argv(command: list[str], root: Path) -> list[str] | None:
    words = list(command)
    if len(words) == 3 and Path(words[0]).name in {"sh", "bash"} and words[1] == "-c":
        if re.search(r"[$`<>|&;\n]", words[2]):
            return None  # Preserve shell expansion/redirection in the isolated path.
        try:
            words = shlex.split(words[2])
        except ValueError:
            return None
        if any(word in {"&&", "||", ";", "|", "&"} or word.endswith(";") for word in words):
            return None
    if len(words) < 2 or Path(words[0]).name not in {"sh", "bash"}:
        return None
    script = Path(words[1])
    if script.is_absolute():
        if script != root / ENTRY:
            return None
    elif script != ENTRY:
        return None
    # Relifting may modify sources. Keep that explicit maintenance operation in
    # its original checkout instead of discarding generated edits in a mirror.
    if "--relift" in words[2:]:
        return None
    return words


def prepare(
    root: Path,
    main: Path,
    command: list[str],
    env: dict[str, str],
    cwd: Path,
    key: str,
    git: Callable[[Path, list[str]], bytes],
    log: Callable[[str], object] | None = None,
) -> tuple[list[str], Path, int] | None:
    from tools.wt import canonical_build as canon

    words = argv(command, root)
    if words is None or not canon.enabled(env):
        return None
    if cwd.resolve() != root.resolve():
        raise RuntimeError("canonical host build must start at the worktree root")
    log = log or (lambda text: print(text, file=sys.stderr, flush=True))
    base = main / canon.CANON_REL
    base.mkdir(parents=True, exist_ok=True)
    name = PREFIX + key
    # One member, independent of local pool preferences and container-local
    # TSFP_CANON_ROOT. The shared lock covers synchronization and all children.
    member, fd, waited = canon.acquire_member(base, name, 1, root, log)
    try:
        mirror = base / member
        ident = canon.base_id(base)
        target = main / "tmp/wt-cache/cargo-target" / name
        foreign = canon.claim_target(target, ident)
        if foreign is not None:
            raise RuntimeError("canonical host target belongs to a different source base")
        stats = canon.sync(root, mirror, git)
        os.set_inheritable(fd, True)
        env["CARGO_TARGET_DIR"] = str(target)
        env["PWD"] = str(mirror)
        # tools.wt acquired the build slot before calling prepare. Prevent
        # build.sh from entering a second canonical tree while holding this lock.
        env["TSFP_BUILD_SLOT_HELD"] = "1"
        log(
            f"tools.wt: host family {key}: {stats['copied']} changed, "
            f"{stats['same']} unchanged sources; lock wait {waited:.3f}s"
        )
        return [canon._map_path(word, root, mirror) for word in words], mirror, fd
    except BaseException:
        os.close(fd)
        # Do not silently run unsynchronized sources against the shared target.
        raise
