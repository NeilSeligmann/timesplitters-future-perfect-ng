// SPDX-License-Identifier: GPL-3.0-or-later
//! T1900 slice B Tier 1: compiles the 17 multiplayer / team pickup roots of the PRIVATE lifted
//! original (gitignored, never committed) into a static object at build time, like `w4_lifted.rs`
//! for T1895. Function text goes to OUT_DIR only. Every guest call a root makes lands in a
//! generated `t19cs_*` stub that forwards to the harness callee stub (`recomp_lookup` in
//! c/shim.c), except the calls listed in [`real_callees`] which run the lifted callee itself
//! (a port that inlines the callee is compared against the lifted pair).
//!
//! The object also carries the strong definitions of the two hooks declared weak in shim.c:
//! the extra callee stack contracts ([`ARGC`]) and a per-call recorder with the full argument
//! list and the entry registers (`t1900_trace_entry`), plus the ST0 return of the roster stat
//! callee (`t1900_fp_set`). Without the lifted tree nothing is built and the `t1900_lifted`
//! cfg stays unset, so tests/differential/t1900.rs compiles to nothing.
use std::{collections::BTreeSet, env, fs, path::{Path, PathBuf}, process::Command};

/// Roots under test (guest VA).
pub const ROOTS: &[u32] = &[
    0x274330, 0x274350, 0x274380, 0x2744F0, 0x274570, 0x274650, 0x275640, 0x160AC0, 0xCB240,
    0x2E4470, 0x2F1FC0, 0x2F2050, 0xD8B10, 0xD8A00, 0xC09B0, 0xC0C50, 0xC11A0,
];

/// Callees whose lifted body is run for real inside a given root (everything else is stubbed).
fn real_callees(root: u32) -> &'static [u32] {
    match root {
        0x275640 => &[0x274380],
        _ => &[],
    }
}

/// Stack argument count (dwords) of every stubbed callee, read from the lifted call sites.
pub const ARGC: &[(u32, u32)] = &[
    (0x274510, 1), (0x3DFA0, 2), (0x25A570, 2), (0xCB0F0, 0), (0xD6C90, 1), (0x238580, 2),
    (0x69640, 0), (0x2F1FC0, 1), (0xC1950, 10), (0xDB420, 2), (0x45CF0, 2), (0x238510, 3),
    (0x64460, 2), (0x12E3B0, 2), (0x3D7B0, 1), (0x151030, 3), (0xCF4D0, 2), (0x12D350, 4),
    (0x164A30, 1), (0xBE1D0, 0), (0x45390, 1), (0x45820, 1), (0xBEF00, 1), (0xBE860, 2),
    (0x217B0, 2), (0x31310, 1), (0x160AC0, 1), (0xC09B0, 0), (0x10EC20, 1), (0x10E6E0, 1),
    (0x2743A0, 1), (0x966E0, 3), (0x153240, 1), (0xD83D0, 3),
];

fn lifted_dir(manifest: &Path) -> Option<PathBuf> {
    println!("cargo:rerun-if-env-changed=TSFP_LIFTED_DIR");
    let dir = env::var("TSFP_LIFTED_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| manifest.join("../../generated/lifted/gen"));
    dir.join("recomp_types.h").is_file().then_some(dir)
}

fn extract(dir: &Path, va: u32) -> Option<String> {
    let header = format!("void sub_{va:08X}(void)");
    for index in 0..200 {
        let Ok(text) = fs::read_to_string(dir.join(format!("recomp_{index:04}.c"))) else {
            continue;
        };
        let mut body = String::new();
        let mut inside = false;
        for line in text.lines() {
            if line == header {
                inside = true;
            }
            if inside {
                body.push_str(line);
                body.push('\n');
                if line == "}" {
                    return Some(body);
                }
            }
        }
    }
    None
}

fn callees(body: &str, own: u32) -> BTreeSet<u32> {
    let mut found = BTreeSet::new();
    let mut rest = body;
    while let Some(at) = rest.find("sub_") {
        let hex = &rest[at + 4..];
        if hex.len() >= 8 {
            if let Ok(va) = u32::from_str_radix(&hex[..8], 16) {
                if va != own {
                    found.insert(va);
                }
            }
        }
        rest = &rest[at + 4..];
    }
    found
}

pub fn build(manifest: &Path, out: &Path, opt: &str) -> Option<PathBuf> {
    let dir = lifted_dir(manifest)?;
    let mut source = String::from(
        "#define RECOMP_GENERATED_CODE\n#include \"recomp_funcs.h\"\n#include <math.h>\n\
         typedef void (*w4_fn)(void);\nextern w4_fn recomp_lookup(uint32_t va);\n\
         #define W4_WEAK __attribute__((weak))\n\
         W4_WEAK int g_force_return; W4_WEAK __thread uint32_t g_seh_ebp;\n\
         W4_WEAK __thread int g_fp_cmp; W4_WEAK __thread uint16_t g_fp_cc;\n\
         W4_WEAK __thread uint32_t g_flag_bridge_eflags, g_flag_bridge_mask;\n\
         W4_WEAK recomp_func_t recomp_lookup_manual(uint32_t va) { return 0; }\n\
         W4_WEAK recomp_func_t recomp_lookup_kernel(uint32_t va) { return 0; }\n\
         W4_WEAK void recomp_icall_unresolved_trap(uint32_t va, const char *kind) { __builtin_trap(); }\n",
    );
    let mut stubs = BTreeSet::new();
    let mut bodies = String::new();
    for &va in ROOTS {
        let body = extract(&dir, va).unwrap_or_else(|| panic!("lifted sub_{va:08X} not found"));
        let real = real_callees(va);
        let found: BTreeSet<u32> =
            callees(&body, va).into_iter().filter(|callee| !real.contains(callee)).collect();
        let mut body = body;
        for callee in &found {
            body = body.replace(&format!("sub_{callee:08X}"), &format!("t19cs_{callee:08X}"));
        }
        // Root symbols get a private prefix: the src/game replacements already define sub_*.
        for root in ROOTS {
            body = body.replace(&format!("sub_{root:08X}"), &format!("t19r_{root:08X}"));
        }
        stubs.extend(found);
        bodies.push_str(&body);
    }
    for va in &stubs {
        source.push_str(&format!("void t19cs_{va:08X}(void);\n"));
    }
    for va in ROOTS {
        source.push_str(&format!("void t19r_{va:08X}(void);\n"));
    }
    source.push_str(&bodies);
    for va in &stubs {
        source.push_str(&format!("void t19cs_{va:08X}(void) {{ recomp_lookup(0x{va:X}u)(); }}\n"));
    }
    // Callee contracts, recorder and ST0 return of the roster stat callee.
    source.push_str("int t1900_extra_argc(uint32_t va)\n{\n    switch (va) {\n");
    for (va, argc) in ARGC {
        source.push_str(&format!("    case 0x{va:X}u: return {argc};\n"));
    }
    source.push_str(
        "    default: return -1;\n    }\n}\n\
         #define T19_WORDS 24\nstatic uint32_t t19_trace[64][T19_WORDS];\nstatic uint32_t t19_fp[64];\n\
         const uint32_t *t1900_trace_entry(uint32_t i) { return t19_trace[i]; }\n\
         void t1900_fp_set(const uint32_t *bits, uint32_t n) { for (uint32_t i = 0; i < n && i < 64; i++) t19_fp[i] = bits[i]; }\n\
         void t1900_callee_hook(uint32_t va, uint32_t index, int phase)\n{\n\
         \x20   if (phase == 0) {\n\
         \x20       uint32_t *e = t19_trace[index];\n\
         \x20       int argc = t1900_extra_argc(va);\n\
         \x20       for (int w = 0; w < T19_WORDS; w++) e[w] = 0;\n\
         \x20       e[0] = va; e[1] = g_esp; e[2] = MEM32(g_esp); e[3] = (uint32_t)argc;\n\
         \x20       for (int a = 0; a < argc && a < 10; a++) e[4 + a] = MEM32(g_esp + 4 + 4 * a);\n\
         \x20       e[14] = g_eax; e[15] = g_ecx; e[16] = g_edx; e[17] = g_ebx; e[18] = g_esi; e[19] = g_edi; e[20] = g_ebp;\n\
         \x20   } else if (va == 0x238580) {\n\
         \x20       union { uint32_t u; float f; } v; v.u = t19_fp[index];\n\
         \x20       g_fp_top = (g_fp_top + 7u) & 7u; g_fp_stack[g_fp_top] = (double)v.f;\n\
         \x20   }\n}\n",
    );
    source.push_str("int t1900_run(uint32_t va, uint32_t r[8])\n{\n    w4_fn fn = 0;\n    switch (va) {\n");
    for va in ROOTS {
        source.push_str(&format!("    case 0x{va:X}u: fn = t19r_{va:08X}; break;\n"));
    }
    source.push_str(
        "    default: return 0;\n    }\n    g_eax = r[0]; g_ecx = r[1]; g_edx = r[2]; g_ebx = r[3];\n\
         g_esi = r[4]; g_edi = r[5]; g_ebp = r[6]; g_esp = r[7];\n\
         g_seh_ebp = r[6]; g_df = 0; g_fp_top = 0;\n    fn();\n\
         r[0] = g_eax; r[1] = g_ecx; r[2] = g_edx; r[3] = g_ebx; r[4] = g_esi; r[5] = g_edi;\n\
         r[6] = g_ebp; r[7] = g_esp;\n    return 1;\n}\n",
    );
    let c_file = out.join("t1900_lifted_private.c");
    fs::write(&c_file, source).expect("write private lifted extract");
    let object = out.join("t1900_lifted_private.o");
    let status = Command::new("cc")
        .args(["-std=gnu11", "-fPIC", "-fno-strict-aliasing", "-w"])
        .arg(format!("-O{opt}"))
        .arg("-I")
        .arg(&dir)
        .arg("-c")
        .arg(&c_file)
        .arg("-o")
        .arg(&object)
        .status()
        .expect("spawn cc");
    assert!(status.success(), "private t1900 lifted extract failed to compile");
    println!("cargo:rustc-cfg=t1900_lifted");
    Some(object)
}
