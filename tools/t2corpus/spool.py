# SPDX-License-Identifier: GPL-3.0-or-later
"""Spool format reader and writer (T3741), spec in docs/t3738-capture-design.md.

All integers are little-endian. A spool is MAGIC, u32 header length, UTF-8 JSON header, then
records (u32 length + record bytes) until EOF. A truncated final record is ignored and counted.

T3745: the capture runtime writes gzip segments (`<spool>.seg-NNNNNN.gz`, `.gz.part` while open),
each a complete spool whose header also carries `segment`. read_spool reads either form; a gzip
stream cut short (a killed host) yields the records it holds and counts as truncated.
"""

from __future__ import annotations

import json
import struct
import zlib
from collections.abc import Iterator
from dataclasses import dataclass, field
from pathlib import Path

MAGIC = b"T2CAP\x01"
REGS_SIZE = 240
HEADER_KEYS = (
    "recording_sha256",
    "build_input_hash",
    "lifted_tree_hash",
    "created",
    "tool_version",
)
FLAG_BITS = {
    "mmio": 1,
    "thread_switch": 2,
    "size": 4,
    "seh_unwind": 8,
    "fpu_overflow": 16,
    "self_modifying": 32,
    # T3740: synthetic, set by the reader on a record without the implicit-state tail (format 1).
    # Such a record cannot be replayed faithfully (g_seh_ebp and friends unknown): recapture it.
    "old_format": 64,
    # T3767: a byte the frame knew changed behind the capture with no callee to attribute it to.
    "host_write": 128,
    # Input annotations are not replayed yet; never credit them as Tier 2.
    "input_unverified": 256,
}
OLD_FORMAT = FLAG_BITS["old_format"]
# T3740 record format 2: this bit of the raw flags word says the record ends with the implicit
# thread-state blocks (entry, exit, then one per callee exit). Never a capture_incomplete reason.
FORMAT_IMPLICIT = 0x80000000
FORMAT_CLOCK_EVENTS = 0x40000000
FORMAT_INPUT_ANNOTATIONS = 0x20000000
INPUT_ANNOTATION_VERSION = 1
INPUT_MAX_ENTRIES = 64
RECORD_FORMAT = 2
IMPLICIT_SIZE = 32
# Words of an implicit-state block (src/host/t2_capture.c snapshot_implicit), little-endian u32.
IMPLICIT_NAMES = (
    "seh_ebp", "df", "fp_cmp", "fp_cc",
    "flag_bridge_eflags", "flag_bridge_mask", "fs_base", "reserved",
)  # fmt: skip
# ExtRegisters ABI offsets (crates/tsfp-port-runtime/src/abi.rs): eax ecx edx esp ebx ebp esi edi.
REG_NAMES = ("eax", "ecx", "edx", "esp", "ebx", "ebp", "esi", "edi")
Run = tuple[int, bytes]  # (addr, data)


class SpoolError(ValueError):
    pass


# T3836: per-run size caps of src/host/t2_capture.c (TSFP_T2_CAPTURE_MAX_SET_BYTES / _MAX_CALLEES).
# A header without `size_caps` was written with the defaults. The runtime clamps to the ceilings.
SET_CAP_DEFAULT = 64 * 1024
SET_CAP_CEILING = 4 * 1024 * 1024
CALLEES_DEFAULT = 256
CALLEES_CEILING = 16384
# Hard ceiling of one record, so a reader's memory stays bounded and a corrupt length word cannot
# claim gigabytes. At the cap ceilings a record holds two 4 MiB sets (runs of whole words, at most
# 12 bytes per 4 set bytes: 24 MiB) plus 16384 callees of a few hundred bytes (under 8 MiB).
MAX_RECORD_BYTES = 64 * 1024 * 1024


@dataclass(frozen=True)
class SizeCaps:
    max_set_bytes: int = SET_CAP_DEFAULT
    max_callees: int = CALLEES_DEFAULT
    allow_only: bool = False

    def exceeds(self, other: SizeCaps) -> bool:
        """These caps are larger than `other` in at least one dimension and smaller in none."""
        ge = self.max_set_bytes >= other.max_set_bytes and self.max_callees >= other.max_callees
        return ge and (self.max_set_bytes, self.max_callees) != (
            other.max_set_bytes,
            other.max_callees,
        )

    @property
    def is_default(self) -> bool:
        return (self.max_set_bytes, self.max_callees) == (SET_CAP_DEFAULT, CALLEES_DEFAULT)


def size_caps(header: dict[str, object]) -> SizeCaps:
    """The size caps a spool was captured with (T3836); defaults for an older header."""
    raw = header.get("size_caps")
    if raw is None:
        return SizeCaps()
    if not isinstance(raw, dict):
        raise SpoolError(f"bad size_caps {raw!r}")
    try:
        caps = SizeCaps(
            int(raw.get("max_set_bytes", SET_CAP_DEFAULT)),
            int(raw.get("max_callees", CALLEES_DEFAULT)),
            bool(raw.get("allow_only", False)),
        )
    except (TypeError, ValueError) as exc:
        raise SpoolError(f"bad size_caps {raw!r}") from exc
    if not (0 < caps.max_set_bytes <= SET_CAP_CEILING and 0 < caps.max_callees <= CALLEES_CEILING):
        raise SpoolError(f"size_caps over the ceiling: {raw!r}")
    return caps


def decode_regs(regs: bytes) -> dict[str, int]:
    """The eight integer registers from a 240-byte ExtRegisters blob."""
    if len(regs) != REGS_SIZE:
        raise SpoolError(f"register blob must be {REGS_SIZE} bytes, got {len(regs)}")
    return dict(zip(REG_NAMES, struct.unpack_from("<8I", regs), strict=True))


def decode_implicit(block: bytes) -> dict[str, int]:
    if len(block) != IMPLICIT_SIZE:
        raise SpoolError(f"implicit block must be {IMPLICIT_SIZE} bytes, got {len(block)}")
    return dict(zip(IMPLICIT_NAMES, struct.unpack("<8I", block), strict=True))


def effective_flags(raw: int) -> int:
    """capture_incomplete bits of a raw flags word: the format bit stripped, old_format added."""
    flags = raw & ~(FORMAT_IMPLICIT | FORMAT_CLOCK_EVENTS | FORMAT_INPUT_ANNOTATIONS) & 0xFFFFFFFF
    return flags if raw & FORMAT_IMPLICIT else flags | OLD_FORMAT


def flag_names(flags: int) -> list[str]:
    return [name for name, bit in FLAG_BITS.items() if flags & bit]


@dataclass
class Callee:
    va: int
    entry_regs: bytes
    args: list[int]
    exit_regs: bytes
    write_runs: list[Run] = field(default_factory=list)
    exit_implicit: bytes | None = None


@dataclass
class ClockEvent:
    after_callees: int
    value: int
    writes: list[Run] = field(default_factory=list)


@dataclass
class InputEntry:
    address: int
    handler: int
    default_return: int


@dataclass
class InputAnnotation:
    callee: int
    generation: int
    lifecycle_sequence: int
    selection_sequence: int
    address: int
    route: int
    handler: int
    default_return: int
    complete: int
    entries: list[InputEntry] = field(default_factory=list)


@dataclass
class Record:
    va: int
    caller_va: int
    return_address: int
    poll: int
    guest_thread: int
    flags: int
    entry_regs: bytes
    exit_regs: bytes
    read_runs: list[Run] = field(default_factory=list)
    write_runs: list[Run] = field(default_factory=list)
    callees: list[Callee] = field(default_factory=list)
    # Format 2 implicit thread state (None in a format 1 record, whose flags then carry old_format).
    entry_implicit: bytes | None = None
    exit_implicit: bytes | None = None
    # None is an older record; [] means the capture observed no direct clock reads.
    clock_events: list[ClockEvent] | None = None
    input_annotations: list[InputAnnotation] | None = None


def _runs(runs: list[Run]) -> bytes:
    out = [struct.pack("<I", len(runs))]
    for addr, data in runs:
        out += [struct.pack("<II", addr, len(data)), data]
    return b"".join(out)


def encode_record(rec: Record) -> bytes:
    """Bytes of a record; format 2 when `entry_implicit` is set (flags given as effective_flags)."""
    for blob in (rec.entry_regs, rec.exit_regs):
        if len(blob) != REGS_SIZE:
            raise SpoolError("register blob size")
    v2 = rec.entry_implicit is not None
    if rec.clock_events is not None and not v2:
        raise SpoolError("clock events require implicit state")
    if rec.input_annotations is not None and not v2:
        raise SpoolError("input annotations require implicit state")
    flags = (rec.flags & ~OLD_FORMAT) | FORMAT_IMPLICIT if v2 else rec.flags & ~OLD_FORMAT
    if rec.clock_events is not None:
        flags |= FORMAT_CLOCK_EVENTS
    if rec.input_annotations is not None:
        flags |= FORMAT_INPUT_ANNOTATIONS | FLAG_BITS["input_unverified"]
    out = [
        struct.pack(
            "<6I", rec.va, rec.caller_va, rec.return_address, rec.poll, rec.guest_thread, flags
        ),
        rec.entry_regs,
        rec.exit_regs,
        _runs(rec.read_runs),
        _runs(rec.write_runs),
        struct.pack("<I", len(rec.callees)),
    ]
    for callee in rec.callees:
        out += [
            struct.pack("<I", callee.va),
            callee.entry_regs,
            struct.pack(f"<I{len(callee.args)}I", len(callee.args), *callee.args),
            callee.exit_regs,
            _runs(callee.write_runs),
        ]
    if v2:
        zero = bytes(IMPLICIT_SIZE)
        blocks = [rec.entry_implicit, rec.exit_implicit or zero]
        blocks += [callee.exit_implicit or zero for callee in rec.callees]
        if any(len(block) != IMPLICIT_SIZE for block in blocks):
            raise SpoolError("implicit block size")
        out += blocks
    if rec.clock_events is not None:
        previous = 0
        out.append(struct.pack("<I", len(rec.clock_events)))
        for event in rec.clock_events:
            if not previous <= event.after_callees <= len(rec.callees):
                raise SpoolError("invalid clock event order")
            previous = event.after_callees
            out += [struct.pack("<IQ", event.after_callees, event.value), _runs(event.writes)]
    if rec.input_annotations is not None:
        out.append(struct.pack("<II", INPUT_ANNOTATION_VERSION, len(rec.input_annotations)))
        previous = -1
        for annotation in rec.input_annotations:
            if (
                not previous < annotation.callee < len(rec.callees)
                or len(annotation.entries) > INPUT_MAX_ENTRIES
            ):
                raise SpoolError("invalid input annotation")
            previous = annotation.callee
            out.append(
                struct.pack(
                    "<IQQQ6I",
                    annotation.callee,
                    annotation.generation,
                    annotation.lifecycle_sequence,
                    annotation.selection_sequence,
                    annotation.address,
                    annotation.route,
                    annotation.handler,
                    annotation.default_return,
                    annotation.complete,
                    len(annotation.entries),
                )
            )
            out.extend(
                struct.pack("<3I", entry.address, entry.handler, entry.default_return)
                for entry in annotation.entries
            )
    return b"".join(out)


class _Cursor:
    def __init__(self, data: bytes) -> None:
        self.data = data
        self.pos = 0

    def take(self, size: int) -> bytes:
        if size < 0 or self.pos + size > len(self.data):
            raise SpoolError("record overruns its length")
        chunk = self.data[self.pos : self.pos + size]
        self.pos += size
        return chunk

    def u32s(self, count: int) -> tuple[int, ...]:
        return struct.unpack(f"<{count}I", self.take(4 * count))

    def runs(self) -> list[Run]:
        (count,) = self.u32s(1)
        out: list[Run] = []
        for _ in range(count):
            addr, length = self.u32s(2)
            out.append((addr, self.take(length)))
        return out


def decode_record(data: bytes) -> Record:
    cur = _Cursor(data)
    va, caller, ret, poll, thread, flags = cur.u32s(6)
    entry, exit_ = cur.take(REGS_SIZE), cur.take(REGS_SIZE)
    rec = Record(va, caller, ret, poll, thread, effective_flags(flags), entry, exit_)
    rec.read_runs = cur.runs()
    rec.write_runs = cur.runs()
    (n_callees,) = cur.u32s(1)
    for _ in range(n_callees):
        (cva,) = cur.u32s(1)
        centry = cur.take(REGS_SIZE)
        (n_args,) = cur.u32s(1)
        args = list(cur.u32s(n_args))
        cexit = cur.take(REGS_SIZE)
        rec.callees.append(Callee(cva, centry, args, cexit, cur.runs()))
    if flags & FORMAT_IMPLICIT:
        rec.entry_implicit = cur.take(IMPLICIT_SIZE)
        rec.exit_implicit = cur.take(IMPLICIT_SIZE)
        for callee in rec.callees:
            callee.exit_implicit = cur.take(IMPLICIT_SIZE)
    if flags & FORMAT_CLOCK_EVENTS:
        if not flags & FORMAT_IMPLICIT:
            raise SpoolError("clock events require implicit state")
        (count,) = cur.u32s(1)
        if count > (len(data) - cur.pos) // 16:
            raise SpoolError("clock event count exceeds record")
        rec.clock_events = []
        previous = 0
        for _ in range(count):
            (ordinal,) = cur.u32s(1)
            (value,) = struct.unpack("<Q", cur.take(8))
            if not previous <= ordinal <= len(rec.callees):
                raise SpoolError("invalid clock event order")
            previous = ordinal
            rec.clock_events.append(ClockEvent(ordinal, value, cur.runs()))
    if flags & FORMAT_INPUT_ANNOTATIONS:
        if not flags & FORMAT_IMPLICIT:
            raise SpoolError("input annotations require implicit state")
        version, count = cur.u32s(2)
        if version != INPUT_ANNOTATION_VERSION or count > len(rec.callees):
            raise SpoolError("unsupported input annotation version or count")
        rec.input_annotations = []
        previous = -1
        for _ in range(count):
            values = struct.unpack("<IQQQ6I", cur.take(52))
            (
                ordinal,
                generation,
                lifecycle,
                selection,
                address,
                route,
                handler,
                default,
                complete,
                n,
            ) = values
            if not previous < ordinal < len(rec.callees) or n > INPUT_MAX_ENTRIES:
                raise SpoolError("invalid input annotation")
            previous = ordinal
            entries = [InputEntry(*cur.u32s(3)) for _ in range(n)]
            rec.input_annotations.append(
                InputAnnotation(
                    ordinal,
                    generation,
                    lifecycle,
                    selection,
                    address,
                    route,
                    handler,
                    default,
                    complete,
                    entries,
                )
            )
        # Even malformed producers that omit the flag cannot gain replay credit.
        rec.flags |= FLAG_BITS["input_unverified"]
    if cur.pos != len(data):
        raise SpoolError("trailing bytes in record")
    return rec


def write_spool(path: Path, header: dict[str, object], raw_records: list[bytes]) -> None:
    """Write a spool from already-encoded record bytes (see encode_record)."""
    missing = [key for key in HEADER_KEYS if key not in header]
    if missing:
        raise SpoolError(f"header missing {missing}")
    blob = json.dumps(header, sort_keys=True).encode()
    with Path(path).open("wb") as out:
        out.write(MAGIC + struct.pack("<I", len(blob)) + blob)
        for raw in raw_records:
            if len(raw) > MAX_RECORD_BYTES:
                raise SpoolError(f"record of {len(raw)} bytes over MAX_RECORD_BYTES")
            out.write(struct.pack("<I", len(raw)) + raw)


@dataclass
class SpoolRead:
    header: dict[str, object]
    records: list[bytes]
    truncated: int  # 1 when a partial final record (or length word) was dropped, else 0


GZIP_MAGIC = b"\x1f\x8b"


def _gunzip(blob: bytes) -> tuple[bytes, bool]:
    """Decompress one gzip stream; (data, complete). A cut stream returns what it holds."""
    inflater = zlib.decompressobj(wbits=31)
    try:
        data = inflater.decompress(blob)
    except zlib.error:
        return b"", False
    return data, inflater.eof


def read_spool(path: Path) -> SpoolRead:
    data = Path(path).read_bytes()
    stream_complete = True
    if data[:2] == GZIP_MAGIC:
        data, stream_complete = _gunzip(data)
    if data[: len(MAGIC)] != MAGIC:
        raise SpoolError("bad magic")
    pos = len(MAGIC)
    if pos + 4 > len(data):
        raise SpoolError("truncated header length")
    (hlen,) = struct.unpack_from("<I", data, pos)
    pos += 4
    if pos + hlen > len(data):
        raise SpoolError("truncated header")
    try:
        header = json.loads(data[pos : pos + hlen].decode())
    except ValueError as exc:
        raise SpoolError(f"bad header: {exc}") from exc
    if not isinstance(header, dict) or any(key not in header for key in HEADER_KEYS):
        raise SpoolError("header missing required keys")
    pos += hlen
    records: list[bytes] = []
    truncated = 0
    while pos < len(data):
        if pos + 4 > len(data):
            truncated = 1
            break
        (size,) = struct.unpack_from("<I", data, pos)
        if size > MAX_RECORD_BYTES:  # T3836: a corrupt length word, the rest cannot be framed
            truncated = 1
            break
        if pos + 4 + size > len(data):
            truncated = 1
            break
        records.append(data[pos + 4 : pos + 4 + size])
        pos += 4 + size
    if not stream_complete:
        truncated = 1
    return SpoolRead(header, records, truncated)


def segment_paths(spool: Path, include_open: bool = False) -> list[Path]:
    """Segments of a segmented spool in order; `.gz.part` (open, or left by a kill) on request."""
    spool = Path(spool)
    found = sorted(spool.parent.glob(spool.name + ".seg-*.gz"))
    if include_open:
        found = sorted([*found, *spool.parent.glob(spool.name + ".seg-*.gz.part")])
    return found


def iter_records(read: SpoolRead) -> Iterator[tuple[bytes, Record | None]]:
    """Yield (raw, decoded); decoded is None for a malformed record (counted by the caller)."""
    for raw in read.records:
        try:
            yield raw, decode_record(raw)
        except (SpoolError, struct.error):
            yield raw, None
