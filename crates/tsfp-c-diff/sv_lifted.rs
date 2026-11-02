// SPDX-License-Identifier: GPL-3.0-or-later
//! T1899 save Tier 1: compiles selected functions of the PRIVATE lifted original (gitignored, never
//! committed) into a static object at build time. The function text is extracted into OUT_DIR
//! only. Every guest call the lifted body makes lands in a generated stub that forwards to the
//! harness callee stub (`recomp_lookup` in c/shim.c). Without the lifted tree nothing is built
//! and the `sv_lifted` cfg stays unset, so the differential tests compile to nothing.
use std::{collections::BTreeSet, env, fs, path::{Path, PathBuf}, process::Command};

/// Roots under test (guest VA): data/save, mapmaker storage and content leaves (T1899).
pub const ROOTS: &[u32] = &[
    0x24900, 0x3F890, 0x31DCD0, 0x3082D0, 0x322150, 0x32A3A0, 0x3346E0, 0x334700, 0x312AD0,
    0x312AE0, 0x69630, 0x696D0, 0x69720, 0x69750, 0x69790, 0x697D0, 0x698D0, 0x69900, 0x69930,
    0x69640, 0x696A0, 0x24E40, 0x24E60, 0x24EB0, 0x24D80, 0x24E00, 0x25B80, 0x25BB0, 0x25BE0,
    0x25240, 0x827C0, 0x827D0, 0x827F0, 0x82800, 0x82820, 0x190690, 0x190A70, 0x1CCEB0, 0x1CD2B0,
    0x1CD330, 0x1CD380, 0x1CD560, 0x1CD040, 0x1CD940, 0x1CD3E0, 0x190D90, 0x190E00, 0x336640, 0x69A60, 0x25E40, 0x25FF0, 0x248A0, 0x26320, 0x32B540, 0x25710, 0x263E0, 0x25280, 0x26740, 0x2FA150, 0x301A90,
    // T2130: save_unlock requirement wrappers (the rest of the file's roots above) ...
    0x1CD890, 0x1CD980, 0x1CD9D0, 0x1CD9F0, 0x1CDA20, 0x2BE5E0, 0x2CBD20,
    // ... and the T1908 cutscene state / key-cache leaves (tsfp-data-formats cutscenes.rs).
    0x60410, 0x60390, 0x8E970, 0x64D90, 0x94220, 0x98DA0, 0x64DD0, 0x58240, 0x98DB0, 0x61680,
    0x6EE00, 0x23EFC0, 0x6F4B0, 0x3EE20, 0x665B0, 0x6F430,
    // T2374 readable Rust-owned cutscene sequencing control.
    0x98380,
];

/// Roots that other roots call and that stay direct calls inside the lifted extract.
const DIRECT: &[u32] = &[
    0x24D80, 0x24E40, 0x24E60, 0x24EB0, 0x827F0, 0x69630, 0x696D0, 0x69900, 0x1CCEB0, 0x1CD2B0, 0x1CD330,
    0x1CD380, 0x1CD940, 0x1CD3E0, 0x248A0, 0x3F890, 0x24900, 0x25710, 0x2BE5E0, 0x60390,
];

/// Stack argument counts of every callee the roots reach (recorded in the call trace).
pub const ARGC: &[(u32, u32)] = &[
    (0x3533F0, 0), (0x356800, 1), (0x150230, 0), (0x46D8F6, 2), (0x37E89E, 1), (0x312A60, 0),
    (0x69620, 1), (0x79B70, 2),
    (0x22E070, 1), (0x356870, 1), (0x3006C0, 2), (0x230530, 1), (0x1CD170, 0),
    (0x3C85D4, 3), (0x37E44D, 2), (0x37F512, 3), (0x37E557, 2), (0x37F662, 2), (0x37E59E, 1),
    (0x37D231, 7), (0x37D1F6, 2), (0x37CBC4, 1), (0x37F0D6, 2), (0x37EB84, 2), (0x150350, 6),
    (0xC0DE0001, 2), (0x24EC0, 3), (0x3C8960, 3),
    (0x331A20, 2), (0x331920, 2), (0x332490, 2), (0x630C0, 1), (0x331970, 1), (0x331990, 1), (0x1531D0, 2),
    (0x1518F0, 4), (0x334B20, 4),
    (0x3C9D90, 0), (0x32A600, 1), (0x2FEBF0, 1), (0x321C00, 0), (0x2FE160, 0),
    (0x31BDB0, 0), (0x325610, 0), (0x320F50, 0), (0x31E160, 0), (0x2FE0F0, 3), (0x31DD10, 2), (0x3210C0, 0),
    (0x320EA0, 2), (0x320FC0, 2), (0x332330, 3), (0x311890, 1), (0x3118F0, 0), (0x311910, 0), (0x311920, 0), (0x2F7830, 1), (0x2F77E0, 1), (0x151E80, 2), (0x9A630, 7),
    (0x3D7B0, 1), (0x2BE590, 2), (0x827F0, 0), (0x827C0, 0), (0x82820, 0), (0x24950, 6), (0x82560, 4), (0x3D7F0, 3),
    (0x25C30, 3), (0x25BE0, 1), (0x825C0, 5), (0x24F40, 0), (0x37E962, 2), (0x37E914, 3), (0x250C0, 0),
    (0x25180, 0), (0x3E1E0, 1), (0x37E9A7, 0), (0x37E11A, 6), (0x37CCF5, 5), (0x25070, 1), (0x37CDCB, 1),
    (0x37E37D, 2), (0x37CC08, 5), (0x37CE4F, 4), (0x3DFA0, 2), (0x25020, 1),
    (0x37F99F, 4), (0x37F9F1, 6), (0x29650, 1), (0x251D0, 0), (0x37FBA9, 1),
];

/// Callees that pop their own arguments (stdcall), in dwords.
const STDCALL_POP: &[(u32, u32)] = &[
    (0x46D8F6, 2), (0x37E89E, 1), (0x37E44D, 2), (0x37F512, 3), (0x37E557, 2), (0x37F662, 2),
    (0x37E59E, 1), (0x37D231, 7), (0x37D1F6, 2), (0x37CBC4, 1), (0x37F0D6, 2), (0x37EB84, 2),
    (0x37E962, 2), (0x37E914, 3), (0x37E11A, 6), (0x37CCF5, 5), (0x37CDCB, 1), (0x37E37D, 2), (0x37CC08, 5), (0x37CE4F, 4),
    (0x37F99F, 4), (0x37F9F1, 6), (0x37FBA9, 1),
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
         typedef void (*sv_fn)(void);\n\
         #define SV_WEAK __attribute__((weak))\n\
         SV_WEAK int g_force_return; SV_WEAK __thread uint32_t g_seh_ebp;\n\
         SV_WEAK __thread int g_fp_cmp; SV_WEAK __thread uint16_t g_fp_cc;\n\
         SV_WEAK __thread uint32_t g_flag_bridge_eflags, g_flag_bridge_mask;\n\
         SV_WEAK volatile uint64_t g_icall_count; SV_WEAK volatile uint32_t g_icall_trace[ICALL_TRACE_SIZE];\n\
         SV_WEAK volatile uint32_t g_icall_trace_idx; SV_WEAK uint32_t g_xbox_code_lo, g_xbox_code_hi;\n\
         SV_WEAK recomp_func_t recomp_lookup_kernel(uint32_t va) { return 0; }\n\
         SV_WEAK void recomp_icall_unresolved_trap(uint32_t va, const char *kind) { __builtin_trap(); }\n",
    );
    // Own callee stub and trace (independent of c/shim.c): records 16 words per call.
    source.push_str("static uint32_t sv_trace[4096][16]; static uint32_t sv_count; static int sv_force_begin;\nvoid sv_set_force_begin(int v) { sv_force_begin = v; }\n\
static int sv_slot_on; static uint32_t sv_slot_val;\nvoid sv_set_slot(int on, uint32_t v) { sv_slot_on = on; sv_slot_val = v; }\n");
    source.push_str("static uint32_t sv_script_rows[128][3]; static uint32_t sv_script_len;\n");
    source.push_str("static int sv_argc_of(uint32_t va)\n{\n    switch (va) {\n");
    for (va, argc) in ARGC {
        source.push_str(&format!("    case 0x{va:X}u: return {argc};\n"));
    }
    source.push_str("    default: return -1;\n    }\n}\n");
    source.push_str("int sv_argc(uint32_t va) { return sv_argc_of(va); }\n");
    source.push_str("static int sv_pop_of(uint32_t va)\n{\n    switch (va) {\n");
    for (va, pop) in STDCALL_POP {
        source.push_str(&format!("    case 0x{va:X}u: return {pop};\n"));
    }
    source.push_str("    default: return 0;\n    }\n}\n");
    source.push_str(
        "static void sv_stub(uint32_t va)\n{\n    int argc = sv_argc_of(va);\n    if (argc < 0 || sv_count >= 4096) __builtin_trap();\n\
         uint32_t i = sv_count++; uint32_t eax_in = g_eax;\n    sv_trace[i][0] = va; sv_trace[i][1] = g_esp; sv_trace[i][2] = MEM32(g_esp);\n\
         for (int a = 0; a < 3; a++) sv_trace[i][3 + a] = a < argc ? MEM32(g_esp + 4 + 4 * a) : 0;\n\
         sv_trace[i][6] = g_eax; sv_trace[i][7] = g_ecx; sv_trace[i][8] = g_edx; sv_trace[i][9] = g_ebx; sv_trace[i][10] = g_esi;\n\
         if (va == 0x24F40u) { uint32_t st = g_eax; MEM32(st) = 7; MEM32(st + 4) = 4; MEM32(st + 8) = 1; MEM32(st + 0xC) = 0x00900A00u; }\n\
         if (i < sv_script_len) { g_eax = sv_script_rows[i][0]; g_ecx = sv_script_rows[i][1]; g_edx = sv_script_rows[i][2]; }\n\
         else { g_eax = 0; g_ecx = 0; g_edx = 0; }\n    if (va == 0x24F40u && sv_force_begin) g_eax = 1;\n    if (va == 0x3210C0u) g_eax = 0x00930000u;\n    if (va == 0x3DFA0u) g_eax = 0x00940000u;\n    if (va == 0x311920u) g_eax = 0x00950000u;\n    if (va == 0x356870u && sv_slot_on) g_eax = sv_slot_val;\n    if (va == 0x3C9D90u) g_esp -= eax_in;\n    switch (va) {\n\
    case 0x331A20u: case 0x331920u: case 0x331970u: case 0x2F7830u: case 0x2F77E0u: g_eax &= 1; break;\n\
    case 0x332490u: g_eax = 0x3E9u + (g_eax & 3); break;\n\
    case 0x331990u: g_eax &= 3; break;\n\
    case 0x630C0u: g_eax = 0x00901000u; break;\n\
    case 0x334B20u: { uint32_t out = MEM32(g_esp + 16); for (uint32_t k = 0; k < 4; k++) MEM32(out + 4 * k) = 0x3E9u + k; g_eax &= 3; break; }\n\
    case 0x9A630u: { uint32_t idx = MEM32(g_esp + 4); for (uint32_t k = 0; k < 8; k++) MEM8(0x7B67A0u + k) = (k <= idx + 1u); break; }\n\
    default: break;\n    }\n    g_esp += 4 + 4 * sv_pop_of(va);\n}\n\
         void sv_script(const uint32_t *w, uint32_t n)\n{\n    sv_script_len = n > 128 ? 128 : n; sv_count = 0;\n\
         for (uint32_t i = 0; i < sv_script_len; i++) for (int k = 0; k < 3; k++) sv_script_rows[i][k] = w[3 * i + k];\n}\n\
         uint32_t sv_trace_len(void) { return sv_count; }\n\
         const uint32_t *sv_trace_entry(uint32_t i) { return sv_trace[i]; }\n",
    );
    let mut stubs = BTreeSet::new();
    let mut bodies = String::new();
    for &va in ROOTS {
        let body = extract(&dir, va).unwrap_or_else(|| panic!("lifted sub_{va:08X} not found"));
        let found = callees(&body, va);
        let mut body = body;
        // Rename callees so they cannot collide with replacements already in the archive.
        for callee in &found {
            body = body.replace(&format!("sub_{callee:08X}"), &format!("svcs_{callee:08X}"));
        }
        for root in ROOTS {
            body = body.replace(&format!("sub_{root:08X}"), &format!("svr_{root:08X}"));
        }
        // Manual (library) calls resolve through the weak icall tables in the other lifted
        // objects, so route the traced ones straight to this object's own stub.
        for (callee, _) in ARGC {
            body = body.replace(
                &format!("RECOMP_ICALL_SAFE(0x{callee:08X}u, _icall_esp)"),
                &format!("sv_stub(0x{callee:X}u)"),
            );
        }
        // Indirect callbacks (`call [ebp+0x10]`) become one traced stub with a fixed tag.
        body = indirect_callback(&body);
        stubs.extend(found);
        bodies.push_str(&body);
    }
    for va in &stubs {
        source.push_str(&format!("void svcs_{va:08X}(void);\n"));
    }
    for va in ROOTS {
        source.push_str(&format!("void svr_{va:08X}(void);\n"));
    }
    source.push_str(&bodies);
    for va in &stubs {
        source.push_str(&format!("void svcs_{va:08X}(void) {{ sv_stub(0x{va:X}u); }}\n"));
    }
    source.push_str("int sv_run(uint32_t va, uint32_t r[8])\n{\n    sv_fn fn = 0;\n    switch (va) {\n");
    for va in ROOTS {
        source.push_str(&format!("    case 0x{va:X}u: fn = svr_{va:08X}; break;\n"));
    }
    source.push_str(
        "    default: return 0;\n    }\n    g_eax = r[0]; g_ecx = r[1]; g_edx = r[2]; g_ebx = r[3];\n\
         g_esi = r[4]; g_edi = r[5]; g_ebp = r[6]; g_esp = r[7];\n\
         g_seh_ebp = r[6]; g_df = 0; g_fp_top = 0;\n    fn();\n\
         r[0] = g_eax; r[1] = g_ecx; r[2] = g_edx; r[3] = g_ebx; r[4] = g_esi; r[5] = g_edi;\n\
         r[6] = g_ebp; r[7] = g_esp;\n    return 1;\n}\n",
    );
    let c_file = out.join("sv_lifted_private.c");
    fs::write(&c_file, source).expect("write private lifted extract");
    let object = out.join("sv_lifted_private.o");
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
    println!("cargo:rustc-cfg=sv_lifted");
    println!("cargo:rustc-check-cfg=cfg(sv_lifted)");
    Some(object)
}

/// Replaces `RECOMP_ICALL_SAFE_AT(_icall_target, _icall_esp, site)` by the tagged callback stub.
fn indirect_callback(body: &str) -> String {
    let marker = "RECOMP_ICALL_SAFE_AT(_icall_target, _icall_esp, ";
    let mut out = String::new();
    let mut rest = body;
    while let Some(at) = rest.find(marker) {
        out.push_str(&rest[..at]);
        let tail = &rest[at + marker.len()..];
        let end = tail.find(')').unwrap_or(0);
        out.push_str("sv_stub(0xC0DE0001u");
        rest = &tail[end..];
    }
    out.push_str(rest);
    out
}
