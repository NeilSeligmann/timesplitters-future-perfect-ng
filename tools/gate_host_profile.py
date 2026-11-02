# SPDX-License-Identifier: GPL-3.0-or-later
"""Host-profile and liveness contracts for capture gates."""

from __future__ import annotations

from pathlib import Path
from typing import Any

from tools import regress
from tools.wsola_gate_activity import check as check_wsola_activity
from tools.wt import build_profile


def validate_profiles(root: Path, candidate: str, baseline: str, *, same_host: bool) -> None:
    """Names and membership come from the build-profile owner, not a copied list."""
    for name in {candidate, baseline}:
        build_profile.load(root, name)
    if same_host and candidate != baseline:
        raise ValueError("same-host gate requires identical build profiles")


def input_hash(backend: Any, commit: str, profile: str) -> str:
    return backend.input_hash(commit) if profile == "gate" else backend.input_hash(commit, profile)


def validate_wsola_run(trace: dict, log: str, expected: str, minimum_seconds: float) -> str | None:
    meta = trace.get("meta") or {}
    if meta.get("host_exit") != 0:
        return "host did not exit cleanly"
    seconds = meta.get("seconds")
    if not isinstance(seconds, (int, float)) or seconds < minimum_seconds:
        return "fault-free duration below requirement"
    end, recorded = meta.get("route_end_poll"), meta.get("recorded_route_end_poll")
    if not isinstance(end, int) or not isinstance(recorded, int) or end < recorded:
        return "recorded route end not reached"
    if regress.scan_log(log) is not None:
        return "fault detected in run log"
    ok, reason = check_wsola_activity(log, expected)
    return None if ok else reason
