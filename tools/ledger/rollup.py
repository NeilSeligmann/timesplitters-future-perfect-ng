# SPDX-License-Identifier: GPL-3.0-or-later
"""Rollups per subsystem tree node and per vertical slice, and the next-to-port ranking."""

from __future__ import annotations

from collections.abc import Mapping, Sequence

from tools.ledger.dispositions import disposition_of
from tools.ledger.execution import Weights, readiness, score
from tools.ledger.model import BLOCKED, DONE, NA, OBLIGATION_IDS, OPEN


def _ancestors(path: str) -> list[str]:
    parts = path.split("/")
    return ["/".join(parts[: i + 1]) for i in range(len(parts))]


def _empty() -> dict[str, object]:
    return {
        "functions": 0,
        "complete": 0,
        "obligations": {name: {DONE: 0, OPEN: 0, BLOCKED: 0, NA: 0} for name in OBLIGATION_IDS},
    }


def _add(bucket: dict[str, object], row: Mapping[str, object]) -> None:
    bucket["functions"] = int(bucket["functions"]) + 1  # type: ignore[call-overload]
    obligations = row["obligations"]
    assert isinstance(obligations, dict)
    if all(o["status"] in (DONE, NA) for o in obligations.values()):
        bucket["complete"] = int(bucket["complete"]) + 1  # type: ignore[call-overload]
    table = bucket["obligations"]
    assert isinstance(table, dict)
    for name in OBLIGATION_IDS:
        table[name][obligations[name]["status"]] += 1


def rollup(rows: Sequence[Mapping[str, object]]) -> dict[str, dict[str, dict[str, object]]]:
    tree: dict[str, dict[str, object]] = {}
    slices: dict[str, dict[str, object]] = {}
    for row in rows:
        for node in _ancestors(str(row["subsystem"])):
            _add(tree.setdefault(node, _empty()), row)
        slice_names = row["slices"]
        assert isinstance(slice_names, list)
        for slice_name in slice_names:
            _add(slices.setdefault(slice_name, _empty()), row)
    return {"tree": dict(sorted(tree.items())), "slices": dict(sorted(slices.items()))}


def rank(
    rows: Sequence[Mapping[str, object]],
    *,
    limit: int,
    slice_name: str | None = None,
    subsystem: str | None = None,
    weights: Weights | None = None,
    weight_mode: str = "none",
    table: Mapping[str, str] | None = None,
    include_replace: bool = False,
    stats: dict[str, int] | None = None,
) -> list[dict[str, object]]:
    """Rule 14: drop systems are skipped (counted in stats["drop"]), replace systems are left out
    unless include_replace and then rank last; port, spec and undecided rank normally.

    Next to port: candidates are named game functions whose rust_port is not done.
    Order: execution score first when weighted (calls per frame x done fraction of the applicable
    obligations; mode rate drops the readiness factor), then fewest open obligations, then in a
    vertical slice, then fewest unported callees, then VA."""
    ported = {str(r["va"]) for r in rows if r["obligations"]["rust_port"]["status"] == DONE}  # type: ignore[index]
    out = []
    for row in rows:
        obligations = row["obligations"]
        assert isinstance(obligations, dict)
        if obligations["rust_port"]["status"] == DONE or obligations["name"]["status"] != DONE:
            continue
        if obligations["reference"]["status"] == NA:
            continue
        if slice_name and slice_name not in row["slices"]:  # type: ignore[operator]
            continue
        if subsystem and not (
            str(row["subsystem"]) == subsystem or str(row["subsystem"]).startswith(subsystem + "/")
        ):
            continue
        disposition = disposition_of(str(row["subsystem"]), table) if table else "port"
        if disposition == "drop" or (disposition == "replace" and not include_replace):
            if stats is not None:
                stats[disposition] = stats.get(disposition, 0) + 1
            continue
        counts = row["counts"]
        assert isinstance(counts, dict)
        open_total = counts[OPEN] + counts[BLOCKED]
        deps = row["deps"]
        assert isinstance(deps, list)
        unported = sum(1 for dep in deps if dep not in ported)
        in_slice = bool(row["slices"])
        weight_columns: dict[str, object] = {}
        row_score = 0.0
        if weights is not None and weight_mode != "none":
            va_int = int(str(row["va"]), 16)
            rate, ready = weights.rate(va_int), readiness(counts)
            row_score = score(weight_mode, rate, ready)
            weight_columns = {
                "calls": weights.calls.get(va_int, 0),
                "calls_per_frame": rate,
                "ready": ready,
                "score": row_score,
            }
        out.append(
            {
                **weight_columns,
                "va": row["va"],
                "name": row["name"],
                "subsystem": row["subsystem"],
                "slices": row["slices"],
                "open": open_total,
                "unported_callees": unported,
                "open_obligations": [
                    n for n in OBLIGATION_IDS if obligations[n]["status"] in (OPEN, BLOCKED)
                ],
                "_key": (
                    disposition == "replace",
                    -row_score,
                    open_total,
                    not in_slice,
                    unported,
                    str(row["va"]),
                ),
            }
        )
    out.sort(key=lambda item: item["_key"])  # type: ignore[arg-type,return-value]
    for item in out:
        del item["_key"]
    return out[:limit]
