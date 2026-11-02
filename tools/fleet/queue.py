"""Work-queue selection for the light workers (replaces sup-feed.sh, sup-feed-inv.sh, sup-retry.sh).

The queue (`p3queue.txt`) holds open REA unknown ids. These pure functions choose which ids to
append from the generated index (`docs/data/rea-continuous/index.json` on origin/main), the
branches already pushed, the ids already launched and the worker dirs seen on the hosts.
"""

from __future__ import annotations

import hashlib
import json
from collections.abc import Iterable, Mapping
from typing import Any

RANK = {"open": 0, "investigating": 1, "answered-static": 2, "resolved": 3}


def unknown_name(unknown_id: str) -> str:
    """Worker/branch name derived from an unknown id (same hash as sup-refill.sh mkb())."""
    short = unknown_id.removeprefix("unk_").replace("_", "-")[:20]
    return f"cx-{short}-{hashlib.md5((unknown_id + chr(10)).encode()).hexdigest()[:4]}"  # noqa: S324


def index_rows(index: Mapping[str, Any]) -> list[dict[str, Any]]:
    """Rows of the T3486 summary index (columns/rows) or a legacy full index (unknowns)."""
    if "unknowns" in index:
        return list(index["unknowns"])
    return [dict(zip(index["columns"], row, strict=False)) for row in index["rows"]]


def parse_index(text: str) -> list[dict[str, Any]]:
    return index_rows(json.loads(text))


def best_ranks(rows: Iterable[Mapping[str, Any]]) -> dict[str, int]:
    """Furthest status reached per id (an id has one row per revision)."""
    best: dict[str, int] = {}
    for row in rows:
        unknown_id = row.get("id")
        if unknown_id:
            best[unknown_id] = max(best.get(unknown_id, 0), RANK.get(row.get("status"), 0))
    return best


def select_feed(
    rows: list[dict[str, Any]],
    branches: set[str],
    launched: set[str],
    queued: set[str],
    batch: int = 60,
) -> tuple[list[str], list[str], int]:
    """Open ids never launched and without a worker branch.

    Returns (ids to queue, ids to record as launched because a branch already exists, remainder).
    A launched id still open on main with no branch lost its result and is queued again.
    """
    best = best_ranks(rows)
    skip = set(queued) | {
        unknown_id for unknown_id in launched if unknown_name(unknown_id) in branches
    }
    out: list[str] = []
    seen_launched: list[str] = []
    for row in rows:
        unknown_id = row.get("id")
        if (
            row.get("status") != "open"
            or best.get(unknown_id, 0) > 0
            or not unknown_id
            or unknown_id in skip
        ):
            continue
        if unknown_id in out:
            continue
        if unknown_name(unknown_id) in branches:
            seen_launched.append(unknown_id)
            continue
        out.append(unknown_id)
    return out[:batch], seen_launched, max(0, len(out) - batch)


def select_inv(
    rows: list[dict[str, Any]],
    seen: Mapping[str, float],
    bad: set[str],
    queued: set[str],
    now: float,
    repeat_hours: float = 24,
    batch: int = 30,
) -> list[str]:
    """Ids whose static answer awaits runtime evidence, each at most once per `repeat_hours`."""
    best = best_ranks(rows)
    return [
        unknown_id
        for unknown_id, rank in best.items()
        if rank == 1
        and unknown_id not in bad
        and unknown_id not in queued
        and now - seen.get(unknown_id, 0) > repeat_hours * 3600
    ][:batch]


def select_retry(
    rows: list[dict[str, Any]],
    launched: list[str],
    bad: set[str],
    queued: set[str],
    busy: set[str],
    batch: int = 40,
) -> list[str]:
    """Launched ids still open with no pushed branch and no recent worker dir (dead at start)."""
    best = best_ranks(rows)
    out = [
        unknown_id
        for unknown_id in dict.fromkeys(launched)
        if best.get(unknown_id, 1) == 0
        and unknown_id not in bad
        and unknown_id not in queued
        and unknown_name(unknown_id) not in busy
    ]
    return out[:batch]


def parse_branch_names(ls_remote_output: str) -> set[str]:
    """`<sha>\\trefs/heads/task/cx-foo-remote` lines to {'cx-foo'}."""
    names = set()
    for line in ls_remote_output.splitlines():
        ref = line.split("refs/heads/task/", 1)[-1].strip()
        if ref and "\t" not in ref and " " not in ref:
            names.add(ref.removesuffix("-remote"))
    return names


def normalize_worker_dirs(paths: Iterable[str]) -> set[str]:
    """Worker dir names (CX-x, wt-cx-x, CX-x.123456) to branch names (cx-x)."""
    names = set()
    for path in paths:
        name = path.strip().rsplit("/", 1)[-1]
        if not name:
            continue
        if name.startswith("CX-"):
            name = "cx-" + name[3:]
        name = name.removeprefix("wt-")
        name = name.removesuffix("-runner.out")
        if "." in name:
            name = name.rsplit(".", 1)[0]
        names.add(name)
    return names
