# SPDX-License-Identifier: GPL-3.0-or-later
"""Default-on gate of the live hit census build switch (T3824).

tools/data/live_census.json holds the threshold and the measured overhead (census on vs off, same
host, same sha, a fast-forward owner replay). build.sh asks `default_on`: ON only when a
measurement exists and is at or under the threshold. No measurement, or a missing file, is OFF
(fail closed); an explicit TSFP_BUILD_LIVE_CENSUS still wins in build.sh.
"""

from __future__ import annotations

import json
from collections.abc import Mapping
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CONFIG = Path("tools/data/live_census.json")


def load(path: Path | None = None) -> Mapping[str, object]:
    target = path if path is not None else ROOT / CONFIG
    try:
        return json.loads(Path(target).read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return {}


def overhead_pct(config: Mapping[str, object]) -> float | None:
    measured = config.get("measured")
    if not isinstance(measured, Mapping):
        return None
    value = measured.get("overhead_pct")
    return float(value) if isinstance(value, (int, float)) else None


def default_on(config: Mapping[str, object]) -> bool:
    threshold = config.get("threshold_pct")
    measured = overhead_pct(config)
    if not isinstance(threshold, (int, float)) or measured is None:
        return False
    return measured <= float(threshold)


def explain(config: Mapping[str, object]) -> str:
    measured = overhead_pct(config)
    threshold = config.get("threshold_pct")
    state = "ON" if default_on(config) else "OFF"
    shown = "unmeasured" if measured is None else f"{measured:.2f}%"
    return f"live census default {state}: measured overhead {shown}, threshold {threshold}%"
