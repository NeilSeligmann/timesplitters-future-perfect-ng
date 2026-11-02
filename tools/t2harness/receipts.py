# SPDX-License-Identifier: GPL-3.0-or-later
"""Constructed-call receipts (T3796; format agreed with claude-rea-supervisor, 2026-10-26).

A receipt is the unchanged `tools.t2capture.runner` receipt (`receipts/t2capture/<va>.json`) plus
two top-level keys:

    "tier2_source": "constructed-call"
    "inputs": {"kind": "fabricated", "generator": "<module>", "seed": <int>}

`tools.t2capture_promote` refuses a constructed-call receipt that lacks any of them, and the ledger
counts the credit apart from recording evidence (`tier2_constructed_call_*` columns); recording
credit always wins. Harness facts that are not part of the receipt format (refusals, shapes, rounds)
go to `summary_path(out)` (`docs/data/t3793-harness-summary.json`).
"""

from __future__ import annotations

import json
from pathlib import Path

from tools.t2capture_promote import Currents
from tools.t2harness import generate as gen

TIER2_SOURCE = "constructed-call"
OUT_DIR = Path("docs/data/t3793-harness")


def summary_path(out_dir: Path) -> Path:
    """`<out>-summary.json` beside the receipt dir, which holds receipts only (promote reads it)."""
    return out_dir.parent / f"{out_dir.name}-summary.json"


def label(
    receipt: dict[str, object], seed: int, generator: str = gen.GENERATOR
) -> dict[str, object]:
    """The runner's receipt with the constructed-call label.

    Refuses anything but a t2capture receipt and a non-integer seed."""
    if receipt.get("kind") != "t2capture":
        raise ValueError(f"not a t2capture receipt: kind={receipt.get('kind')!r}")
    if isinstance(seed, bool) or not isinstance(seed, int):
        raise ValueError(f"seed must be an integer, got {seed!r}")
    labelled = dict(receipt)
    labelled["tier2_source"] = TIER2_SOURCE
    labelled["inputs"] = {"kind": "fabricated", "generator": generator, "seed": seed}
    return labelled


def problems(receipt: dict[str, object]) -> list[str]:
    """Why a receipt is not a valid constructed-call receipt (empty when it is)."""
    found = []
    if receipt.get("kind") != "t2capture":
        found.append("kind is not t2capture")
    if receipt.get("tier2_source") != TIER2_SOURCE:
        found.append(f"tier2_source is {receipt.get('tier2_source')!r}")
    inputs = receipt.get("inputs")
    if not isinstance(inputs, dict):
        return [*found, "inputs missing"]
    if inputs.get("kind") != "fabricated":
        found.append(f"inputs.kind is {inputs.get('kind')!r}")
    if not isinstance(inputs.get("generator"), str) or not inputs.get("generator"):
        found.append("inputs.generator missing")
    seed = inputs.get("seed")
    if isinstance(seed, bool) or not isinstance(seed, int):
        found.append(f"inputs.seed is not an integer: {seed!r}")
    return found


def stale(receipt: dict[str, object], currents: Currents) -> list[str]:
    """Why a receipt no longer describes the tree (empty when current), by the promoter's own
    `Currents`; an unknown current counts as stale, as promote refuses it too."""
    found = []
    for key, now in (
        ("compare_logic", currents.compare_logic()),
        ("lifted_tree_hash", currents.lifted_tree_hash()),
    ):
        if now is None or receipt.get(key) != now:
            found.append(f"{key} {receipt.get(key)!r} -> {now!r}")
    module = receipt.get("port_module")
    commit = currents.port_source_commit(module) if isinstance(module, str) and module else None
    if commit is None or receipt.get("port_source_commit") != commit:
        found.append(f"port_source_commit {receipt.get('port_source_commit')!r} -> {commit!r}")
    return found


def write(out_dir: Path, receipt: dict[str, object]) -> Path:
    bad = problems(receipt)
    if bad:
        raise ValueError(f"refusing to write {receipt.get('va')}: {'; '.join(bad)}")
    out_dir.mkdir(parents=True, exist_ok=True)
    path = out_dir / f"{receipt['va']}.json"
    path.write_text(json.dumps(receipt, indent=1, sort_keys=True) + "\n")
    return path


def check(out_dir: Path) -> list[str]:
    """Problems of every receipt in `out_dir` (the landing check)."""
    found = []
    for path in sorted(out_dir.glob("0x*.json")):
        for problem in problems(json.loads(path.read_text())):
            found.append(f"{path.name}: {problem}")
    return found


def merge_summary(
    out_dir: Path,
    functions: dict[str, dict[str, object]],
    task: str = "T3796",
    generator: str = gen.GENERATOR,
    generator_version: int = gen.GENERATOR_VERSION,
) -> dict[str, object]:
    """Fold per-function harness facts into the summary file and recount.

    Batches add to the functions of earlier runs. `task` and `generator` name the harness (T3822
    mutation runs pass their own)."""
    path = summary_path(out_dir)
    doc = json.loads(path.read_text()) if path.is_file() else {}
    merged = dict(doc.get("functions", {}))
    merged.update(functions)
    status: dict[str, int] = {}
    refused: dict[str, int] = {}
    totals = {
        "functions": len(merged),
        "inputs": 0,
        "accepted": 0,
        "compared": 0,
        "equal": 0,
        "diverged": 0,
    }
    for entry in merged.values():
        status[str(entry.get("status"))] = status.get(str(entry.get("status")), 0) + 1
        for reason, count in dict(entry.get("refused", {})).items():
            refused[reason] = refused.get(reason, 0) + int(count)
        for key in ("inputs", "accepted", "compared", "equal", "diverged"):
            totals[key] += int(entry.get(key, 0) or 0)
    summary = {
        "schema": 1,
        "task": task,
        "tier2_source": TIER2_SOURCE,
        "inputs": {
            "kind": "fabricated",
            "generator": generator,
            "generator_version": generator_version,
        },
        "totals": totals,
        "status_counts": dict(sorted(status.items())),
        "refused_inputs": dict(sorted(refused.items())),
        "functions": dict(sorted(merged.items())),
    }
    out_dir.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(summary, indent=1, sort_keys=True) + "\n")
    return summary
