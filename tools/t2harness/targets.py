# SPDX-License-Identifier: GPL-3.0-or-later
"""Class d targets, their contracts and their order (T3796).

Read through their owners (rule 45): the class d rows from `tools.t2reach.audit.csv_path()` (rebuilt
by `python3 -m tools.t2reach audit --write`), static fan-in from the same `tools.t2reach` graph, the
port registry and recording evidence paths from `tools.t2capture.runner` /
`tools.t2capture_promote`, and the lifted body from `tools.replace.invpose_meta` (stack arity,
x87/SSE use).
"""

from __future__ import annotations

import csv
import json
import re
from collections.abc import Callable
from dataclasses import dataclass
from pathlib import Path

from tools.t2harness.generate import Contract

CONDITIONAL = re.compile(r"\)\s*goto\s+loc_[0-9A-Fa-f]+;")


@dataclass(frozen=True)
class Target:
    va: int
    subsystem: str
    fan_in: int


def class_d_rows(path: Path | None = None) -> list[dict[str, str]]:
    from tools.t2reach import audit

    source = path or audit.csv_path()
    if not source.is_file():
        raise SystemExit(f"t2harness: {source} missing: run python3 -m tools.t2reach audit --write")
    with source.open(newline="") as handle:
        return [row for row in csv.DictReader(handle) if row["class"] == "d"]


def fan_in(vas: list[int]) -> dict[int, int]:
    """Static callers of each VA in the T3793 graph (code and data referrers alike)."""
    from tools.t2reach import audit

    graph, _reach, _pins = audit.load_graph(audit.find_xbe(None))
    callers = graph.callers()
    return {va: len(callers.get(va, ())) for va in vas}


def recording_credit(root: Path) -> set[int]:
    """VAs with recording capture Tier 2 (rule 43 evidence): constructed calls never replace it."""
    from tools.t2capture_promote import EVIDENCE_PATH

    path = root / EVIDENCE_PATH
    if not path.is_file():
        raise SystemExit(f"t2harness: {path} missing (sparse checkout without docs/data?)")
    functions = json.loads(path.read_text()).get("functions", {})
    return {
        int(va, 16)
        for va, entry in functions.items()
        if str(entry.get("source", "capture")) != "constructed-call"
    }


def ordered(rows: list[dict[str, str]], fan: dict[int, int], skip: set[int]) -> list[Target]:
    """Heaviest static fan-in first, ties by address (deterministic)."""
    targets = [
        Target(int(row["va"], 16), row["subsystem"], fan.get(int(row["va"], 16), 0))
        for row in rows
        if int(row["va"], 16) not in skip
    ]
    return sorted(targets, key=lambda t: (-t.fan_in, t.va))


def paths_hint(body: str) -> int:
    return 1 + len(CONDITIONAL.findall(body))


def contract(gen_dir: Path, va: int) -> Contract:
    from tools.replace.invpose_meta import describe, lifted_body

    meta = describe(gen_dir, va)
    return Contract(
        va=va,
        stack_args=int(str(meta["stack_args"])),
        calls=bool(meta["calls"]),
        x87=bool(meta["x87"]),
        sse=bool(meta["sse"]),
        paths_hint=paths_hint(lifted_body(gen_dir, va)),
    )


def gen_dir(root: Path) -> Path:
    from tools.lane_rows import default_gen_dir

    found = default_gen_dir(root)
    if not (found / "recomp_dispatch.c").is_file():
        raise SystemExit(f"t2harness: no lifted tree at {found}")
    return found


def image_reader() -> Callable[[int], tuple[bytes, bool] | None]:
    """page VA -> (that page of the certified XBE image as loaded, zeros past a section's raw bytes;
    whether a writable section covers it), None outside the image.

    The bytes are owner data: they go only into the uncommitted harness store, never into git or
    output."""
    from tools.t2reach import audit
    from tools.xbe import parse_xbe

    data = audit.find_xbe(None).read_bytes()
    sections = parse_xbe(data).sections
    page_size = 0x1000

    def page(va: int) -> tuple[bytes, bool] | None:
        out = bytearray(page_size)
        hit = writable = False
        for section in sections:
            lo, hi = section.virtual_addr, section.virtual_addr + section.virtual_size
            start, end = max(lo, va), min(hi, va + page_size)
            if start >= end:
                continue
            hit = True
            writable = writable or bool(section.writable)
            raw_end = min(end, lo + section.raw_size)
            if start < raw_end:
                chunk = data[section.raw_addr + start - lo : section.raw_addr + raw_end - lo]
                out[start - va : start - va + len(chunk)] = chunk
        return (bytes(out), writable) if hit else None

    return page
