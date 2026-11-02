# SPDX-License-Identifier: GPL-3.0-or-later
"""Typed replay-clock boundaries for guest-pad-keyed sample comparisons."""

from __future__ import annotations

import re
from dataclasses import dataclass
from typing import Any

_MARK = re.compile(
    r"^TSFP_ROUTE_CLOCK_V1 host_poll=(\d+) recorded_sim_tick=(\d+) guest_pad_exclusive=(\d+)$"
)


@dataclass(frozen=True)
class RouteClock:
    host_poll: int
    recorded_sim_tick: int
    guest_pad_exclusive: int


def parse_marker(log: str, observed_host_end: int | None) -> RouteClock | None:
    """Require one complete nonzero ordered guest boundary paired to host end."""
    candidates = [line.strip() for line in log.splitlines() if "TSFP_ROUTE_CLOCK_V1" in line]
    if len(candidates) != 1:
        return None
    match = _MARK.fullmatch(candidates[0])
    if match is None:
        return None
    host, tick, guest = (int(part) for part in match.groups())
    if host <= 0 or guest <= 0 or observed_host_end != host:
        return None
    return RouteClock(host, tick, guest)


def guest_limit(traces: list[dict[str, Any]]) -> int | None:
    """Return last included guest-pad index; fail closed on missing/unequal boundaries."""
    bounds = []
    for trace in traces:
        meta = trace.get("meta") or {}
        clock = meta.get("route_clock")
        if not isinstance(clock, dict) or clock.get("domain") != "guest-pad-exclusive-v1":
            return None
        guest = clock.get("guest_pad_exclusive")
        host = clock.get("host_poll")
        if (
            type(guest) is not int
            or guest <= 0
            or type(host) is not int
            or host != meta.get("route_end_poll")
        ):
            return None
        bounds.append(guest)
    if not bounds or len(set(bounds)) != 1:
        return None
    return bounds[0] - 1
