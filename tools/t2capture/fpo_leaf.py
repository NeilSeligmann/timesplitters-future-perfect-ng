# SPDX-License-Identifier: GPL-3.0-or-later
"""Lifted `fpo_leaf` roots for the capture-based Tier 2 compare (exit_regs rule v4).

`python3 -m tools.t2capture.fpo_leaf --write --lift <tmp/private-host/lift-...>` lists every
function whose lifted body opens with `ebp = g_seh_ebp; /* fpo_leaf: inherit caller's frame */`.
Such a body returns with EBP = the entry `seh_ebp`, while hardware (and the Rust port) keep the
entry EBP. The table is `generated/t2capture/fpo-leaf.json` (lift-derived, untracked), keyed by
the lift tree name. The runner passes `--fpo-leaf` only when the table's tree is the receipt's
lifted tree. A missing table or another tree means strict (fail closed).
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
TABLE_PATH = Path("generated/t2capture/fpo-leaf.json")
FUNCTION = re.compile(r"^void sub_([0-9A-F]{8})\(void\)$")
MARKER = "ebp = g_seh_ebp; /* fpo_leaf: inherit caller's frame */"
LABEL = re.compile(r"^loc_[0-9A-F]{8}:")
# v6: the prologue ends at the first `loc_` label. 12 fixed lines missed the roots whose prologue
# also defines the x87 fp_push macros (marker at lines 14 to 19, 1,033 of 4,924 in c2a28a25).
# A body with no label within this bound is ambiguous: not fpo_leaf (fail closed).
PROLOGUE_LIMIT = 200


def scan(gen_dir: Path) -> list[int]:
    """Function VAs whose lifted prologue loads EBP from g_seh_ebp (the fpo_leaf form)."""
    found: list[int] = []
    for source in sorted(gen_dir.glob("recomp_*.c")):
        lines = source.read_text(errors="replace").splitlines()
        for index, line in enumerate(lines):
            match = FUNCTION.match(line)
            if not match:
                continue
            if _prologue_has_marker(lines[index + 1 : index + 1 + PROLOGUE_LIMIT]):
                found.append(int(match.group(1), 16))
    return sorted(set(found))


def _prologue_has_marker(body: list[str]) -> bool:
    """The marker before the first `loc_` label; no label (body end or limit) = not fpo_leaf."""
    marker = False
    for text in body:
        if LABEL.match(text):
            return marker
        if text == "}" or FUNCTION.match(text):
            return False  # the body ended without a label: never read the next function
        marker = marker or MARKER in text
    return False


def table_path() -> Path:
    local = ROOT / TABLE_PATH
    if local.exists():
        return local
    from tools.trace_store import main_root

    return main_root() / TABLE_PATH


def load(tree: str, path: Path | None = None) -> frozenset[int]:
    """fpo_leaf VAs for lift tree `tree`; empty (strict) when missing or for another tree."""
    target = path or table_path()
    if not target.exists():
        print(f"fpo_leaf: no table at {target}, EBP compared strictly", file=sys.stderr)
        return frozenset()
    table = json.loads(target.read_text())
    if table.get("lift_tree") != tree:
        print(
            f"fpo_leaf: table is for {table.get('lift_tree')!r}, not {tree!r}: EBP strict",
            file=sys.stderr,
        )
        return frozenset()
    return frozenset(int(va, 16) for va in table.get("fpo_leaf", []))


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        prog="python3 -m tools.t2capture.fpo_leaf", description=__doc__
    )
    parser.add_argument("--lift", type=Path, required=True, help="lift dir (holds gen/)")
    parser.add_argument("--write", action="store_true", help=f"write {TABLE_PATH} (main checkout)")
    args = parser.parse_args(argv)
    gen = args.lift / "gen"
    if not gen.is_dir():
        print(f"fpo_leaf: no {gen}", file=sys.stderr)
        return 2
    vas = scan(gen)
    table = {
        "schema": 1,
        "task": "T3738",
        "lift_tree": args.lift.name,
        "fpo_leaf": [f"0x{va:08X}" for va in vas],
    }
    if args.write:
        from tools.trace_store import main_root

        target = main_root() / TABLE_PATH
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(json.dumps(table, indent=1) + "\n")
        print(f"wrote {target}")
    print(f"lift_tree={args.lift.name} fpo_leaf={len(vas)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
