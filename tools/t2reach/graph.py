# SPDX-License-Identifier: GPL-3.0-or-later
"""Whole-image static reachability over the lifter's function table (T3793).

The question: can the retail title ever enter function F? The answer is a forward fixpoint
from the real entry points over every reference the image holds. It is built to
OVER-approximate reachability, so "unreachable" is only claimed for a function that nothing
live can name.

NODES. Every function table row (`generated/retail/functions.csv`), its body clipped at the
next entry so each byte has one owner, plus one GAP node per stretch of an executable code
section that no body covers. Gaps hold far chunks, alignment padding, inline tables and code
the table does not list (T3793 measured 400 executed functions whose only callers sit in
gaps), so a gap is code: it has edges of its own and it is live when its preceding function
is live (a chunk or a fallthrough) or when anything live points into it.

DATA. Every byte outside a code section (`.rdata`, `.data`, the XDK data sections, TLS and
callback tables, vtables, dispatch tables) is data. A data reference makes its target a ROOT:
any function entry named by 4 bytes at any offset, and any gap address named by an aligned
dword (an unlisted function in a vtable).

EDGES from a code node (function or gap), all scans are over raw bytes:

* exact: any 4 bytes equal to a function ENTRY, or a `E8`/`E9`/`0F 8x` rel32 landing on one,
  at every byte offset (no disassembly, so desync and inline data can not hide one);
* decoded: a linear Capstone sweep, every immediate or absolute displacement operand that
  lands in another node (rel8 short jumps, branches into a shared tail or a far chunk,
  `push offset`, `mov reg, offset`);
* chunk: a function to the gap that follows it.

A pointer into the middle of a FUNCTION from data is not a root: an SEH handler or a switch
arm only runs while its own body runs, and the body is entered through its entry.

ROOTS: the XBE entry point and every data reference. The fixpoint is a breadth-first search.
A spurious byte match only ADDS a root or an edge, so the error is one-sided: it can hide a
dead function, never invent one. Soundness is also checked against every function with
execution evidence (`tools.t2reach.audit`: none may come out unreachable).
"""

from __future__ import annotations

import bisect
import csv
from collections import deque
from collections.abc import Iterable, Mapping
from dataclasses import dataclass, field
from pathlib import Path

#: Code sections of the retail image. The XBE marks every section executable, so this is the
#: layout: the game `.text` and the statically linked XDK library sections.
CODE_SECTIONS = frozenset(
    {".text", "D3D", "XGRPH", "DSOUND", "XONLINE", "XNET", "XMV", "XPP", "DOLBY"}
)


@dataclass(frozen=True)
class Node:
    start: int
    end: int  # exclusive
    gap: bool = False

    def contains(self, va: int) -> bool:
        return self.start <= va < self.end


@dataclass
class Graph:
    """Static reference graph of the image; gap nodes are keyed by their start address."""

    nodes: dict[int, Node]
    edges: dict[int, set[int]] = field(default_factory=dict)
    #: node -> reasons it is a root
    roots: dict[int, set[str]] = field(default_factory=dict)
    #: code node -> kernel imports its decoded operands name (`call [thunk slot]`)
    imports: dict[int, set[str]] = field(default_factory=dict)
    #: start VAs of the data-object nodes
    data_objects: frozenset[int] = frozenset()

    @property
    def functions(self) -> frozenset[int]:
        return frozenset(va for va, node in self.nodes.items() if not node.gap)

    def callers(self) -> dict[int, set[int]]:
        result: dict[int, set[int]] = {}
        for caller, callees in self.edges.items():
            for callee in callees:
                result.setdefault(callee, set()).add(caller)
        return result

    def add_root(self, va: int, why: str) -> None:
        self.roots.setdefault(va, set()).add(why)


@dataclass(frozen=True)
class Reach:
    live: frozenset[int]
    graph: Graph

    def dead_functions(self) -> frozenset[int]:
        return self.graph.functions - self.live


def load_functions(path: Path) -> dict[int, Node]:
    """Function table rows, entry and body end (`body_max_va`, else `size_bytes`), clipped."""
    result: dict[int, Node] = {}
    with path.open(encoding="utf-8", newline="") as handle:
        for row in csv.DictReader(handle):
            entry = int(row["entry_va"], 16)
            raw_max = (row.get("body_max_va") or "").strip()
            end = int(raw_max, 16) + 1 if raw_max else entry + max(1, int(row["size_bytes"]))
            result[entry] = Node(entry, max(end, entry + 1))
    return partition(result)


def partition(functions: Mapping[int, Node]) -> dict[int, Node]:
    """Clip every body at the next entry, so each byte belongs to at most one function.

    A body bound that runs over later entries (a far chunk, or a broken `body_max_va` that
    spans hundreds of functions) would hide those functions' references as self-references.
    """
    ordered = sorted(functions)
    result: dict[int, Node] = {}
    for index, entry in enumerate(ordered):
        end = functions[entry].end
        if index + 1 < len(ordered):
            end = min(end, ordered[index + 1])
        result[entry] = Node(entry, max(end, entry + 1))
    return result


def gaps(lo: int, hi: int, ordered: Iterable[Node]) -> list[tuple[int, int]]:
    """Address ranges in [lo, hi) not covered by any of the (sorted, disjoint) bodies."""
    found: list[tuple[int, int]] = []
    cursor = lo
    for node in ordered:
        if node.start >= hi:
            break
        if node.end <= cursor:
            continue
        if node.start > cursor:
            found.append((cursor, node.start))
        cursor = max(cursor, node.end)
    if cursor < hi:
        found.append((cursor, hi))
    return found


class Locator:
    """Map an address to the node whose bytes hold it."""

    def __init__(self, nodes: Mapping[int, Node]) -> None:
        self.ordered = sorted(nodes.values(), key=lambda node: node.start)
        self.starts = [node.start for node in self.ordered]
        self.entries = frozenset(va for va, node in nodes.items() if not node.gap)

    def owner(self, va: int) -> Node | None:
        index = bisect.bisect_right(self.starts, va) - 1
        if index >= 0 and self.ordered[index].contains(va):
            return self.ordered[index]
        return None

    def before(self, va: int) -> Node | None:
        index = bisect.bisect_left(self.starts, va) - 1
        return self.ordered[index] if index >= 0 else None


def scan_exact(blob: bytes, base: int, entries: frozenset[int]) -> set[int]:
    """Function entries named by `blob` at any byte offset, as a 4-byte value or a rel32."""
    found: set[int] = set()
    size = len(blob)
    for offset in range(size - 3):
        value = int.from_bytes(blob[offset : offset + 4], "little")
        if value in entries:
            found.add(value)
        byte = blob[offset]
        if byte in (0xE8, 0xE9) and offset + 5 <= size:
            rel = int.from_bytes(blob[offset + 1 : offset + 5], "little", signed=True)
            target = (base + offset + 5 + rel) & 0xFFFFFFFF
            if target in entries:
                found.add(target)
        elif byte == 0x0F and offset + 6 <= size and 0x80 <= blob[offset + 1] <= 0x8F:
            rel = int.from_bytes(blob[offset + 2 : offset + 6], "little", signed=True)
            target = (base + offset + 6 + rel) & 0xFFFFFFFF
            if target in entries:
                found.add(target)
    return found


def decoded_values(blob: bytes, base: int) -> set[int]:
    """Immediate and absolute displacement operands of a linear Capstone sweep of `blob`.

    Undecodable bytes are skipped one at a time, so the sweep resynchronises after data.
    """
    from capstone import CS_ARCH_X86, CS_MODE_32, Cs
    from capstone.x86 import X86_OP_IMM, X86_OP_MEM

    md = Cs(CS_ARCH_X86, CS_MODE_32)
    md.detail = True
    found: set[int] = set()
    offset = 0
    while offset < len(blob):
        for insn in md.disasm(blob[offset:], base + offset):
            offset = insn.address - base + insn.size
            for operand in insn.operands:
                if operand.type == X86_OP_IMM:
                    found.add(operand.imm & 0xFFFFFFFF)
                elif operand.type == X86_OP_MEM and operand.mem.base == 0:
                    found.add(operand.mem.disp & 0xFFFFFFFF)
        offset += 1  # the sweep stopped on an undecodable byte (or at the end)
    return found


def aligned_values(blob: bytes, base: int) -> list[int]:
    """The dwords of `blob` at 4-aligned virtual addresses."""
    skip = (-base) % 4
    return [
        int.from_bytes(blob[offset : offset + 4], "little")
        for offset in range(skip, len(blob) - 3, 4)
    ]


@dataclass(frozen=True)
class Region:
    """A stretch of raw image bytes: `code` holds function or gap nodes, else data objects."""

    name: str
    start: int
    data: bytes
    code: bool


#: XBE header offset of the TLS directory address (the loader runs its callbacks).
XBE_TLS_OFFSET = 0x12C


def loader_roots(image: object) -> dict[int, str]:
    """Data addresses the XBE loader uses before or beside the entry point."""
    data = image.data  # type: ignore[attr-defined]
    tls = int.from_bytes(data[XBE_TLS_OFFSET : XBE_TLS_OFFSET + 4], "little")
    return {tls: "XBE TLS directory"} if tls else {}


def kernel_slots(image: object) -> dict[int, str]:
    """Kernel thunk slot address -> kernel export name."""
    from tools.kernel_ordinals import KERNEL_ORDINALS

    xbe = image.xbe  # type: ignore[attr-defined]
    return {
        xbe.kernel_thunk_addr + 4 * index: KERNEL_ORDINALS.get(ordinal & 0x7FFFFFFF, f"#{ordinal}")
        for index, ordinal in enumerate(xbe.kernel_import_ordinals)
    }


def regions_of(image: object) -> list[Region]:
    """Raw-backed sections of an image (`image.read`, `image.xbe.sections`)."""
    read = image.read  # type: ignore[attr-defined]
    found = []
    for section in image.xbe.sections:  # type: ignore[attr-defined]
        blob = read(section.virtual_addr, section.raw_size) or b""
        found.append(
            Region(section.name, section.virtual_addr, blob, section.name in CODE_SECTIONS)
        )
    return found


def build(
    regions: list[Region],
    functions: Mapping[int, Node],
    *,
    entry_point: int,
    data_roots: Mapping[int, str] | None = None,
    kernel_slots: Mapping[int, str] | None = None,
) -> Graph:
    """Reference graph over code nodes (functions, gaps) and data objects.

    DATA OBJECTS. A data section is split at every LABEL: an address that a decoded operand of
    any code, or an aligned dword of any data, points at. An object is the bytes from one
    label to the next. It is live only when a live node points into it (a constructor storing
    a vtable, `call [table + eax*4]`, a pointer in another live object), and only a live
    object's pointers count. `data_roots` are data addresses the loader itself uses (the TLS
    directory named by the XBE header); the header's section table is layout, not a use.
    """
    data_roots = dict(data_roots or {})
    nodes = dict(functions)
    ordered_functions = sorted(functions.values(), key=lambda node: node.start)
    for region in regions:
        if region.code:
            lo = region.start
            for start, stop in gaps(lo, lo + len(region.data), ordered_functions):
                nodes[start] = Node(start, stop, gap=True)
    code = Locator(nodes)
    blobs: dict[int, tuple[bytes, int]] = {}
    for region in regions:
        if region.code:
            for node in code.ordered:
                if region.start <= node.start < region.start + len(region.data):
                    lo = node.start - region.start
                    blobs[node.start] = (region.data[lo : lo + node.end - node.start], node.start)
    decoded = {va: decoded_values(blob, base) for va, (blob, base) in blobs.items()}
    data_regions = [region for region in regions if not region.code and region.data]

    def in_data(value: int) -> bool:
        return any(r.start <= value < r.start + len(r.data) for r in data_regions)

    labels: set[int] = {region.start for region in data_regions} | set(data_roots)
    for values in decoded.values():
        labels.update(value for value in values if in_data(value))
    for region in data_regions:
        labels.update(
            value for value in aligned_values(region.data, region.start) if in_data(value)
        )
    objects: dict[int, tuple[Region, int, int]] = {}
    for region in data_regions:
        hi = region.start + len(region.data)
        cuts = sorted(label for label in labels if region.start <= label < hi)
        for index, cut in enumerate(cuts):
            stop = cuts[index + 1] if index + 1 < len(cuts) else hi
            objects[cut] = (region, cut, stop)
            nodes[cut] = Node(cut, stop, gap=True)
    locator = Locator(nodes)
    graph = Graph(nodes)
    graph.data_objects = frozenset(objects)
    named = locator.entries | frozenset(objects)
    for va, (blob, base) in blobs.items():
        # Raw scan as well as the decode: a desynchronised sweep can not hide a label.
        found = scan_exact(blob, base, named)
        for value in decoded[va]:
            target = locator.owner(value)
            if target is not None:
                found.add(target.start)
            if kernel_slots and value in kernel_slots:
                graph.imports.setdefault(va, set()).add(kernel_slots[value])
        if va not in functions:  # a gap: a chunk or fallthrough of the body before it
            previous = code.before(va)
            if previous is not None and previous.end == va:
                graph.edges.setdefault(previous.start, set()).add(va)
        found.discard(va)
        graph.edges.setdefault(va, set()).update(found)
    for start, (region, lo, hi) in objects.items():
        # Three bytes past the end: a pointer may straddle the next (unaligned) label.
        blob = region.data[lo - region.start : hi - region.start + 3]
        found = scan_exact(blob, lo, locator.entries)
        for value in aligned_values(blob, lo):
            target = locator.owner(value)
            if target is not None and (target.gap or target.start == value):
                found.add(target.start)
        found.discard(start)
        graph.edges.setdefault(start, set()).update(found)
    for va, why in data_roots.items():
        target = locator.owner(va)
        if target is not None:
            graph.add_root(target.start, why)
    entry = locator.owner(entry_point)
    if entry is not None:
        graph.add_root(entry.start, "XBE entry point")
    return graph


def fixpoint(graph: Graph) -> Reach:
    """Forward closure from the roots over code references."""
    live: set[int] = set()
    queue = deque(sorted(graph.roots))
    while queue:
        va = queue.popleft()
        if va in live:
            continue
        live.add(va)
        queue.extend(graph.edges.get(va, ()))
    return Reach(frozenset(live), graph)
