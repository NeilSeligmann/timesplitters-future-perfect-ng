# SPDX-License-Identifier: GPL-3.0-or-later
"""Coverage gaps: executed port functions with few corpus shapes, joined to T3706 routes (T3741)."""

from __future__ import annotations

import csv
import json
from collections.abc import Mapping
from pathlib import Path

from tools.ledger import dispositions

NO_ROUTE = "no recording reaches it"
# T3795: a function asked for fewer than the flat minimum (its CFG has fewer paths) is covered
# only with at least this many entries (the store keeps 8 per shape, so one full shape).
ENTRY_FLOOR = 8


def executed_port_functions(
    weights_csv: Path,
    dispositions_json: Path,
    live: frozenset[int] | set[int] = frozenset(),
    live_systems: Mapping[int, str] | None = None,
) -> dict[int, dict]:
    """Port functions with entry weights, plus (T3824) those only the live census saw run
    (calls 0: the weights come from replays, the live census has no counts). A live VA without
    a weights row takes its subsystem from `live_systems` (the ledger rows)."""
    table = dispositions.load(dispositions_json)
    out: dict[int, dict] = {}
    with Path(weights_csv).open(newline="") as src:
        rows = csv.DictReader(line for line in src if not line.startswith("#"))
        for row in rows:
            calls = int(row["calls"] or 0)
            if calls <= 0 and int(row["va"], 16) not in live:
                continue
            system = row.get("subsystem", "") or "unknown"
            if dispositions.disposition_of(system, table) != "port":
                continue
            out[int(row["va"], 16)] = {"calls": calls, "system": system}
    for va in sorted(set(live) - set(out)):
        system = (live_systems or {}).get(va)
        if system and dispositions.disposition_of(system, table) == "port":
            out[va] = {"calls": 0, "system": system}
    return out


def route_index(route_json: Path) -> dict[int, list[str]]:
    path = Path(route_json)
    if not path.exists():
        return {}
    funcs = json.loads(path.read_text()).get("functions", {})
    return {int(key, 16): list(val.get("routes", [])) for key, val in funcs.items()}


def coverage_gaps(
    executed: dict[int, dict],
    stats: dict[int, dict],
    routes: dict[int, list[str]],
    min_shapes: int,
    paths: dict[int, int] | None = None,
) -> list[dict]:
    """Executed port functions below their shape minimum: min(min_shapes, CFG path count) from
    `paths` (tools.t2capture.path_count; absent = min_shapes, fail closed). A reduced minimum also
    needs ENTRY_FLOOR entries."""
    gaps = []
    for va, info in executed.items():
        shapes = int(stats.get(va, {}).get("shapes", 0))
        entries = int(stats.get(va, {}).get("entries", 0))
        need = min(min_shapes, (paths or {}).get(va, min_shapes))
        if shapes >= need and (need >= min_shapes or entries >= ENTRY_FLOOR):
            continue
        reach = sorted(routes.get(va, []))
        gaps.append(
            {
                "va": f"0x{va:08x}",
                "system": info["system"],
                "calls": info["calls"],
                "entries": entries,
                "shapes": shapes,
                "min_shapes": need,
                "routes": reach,
                "route_note": ", ".join(reach) if reach else NO_ROUTE,
            }
        )
    gaps.sort(key=lambda gap: (-gap["calls"], gap["va"]))
    return gaps


def render_table(gaps: list[dict]) -> str:
    lines = ["| va | system | calls | entries | shapes | routes |", "|---|---|---:|---:|---:|---|"]
    for gap in gaps:
        cells = (gap["va"], gap["system"], gap["calls"], gap["entries"], gap["shapes"])
        lines.append("| " + " | ".join(str(cell) for cell in cells) + f" | {gap['route_note']} |")
    return "\n".join(lines)
