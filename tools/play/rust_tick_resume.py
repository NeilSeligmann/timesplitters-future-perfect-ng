# SPDX-License-Identifier: GPL-3.0-or-later
# ruff: noqa: ANN001, E501, UP031
"""T1835 w12: preserve the lifted entity tick 0x2784D0 twice in an ignored build file.

`tsfp_original_002784d0` is the unchanged lifted body (the comparison oracle). `tsfp_tick_resume`
is the same body with an entry switch added: when `g_tick_resume_entry` is non-zero the function
starts at that already-existing `loc_XXXXXXXX` label instead of the top, with the local EBP seeded
from `g_ebp`. A Rust port of a leading region can therefore hand its exit state to the lifted
remainder. Nothing is changed besides the added switch and the rename. Private input, private output.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
from pathlib import Path

ROOT = 0x2784D0
SEED_LINE = "ebp = g_seh_ebp; /* fpo_leaf: inherit caller's frame */"


def find_body(gen: Path) -> tuple[str, str]:
    pattern = rf"^void sub_{ROOT:08X}\(void\)\s*\n\{{.*?^\}}"
    found = []
    for path in sorted(gen.glob("recomp_*.c")):
        found += [
            (path.name, m.group())
            for m in re.finditer(pattern, path.read_text(), re.MULTILINE | re.DOTALL)
        ]
    if len(found) != 1:
        raise ValueError(
            f"expected exactly one lifted definition of {ROOT:08X}, found {len(found)}"
        )
    return found[0]


HOOK_LABELS = (0x278822, 0x27889B)
HOOK_TARGETS = (0x27B716, 0x278B6A, 0x2788B8)


def hook_out(text: str, labels: list[str]) -> str:
    """At each ported label the lifted body calls the Rust port and continues where it says."""
    cases = " ".join(f"case 0x{t:08X}u: goto loc_{t:08X};" for t in HOOK_TARGETS)
    for label in HOOK_LABELS:
        name = f"loc_{label:08X}"
        if name[4:] not in labels or text.count(f"\n{name}: ;\n") != 1:
            raise ValueError(f"hook label {name} not found exactly once")
        call = (
            f"{name}: ;\n    if (g_tick_hook_fn) {{ g_tick_hook_ebp = ebp; "
            f"uint32_t _next = g_tick_hook_fn(0x{label:08X}u); ebp = g_tick_hook_ebp;\n"
            f"    if (_next) switch (_next) {{ {cases} default: abort(); }} }}"
        )
        text = text.replace(f"\n{name}: ;", "\n" + call, 1)
    return text


def resumable(body: str) -> str:
    labels = sorted(set(re.findall(r"^loc_([0-9A-F]{8}): ;", body, re.MULTILINE)))
    if body.count(SEED_LINE) != 1 or not labels:
        raise ValueError("unexpected lifted shape: seed line or labels missing")
    switch = "\n".join(f"    case 0x{label}u: goto loc_{label};" for label in labels)
    inject = (
        f"{SEED_LINE}\n    if (g_tick_resume_entry) {{ uint32_t _entry = g_tick_resume_entry; "
        f"g_tick_resume_entry = 0; ebp = g_tick_resume_ebp;\n    switch (_entry) {{\n{switch}\n    default: abort(); }} }}"
    )
    text = body.replace(SEED_LINE, inject, 1)
    text = text.replace(f"void sub_{ROOT:08X}(void)", "void tsfp_tick_resume(void)", 1)
    return hook_out(text, labels)


TABLES = ((0x27B7A0, 68), (0x27B7F8, 46), (0x520628, 8), (0x520648, 8))


def table_words(xbe: Path) -> str:
    """Retail jump and pointer tables the tick reads, as a private C array (never committed)."""
    import struct

    from tools.harness.image import build_guest_image

    image = build_guest_image(xbe)
    rows = []
    for base, count in TABLES:
        words = struct.unpack("<%dI" % count, image.code_at(base, count * 4))
        rows += [
            f"    {{0x{base + 4 * index:08x}u, 0x{word:08x}u}}," for index, word in enumerate(words)
        ]
    return (
        "static const struct { unsigned address, word; } TICK_TABLES[] = {\n"
        + "\n".join(rows)
        + "\n};\n"
    )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--gen-dir", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--xbe", type=Path, help="retail XBE: also write <out>.tables.inc")
    args = parser.parse_args()
    chunk, body = find_body(args.gen_dir)
    original = body.replace(f"void sub_{ROOT:08X}(void)", f"void tsfp_original_{ROOT:08x}(void)", 1)
    source = (
        '#define RECOMP_GENERATED_CODE\n#include "recomp_funcs.h"\n#include <math.h>\n#include <stdlib.h>\n'
        "uint32_t g_tick_resume_entry, g_tick_resume_ebp, g_tick_hook_ebp;\nuint32_t (*g_tick_hook_fn)(uint32_t);\n"
        + original
        + "\n"
        + resumable(body)
        + "\n"
    )
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(source)
    if args.xbe:
        args.out.with_suffix(".tables.inc").write_text(table_words(args.xbe))
    args.out.with_suffix(".json").write_text(
        json.dumps(
            {
                "schema": 1,
                "root": f"0x{ROOT:08X}",
                "chunk": chunk,
                "body_sha256": hashlib.sha256(body.encode()).hexdigest(),
                "source_sha256": hashlib.sha256(source.encode()).hexdigest(),
            },
            indent=2,
        )
        + "\n"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
