# SPDX-License-Identifier: GPL-3.0-or-later
"""Append-only corpus store: sqlite index plus per-function shards (T3741, requirements a-d)."""

from __future__ import annotations

import fcntl
import hashlib
import re
import sqlite3
from dataclasses import dataclass, field
from datetime import UTC, datetime
from pathlib import Path

from tools.t2corpus.keys import input_digest, shape_key
from tools.t2corpus.spool import (
    FLAG_BITS,
    Record,
    SizeCaps,
    SpoolRead,
    iter_records,
    size_caps,
    write_spool,
)

SHAPE_CAP = 8
FUNCTION_CAP = 256
HARD_CEILING = 512
# T3767: capture runtime version of an entry (`t2_capture.c/<n>` in the spool header). A recording
# captured again by a newer runtime supersedes its older entries (kept on disk, never exported):
# version 3 attributes host-callee writes and flags host_write, which version 2 records missed.
CAPTURE_VERSION_DEFAULT = 2
# The version src/host/t2_capture.c writes (tests/test_t2corpus.py keeps them equal).
CAPTURE_VERSION_CURRENT = 4  # T3794: src/game replacements run their lifted originals
_TOOL_VERSION = re.compile(r"t2_capture\.c/(\d+)")

SCHEMA = """
CREATE TABLE IF NOT EXISTS entries (
  id INTEGER PRIMARY KEY,
  va INTEGER NOT NULL, digest TEXT NOT NULL, shape TEXT NOT NULL,
  recording_sha256 TEXT NOT NULL, lifted_tree_hash TEXT NOT NULL,
  poll INTEGER NOT NULL, guest_thread INTEGER NOT NULL, caller_va INTEGER NOT NULL,
  return_address INTEGER NOT NULL, flags INTEGER NOT NULL,
  shard_offset INTEGER NOT NULL, shard_length INTEGER NOT NULL, ingested_at TEXT NOT NULL,
  UNIQUE(va, digest, lifted_tree_hash)
);
CREATE INDEX IF NOT EXISTS entries_va_tree ON entries(va, lifted_tree_hash, shape);
CREATE TABLE IF NOT EXISTS recordings (
  id INTEGER PRIMARY KEY,
  recording_sha256 TEXT NOT NULL, lifted_tree_hash TEXT NOT NULL, build_input_hash TEXT NOT NULL,
  ingested_at TEXT NOT NULL, record_count INTEGER NOT NULL, kept_count INTEGER NOT NULL,
  UNIQUE(recording_sha256, lifted_tree_hash)
);
"""


@dataclass
class IngestReport:
    recording_sha256: str = ""
    skipped_recording: bool = False
    records: int = 0
    kept: int = 0
    duplicates: int = 0
    capped_shape: int = 0
    capped_function: int = 0
    new_shape_over_function_cap: int = 0
    over_hard_ceiling: int = 0
    malformed: int = 0
    truncated: int = 0
    superseded: int = 0  # T3767: older-version entries this ingest replaced
    superseded_size: int = 0  # T3836: size-flagged entries of smaller caps this ingest replaced
    by_function: dict[int, int] = field(default_factory=dict)

    def merge(self, other: IngestReport) -> None:
        for name in (
            "records kept duplicates capped_shape capped_function new_shape_over_function_cap "
            "over_hard_ceiling malformed truncated superseded superseded_size"
        ).split():
            setattr(self, name, getattr(self, name) + getattr(other, name))


def capture_version(header: dict[str, object]) -> int:
    """The capture runtime version of a spool: `capture_version` (bundles), else the
    `t2_capture.c/<n>` tool version, else CAPTURE_VERSION_DEFAULT (older and synthetic spools)."""
    if "capture_version" in header:
        return int(str(header["capture_version"]))
    found = _TOOL_VERSION.search(str(header.get("tool_version", "")))
    return int(found.group(1)) if found else CAPTURE_VERSION_DEFAULT


class Corpus:
    def __init__(self, root: Path, *, readonly: bool = False) -> None:
        self.root = Path(root)
        self.readonly = readonly
        if readonly:
            self.db = sqlite3.connect(
                (self.root / "index.sqlite").resolve().as_uri() + "?mode=ro", uri=True, timeout=5
            )
            return
        self.root.mkdir(parents=True, exist_ok=True)
        self.db = sqlite3.connect(self.root / "index.sqlite", timeout=60)
        self.db.execute("PRAGMA journal_mode=WAL")
        self.db.executescript(SCHEMA)
        columns = {row[1] for row in self.db.execute("PRAGMA table_info(entries)")}
        if "capture_version" not in columns:  # T3767: stores made before capture versions
            self.db.execute(
                "ALTER TABLE entries ADD COLUMN capture_version INTEGER NOT NULL"
                f" DEFAULT {CAPTURE_VERSION_DEFAULT}"
            )
        if "superseded" not in columns:
            self.db.execute("ALTER TABLE entries ADD COLUMN superseded INTEGER NOT NULL DEFAULT 0")
        default = SizeCaps()
        for name, value in (
            ("max_set_bytes", default.max_set_bytes),
            ("max_callees", default.max_callees),
        ):
            if name not in columns:  # T3836: the size caps an entry was captured with
                self.db.execute(
                    f"ALTER TABLE entries ADD COLUMN {name} INTEGER NOT NULL DEFAULT {value}"
                )
        self.db.commit()

    def close(self) -> None:
        self.db.close()

    def newest_tree(self) -> str | None:
        row = self.db.execute(
            "SELECT lifted_tree_hash FROM recordings ORDER BY id DESC LIMIT 1"
        ).fetchone()
        return row[0] if row else None

    def _shard(self, va: int) -> Path:
        folder = self.root / f"{va:08x}"
        if not self.readonly:
            folder.mkdir(exist_ok=True)
        return folder / "entries.bin"

    def ingest(self, read: SpoolRead) -> IngestReport:
        # Bulk run 2026-10-25: two captures ingest live into one store; the shard appends and the
        # sqlite write transaction must not interleave (a second writer waited out the timeout).
        with (self.root / ".ingest.lock").open("a") as lock:
            fcntl.flock(lock, fcntl.LOCK_EX)
            return self._ingest_locked(read)

    def _ingest_locked(self, read: SpoolRead) -> IngestReport:
        head = read.header
        sha, tree = str(head["recording_sha256"]), str(head["lifted_tree_hash"])
        report = IngestReport(recording_sha256=sha, truncated=read.truncated)
        # T3745: each segment of a segmented capture is its own ingest unit; entries keep the sha
        key = sha if "segment" not in head else f"{sha}#seg{int(str(head['segment'])):06d}"
        if "bundle" in head:  # T3740 remote capture: one bundle per remote run, its own unit
            key = f"{sha}#{head['bundle']}"
        version = capture_version(head)
        if version != CAPTURE_VERSION_DEFAULT:  # T3767: a recapture reuses the segment names
            key += f"@v{version}"
        caps = size_caps(head)
        if not caps.is_default:  # T3836: a larger-cap recapture is its own ingest unit
            key += f"@caps{caps.max_set_bytes}x{caps.max_callees}"
        if self.db.execute(
            "SELECT 1 FROM recordings WHERE recording_sha256=? AND lifted_tree_hash=?", (key, tree)
        ).fetchone():
            report.skipped_recording = True
            return report
        now = datetime.now(UTC).isoformat()
        report.superseded = self.db.execute(
            "UPDATE entries SET superseded=1 WHERE recording_sha256=? AND lifted_tree_hash=?"
            " AND capture_version<? AND superseded=0",
            (sha, tree, version),
        ).rowcount
        decoded = list(iter_records(read))
        if not caps.is_default:
            vas = {rec.va for _, rec in decoded if rec is not None}
            report.superseded_size = self._supersede_size(sha, tree, vas, caps)
            report.superseded += report.superseded_size
        for raw, rec in decoded:
            report.records += 1
            if rec is None:
                report.malformed += 1
            elif self._offer(raw, rec, sha, tree, now, report, version, caps):
                report.kept += 1
                report.by_function[rec.va] = report.by_function.get(rec.va, 0) + 1
        self.db.execute(
            "INSERT INTO recordings(recording_sha256, lifted_tree_hash, build_input_hash,"
            " ingested_at, record_count, kept_count) VALUES (?,?,?,?,?,?)",
            (key, tree, str(head["build_input_hash"]), now, report.records, report.kept),
        )
        self.db.commit()
        return report

    def _supersede_size(self, sha: str, tree: str, vas: set[int], caps: SizeCaps) -> int:
        """T3836: a recapture of `sha` with larger caps replaces the size-flagged entries the
        smaller caps left for the VAs it recorded (those calls were cut at the cap, never judged).
        Entries of other recordings, and unflagged entries, stay."""
        changed = 0
        for va in sorted(vas):
            changed += self.db.execute(
                "UPDATE entries SET superseded=1 WHERE recording_sha256=? AND lifted_tree_hash=?"
                " AND va=? AND superseded=0 AND (flags & ?) != 0 AND max_set_bytes<=?"
                " AND max_callees<=? AND (max_set_bytes<? OR max_callees<?)",
                (
                    sha, tree, va, FLAG_BITS["size"], caps.max_set_bytes, caps.max_callees,
                    caps.max_set_bytes, caps.max_callees,
                ),
            ).rowcount  # fmt: skip
        return changed

    def _offer(
        self,
        raw: bytes,
        rec: Record,
        sha: str,
        tree: str,
        now: str,
        report: IngestReport,
        version: int = CAPTURE_VERSION_DEFAULT,
        caps: SizeCaps = SizeCaps(),  # noqa: B008 - frozen dataclass
    ) -> bool:
        digest, shape = input_digest(rec), shape_key(rec)
        same = self.db.execute(
            "SELECT id, superseded, capture_version FROM entries"
            " WHERE va=? AND digest=? AND lifted_tree_hash=?",
            (rec.va, digest, tree),
        ).fetchone()
        # A live entry with these inputs stays unless this capture is newer (T3767).
        if same is not None and not same[1] and same[2] >= version:
            report.duplicates += 1
            return False
        in_shape = self.db.execute(
            "SELECT COUNT(*) FROM entries WHERE va=? AND lifted_tree_hash=? AND shape=?"
            " AND superseded=0",
            (rec.va, tree, shape),
        ).fetchone()[0]
        in_function = self.db.execute(
            "SELECT COUNT(*) FROM entries WHERE va=? AND lifted_tree_hash=? AND superseded=0",
            (rec.va, tree),
        ).fetchone()[0]
        if same is not None and not same[1]:  # an older live entry of the same input: replace it
            in_shape -= 1
            in_function -= 1
        if in_function >= HARD_CEILING:
            report.over_hard_ceiling += 1
            return False
        if in_shape >= SHAPE_CAP:
            report.capped_shape += 1
            return False
        if in_function >= FUNCTION_CAP:
            if in_shape:
                report.capped_function += 1
                return False
            report.new_shape_over_function_cap += 1
        shard = self._shard(rec.va)
        with shard.open("ab") as out:
            offset = out.tell()
            out.write(raw)
        if same is not None:  # UNIQUE(va, digest, tree): the row is reused for the new record
            if not same[1]:
                report.superseded += 1
            self.db.execute(
                "UPDATE entries SET shape=?, recording_sha256=?, poll=?, guest_thread=?,"
                " caller_va=?, return_address=?, flags=?, shard_offset=?, shard_length=?,"
                " ingested_at=?, capture_version=?, superseded=0, max_set_bytes=?, max_callees=?"
                " WHERE id=?",
                (
                    shape, sha, rec.poll, rec.guest_thread, rec.caller_va, rec.return_address,
                    rec.flags, offset, len(raw), now, version, caps.max_set_bytes,
                    caps.max_callees, same[0],
                ),
            )  # fmt: skip
            return True
        self.db.execute(
            "INSERT INTO entries(va, digest, shape, recording_sha256, lifted_tree_hash, poll,"
            " guest_thread, caller_va, return_address, flags, shard_offset, shard_length,"
            " ingested_at, capture_version, max_set_bytes, max_callees)"
            " VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)",
            (
                rec.va, digest, shape, sha, tree, rec.poll, rec.guest_thread, rec.caller_va,
                rec.return_address, rec.flags, offset, len(raw), now, version,
                caps.max_set_bytes, caps.max_callees,
            ),
        )  # fmt: skip
        return True

    def entry_rows(self, tree: str, va: int | None = None) -> list[sqlite3.Row]:
        self.db.row_factory = sqlite3.Row
        sql = "SELECT * FROM entries WHERE lifted_tree_hash=? AND superseded=0"
        args: list[object] = [tree]
        if va is not None:
            sql += " AND va=?"
            args.append(va)
        return self.db.execute(sql + " ORDER BY va, id", args).fetchall()

    def raw_record(self, row: sqlite3.Row) -> bytes:
        with self._shard(row["va"]).open("rb") as src:
            src.seek(row["shard_offset"])
            return src.read(row["shard_length"])

    def function_stats(self, tree: str) -> dict[int, dict[str, object]]:
        out: dict[int, dict[str, object]] = {}
        shapes: dict[int, set[str]] = {}
        recordings: dict[int, set[str]] = {}
        digests: dict[int, list[str]] = {}
        for row in self.entry_rows(tree):
            va = row["va"]
            stat = out.setdefault(va, {"entries": 0, "flag_counts": {}})
            stat["entries"] += 1  # type: ignore[operator]
            for name, bit in FLAG_BITS.items():
                if row["flags"] & bit:
                    counts = stat["flag_counts"]
                    counts[name] = counts.get(name, 0) + 1  # type: ignore[attr-defined]
            shapes.setdefault(va, set()).add(row["shape"])
            recordings.setdefault(va, set()).add(row["recording_sha256"])
            digests.setdefault(va, []).append(row["digest"])
        for va, stat in out.items():
            stat["shapes"] = len(shapes[va])
            stat["recordings"] = len(recordings[va])
            stat["digest_set_hash"] = hashlib.sha256(
                "\n".join(sorted(digests[va])).encode()
            ).hexdigest()
        return out

    def bundle(self, out_dir: Path, tag: str) -> list[Path]:
        """Every entry as spools, one per (recording, tree), header `bundle` = tag (T3740).

        A remote capture compacts into a run-local corpus; the bundle carries its kept records
        to the shared corpus, where `ingest` treats each bundle as its own ingest unit."""
        out_dir = Path(out_dir)
        out_dir.mkdir(parents=True, exist_ok=True)
        self.db.row_factory = sqlite3.Row
        builds = {
            (row["recording_sha256"].split("#")[0], row["lifted_tree_hash"]): row[
                "build_input_hash"
            ]
            for row in self.db.execute("SELECT * FROM recordings")
        }
        groups: dict[tuple[str, str, int, int, int], list[sqlite3.Row]] = {}
        for row in self.db.execute("SELECT * FROM entries WHERE superseded=0 ORDER BY id"):
            key = (
                row["recording_sha256"], row["lifted_tree_hash"], row["capture_version"],
                row["max_set_bytes"], row["max_callees"],
            )  # fmt: skip
            groups.setdefault(key, []).append(row)
        paths = []
        for index, (key, rows) in enumerate(sorted(groups.items())):
            sha, tree, version, set_bytes, callees = key
            path = out_dir / f"bundle-{index:03d}.spool"
            header = {
                "recording_sha256": sha,
                "build_input_hash": builds.get((sha, tree), ""),
                "lifted_tree_hash": tree,
                "created": datetime.now(UTC).isoformat(),
                "tool_version": "t2corpus-bundle-1",
                "capture_version": version,
                "bundle": tag,
            }
            caps = SizeCaps(set_bytes, callees)
            if not caps.is_default:  # T3836: the shared corpus supersedes by these
                header["size_caps"] = {"max_set_bytes": set_bytes, "max_callees": callees}
            write_spool(path, header, [self.raw_record(row) for row in rows])
            paths.append(path)
        return paths

    def export(self, va: int, tree: str, out: Path, skip_flagged: bool = False) -> int:
        """Write one function's records as a spool; skip_flagged drops flagged ones (T3743)."""
        rows = [row for row in self.entry_rows(tree, va) if not (skip_flagged and row["flags"])]
        write_spool(
            out,
            {
                "recording_sha256": "export",
                "build_input_hash": "",
                "lifted_tree_hash": tree,
                "created": datetime.now(UTC).isoformat(),
                "tool_version": "t2corpus-export-1",
            },
            [self.raw_record(row) for row in rows],
        )
        return len(rows)
