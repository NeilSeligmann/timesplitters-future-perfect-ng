# SPDX-License-Identifier: GPL-3.0-or-later
"""Declared inlined callees for the capture-based Tier 2 compare (compare v6, INFERRED).

`tools/data/t2capture_inlined_callees.json` lists, per root VA, the callee VAs its port runs
natively instead of through a guest call (rule 20). The runner passes them as
`--inlined-callees`. The offline compare then drops such a captured call when the port does not
make it. Write-set, read-set and exit registers stay strict, and every undeclared callee
difference still diverges. Keep the list small and reviewed. Each entry names its port module
and a reason (tests/test_t2capture_inlined_callees.py checks them against the port registry).
"""

from __future__ import annotations

import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
TABLE_PATH = Path("tools/data/t2capture_inlined_callees.json")
MAX_PER_ROOT = 8  # crates/tsfp-t2-offline/src/stub.rs MAX_INLINED


def table_path(root: Path = ROOT) -> Path:
    return root / TABLE_PATH


def load(path: Path | None = None) -> dict[int, list[int]]:
    """Root VA -> declared callee VAs (empty when the table is missing: strict)."""
    target = path or table_path()
    if not target.exists():
        return {}
    roots = json.loads(target.read_text()).get("roots", {})
    return {
        int(va, 16): [int(callee, 16) for callee in entry.get("callees", [])]
        for va, entry in roots.items()
    }


def flag_value(va: int, table: dict[int, list[int]]) -> str:
    """The `--inlined-callees` value for `va` ("" = none declared)."""
    return ",".join(f"0x{callee:x}" for callee in table.get(va, []))
