# SPDX-License-Identifier: GPL-3.0-or-later
"""Rewrite lifted C chunks so every guest-memory access goes through CAP_* macros (T3739).

`MEMn(a) = v;` becomes `CAP_STn(a, v);`, every other `MEMn(a)` becomes `CAP_LDn(a)`, each lifted
function gets `CAP_ENTER(0xVA);` after its opening brace and `CAP_EXIT();` before every `return`
and before a falling-off-the-end close brace. `XMM_MEM/XMM_STORE/MMX_MEM/MMX_STORE` are renamed
to their `CAP_` twins. One `#include "t2_capture.h"` line follows `#include "recomp_funcs.h"`.
Everything else is copied byte for byte. The macros in `src/host/t2_capture.h` expand to today's
code when `TSFP_CAPTURE` is undefined, so the normal build is unchanged.

Parsing is token based (comments, string and char literals, preprocessor lines are opaque) with
balanced-bracket matching, never a regex over arguments. Unsupported constructs raise
`TransformError` instead of being skipped silently.

    python3 -m tools.t2capture.transform <in.c> <out.c>
    python3 -m tools.t2capture.transform --all <lifted_gen_dir> <out_dir>
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from collections import Counter
from pathlib import Path

MEM_MACROS = {
    "MEM8": "8",
    "MEM16": "16",
    "MEM32": "32",
    "SMEM8": "S8",
    "SMEM16": "S16",
    "SMEM32": "S32",
    "SMEM64": "S64",
    "MEMF": "F",
    "MEMD": "D",
}
RENAMED_HELPERS = {
    "XMM_MEM": "CAP_XMM_LD",
    "XMM_STORE": "CAP_XMM_ST",
    "MMX_MEM": "CAP_MMX_LD",
    "MMX_STORE": "CAP_MMX_ST",
}
# T3740: plus the host build's retained originals (tsfp_original_X, tsfp_hud_original_X, ...),
# which the Rust lane, HUD gates and code-write hooks run in place of the weakened lifted body.
FUNC_NAME = re.compile(
    r"(?:sub_|recomp_original_|recomp_xmv_original_|tsfp_(?:[a-z]+_)?original_)([0-9A-Fa-f]{8})$"
)
ORIGINAL_VA = re.compile(r"Original:\s*0x([0-9A-Fa-f]+)")
MEM_WORD = re.compile(r"\bS?MEM(?:8|16|32|64|F|D)?\b")
INCLUDE_ANCHOR = re.compile(r'^([ \t]*#[ \t]*include[ \t]+"recomp_funcs\.h"[^\n]*\n)', re.M)
INCLUDE_LINE = '#include "t2_capture.h"\n'
COMPOUND = {"+=", "-=", "*=", "/=", "%=", "&=", "|=", "^=", "<<=", ">>=", "++", "--"}
STATEMENT_PREV = {";", "{", "}", ":", ")", "else", None}
OPERAND_END = re.compile(r"[A-Za-z0-9_)\]]$")
PP_LINE = re.compile(r"\#(?:[^\n\\]|\\.)*", re.S)

TOKEN = re.compile(
    r"""
    (?P<comment>/\*.*?\*/|//[^\n]*)
  | (?P<str>"(?:[^"\\\n]|\\.)*")
  | (?P<chr>'(?:[^'\\\n]|\\.)*')
  | (?P<ws>\s+)
  | (?P<id>[A-Za-z_][A-Za-z0-9_]*)
  | (?P<num>\.?[0-9](?:[eEpP][+-]|[A-Za-z0-9_.])*)
  | (?P<punct><<=|>>=|\.\.\.|->|\+\+|--|<<|>>|<=|>=|==|!=|&&|\|\||[-+*/%&|^]=|.)
    """,
    re.X | re.S,
)


class TransformError(Exception):
    """An input construct the transform refuses to handle."""


class Tok:
    __slots__ = ("kind", "text", "pos")

    def __init__(self, kind: str, text: str, pos: int) -> None:
        self.kind, self.text, self.pos = kind, text, pos

    @property
    def sig(self) -> bool:
        return self.kind not in ("ws", "comment", "pp")


def tokenize(src: str) -> list[Tok]:
    toks: list[Tok] = []
    pos, size = 0, len(src)
    while pos < size:
        if src[pos] == "#":
            line_start = src.rfind("\n", 0, pos) + 1
            if not src[line_start:pos].strip():
                m = PP_LINE.match(src, pos)
                if m:
                    toks.append(Tok("pp", m.group(), pos))
                    pos = m.end()
                    continue
        m = TOKEN.match(src, pos)
        if m is None or m.end() == pos:
            raise TransformError(f"cannot tokenize at line {src.count(chr(10), 0, pos) + 1}")
        toks.append(Tok(m.lastgroup or "punct", m.group(), pos))
        pos = m.end()
    return toks


class Transformer:
    def __init__(self, src: str, name: str = "<input>", helpers: bool = False) -> None:
        self.src, self.name = src, name
        # T3794: hand-written helpers next to retained originals (hud_gate_originals.c) get their
        # guest accesses rewritten too, without a frame of their own (they run inside the caller's).
        self.helpers, self.in_helper = helpers, False
        self.toks = tokenize(src)
        self.stats: Counter[str] = Counter()
        n = len(self.toks)
        self.match = [-1] * n
        stack: list[int] = []
        closers = {")": "(", "]": "[", "}": "{"}
        for i, tok in enumerate(self.toks):
            if tok.kind != "punct":
                continue
            if tok.text in ("(", "[", "{"):
                stack.append(i)
            elif tok.text in closers:
                if not stack or self.toks[stack[-1]].text != closers[tok.text]:
                    self.fail(i, f"unbalanced '{tok.text}'")
                j = stack.pop()
                self.match[i], self.match[j] = j, i
        if stack:
            self.fail(stack[-1], "unclosed bracket")
        self.prev_sig = [-1] * n
        self.next_sig = [-1] * n
        last = -1
        for i in range(n):
            self.prev_sig[i] = last
            if self.toks[i].sig:
                last = i
        last = -1
        for i in range(n - 1, -1, -1):
            self.next_sig[i] = last
            if self.toks[i].sig:
                last = i

    def fail(self, idx: int, msg: str) -> None:
        line = self.src.count("\n", 0, self.toks[idx].pos) + 1
        raise TransformError(f"{self.name}:{line}: {msg}")

    def text_at(self, idx: int) -> str | None:
        return self.toks[idx].text if idx >= 0 else None

    # -- top level: find function definitions ---------------------------------------------------
    def run(self) -> str:
        out: list[str] = []
        i, n = 0, len(self.toks)
        while i < n:
            tok = self.toks[i]
            if tok.kind == "pp" and MEM_WORD.search(tok.text):
                self.fail(i, "MEM macro inside a preprocessor line outside a function")
            if tok.kind == "punct" and tok.text == "{":
                end = self.match[i]
                head = self.prev_sig[i]
                if head >= 0 and self.toks[head].text == ")":
                    out.append(self.function(i, end))
                else:
                    out.append(self.plain(i, end + 1))
                i = end + 1
                continue
            out.append(tok.text)
            i += 1
        text = "".join(out)
        if self.stats["functions"]:
            text, count = INCLUDE_ANCHOR.subn(lambda m: m.group(1) + INCLUDE_LINE, text, count=1)
            if count == 0:
                raise TransformError(
                    f'{self.name}: no #include "recomp_funcs.h" to anchor t2_capture.h'
                )
        return text

    def plain(self, i: int, j: int) -> str:
        """Copy a non-function brace block verbatim, refusing hidden MEM uses."""
        for k in range(i, j):
            if self.toks[k].kind == "id" and self.toks[k].text in MEM_MACROS:
                self.fail(k, "MEM macro in a non-lifted function or block (unsupported)")
        return "".join(t.text for t in self.toks[i:j])

    def function_info(self, open_idx: int) -> tuple[str, int] | None:
        close = self.prev_sig[open_idx]
        lpar = self.match[close]
        name_idx = self.prev_sig[lpar]
        if name_idx < 0 or self.toks[name_idx].kind != "id":
            return None
        inner = self.next_sig[lpar]
        if self.text_at(inner) != "void" or self.next_sig[inner] != close:
            return None
        name = self.toks[name_idx].text
        m = FUNC_NAME.match(name)
        if m:
            return name, int(m.group(1), 16)
        if name == "xbe_entry_point":
            for k in range(name_idx - 1, -1, -1):
                if self.toks[k].kind == "comment":
                    mm = ORIGINAL_VA.search(self.toks[k].text)
                    if mm:
                        return name, int(mm.group(1), 16)
        return None

    def function(self, open_idx: int, close_idx: int) -> str:
        info = self.function_info(open_idx)
        if info is None and self.helpers:
            self.stats["helper_functions"] += 1
            self.in_helper = True
            try:
                return "{" + self.render(open_idx + 1, close_idx) + "}"
            finally:
                self.in_helper = False
        if info is None:
            return self.plain(open_idx, close_idx + 1)
        _name, va = info
        self.stats["functions"] += 1
        body = self.render(open_idx + 1, close_idx)
        tail = self.prev_sig[close_idx]
        tail_is_return = (
            tail > open_idx
            and self.text_at(tail) == ";"
            and self.text_at(self.prev_sig[tail]) == "return"
        )
        exit_text = ""
        if not tail_is_return:
            self.stats["falloff_exits"] += 1
            exit_text = "    CAP_EXIT();\n" if body.endswith("\n") else " CAP_EXIT(); "
        return f"{{\n    CAP_ENTER(0x{va:08X}u);" + body + exit_text + "}"

    # -- expression and statement rewriting ------------------------------------------------------
    def render(self, i: int, j: int) -> str:
        out: list[str] = []
        toks = self.toks
        while i < j:
            tok = toks[i]
            if tok.kind == "pp":
                if MEM_WORD.search(tok.text):
                    self.fail(i, "MEM macro inside a #define in a function body (unsupported)")
                if re.search(r"\breturn\b", tok.text):
                    self.fail(i, "return inside a macro defined in a function body (unsupported)")
                out.append(tok.text)
                i += 1
            elif tok.kind == "id" and tok.text in MEM_MACROS and self.next_is(i, "("):
                text, i = self.memory_access(i)
                out.append(text)
            elif tok.kind == "id" and tok.text in RENAMED_HELPERS and self.next_is(i, "("):
                self.stats["helper_" + tok.text] += 1
                out.append(RENAMED_HELPERS[tok.text])
                i += 1
            elif tok.kind == "id" and tok.text == "return" and not self.in_helper:
                text, i = self.return_stmt(i)
                out.append(text)
            else:
                out.append(tok.text)
                i += 1
        return "".join(out)

    def next_is(self, i: int, text: str) -> bool:
        k = self.next_sig[i]
        return k >= 0 and self.toks[k].text == text

    def memory_access(self, i: int) -> tuple[str, int]:
        name = self.toks[i].text
        kind = MEM_MACROS[name]
        lpar = self.next_sig[i]
        rpar = self.match[lpar]
        arg = self.render(lpar + 1, rpar).strip()
        prev = self.prev_sig[i]
        nxt = self.next_sig[rpar]
        nxt_text = self.text_at(nxt)
        if nxt_text in COMPOUND or self.text_at(prev) in ("++", "--"):
            self.fail(i, f"compound assignment or increment on {name} (unsupported)")
        if self.text_at(prev) == "&":
            before = self.text_at(self.prev_sig[prev])
            if before is None or not OPERAND_END.search(before):
                self.fail(i, f"address-of {name}(...) cannot be captured (unsupported)")
        if nxt_text == "=":
            if self.text_at(prev) not in STATEMENT_PREV:
                self.fail(i, f"{name} store is not statement-initial (unsupported)")
            vend = nxt + 1
            while vend < len(self.toks):
                t = self.toks[vend]
                if t.kind == "punct" and t.text in ("(", "[", "{"):
                    vend = self.match[vend] + 1
                    continue
                if t.kind == "punct" and t.text in (")", "]", "}", ";", ","):
                    break
                vend += 1
            raw = self.render(nxt + 1, vend)
            value = raw.strip()
            if not value:
                self.fail(i, f"{name} store with empty value")
            trailing = raw[len(raw.rstrip()) :]
            self.stats["stores"] += 1
            self.stats["stores_" + name] += 1
            return f"CAP_ST{kind}({arg}, {value}){trailing}", vend
        self.stats["loads"] += 1
        self.stats["loads_" + name] += 1
        return f"CAP_LD{kind}({arg})", rpar + 1

    def return_stmt(self, i: int) -> tuple[str, int]:
        semi = self.next_sig[i]
        if self.text_at(semi) != ";":
            self.fail(i, "return with a value (unsupported)")
        self.stats["returns"] += 1
        gap = "".join(t.text for t in self.toks[i + 1 : semi])
        if self.text_at(self.prev_sig[i]) in (")", "else"):
            self.stats["returns_braced"] += 1
            return "{ CAP_EXIT(); return" + gap + "; }", semi + 1
        return "CAP_EXIT(); return", i + 1


def transform_source(
    src: str, name: str = "<input>", helpers: bool = False
) -> tuple[str, Counter[str]]:
    tr = Transformer(src, name, helpers)
    text = tr.run()
    return text, tr.stats


def survey(src: str) -> Counter[str]:
    """Count the constructs of a chunk without failing on the unsupported ones."""
    toks = tokenize(src)
    stats: Counter[str] = Counter()
    sig_prev: list[str] = []
    for tok in toks:
        if tok.kind == "id" and tok.text == "goto":
            stats["goto"] += 1
        elif tok.kind == "comment" and "tail jmp" in tok.text:
            stats["tail_jmp_comments"] += 1
        elif tok.kind == "pp":
            if MEM_WORD.search(tok.text):
                stats["mem_in_pp"] += 1
            if re.match(r"\s*#\s*define", tok.text):
                stats["defines"] += 1
        elif tok.kind == "id" and tok.text in MEM_MACROS:
            if (
                sig_prev
                and sig_prev[-1] == "&"
                and not (len(sig_prev) > 1 and OPERAND_END.search(sig_prev[-2]))
            ):
                stats["addr_of_mem"] += 1
        if tok.sig:
            sig_prev.append(tok.text)
            del sig_prev[:-3]
    return stats


def lifted_chunks(gen_dir: Path) -> list[Path]:
    return sorted(set(gen_dir.glob("recomp_[0-9]*.c")) | set(gen_dir.glob("recomp_*_original.c")))


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    ap.add_argument(
        "paths", nargs="+", help="<in.c> <out.c>, or with --all <lifted_gen_dir> <out_dir>"
    )
    ap.add_argument(
        "--all", action="store_true", help="transform every lifted chunk of a directory"
    )
    ap.add_argument("--stats-json", help="write the counters to this file")
    ap.add_argument(
        "--helpers",
        action="store_true",
        help="also rewrite guest accesses in non-lifted functions, without a frame (T3794)",
    )
    ap.add_argument(
        "--survey", action="store_true", help="with --all, only count constructs (read-only)"
    )
    args = ap.parse_args(argv)
    total: Counter[str] = Counter()
    try:
        if args.all and args.survey:
            for chunk in lifted_chunks(Path(args.paths[0])):
                total.update(survey(chunk.read_text(errors="surrogateescape")))
                total["chunks"] += 1
        elif args.all:
            if len(args.paths) != 2:
                ap.error("--all needs <lifted_gen_dir> <out_dir>")
            out_dir = Path(args.paths[1])
            out_dir.mkdir(parents=True, exist_ok=True)
            for chunk in lifted_chunks(Path(args.paths[0])):
                text, stats = transform_source(
                    chunk.read_text(errors="surrogateescape"), chunk.name
                )
                (out_dir / chunk.name).write_text(text, errors="surrogateescape")
                total.update(stats)
                total["chunks"] += 1
        else:
            if len(args.paths) != 2:
                ap.error("need <in.c> <out.c>")
            src_path, dst_path = Path(args.paths[0]), Path(args.paths[1])
            text, stats = transform_source(
                src_path.read_text(errors="surrogateescape"), src_path.name, args.helpers
            )
            dst_path.write_text(text, errors="surrogateescape")
            total.update(stats)
    except TransformError as exc:
        print(f"t2capture: error: {exc}", file=sys.stderr)
        return 2
    summary = json.dumps(dict(sorted(total.items())), indent=1)
    if args.stats_json:
        Path(args.stats_json).write_text(summary + "\n")
    print(summary)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
