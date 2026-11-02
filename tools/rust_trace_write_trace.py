"""T2121: write-trace summary for lane memory mutants.

The mutation host (src/host/rust_lane_mutation.c) decides each flipped byte at the first
XInputGetState poll after the flip (the poll the rust_trace sampler sees) and prints
`MUTSURV <va> <address> flips=N seen=S lost=L superseded=U`: seen if the byte still held the
flipped value, lost if something rewrote it first, superseded if the root flipped it again
before any poll. The last line per (va, address) is the final count. Classes: `intra-poll`
(every decided flip was lost: overwritten within one poll period, so per-poll sampling cannot
see it), `visible` (every decided flip reached a poll), `mixed`, or `undecided` (no poll after
the flip, for example the run ended).

    python3 -m tools.rust_trace_write_trace RUN/run.log [RUN2/run.log ...] [--json]
"""

from __future__ import annotations

import argparse
import json
import re
from pathlib import Path

LINE = re.compile(
    r"^MUTSURV ([0-9a-f]{8}) ([0-9a-f]{8}) flips=(\d+) seen=(\d+) lost=(\d+) superseded=(\d+)$"
)
FLIP = re.compile(r"^MUTWRITE ([0-9a-f]{8}) ([0-9a-f]{8})$")


def parse(text: str) -> dict[tuple[int, int], tuple[int, int, int, int]]:
    """Final (flips, seen, lost, superseded) per (va, address).

    Flips with no MUTSURV line yet are undecided.
    """
    counts: dict[tuple[int, int], tuple[int, int, int, int]] = {}
    flips: dict[tuple[int, int], int] = {}
    for line in text.splitlines():
        line = line.strip()
        flip = FLIP.match(line)
        if flip:
            key = (int(flip[1], 16), int(flip[2], 16))
            flips[key] = flips.get(key, 0) + 1
            continue
        match = LINE.match(line)
        if match:
            key = (int(match[1], 16), int(match[2], 16))
            counts[key] = (int(match[3]), int(match[4]), int(match[5]), int(match[6]))
    for key, total in flips.items():
        _, seen, lost, superseded = counts.get(key, (0, 0, 0, 0))
        counts[key] = (total, seen, lost, superseded)
    return counts


def classify(seen: int, lost: int) -> str:
    if seen + lost == 0:
        return "undecided"
    if seen == 0:
        return "intra-poll"
    return "visible" if lost == 0 else "mixed"


def summarise(counts: dict[tuple[int, int], tuple[int, int, int, int]]) -> list[dict[str, object]]:
    rows = []
    for (va, address), (flips, seen, lost, superseded) in sorted(counts.items()):
        rows.append(
            {
                "va": f"0x{va:X}",
                "address": f"0x{address:08x}",
                "flips": flips,
                "seen": seen,
                "lost": lost,
                "superseded": superseded,
                "class": classify(seen, lost),
            }
        )
    return rows


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("logs", nargs="+", type=Path)
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args(argv)
    for log in args.logs:
        rows = summarise(parse(log.read_text(errors="replace")))
        if args.json:
            print(json.dumps({"log": str(log), "rows": rows}))
            continue
        for row in rows:
            print(
                f"{log} {row['va']} {row['address']} flips={row['flips']} "
                f"seen={row['seen']} lost={row['lost']} "
                f"superseded={row['superseded']} {row['class']}"
            )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
