"""Host-side git invariant (T3678, incident 2026-10-25): every git command that runs ON a worker
host carries ``GIT_NO_LAZY_FETCH=1``.

The worker repos (``~/rea-codex2/repo``) are blobless partial clones by design. A host-side git
command that touches a missing object silently lazy-fetches from the git server and can hang; our
local ssh timeout then leaves it running on the host (95 piled up on the laptop, load 76). With the
env set, a missing object is an immediate error instead. Explicit ``git fetch`` and ``git push`` are
unaffected. ``tests/test_no_lazy_fetch_guard.py`` makes modules that build remote commands use this.
"""

from __future__ import annotations

NO_LAZY_ENV = "GIT_NO_LAZY_FETCH=1"
NO_LAZY_EXPORT = f"export {NO_LAZY_ENV}; "


def remote_git(args: str) -> str:
    """One remote git command line with the env in force (``args`` is already shell-quoted)."""
    return f"{NO_LAZY_ENV} git {args}"


def with_no_lazy(script: str) -> str:
    """A remote shell script with the env exported before its first command."""
    return NO_LAZY_EXPORT + script
