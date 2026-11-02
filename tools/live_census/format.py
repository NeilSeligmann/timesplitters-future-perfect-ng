# SPDX-License-Identifier: GPL-3.0-or-later
"""The census-hits.bin format of src/host/live_census.c (T3824): read, and written for tests.

Little endian. A LIVE_CENSUS_HEADER_SIZE header, then u32 va[function_count] ascending (the lifted
functions of the lift named by `lift_key`, so bit index i maps to va[i]), then the bitmap: bit i of
byte i >> 3, LSB first, set when va[i] ran.
"""

from __future__ import annotations

import re
import struct
from dataclasses import dataclass, field
from pathlib import Path

MAGIC = b"TSFPHIT1"
VERSION = 1
HEADER_SIZE = 192
REASONS = {0: "exit", 1: "signal"}
# Offsets match live_census_write. One struct, so a moved field breaks the round trip test.
_HEADER = struct.Struct("<8sIIIIIIQ32s40sIIIIQ48sII")
assert _HEADER.size == HEADER_SIZE
_DISPATCH_ROW = re.compile(rb"\{\s*0x([0-9A-Fa-f]{8})u,\s*\(recomp_func_t\)")


class CensusError(ValueError):
    """A file that is not a readable census (bad magic, version, size or VA table)."""


class LiftKeyMismatch(CensusError):
    """The file was written by a host of another lift than the one asked for."""


@dataclass(frozen=True)
class Census:
    lift_key: str
    build_sha: str
    port_spec: str
    port_roots: int
    port_digest: int
    rust_root_hits: int
    reason: str
    written_unix: int
    image_base: int
    image_size: int
    bucket_shift: int
    ambiguous: int
    vas: tuple[int, ...]
    bits: bytes
    source: str = field(default="", compare=False)

    @property
    def port_set(self) -> str:
        """Identity of the enabled Rust root set: spec plus the digest of the resolved roots."""
        return f"{self.port_spec or 'off'}#{self.port_roots}:{self.port_digest:016x}"

    def hit(self, index: int) -> bool:
        return bool(self.bits[index >> 3] >> (index & 7) & 1)

    def hit_vas(self) -> frozenset[int]:
        return frozenset(va for i, va in enumerate(self.vas) if self.hit(i))


def _text(raw: bytes) -> str:
    return raw.split(b"\0", 1)[0].decode("ascii", "replace")


def _put_text(text: str, size: int) -> bytes:
    return text.encode("ascii")[:size].ljust(size, b"\0")


def parse(data: bytes, *, expected_lift_key: str | None = None, source: str = "") -> Census:
    if len(data) < HEADER_SIZE:
        raise CensusError(f"{source or 'census'}: {len(data)} bytes, shorter than the header")
    (
        magic,
        version,
        header_size,
        count,
        hits,
        ambiguous,
        reason,
        written,
        lift_key,
        build_sha,
        image_base,
        image_size,
        shift,
        roots,
        digest,
        port_spec,
        root_hits,
        _reserved,
    ) = _HEADER.unpack_from(data)
    if magic != MAGIC:
        raise CensusError(f"{source or 'census'}: bad magic {magic!r}")
    if version != VERSION or header_size != HEADER_SIZE:
        raise CensusError(
            f"{source or 'census'}: version {version} header {header_size} unsupported"
        )
    expected_size = HEADER_SIZE + 4 * count + (count + 7) // 8
    if len(data) != expected_size:
        raise CensusError(f"{source or 'census'}: {len(data)} bytes, header says {expected_size}")
    key = _text(lift_key)
    if expected_lift_key is not None and key != expected_lift_key:
        raise LiftKeyMismatch(f"{source or 'census'}: lift key {key}, expected {expected_lift_key}")
    vas = struct.unpack_from(f"<{count}I", data, HEADER_SIZE)
    if any(b <= a for a, b in zip(vas, vas[1:], strict=False)):
        raise CensusError(f"{source or 'census'}: VA table not strictly ascending")
    bits = data[HEADER_SIZE + 4 * count :]
    census = Census(
        lift_key=key,
        build_sha=_text(build_sha),
        port_spec=_text(port_spec),
        port_roots=roots,
        port_digest=digest,
        rust_root_hits=root_hits,
        reason=REASONS.get(reason, f"reason-{reason}"),
        written_unix=written,
        image_base=image_base,
        image_size=image_size,
        bucket_shift=shift,
        ambiguous=ambiguous,
        vas=tuple(vas),
        bits=bytes(bits),
        source=source,
    )
    if sum(bin(b).count("1") for b in bits) != hits:
        raise CensusError(
            f"{source or 'census'}: bitmap holds a different hit count than the header ({hits})"
        )
    return census


def load(path: Path, *, expected_lift_key: str | None = None) -> Census:
    return parse(Path(path).read_bytes(), expected_lift_key=expected_lift_key, source=str(path))


def build(
    vas: list[int] | tuple[int, ...],
    hit: set[int] | frozenset[int],
    *,
    lift_key: str,
    build_sha: str = "unknown",
    port_spec: str = "off",
    port_roots: int = 0,
    port_digest: int = 0xCBF29CE484222325,
    reason: int = 0,
    written_unix: int = 0,
) -> bytes:
    """The bytes live_census_write would produce for `hit` over the ascending `vas`."""
    ordered = sorted(vas)
    bits = bytearray((len(ordered) + 7) // 8)
    for i, va in enumerate(ordered):
        if va in hit:
            bits[i >> 3] |= 1 << (i & 7)
    header = _HEADER.pack(
        MAGIC,
        VERSION,
        HEADER_SIZE,
        len(ordered),
        sum(1 for va in ordered if va in hit),
        0,
        reason,
        written_unix,
        _put_text(lift_key, 32),
        _put_text(build_sha, 40),
        0x10000,
        0,
        4,
        port_roots,
        port_digest,
        _put_text(port_spec, 48),
        0,
        0,
    )
    return header + struct.pack(f"<{len(ordered)}I", *ordered) + bytes(bits)


def dispatch_vas(dispatch_c: Path) -> tuple[int, ...]:
    """The VA list of a lift's generated recomp_dispatch.c (g_recomp_table rows)."""
    return tuple(
        sorted({int(m.group(1), 16) for m in _DISPATCH_ROW.finditer(Path(dispatch_c).read_bytes())})
    )


def check_against_lift(census: Census, lift_vas: tuple[int, ...]) -> None:
    """Refuse a file whose VA table is not the function list of the lift its key names."""
    if tuple(census.vas) != tuple(lift_vas):
        raise LiftKeyMismatch(
            f"{census.source or 'census'}: VA table ({len(census.vas)} functions) "
            f"differs from lift "
            f"{census.lift_key} ({len(lift_vas)} functions)"
        )
