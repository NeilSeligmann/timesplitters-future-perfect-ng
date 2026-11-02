#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""T1831: build and run the Rust-vs-lifted-original differential for the weapon leaves
(tests/c/test_rust_weapons.c) at lifted O0/O3 x Rust O0/O3, plus mutation controls.

The lifted sources are private and ignored, so pass --gen-dir (the MAIN checkout's
generated/lifted/gen from a worktree). Output goes to --out-dir (default tmp/t1831).
"""

# ruff: noqa: E501
from __future__ import annotations

import argparse
import re
import shutil
import subprocess
import sys
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

from tools.datadir import output_path
from tools.play.rust_originals import preserve

ROOTS = (0x1B5660, 0x1B6DB0, 0x1B6790, 0x1B7ED0, 0x1C11C0)
# batch 3 roots: name -> (harness, lifted root, mutated source file, edge dump prefix)
BATCH3 = {
    "1c0200": ("tests/c/test_rust_weapons3.c", (0x1C0200,), "weapons_shot.rs"),
    "1bb5c0": ("tests/c/test_rust_weapons_bb5c0.c", (0x1BB5C0,), "weapons_bb5c0.rs"),
}
RUSTC = (
    "rustc",
    "--edition=2024",
    "--target",
    "x86_64-unknown-linux-gnu",
    "--crate-name",
    "tsfp_ports",
    "--crate-type",
    "staticlib",
    "-Cpanic=abort",
    "-Coverflow-checks=off",
    "-Crelocation-model=pic",
)
# (name, file text to find, replacement). Every mutation must be KILLED by the differential.
MUTATIONS = (
    (
        "b5660 sign test < to <=",
        "regs.eax = if (first as i32) < 0",
        "regs.eax = if (first as i32) <= 0",
    ),
    ("b5660 ecx residue", "regs.ecx = base;", "regs.ecx = index;"),
    (
        "b5660 stride",
        "index.wrapping_mul(0x7c).wrapping_add(base)",
        "index.wrapping_mul(0x78).wrapping_add(base)",
    ),
    ("b6db0 cap compare >= to >", ">= (cap as i32)", "> (cap as i32)"),
    ("b6db0 cap offsets swapped", "0x90 } else { 0x8c }", "0x8c } else { 0x90 }"),
    (
        "b6db0 sext16 to zext16",
        "m.read16(address) as i16 as i32 as u32",
        "m.read16(address) as u32",
    ),
    ("b6db0 fallback constant arg", "&[owner, state, 1, flag]", "&[owner, state, 0, flag]"),
    ("b6db0 fallback arg order", "&[owner, state, 1, flag]", "&[state, owner, 1, flag]"),
    ("b6db0 negative index test", "(index as i32) >= 0", "index != 0xffff_ffff"),
    ("b6db0 callee truth", "passes = r.eax != 0;", "passes = r.eax == 0;"),
    ("b6790 gate polarity", "let mut proceed = r.eax == 0;", "let mut proceed = r.eax != 0;"),
    ("b6790 forwarded arg order", "&[a1, a2, a3, a4, a5]", "&[a1, a2, a3, a5, a4]"),
    ("b6790 esi register input", "r.esi = a1;", "r.esi = entry.esi;"),
    (
        "b6790 suppress global",
        "if r.eax == 0 {\n            let (a2",
        "if r.eax != 0 {\n            let (a2",
    ),
    (
        "b6790 check result",
        "proceed = r.eax != 0;\n    }\n    if proceed",
        "proceed = r.eax == 0;\n    }\n    if proceed",
    ),
    # ---- batch 2: 0x1B7ED0 ----
    (
        "b7ed0 type test",
        "m.read32(weapon.wrapping_add(0x3c)) != 2",
        "m.read32(weapon.wrapping_add(0x3c)) != 3",
    ),
    (
        "b7ed0 definition flag mask",
        "wrapping_add(0xc)) & 0x4000 == 0 { break 'main; }",
        "wrapping_add(0xc)) & 0x8000 == 0 { break 'main; }",
    ),
    ("b7ed0 entry index is alt", "ecx = alt.wrapping_mul(0x7c);", "ecx = mode.wrapping_mul(0x7c);"),
    (
        "b7ed0 bit7 mask",
        "(entry_flags as u8) & 0x80 != 0 && flag == 1",
        "(entry_flags as u8) & 0x40 != 0 && flag == 1",
    ),
    ("b7ed0 flag==1 to !=0", "&& flag == 1 { break 'body 0; }", "&& flag != 0 { break 'body 0; }"),
    ("b7ed0 armed zero test", "if ecx == 0 { armed = 1; }", "if ecx != 0 { armed = 1; }"),
    (
        "b7ed0 cooldown select",
        "if alt != 0 { 0xf4 } else { 0xf0 }",
        "if alt != 0 { 0xf0 } else { 0xf4 }",
    ),
    (
        "b7ed0 rate select",
        "if flag != 0 { 0x14 } else { 0x10 }",
        "if flag != 0 { 0x10 } else { 0x14 }",
    ),
    (
        "b7ed0 round instead of truncate",
        "sse::cvttss2si(lane0(&xmm0));\n        let cost",
        "((lane0(&xmm0) + 0.5) as i32) as u32;\n        let cost",
    ),
    (
        "b7ed0 unsigned frame count",
        "int_to_float(m.read32(0x4e8cec)).to_bits();\n        m.write32(at(0x14)",
        "(m.read32(0x4e8cec) as f32).to_bits();\n        m.write32(at(0x14)",
    ),
    (
        "b7ed0 primary token swap",
        "if token_in_primary { slot30 } else { slot28 }",
        "if token_in_primary { slot28 } else { slot30 }",
    ),
    ("b7ed0 secondary token", "m.write32(at(0x24), ecx);", "m.write32(at(0x24), edi);"),
    ("b7ed0 player mode word", "!= 1 { break 'to_80e6; }", "!= 2 { break 'to_80e6; }"),
    ("b7ed0 held offsets swapped", "{ 0x61e } else { 0xdc }", "{ 0xdc } else { 0x61e }"),
    ("b7ed0 count > 0 to >= 0", "as i32 > 0", "as i32 >= 0"),
    (
        "b7ed0 token compare > to >=",
        "(slot28 as i32) > (edx as i32)",
        "(slot28 as i32) >= (edx as i32)",
    ),
    (
        "b7ed0 5A0 test polarity",
        "m.read32(eax.wrapping_add(0x5a0)) != 1 || edi == 0",
        "m.read32(eax.wrapping_add(0x5a0)) == 1 || edi == 0",
    ),
    ("b7ed0 edi test polarity", "!= 1 || edi == 0", "!= 1 || edi != 0"),
    (
        "b7ed0 half rounds down",
        "((v.wrapping_sub(sign) as i32) >> 1) as u32",
        "((v.wrapping_sub(0 * sign) as i32) >> 1) as u32",
    ),
    ("b7ed0 clamp value", "if (eax as i32) >= 1 { eax = 1; }", "if (eax as i32) >= 1 { eax = 2; }"),
    (
        "b7ed0 shortcut cooldown <=",
        "if flag != 0 { break 'general; }\n            if (cooldown as i32) < (ecx as i32)",
        "if flag != 0 { break 'general; }\n            if (cooldown as i32) <= (ecx as i32)",
    ),
    (
        "b7ed0 general cooldown <=",
        "if (cooldown as i32) < (ecx as i32) { break 'body 0; }\n        if ebp != 0 {\n            eax = half",
        "if (cooldown as i32) <= (ecx as i32) { break 'body 0; }\n        if ebp != 0 {\n            eax = half",
    ),
    (
        "b7ed0 token mismatch polarity",
        "if esi != token_flag { break 'body 0; }",
        "if esi == token_flag { break 'body 0; }",
    ),
    (
        "b7ed0 shortcut accept polarity",
        "if esi == token_flag { break 'body 1; }",
        "if esi != token_flag { break 'body 1; }",
    ),
    (
        "b7ed0 ebp restore after refine",
        "            ebp = armed;\n        }",
        "            ebp = 0;\n        }",
    ),
    ("b7ed0 clear keeps flag", "if clear { token_flag = 0;", "if clear { token_flag = 1;"),
    (
        "b7ed0 ammo kind offset",
        "let kind = m.read32(definition.wrapping_add(0x14));\n        ecx = ecx.wrapping_mul(0x3c);",
        "let kind = m.read32(definition.wrapping_add(0x18));\n        ecx = ecx.wrapping_mul(0x3c);",
    ),
    ("b7ed0 secondary count offset", "count(m, 0x20c)", "count(m, 0xdc)"),
    (
        "b7ed0 half token index",
        "ecx = alt;\n            edx = token_at(m, ecx);",
        "ecx = alt;\n            edx = token_at(m, 0);",
    ),
    # ---- batch 2: 0x1C11C0 ----
    (
        "c11c0 entry select sign test",
        "if (first as i32) < 0 { definition",
        "if (first as i32) <= 0 { definition",
    ),
    (
        "c11c0 entry index field",
        "select_entry(m, definition, m.read32(record.wrapping_add(0x5c)))",
        "select_entry(m, definition, m.read32(record.wrapping_add(0x58)))",
    ),
    (
        "c11c0 dt operand order",
        "sse::divss(lane0(&r.xmm[0]), lane0(&r.xmm[2]))",
        "sse::divss(lane0(&r.xmm[2]), lane0(&r.xmm[0]))",
    ),
    ("c11c0 dt scale address", "float(m, 0x4e794c)", "float(m, 0x4e7948)"),
    (
        "c11c0 unit timer store",
        "m.write32(record.wrapping_add(0xbc), r.xmm[1].lanes[0]);",
        "m.write32(record.wrapping_add(0xc0), r.xmm[1].lanes[0]);",
    ),
    ("c11c0 42760 argument", "&[player_word]", "&[player_word ^ 1]"),
    (
        "c11c0 dt slot store",
        "m.write32(e.wrapping_sub(4), r.xmm[0].lanes[0]); // dt overwrites",
        "let _ = 0; // dt overwrites",
    ),
    ("c11c0 pending test polarity", "if r.ecx & r.eax != 0 {", "if r.ecx & r.eax == 0 {"),
    (
        "c11c0 unordered clamp",
        "if !(lane0(&r.xmm[0]) >= lane0(&r.xmm[1])) { break 'timers_done; }",
        "if lane0(&r.xmm[0]) < lane0(&r.xmm[1]) { break 'timers_done; }",
    ),
    (
        "c11c0 dt read slot",
        "float(m, e.wrapping_sub(4))).to_bits();\n                r.xmm[1]",
        "float(m, e.wrapping_sub(8))).to_bits();\n                r.xmm[1]",
    ),
    (
        "c11c0 player sign test",
        "if (r.edx as i32) < 0 { break 'no_definition; }",
        "if (r.edx as i32) <= 0 { break 'no_definition; }",
    ),
    ("c11c0 player E84 field", "wrapping_add(0xe84)", "wrapping_add(0xe80)"),
    ("c11c0 state constant", "r.eax = 0x2d;", "r.eax = 0x2e;"),
    (
        "c11c0 F8 test polarity",
        "if m.read32(record.wrapping_add(0xf8)) != r.eax { break 'exit; }",
        "if m.read32(record.wrapping_add(0xf8)) == r.eax { break 'exit; }",
    ),
    (
        "c11c0 pending 44 polarity",
        "if r.eax != 0 {\n                    r.edx = m.read32(record.wrapping_add(0x3cc));",
        "if r.eax == 0 {\n                    r.edx = m.read32(record.wrapping_add(0x3cc));",
    ),
    ("c11c0 first 12F70 arg order", "[0x12, record, r.edx, 0]", "[0x12, r.edx, record, 0]"),
    ("c11c0 second 12F70 selector", "[0x11, record, r.ecx, ebp]", "[0x10, record, r.ecx, ebp]"),
    ("c11c0 flag word or", "(r.eax & 0xffff_fff7) | 6", "(r.eax & 0xffff_fff7) | 7"),
    ("c11c0 flag word mask", "(r.eax & 0xffff_fff7) | 6", "(r.eax & 0xffff_ffff) | 6"),
    ("c11c0 new state", "wrapping_add(0xe8), 0x2e", "wrapping_add(0xe8), 0x2f"),
    (
        "c11c0 timers cleared",
        "m.write32(record.wrapping_add(0xf4), 0);",
        "m.write32(record.wrapping_add(0xf4), 1);",
    ),
    ("c11c0 clear bit0", "r.eax &= 0xffff_fffe;", "r.eax &= 0xffff_fffd;"),
    ("c11c0 counter step address", "r.edx = m.read32(0x4e7944);", "r.edx = m.read32(0x4e7948);"),
    (
        "c11c0 counter step 1355",
        "r.eax = m.read32(0x4e7944);\n            let total",
        "r.eax = m.read32(0x4e7948);\n            let total",
    ),
    (
        "c11c0 AC test polarity",
        "r.eax = m.read32(record.wrapping_add(0xa8));\n        if r.ecx != 0 { break 'exit; }",
        "r.eax = m.read32(record.wrapping_add(0xa8));\n        if r.ecx == 0 { break 'exit; }",
    ),
    (
        "c11c0 threshold float field",
        "float(m, r.edi.wrapping_add(0xc))",
        "float(m, r.edi.wrapping_add(0x10))",
    ),
    (
        "c11c0 shot compare > ",
        "if !(lane0(&r.xmm[0]) >= lane0(&r.xmm[1])) { break 'exit; }",
        "if !(lane0(&r.xmm[0]) > lane0(&r.xmm[1])) { break 'exit; }",
    ),
    (
        "c11c0 step minimum",
        "(m.read32(0x4e7944) as i32) < (ebp as i32)",
        "(m.read32(0x4e7944) as i32) <= (ebp as i32)",
    ),
    ("c11c0 1C0200 argument", "&[owner]", "&[record]"),
    (
        "c11c0 pending float store",
        "m.write32(record.wrapping_add(0x48), 0);",
        "m.write32(record.wrapping_add(0x4c), 0);",
    ),
    (
        "c11c0 pending flag store",
        "m.write32(record.wrapping_add(0x44), ebp);",
        "m.write32(record.wrapping_add(0x44), 0);",
    ),
    (
        "c11c0 xmm0 zero before 1C0200",
        "r.xmm[0] = crate::abi::Xmm::default();\n        m.write32(record.wrapping_add(0x48), 0);",
        "m.write32(record.wrapping_add(0x48), 0);",
    ),
    ("c11c0 ecx pop", "r.ecx = m.read32(base.wrapping_add(12));", "r.ecx = r.ecx.wrapping_add(0);"),
    (
        "c11c0 pending store B8",
        "m.write32(record.wrapping_add(0xb8), ebp);",
        "m.write32(record.wrapping_add(0xb8), 0);",
    ),
    (
        "c11c0 definition word check",
        "== 0xffff_ffff { break 'no_definition; }\n                r.ecx = m.read32(record.wrapping_add(0xe8));",
        "== 0 { break 'no_definition; }\n                r.ecx = m.read32(record.wrapping_add(0xe8));",
    ),
)


# ---- batch 3: 0x1C0200 (target crates/tsfp-ports/src/weapons_shot.rs). Optional 4th item: expected
# number of anchor matches, all replaced together.
MUTATIONS3 = (
    (
        "c0200 entry stride",
        "(eax as i32 as i64) * (0x7C as u32 as i32 as i64)",
        "(eax as i32 as i64) * (0x78 as u32 as i32 as i64)",
    ),
    (
        "c0200 definition stride",
        "edx = ecx;\n                { let product = (edx as i32 as i64) * (0x29C",
        "edx = ecx;\n                { let product = (edx as i32 as i64) * (0x298",
    ),
    (
        "c0200 base entry fallback offset",
        "ebp = edx.wrapping_add(0x68);\n                pc = 0x1c023d",
        "ebp = edx.wrapping_add(0x6C);\n                pc = 0x1c023d",
    ),
    (
        "c0200 negative entry test",
        "if (fa.wrapping_sub(fb) >> 31) == 0 { pc = 0x1c023d;",
        "if (fa.wrapping_sub(fb) >> 31) != 0 { pc = 0x1c023d;",
    ),
    (
        "c0200 player eye object offset",
        "edx = m.read32(edx.wrapping_add(0x7C));\n                m.write32(esp.wrapping_add(0x44), edx);",
        "edx = m.read32(edx.wrapping_add(0x78));\n                m.write32(esp.wrapping_add(0x44), edx);",
    ),
    (
        "c0200 shots field",
        "edx = m.read32(ebp.wrapping_add(8));",
        "edx = m.read32(ebp.wrapping_add(0xC));",
    ),
    ("c0200 shooter id 6 test", "cmp!(ecx, 6);", "cmp!(ecx, 7);"),
    (
        "c0200 burst increment",
        "eax = eax.wrapping_add(1);\n                result!(eax);\n                fb = (fa == 0x8000_0000) as u32;",
        "eax = eax.wrapping_add(2);\n                result!(eax);\n                fb = (fa == 0x8000_0000) as u32;",
    ),
    (
        "c0200 burst counter clear",
        "m.write32(ebx.wrapping_add(0xC0), esi);",
        "m.write32(ebx.wrapping_add(0xC0), 1);",
    ),
    (
        "c0200 gate sign test",
        "cmp!(ecx, esi);\n                if fas < fbs { pc = 0x1c029c;",
        "cmp!(ecx, esi);\n                if fas <= fbs { pc = 0x1c029c;",
    ),
    (
        "c0200 gate flag bit",
        "cmp8!((m.read8(edx.wrapping_add(0x21C)) as u32), (4));",
        "cmp8!((m.read8(edx.wrapping_add(0x21C)) as u32), (8));",
    ),
    (
        "c0200 record reset marker",
        "m.write32(ebx.wrapping_add(0x68), 1);",
        "m.write32(ebx.wrapping_add(0x68), 2);",
    ),
    (
        "c0200 record reset field 0x74",
        "m.write32(ebx.wrapping_add(0x74), x[0].lanes[0]);",
        "m.write32(ebx.wrapping_add(0x78), x[0].lanes[0]);",
    ),
    (
        "c0200 shooter id 0xA jitter test",
        "cmp!(m.read32(edi.wrapping_add(0x30)), 0xA);\n                if fa != fb { pc = 0x1c0371;",
        "cmp!(m.read32(edi.wrapping_add(0x30)), 0xB);\n                if fa != fb { pc = 0x1c0371;",
    ),
    ("c0200 unsigned fixup constant", "0x475CCC", "0x475CD0", 5),
    ("c0200 rand scale constant", "0x4785D0", "0x4785E4", 5),
    (
        "c0200 accuracy reset value",
        "m.write32(ebx.wrapping_add(0x9C), x[0].lanes[0]);",
        "m.write32(ebx.wrapping_add(0x98), x[0].lanes[0]);",
    ),
    ("c0200 recoil table base", ".wrapping_add(0x4ED3C0)", ".wrapping_add(0x4ED3C8)"),
    ("c0200 recoil table stride", "eax.wrapping_mul(8)", "eax.wrapping_mul(4)"),
    (
        "c0200 recoil entry field",
        "eax = m.read32(ebp.wrapping_add(0x64));",
        "eax = m.read32(ebp.wrapping_add(0x60));",
    ),
    (
        "c0200 recoil store field",
        "m.write32(ebx.wrapping_add(0xC4), ecx);",
        "m.write32(ebx.wrapping_add(0xC8), ecx);",
    ),
    (
        "c0200 1B7260 argument order",
        "push!(edx);\n                push!(edi);\n                m.write32(ebx.wrapping_add(0xC4), ecx);",
        "push!(edi);\n                push!(edx);\n                m.write32(ebx.wrapping_add(0xC4), ecx);",
    ),
    (
        "c0200 1BB2F0 argument order",
        "push!(eax);\n                push!(ebx);\n                push!(ecx);\n                call!(0x1bb2f0);",
        "push!(ebx);\n                push!(eax);\n                push!(ecx);\n                call!(0x1bb2f0);",
    ),
    (
        "c0200 1BB2F0 condition",
        "cmp!(eax, 0);\n                if fa == fb { pc = 0x1c042e;",
        "cmp!(eax, 0);\n                if fa != fb { pc = 0x1c042e;",
    ),
    (
        "c0200 early exit id",
        "cmp!(m.read32(ebx.wrapping_add(0x30)), 0x1B);",
        "cmp!(m.read32(ebx.wrapping_add(0x30)), 0x1C);",
    ),
    (
        "c0200 early exit fired flag",
        "m.write32(ebx.wrapping_add(0xAC), 1);\n                ebp = m.read32(esp);",
        "m.write32(ebx.wrapping_add(0xAC), 0);\n                ebp = m.read32(esp);",
    ),
    ("c0200 flag word mask", "cmp!(eax, 0x80000);", "cmp!(eax, 0x40000);"),
    ("c0200 effect call selector 3", "push!(3);", "push!(4);"),
    ("c0200 effect call selector 8", "push!(8);", "push!(9);"),
    (
        "c0200 shot loop exit compare",
        "if fas <= fbs { pc = 0x1c0e77;",
        "if fas < fbs { pc = 0x1c0e77;",
    ),
    (
        "c0200 shot loop back edge",
        "if fas < fbs { pc = 0x1c04e0;",
        "if fas <= fbs { pc = 0x1c04e0;",
    ),
    (
        "c0200 shot index start",
        "m.write32(esp.wrapping_add(0x48), 0);",
        "m.write32(esp.wrapping_add(0x48), 1);",
    ),
    ("c0200 spread constant 0.3", "push!(0x3E99999A);", "push!(0x3E99999B);"),
    ("c0200 spread constant 0.7", "push!(0x3F333333);", "push!(0x3F333334);"),
    (
        "c0200 spread argument field",
        "eax = edi.wrapping_add(0x114);",
        "eax = edi.wrapping_add(0x118);",
    ),
    (
        "c0200 spread second factor",
        "fl(m.read32(esp.wrapping_add(0x9C)))",
        "fl(m.read32(esp.wrapping_add(0x98)))",
    ),
    (
        "c0200 jitter flag bit",
        "cmp8!(((eax >> 8) & 0xff), (0x10));",
        "cmp8!(((eax >> 8) & 0xff), (0x20));",
    ),
    (
        "c0200 spread modifier bit",
        "cmp8!((m.read8(ebp) as u32), (2));",
        "cmp8!((m.read8(ebp) as u32), (4));",
    ),
    ("c0200 weapon id 0x22 spread", "cmp!(ecx, 0x22);", "cmp!(ecx, 0x23);"),
    ("c0200 spread add constant", "0x475CF8", "0x475CF4"),
    ("c0200 owner scale constant", "0x4765E0", "0x4765F4", 2),
    ("c0200 normalise threshold", "0x4765F4", "0x4765E0", 4),
    ("c0200 rsqrt numerator", "sse::divss(1.0, xsqrt::ss(", "sse::divss(2.0, xsqrt::ss(", 3),
    (
        "c0200 x87 reciprocal direction",
        "fl(m.read32(0x475C78)) as f64 / st(&fpu, 0)",
        "st(&fpu, 0) / fl(m.read32(0x475C78)) as f64",
        2,
    ),
    ("c0200 x87 precision control", "if f.control & 0x300 != 0 { v } else { round24(v) }", "v"),
    ("c0200 x87 wide-range rounding", "mantissa * pow2(e)", "mantissa * pow2(e + 1)"),
    (
        "c0200 spawn view field",
        "m.write32(eax.wrapping_add(0xF60), edx);",
        "m.write32(eax.wrapping_add(0xF64), edx);",
    ),
    (
        "c0200 spawn argument order",
        "push!(eax);\n                push!(esi);\n                push!(1);",
        "push!(esi);\n                push!(eax);\n                push!(1);",
    ),
    ("c0200 spawn mask", "edx = (edx & ebx);", "edx = (edx & edi);"),
    ("c0200 spawn carry polarity", "cf = edx != 0;", "cf = edx == 0;"),
    ("c0200 11F780 argument", "edx = esi.wrapping_add(0xC68);", "edx = esi.wrapping_add(0xC6C);"),
    (
        "c0200 float compare constant",
        "fpu.push(fl(m.read32(0x475C78)) as f64);\n                source!(0x38);",
        "fpu.push(fl(m.read32(0x475CD4)) as f64);\n                source!(0x38);",
    ),
    (
        "c0200 lahf equal bit",
        "if fpcmp == 0 || fpcmp == 2 { 0x40u32 }",
        "if fpcmp == 0 { 0x40u32 }",
    ),
    (
        "c0200 parity polarity",
        "if !parity8(fa & fb) { pc = 0x1c0bc5;",
        "if parity8(fa & fb) { pc = 0x1c0bc5;",
    ),
    (
        "c0200 refire definition field",
        "fl(m.read32(ecx.wrapping_add(0x298)))",
        "fl(m.read32(ecx.wrapping_add(0x294)))",
    ),
    (
        "c0200 refire store field",
        "m.write32(edi.wrapping_add(0x110), eax);",
        "m.write32(edi.wrapping_add(0x114), eax);",
    ),
    ("c0200 refire compare", "if fas > fbs { pc = 0x1c0eb8;", "if fas >= fbs { pc = 0x1c0eb8;"),
    (
        "c0200 1C0110 argument order",
        "push!(ebp);\n                push!(ecx);\n                push!(edi);\n                push!(ebx);",
        "push!(ecx);\n                push!(ebp);\n                push!(edi);\n                push!(ebx);",
    ),
    (
        "c0200 refire clear",
        "m.write32(edi.wrapping_add(0x110), 0);",
        "m.write32(edi.wrapping_add(0x110), 1);",
    ),
    (
        "c0200 ammo take count",
        "push!(1);\n                eax = esp.wrapping_add(0x70);",
        "push!(2);\n                eax = esp.wrapping_add(0x70);",
    ),
    (
        "c0200 ammo take record field",
        "ecx = m.read32(ebx.wrapping_add(0x60));",
        "ecx = m.read32(ebx.wrapping_add(0x64));",
    ),
    (
        "c0200 fired flag",
        "eax = m.read32(ebx.wrapping_add(0x30));\n                source!(0x1C);",
        "eax = m.read32(ebx.wrapping_add(0x34));\n                source!(0x1C);",
    ),
    (
        "c0200 pending state value",
        "m.write32(ebx.wrapping_add(0x44), 5);",
        "m.write32(ebx.wrapping_add(0x44), 4);",
    ),
    (
        "c0200 pending timer constant",
        "x[0] = Xmm::scalar(m.read32(0x475D04));",
        "x[0] = Xmm::scalar(m.read32(0x475D08));",
    ),
    ("c0200 hook event 9", "push!(9);", "push!(8);"),
    ("c0200 hook event 7", "push!(7);", "push!(6);"),
    (
        "c0200 hook definition field",
        "m.read32(eax.wrapping_add(0x294))",
        "m.read32(eax.wrapping_add(0x290))",
        2,
    ),
    ("c0200 effect list step", "edi = edi.wrapping_add(4);", "edi = edi.wrapping_add(8);", 2),
    ("c0200 default effect object", "0x7AD288", "0x7AD28C", 3),
    ("c0200 mask inversion", "eax = (!eax);", "eax = eax;", 2),
    (
        "c0200 zero-count shift flags",
        "if fa == fb { 0 } else if (fa.wrapping_sub(fb) >> 31) != 0 { 0x8000_0000 } else { 1 }",
        "if fa == fb { 0 } else if (fa.wrapping_sub(fb) >> 31) != 0 { 1 } else { 1 }",
    ),
    ("c0200 shift count mask", "let count = ecx & 31;", "let count = ecx & 15;"),
)

# ---- batch 3b: 0x1BB5C0 (target crates/tsfp-ports/src/weapons_bb5c0.rs).
MUTATIONS_BB5C0 = (
    ("bb5c0 embedded object A", "edi = esi.wrapping_add(0xA4);", "edi = esi.wrapping_add(0xA8);"),
    ("bb5c0 embedded object B", "edi = esi.wrapping_add(0x1D4);", "edi = esi.wrapping_add(0x1D8);"),
    (
        "bb5c0 object select polarity",
        "cmp!(eax, 0);\n                push!(ebx);\n                ebx = m.read32(esp.wrapping_add(0xE8));\n                push!(esi);\n                esi = m.read32(ebx.wrapping_add(0x7C));\n                push!(edi);\n                edi = esi.wrapping_add(0xA4);\n                if fa == fb {",
        "cmp!(eax, 0);\n                push!(ebx);\n                ebx = m.read32(esp.wrapping_add(0xE8));\n                push!(esi);\n                esi = m.read32(ebx.wrapping_add(0x7C));\n                push!(edi);\n                edi = esi.wrapping_add(0xA4);\n                if fa != fb {",
    ),
    (
        "bb5c0 controller field",
        "esi = m.read32(ebx.wrapping_add(0x7C));",
        "esi = m.read32(ebx.wrapping_add(0x78));",
    ),
    (
        "bb5c0 target null exit",
        "ecx = m.read32(esi.wrapping_add(4));\n                cmp!(ecx, 0);\n                if fa == fb { pc = 0x1bc643;",
        "ecx = m.read32(esi.wrapping_add(4));\n                cmp!(ecx, 0);\n                if fa != fb { pc = 0x1bc643;",
    ),
    (
        "bb5c0 entity field null exit",
        "ecx = m.read32(ebx.wrapping_add(0x34));\n                cmp!(ecx, 0);\n                if fa == fb { pc = 0x1bc643;",
        "ecx = m.read32(ebx.wrapping_add(0x34));\n                cmp!(ecx, 0);\n                if fa != fb { pc = 0x1bc643;",
    ),
    (
        "bb5c0 tick minimum",
        "cmp!(m.read32(0x4E7944), 1);\n                if fas < fbs {",
        "cmp!(m.read32(0x4E7944), 1);\n                if fas <= fbs {",
    ),
    (
        "bb5c0 weapon sentinel",
        "cmp!(m.read32(edi.wrapping_add(0x30)), 0xFFFFFFFF);\n                if fa == fb { pc = 0x1bc643;",
        "cmp!(m.read32(edi.wrapping_add(0x30)), 0xFFFFFFFE);\n                if fa == fb { pc = 0x1bc643;",
    ),
    (
        "bb5c0 gate field polarity",
        "ecx = m.read32(ebx.wrapping_add(0x150));\n                cmp!(ecx, 0);\n                if fa != fb {",
        "ecx = m.read32(ebx.wrapping_add(0x150));\n                cmp!(ecx, 0);\n                if fa == fb {",
    ),
    (
        "bb5c0 35DE60 argument order",
        "push!(ecx);\n                push!(edx);\n                push!(eax);\n                push!(ebx);\n                call!(0x35de60);",
        "push!(edx);\n                push!(ecx);\n                push!(eax);\n                push!(ebx);\n                call!(0x35de60);",
    ),
    (
        "bb5c0 flag bit 0x1000",
        "cmp8!(((eax >> 8) & 0xff), (0x10));",
        "cmp8!(((eax >> 8) & 0xff), (0x20));",
    ),
    (
        "bb5c0 124F0 argument",
        "m.write32(esp, x[3].lanes[0]);\n                call!(0x124f0);",
        "m.write32(esp, x[2].lanes[0]);\n                call!(0x124f0);",
    ),
    (
        "bb5c0 target kind",
        "cmp!(m.read32(ecx.wrapping_add(0x20)), 8);",
        "cmp!(m.read32(ecx.wrapping_add(0x20)), 9);",
    ),
    (
        "bb5c0 time compare",
        "cmp!(m.read32(esi.wrapping_add(8)), edx);\n                if fas < fbs {",
        "cmp!(m.read32(esi.wrapping_add(8)), edx);\n                if fas <= fbs {",
    ),
    ("bb5c0 flag mask 0x100", "ebp = 0x100;", "ebp = 0x200;"),
    (
        "bb5c0 0xFFFF test entity",
        "cmp!(m.read32(eax.wrapping_add(0x10)), 0xFFFF);",
        "cmp!(m.read32(eax.wrapping_add(0x10)), 0xFFFE);",
    ),
    (
        "bb5c0 0xFFFF test target",
        "cmp!(m.read32(ecx.wrapping_add(0x10)), 0xFFFF);",
        "cmp!(m.read32(ecx.wrapping_add(0x10)), 0xFFFE);",
    ),
    (
        "bb5c0 second object field",
        "eax = m.read32(ecx.wrapping_add(0x7C));",
        "eax = m.read32(ecx.wrapping_add(0x78));",
    ),
    (
        "bb5c0 difficulty byte",
        "cmp8!((eax & 0xff), (0xA));\n                if fa == fb { pc = 0x1bba77;",
        "cmp8!((eax & 0xff), (0xB));\n                if fa == fb { pc = 0x1bba77;",
    ),
    ("bb5c0 entry id sentinel", "cmp!(eax, 0x15);", "cmp!(eax, 0x16);"),
    ("bb5c0 flag word compare", "cmp!(eax, 0x802);", "cmp!(eax, 0x801);"),
    (
        "bb5c0 burst selector 8",
        "push!(eax);\n                push!(8);\n                call!(0x238510);",
        "push!(eax);\n                push!(7);\n                call!(0x238510);",
    ),
    ("bb5c0 burst selector 9", "push!(9);", "push!(10);"),
    (
        "bb5c0 burst loop compare",
        "m.write32(esp.wrapping_add(0x10), eax);\n                if fas < fbs { pc = 0x1bb9e0;",
        "m.write32(esp.wrapping_add(0x10), eax);\n                if fas <= fbs { pc = 0x1bb9e0;",
    ),
    (
        "bb5c0 reset field F0",
        "m.write32(edi.wrapping_add(0xF0), eax);",
        "m.write32(edi.wrapping_add(0xF8), eax);",
        2,
    ),
    (
        "bb5c0 reset field F4",
        "m.write32(edi.wrapping_add(0xF4), eax);",
        "m.write32(edi.wrapping_add(0xFC), eax);",
        2,
    ),
    (
        "bb5c0 accumulator field",
        "m.write32(edi.wrapping_add(0x104), x[1].lanes[0]);",
        "m.write32(edi.wrapping_add(0x108), x[1].lanes[0]);",
    ),
    ("bb5c0 frame step constant", "fl(m.read32(0x4E794C))", "fl(m.read32(0x4E7948))"),
    (
        "bb5c0 frame count constant",
        "(m.read32(0x4E8CEC) as i32 as f32)",
        "(m.read32(0x4E8CE8) as i32 as f32)",
    ),
    ("bb5c0 normalise threshold", "fl(m.read32(0x4765F4))", "fl(m.read32(0x4765E0))", 8),
    ("bb5c0 rsqrt numerator", "sse::divss(1.0, xsqrt::ss(", "sse::divss(2.0, xsqrt::ss(", 8),
    ("bb5c0 unsigned fixup", "fl(m.read32(0x475CCC))", "fl(m.read32(0x475CD0))", 8),
    ("bb5c0 rand scale", "fl(m.read32(0x4785D0))", "fl(m.read32(0x4785E4))", 8),
    (
        "bb5c0 rand cast",
        "fpu.push(m.read32(esp.wrapping_add(0x4C)) as i32 as f64);",
        "fpu.push(m.read32(esp.wrapping_add(0x4C)) as f64);",
        7,
    ),
    (
        "bb5c0 fp compare order",
        "fpcmp = fcmp(st(&fpu, 0), st(&fpu, 1));",
        "fpcmp = fcmp(st(&fpu, 1), st(&fpu, 0));",
    ),
    (
        "bb5c0 lahf equal bit",
        "if fpcmp == 0 || fpcmp == 2 { 0x40u32 }",
        "if fpcmp == 0 { 0x40u32 }",
    ),
    (
        "bb5c0 parity polarity",
        "if !parity8(fa & fb) { pc = 0x1bc4d4;",
        "if parity8(fa & fb) { pc = 0x1bc4d4;",
    ),
    (
        "bb5c0 final weapon test",
        "cmp!(m.read32(edi.wrapping_add(0x30)), 0xA);\n                if fa != fb { pc = 0x1bba85;",
        "cmp!(m.read32(edi.wrapping_add(0x30)), 0xB);\n                if fa != fb { pc = 0x1bba85;",
    ),
    (
        "bb5c0 entry list field",
        "eax = m.read32(ebx.wrapping_add(0x74));",
        "eax = m.read32(ebx.wrapping_add(0x70));",
    ),
    (
        "bb5c0 entry list step field",
        "edx = m.read32(ebx.wrapping_add(0x74));",
        "edx = m.read32(ebx.wrapping_add(0x70));",
    ),
    ("bb5c0 list step", "esi = esi.wrapping_add(4);", "esi = esi.wrapping_add(8);"),
    ("bb5c0 default effect object", "eax = m.read32(0x7AD288);", "eax = m.read32(0x7AD28C);"),
    (
        "bb5c0 list bound compare",
        "cmp!(eax, m.read32(ecx.wrapping_add(0x1E8)));\n                if fas >= fbs {",
        "cmp!(eax, m.read32(ecx.wrapping_add(0x1E8)));\n                if fas > fbs {",
    ),
    (
        "bb5c0 list array scale",
        "eax = m.read32(ecx.wrapping_add(eax.wrapping_mul(4)));",
        "eax = m.read32(ecx.wrapping_add(eax.wrapping_mul(8)));",
    ),
    (
        "bb5c0 list array field",
        "ecx = m.read32(ecx.wrapping_add(0x1FC));",
        "ecx = m.read32(ecx.wrapping_add(0x1F8));",
    ),
    (
        "bb5c0 fired return value",
        "eax = 1;\n                ebx = m.read32(esp); esp = esp.wrapping_add(4);",
        "eax = 2;\n                ebx = m.read32(esp); esp = esp.wrapping_add(4);",
        2,
    ),
    (
        "bb5c0 early exit return value",
        "eax = 0;\n                result!(eax);\n                ebx = m.read32(esp); esp = esp.wrapping_add(4);",
        "eax = 1;\n                result!(eax);\n                ebx = m.read32(esp); esp = esp.wrapping_add(4);",
        2,
    ),
    ("bb5c0 spawn marker", "push!(0x2000058);", "push!(0x2000059);"),
    (
        "bb5c0 1B7680 stack argument",
        "eax = m.read32(esp.wrapping_add(0x108));",
        "eax = m.read32(esp.wrapping_add(0x10C));",
    ),
    (
        "bb5c0 DFEC0 sentinel",
        "push!(0xFFFFFFFF);\n                ecx = esp.wrapping_add(0x18);",
        "push!(0xFFFFFFFE);\n                ecx = esp.wrapping_add(0x18);",
    ),
    (
        "bb5c0 ammo take count",
        "push!(1);\n                ecx = esp.wrapping_add(0x80);",
        "push!(2);\n                ecx = esp.wrapping_add(0x80);",
    ),
    ("bb5c0 flag bit 4", "cmp8!(((eax >> 8) & 0xff), (4));", "cmp8!(((eax >> 8) & 0xff), (8));"),
    (
        "bb5c0 flag bits 0xE",
        "cmp8!(((eax >> 8) & 0xff), (0xE));",
        "cmp8!(((eax >> 8) & 0xff), (0xF));",
    ),
    ("bb5c0 flag bit 2", "cmp8!(((eax >> 8) & 0xff), (2));", "cmp8!(((eax >> 8) & 0xff), (4));"),
    (
        "bb5c0 flag 0x20000000",
        "cmp!(m.read32(esi.wrapping_add(0x2C)), 0x20000000);",
        "cmp!(m.read32(esi.wrapping_add(0x2C)), 0x10000000);",
    ),
    (
        "bb5c0 table compare",
        "cmp!(m.read32(edx.wrapping_mul(4).wrapping_add(0x4ECA78)), 1);",
        "cmp!(m.read32(edx.wrapping_mul(4).wrapping_add(0x4ECA78)), 2);",
    ),
    (
        "bb5c0 table row stride",
        "edx = eax.wrapping_add(eax.wrapping_mul(8));",
        "edx = eax.wrapping_add(eax.wrapping_mul(4));",
    ),
    (
        "bb5c0 int16 sign extension",
        "m.read16(esi.wrapping_add(0x48)) as i16 as i32 as u32",
        "m.read16(esi.wrapping_add(0x48)) as u32",
    ),
    (
        "bb5c0 row stride x3",
        "eax = eax.wrapping_add(eax.wrapping_mul(2));",
        "eax = eax.wrapping_add(eax.wrapping_mul(1));",
    ),
    (
        "bb5c0 row stride shift",
        "eax = (eax << (((4) & 31) & 31));",
        "eax = (eax << (((3) & 31) & 31));",
    ),
    ("bb5c0 table base", "0x53A3E4", "0x53A3E8"),
    ("bb5c0 byte flag 7DE457", "m.read8(0x7DE457)", "m.read8(0x7DE456)"),
    ("bb5c0 aim threshold constant", "fl(m.read32(0x47D670))", "fl(m.read32(0x47D66C))", 2),
    (
        "bb5c0 lower bound constant",
        "x[0] = Xmm::scalar(m.read32(0x47D55C));",
        "x[0] = Xmm::scalar(m.read32(0x47D558));",
    ),
    (
        "bb5c0 hysteresis constants",
        "x[0] = Xmm::scalar(m.read32(0x47E2B4));",
        "x[0] = Xmm::scalar(m.read32(0x47E2B8));",
    ),
    ("bb5c0 speed limit constant", "fl(m.read32(0x47E2B8))", "fl(m.read32(0x47E2B4))"),
    (
        "bb5c0 reciprocal gate",
        "x[0] = Xmm::scalar(m.read32(0x475D1C));",
        "x[0] = Xmm::scalar(m.read32(0x475D18));",
    ),
    ("bb5c0 abs mask", "eax = (eax & 0x7FFFFFFF);", "eax = (eax & 0xFFFFFFFF);"),
)


# Mutants proven equivalent by reading the code (not by the differential): the zero-count `shl`
# only rewrites flags and every `shl eax, cl` in 0x1C0200 is followed by an instruction that sets
# new flags (ADD/CALL/PUSH chains) before any conditional jump, so ZF/SF there are dead.
EQUIVALENT_BB5C0 = {
    "bb5c0 lahf equal bit": "NOT EQUIVALENT in general: differs only for an unordered compare, and this fixture is NaN-free by design (see the harness comment on fchs sign mixing), so the mutant is unreachable here",
    "bb5c0 fp compare order": "the only reader tests bits 0x44 (ZF|PF = equal or unordered) of the lahf image, and swapping the operands changes only CF",
}
EQUIVALENT3 = {
    "c0200 zero-count shift flags": "flags written by a zero-count shl are overwritten before any jcc reads them",
}


def run(
    cmd: list[str], cwd: Path | None = None, timeout: int = 900
) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        cmd, cwd=cwd, text=True, capture_output=True, timeout=timeout, check=False
    )


def build_and_run(
    root: Path,
    src: Path,
    out: Path,
    gen: Path,
    lifted_opt: str,
    rust_opt: str,
    seeds: int,
    batch3: str = "",
    cover: bool = False,
) -> tuple[bool, str]:
    out.mkdir(parents=True, exist_ok=True)
    archive = out / "libtsfp_ports.a"
    extra = ["--cfg", "tsfp_cover"] if cover else []
    result = run(
        [*RUSTC, f"-Copt-level={rust_opt}", *extra, str(src / "lib.rs"), "-o", str(archive)]
    )
    if result.returncode:
        return False, "rust build failed\n" + result.stderr[-2000:]
    originals = out / "originals.c"
    preserve(gen, originals, BATCH3[batch3][1] if batch3 else ROOTS)
    exe = out / "test_rust_weapons"
    cc = [
        "cc",
        f"-O{lifted_opt}",
        "-fno-strict-aliasing",
        "-w",
        "-DTSFP_RUST_PORTS=1",
        "-I",
        str(root / "src/game"),
        "-I",
        str(root / "src/host"),
        "-I",
        str(gen),
        str(root / (BATCH3[batch3][0] if batch3 else "tests/c/test_rust_weapons.c")),
        str(root / "src/host/rust_ext_bridge.c"),
        str(originals),
        str(archive),
        "-lm",
        "-o",
        str(exe),
    ]
    result = run(cc)
    if result.returncode:
        return False, "C build failed\n" + result.stderr[-2000:]
    result = run([str(exe), str(seeds)])
    return result.returncode == 0, result.stdout + result.stderr


def edge_report(src_text: str, output: str) -> tuple[bool, str]:
    """Compare the edges the Rust batch 3 dispatcher took (printed by the harness built with
    --cfg tsfp_cover) with every static `pc = X; continue` edge of its source file."""
    start = src_text.index("        match pc {\n")
    arms = re.split(r"\n            (0x[0-9a-f]+) => \{\n", src_text[start:])
    static: set[tuple[int, int]] = set()
    blocks = set()
    for index in range(1, len(arms), 2):
        label = int(arms[index], 16)
        blocks.add(label & 0xFFFF)
        for target in re.findall(r"pc = (0x[0-9a-f]+); continue", arms[index + 1]):
            static.add((label & 0xFFFF, int(target, 16) & 0xFFFF))
    seen = {
        (int(m.group(1), 16), int(m.group(2), 16))
        for m in re.finditer(r"^edge ([0-9a-f]+) ([0-9a-f]+)$", output, re.MULTILINE)
    }
    missing = sorted(static - seen)
    entered = {b for _, b in seen} | {0xC0200}
    unentered = sorted(blocks - entered)
    text = (
        f"edge coverage: blocks {len(blocks) - len(unentered)}/{len(blocks)}, "
        f"edges {len(static & seen)}/{len(static)}; "
        f"missing edges {[(hex(a), hex(b)) for a, b in missing]}; "
        f"unentered blocks {[hex(b) for b in unentered]}"
    )
    return not missing and not unentered, text


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--gen-dir", type=Path, required=True, help="private lifted generated/lifted/gen"
    )
    parser.add_argument("--out-dir", type=output_path, default=output_path("tmp/t1831"))
    parser.add_argument("--seeds", type=int, default=20000)
    parser.add_argument(
        "--batch3",
        action="store_true",
        help="run the 0x1C0200 harness (tests/c/test_rust_weapons3.c) and its mutations",
    )
    parser.add_argument(
        "--root",
        choices=sorted(BATCH3),
        default="1c0200",
        help="with --batch3: which batch 3 root to run",
    )
    parser.add_argument(
        "--cover", action="store_true", help="with --batch3: edge coverage run (Rust O0, lifted O0)"
    )
    parser.add_argument(
        "--mutations", action="store_true", help="also run the mutation controls (O0/O0)"
    )
    parser.add_argument("--jobs", type=int, default=8, help="parallel mutation builds")
    parser.add_argument("--mutation-seeds", type=int, default=4000)
    parser.add_argument("--only", default="", help="run only mutations whose name contains this")
    args = parser.parse_args()
    root = Path.cwd()
    src = root / "crates/tsfp-ports/src"
    failed = False
    selected = args.root if args.batch3 else ""
    mutation_set = (
        (MUTATIONS_BB5C0 if selected == "1bb5c0" else MUTATIONS3) if selected else MUTATIONS
    )
    equivalent = {"1c0200": EQUIVALENT3, "1bb5c0": EQUIVALENT_BB5C0}.get(selected, {})
    if args.cover:
        ok, text = build_and_run(
            root, src, args.out_dir / "cover", args.gen_dir, "0", "0", args.seeds, selected, True
        )
        covered, report = edge_report((src / BATCH3[selected][2]).read_text(), text)
        print(f"== coverage run: {'PASS' if ok else 'FAIL'}\n{text.splitlines()[0]}\n{report}")
        failed |= not (ok and covered)
    else:
        for lifted_opt in ("0", "3"):
            for rust_opt in ("0", "3"):
                ok, text = build_and_run(
                    root,
                    src,
                    args.out_dir / f"l{lifted_opt}r{rust_opt}",
                    args.gen_dir,
                    lifted_opt,
                    rust_opt,
                    args.seeds,
                    selected,
                )
                print(
                    f"== lifted O{lifted_opt} rust O{rust_opt}: {'PASS' if ok else 'FAIL'}\n{text}"
                )
                failed |= not ok
    if args.mutations:

        def mutate(item: tuple[int, tuple]) -> tuple[str, str, bool]:
            index, (name, old, new, *rest) = item
            expected = rest[0] if rest else 1
            work = args.out_dir / f"mut{index}"
            shutil.rmtree(work, ignore_errors=True)
            # sibling crates are reached through `#[path = "../../tsfp-engine-*/..."]`, so the
            # mutated copy keeps the same crates/<name>/src layout with symlinked siblings
            mutated = work / "crates/tsfp-ports/src"
            mutated.parent.mkdir(parents=True)
            for sibling in (root / "crates").iterdir():
                if sibling.name != "tsfp-ports":
                    (work / "crates" / sibling.name).symlink_to(sibling.resolve())
            shutil.copytree(src, mutated)
            target = mutated / (BATCH3[selected][2] if selected else "weapons.rs")
            text = target.read_text()
            if text.count(old) != expected:
                return name, "ANCHOR DRIFTED", False
            target.write_text(text.replace(old, new))
            ok, output = build_and_run(
                root,
                mutated,
                work,
                args.gen_dir,
                "0",
                "0",
                min(args.seeds, args.mutation_seeds),
                selected,
            )
            first = next((line for line in output.splitlines() if "DISAGREE" in line), "")
            if "build failed" in output:
                return name, "INVALID MUTANT (build failed)", True
            if ok and name in equivalent:
                return name, f"EQUIVALENT (explained: {equivalent[name]})", False
            return name, ("SURVIVED" if ok else "killed") + f" {first}", ok

        survivors = []
        with ThreadPoolExecutor(max_workers=args.jobs) as pool:
            for name, verdict, survived in pool.map(
                mutate, [item for item in enumerate(mutation_set) if args.only in item[1][0]]
            ):
                print(f"mutation {name}: {verdict}")
                if survived or verdict.startswith("ANCHOR"):
                    survivors.append(name)
        failed |= bool(survivors)
        print(
            f"mutations {sum(args.only in m[0] for m in mutation_set)} survivors {len(survivors)}: {survivors}"
        )
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
