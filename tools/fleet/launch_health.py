"""Per-host launch health: do launched Codex workers complete steps? (T3763 follow-up, I-T3763-3)

On 2026-10-26 every remote Codex worker failed every step for about 9 h (`run.sh` passed
`-c mcp_servers.godot-ai.enabled=false`, which the remote Codex config rejects with "invalid
transport"). The refill kept popping queue ids for those hosts and 25 Tier 2 items were lost.

The runner (`tools/fleet/runner/run.sh.in`) appends one line per event to
`<codexapi>/step-outcomes.log`: `<epoch> <lane> start|ok|fail`, stops a lane after
`FAIL_STOP` failed steps with none completed, and appends that lane's queue id to
`<codexapi>/requeue.txt`. The status probe turns the last `WINDOW_S` of the log into
`steps_start`, `steps_ok` and `steps_failed` (and `requeue_ids`), so:

- `scale.refill_blocker` returns `launch-failing` and no ids are popped for that host;
- the HEALTH detector `host-launch-failing` names the host;
- the balancer puts the re-queue ids back at the head of the shared queue (once per id per day).
"""

from __future__ import annotations

from collections.abc import Mapping
from dataclasses import dataclass

WINDOW_S = 1800
MIN_FAILED = 2
"""A first step may legitimately run 45 min, so only failures (never mere silence) count."""
OUTCOMES = "step-outcomes.log"
REQUEUE = "requeue.txt"

PROBE_SH = (
    "awk -v t=$(($(date +%s)-" + str(WINDOW_S) + ")) "
    '\'$1>=t{c[$3]++} END{printf "steps_start=%d steps_ok=%d steps_failed=%d\\n", '
    'c["start"], c["ok"], c["fail"]}\' ~/csup/codexapi/' + OUTCOMES + " 2>/dev/null; "
    "[ -s ~/csup/codexapi/"
    + REQUEUE
    + ' ] && echo "requeue_ids=$(tail -n 20 ~/csup/codexapi/'
    + REQUEUE
    + ' | paste -sd, -)"; true'
)
"""Appended to the remote status probe (tools.fleet.status.PROBE_SH)."""


@dataclass(frozen=True)
class Counts:
    launches: int = 0
    ok: int = 0
    failed: int = 0


def counts(text: str, now: float, window: float = WINDOW_S) -> Counts:
    """Events of the last `window` seconds of a step-outcomes log."""
    tally = {"start": 0, "ok": 0, "fail": 0}
    for line in text.splitlines():
        parts = line.split()
        if len(parts) < 3 or parts[2] not in tally:
            continue
        try:
            when = float(parts[0])
        except ValueError:
            continue
        if when >= now - window:
            tally[parts[2]] += 1
    return Counts(tally["start"], tally["ok"], tally["fail"])


def fields(found: Counts) -> dict[str, str]:
    return {
        "steps_start": str(found.launches),
        "steps_ok": str(found.ok),
        "steps_failed": str(found.failed),
    }


def from_fields(status: Mapping[str, str]) -> Counts:
    def number(key: str) -> int:
        try:
            return int(float(status.get(key, 0)))
        except ValueError:
            return 0

    return Counts(number("steps_start"), number("steps_ok"), number("steps_failed"))


def failing(status: Mapping[str, str]) -> bool:
    """Launches in the window, at least MIN_FAILED failed steps and not one completed."""
    found = from_fields(status)
    return found.ok == 0 and found.failed >= MIN_FAILED


def requeue_ids(status: Mapping[str, str], local_text: str = "") -> list[str]:
    """Ids a host's stopped lanes handed back (probe field, or the local requeue file)."""
    raw = status.get("requeue_ids", "").split(",") + local_text.split()
    return list(dict.fromkeys(item.strip() for item in raw if item.strip()))
