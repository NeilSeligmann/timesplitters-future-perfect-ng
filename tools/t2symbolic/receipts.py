# SPDX-License-Identifier: GPL-3.0-or-later
"""T3875: `tier2_source: symbolic` receipts from `tools.t2symbolic` results.

    python3 -m tools.t2symbolic receipts --results pilot.jsonl --out DIR

Only `proved_equal` results become receipts; every other verdict is skipped (never promoted,
never demoting). The receipt is a schema 1 `t2capture` receipt with a `proof` object and goes
through `python3 -m tools.t2capture_promote --receipts DIR`, which refuses it when the port
source, the lift or the Kani version moved.
"""

from __future__ import annotations

import hashlib
import json
from collections.abc import Callable
from datetime import UTC, datetime
from pathlib import Path

from tools.t2symbolic import verdict


def receipt_of(
    result: dict,
    *,
    module: str,
    port_source_commit: str | None,
    lifted_tree_hash: str,
    tool_version: str | None,
    unwind: int,
) -> dict | None:
    """The receipt for one `runner.check` record, None unless the proof is `proved_equal`."""
    symbolic = result.get("symbolic")
    if not isinstance(symbolic, dict) or symbolic.get("verdict") != verdict.PROVED_EQUAL:
        return None
    log = Path(str(symbolic.get("log", "")))
    return {
        "schema": 1,
        "kind": "t2capture",
        "tier2_source": "symbolic",
        "va": f"0x{int(str(result['va']), 16):08X}",
        "status": "equal",
        "port_module": module,
        "port_source_commit": port_source_commit,
        "lifted_tree_hash": lifted_tree_hash,
        "created": datetime.now(UTC).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "proof": {
            "verdict": verdict.PROVED_EQUAL,
            "unwind": unwind,
            "tool": "kani",
            "tool_version": tool_version,
            "log_sha256": hashlib.sha256(log.read_bytes()).hexdigest() if log.is_file() else "",
            "seconds": symbolic.get("seconds"),
        },
    }


def write_all(
    results: list[dict],
    out: Path,
    *,
    module_of: Callable[[int], str | None],
    commit_of: Callable[[str], str | None],
    lifted_tree_hash: str,
    tool_version: str | None,
    unwind: int,
) -> list[str]:
    """Write one receipt per proved result; returns the VAs written."""
    out.mkdir(parents=True, exist_ok=True)
    written = []
    for result in results:
        module = module_of(int(str(result["va"]), 16))
        receipt = (
            receipt_of(
                result,
                module=module,
                port_source_commit=commit_of(module),
                lifted_tree_hash=lifted_tree_hash,
                tool_version=tool_version,
                unwind=unwind,
            )
            if module
            else None
        )
        if receipt is None:
            continue
        text = json.dumps(receipt, indent=2, sort_keys=True) + "\n"
        (out / f"{receipt['va']}.json").write_text(text)
        written.append(receipt["va"])
    return written
