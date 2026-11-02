# SPDX-License-Identifier: GPL-3.0-or-later
"""Live compaction of a segmented capture spool (T3745, docs/t3745-disk-safety.md).

The capture runtime closes a gzip segment every TSFP_T2_CAPTURE_SEGMENT_BYTES and renames it from
`.gz.part` to `.gz`. compact() ingests every finished segment into the deduplicated corpus
(tools.t2corpus.store, itself capped per function and shape) and deletes the raw segment, so the raw
spool on disk stays at about one open segment plus one being ingested, however long the session.
The final pass (after the host exited) also takes a `.gz.part` a kill left behind.

A cumulative ledger `<spool>.compact.json` records what was ingested and deleted.
"""

from __future__ import annotations

import json
import os
import shutil
import threading
from dataclasses import asdict, dataclass
from pathlib import Path

from tools.t2corpus.spool import SpoolError, read_spool, segment_paths
from tools.t2corpus.store import Corpus

LEDGER_SUFFIX = ".compact.json"
DEFAULT_INTERVAL_S = 20.0


@dataclass
class CompactLedger:
    segments: int = 0
    records: int = 0
    kept: int = 0
    duplicates: int = 0
    truncated_segments: int = 0
    unreadable_segments: int = 0
    raw_bytes_deleted: int = 0
    peak_raw_bytes: int = 0


def ledger_path(spool: Path) -> Path:
    return Path(spool).with_name(Path(spool).name + LEDGER_SUFFIX)


def load_ledger(spool: Path) -> CompactLedger:
    try:
        raw = json.loads(ledger_path(spool).read_text())
    except (OSError, ValueError):
        return CompactLedger()
    known = CompactLedger.__dataclass_fields__
    return CompactLedger(**{key: int(value) for key, value in raw.items() if key in known})


def raw_bytes(spool: Path) -> int:
    """Bytes of raw segments on disk now (finished and open)."""
    total = 0
    for path in segment_paths(spool, include_open=True):
        try:
            total += path.stat().st_size
        except OSError:
            pass  # renamed or deleted between listing and stat
    return total


KEEP_ENV = "TSFP_T2_KEEP_SEGMENTS"


def compact(spool: Path, corpus_root: Path, *, final: bool = False) -> CompactLedger:
    """Ingest finished segments (plus a leftover `.gz.part` when final) and delete them."""
    spool = Path(spool)
    ledger = load_ledger(spool)
    ledger.peak_raw_bytes = max(ledger.peak_raw_bytes, raw_bytes(spool))
    segments = segment_paths(spool, include_open=final)
    if not segments:
        _write_ledger(spool, ledger)
        return ledger
    corpus = Corpus(corpus_root)
    try:
        for segment in segments:
            size = segment.stat().st_size
            try:
                read = read_spool(segment)
            except (SpoolError, OSError):
                if not final:
                    continue  # retried next pass; only a final pass gives a segment up
                ledger.unreadable_segments += 1
                continue  # kept on disk for diagnosis; the run's byte cap still bounds it
            report = corpus.ingest(read)
            ledger.segments += 1
            ledger.records += report.records
            ledger.kept += report.kept
            ledger.duplicates += report.duplicates
            ledger.truncated_segments += report.truncated
            keep_dir = os.environ.get(KEEP_ENV)
            if keep_dir:  # remote recapture: the master corpus ingests the kept originals
                Path(keep_dir).mkdir(parents=True, exist_ok=True)
                shutil.move(str(segment), str(Path(keep_dir) / segment.name))
            else:
                segment.unlink()
            ledger.raw_bytes_deleted += size
    finally:
        corpus.close()
    _write_ledger(spool, ledger)
    return ledger


def _write_ledger(spool: Path, ledger: CompactLedger) -> None:
    path = ledger_path(spool)
    temp = path.with_name(path.name + ".tmp")
    temp.write_text(json.dumps(asdict(ledger), indent=1, sort_keys=True) + "\n")
    temp.replace(path)


class Compactor:
    """Background compaction while the capture host runs; stop() runs the final pass."""

    def __init__(
        self, spool: Path, corpus_root: Path, interval_s: float = DEFAULT_INTERVAL_S
    ) -> None:
        self.spool, self.corpus_root, self.interval_s = Path(spool), Path(corpus_root), interval_s
        self._stop = threading.Event()
        self._thread = threading.Thread(target=self._loop, name="t2-compactor", daemon=True)
        self.error: str | None = None

    def start(self) -> Compactor:
        self._thread.start()
        return self

    def _loop(self) -> None:
        while not self._stop.wait(self.interval_s):
            try:
                compact(self.spool, self.corpus_root)
            except Exception as exc:  # noqa: BLE001 - reported in the final ledger, never kills the run
                self.error = f"{type(exc).__name__}: {exc}"

    def stop(self) -> CompactLedger:
        self._stop.set()
        self._thread.join()
        return compact(self.spool, self.corpus_root, final=True)
