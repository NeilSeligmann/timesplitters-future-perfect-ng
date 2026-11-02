# SPDX-License-Identifier: GPL-3.0-or-later
"""Return-register liveness per function for the capture-based Tier 2 compare (exit_regs rule v3).

`python3 -m tools.t2capture.ret_liveness --write [--va 0x..]` scans the lifter's disassembly
(`generated/lifted/disasm/asm/text.asm`, the code the lifted C is generated from) and records, per
function, whether EAX, EDX and EBP may be read by any caller after the function returns. The table
is
`generated/t2capture/ret-liveness.json` (derived from the XBE, so untracked like the rest of
`generated/`; read from this checkout, else the main checkout); the runner passes the dead
registers to the offline host (`--ret-dead`) and copies the entry into the receipt as `ret_live`.

The analysis is a may-liveness over-approximation, so "dead" is the only claim it makes:
- a register is live after a call site when any path from the return address reads it (any
  explicit mention other than as the destination of a pure write, or an implicit read such as
  `cdq`, `div`, `stos`, `pushal`) before every path overwrites it;
- `ret` in the caller: live when the caller's own return register is live (fixpoint);
- a direct call on the path: live when the callee may read the register at entry, otherwise the
  path continues after it (pass-through, conservative);
- a jump to another function's start (tail call): the target's entry liveness OR the caller's;
- an indirect jump, an indirect call, or a branch out of the known code: live (unknown);
- a function whose address appears as a 32-bit value anywhere in the image (vtable, callback,
  `push offset`) has callers this cannot see: both registers live.
Unknown functions are absent from the table and the compare stays strict for them (fail closed).
"""

from __future__ import annotations

import argparse
import csv
import json
import re
import struct
import sys
from collections.abc import Callable
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
TABLE_PATH = Path("generated/t2capture/ret-liveness.json")
ASM = Path("generated/lifted/disasm/asm/text.asm")
FUNCTIONS = Path("generated/lifted/disasm/functions.json")
XBE = Path("build/default.xbe")
SWITCHES = Path("generated/jumptables/tables.csv")
REGS = ("eax", "edx", "ebp")
ALIASES = {
    "eax": re.compile(r"\b(eax|ax|al|ah)\b"),
    "edx": re.compile(r"\b(edx|dx|dl|dh)\b"),
    "ebp": re.compile(r"\b(ebp|bp)\b"),
}
LINE = re.compile(r"^\s+0x([0-9A-F]{8})\s+([0-9a-f]+)\s+(\S+)(?:\s+(.*?))?\s*(?:;.*)?$")
FUNC = re.compile(r"^; Start: 0x([0-9A-F]{8})\s+End: 0x([0-9A-F]{8})")
PURE_WRITES = frozenset({"mov", "movzx", "movsx", "lea", "pop", "movd"})
IMPLICIT_READS = {
    "eax": frozenset(
        {
            "cdq",
            "cwde",
            "cbw",
            "cwd",
            "div",
            "idiv",
            "sahf",
            "xlatb",
            "out",
            "cmpxchg",
            "pushal",
            "pushad",
            "cpuid",
            "int",
            "aaa",
            "aas",
            "aam",
            "aad",
            "daa",
            "das",
            "stosb",
            "stosw",
            "stosd",
            "scasb",
            "scasw",
            "scasd",
            "cmpxchg8b",
            "syscall",
            "sysenter",
        }
    ),  # fmt: skip
    "edx": frozenset(
        {
            "div",
            "idiv",
            "pushal",
            "pushad",
            "in",
            "insb",
            "insd",
            "out",
            "outsb",
            "outsd",
            "int",
            "cmpxchg8b",
            "wrmsr",
            "syscall",
            "sysenter",
        }
    ),  # fmt: skip
    "ebp": frozenset({"leave", "enter", "pushal", "pushad", "pusha", "int", "syscall", "sysenter"}),
}
ENDS = frozenset({"hlt", "ud2", "int3"})
# A walk step: (address, "insn" | "call" | "ret" | "unknown", target or lanes read, lanes
# still holding the tracked value) -> True (live), False (go on) or "end" (path ends).
Step = Callable[[int, str, int | None, int], bool | str]


def _mnemonic(text: str) -> str:
    return text.split()[-1] if text.startswith(("rep", "lock")) else text


def _operands(ops: str) -> list[str]:
    depth, cur, out = 0, "", []
    for ch in ops:
        if ch == "[":
            depth += 1
        elif ch == "]":
            depth -= 1
        if ch == "," and depth == 0:
            out.append(cur.strip())
            cur = ""
        else:
            cur += ch
    if cur.strip():
        out.append(cur.strip())
    return out


LANES = {"e": 7, "x": 3, "l": 1, "h": 2}  # al/dl = 1, ah/dh = 2, upper 16 bits = 4
FULL = 7
SAME_REG_IDIOMS = frozenset({"xor", "sub", "sbb"})


def lanes(text: str, reg: str) -> int:
    """Byte lanes of `reg` named anywhere in an operand."""
    mask = 0
    for token in ALIASES[reg].findall(text):
        mask |= LANES["e"] if token.startswith("e") else LANES["x" if token == "bp" else token[1]]
    return mask


def _immediate(text: str) -> int | None:
    try:
        return int(text, 0) & 0xFFFF_FFFF
    except ValueError:
        return None


def _imm_lanes(imm: int) -> int:
    return (1 if imm & 0xFF else 0) | (2 if imm & 0xFF00 else 0) | (4 if imm & 0xFFFF_0000 else 0)


def effect(mnem: str, ops: str, reg: str) -> tuple[int, int]:
    """(lanes read, lanes overwritten) of `reg` by one non-branch instruction."""
    base = _mnemonic(mnem)
    operands = _operands(ops or "")
    if reg == "ebp":
        if base in IMPLICIT_READS[reg]:
            return FULL, 0
        if base in ("popal", "popad"):
            return 0, FULL
        if base in ("cdq", "rdtsc", "cpuid"):
            return 0, 0
        if base in ("mul", "imul", "div", "idiv") and len(operands) == 1:
            return lanes(operands[0], reg), 0
    if base == "cdq":
        return (FULL, 0) if reg == "eax" else (0, FULL)
    if base in ("mul", "imul") and len(operands) == 1:
        return (FULL if reg == "eax" else lanes(operands[0], reg), FULL)
    if base in ("div", "idiv"):
        return FULL, FULL
    if base in ("rdtsc", "popal", "popad"):
        return 0, FULL
    if base == "cpuid":
        return (FULL if reg == "eax" else 0), FULL
    if reg == "eax":
        if base in ("fnstsw", "fstsw") and operands == ["ax"]:
            return 0, LANES["x"]
        if base == "lahf":
            return 0, LANES["h"]
        if base == "sahf":
            return LANES["h"], 0
        if base in ("lodsb", "lodsw", "lodsd"):
            return 0, {"b": 1, "w": 3, "d": 7}[base[-1]]
    if base in IMPLICIT_READS[reg]:
        return FULL, 0
    if not operands:
        return 0, 0
    dest, sources = operands[0], operands[1:]
    source_read = 0
    for op in sources:
        source_read |= lanes(op, reg)
    dest_lanes = lanes(dest, reg)
    dest_is_reg = ALIASES[reg].fullmatch(dest) is not None
    if not dest_is_reg:  # memory or another register: any mention is a read (address or value)
        return dest_lanes | source_read, 0
    if base in SAME_REG_IDIOMS and sources == [dest]:
        return 0, dest_lanes  # xor eax, eax: dependency-breaking zero
    if len(sources) == 1 and (
        (base == "or" and sources[0] in ("0xffffffff", "-1"))
        or (base == "and" and sources[0] == "0")
    ):
        return 0, dest_lanes  # or eax, -1 / and eax, 0: the result ignores the old value
    imm = _immediate(sources[0]) if len(sources) == 1 else None
    if base in ("and", "test") and imm is not None and dest == reg:
        # Only the lanes the mask keeps are read; `and` zeroes the others.
        kept = _imm_lanes(imm)
        return kept, (FULL & ~kept if base == "and" else 0)
    if base in PURE_WRITES or base.startswith("set") or (base == "imul" and len(operands) == 3):
        return source_read, dest_lanes
    return dest_lanes | source_read, 0  # read-modify-write, compare, test, xchg


class Program:
    def __init__(
        self, asm_text: str, noreturn: set[int], switches: dict[int, list[int]] | None = None
    ) -> None:
        self.switches = switches or {}
        self.insns: dict[int, tuple[str, str]] = {}
        self.next: dict[int, int] = {}
        self.body: dict[int, set[int]] = {}
        self.starts: list[int] = []
        self.noreturn = noreturn
        current = None
        for line in asm_text.splitlines():
            head = FUNC.match(line)
            if head:
                current = int(head.group(1), 16)
                self.starts.append(current)
                continue
            if line.startswith("; end of function"):
                current = None
                continue
            match = LINE.match(line)
            if not match or current is None:
                continue
            addr = int(match.group(1), 16)
            self.insns[addr] = (match.group(3), (match.group(4) or "").strip())
            self.body.setdefault(current, set()).add(
                addr
            )  # chunks can be listed under two functions
            self.next[addr] = (
                addr + len(match.group(2)) // 2
            )  # fall-through, also into the next function
        self.start_set = set(self.starts)
        self.edx_arg_targets = self._edx_arg_targets()
        self.kills: dict[tuple[int, str], bool] = {}
        self.indirect_live = {reg: False for reg in REGS}
        self.entry: dict[tuple[int, str], bool] = {}
        self.why = 0  # last instruction that made a register live (diagnostics)
        self.callers: dict[int, list[tuple[int, int]]] = {s: [] for s in self.starts}
        self.tail_jumps: dict[int, list[int]] = {s: [] for s in self.starts}
        for func, body in self.body.items():
            for addr in body:
                mnem, ops = self.insns[addr]
                target = _direct(ops)
                if target is None or target not in self.start_set:
                    continue
                if mnem == "call":
                    self.callers[target].append((func, self.next[addr]))
                elif mnem == "jmp" and target != func:
                    self.tail_jumps[target].append(func)

    def _edx_arg_targets(self, window: int = 12) -> set[int]:
        """Direct callees outside the listed code that some call site passes EDX to: EDX fully
        written in the straight-line code before the call and not read since (a fastcall arg)."""
        order = sorted(self.insns)
        position = {addr: i for i, addr in enumerate(order)}
        found: set[int] = set()
        for addr in order:
            mnem, ops = self.insns[addr]
            target = _direct(ops)
            if mnem != "call" or target is None or target in self.insns or target in found:
                continue
            i = position[addr]
            for back in range(1, window + 1):
                if i < back:
                    break
                prev_mnem, prev_ops = self.insns[order[i - back]]
                prev_base = _mnemonic(prev_mnem)
                if prev_base.startswith(("j", "ret", "call", "loop")):
                    break
                read, written = effect(prev_mnem, prev_ops, "edx")
                if read:
                    break
                if written:
                    found.add(target)
                    break
        return found

    def _walk(self, addr: int, reg: str, func: int, step: Step) -> bool:
        """Shared path walk: `step(at, kind, target, cur)` decides calls, jumps and returns.

        Returns True as soon as a step says live, False when every path ended."""
        seen: set[tuple[int, int]] = set()
        todo = [(addr, FULL)]
        while todo:
            at, cur = todo.pop()
            while True:
                if (at, cur) in seen:
                    break
                seen.add((at, cur))
                # Code is followed wherever it leads (a fall-through or jump into another listed
                # function is a tail continuation: its `ret` still returns to `func`'s caller).
                insn = self.insns.get(at)
                if insn is None:
                    if step(at, "unknown", None, cur):
                        self.why = at
                        return True
                    break
                mnem, ops = insn
                base = _mnemonic(mnem)
                if base in ENDS:
                    break
                if base.startswith("ret") or base == "iretd":
                    if step(at, "ret", None, cur):
                        self.why = at
                        return True
                    break
                target = _direct(ops)
                if base == "call":
                    verdict = step(at, "call", target, cur)
                    if verdict is True:
                        self.why = at
                        return True
                    if verdict == "end":
                        break
                elif base == "jmp":
                    if target is None and at in self.switches:
                        todo.extend((t, cur) for t in self.switches[at])
                        break
                    if target is None:
                        # Indirect tail call (`jmp dword ptr [eax + 0x68]`): a standard-convention
                        # callee, the value flows on to `func`'s return like at a `ret`.
                        if step(at, "ret", None, cur):
                            self.why = at
                            return True
                        break
                    at = target
                    continue
                elif base.startswith(("j", "loop")):
                    if target is None:
                        if step(at, "unknown", None, cur):
                            self.why = at
                            return True
                        break
                    todo.append((target, cur))
                else:
                    read, written = effect(mnem, ops, reg)
                    if step(at, "insn", read, cur):
                        self.why = at
                        return True
                    cur &= ~written
                    if cur == 0:
                        break
                at = self.next[at]
        return False

    def live_from(self, addr: int, reg: str, func: int, entry: dict, ret: dict | None) -> bool:
        """May the value `reg` holds at `addr` be read on some path inside `func` or beyond?

        `ret=None` is entry mode: a value that reaches `ret` unread is not a read of this function
        (the call site keeps scanning after the call)."""

        def step(at: int, kind: str, target: int | None, cur: int) -> bool | str:
            if kind == "insn":
                return bool(target & cur)
            if kind == "ret":
                return ret is not None and ret[(func, reg)]
            if kind == "call":
                if target is None:
                    # Indirect call (vtable, import thunk): its callee uses a standard MSVC
                    # convention (no EAX argument; EDX only for fastcall, which the 1804 indirect
                    # call sites never set up from a live value, see the design doc). Pass-through.
                    return False
                if target not in self.start_set and target not in self.insns:
                    # Outside the disassembled code (XDK library sections): separately compiled,
                    # standard conventions, so no EAX argument; EDX only for a fastcall target,
                    # which some call site sets EDX up for (`edx_arg_targets`).
                    return reg == "edx" and target in self.edx_arg_targets
                if target in self.start_set and entry[(target, reg)]:
                    return True
                if target in self.noreturn or self.kills.get((target, reg)):
                    return "end"
                return False
            return True  # left the known code

        return self._walk(addr, reg, func, step)

    def kills_from(self, addr: int, reg: str, func: int) -> bool:
        """Does every path from `addr` inside `func` overwrite all of `reg` before returning?"""

        def step(at: int, kind: str, target: int | None, cur: int) -> bool | str:
            if kind == "insn":
                return False
            if kind == "call":
                if target is not None and (
                    target in self.noreturn or self.kills.get((target, reg))
                ):
                    return "end"
                return False
            return True  # ret or unknown with the register not overwritten

        return not self._walk(addr, reg, func, step)

    def solve_kills(self) -> None:
        """Least fixpoint (a recursive cycle stays 'may not write', the conservative side)."""
        self.kills = {(s, r): False for s in self.starts for r in REGS}
        changed = True
        while changed:
            changed = False
            for s in self.starts:
                for r in REGS:
                    if not self.kills[(s, r)] and self.kills_from(s, r, s):
                        self.kills[(s, r)] = changed = True

    def solve(self, taken: set[int]) -> dict[tuple[int, str], bool]:
        self.solve_kills()
        entry = {(s, r): False for s in self.starts for r in REGS}
        ret = {(s, r): False for s in self.starts for r in REGS}
        # Callers of an address-taken function are the indirect call sites: its return register is
        # live when any indirect call site reads it afterwards (or that site is itself unknown).
        indirect = [
            (func, self.next[a])
            for func, body in self.body.items()
            for a in body
            if self.insns[a][0] == "call" and _direct(self.insns[a][1]) is None
        ]
        changed = True
        while changed:
            changed = False
            for r in REGS:
                if not self.indirect_live[r] and any(
                    self.live_from(after, r, caller, entry, ret) for caller, after in indirect
                ):
                    self.indirect_live[r] = changed = True
                for s in taken:
                    if self.indirect_live[r] and not ret.get((s, r), True):
                        ret[(s, r)] = changed = True
            for s in self.starts:
                for r in REGS:
                    if not entry[(s, r)] and self.live_from(s, r, s, entry, None):
                        entry[(s, r)] = changed = True
                    if not ret[(s, r)]:
                        live = any(
                            self.live_from(after, r, caller, entry, ret)
                            for caller, after in self.callers[s]
                        ) or any(ret[(g, r)] for g in self.tail_jumps[s])
                        if live:
                            ret[(s, r)] = changed = True
        self.entry = entry
        return ret


def _direct(ops: str) -> int | None:
    match = re.fullmatch(r"0x([0-9a-fA-F]+)", ops.strip())
    return int(match.group(1), 16) if match else None


def load_switches(path: Path) -> dict[int, list[int]]:
    """Resolved jump tables (tools.jumptables): indirect `jmp` VA -> case targets.

    Rows flagged `suspect` are skipped (their jumps stay unknown, so live)."""
    out: dict[int, list[int]] = {}
    if not path.exists():
        return out
    with path.open(newline="") as src:
        for row in csv.DictReader(src):
            if (row.get("suspect") or "").strip().lower() in ("1", "true", "yes"):
                continue
            targets = [int(t, 16) for t in row["targets"].split()]
            if targets:
                out[int(row["jump_va"], 16)] = targets
    return out


def address_taken(image: bytes, starts: list[int]) -> set[int]:
    wanted = set(starts)
    found: set[int] = set()
    for offset in range(len(image) - 3):
        (value,) = struct.unpack_from("<I", image, offset)
        if value in wanted:
            found.add(value)
    return found


def analyse(root: Path) -> dict[int, dict[str, bool]]:
    """Per function start: {'eax': live, 'edx': live} (True = some caller may read it)."""
    functions = json.loads((root / FUNCTIONS).read_text())
    noreturn = {int(f["start"], 16) for f in functions if f.get("noreturn")}
    switches = load_switches(root / SWITCHES)
    program = Program((root / ASM).read_text(errors="replace"), noreturn, switches)
    taken = address_taken((root / XBE).read_bytes(), program.starts)
    ret = program.solve(taken)
    return {start: {reg: ret[(start, reg)] for reg in REGS} for start in program.starts}


def build_table(root: Path) -> dict:
    """The tracked table: only the dead registers are listed (absent = live or unknown = strict)."""
    live = analyse(root)
    return {
        "schema": 1,
        "task": "T3738",
        "note": "exit_regs v3: functions whose return register no caller reads "
        "(python3 -m tools.t2capture.ret_liveness --write; absent = compared strictly)",
        "functions_analysed": len(live),
        "dead": {reg: [f"0x{va:08X}" for va in sorted(live) if not live[va][reg]] for reg in REGS},
    }


def table_path() -> Path:
    """This checkout's table, else the main checkout's (worktrees have no generated/)."""
    local = ROOT / TABLE_PATH
    if local.exists():
        return local
    from tools.trace_store import main_root

    return main_root() / TABLE_PATH


def load(path: Path | None = None) -> dict[str, frozenset[int]]:
    """Dead sets by register; empty (everything strict) when the table is missing."""
    target = path or table_path()
    if not target.exists():
        print(f"ret_liveness: no table at {target}, EAX/EDX/EBP compared strictly", file=sys.stderr)
        return {reg: frozenset() for reg in REGS}
    dead = json.loads(target.read_text()).get("dead", {})
    return {reg: frozenset(int(va, 16) for va in dead.get(reg, [])) for reg in REGS}


def ret_live(va: int, dead: dict[str, frozenset[int]]) -> dict[str, bool]:
    """Receipt field: True = compared (live or unknown), False = dead at every call site."""
    live = {reg: va not in dead.get(reg, frozenset()) for reg in REGS}
    live["ebp"] = caller_live_ebp(va, dead)
    return live


def caller_live_ebp(va: int, dead: dict[str, frozenset[int]]) -> bool:
    """Unknown or older tables keep EBP live and the comparison strict."""
    return va not in dead.get("ebp", frozenset())


def dead_regs(live: dict[str, bool]) -> list[str]:
    return [reg for reg in REGS if live.get(reg) is False]


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        prog="python3 -m tools.t2capture.ret_liveness", description=__doc__
    )
    parser.add_argument(
        "--source", type=Path, help="checkout holding generated/ and build/ (default: main)"
    )
    parser.add_argument("--write", action="store_true", help=f"write {TABLE_PATH}")
    parser.add_argument("--va", action="append", default=[], help="print one function's liveness")
    args = parser.parse_args(argv)
    source = args.source
    if source is None:
        from tools.trace_store import main_root

        source = ROOT if (ROOT / ASM).exists() else main_root()
    missing = [str(path) for path in (ASM, FUNCTIONS, XBE) if not (source / path).exists()]
    if missing:
        print(f"ret_liveness: missing inputs under {source}: {', '.join(missing)}", file=sys.stderr)
        return 2
    table = build_table(source)
    if args.write:
        target = source / TABLE_PATH
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(json.dumps(table, indent=1) + "\n")
        print(f"wrote {target}")
    dead = {reg: frozenset(int(va, 16) for va in table["dead"][reg]) for reg in REGS}
    for va in args.va:
        print(va, ret_live(int(va, 16), dead))
    counts = " ".join(f"{reg}_dead={len(table['dead'][reg])}" for reg in REGS)
    print(f"functions={table['functions_analysed']} {counts}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
