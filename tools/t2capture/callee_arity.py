# SPDX-License-Identifier: GPL-3.0-or-later
"""Callee stack arity for the capture-based Tier 2 compare (compare v8, T3738).

The capture records a fixed 16-word argument window for every non-kernel callee. Words at or
above the callee's stack arity are not arguments: they are the caller's own spills and locals
(a readable port does not spill, so they hold 0xCDCDCDCD poison there). Compare v8 compares only
the words below the callee's arity.

`python3 -m tools.t2capture.callee_arity --write --lift <tmp/private-host/lift-...>` derives the
arity of every lifted function from its lifted body and writes
`generated/t2capture/callee-arity.json`
(lift-derived, untracked, keyed by the lift tree name, like `fpo_leaf`). Only functions whose
arity is PROVEN by the analysis below are listed; any other callee is compared on all 16 words
(strict, fail closed).

The analysis (`analyse`) follows ESP through the body in text order (the lift is address ordered):
pushes, pops, `esp = esp +/- K`, `ebp = esp` frames, calls (+4 for the return slot plus the callee's
own `ret N` cleanup, resolved through the table), and records every stack-relative access. The
arity is one past the highest argument word read (`MEM*`/`SMEM*` at entry ESP + 4 + 4i or above,
size-aware). It is UNKNOWN (strict) when any of these hold:
- a stack access or a frame-pointer use happens while the ESP offset is unknown (after an indirect
  call or a callee whose cleanup is unknown, at a label only reached by a jump table or a later
  backward branch, or where two paths disagree);
- the address of an argument slot escapes (`reg = esp + K` reaching the argument area);
- the body has no `ret`, or tail-calls another function (`sub_X(); return;`, `RECOMP_ITAIL`,
  `RECOMP_TAIL`) without a stdcall `ret N` (whose N / 4 is then the arity); an HLE thunk body
  takes `ret N` from the lift's abi stage (`abi/abi_functions.json`, "ret cleaned N bytes");
- ESP or EBP is assigned in any form not listed above, or EBP is used as a base while it still
  holds the caller's value (`ebp = g_ebp` / `g_seh_ebp`);
- `ret N` cleanups disagree, or the body reads above its own stdcall cleanup.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
TABLE_PATH = Path("generated/t2capture/callee-arity.json")
WINDOW_WORDS = 16  # src/host/t2_capture.c T2_MAX_ARGS: the captured argument window

# The lifted analyzer intentionally treats indirect or unresolved tail paths as unknown. These
# these callees have a narrower carrier-proven contract for the capture argument window:
# 0x431F0 reads exactly its first three arguments before tail-dispatching (carrier 0x431F0),
# 0x227B50 reads no stack arguments (carrier 0x227B50), and 0x1CEB00 reads only argument 0
# (carrier 0x1CEB00). Scope the evidence to the receipt lift tree; another lift stays strict
# until independently checked.
_KNOWN_ARITY_OVERRIDES = {
    # T2 UI/menu: carrier/lift-derived caller-spill windows for this exact tree.
    # Carrier 0x19d70 reads entry [esp+8] (2 words); 0x1cd90 reads through
    # entry [esp+0x10] (4 words). 0x41dc0 reads through entry [esp+0x20]
    # (9 words). Lifted entry-relative reads prove the other three windows.
    "lift-9dfafd68e668d4d0": {
        0x00019650: 1,  # lifted sub_00019650 reads entry [esp+4] after PUSH ESI.
        0x00028180: 5,  # lift reads through entry [esp+0x14]; tail 0x3c848c takes no stack args.
        0x00407B14: 2,  # carrier jumps to 0x4078e5: two stack arguments, ret 8.
        0x00019D70: 2,  # carrier 0x00019d71 reads [esp+8] after PUSH ESI.
        0x0001ED50: 1,  # thunk to carrier 0x0001c720, which reads entry [esp+4].
        0x002E5F20: 6,  # carrier frame reads highest incoming word at entry [esp+0x14].
        0x002E5D70: 1,  # carrier pushes ECX/EBX then reads entry [esp+4].
        0x0001CD90: 4,  # carrier max entry-relative read: [esp+0x18] after 3 pushes.
        0x0002BF2C0: 3,  # lifted sub_002BF2C0 max entry-relative read is +0xC.
        0x00041DC0: 9,  # carrier max entry-relative read: [esp+0x68] after sub esp,0x48.
        0x003C848C: 0,  # carrier consumes ST0; no entry stack args.
    },
    "lift-c2a28a25ffab814c": {
        0x000431F0: 3,
        0x00227B50: 0,
        0x001CEB00: 1,
        0x0006B2B0: 1,
    },
}

_FUNCTION = re.compile(r"^(?:static )?void sub_([0-9A-F]{8})\(void\)$")
_LABEL = re.compile(r"^(loc_[0-9A-F]{8}):")
_NUM = r"(0x[0-9A-Fa-f]+|\d+)u?"
_ACCESS = re.compile(r"\b(S?MEM(?:8|16|32|64|F|D)?)\((esp|ebp)(?: (\+|-) (-?)" + _NUM + r")?\)")
_SIZE = {"8": 1, "16": 2, "32": 4, "F": 4, "64": 8, "D": 8, "": 8}
_LEA = re.compile(r"^\s*(?:e[a-d]x|e[sd]i|ebp) = (esp|ebp)(?: (\+|-) (-?)" + _NUM + r")?;")
_ESP_STEP = re.compile(r"\besp = esp (\+|-) " + _NUM + r";|\besp (\+|-)= " + _NUM + r";")
_RET = re.compile(r"\besp \+= (?:4 \+ )?" + _NUM + r"; return;")
_CALL = re.compile(r"RECOMP_ABI_CALL\(0x([0-9A-Fa-f]+)u?, sub_[0-9A-F]{8}\)")
_ICALL = re.compile(r"RECOMP_ICALL")
_TAIL = re.compile(r"\bsub_([0-9A-F]{8})\(\); return;|RECOMP_ITAIL|RECOMP_TAIL|xdk_thunk_dispatch")
_GOTO = re.compile(r"\bgoto (loc_[0-9A-F]{8});")
_UNCONDITIONAL = re.compile(r"^\s*goto loc_[0-9A-F]{8};|\breturn;")
# Flag computations only read ESP or EBP as a value; they never address memory.
_FLAG_USE = re.compile(r"\(uint(?:32|64)_t\)\((esp|ebp)\)")


class Unknown(Exception):
    """The arity cannot be proven (strict)."""


def _int(text: str) -> int:
    return int(text, 0)


def _offset(sign: str | None, neg: str | None, number: str | None) -> int:
    if number is None:
        return 0
    value = _int(number)
    if neg:
        value = -value
    return -value if sign == "-" else value


def analyse(body: list[str], pops: dict[int, int]) -> tuple[int, int]:
    """(argument words, own `ret N` cleanup bytes). Raises Unknown.

    `pops` maps a callee VA to its proven cleanup bytes (the `ret N` immediate).
    """
    delta: int | None = 0  # ESP - entry ESP (bytes); None = unknown
    frame: int | None = None  # EBP = entry ESP + frame when set by `ebp = esp`
    ebp_state = "caller"  # caller | frame | general
    labels: dict[str, int | None] = {}
    words = 0
    cleanups: set[int] = set()
    tail = False

    def read(offset: int, size: int) -> None:
        nonlocal words
        top = offset + size - 1  # last byte read, relative to entry ESP
        if top >= 4:
            words = max(words, (top - 4) // 4 + 1)

    for raw in body:
        code = raw.split("/*")[0]
        label = _LABEL.match(code.strip())
        if label:
            name = label.group(1)
            if delta is None:
                delta = labels.get(name)
            elif name in labels and labels[name] != delta:
                raise Unknown(f"{name}: stack offset {labels[name]} vs {delta}")
            labels[name] = delta
            continue
        if not re.search(r"\b(esp|ebp)\b|RECOMP_|sub_[0-9A-F]{8}\(|goto |return;|xdk_thunk", code):
            continue
        if _TAIL.search(code):
            tail = True
        ret = _RET.search(code)
        if ret:
            cleanups.add(
                _int(ret.group(1)) - 4 if "4 + " not in ret.group(0) else _int(ret.group(1))
            )
            delta = None
            continue
        # Stack accesses, before this line's ESP update (PUSH32 evaluates its value first).
        for match in _ACCESS.finditer(code):
            kind, base, sign, neg, number = match.groups()
            size = _SIZE[re.sub(r"^S?MEM", "", kind)]
            offset = _offset(sign, neg, number)
            if base == "esp":
                if delta is None:
                    raise Unknown("stack access at an unknown offset")
                read(delta + offset, size)
            elif ebp_state == "frame":
                if frame is None:
                    raise Unknown("frame access without a frame")
                read(frame + offset, size)
            elif ebp_state == "caller":
                raise Unknown("EBP-relative access through the caller's EBP")
        stripped = code.strip()
        lea = _LEA.match(code)
        if lea and not stripped.startswith("ebp = esp;"):
            base, sign, neg, number = lea.groups()
            offset = _offset(sign, neg, number)
            if base == "esp":
                if delta is None or delta + offset >= 4:
                    raise Unknown("a stack address near the argument area escapes")
            elif ebp_state == "frame":
                if frame is None or frame + offset >= 4:
                    raise Unknown("a frame address near the argument area escapes")
        # ESP and EBP updates.
        rest = _FLAG_USE.sub("", code)
        rest = _ACCESS.sub("", rest)
        handled = False
        for _ in re.finditer(r"PUSH32\(esp,", rest):
            delta = None if delta is None else delta - 4
            handled = True
        for _ in re.finditer(r"POP32\(esp,", rest):
            delta = None if delta is None else delta + 4
            handled = True
        if re.search(r"POP32\(esp, ebp\)", rest):
            ebp_state = "general"
        for step in _ESP_STEP.finditer(rest):
            sign = step.group(1) or step.group(3)
            amount = _int(step.group(2) or step.group(4))
            if delta is not None:
                delta += amount if sign == "+" else -amount
            handled = True
        if re.search(r"\bebp = esp;", rest):
            frame, ebp_state, handled = delta, "frame", True
        elif re.search(r"\besp = ebp;", rest):
            if ebp_state != "frame":
                raise Unknown("esp = ebp without a frame")
            delta, handled = frame, True
        elif re.search(r"\bebp = g_(?:seh_)?ebp;|uint32_t ebp = ", rest):
            ebp_state, handled = "caller", True
        elif re.search(r"(?<![.\w])ebp = ", rest):
            ebp_state, handled = "general", True
        call = _CALL.search(rest)
        if call:
            callee = int(call.group(1), 16)
            if callee not in pops or delta is None:
                delta = None
            else:
                delta += 4 + pops[callee]
            handled = True
        elif _ICALL.search(rest):
            delta, handled = None, True
        if re.search(r"(?<![.\w])esp (?:=|\+=|-=)", rest) and not handled:
            raise Unknown(f"unmodelled ESP update: {stripped[:60]}")
        for goto in _GOTO.finditer(rest):
            target = goto.group(1)
            if target in labels and delta is not None and labels[target] not in (None, delta):
                raise Unknown(f"{target}: stack offset {labels[target]} vs {delta}")
            if delta is not None:
                labels.setdefault(target, delta)
        if _UNCONDITIONAL.search(rest):
            delta = None
    if not cleanups:
        raise Unknown("no ret")
    if len(cleanups) > 1:
        raise Unknown(f"ret cleanups disagree: {sorted(cleanups)}")
    own = next(iter(cleanups))
    if tail and not own:
        raise Unknown("tail call or thunk")  # the target reads the caller's arguments too
    if own:  # stdcall: `ret N` pops exactly the arguments (cdecl `ret` keeps the reads)
        if own % 4:
            raise Unknown(f"odd ret cleanup {own}")
        if words > own // 4:
            raise Unknown("reads above its stdcall cleanup")
        words = own // 4
    return words, own


def bodies(gen: Path) -> dict[int, list[str]]:
    """Every `sub_X(void)` body (first definition) of the lift, as lines."""
    found: dict[int, list[str]] = {}
    for path in sorted(gen.glob("recomp_*.c")):
        lines = path.read_text(errors="replace").splitlines()
        index = 0
        while index < len(lines):
            match = _FUNCTION.match(lines[index])
            if not match:
                index += 1
                continue
            end = index + 1
            while end < len(lines) and lines[end] != "}":
                end += 1
            found.setdefault(int(match.group(1), 16), lines[index + 1 : end])
            index = end + 1
    return found


def thunk_cleanups(lift: Path) -> dict[int, int]:
    """`ret N` of HLE thunk bodies from the lift's abi stage (`abi/abi_functions.json`)."""
    path = lift / "abi" / "abi_functions.json"
    if not path.exists():
        print(f"callee_arity: no {path}, thunks stay strict", file=sys.stderr)
        return {}
    out: dict[int, int] = {}
    for row in json.loads(path.read_text()):
        note = re.search(r"ret cleaned (\d+) bytes", row.get("heuristic_notes", ""))
        if note:
            out[int(row["address"], 16)] = int(note.group(1))
    return out


def derive(
    gen: Path, thunks: dict[int, int] | None = None
) -> tuple[dict[int, int], dict[str, int]]:
    """Proven arity per VA and unknown-reason counts; iterates until callee cleanups settle."""
    found = bodies(gen)
    thunks = thunks or {}
    pops: dict[int, int] = {}
    arity: dict[int, int] = {}
    reasons: dict[str, int] = {}
    for _ in range(8):
        reasons = {}
        changed = False
        for va, body in found.items():
            if any("xdk_thunk_dispatch" in line for line in body) and va in thunks:
                words, own = thunks[va] // 4, thunks[va]
            else:
                try:
                    words, own = analyse(body, pops)
                except Unknown as reason:
                    key = re.sub(r"0x[0-9A-Fa-f]+|-?\d+|loc_\w+", "N", str(reason))[:40]
                    reasons[key] = reasons.get(key, 0) + 1
                    continue
            if pops.get(va) != own or arity.get(va) != words:
                pops[va], arity[va] = own, words
                changed = True
        if not changed:
            break
    return arity, reasons


def table_path() -> Path:
    local = ROOT / TABLE_PATH
    if local.exists():
        return local
    from tools.trace_store import main_root

    return main_root() / TABLE_PATH


def load(tree: str, path: Path | None = None) -> dict[int, int]:
    """VA -> proven arity for lift tree `tree`; empty (strict) when missing or for another tree."""
    target = path or table_path()
    if not target.exists():
        print(f"callee_arity: no table at {target}, callee arguments strict", file=sys.stderr)
        return {}
    table = json.loads(target.read_text())
    if table.get("lift_tree") != tree:
        print(
            f"callee_arity: table is for {table.get('lift_tree')!r}, not {tree!r}: strict",
            file=sys.stderr,
        )
        return {}
    result = {int(va, 16): int(words) for va, words in table.get("arity", {}).items()}
    result.update(_KNOWN_ARITY_OVERRIDES.get(tree, {}))
    return result


def flag_text(table: dict[int, int]) -> str:
    """The `--callee-arity` file body: one `<hex va> <words>` line per proven callee."""
    return "".join(f"{va:x} {words}\n" for va, words in sorted(table.items()))


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        prog="python3 -m tools.t2capture.callee_arity", description=__doc__
    )
    parser.add_argument("--lift", type=Path, required=True, help="lift dir (holds gen/ and abi/)")
    parser.add_argument("--write", action="store_true", help=f"write {TABLE_PATH} (main checkout)")
    parser.add_argument("--show", type=lambda text: int(text, 16), action="append", default=[])
    args = parser.parse_args(argv)
    gen = args.lift / "gen"
    if not gen.is_dir():
        print(f"callee_arity: no {gen}", file=sys.stderr)
        return 2
    arity, reasons = derive(gen, thunk_cleanups(args.lift))
    for va in args.show:
        print(f"0x{va:08X} {arity.get(va, 'unknown')}")
    table = {
        "schema": 1,
        "task": "T3738",
        "lift_tree": args.lift.name,
        "arity": {f"0x{va:08X}": words for va, words in sorted(arity.items())},
    }
    if args.write:
        from tools.trace_store import main_root

        target = main_root() / TABLE_PATH
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(json.dumps(table, indent=1) + "\n")
        print(f"wrote {target}")
    print(f"lift_tree={args.lift.name} proven={len(arity)} unknown={sum(reasons.values())}")
    for reason, count in sorted(reasons.items(), key=lambda item: -item[1])[:10]:
        print(f"  {count} {reason}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
