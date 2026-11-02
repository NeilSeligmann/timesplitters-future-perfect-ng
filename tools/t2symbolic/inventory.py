# SPDX-License-Identifier: GPL-3.0-or-later
"""T3871: inventory of never-executed port functions for recording-free verification.

Joins the T3793 reachability table (never-executed port functions), the port registry
(`docs/data/port-registry.json`, which crate module ports a VA) and the two bodies:

- the lifted C body (`void sub_<VA>(void)` in the lift's `gen/recomp_*.c`);
- the Rust port (`pub fn` after the `original 0x<VA>` doc line in the registry source).

A function is a **leaf** when neither body calls out: the lifted body calls no other lifted
function, runtime helper or kernel import (only the pure helpers in `PURE_C_HELPERS`), and the
Rust port takes no `GuestCall`. It is **loop-free** when the lifted body has no backward goto and
no `for`/`while`, and the Rust port has no `for`/`while`/`loop`. The pilot is the first `count`
eligible leaves ordered by sha256 of `T3871:<va>` (deterministic, spread across subsystems).
"""

from __future__ import annotations

import csv
import functools
import hashlib
import json
import re
from collections.abc import Callable
from dataclasses import asdict, dataclass, field
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
REGISTRY = Path("docs/data/port-registry.json")
REACH_CSV = Path("generated/t2reach/tier2_reachability.csv")
NAMES_CSV = Path("tools/data/function_names.csv")
SELECTION_SALT = "T3871:"

# C identifiers the lift uses as pure expressions (macros or libm), not calls out of the function.
PURE_C_HELPERS = frozenset(
    {
        "MEM8",
        "MEM16",
        "MEM32",
        "MEM64",
        "SMEM8",
        "SMEM16",
        "SMEM32",
        "SMEM64",
        "MEMF",
        "MEMD",
        "XBOX_PTR",
        "fp_push",
        "fp_pop",
        "fp_top",
        "fp_st",
        "fp_st1",
        "sizeof",
        "if",
        "for",
        "while",
        "switch",
        "return",
        "do",
    }
)
# Function-like macros of the lift's recomp_types.h that expand to a call out of the function.
CALL_MACROS = frozenset(
    {
        "RECOMP_TRACE_ENTER",
        "RECOMP_TRACE_EXIT",
        "RECOMP_TRACE_ESP",
        "RECOMP_CALL_SAFEPOINT",
        "RECOMP_ABI_CALL",
    }
)
# libm and libc calls the lift emits for x87/SSE instructions and string moves (pure, in a leaf).
LIBC_PURE = frozenset(
    {
        "sqrt",
        "sqrtf",
        "sin",
        "cos",
        "tan",
        "atan",
        "atan2",
        "floor",
        "ceil",
        "fabs",
        "fmod",
        "log",
        "exp",
        "pow",
        "log2",
        "exp2",
        "ldexp",
        "frexp",
        "round",
        "trunc",
        "rint",
        "nearbyint",
        "lrint",
        "memcpy",
        "memset",
        "memmove",
    }
)
TRANSCENDENTAL = frozenset(
    {"sin", "cos", "tan", "atan", "atan2", "log", "exp", "pow", "log2", "exp2"}
)
_FUNC_RE = re.compile(r"^void sub_([0-9A-F]{8})\(void\)\n\{\n(.*?)^\}\n", re.M | re.S)
_HEADER_RE = re.compile(r"/\*\*\n \* sub_([0-9A-F]{8})\n(.*?)\*/\n", re.S)
_CALL_RE = re.compile(r"\b([A-Za-z_][A-Za-z0-9_]*)\s*\(")
_LABEL_RE = re.compile(r"^loc_([0-9A-F]{8}): ;", re.M)
_GOTO_RE = re.compile(r"goto loc_([0-9A-F]{8});")
_DEFINE_RE = re.compile(r"#define (\w+)")
_RUST_FN_RE = re.compile(r"pub fn (\w+)(<[^>]*>)?\s*\(")


@dataclass
class Lifted:
    va: int
    header: str
    body: str

    def identifiers_called(self) -> set[str]:
        return set(_CALL_RE.findall(_strip_comments(self.body)))

    def calls(self, macros: frozenset[str] = frozenset()) -> set[str]:
        """Calls out of the function: everything but pure helpers, local and lift macros, libm."""
        local = set(_DEFINE_RE.findall(self.body))
        pure = PURE_C_HELPERS | LIBC_PURE | (macros - CALL_MACROS)
        return {
            name for name in self.identifiers_called() if name not in pure and name not in local
        }

    def transcendental(self) -> bool:
        return bool(self.identifiers_called() & TRANSCENDENTAL)

    def backward_gotos(self) -> int:
        """Gotos to a label at or above the goto (a loop). Labels are VAs, so compare VAs."""
        count = 0
        for match in _GOTO_RE.finditer(self.body):
            labels_before = {int(x, 16) for x in _LABEL_RE.findall(self.body[: match.start()])}
            if int(match.group(1), 16) in labels_before:
                count += 1
        return count

    def structured_loops(self) -> int:
        text = _strip_comments(self.body)
        return len(re.findall(r"\b(for|while)\s*\(", text))

    def fpo_leaf(self) -> bool:
        return "fpo_leaf: inherit caller's frame" in self.body

    def uses_fpu(self) -> bool:
        code = _strip_comments(self.body)
        return re.search(r"\bfp_(push|pop|top|st1?)\(|\bg_fp_", code) is not None

    def ret_pops(self) -> int | None:
        """Bytes RET pops beyond the return address (`esp += 4 + N; return;`), None if unclear."""
        pops = set()
        for match in re.finditer(r"esp \+= (\d+)(?: \+ (\d+))?; return;", self.body):
            pops.add(int(match.group(2) or 0) if match.group(1) == "4" else int(match.group(1)) - 4)
        return pops.pop() if len(pops) == 1 else None


@dataclass
class RustPort:
    crate: str
    module: str
    source: str
    function: str
    takes_guest_call: bool
    loops: int
    in_run_table: bool


@dataclass
class Row:
    va: int
    name: str
    subsystem: str
    reach_class: str
    lifted: bool = False
    c_calls: list[str] = field(default_factory=list)
    c_loops: int = 0
    fpu: bool = False
    transcendental: bool = False
    fpo_leaf: bool = False
    ret_pops: int | None = None
    rust: RustPort | None = None
    rust_candidates: int = 0

    @property
    def leaf(self) -> bool:
        return (
            self.lifted
            and not self.c_calls
            and self.rust is not None
            and not self.rust.takes_guest_call
        )

    @property
    def loop_free(self) -> bool:
        return self.leaf and self.c_loops == 0 and self.rust is not None and self.rust.loops == 0

    @property
    def eligible(self) -> bool:
        """A leaf the harness can call: dispatched by its module's `run` table, known RET pops."""
        return (
            self.leaf
            and self.rust is not None
            and self.rust.in_run_table
            and self.ret_pops is not None
        )

    def as_json(self) -> dict[str, object]:
        data = asdict(self)
        data["va"] = f"{self.va:#x}"
        data.update(leaf=self.leaf, loop_free=self.loop_free, eligible=self.eligible)
        return data


def _strip_comments(text: str) -> str:
    """Code only: no comments, no preprocessor lines (the per-body x87 macros hold `while (0)`)."""
    text = re.sub(r"/\*.*?\*/", "", text, flags=re.S)
    text = re.sub(r"^\s*#.*?(?<!\\)$", "", re.sub(r"\\\n", " ", text), flags=re.M)
    return text


def lift_macros(gen_dir: Path) -> frozenset[str]:
    """Function-like macro names of the lift's runtime header (`recomp_types.h`)."""
    header = gen_dir / "recomp_types.h"
    if not header.is_file():
        raise FileNotFoundError(f"{header} missing: not a lift gen dir")
    return frozenset(re.findall(r"^#\s*define\s+(\w+)\(", header.read_text(errors="replace"), re.M))


@functools.lru_cache(maxsize=4)
def parse_lifted(gen_dir: Path) -> dict[int, Lifted]:
    """Every `void sub_<VA>(void)` body of the lift, with its doc header."""
    out: dict[int, Lifted] = {}
    chunks = sorted(gen_dir.glob("recomp_[0-9]*.c"))
    if not chunks:
        raise FileNotFoundError(f"no recomp_<n>.c chunks under {gen_dir}")
    for chunk in chunks:
        text = chunk.read_text(errors="replace")
        headers = {int(m.group(1), 16): m.group(2) for m in _HEADER_RE.finditer(text)}
        for match in _FUNC_RE.finditer(text):
            va = int(match.group(1), 16)
            out[va] = Lifted(va, headers.get(va, ""), match.group(2))
    return out


def rust_function(source_text: str, va: int) -> tuple[str, str, int] | None:
    """(name, body, generic-signature) of the `pub fn` documented as `original <va>`."""
    marker = re.search(rf"original {va:#X}\b".replace("0X", "0x"), source_text)
    if marker is None:
        return None
    fn_match = _RUST_FN_RE.search(source_text, marker.end())
    if fn_match is None:
        return None
    start = fn_match.start()
    return fn_match.group(1), source_text[start : _block_end(source_text, start)], start


def _block_end(text: str, start: int) -> int:
    """End of the brace block opened by the first `{` at or after `start` (or the text end)."""
    depth = 0
    for index in range(text.find("{", start), len(text)):
        if index < 0:
            break
        if text[index] == "{":
            depth += 1
        elif text[index] == "}":
            depth -= 1
            if depth == 0:
                return index + 1
    return len(text)


def rust_port_for(
    root: Path, va: int, entries: list[dict[str, object]], cache: dict[str, str]
) -> RustPort | None:
    """The registry entry whose module dispatches the VA in a `run` table, facades last."""

    def rank(entry: dict[str, object]) -> tuple[int, int, str]:
        crate = str(entry["crate"])
        facade = crate in {"tsfp-ports", "tsfp-lane-c2r"}
        return (0 if "roots" in entry["forms"] else 1, 1 if facade else 0, str(entry["source"]))  # type: ignore[operator]

    for entry in sorted(entries, key=rank):
        source = str(entry["source"])
        if source not in cache:
            path = root / source
            cache[source] = path.read_text(errors="replace") if path.is_file() else ""
        text = cache[source]
        found = rust_function(text, va)
        if found is None:
            continue
        name, body, _ = found
        signature = body.split("{", 1)[0]
        in_run = re.search(rf"^\s*{va:#x}\s*=>\s*\{{?\s*{name}\(", text, re.M) is not None
        return RustPort(
            crate=str(entry["crate"]),
            module=str(entry["module"]),
            source=source,
            function=name,
            takes_guest_call=("GuestCall" in signature),
            loops=len(
                re.findall(r"\b(for|while|loop)\b", body.split("{", 1)[1] if "{" in body else "")
            ),
            in_run_table=in_run,
        )
    return None


def read_names(root: Path) -> dict[int, str]:
    names: dict[int, str] = {}
    path = root / NAMES_CSV
    if path.is_file():
        with path.open(newline="") as handle:
            for row in csv.DictReader(handle):
                names[int(row["entry_va"], 16)] = row["name"]
    return names


def read_reach(path: Path) -> list[dict[str, str]]:
    if not path.is_file():
        raise FileNotFoundError(
            f"{path} missing: rebuild with `python3 -m tools.t2reach audit --write`"
        )
    with path.open(newline="") as handle:
        return list(csv.DictReader(handle))


def build(root: Path, gen_dir: Path, reach_csv: Path) -> list[Row]:
    registry = json.loads((root / REGISTRY).read_text())["ports"]
    lifted = parse_lifted(gen_dir)
    macros = lift_macros(gen_dir)
    names = read_names(root)
    cache: dict[str, str] = {}
    rows: list[Row] = []
    for item in read_reach(reach_csv):
        va = int(item["va"], 16)
        row = Row(
            va=va,
            name=names.get(va, f"sub_{va:08X}"),
            subsystem=item["subsystem"],
            reach_class=item["class"],
        )
        body = lifted.get(va)
        if body is not None:
            row.lifted = True
            row.c_calls = sorted(body.calls(macros))
            row.transcendental = body.transcendental()
            row.c_loops = body.backward_gotos() + body.structured_loops()
            row.fpu = body.uses_fpu()
            row.fpo_leaf = body.fpo_leaf()
            row.ret_pops = body.ret_pops()
        entries = registry.get(f"{va:#010x}", [])
        row.rust_candidates = len(entries)
        row.rust = rust_port_for(root, va, entries, cache)
        rows.append(row)
    return rows


def selection_key(va: int) -> str:
    return hashlib.sha256(f"{SELECTION_SALT}{va:#x}".encode()).hexdigest()


def select(rows: list[Row], count: int) -> list[Row]:
    """The first `count` eligible leaves by sha256(`T3871:<va>`): deterministic per input."""
    return sorted((row for row in rows if row.eligible), key=lambda row: selection_key(row.va))[
        :count
    ]


def summary(rows: list[Row]) -> dict[str, object]:
    def count(predicate: Callable[[Row], bool]) -> int:
        return sum(1 for row in rows if predicate(row))

    by_class: dict[str, dict[str, int]] = {}
    for row in rows:
        bucket = by_class.setdefault(
            row.reach_class, {"never_executed": 0, "leaf": 0, "loop_free_leaf": 0}
        )
        bucket["never_executed"] += 1
        bucket["leaf"] += int(row.leaf)
        bucket["loop_free_leaf"] += int(row.loop_free)
    return {
        "never_executed_port_functions": len(rows),
        "with_lifted_body": count(lambda r: r.lifted),
        "with_rust_port_located": count(lambda r: r.rust is not None),
        "c_side_no_calls": count(lambda r: r.lifted and not r.c_calls),
        "rust_side_no_guest_call": count(
            lambda r: r.rust is not None and not r.rust.takes_guest_call
        ),
        "leaf": count(lambda r: r.leaf),
        "loop_free_leaf": count(lambda r: r.loop_free),
        "leaf_with_x87": count(lambda r: r.leaf and r.fpu),
        "leaf_with_transcendental_libm": count(lambda r: r.leaf and r.transcendental),
        "eligible_for_harness": count(lambda r: r.eligible),
        "by_reach_class": dict(sorted(by_class.items())),
    }
