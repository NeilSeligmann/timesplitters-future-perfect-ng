# SPDX-License-Identifier: GPL-3.0-or-later
"""T2329: compare authoritative HUD lifecycle, not retired guest storage.

Weapon trace rows: sample, port-0 poll, shell frame, slot, six raw state words.
No nondeterminism masking is applied to these owned logical fields.
"""

from __future__ import annotations

from pathlib import Path


def load(path: Path) -> list[dict]:
    samples = []
    previous_poll = 0
    for line in path.read_text().splitlines():
        values = [int(value) for value in line.split()]
        if len(values) != 10:
            raise ValueError(f"invalid HUD state row {len(samples)}: expected ten integers")
        index, poll, frame, slot, *words = values
        if index != len(samples) or poll < previous_poll or not 0 <= slot < 4:
            raise ValueError(f"invalid HUD state ordering/slot at sample {index}")
        if any(not 0 <= word <= 0xFFFFFFFF for word in words):
            raise ValueError(f"invalid HUD state word at sample {index}")
        samples.append({"poll": poll, "frame": frame, "slot": slot, "words": words})
        previous_poll = poll
    return samples


def compare(left: list[dict], right: list[dict], limit: int, minimum: int) -> dict:
    def keyed(samples: list[dict]) -> dict:
        counts: dict[tuple[int, int], int] = {}
        result = {}
        for sample in samples:
            if sample["poll"] > limit:
                continue
            pair = (sample["poll"], sample["slot"])
            ordinal = counts.get(pair, 0)
            counts[pair] = ordinal + 1
            result[(*pair, ordinal)] = sample["words"]
        return result

    a, b = keyed(left), keyed(right)
    common = a.keys() & b.keys()
    differences = [key for key in sorted(common) if a[key] != b[key]]
    missing = a.keys() ^ b.keys()
    active = sum(a[key][0] != 0 for key in common)
    enough = len(common) >= minimum and active > 0
    return {
        "equal": not differences and not missing and enough,
        "samples_compared": len(common),
        "active_phase_samples": active,
        "missing_samples": len(missing),
        "diverging_samples": len(differences),
        "first_divergence": list(differences[0]) if differences else None,
        "reason": None if enough else "too few samples or no active weapon phase",
        "evidence": "Tier 2 logical state; raw words exact, no noise masking",
    }


def load_rows(path: Path) -> list[dict]:
    samples = []
    previous_poll = 0
    for line in path.read_text().splitlines():
        values = [int(value) for value in line.split()]
        if len(values) != 7:
            raise ValueError("HUD row trace requires seven integers")
        index, poll, frame, slot, drawer, first, second = values
        if index != len(samples) or poll < previous_poll or not 0 <= slot < 4:
            raise ValueError("invalid HUD row ordering/slot")
        if any(not 0 <= word <= 0xFFFFFFFF for word in (drawer, first, second)):
            raise ValueError("invalid HUD row word")
        samples.append(
            dict(poll=poll, frame=frame, slot=slot, drawer=drawer, words=[first, second])
        )
        previous_poll = poll
    return samples


def compare_rows(left: list[dict], right: list[dict], limit: int, minimum: int = 1) -> dict:
    def keyed(samples: list[dict]) -> dict:
        counts, result = {}, {}
        for sample in samples:
            if sample["poll"] > limit:
                continue
            pair = (sample["poll"], sample["slot"], sample["drawer"])
            ordinal = counts.get(pair, 0)
            counts[pair] = ordinal + 1
            result[(*pair, ordinal)] = sample["words"]
        return result

    a, b = keyed(left), keyed(right)
    common = a.keys() & b.keys()
    differences = [key for key in sorted(common) if a[key] != b[key]]
    missing = a.keys() ^ b.keys()
    return dict(
        equal=len(common) >= minimum and not differences and not missing,
        samples_compared=len(common),
        missing_samples=len(missing),
        diverging_samples=len(differences),
        drawers=sorted({key[2] for key in common}),
        first_divergence=list(differences[0]) if differences else None,
        evidence="Tier 2 native row inputs and advances; exact, no masking",
    )
