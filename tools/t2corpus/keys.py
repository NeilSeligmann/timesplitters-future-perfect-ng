# SPDX-License-Identifier: GPL-3.0-or-later
"""Input digest and branch-path shape of a captured call (T3741)."""

from __future__ import annotations

import hashlib
import struct

from tools.t2corpus.spool import Record, Run, decode_regs

STACK_WINDOW = 0x10000


def canonical_runs(runs: list[Run]) -> list[Run]:
    """Sorted by address and coalesced (overlapping or adjacent); earlier run wins."""
    ordered = sorted(enumerate(runs), key=lambda item: (item[1][0], item[0]))
    addrs: list[int] = []
    blobs: list[bytes] = []
    for _, (addr, data) in ordered:
        if not data:
            continue
        if addrs and addr <= addrs[-1] + len(blobs[-1]):
            overlap = addrs[-1] + len(blobs[-1]) - addr
            blobs[-1] += data[overlap:]
        else:
            addrs.append(addr)
            blobs.append(data)
    return list(zip(addrs, blobs, strict=True))


def input_digest(rec: Record) -> str:
    """sha256 over entry regs and the canonical read set (stack args live in the read set).

    Format 2 (T3740): the entry implicit-state block follows, so two calls that differ only in the
    inherited g_seh_ebp (or flag bridge, DF, ...) are distinct inputs.
    Format 1 digests are unchanged.
    """
    sha = hashlib.sha256(rec.entry_regs)
    for addr, data in canonical_runs(rec.read_runs):
        sha.update(struct.pack("<II", addr, len(data)))
        sha.update(data)
    if rec.entry_implicit is not None:
        sha.update(b"implicit")
        sha.update(rec.entry_implicit)
    if rec.clock_events is not None:
        sha.update(b"clock_events")
        sha.update(struct.pack("<I", len(rec.clock_events)))
        for event in rec.clock_events:
            sha.update(struct.pack("<IQI", event.after_callees, event.value, len(event.writes)))
            for addr, data in event.writes:
                sha.update(struct.pack("<II", addr, len(data)))
                sha.update(data)
    return sha.hexdigest()


def _bucket(runs: list[Run]) -> int:
    return sum(len(data) for _, data in runs).bit_length()


def _relative(runs: list[Run], esp: int) -> list[tuple[str, int, int]]:
    out = []
    for addr, data in canonical_runs(runs):
        delta = addr - esp
        if abs(delta) <= STACK_WINDOW:
            out.append(("s", delta, len(data)))
        else:
            out.append(("a", addr, len(data)))
    return out


def shape_key(rec: Record) -> str:
    """Callee VA sequence plus esp-relative read/write address sets and size buckets."""
    esp = decode_regs(rec.entry_regs)["esp"]
    sha = hashlib.sha256()
    sha.update(repr([callee.va for callee in rec.callees]).encode())
    sha.update(repr(_relative(rec.read_runs, esp)).encode())
    sha.update(repr(_relative(rec.write_runs, esp)).encode())
    sha.update(repr((_bucket(rec.read_runs), _bucket(rec.write_runs))).encode())
    return sha.hexdigest()
