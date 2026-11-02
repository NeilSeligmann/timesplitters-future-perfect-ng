# SPDX-License-Identifier: GPL-3.0-or-later
"""T2346: call-argument detector stream for `tools.rust_trace --call-args`.

The host (src/host/rust_ports.c, TSFP_CALL_ARGS_TRACE) writes one row per call of a listed root:
`poll va 0 a0 a1 a2 a3 d0 d1 d2 d3 s0..s7` at entry and
`poll va 1 eax st0 d0 d1 d2 d3 s0..s7` at exit, hex fields. `di` is FNV-1a of the 64 bytes
argument i points to; `s0..s7` digest the pad mapping/records, button ring, input gates/selectors,
device state and local-player input flags. ST0 is recorded as f32 bits. Rows are keyed by
(poll, va, phase, ordinal within that poll) and compared field by field inside the deterministic
window. Fields that differ between a reference run and its control runs (heap pointers, wall-clock
state) are masked per key, so the stream only flags what the compared run changes. Normal
off-versus-selected traces use off/off controls; mutation checks use same-root selected references.
A requested root with no stable rows is reported as missing coverage and makes the comparison
unequal.

Stack residue (T3702): a host that records `esp stack_lo stack_hi` (three trailing hex fields on
every row, 22 entry / 20 exit fields) lets the comparison excuse a digest difference of a pointer
argument whose 64-byte window lies in the calling thread's DEAD stack. A window straddling ESP also
records the digest of its live suffix (count, then argument-index/digest pairs); only when that
suffix matches on both sides can the dead prefix be excused. Only those argument digests are
excused: the row must differ in nothing else. Rows without stack fields or required suffix digests
are never excused.
"""

from __future__ import annotations

from collections.abc import Iterator
from pathlib import Path

Key = tuple[int, int, int, int]

ENTRY_FIELDS = 19
EXIT_FIELDS = 17
STACK_FIELDS = 3
DIGEST_BYTES = 64
"""Bytes each `d<i>` digest covers (src/host/rust_ports.c call_args_digest(arg, 64))."""
EXCLUDED_ADDRESS_CAP = 64
OVER_HALF = 0.5
_ENTRY_CACHE_LIMIT = 65536


TAIL_HEALTH_FRACTION = 0.10
"""Excluded tail above this share of a root's rows in one run is reported, never silent."""


def last_sample_poll(trace: dict) -> int | None:
    """Highest region-sample poll recorded in a trace.json dict, else None."""
    samples = trace.get("samples")
    polls = []
    for sample in samples if isinstance(samples, list) else []:
        try:
            polls.append(int(sample["poll"]))
        except (KeyError, TypeError, ValueError):
            continue
    return max(polls) if polls else None


def common_last_poll(left_trace: dict, right_trace: dict) -> int | None:
    """Last poll BOTH runs reached (min of the final region samples), else None."""
    left, right = last_sample_poll(left_trace), last_sample_poll(right_trace)
    return min(left, right) if left is not None and right is not None else None


class TailTally:
    """Rows after the common last poll, per root and per run (T3702 call-tail guard).

    A capped run and a run that ended later differ only in rows past the final poll both reached;
    those rows have no peer, so they are excluded and counted, never silently dropped.
    """

    def __init__(self, horizon: int | None) -> None:
        self.horizon = horizon
        self.excluded: dict[str, dict[str, int]] = {}
        self.total: dict[str, dict[str, int]] = {}

    def note(self, side: str, root: int, excluded: bool) -> None:
        name = f"0x{root:X}"
        self.total.setdefault(side, {}).setdefault(name, 0)
        self.total[side][name] += 1
        if excluded:
            self.excluded.setdefault(side, {}).setdefault(name, 0)
            self.excluded[side][name] += 1

    def summary(self) -> dict:
        per_root: dict[str, int] = {}
        health = []
        for side, roots in sorted(self.excluded.items()):
            for root, rows in roots.items():
                per_root[root] = per_root.get(root, 0) + rows
                total = self.total.get(side, {}).get(root, 0)
                if total and rows / total > TAIL_HEALTH_FRACTION:
                    health.append(
                        f"HEALTH: call-args tail after poll {self.horizon} excludes {rows} of "
                        f"{total} rows ({rows / total:.0%}) of root {root} in the {side} run, "
                        f"above {TAIL_HEALTH_FRACTION:.0%}: capture lengths differ a lot"
                    )
        rows = sum(per_root.values())
        return {
            "call_args_common_last_poll": self.horizon,
            "call_args_tail_excluded": per_root,
            "call_args_tail_excluded_rows": rows,
            "call_args_tail_health": health,
            "call_args_tail_text": (
                f"tail after poll {self.horizon} excluded ({rows} rows)" if rows else ""
            ),
        }


def count_tail(
    path: Path, horizon: int, side: str, tally: TailTally, roots: set[int] | None = None
) -> None:
    """Stream a call-args file into the tally (rows after `horizon` counted as excluded)."""
    for row in iter_file(path):
        if roots is None or row[1] in roots:
            tally.note(side, row[1], row[0] > horizon)


def row_field_counts(phase: int) -> tuple[int, int]:
    """Accepted field counts for a row: (without stack info, with stack info)."""
    if phase not in (0, 1):
        return -1, -1
    base = ENTRY_FIELDS if phase == 0 else EXIT_FIELDS
    return base, base + STACK_FIELDS


def valid_row_fields(parts: list[str], phase: int) -> bool:
    """Validate classic, stack, and bounded partial-window metadata row shapes."""
    if phase not in (0, 1):
        return False
    base, with_stack = row_field_counts(phase)
    if len(parts) in (base, with_stack):
        return True
    if len(parts) < with_stack + 1:
        return False
    try:
        count = int(parts[with_stack], 16)
        if not 1 <= count <= 4:
            return False
        indexes = [int(parts[with_stack + 1 + index * 2], 16) for index in range(count)]
    except (IndexError, ValueError):
        return False
    return (
        1 <= count <= 4
        and len(parts) == with_stack + 1 + count * 2
        and all(index < 4 for index in indexes)
        and len(set(indexes)) == count
    )


def split_row(
    row: list[int],
) -> tuple[list[int], tuple[int, int, int] | None, dict[int, int]]:
    """Split classic fields, optional stack bounds, and live-suffix digests by argument."""
    base, with_stack = row_field_counts(row[2])
    if len(row) == base:
        return row, None, {}
    stack = (row[base], row[base + 1], row[base + 2])
    tail = row[with_stack:]
    if not tail:
        return row[:base], stack, {}
    count = tail[0]
    live_suffixes = {tail[1 + index * 2]: tail[2 + index * 2] for index in range(count)}
    return row[:base], stack, live_suffixes


def _dead_stack_pointer(stack: tuple[int, int, int] | None, pointer: int) -> bool:
    """True when the whole digest window of `pointer` is dead stack of the calling thread."""
    if stack is None:
        return False
    esp, low, high = stack
    if not (0 < low < high and low <= esp <= high):
        return False
    return low <= pointer and pointer + DIGEST_BYTES <= esp


def _straddles_dead_stack(stack: tuple[int, int, int] | None, pointer: int) -> bool:
    """True when a pointer digest has a dead prefix and a live suffix inside the thread stack."""
    if stack is None:
        return False
    esp, low, high = stack
    end = pointer + DIGEST_BYTES
    return 0 < low < high and low <= pointer < esp < end <= high


def residue_addresses(
    phase: int,
    changed: list[int],
    left_stack: tuple[int, int, int] | None,
    right_stack: tuple[int, int, int] | None,
    left_args: tuple[int, ...] | None,
    right_args: tuple[int, ...] | None,
    left_live_suffixes: dict[int, int],
    right_live_suffixes: dict[int, int],
) -> tuple[list[int], list[int]] | None:
    """Pointer addresses excused for a row that differs only in argument digests, else None.

    `changed` holds indexes into the row's fields after (poll, va, phase). Every changed field
    must be a pointer-argument digest `d<i>` whose pointer is equal on both sides and whose 64
    byte window lies below the call-time ESP and inside the thread stack, with the same ESP and
    stack range on both sides. Anything else (return values, other args, region digests, missing
    stack info, heap or live-frame pointers) returns None and the row stays a divergence.
    """
    first_digest = 4 if phase == 0 else 2
    if not changed or left_stack is None or left_stack != right_stack:
        return None
    if left_args is None or right_args is None or left_args != right_args:
        return None
    addresses, partial = [], []
    for index in changed:
        arg = index - first_digest
        if not 0 <= arg < 4:
            return None
        pointer = left_args[arg]
        if not _dead_stack_pointer(left_stack, pointer):
            if (
                not _straddles_dead_stack(left_stack, pointer)
                or arg not in left_live_suffixes
                or left_live_suffixes.get(arg) != right_live_suffixes.get(arg)
            ):
                return None
            partial.append(pointer)
        addresses.append(pointer)
    return addresses, partial


class EntryArgs:
    """Entry arguments by (root, call-time ESP) so an exit row finds its pointer arguments."""

    def __init__(self) -> None:
        self._args: dict[tuple[int, int], tuple[int, ...]] = {}

    def note(self, row: list[int]) -> None:
        base, stack, _ = split_row(row)
        if stack is not None and base[2] == 0:
            if len(self._args) >= _ENTRY_CACHE_LIMIT:
                self._args.clear()
            self._args[(base[1], stack[0])] = tuple(base[3:7])

    def args_of(self, row: list[int]) -> tuple[int, ...] | None:
        base, stack, _ = split_row(row)
        if stack is None:
            return None
        if base[2] == 0:
            return tuple(base[3:7])
        return self._args.get((base[1], stack[0]))


class ResidueTally:
    """Per-root counts of excused rows and the (capped) excluded addresses."""

    def __init__(self) -> None:
        self.rows: dict[str, int] = {}
        self.addresses: dict[str, set[int]] = {}
        self.partial_rows: dict[str, int] = {}

    def add(self, root: str, addresses: list[int], partial_addresses: list[int]) -> None:
        self.rows[root] = self.rows.get(root, 0) + 1
        self.addresses.setdefault(root, set()).update(addresses)
        if partial_addresses:
            self.partial_rows[root] = self.partial_rows.get(root, 0) + 1

    def summary(self, rows_per_root: dict[str, int]) -> dict:
        """Report fields: totals, capped addresses and roots whose excused rows exceed 50%."""
        excluded = {
            root: {
                "rows": self.rows[root],
                "addresses": [f"0x{a:X}" for a in sorted(self.addresses[root])][
                    :EXCLUDED_ADDRESS_CAP
                ],
                "address_count": len(self.addresses[root]),
                "partial_rows": self.partial_rows.get(root, 0),
            }
            for root in sorted(self.rows)
        }
        health = [
            f"HEALTH stack-residue {root}: {item['rows']} of {rows_per_root[root]} rows "
            f"({100 * item['rows'] // rows_per_root[root]}%) excluded as dead-stack bytes, "
            f"including {item['partial_rows']} partial-window rows, over 50%"
            for root, item in excluded.items()
            if rows_per_root.get(root) and item["rows"] > OVER_HALF * rows_per_root[root]
        ]
        return {
            "stack_residue_rows": sum(self.rows.values()),
            "stack_residue_partial_rows": sum(self.partial_rows.values()),
            "stack_residue_excluded": excluded,
            "stack_residue_health": health,
        }


def iter_file(
    path: Path, from_poll: int = 0, strict_poll: int | None = None
) -> Iterator[list[int]]:
    """Yield validated rows in a poll window without retaining the trace in memory."""
    with path.open() as handle:
        for number, line in enumerate(handle, 1):
            parts = line.split()
            if not parts:
                continue
            phase = int(parts[2], 10) if len(parts) > 2 else -1
            if not valid_row_fields(parts, phase):
                raise ValueError(f"invalid call-args row {number}: {len(parts)} fields")
            row = [int(parts[0]), int(parts[1], 16), phase]
            if row[0] < from_poll or (strict_poll is not None and row[0] > strict_poll):
                continue
            row.extend(int(part, 16) for part in parts[3:])
            yield row


def _field_names(phase: int) -> tuple[str, ...]:
    return (
        ("a0", "a1", "a2", "a3", "d0", "d1", "d2", "d3", *(f"s{i}" for i in range(8)))
        if phase == 0
        else ("eax", "st0", "d0", "d1", "d2", "d3", *(f"s{i}" for i in range(8)))
    )


def compare_files(
    left_path: Path,
    right_path: Path,
    *,
    from_poll: int = 0,
    strict_poll: int | None = None,
    roots: set[int] | None = None,
) -> dict:
    """Compare two external streams in lockstep with O(1) row memory."""

    def selected(path: Path) -> Iterator[list[int]]:
        for row in iter_file(path, from_poll, strict_poll):
            if roots is None or row[1] in roots:
                yield row

    left = selected(left_path)
    right = selected(right_path)
    left_entries, right_entries = EntryArgs(), EntryArgs()
    tally = ResidueTally()
    compared = diverging = missing = 0
    per_root: dict[str, dict[str, int]] = {}
    first = None
    sentinel = object()
    while True:
        a = next(left, sentinel)
        b = next(right, sentinel)
        if a is sentinel and b is sentinel:
            break
        row = a if a is not sentinel else b
        poll, root, phase = row[:3]
        root_name = f"0x{root:X}"
        stats = per_root.setdefault(root_name, {"rows": 0, "diverging": 0, "missing": 0})
        if a is sentinel or b is sentinel:
            missing += 1
            stats["missing"] += 1
            if first is None:
                first = {"poll": poll, "root": root_name, "args": "missing row"}
            continue
        compared += 1
        stats["rows"] += 1
        left_entries.note(a)
        right_entries.note(b)
        a_args, b_args = left_entries.args_of(a), right_entries.args_of(b)
        a, a_stack, a_live = split_row(a)
        b, b_stack, b_live = split_row(b)
        key_a, key_b = a[:3], b[:3]
        names = _field_names(phase)
        left_fields, right_fields = a[3:], b[3:]
        changes = {}
        if key_a != key_b:
            changes["row"] = {"left": key_a, "right": key_b}
        if key_a[2] == key_b[2]:
            for index, (x, y) in enumerate(zip(left_fields, right_fields, strict=True)):
                if x != y:
                    name = names[index]
                    changes[name] = {"left": f"0x{x:X}", "right": f"0x{y:X}"}
            if changes and key_a == key_b:
                changed = [
                    i
                    for i, (x, y) in enumerate(zip(left_fields, right_fields, strict=True))
                    if x != y
                ]
                excused = residue_addresses(
                    phase, changed, a_stack, b_stack, a_args, b_args, a_live, b_live
                )
                if excused is not None:
                    tally.add(root_name, *excused)
                    changes = {}
        else:
            changes["values"] = {
                "left": [f"0x{x:X}" for x in left_fields],
                "right": [f"0x{x:X}" for x in right_fields],
            }
        if changes:
            diverging += 1
            stats["diverging"] += 1
            if first is None:
                first = {"poll": poll, "root": root_name, "args": changes}
    return {
        "equal": compared > 0 and diverging == 0 and missing == 0,
        "rows_compared": compared,
        "diverging_rows": diverging,
        "missing_rows": missing,
        "first_divergence": first,
        "per_root": per_root,
        **tally.summary({root: item["rows"] for root, item in per_root.items()}),
        "poll_window": {"from": from_poll, "through": strict_poll},
        "memory": "streaming, one row per side",
    }


def load(path: Path) -> list[list[int]]:
    rows = []
    for number, line in enumerate(path.read_text().splitlines()):
        parts = line.split()
        if not parts:
            continue
        phase = int(parts[2], 10) if len(parts) > 2 else -1
        if not valid_row_fields(parts, phase):
            raise ValueError(f"invalid call-args row {number}: {len(parts)} fields")
        rows.append(
            [int(parts[0]), int(parts[1], 16), phase] + [int(part, 16) for part in parts[3:]]
        )
    return rows


EMBED_LIMIT_BYTES = 256 * 1024 * 1024
"""T3388: a call-args trace above this stays a separate file; embedding story-level1's 4 GB
(28.6 M rows) in trace.json needed tens of GB of Python objects and OOM-killed the host."""


def scan(path: Path) -> dict:
    """Stream-validate a call-args trace without keeping rows: row and malformed-row counts."""
    rows = malformed = 0
    with path.open() as handle:
        for line in handle:
            parts = line.split()
            if not parts:
                continue
            phase = int(parts[2], 10) if len(parts) > 2 and parts[2].isdigit() else -1
            if valid_row_fields(parts, phase):
                rows += 1
            else:
                malformed += 1
    return {"rows": rows, "malformed": malformed}


def keyed(
    rows: list[list[int]],
    limit: int,
    stacks: dict[
        Key,
        tuple[tuple[int, int, int], tuple[int, ...] | None, dict[int, int]],
    ]
    | None = None,
) -> dict[Key, tuple[int, ...]]:
    """Key rows by (poll, va, phase, ordinal); classic fields only.

    When `stacks` is given it receives stack bounds, entry arguments, and live-suffix digests.
    """
    counts: dict[tuple[int, int, int], int] = {}
    result: dict[Key, tuple[int, ...]] = {}
    entries = EntryArgs()
    for row in rows:
        if row[0] > limit:
            continue
        entries.note(row)
        base, stack, live_suffixes = split_row(row)
        poll, va, phase, *fields = base
        group = (poll, va, phase)
        ordinal = counts.get(group, 0)
        counts[group] = ordinal + 1
        result[(*group, ordinal)] = tuple(fields)
        if stacks is not None and stack is not None:
            stacks[(*group, ordinal)] = (stack, entries.args_of(row), live_suffixes)
    return result


def compare(
    left: list[list[int]],
    right: list[list[int]],
    controls: list[list[list[int]]],
    limit: int,
    expected_vas: list[int] | None = None,
) -> dict:
    a_stacks: dict[Key, tuple[tuple[int, int, int], tuple[int, ...] | None, dict[int, int]]] = {}
    b_stacks: dict[Key, tuple[tuple[int, int, int], tuple[int, ...] | None, dict[int, int]]] = {}
    a, b = keyed(left, limit, a_stacks), keyed(right, limit, b_stacks)
    masks: dict[Key, set[int]] = {}
    unstable: set[Key] = set()
    for control in controls:
        c = keyed(control, limit)
        unstable |= a.keys() ^ c.keys()
        for key in a.keys() & c.keys():
            noisy = {i for i, (x, y) in enumerate(zip(a[key], c[key], strict=True)) if x != y}
            if noisy:
                masks.setdefault(key, set()).update(noisy)
    common = (a.keys() & b.keys()) - unstable
    tally = ResidueTally()
    differences = []
    for key in sorted(common):
        mask = masks.get(key, set())
        fields = [
            i
            for i, (x, y) in enumerate(zip(a[key], b[key], strict=True))
            if x != y and i not in mask
        ]
        if fields:
            left_info, right_info = a_stacks.get(key), b_stacks.get(key)
            excused = (
                residue_addresses(
                    key[2],
                    fields,
                    left_info[0] if left_info else None,
                    right_info[0] if right_info else None,
                    left_info[1] if left_info else None,
                    right_info[1] if right_info else None,
                    left_info[2] if left_info else {},
                    right_info[2] if right_info else {},
                )
                if left_info and right_info
                else None
            )
            if excused is not None:
                tally.add(f"0x{key[1]:X}", *excused)
                continue
            differences.append((key, fields))
    missing = sorted((a.keys() ^ b.keys()) - unstable)
    expected = (
        set(expected_vas)
        if expected_vas is not None
        else {row[1] for row in left if row[0] <= limit}
    )
    covered = {key[1] for key in common}
    missing_roots = sorted(expected - covered)
    per_va: dict[str, dict[str, int]] = {}
    for key in sorted(common):
        per_va.setdefault(f"0x{key[1]:X}", {"rows": 0, "diverging": 0})["rows"] += 1
    for key, _ in differences:
        per_va[f"0x{key[1]:X}"]["diverging"] += 1
    for key in missing:
        per_va.setdefault(f"0x{key[1]:X}", {"rows": 0, "diverging": 0})["diverging"] += 1
    equal = bool(common) and not differences and not missing and not missing_roots
    excluded_rows = sum(tally.rows.values())
    return {
        "equal": equal,
        "rows_compared": len(common),
        "masked_fields": sum(len(m) for m in masks.values()),
        "unstable_keys": len(unstable),
        "missing_rows": len(missing),
        "missing_roots": [f"0x{va:X}" for va in missing_roots],
        "diverging_rows": len(differences),
        "first_divergence": (
            {"key": list(differences[0][0]), "fields": differences[0][1]}
            if differences
            else ({"key": list(missing[0]), "fields": "missing"} if missing else None)
        ),
        "per_va": per_va,
        **tally.summary({va: item["rows"] for va, item in per_va.items()}),
        "evidence": (
            "Tier 2 call-argument stream (entry args, pointer and input-state digests, "
            "lane exit EAX/ST0)"
            + (
                f"; EQUAL with {excluded_rows} stack-residue rows excluded"
                + (
                    f" ({sum(tally.partial_rows.values())} partial-window rows)"
                    if tally.partial_rows
                    else ""
                )
                if equal and excluded_rows
                else ""
            )
        ),
    }
