# SPDX-License-Identifier: GPL-3.0-or-later
"""Recorded-call mutation (T3822): FABRICATED inputs derived from real corpus entries.

A seed is a recording capture entry (`tools.t2corpus` record format 2): its entry registers, its
implicit state and its read-set R, the game state the call actually read. A mutant changes one to
three 32-bit slots of that input and keeps everything else:

- slots: the six general registers (not ESP/EBP: the frame stays where the read-set put it), the
  stack argument words (R words at ESP+4 .. ESP+4*arity) and every other aligned R word;
- a slot is a POINTER when its value lies inside the supplied memory (the seed's R). A pointer is
  only replaced by 0 (null: the read-set shows it is dereferenced into a mapped region we supply,
  so null probes the original's null handling), by itself +-4 or +-1 while still inside supplied
  memory, or by the start of another supplied run. A mutant therefore never points into memory
  it does not supply (self-consistent read-set, `consistent`);
- any other slot gets an edge value: 0, -1, INT_MAX, INT_MIN, 0xFFFF, 0x10000, two's complement
  negation, +1, -1, float sign flip, a flipped bit, a small value.

Every choice comes from `random.Random(derived_seed(seed, va, seed digest, k))`: the same seed,
function, seed entry and mutant number always give the same bytes (`mutant` is pure). The return
address at ESP is replaced by the input tag `generate.RET_BASE + index`, as in the T3796 harness.

The ORIGINAL decides (`tools.t2harness.construct`): a mutant is refused when the lifted original
faults, hangs, sets a capture_incomplete flag, leaves the guest range, or reads a byte the mutant
did not supply (`unsupplied_read`: it read arena poison, so the input is not evidence). Everything
here is FABRICATED (rule 6 label): agreement says nothing about whether the game ever reaches the
mutated state.
"""

from __future__ import annotations

import bisect
import hashlib
import random
import struct
from collections.abc import Callable
from dataclasses import dataclass, field

from tools.t2corpus.spool import REG_NAMES, Record, decode_record, encode_record
from tools.t2harness import generate as gen

GENERATOR = "tools.t2harness.mutate"
GENERATOR_VERSION = 1
DEFAULT_SEED = 3822
MUTABLE_REGS = ("eax", "ecx", "edx", "ebx", "esi", "edi")
EDGE_OPS = (
    "zero", "minus_one", "int_max", "int_min", "u16_max", "u16_wrap", "negate", "plus_one",
    "minus_one_delta", "float_sign", "bit_flip", "small",
)  # fmt: skip
POINTER_OPS = ("null", "plus_four", "minus_four", "plus_one_byte", "retarget")
MAX_SLOTS_PER_MUTANT = 3
# A stack argument or register slot is picked this much more often than another R word.
ARG_WEIGHT = 4


@dataclass(frozen=True)
class Seed:
    """One corpus entry used as the base of mutants."""

    digest: str
    record: Record


@dataclass
class Change:
    slot: str  # "reg:eax" or "mem:0x..."
    op: str
    old: int
    new: int


@dataclass
class Mutant:
    index: int
    seed_digest: str
    regs_blob: bytes
    implicit: bytes
    runs: list[tuple[int, bytes]]
    changes: list[Change] = field(default_factory=list)
    strategy: str = "mutate"

    @property
    def return_address(self) -> int:
        return gen.RET_BASE + self.index


class Supplied:
    """Sorted, merged [lo, hi) intervals of supplied memory."""

    def __init__(self, runs: list[tuple[int, bytes]]) -> None:
        spans = sorted((addr, addr + len(data)) for addr, data in runs if data)
        merged: list[list[int]] = []
        for lo, hi in spans:
            if merged and lo <= merged[-1][1]:
                merged[-1][1] = max(merged[-1][1], hi)
            else:
                merged.append([lo, hi])
        self.spans = [(lo, hi) for lo, hi in merged]
        self._starts = [lo for lo, _ in self.spans]

    def contains(self, addr: int, size: int = 1) -> bool:
        at = bisect.bisect_right(self._starts, addr) - 1
        return at >= 0 and addr + size <= self.spans[at][1]

    def outside(self, runs: list[tuple[int, bytes]]) -> int | None:
        """The first address of `runs` not supplied, else None."""
        for addr, data in runs:
            pos = addr
            end = addr + max(len(data), 1)
            while pos < end:
                at = bisect.bisect_right(self._starts, pos) - 1
                if at < 0 or pos >= self.spans[at][1]:
                    return pos
                pos = self.spans[at][1]
        return None


def derived_seed(seed: int, va: int, seed_digest: str, number: int) -> int:
    text = f"{GENERATOR}/{GENERATOR_VERSION}:{seed}:{va:#010x}:{seed_digest}:{number}"
    return int.from_bytes(hashlib.sha256(text.encode()).digest()[:8], "little")


def entry_esp(record: Record) -> int:
    return struct.unpack_from("<I", record.entry_regs, 4 * REG_NAMES.index("esp"))[0]


def _memory(runs: list[tuple[int, bytes]]) -> dict[int, int]:
    out: dict[int, int] = {}
    for addr, data in runs:
        for offset, byte in enumerate(data):
            out.setdefault(addr + offset, byte)
    return out


def _runs(memory: dict[int, int]) -> list[tuple[int, bytes]]:
    runs: list[tuple[int, bytes]] = []
    start = prev = None
    chunk = bytearray()
    for addr in sorted(memory):
        if prev is not None and addr == prev + 1:
            chunk.append(memory[addr])
        else:
            if start is not None:
                runs.append((start, bytes(chunk)))
            start, chunk = addr, bytearray([memory[addr]])
        prev = addr
    if start is not None:
        runs.append((start, bytes(chunk)))
    return runs


def mem_slots(record: Record) -> list[int]:
    """Aligned 4-byte words fully inside the read-set."""
    supplied = Supplied(record.read_runs)
    words: set[int] = set()
    for addr, data in record.read_runs:
        first = (addr + 3) & ~3
        for word in range(first, addr + len(data) - 3, 4):
            if supplied.contains(word, 4):
                words.add(word)
    return sorted(words)


def slots(record: Record, stack_args: int) -> list[tuple[str, int]]:
    """(slot, weight): registers and stack arguments weigh `ARG_WEIGHT`, other R words 1."""
    esp = entry_esp(record)
    args = range(esp + 4, esp + 4 + 4 * max(0, stack_args))
    out = [(f"reg:{name}", ARG_WEIGHT) for name in MUTABLE_REGS]
    for word in mem_slots(record):
        if esp <= word < esp + 4:
            continue  # the return address slot carries the input tag
        out.append((f"mem:{word:#x}", ARG_WEIGHT if word in args else 1))
    return out


def edge(op: str, value: int, rng: random.Random) -> int:
    table = {
        "zero": 0,
        "minus_one": 0xFFFFFFFF,
        "int_max": 0x7FFFFFFF,
        "int_min": 0x80000000,
        "u16_max": 0xFFFF,
        "u16_wrap": 0x10000,
        "negate": (-value) & 0xFFFFFFFF,
        "plus_one": (value + 1) & 0xFFFFFFFF,
        "minus_one_delta": (value - 1) & 0xFFFFFFFF,
        "float_sign": value ^ 0x80000000,
    }
    if op in table:
        return table[op]
    if op == "bit_flip":
        return value ^ (1 << rng.randrange(32))
    if op == "small":
        return rng.randrange(16)
    raise ValueError(f"unknown edge op {op!r}")


def pointer_op(op: str, value: int, supplied: Supplied, rng: random.Random) -> int | None:
    """The new pointer value, None when the op would leave supplied memory."""
    if op == "null":
        return 0
    if op == "retarget":
        others = [lo for lo, _hi in supplied.spans if not lo <= value < _hi]
        return rng.choice(others) if others else None
    delta = {"plus_four": 4, "minus_four": -4, "plus_one_byte": 1}[op]
    new = (value + delta) & 0xFFFFFFFF
    return new if supplied.contains(new) else None


def mutant(
    seed_entry: Seed, va: int, stack_args: int, seed: int, number: int, index: int
) -> Mutant:
    """Mutant `number` of `seed_entry`, tagged with input `index`. Pure."""
    rec = seed_entry.record
    rng = random.Random(derived_seed(seed, va, seed_entry.digest, number))
    supplied = Supplied(rec.read_runs)
    memory = _memory(rec.read_runs)
    regs = list(struct.unpack_from("<8I", rec.entry_regs, 0))
    candidates = slots(rec, stack_args)
    weights = [weight for _slot, weight in candidates]
    count = 1 + rng.randrange(MAX_SLOTS_PER_MUTANT)
    changes: list[Change] = []
    used: set[str] = set()
    # A slot the op leaves as it was (zero on 0) does not count: picks go on (bounded) until
    # `count` slots changed, so a mutant always differs from its seed and a recorded input never
    # passes as a fabricated one.
    for _attempt in range(4 * MAX_SLOTS_PER_MUTANT):
        if len(changes) >= count or len(used) >= len(candidates):
            break
        slot = rng.choices(candidates, weights)[0][0]
        if slot in used:
            continue
        used.add(slot)
        kind, name = slot.split(":", 1)
        if kind == "reg":
            old = regs[REG_NAMES.index(name)]
        else:
            addr = int(name, 16)
            old = int.from_bytes(bytes(memory[addr + i] for i in range(4)), "little")
        if old and supplied.contains(old):
            op = rng.choice(POINTER_OPS)
            new = pointer_op(op, old, supplied, rng)
            if new is None:
                op, new = "null", 0
        else:
            op = rng.choice(EDGE_OPS)
            new = edge(op, old, rng)
        if new == old:
            continue
        changes.append(Change(slot, op, old, new))
        if kind == "reg":
            regs[REG_NAMES.index(name)] = new
        else:
            for i, byte in enumerate(new.to_bytes(4, "little")):
                memory[addr + i] = byte
    esp = entry_esp(rec)
    for i, byte in enumerate((gen.RET_BASE + index).to_bytes(4, "little")):
        memory[esp + i] = byte
    blob = struct.pack("<8I", *regs) + rec.entry_regs[32:]
    return Mutant(index, seed_entry.digest, blob, rec.entry_implicit, _runs(memory), changes)


def mutants(
    seeds: list[Seed], va: int, stack_args: int, seed: int, count: int, first_number: int = 0,
    first_index: int = 0,
) -> list[Mutant]:  # fmt: skip
    """`count` mutants spread round-robin over `seeds` (mutant numbers from `first_number`)."""
    out: list[Mutant] = []
    for offset in range(count):
        entry = seeds[offset % len(seeds)]
        number = first_number + offset // len(seeds)
        out.append(mutant(entry, va, stack_args, seed, number, first_index + offset))
    return out


def consistent(seed_entry: Seed, item: Mutant) -> list[str]:
    """Why a mutant breaks the self-consistency rule (empty when it holds): every seed pointer slot
    is unchanged, null or still inside supplied memory; every byte outside the changed slots and the
    tag equals the seed; nothing beyond the seed's R and the tag is supplied."""
    rec = seed_entry.record
    supplied = Supplied(rec.read_runs)
    esp = entry_esp(rec)
    allowed = {esp + i for i in range(4)}
    problems = []
    for change in item.changes:
        if change.old and supplied.contains(change.old):
            if change.new and not supplied.contains(change.new):
                problems.append(f"{change.slot}: pointer {change.old:#x} -> {change.new:#x}")
        if change.slot.startswith("mem:"):
            addr = int(change.slot[4:], 16)
            allowed.update(range(addr, addr + 4))
    before, after = _memory(rec.read_runs), _memory(item.runs)
    extra = set(after) - set(before) - allowed
    if extra:
        problems.append(f"supplies {len(extra)} byte(s) beyond the seed read-set")
    changed = [a for a in after if a in before and after[a] != before[a] and a not in allowed]
    if changed:
        problems.append(f"{len(changed)} unchanged-slot byte(s) differ, first {changed[0]:#x}")
    return problems


def encode_input(va: int, item: Mutant, _global_pages: object = None) -> bytes:
    """The `--t2-construct` input record of a mutant (same layout as `generate.encode_input`)."""
    return encode_record(
        Record(
            va=va,
            caller_va=0,
            return_address=item.return_address,
            poll=0,
            guest_thread=0,
            flags=0,
            entry_regs=item.regs_blob,
            exit_regs=item.regs_blob,
            read_runs=list(item.runs),
            entry_implicit=item.implicit,
            exit_implicit=item.implicit,
        )
    )


def unsupplied_read(item: Mutant, raw: bytes) -> int | None:
    """The first address the original's record read that the mutant did not supply, else None."""
    return Supplied(item.runs).outside(decode_record(raw).read_runs)


# Read-set fill (T3826): class c retry of `unsupplied_read` refusals ----------------------------


def unsupplied_bytes(item: Mutant, raw: bytes) -> list[int]:
    """Every byte address the original's record read that the mutant did not supply."""
    supplied = Supplied(item.runs)
    out = []
    for addr, data in decode_record(raw).read_runs:
        out += [a for a in range(addr, addr + max(len(data), 1)) if not supplied.contains(a)]
    return out


class FillSource:
    """Bytes for unsupplied reads: the caller's recorded memory (union of its entries' read-sets,
    first entry by digest wins), then the XBE image as loaded (`targets.image_reader`). Both are
    INFERRED call-site state; image bytes are owner data and stay in the run-local store."""

    def __init__(
        self,
        caller: dict[int, int],
        image: Callable[[int], tuple[bytes, bool] | None] | None = None,
    ) -> None:
        self.caller = caller
        self.image = image
        self._pages: dict[int, bytes | None] = {}

    def get(self, addr: int) -> int | None:
        if addr in self.caller:
            return self.caller[addr]
        if self.image is None:
            return None
        page = addr & ~0xFFF
        if page not in self._pages:
            found = self.image(page)
            self._pages[page] = found[0] if found else None
        data = self._pages[page]
        return data[addr - page] if data is not None else None


def caller_memory(seeds: list[Seed]) -> dict[int, int]:
    """Union of the read-sets of the caller's entries (earlier seed wins on overlap)."""
    out: dict[int, int] = {}
    for entry in seeds:
        for addr, byte in _memory(entry.record.read_runs).items():
            out.setdefault(addr, byte)
    return out


def filled(item: Mutant, raw: bytes, source: FillSource, index: int) -> Mutant | None:
    """`item` with every byte its refused record read but it did not supply taken from `source`
    (the caller's recorded memory, then the XBE image: INFERRED call-site state), tagged as
    input `index`. None when a byte is in neither, so the input stays refused."""
    wanted = unsupplied_bytes(item, raw)
    values = [source.get(a) for a in wanted]
    if not wanted or any(v is None for v in values):
        return None
    memory = _memory(item.runs)
    for addr, value in zip(wanted, values, strict=True):
        memory[addr] = int(value)  # type: ignore[arg-type]
    esp = struct.unpack_from("<I", item.regs_blob, 4 * REG_NAMES.index("esp"))[0]
    for i, byte in enumerate((gen.RET_BASE + index).to_bytes(4, "little")):
        memory[esp + i] = byte  # the input tag moves to the new index
    return Mutant(
        index, item.seed_digest, item.regs_blob, item.implicit, _runs(memory), item.changes,
        strategy="mutate+fill",
    )  # fmt: skip
