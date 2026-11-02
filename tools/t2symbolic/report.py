# SPDX-License-Identifier: GPL-3.0-or-later
"""T3871: summarise pilot JSON lines. Symbolic verdicts are reclassified from the kept Kani log
with the current classifier (`verdict.classify`), so a classifier fix applies to old runs."""

from __future__ import annotations

import json
import statistics
from collections import Counter
from pathlib import Path

from tools.t2symbolic import verdict


def load(files: list[Path]) -> list[dict[str, object]]:
    records = []
    for path in files:
        for line in path.read_text().splitlines():
            if not line.strip():
                continue
            record = json.loads(line)
            symbolic = record["symbolic"]
            log = Path(str(symbolic.get("log", "")))
            if log.is_file() and symbolic["verdict"] != verdict.TIMEOUT:
                fresh = verdict.classify(log.read_text(errors="replace"), False)
                keep = {
                    k: symbolic[k]
                    for k in ("seconds", "peak_rss_mb", "log", "input")
                    if k in symbolic
                }
                record["symbolic"] = {**fresh, **keep}
            records.append(record)
    return sorted(records, key=lambda r: int(r.get("pilot_index", 0)))


def render(records: list[dict[str, object]]) -> str:
    lines = [
        "| # | VA | crate | symbolic | s | peak MB | detail | fuzz |",
        "|---|---|---|---|---|---|---|---|",
    ]
    for r in records:
        s: dict = r["symbolic"]  # type: ignore[assignment]
        f: dict = r.get("fuzz") or {}  # type: ignore[assignment]
        detail = s.get("differs") or s.get("reason") or (s.get("failed") or [""])[0]
        fuzz = f"{f['verdict']} ({f.get('executions', 0)} execs)" if f else ""
        lines.append(
            f"| {r.get('pilot_index')} | {r['va']} | {r['crate']} | {s['verdict']} | "
            f"{s.get('seconds')} | {s.get('peak_rss_mb')} | {str(detail)[:60]} | {fuzz} |"
        )
    counts = Counter(str(r["symbolic"]["verdict"]) for r in records)  # type: ignore[index]
    seconds = [float(r["symbolic"]["seconds"]) for r in records]  # type: ignore[index]
    rss = [int(r["symbolic"]["peak_rss_mb"]) for r in records if r["symbolic"].get("peak_rss_mb")]  # type: ignore[index,union-attr]
    lines.append("")
    lines.append("verdicts: " + ", ".join(f"{k} {v}" for k, v in sorted(counts.items())))
    if seconds:
        lines.append(
            f"seconds: median {statistics.median(seconds):.1f}, max {max(seconds):.1f}, "
            f"total {sum(seconds):.0f}"
        )
    if rss:
        lines.append(f"peak RSS MB: median {statistics.median(rss):.0f}, max {max(rss)}")
    return "\n".join(lines)
