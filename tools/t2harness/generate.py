# SPDX-License-Identifier: GPL-3.0-or-later
"""Deterministic FABRICATED inputs for constructed calls (T3796).

An input is what a capture record holds before the call: the entry register file, the implicit
thread state and the memory the call may read. It is built from the function's contract (`Contract`:
stack arity from `tools.replace.invpose_meta.describe`, x87/SSE use, a branch-count path hint from
the lifted body) and a seed.

Memory layout of the constructed state (guest addresses outside the XBE image and the MMIO ranges):

- stack: the return address (a tag naming the input, `RET_BASE + index`) at `ESP`, then the stack
  arguments and `SLACK_WORDS` seeded caller words (in case the arity under-counts);
- heap: `HEAP_PAGES` pages at `HEAP` holding seeded words; pointer arguments and pointer-valued heap
  words point into it (objects every `OBJECT_STRIDE` bytes);
- a thread information block at `TIB` (`fs_base`): SEH chain end, stack bounds, self pointer;
- global pages: pages of the XBE image the original read in an earlier round (`construct` adds them;
  read-only sections and even variants hold the image bytes, odd variants refill writable sections
  from the strategy; never committed).

Strategies (one input shape family each; several seeded variants per strategy): zero, small,
pointer, mixed, float, extreme. Everything here is FABRICATED (rule 6 label): agreement on these
inputs says nothing about inputs outside the constructed domain.
"""

from __future__ import annotations

import hashlib
import random
import struct
from dataclasses import dataclass, field

from tools.t2corpus.spool import Record, encode_record

GENERATOR = "tools.t2harness.generate"
GENERATOR_VERSION = 1
DEFAULT_SEED = 3796
STRATEGIES = ("zero", "small", "pointer", "mixed", "float", "extreme")
VARIANTS = 4  # seeded variants per strategy ("zero" has one: its variants would be identical)

ESP = 0x0E00F000
STACK_TOP = 0x0E010000
SLACK_WORDS = 8
HEAP = 0x0D000000
HEAP_PAGES = 16
OBJECT_STRIDE = 0x100
OBJECT_SPAN_PAGES = (
    8  # objects (pointer targets) start in the first half, so fields up to +32 KiB stay inside
)
TIB = 0x0C000000
RET_BASE = 0x7E000000
PAGE = 0x1000
FPU_CONTROL = 0x037F


@dataclass(frozen=True)
class Contract:
    """What the generator knows about a function (all of it from the lifted body and the port
    registry)."""

    va: int
    stack_args: int
    calls: bool = False
    x87: bool = False
    sse: bool = False
    #: 1 + conditional branches in the lifted body: an upper-bound proxy for CFG paths (INFERRED).
    paths_hint: int = 1

    @property
    def shape_target(self) -> int:
        return min(3, max(1, self.paths_hint))


@dataclass
class Candidate:
    index: int
    strategy: str
    variant: int
    seed: int
    regs: dict[str, int]
    implicit: tuple[int, ...]
    runs: list[tuple[int, bytes]] = field(default_factory=list)

    @property
    def return_address(self) -> int:
        return RET_BASE + self.index


def derived_seed(seed: int, va: int, strategy: str, variant: int) -> int:
    text = f"{GENERATOR}/{GENERATOR_VERSION}:{seed}:{va:#010x}:{strategy}:{variant}"
    return int.from_bytes(hashlib.sha256(text.encode()).digest()[:8], "little")


def f32(value: float) -> int:
    return struct.unpack("<I", struct.pack("<f", value))[0]


def _word(rng: random.Random, strategy: str, heap_words: int) -> int:
    """One seeded 32-bit value of the strategy's distribution."""
    if strategy == "zero":
        return 0
    if strategy == "small":
        return rng.randrange(0, 16)
    if strategy == "float":
        return rng.choice((f32(0.0), f32(1.0), f32(-1.0), f32(rng.uniform(-1000.0, 1000.0))))
    if strategy == "extreme":
        return rng.choice((0, 1, 0xFFFFFFFF, 0x80000000, 0x7FFFFFFF, 0xFFFF, 0x10000))
    pick = rng.random()
    heap_pointer = HEAP + 4 * rng.randrange(heap_words // 2)
    if strategy == "pointer":
        if pick < 0.4:
            return heap_pointer
        if pick < 0.7:
            return rng.randrange(0, 16)
        if pick < 0.9:
            return f32(rng.uniform(-100.0, 100.0))
        return 0
    # mixed
    return rng.choice(
        (
            heap_pointer,
            rng.randrange(0, 256),
            0,
            0xFFFFFFFF,
            f32(rng.uniform(-10.0, 10.0)),
            rng.getrandbits(32),
        )
    )


def _argument(rng: random.Random, strategy: str, heap_words: int) -> int:
    if strategy in ("pointer", "mixed") and rng.random() < 0.6:
        return HEAP + OBJECT_STRIDE * rng.randrange(OBJECT_SPAN_PAGES * PAGE // OBJECT_STRIDE)
    return _word(rng, strategy, heap_words)


def candidate(contract: Contract, seed: int, strategy: str, variant: int, index: int) -> Candidate:
    """One input. Pure: the same arguments always give the same bytes."""
    own = derived_seed(seed, contract.va, strategy, variant)
    rng = random.Random(own)
    heap_words = HEAP_PAGES * PAGE // 4
    heap = struct.pack(
        f"<{heap_words}I", *(_word(rng, strategy, heap_words) for _ in range(heap_words))
    )
    args = [_argument(rng, strategy, heap_words) for _ in range(contract.stack_args)]
    slack = [_word(rng, strategy, heap_words) for _ in range(SLACK_WORDS)]
    ret = RET_BASE + index
    stack = struct.pack(f"<{1 + len(args) + len(slack)}I", ret, *args, *slack)
    ebp = STACK_TOP - 0x20
    names = ("eax", "ecx", "edx", "ebx", "esi", "edi")
    regs = {name: _argument(rng, strategy, heap_words) for name in names}
    regs.update(esp=ESP, ebp=ebp)
    tib = bytearray(PAGE)
    struct.pack_into("<III", tib, 0, 0xFFFFFFFF, STACK_TOP, STACK_TOP - 0x10000)
    struct.pack_into("<I", tib, 0x18, TIB)
    implicit = (ebp, 0, 0, 0, 0, 0, TIB, 0)
    runs = [(ESP, stack), (HEAP, heap), (TIB, bytes(tib))]
    return Candidate(index, strategy, variant, own, regs, implicit, runs)


def candidates(
    contract: Contract, seed: int = DEFAULT_SEED, variants: int = VARIANTS
) -> list[Candidate]:
    out: list[Candidate] = []
    for strategy in STRATEGIES:
        for variant in range(1 if strategy == "zero" else variants):
            out.append(candidate(contract, seed, strategy, variant, len(out)))
    return out


def regs_blob(regs: dict[str, int]) -> bytes:
    """A 240-byte ExtRegisters (crates/tsfp-port-runtime/src/abi.rs): empty x87 stack, default
    control word."""
    order = ("eax", "ecx", "edx", "esp", "ebx", "ebp", "esi", "edi")
    head = struct.pack("<9I", *(regs.get(name, 0) & 0xFFFFFFFF for name in order), 0)
    fpu = (
        bytes(4) + struct.pack("<8d", *([0.0] * 8)) + struct.pack("<IH", 0, FPU_CONTROL) + bytes(2)
    )
    blob = head + fpu + bytes(80 + 48)
    if len(blob) != 240:
        raise AssertionError(f"ExtRegisters blob is {len(blob)} bytes")
    return blob


def global_page(cand: Candidate, page: int, image: bytes, writable: bool) -> bytes:
    """A page of XBE globals for this input: read-only pages and even variants keep the image bytes
    (the boot state); odd variants refill writable pages from the strategy (runtime state the image
    does not hold)."""
    if not writable or cand.variant % 2 == 0:
        return image
    rng = random.Random(cand.seed ^ page)
    heap_words = HEAP_PAGES * PAGE // 4
    return struct.pack(
        f"<{PAGE // 4}I", *(_word(rng, cand.strategy, heap_words) for _ in range(PAGE // 4))
    )


def encode_input(
    va: int, cand: Candidate, global_pages: dict[int, tuple[bytes, bool]] | None = None
) -> bytes:
    """The `--t2-construct` input record: inputs only (outputs are what the original will produce).
    `global_pages`: page VA -> (image bytes, writable section)."""
    blob = regs_blob(cand.regs)
    implicit = struct.pack("<8I", *cand.implicit)
    pages = sorted((global_pages or {}).items())
    runs = list(cand.runs) + [
        (page, global_page(cand, page, image, writable)) for page, (image, writable) in pages
    ]
    record = Record(
        va=va,
        caller_va=0,
        return_address=cand.return_address,
        poll=0,
        guest_thread=0,
        flags=0,
        entry_regs=blob,
        exit_regs=blob,
        read_runs=runs,
        entry_implicit=implicit,
        exit_implicit=implicit,
    )
    return encode_record(record)
