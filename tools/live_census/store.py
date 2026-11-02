# SPDX-License-Identifier: GPL-3.0-or-later
"""Durable store and union of live hit census files (T3824).

play.sh writes tmp/play/<run>/census-hits.bin and, when the host exits, `persist` copies it to
the durable store tmp/live-census/<run>.bin (a top-level tmp entry the ephemeral sweep never
touches, tools.wt.hygiene TMP_PROTECTED; a few tens of KB per session, owner-private run data,
never committed). `collect` reads the store plus any run dir not persisted yet, so a run whose
persist step was skipped still counts once.
"""

from __future__ import annotations

import os
import sys
from collections.abc import Iterable
from dataclasses import dataclass
from pathlib import Path

from tools.datadir import data_dir
from tools.live_census import format as fmt

STORE_NAME = "live-census"
PLAY_NAME = "play"
RUN_FILE = "census-hits.bin"
LIFT_DISPATCH = Path("private-host")  # under tmp: lift-<key>/gen/recomp_dispatch.c


def store_dir() -> Path:
    return data_dir(STORE_NAME)


def play_dir() -> Path:
    return data_dir(PLAY_NAME)


def persist(run_dir: Path, store: Path | None = None) -> Path | None:
    """Copy run_dir/census-hits.bin into the store as <run>.bin; None when the run wrote none.
    The file is parsed first, so a torn or foreign file never enters the store."""
    source = Path(run_dir) / RUN_FILE
    if not source.is_file():
        return None
    data = source.read_bytes()
    fmt.parse(data, source=str(source))
    target_dir = Path(store) if store is not None else store_dir()
    target_dir.mkdir(parents=True, exist_ok=True)
    target = target_dir / f"{Path(run_dir).resolve().name}.bin"
    temp = target.with_name(target.name + f".{os.getpid()}.tmp")
    temp.write_bytes(data)
    os.replace(temp, target)
    return target


@dataclass(frozen=True)
class Collected:
    runs: tuple[fmt.Census, ...]
    refused: tuple[str, ...]


def _lift_vas(key: str, lift_root: Path | None) -> tuple[int, ...] | None:
    root = lift_root if lift_root is not None else data_dir(LIFT_DISPATCH)
    dispatch = root / f"lift-{key}" / "gen" / "recomp_dispatch.c"
    return fmt.dispatch_vas(dispatch) if dispatch.is_file() else None


def candidates(store: Path, play: Path) -> list[tuple[str, Path]]:
    """(run name, file): every stored run, then every run dir file not stored yet."""
    found: dict[str, Path] = {}
    if store.is_dir():
        for path in sorted(store.glob("*.bin")):
            found[path.stem] = path
    if play.is_dir():
        for path in sorted(play.glob(f"*/{RUN_FILE}")):
            found.setdefault(path.parent.name, path)
    return sorted(found.items())


def collect(
    *,
    store: Path | None = None,
    play: Path | None = None,
    lift_root: Path | None = None,
    expected_lift_key: str | None = None,
) -> Collected:
    """Every readable census. A file that fails to parse, names another lift than
    `expected_lift_key`, or whose VA table is not its lift's function list is refused (listed)."""
    runs: list[fmt.Census] = []
    refused: list[str] = []
    lift_cache: dict[str, tuple[int, ...] | None] = {}
    for _name, path in candidates(
        Path(store) if store is not None else store_dir(),
        Path(play) if play is not None else play_dir(),
    ):
        try:
            census = fmt.load(path, expected_lift_key=expected_lift_key)
            if census.lift_key not in lift_cache:
                lift_cache[census.lift_key] = _lift_vas(census.lift_key, lift_root)
            lift_vas = lift_cache[census.lift_key]
            if lift_vas is not None:
                fmt.check_against_lift(census, lift_vas)
        except (OSError, fmt.CensusError) as error:
            refused.append(f"{path}: {error}")
            continue
        runs.append(census)
    return Collected(tuple(runs), tuple(refused))


def union(
    runs: Iterable[fmt.Census], *, build_sha: str | None = None, port_set: str | None = None
) -> frozenset[int]:
    """VAs any selected run hit. VAs are guest addresses, so runs of different lifts union by VA;
    `build_sha` / `port_set` restrict to runs of one build or one Rust port set."""
    hit: set[int] = set()
    for census in runs:
        if build_sha is not None and census.build_sha != build_sha:
            continue
        if port_set is not None and census.port_set != port_set:
            continue
        hit |= census.hit_vas()
    return frozenset(hit)


def union_vas(**selection: str | None) -> frozenset[int]:
    """The live union for consumers (tools.t2reach, tools.t2corpus): empty with 0 live runs."""
    collected = collect()
    for line in collected.refused:
        print(f"live census: refused {line}", file=sys.stderr)
    return union(collected.runs, **selection)
