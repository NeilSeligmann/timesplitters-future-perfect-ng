# SPDX-License-Identifier: GPL-3.0-or-later
"""Diff two ledger snapshots. A regression is an obligation that was done and no longer is."""

from __future__ import annotations

from collections.abc import Mapping

from tools.ledger.model import DONE, OBLIGATION_IDS


def _status(row: Mapping[str, object] | None, obligation: str) -> str:
    if row is None:
        return "absent"
    obligations = row["obligations"]
    assert isinstance(obligations, dict)
    return str(obligations[obligation]["status"]) if obligation in obligations else "absent"


def compare(
    old: Mapping[str, Mapping[str, object]], new: Mapping[str, Mapping[str, object]]
) -> dict[str, list[dict[str, str]]]:
    regressions, improvements, changes = [], [], []
    for va in sorted(set(old) | set(new)):
        for obligation in OBLIGATION_IDS:
            before, after = _status(old.get(va), obligation), _status(new.get(va), obligation)
            if before == after:
                continue
            item = {
                "va": va,
                "name": str((new.get(va) or old.get(va) or {}).get("name", "")),
                "obligation": obligation,
                "before": before,
                "after": after,
            }
            if before == DONE:
                regressions.append(item)
            elif after == DONE:
                improvements.append(item)
            else:
                changes.append(item)
    removed = [
        {"va": va, "name": str(old[va].get("name", ""))} for va in sorted(set(old) - set(new))
    ]
    added = [{"va": va, "name": str(new[va].get("name", ""))} for va in sorted(set(new) - set(old))]
    return {
        "regressions": regressions,
        "improvements": improvements,
        "changes": changes,
        "functions_removed": removed,
        "functions_added": added,
    }


def render(result: Mapping[str, list[dict[str, str]]]) -> str:
    lines = [f"{key}: {len(result[key])}" for key in result]
    for item in result["regressions"]:
        lines.append(
            f"REGRESSION {item['va']} {item['name']} {item['obligation']}: {item['before']} -> {item['after']}"  # noqa: E501
        )
    for item in result["functions_removed"]:
        lines.append(f"REMOVED {item['va']} {item['name']} (all its done obligations are lost)")
    return "\n".join(lines)


def has_regression(result: Mapping[str, list[dict[str, str]]]) -> bool:
    return bool(result["regressions"] or result["functions_removed"])
