"""Execution jobs in flight per remote host (rule 47, T3769 follow-up, I-T3763-7).

Remote hosts are execution-only since 2026-10-26: the light Codex lanes moved to the owner PC,
so a host's Codex worker count is 0 by design and every "workers < min" check fired forever.
A host's load is now its execution jobs:

- remote verify and remote prebuilt builds hold the host's slot locks
  (`tools.wt.remote_exec.slot_paths`, `~/.cache/tsfp-remote-exec/slot*.lock`), counted by the
  status probe as `exec_slots_busy`;
- replays hold an owner-PC lock per host slot (`tools.wt.remote_replay.exec_slot`), counted here
  as `replay_jobs`.

`exec_jobs` = the sum, compared with the host's `min_exec_jobs`
(tools/data/remote_hosts.json, default 0 = no floor).

The floor only matters while remote-runnable exec work waits (`work_waiting`: replay queue,
verify queue; I-T3769-11): an idle host with nothing to
run is fine (claude-1 decision 2026-10-26, I-T3769-3). The alert clock is `UNDERUSED_AFTER_S`.
"""

from __future__ import annotations

import json
import time
from collections.abc import Mapping
from pathlib import Path

from tools.fleet.core import flock_held

PROBE_SH = (
    'n=0; for f in ~/.cache/tsfp-remote-exec/slot*.lock; do [ -f "$f" ] && '
    '! flock -n "$f" true 2>/dev/null && n=$((n+1)); done; echo "exec_slots_busy=$n"'
)
"""Appended to the remote status probe (tools.fleet.status.PROBE_SH)."""


def replay_slot_dir() -> Path:
    from tools.wt import remote_replay  # the owner of the replay slot locks

    return remote_replay.exec_slot_dir()


def replay_jobs(names: list[str], directory: Path | None = None) -> dict[str, int]:
    """Held replay slot locks per host name (`<host>-<slot>.lock`, flock held = job running)."""
    directory = directory or replay_slot_dir()
    counts = dict.fromkeys(names, 0)
    for name in names:
        for lock in directory.glob(f"{name}-*.lock"):
            if lock.stem.removeprefix(f"{name}-").isdigit() and flock_held(lock):
                counts[name] += 1
    return counts


def exec_jobs(fields: Mapping[str, str]) -> int:
    """`exec_jobs` when the collector set it, else exec_slots_busy + replay_jobs."""
    if "exec_jobs" in fields:
        try:
            return int(float(fields["exec_jobs"]))
        except ValueError:
            pass
    total = 0
    for key in ("exec_slots_busy", "replay_jobs"):
        try:
            total += int(float(fields.get(key, 0)))
        except ValueError:
            continue
    return total


UNDERUSED_AFTER_S = 20 * 60
"""A host below its exec floor with work waiting alerts after this long (claude-1, 2026-10-26)."""

VERIFY_TICKET_FRESH_S = 90.0
"""A merge-queue ticket is live while refreshed this recently (merge_task.TICKET_FRESH_S)."""


def _waiting_replays(cache: Path, now: float) -> int:
    """Replay-queue tickets waiting for a slot (no `slot` yet) with a live heartbeat.

    Read-only view of every `tools.hostlock.HostLock` queue under the replay cache
    (`<cache>/hostlock-*/queue.json`, written atomically); a ticket whose heartbeat is older
    than the lease TTL is a dead waiter the next admission sweeps (I-T3769-11)."""
    from tools.wt import lease

    count = 0
    for path in sorted(cache.glob("hostlock-*/queue.json")):
        try:
            tickets = json.loads(path.read_text())
        except (OSError, ValueError):
            continue
        for ticket in tickets if isinstance(tickets, list) else ():
            beat = ticket.get("heartbeat") if isinstance(ticket, dict) else None
            count += (
                isinstance(ticket, dict)
                and "slot" not in ticket
                and type(beat) in (int, float)
                and now - beat <= lease.TTL_S
            )
    return count


def _queued_verifies(root: Path, now: float) -> int:
    """Live landings waiting in the merge queue (tmp/merge.queue/*): each runs a remote verify."""
    try:
        tickets = [entry for entry in (root / "tmp" / "merge.queue").iterdir() if entry.is_file()]
    except OSError:
        return 0
    live = 0
    for ticket in tickets:
        try:
            live += now - ticket.stat().st_mtime < VERIFY_TICKET_FRESH_S
        except OSError:
            continue
    return live


def work_waiting(
    root: Path, codexapi_dir: Path, now: float | None = None, replay_cache: Path | None = None
) -> str:
    """Why remote-runnable exec work is waiting ("" when none) (I-T3769-11).

    Only work a remote can take counts (rule 47): replays waiting in the replay queue and live
    merge-queue verifies (each runs a remote verify). Codex Tier 2 items run on the owner PC and
    the recapture driver keeps its queue in memory (no on-disk pending state), so neither counts.
    `codexapi_dir` stays for the callers' signature."""
    del codexapi_dir
    now = time.time() if now is None else now
    if replay_cache is None:
        from tools.wt import replay

        replay_cache = replay.cache_dir()
    reasons = []
    if replays := _waiting_replays(replay_cache, now):
        reasons.append(f"replay-queue={replays}")
    if verifies := _queued_verifies(root, now):
        reasons.append(f"verify-queue={verifies}")
    return ",".join(reasons)
