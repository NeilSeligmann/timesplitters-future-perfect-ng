# SPDX-License-Identifier: GPL-3.0-or-later
"""Per-function Tier 2 denominator exclusions (T3793, rule 14 "drop" for unreachable code).

Dispositions are per subsystem (`tools/data/system_dispositions.json`); a function that the
static fixpoint (`tools.t2reach`) proves unreachable is dropped one at a time through
`tools/data/tier2_exclusions.json`. The ledger never hides one: `tools.ledger tier2` counts
them apart ("z excluded as unreachable"), and REFUSES an exclusion (counts the function
normally and reports why) when

* the function has execution evidence (weights calls or Tier 2 credit): it ran, so it is
  reachable;
* a recorded static caller is not itself in the file's dead set: a real caller exists;
* its confidence is not MEASURED (a name-based guess never drives a drop).

Deleting the file, or an entry, restores the function to the denominator.
"""

from __future__ import annotations

import json
from collections.abc import Collection, Mapping
from dataclasses import dataclass, field
from pathlib import Path

DEFAULT_PATH = Path("tools/data/tier2_exclusions.json")


@dataclass(frozen=True)
class Exclusions:
    path: str
    accepted: frozenset[int] = frozenset()
    refused: Mapping[int, str] = field(default_factory=dict)


def load(path: Path) -> dict[str, object] | None:
    if not path.is_file():
        return None
    document = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(document, dict) or not isinstance(document.get("functions"), dict):
        raise SystemExit(f"{path}: not a tier2 exclusions document (needs a 'functions' map)")
    return document


def validate(
    document: Mapping[str, object] | None,
    path: Path | str,
    *,
    executed: Collection[int] = (),
    tier2: Collection[int] = (),
) -> Exclusions:
    """Split the file's functions into accepted and refused (with the reason)."""
    if document is None:
        return Exclusions(str(path))
    dead = {int(str(va), 16) for va in document.get("dead_nodes", ())}  # type: ignore[union-attr]
    ran = set(executed) | set(tier2)
    accepted: set[int] = set()
    refused: dict[int, str] = {}
    functions: Mapping[str, Mapping[str, object]] = document["functions"]  # type: ignore[assignment]
    for va_text, entry in functions.items():
        va = int(va_text, 16)
        callers = [int(str(c), 16) for c in entry.get("callers", ())]  # type: ignore[union-attr]
        real = [c for c in callers if c not in dead]
        if entry.get("confidence") != "MEASURED":
            refused[va] = f"confidence {entry.get('confidence')!r} is not MEASURED"
        elif va in ran:
            refused[va] = "has execution evidence or Tier 2 credit"
        elif real:
            refused[va] = "real caller(s) " + ", ".join(f"{c:#x}" for c in real[:5])
        elif va not in dead:
            refused[va] = "not in the dead set of the same file"
        else:
            accepted.add(va)
    return Exclusions(str(path), frozenset(accepted), refused)
