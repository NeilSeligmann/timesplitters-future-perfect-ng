# SPDX-License-Identifier: GPL-3.0-or-later
"""Live hits against the replay evidence, per subsystem (T3824).

For every port-class function the replay evidence (tools.t2reach execution sources without the
live census) never saw execute:
  ran-live-uncaptured  a live owner session ran it: a replay or capture gap
  never-ran            no live session ran it either: a recording gap
"""

from __future__ import annotations

from collections import Counter
from collections.abc import Iterable, Mapping

LIVE_GAP = "ran-live-uncaptured"
NEVER = "never-ran"


def breakdown(
    rows: Iterable[Mapping[str, object]],
    replay_executed: frozenset[int] | set[int],
    live: frozenset[int],
) -> dict[str, Counter[str]]:
    """{subsystem: Counter(LIVE_GAP / NEVER)} over port rows without replay execution evidence."""
    out: dict[str, Counter[str]] = {}
    for row in rows:
        va = int(str(row["va"]), 16)
        if va in replay_executed:
            continue
        system = str(row.get("subsystem") or "unknown")
        out.setdefault(system, Counter())[LIVE_GAP if va in live else NEVER] += 1
    return out


def render(table: Mapping[str, Counter[str]], runs: int) -> str:
    lines = [
        f"live census: {runs} live run(s); port functions without replay execution "
        "evidence, per subsystem",
        f"{'subsystem':<28} {LIVE_GAP:>20} {NEVER:>10}",
    ]
    total: Counter[str] = Counter()
    for system in sorted(table, key=lambda name: (-sum(table[name].values()), name)):
        counts = table[system]
        total.update(counts)
        lines.append(f"{system:<28} {counts[LIVE_GAP]:>20} {counts[NEVER]:>10}")
    lines.append(f"{'total':<28} {total[LIVE_GAP]:>20} {total[NEVER]:>10}")
    return "\n".join(lines)
