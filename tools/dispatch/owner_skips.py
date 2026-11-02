"""A pytest SKIP for a missing owner input never counts as a pass (rule 48, cx-ownerxbe).

MEASURED 2026-10-31: the laptop had no `build/default.xbe`, so the dsound movie oracle tests
SKIPPED there and the run reported green. `tools.dispatch.pytest_jobs` runs remote pytest with
`-rs`; files that skipped for an owner input are re-run on the owner PC (which holds them)."""

from __future__ import annotations

import re

SKIP_LINE = re.compile(r"^SKIPPED \[\d+\] (tests/[^\s:]+\.py):\d+: (.*)$", re.M)
OWNER_REASON = re.compile(
    r"default\.xbe|\bretail (?:xbe|image)\b|owner (?:file|input)|disc image|tmp/owner|\bXBE\b",
    re.I,
)


def skipped_files(output: str) -> list[str]:
    """Test files with a skip whose reason names a missing owner input."""
    return sorted({f for f, reason in SKIP_LINE.findall(output) if OWNER_REASON.search(reason)})
