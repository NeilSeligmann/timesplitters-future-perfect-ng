# SPDX-License-Identifier: GPL-3.0-or-later
# ruff: noqa: E501
"""T1874: derive the replacement-registration metadata of the 136 T1850 roots from the private lifted C.

Output (tracked, snippet-free): tools/replace/invpose_roots.json with per root the module, the
cleanup convention (`ret imm` of the lifted body), a conservative stack argument count, whether
the lifted body calls other functions, uses x87 or SSE state. Nothing here is a verdict.
"""

from __future__ import annotations

import argparse
import functools
import json
import re
from pathlib import Path


def lane_modules() -> tuple[str, ...]:
    manifest = Path(__file__).resolve().parents[2] / "crates/tsfp-ports/lanes.json"
    return tuple(lane["module"] for lane in json.loads(manifest.read_text())["lanes"])


def rust_roots(crate: Path) -> dict[int, str]:
    found: dict[int, str] = {}
    for module in lane_modules():
        text = (crate / f"src/{module}.rs").read_text()
        start = text.find("pub fn run<")
        for match in re.finditer(r"0x([0-9a-fA-F]+)(?=\s*(?:=>|\|))", text[start:]):
            found[int(match[1], 16)] = module
    return found


_DEFINITION = re.compile(r"(?m)^(?:static )?\w[^\n]*\bsub_([0-9A-F]{8})\(void\)\s*\{")


@functools.lru_cache(maxsize=4)
def _definitions(gen: Path) -> tuple[dict[int, tuple[str, int]], dict[str, str]]:
    """First definition of every `sub_XXXXXXXX(void)` in the lifted chunks, found in one pass.

    T3605 (MEASURED, droplet): the per-root search read and scanned all 70 chunks (101 MB) once per root, 401 roots
    (rust_weapons 188 s, hud_gate 22 s, the whole configure-generators miss of every landing that touches lanes.json
    or a lane file). One pass over each chunk gives the same first match per root (sorted chunk order, first match
    in the chunk) in a few seconds.
    """
    found: dict[int, tuple[str, int]] = {}
    texts: dict[str, str] = {}
    for path in sorted(gen.glob("recomp_*.c")):
        text = path.read_text()
        texts[path.name] = text
        for match in _DEFINITION.finditer(text):
            found.setdefault(int(match[1], 16), (path.name, match.start()))
    return found, texts


def lifted_body(gen: Path, va: int) -> str:
    found, texts = _definitions(gen)
    if va not in found:
        raise SystemExit(f"sub_{va:08X} not found")
    name, start = found[va]
    text = texts[name]
    return text[start : text.find("\n}\n", start) + 3]


def arguments(body: str) -> int:
    """Highest stack argument index read, tracking esp (and an ebp frame) in text order."""
    delta = 0  # esp relative to entry, bytes (negative after pushes)
    frame: int | None = None  # ebp = esp delta
    highest = -1
    for line in body.splitlines():
        code = line.split("/*")[0]
        for _ in re.finditer(r"PUSH32\(esp", code):
            delta -= 4
        for _ in re.finditer(r"POP32\(esp", code):
            delta += 4
        sub = re.search(r"\besp (-|\+)= (0x[0-9A-Fa-f]+|\d+)u?;", code)
        if sub and "return" not in code:
            amount = int(sub[2], 0)
            delta += -amount if sub[1] == "-" else amount
        if re.search(r"\bebp = esp;", code):
            frame = delta
        for m in re.finditer(r"MEM(?:8|16|32|F)\(esp(?: \+ (0x[0-9A-Fa-f]+|\d+))?\)", code):
            offset = delta + int(m[1], 0) - 4 if m[1] else delta - 4
            # entry esp points at the return address, argument i at +4+4i
            index = (offset - 0) // 4 - 0
            index = (int(m[1], 0) if m[1] else 0) + delta
            if index >= 4:
                highest = max(highest, (index - 4) // 4)
        if frame is not None:
            for m in re.finditer(r"MEM(?:8|16|32|F)\(ebp \+ (0x[0-9A-Fa-f]+|\d+)\)", code):
                offset = int(m[1], 0)
                if offset >= 8:
                    highest = max(highest, (offset - 8) // 4)
    return highest + 1


def describe(gen: Path, va: int) -> dict[str, object]:
    body = lifted_body(gen, va)
    rets = {
        int(m[1], 0)
        for m in re.finditer(r"esp \+= (?:4 \+ )?(0x[0-9A-Fa-f]+|\d+)u?; return;", body)
    }
    cleanup = sorted(rets)
    pops = {r - 4 for r in cleanup}
    return {
        "ret_pops": sorted(pops),
        "stack_args": arguments(body),
        "calls": bool(
            re.search(
                r"\bsub_[0-9A-F]{8}\(\)|RECOMP_ICALL|RECOMP_ABI_CALL|RECOMP_CALL|RECOMP_ITAIL|RECOMP_TAIL",
                body,
            )
        ),
        "x87": bool(re.search(r"g_fp_|FP_|fld|x87", body)),
        "sse": bool(re.search(r"xmm[0-7]", body)),
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--gen-dir", type=Path, required=True)
    parser.add_argument("--out", type=Path, default=Path("tools/replace/invpose_roots.json"))
    args = parser.parse_args()
    if args.out.resolve() == Path("tools/replace/invpose_roots.json").resolve():
        parser.error(
            "write evidence with --out <scratch.json>; update lanes.json, then tools.lanes regen"
        )
    crate = Path(__file__).resolve().parents[2] / "crates/tsfp-ports"
    rows = []
    for va, module in sorted(rust_roots(crate).items()):
        info = describe(args.gen_dir, va)
        rows.append({"va": f"{va:08X}", "module": module, **info})
    args.out.write_text(json.dumps({"schema": 1, "roots": rows}, indent=1) + "\n")
    print(len(rows), "roots")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
