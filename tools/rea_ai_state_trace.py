# SPDX-License-Identifier: GPL-3.0-or-later
"""T2407 exact comparison of reference guest and Rust-owned REA actor rows."""

from __future__ import annotations

from pathlib import Path

# Fourteen behavior/local-state words plus ten route-navigation words.
STATE_WORDS = 24


def load(path: Path) -> list[dict]:
    rows = []
    previous_poll = 0
    for line in path.read_text().splitlines():
        fields = line.split()
        if len(fields) != 8 + STATE_WORDS:
            raise ValueError(f"REA AI trace row {len(rows)}: expected {8 + STATE_WORDS} fields")
        sample, poll, route_cursor, route_tick, actor, source, slot, generation = fields[:8]
        index, poll = int(sample), int(poll)
        route_cursor, route_tick = int(route_cursor), int(route_tick)
        actor = int(actor, 0)
        source, slot, generation = int(source), int(slot), int(generation)
        words = [int(value) for value in fields[8:]]
        if (
            index != len(rows)
            or poll < previous_poll
            or route_cursor < 0
            or route_tick < 0
            or (rows and route_cursor < rows[-1]["route_cursor"])
            or source not in (-1, 0, 1)
        ):
            raise ValueError(f"invalid REA AI trace ordering/source at sample {index}")
        if not 0 <= actor <= 0xFFFFFFFF or any(not 0 <= word <= 0xFFFFFFFF for word in words):
            raise ValueError(f"invalid REA AI trace word at sample {index}")
        if source == 1 and (not 0 <= slot <= 0xFFFF or not 0 < generation <= 0xFFFFFFFF):
            raise ValueError(f"invalid Rust actor identity at sample {index}")
        rows.append(
            dict(
                poll=poll,
                route_cursor=route_cursor,
                route_tick=route_tick,
                actor=actor,
                source=source,
                slot=slot,
                generation=generation,
                words=words,
            )
        )
        previous_poll = poll
    return rows


def compare(
    left: list[dict],
    right: list[dict],
    limit: int,
    minimum: int,
    left_selection: str | None,
    right_selection: str | None,
    left_route_end: int | None,
    right_route_end: int | None,
    recorded_route_end: int | None,
    selected_root_calls: int,
) -> dict:
    route_complete = (
        recorded_route_end is not None
        and left_route_end is not None
        and right_route_end is not None
        and left_route_end >= recorded_route_end
        and right_route_end >= recorded_route_end
    )
    left_route_offset = (
        left_route_end - recorded_route_end
        if route_complete and left_route_end is not None and recorded_route_end is not None
        else 0
    )
    right_route_offset = (
        right_route_end - recorded_route_end
        if route_complete and right_route_end is not None and recorded_route_end is not None
        else 0
    )
    route_limit = limit - left_route_offset
    owner_cursors = [
        row.get("route_cursor", row["poll"] - right_route_offset)
        for row in right
        if row["source"] == 1
        and row.get("route_cursor", row["poll"] - right_route_offset) <= route_limit
    ]
    first_owned_cursor = min(owner_cursors) if owner_cursors else None

    def keyed(rows: list[dict], route_offset: int) -> dict:
        # Prefer the route's recorded T-row cursor, which stays stable when the
        # two replay hosts have different controller-poll rates. Older rows
        # fall back to route-relative polls and require a complete receipt.
        counts: dict[int, int] = {}
        result = {}
        for row in rows:
            cursor = row.get("route_cursor", row["poll"] - route_offset)
            if (
                cursor < 0
                or cursor > route_limit
                or first_owned_cursor is None
                or cursor < first_owned_cursor
            ):
                continue
            ordinal = counts.get(cursor, 0)
            counts[cursor] = ordinal + 1
            result[(cursor, ordinal)] = row
        return result

    a, b = keyed(left, left_route_offset), keyed(right, right_route_offset)
    common = a.keys() & b.keys()
    missing = a.keys() ^ b.keys()
    differences = [key for key in sorted(common) if a[key]["words"] != b[key]["words"]]
    compared_left = [
        row
        for row in left
        if first_owned_cursor is not None
        and first_owned_cursor
        <= row.get("route_cursor", row["poll"] - left_route_offset)
        <= route_limit
    ]
    compared_right = [
        row
        for row in right
        if first_owned_cursor is not None
        and first_owned_cursor
        <= row.get("route_cursor", row["poll"] - right_route_offset)
        <= route_limit
    ]
    reference_ok = (
        left_selection == "reference"
        and bool(compared_left)
        and all(row["source"] == 0 for row in compared_left)
    )
    ownership_ok = (
        right_selection == "owned"
        and bool(compared_right)
        and all(row["source"] == 1 for row in compared_right)
    )
    root_reached = selected_root_calls > 0
    has_route_cursor = bool(left and right) and all(
        "route_cursor" in row for row in (*left, *right)
    )
    route_cursor_aligned = bool(common) and has_route_cursor
    # Recorded cursors align samples for diagnosis when a capture stops early,
    # but a partial owner route is not adoption evidence under Rule 24. Keep
    # cursor alignment visible in the report while requiring both route-end
    # receipts to reach the recording's endpoint for an EQUAL verdict.
    coverage_aligned = route_complete
    enough = len(common) >= minimum
    return {
        "equal": bool(
            enough
            and not missing
            and not differences
            and reference_ok
            and ownership_ok
            and coverage_aligned
            and root_reached
        ),
        "samples_compared": len(common),
        "left_samples": len(a),
        "right_samples": len(b),
        "missing_samples": len(missing),
        "diverging_samples": len(differences),
        "ignored_leading_owned_rows": sum(
            row["source"] != 1
            and first_owned_cursor is not None
            and row.get("route_cursor", row["poll"] - right_route_offset) < first_owned_cursor
            for row in right
        ),
        "left_selection": left_selection,
        "right_selection": right_selection,
        "reference_rows_ok": reference_ok,
        "owned_rows_ok": ownership_ok,
        "route_complete": route_complete,
        "route_cursor_aligned": route_cursor_aligned,
        "coverage_aligned": coverage_aligned,
        "left_route_offset": left_route_offset,
        "right_route_offset": right_route_offset,
        "route_poll_limit": route_limit,
        "selected_root_calls": selected_root_calls,
        "selected_root_reached": root_reached,
        "first_divergence": list(sorted(differences)[0]) if differences else None,
        "reason": None
        if enough
        and not missing
        and not differences
        and reference_ok
        and ownership_ok
        and coverage_aligned
        and root_reached
        else (
            "insufficient rows, mismatched rows, incorrect ownership source, "
            "incomplete recorded route, "
            "or no selected AI tick-boundary calls"
        ),
        "evidence": "Tier 2 typed REA actor state; exact words, no noise masking",
    }
