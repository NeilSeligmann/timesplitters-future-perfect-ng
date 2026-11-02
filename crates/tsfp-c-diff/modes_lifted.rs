// SPDX-License-Identifier: GPL-3.0-or-later
//! T1896 modes Tier 1: compiles selected functions of the PRIVATE lifted original (gitignored, never
//! committed) into a static object at build time. The function text is extracted into OUT_DIR
//! only. Every guest call the lifted body makes lands in a generated stub that forwards to the
//! harness callee stub (`recomp_lookup` in c/shim.c). Without the lifted tree nothing is built
//! and the `modes_lifted` cfg stays unset, so the differential tests compile to nothing.
use std::{
    collections::BTreeSet,
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

/// Roots under test (guest VA).
pub const ROOTS: &[u32] = &[
    0x425E0, 0x42620, 0x431F0, 0x69E70, 0x69E80, 0xBE850, 0x1B6030, 0x1CD1E0, 0x227C30, 0x652C0,
    0x67450, 0x67E60, 0x13DCF0, 0x158C80, 0x233EF0, 0x238D50, 0x23F320, 0x23F350, 0x2719F0,
    0x2B0AF0, 0x2E7EB0, 0x311860, 0x311870, 0x311880, 0x3118B0, 0x320FC0, 0x2F76D0, 0x2F76E0,
    0x67280, 0x670C0, 0x2D2C70, 0x68610, 0x28C560, 0x67CD0, 0x80F40, 0xB9D00, 0x253020, 0x252ED0,
    0x67F00, 0x298BA0, 0x254690, 0x2548F0, 0x66FC0, 0x158BC0,
    // T2371 reads the private lifted Map Maker launch and setup-roster bodies directly.
    0x3125A0, 0x313A80, // T2383: REA Map Maker reversible edit-history helpers.
    0x32A600, 0x3366F0, 0x3367A0, 0x337320, 0x337410,
    // T2431: Story progression sequencer, the Story menu callbacks, the page tick and `_ftol`.
    0x65940, 0x2CAFE0, 0x2CAD20, 0x2BEAA0, 0x2D3BF0, 0x3C848C,
    // T1703: timed effect entry table update (Story L1 crash suspect, ladder 0).
    0x216230,
];

/// Roots that other roots call and that stay direct calls inside the lifted extract.
const DIRECT: &[u32] = &[
    0x69E70, 0x3125A0, 0x32A600, 0x3367A0,
    0x337320, // T2431: the sequencer and `_ftol` run inline.
    0x65940, 0x3C848C,
];

/// Stack argument counts of every callee the roots reach (recorded in the call trace). Callees
/// that take register arguments are recorded with 0 stack arguments.
/// Callees that return a float in ST0 (x87). The stub pushes the scripted value onto the x87
/// stack model (`g_fp_stack`), exactly where the lifted caller expects it.
pub const FP_RET: &[u32] = &[0x350CF0, 0x3C9CC4, 0x153BD0];

/// Callees that take their argument in ST0 (x87 stack top): the stub pops it and records its f32
/// bits as the first traced argument word.
pub const FP_ARG: &[u32] = &[0x3C9CC4, 0x153BD0];

pub const ARGC: &[(u32, u32)] = &[
    (0x153AC0, 0),
    (0x3567F0, 1),
    (0x64090, 0),
    (0x640B0, 0),
    (0x64180, 2),
    (0x42510, 3),
    (0x431F0, 3),
    (0x1CD170, 0),
    (0x33ED70, 1),
    (0x3D7B0, 1),
    (0x66FC0, 0),
    (0x231A20, 0),
    (0x66E90, 0),
    (0x355F10, 0),
    (0x357690, 0),
    (0x275620, 0),
    (0x275EF0, 0),
    (0x271910, 0),
    (0x274650, 0),
    (0x273240, 0),
    (0x6A850, 1),
    (0x6B2B0, 1),
    (0x64160, 1),
    (0x1B5C70, 3),
    (0xD7780, 2),
    (0x1CDAE0, 0),
    (0x2F7610, 0),
    (0x69E80, 1),
    (0x69E60, 0),
    (0x69A60, 1),
    (0x153B40, 0),
    (0x1CEB00, 1),
    (0x670C0, 0),
    (0x1CD1E0, 0),
    (0x667B0, 0),
    (0x227B50, 0),
    (0x25F010, 0),
    (0x389B0, 0),
    (0x673B0, 0),
    (0x45FD0, 0),
    (0xA9070, 1),
    (0x151030, 3),
    (0x356800, 2),
    (0xCF4D0, 1),
    (0x12D350, 4),
    (0x12E3B0, 2),
    (0x64D90, 1),
    (0x36CFA0, 0),
    (0xF8AD0, 1),
    (0xFD7E0, 1),
    (0x67450, 1),
    (0x153550, 4),
    (0x1533E0, 2),
    (0x153650, 2),
    (0x350CF0, 1),
    (0xCF370, 2),
    (0x296590, 2),
    (0x298570, 1),
    (0x152090, 2),
    (0xEA9C0, 12),
    (0x251CD0, 0),
    (0x3C9CC4, 0),
    (0x254690, 0),
    (0x3533F0, 0),
    (0x355EC0, 0),
    (0x2CB210, 0),
    (0x69640, 0),
    (0x22E050, 1),
    (0x2D2C10, 0),
    (0x2D2C40, 0),
    (0x63EB0, 0),
    (0x668F0, 0),
    (0x2E4B80, 1),
    (0x2E6D40, 1),
    (0x2E7010, 1),
    (0x455D0, 0),
    (0x15BDC0, 0),
    (0x45390, 1),
    (0x45820, 2),
    (0x45A00, 2),
    (0x6BC00, 0),
    (0x6BC50, 3),
    (0x2C0290, 1),
    (0x3006C0, 2),
    (0x3C230, 1),
    (0x3125A0, 4),
    (0x32A600, 1),
    (0x32A5B0, 2),
    (0x3366F0, 1),
    (0x3367A0, 4),
    (0x337320, 1),
    (0x337410, 4),
    (0xD6C90, 1),
    (0x658D0, 1),
    (0x2F79E0, 0),
    (0x300630, 1),
    (0x7A270, 3),
    (0x2BE270, 1),
    (0x1B5630, 1),
    // T2431: 0x65940 and its service callees (0x389B0 is already listed with 0).
    (0x65940, 1),
    // T1703: 0x216230 callees (0x153BD0 takes and returns ST0).
    (0x23FF20, 1),
    (0x152B50, 2),
    (0xFFDA0, 6),
    (0xFFE40, 2),
    (0x153BB0, 0),
    (0x153BD0, 0),
    (0x216190, 3),
    (0x64D10, 0),
    (0x64DD0, 2),
    (0x1CD590, 1),
    (0x35DE70, 0),
    // T2431: Story menu chain callees (0x2D2C70 mode 4, 0x2D3BF0 tick, 0x2CAFE0/0x2CAD20/0x2BEAA0).
    (0x79B70, 2),
    (0x21780, 1),
    (0x70D90, 0),
    (0x70D50, 0),
    (0x46260, 1),
    (0x462B0, 1),
    (0x3C420, 0),
    (0x30210, 3),
    (0x30280, 2),
    (0x7A7A0, 1),
    (0x2D74F0, 1),
    (0x1553E0, 4),
    (0x2D38C0, 0),
    (0x2CAFE0, 1),
    (0x2CAD20, 1),
    (0x2BEAA0, 0),
    (0x2D3BF0, 1),
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
                if va != own && !DIRECT.contains(&va) {
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
         typedef void (*md_fn)(void);\n\
         #define MD_WEAK __attribute__((weak))\n\
         MD_WEAK int g_force_return; MD_WEAK __thread uint32_t g_seh_ebp;\n\
         MD_WEAK __thread int g_fp_cmp; MD_WEAK __thread uint16_t g_fp_cc;\n\
         MD_WEAK __thread uint32_t g_flag_bridge_eflags, g_flag_bridge_mask;\n\
         MD_WEAK recomp_func_t recomp_lookup_manual(uint32_t va) { return 0; }\n\
         MD_WEAK recomp_func_t recomp_lookup_kernel(uint32_t va) { return 0; }\n\
         MD_WEAK void recomp_icall_unresolved_trap(uint32_t va, const char *kind) { __builtin_trap(); }\n",
    );
    // Own callee stub and trace (independent of c/shim.c): records 16 words per call.
    source.push_str("static uint32_t md_trace[256][16]; static uint32_t md_count;\n");
    source.push_str("static uint32_t md_script_rows[256][3]; static uint32_t md_script_len;\n");
    source.push_str("static uint32_t md_st0_rows[64]; static uint32_t md_st0_len;\n");
    source.push_str("static uint32_t md_write_rows[64][3]; static uint32_t md_write_len;\n");
    source.push_str("static int md_fp_ret_of(uint32_t va)\n{\n    switch (va) {\n");
    for va in FP_RET {
        source.push_str(&format!("    case 0x{va:X}u: return 1;\n"));
    }
    source.push_str("    default: return 0;\n    }\n}\n");
    source.push_str("static int md_fp_arg_of(uint32_t va)\n{\n    switch (va) {\n");
    for va in FP_ARG {
        source.push_str(&format!("    case 0x{va:X}u: return 1;\n"));
    }
    source.push_str("    default: return 0;\n    }\n}\n");
    source.push_str("static int md_argc_of(uint32_t va)\n{\n    switch (va) {\n");
    for (va, argc) in ARGC {
        source.push_str(&format!("    case 0x{va:X}u: return {argc};\n"));
    }
    source.push_str("    default: return -1;\n    }\n}\n");
    source.push_str("int md_argc(uint32_t va) { return md_argc_of(va); }\n");
    source.push_str(
        "static void md_stub(uint32_t va)\n{\n    int argc = md_argc_of(va);\n    if (argc < 0 || md_count >= 256) __builtin_trap();\n\
         uint32_t i = md_count++;\n    md_trace[i][0] = va; md_trace[i][1] = g_esp; md_trace[i][2] = MEM32(g_esp);\n\
         for (int a = 0; a < 3; a++) md_trace[i][3 + a] = a < argc ? MEM32(g_esp + 4 + 4 * a) : 0;\n\
         if (md_fp_arg_of(va)) { union { uint32_t u; float f; } arg; arg.f = (float)g_fp_stack[g_fp_top]; g_fp_top = (g_fp_top + 1) & 7; if (va != 0x153BD0u) md_trace[i][3] = arg.u; /* T1703: 0x153BD0's ST0 argument is stale x87 residue in this harness */ }\n\
         md_trace[i][6] = g_eax; md_trace[i][7] = g_ecx; md_trace[i][8] = g_edx;\n\
         if (i < md_script_len) { g_eax = md_script_rows[i][0]; g_ecx = md_script_rows[i][1]; g_edx = md_script_rows[i][2]; }\n\
         else { g_eax = 0; g_ecx = 0; g_edx = 0; }\n\
         for (uint32_t w = 0; w < md_write_len; w++) if (md_write_rows[w][0] == i) MEM32(md_write_rows[w][1]) = md_write_rows[w][2];\n\
         if (md_fp_ret_of(va)) { union { uint32_t u; float f; } bits; bits.u = i < md_st0_len ? md_st0_rows[i] : 0;\n\
             g_fp_top = (g_fp_top + 7) & 7; g_fp_stack[g_fp_top] = (double)bits.f; }\n\
         g_esp += 4;\n}\n\
         void md_script(const uint32_t *w, uint32_t n)\n{\n    md_script_len = n > 256 ? 256 : n; md_count = 0; md_st0_len = 0; md_write_len = 0;\n    /* T1703: every case starts with an empty x87 stack, like the Rust side (no stale ST0 across cases). */\n    g_fp_top = 0; for (int k = 0; k < 8; k++) g_fp_stack[k] = 0.0;\n\
         for (uint32_t i = 0; i < md_script_len; i++) for (int k = 0; k < 3; k++) md_script_rows[i][k] = w[3 * i + k];\n}\n\
         void md_st0(const uint32_t *w, uint32_t n)\n{\n    md_st0_len = n > 64 ? 64 : n;\n    for (uint32_t i = 0; i < md_st0_len; i++) md_st0_rows[i] = w[i];\n}\n\
         void md_writes(const uint32_t *w, uint32_t n)\n{\n    md_write_len = n > 64 ? 64 : n;\n    for (uint32_t i = 0; i < md_write_len; i++) for (int k = 0; k < 3; k++) md_write_rows[i][k] = w[3 * i + k];\n}\n\
         uint32_t md_trace_len(void) { return md_count; }\n\
         const uint32_t *md_trace_entry(uint32_t i) { return md_trace[i]; }\n",
    );
    let mut stubs = BTreeSet::new();
    let mut bodies = String::new();
    for &va in ROOTS {
        let body = extract(&dir, va).unwrap_or_else(|| panic!("lifted sub_{va:08X} not found"));
        let found = callees(&body, va);
        let mut body = body;
        // Rename callees so they cannot collide with replacements already in the archive.
        for callee in &found {
            body = body.replace(&format!("sub_{callee:08X}"), &format!("mdcs_{callee:08X}"));
        }
        for root in ROOTS {
            body = body.replace(&format!("sub_{root:08X}"), &format!("mdr_{root:08X}"));
        }
        stubs.extend(found);
        bodies.push_str(&body);
    }
    for va in &stubs {
        source.push_str(&format!("void mdcs_{va:08X}(void);\n"));
    }
    for va in ROOTS {
        source.push_str(&format!("void mdr_{va:08X}(void);\n"));
    }
    source.push_str(&bodies);
    for va in &stubs {
        source.push_str(&format!(
            "void mdcs_{va:08X}(void) {{ md_stub(0x{va:X}u); }}\n"
        ));
    }
    source.push_str(
        "int md_run(uint32_t va, uint32_t r[8])\n{\n    md_fn fn = 0;\n    switch (va) {\n",
    );
    for va in ROOTS {
        source.push_str(&format!("    case 0x{va:X}u: fn = mdr_{va:08X}; break;\n"));
    }
    source.push_str(
        "    default: return 0;\n    }\n    g_eax = r[0]; g_ecx = r[1]; g_edx = r[2]; g_ebx = r[3];\n\
         g_esi = r[4]; g_edi = r[5]; g_ebp = r[6]; g_esp = r[7];\n\
         g_seh_ebp = r[6]; g_df = 0; g_fp_top = 0;\n    fn();\n\
         r[0] = g_eax; r[1] = g_ecx; r[2] = g_edx; r[3] = g_ebx; r[4] = g_esi; r[5] = g_edi;\n\
         r[6] = g_ebp; r[7] = g_esp;\n    return 1;\n}\n",
    );
    let c_file = out.join("modes_lifted_private.c");
    fs::write(&c_file, source).expect("write private lifted extract");
    let object = out.join("modes_lifted_private.o");
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
    assert!(status.success(), "private lifted extract failed to compile");
    println!("cargo:rustc-cfg=modes_lifted");
    println!("cargo:rustc-check-cfg=cfg(modes_lifted)");
    Some(object)
}
