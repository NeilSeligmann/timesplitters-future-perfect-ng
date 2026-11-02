# SPDX-License-Identifier: GPL-3.0-or-later
"""T3871: make a lifted body's guest-memory accesses functional (byte-wise reads and writes).

The lift accesses guest memory through lvalue macros (`MEM32(a) = v`, `x = MEMF(a)`), a typed
host pointer into one flat mapping. CBMC cannot reason about a typed access into a 2^32-byte
object without flattening it, so every access becomes a call into the harness memory model:

    MEMn(a) = v;          ->  T2S_W_MEMn(a, v);
    MEMn(a) op= v;        ->  T2S_W_MEMn(a, T2S_R_MEMn(a) op (v));
    MEMn(a)++ / ++MEMn(a) ->  T2S_W_MEMn(a, T2S_R_MEMn(a) + 1);   (statement form only)
    MEMn(a) (any read)    ->  T2S_R_MEMn(a)

Anything else that takes a guest address (`XBOX_PTR`, an `&MEMn(...)`) raises `Unsupported`, so
the harness never silently runs a body it did not translate.
"""

from __future__ import annotations

import re

MACROS = ("SMEM64", "SMEM32", "SMEM16", "SMEM8", "MEM64", "MEM32", "MEM16", "MEM8", "MEMF", "MEMD")
_MACRO_RE = re.compile(r"\b(" + "|".join(MACROS) + r")\(")
_COMPOUND = ("<<=", ">>=", "+=", "-=", "*=", "/=", "%=", "&=", "|=", "^=")


# Header helpers that touch guest memory and that templates/glue.c redefines over the model.
GLUE_REPLACED = frozenset(
    {
        "PUSH32",
        "POP32",
        "XMM_MEM",
        "XMM_STORE",
        "XMM_LOAD_LOW",
        "XMM_LOAD_HIGH",
        "XMM_STORE_LOW",
        "XMM_STORE_HIGH",
        "MMX_MEM",
        "MMX_STORE",
    }
)
_HELPER_START = re.compile(r"^(?:#\s*define\s+(\w+)\(|static inline [\w ]+?\**\s*(\w+)\()", re.M)


class Unsupported(ValueError):
    """The body uses a guest-memory form the rewrite does not translate."""


def memory_helpers(header_text: str) -> frozenset[str]:
    """Macros and static inline functions of the lift header whose body touches guest memory
    (uses MEMn or XBOX_PTR), derived from the header itself (rule 45)."""
    starts = list(_HELPER_START.finditer(header_text))
    found = set()
    for index, match in enumerate(starts):
        name = match.group(1) or match.group(2)
        end = starts[index + 1].start() if index + 1 < len(starts) else len(header_text)
        if name in MACROS or name == "XBOX_PTR":
            continue
        if re.search(r"\b(?:S?MEM\w*|XBOX_PTR)\(", header_text[match.end() : end]):
            found.add(name)
    return frozenset(found)


def refuse_unmodelled_helpers(body: str, helpers: frozenset[str]) -> None:
    """Raise when the body calls a memory-touching helper that the glue does not redefine."""
    for name in sorted(helpers - GLUE_REPLACED):
        if re.search(rf"\b{name}\s*\(", body):
            raise Unsupported(f"memory helper {name} not modelled")


def _close_paren(text: str, open_index: int) -> int:
    """Index of the parenthesis closing the one at `open_index`."""
    depth = 0
    for index in range(open_index, len(text)):
        char = text[index]
        if char == "(":
            depth += 1
        elif char == ")":
            depth -= 1
            if depth == 0:
                return index
    raise Unsupported("unbalanced parentheses")


def _statement_end(text: str, start: int) -> int:
    """Index of the `;` ending the expression that starts at `start` (depth 0)."""
    depth = 0
    for index in range(start, len(text)):
        char = text[index]
        if char in "([{":
            depth += 1
        elif char in ")]}":
            depth -= 1
            if depth < 0:
                raise Unsupported("assignment inside a larger expression")
        elif char == ";" and depth == 0:
            return index
    raise Unsupported("unterminated assignment")


def _statement_start(text: str, index: int) -> bool:
    """True when the macro at `index` begins a statement (previous token ends one)."""
    before = text[:index].rstrip()
    if before.endswith(")"):
        # The body statement of a control header: `for (...) MEM32(a) = v;` (rep string ops).
        depth = 0
        for position in range(len(before) - 1, -1, -1):
            depth += {")": 1, "(": -1}.get(before[position], 0)
            if depth == 0:
                return re.search(r"\b(?:for|while|if)\s*$", before[:position]) is not None
        return False
    return not before or before[-1] in ";{}:" or before.endswith("*/")


def functionalise(body: str) -> str:
    if "XBOX_PTR" in body:
        raise Unsupported("raw XBOX_PTR address use")
    out: list[str] = []
    position = 0
    while True:
        match = _MACRO_RE.search(body, position)
        if match is None:
            out.append(body[position:])
            break
        macro = match.group(1)
        start = match.start()
        if body[:start].rstrip().endswith("&"):
            raise Unsupported(f"address of {macro}")
        close = _close_paren(body, match.end() - 1)
        address = functionalise(body[match.end() : close])
        rest = body[close + 1 :]
        stripped = rest.lstrip()
        gap = len(rest) - len(stripped)
        out.append(body[position:start])
        operator = next((op for op in _COMPOUND if stripped.startswith(op)), None)
        if stripped.startswith("=") and not stripped.startswith("=="):
            if not _statement_start(body, start):
                raise Unsupported(f"{macro} assignment used as a value")
            value_start = close + 1 + gap + 1
            end = _statement_end(body, value_start)
            value = functionalise(body[value_start:end].strip())
            out.append(f"T2S_W_{macro}({address}, {value})")
            position = end
        elif operator is not None:
            if not _statement_start(body, start):
                raise Unsupported(f"{macro} compound assignment used as a value")
            value_start = close + 1 + gap + len(operator)
            end = _statement_end(body, value_start)
            value = functionalise(body[value_start:end].strip())
            op = operator[:-1]
            out.append(f"T2S_W_{macro}({address}, T2S_R_{macro}({address}) {op} ({value}))")
            position = end
        elif stripped.startswith(("++", "--")):
            if not _statement_start(body, start):
                raise Unsupported(f"{macro} post-increment used as a value")
            op = "+" if stripped.startswith("++") else "-"
            out.append(f"T2S_W_{macro}({address}, T2S_R_{macro}({address}) {op} 1)")
            position = close + 1 + gap + 2
        else:
            if body[:start].rstrip().endswith(("++", "--")):
                raise Unsupported(f"pre-increment of {macro}")
            out.append(f"T2S_R_{macro}({address})")
            position = close + 1
    return "".join(out)
