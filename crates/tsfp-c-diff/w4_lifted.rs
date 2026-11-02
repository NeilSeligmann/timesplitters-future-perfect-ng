// SPDX-License-Identifier: GPL-3.0-or-later
//! T1895 wave 4: compiles selected functions of the PRIVATE lifted original (gitignored, never
//! committed) into a static object at build time. The function text is extracted into OUT_DIR
//! only. Every guest call the lifted body makes lands in a generated stub that forwards to the
//! harness callee stub (`recomp_lookup` in c/shim.c). Without the lifted tree nothing is built
//! and the `w4_lifted` cfg stays unset, so the differential tests compile to nothing.
use std::{
    collections::BTreeSet,
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

/// Roots under test (guest VA).
pub const ROOTS: &[u32] = &[
    0x45D60, // T3633: audio global mask bit query (call-free leaf).
    0x1B99B0, 0x1B99F0, 0xBE860, // T3633: inventory lookup roots used by capdiff.
    0x35DE60, 0x40260, 0x2D8060, 0xD9030, 0xD8E30, 0x1C6AA0, 0x162200, 0x1622C0, 0x87260, 0x5F430,
    0x33CC90, 0x167780, // T1920: engine/core second half
    // T2411/T3621: captured real inputs for the native weapon transfer adapter.
    0x1B6EC0, 0x35DE60, 0x40260, 0x2D8060, 0xD9030, 0xD8E30, 0x1C6AA0, 0x162200, 0x1622C0, 0x87260,
    0x5F430, 0x33CC90, 0x167780, // T1920: engine/core second half
    0x3D2E0, 0x3D300, 0x3D310, 0x3D320, 0x3D330, 0x3D340, 0x3D350, 0x3D360, 0x3DB70, 0x3ED60,
    0x3ED90, 0x454F0, 0x196450, 0x56EC0, 0x6A500, 0x18E8D0, 0x257B50, 0x288B80, 0x2BD970,
    // T1920: big five
    0x235240, 0xC1850, 0x25F520, 0x1C5DB0, // T1920: engine/memory arena core
    0x3E970, 0x3DFA0, 0x3E1E0, 0x3E240, 0x3E420, 0x3E430,
    // T2048: pool allocator and aligned arena allocation (real on the C side, nested calls included)
    0x3E0B0, 0x3EA00, 0x3E9A0, 0x3EA20, 0x3EA40, 0x3EAD0, 0x3EAF0, 0x3EBD0, 0x3ECA0, 0x3ED20,
    // T2048 slices L and I: ledgers, bounds, history reset, scratch swap
    0x3E440, 0x3E4E0, 0x3E560, 0x3E5A0, 0x3E650, 0x3E740, 0x3E820, 0x3E930,
    // T2048 follow-up: arena region initialiser (straight-line, no callees)
    0x3DF00, 0x3DF70, // T2048: arena initializer and measured conditional teardown clear
    // T2048 slice W: game_alloc_* wrappers (ledger and arena constructors)
    0x323E0, 0x36FC0, 0x37410, 0x18CA0, 0x32A860, 0x2FE200, 0x336600, 0x386E0, 0x69EE0, 0x1D1E80,
    0x271910, 0x6B360, 0xB05B0, 0xC3540, 0x21AF30, 0x211040, 0x220E20, 0x220E40, 0x2210B0,
    0x2226F0, 0x2259D0, 0x2A62C0, 0x2A8300,
    0x45590, // T2048: lifted helper reached by the 0x220E20 state-block constructor
    0x2760F0, 0x307CC0, 0x274650, 0x9A0F0, 0x12C800, 0x31AAA0, 0x31C930, 0x31E430, 0x3796B0,
    0x210FF0, 0x1620C0, 0x10C0E0, 0x10C190,
    // T2048: CRT atexit table growth and memmove-equivalent realloc payload copy.
    0x3C8A84, 0x3C9800, 0x298B0,
    // T3397: HUD message aging/cue/retirement and kind-7 replay Tier 1 comparisons.
    0x14A5A0, 0x14AC80, 0x149590, 0x149810, 0x12D8E0, 0x12D9C0, 0x149710, 0x148EE0, 0x148DC0,
    // T3402: animation dependency-list prefix against the lifted original.
    0x1C8540, 0x156DB0,
    0x5B980,  // T3402: animation dependency prefix and post-load slot/policy binding
    0x60B30,  // T3402: animation instance failure-path writeback differential
    0x1506B0, // T3402: quaternion-to-matrix differential for node-9 shadow triage
    // T2048: node-pool constructor and its lifted initializer helper
    // T1920: network/syslink call-free and link-status roots
    0x2DC50, 0x2DC60, 0x2DF20, 0x2E200, 0x2E210, 0x2E220, 0x2E230, 0x2E240, 0x2E4A0, 0x2E4B0,
    0x2E500, 0x2E510, 0x2E520,
    // T2369: Story profile progress writer and record readers.
    0x668F0, 0x69900, 0x69930, 0x69980, 0x173E0, 0x17440,
    // T2409: launch-argument lookup, numeric/name handlers (engine/core launch.rs boundary).
    0x3C230, 0x3C920, 0x3C940, 0x3C970, 0x3C990, 0x3CEE0, 0x3D140, 0x3D170, 0x3D1A0, 0x3D1C0,
    0x3D1E0, 0x3D200, 0x3D2B0,
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
                if va != own && !ROOTS.contains(&va) {
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
    // Root lists are extended by independent differential slices and can overlap. Keep one
    // extracted definition and one dispatcher arm per guest VA.
    let roots: BTreeSet<u32> = ROOTS.iter().copied().collect();
    let mut source = String::from(
        "#define RECOMP_GENERATED_CODE\n#include \"recomp_funcs.h\"\n#include <math.h>\n#include <setjmp.h>\n\
         typedef void (*w4_fn)(void);\nextern w4_fn recomp_lookup(uint32_t va);\n\
         #define W4_WEAK __attribute__((weak))\n\
         W4_WEAK int g_force_return; W4_WEAK __thread uint32_t g_seh_ebp;\n\
         W4_WEAK __thread int g_fp_cmp; W4_WEAK __thread uint16_t g_fp_cc;\n\
         W4_WEAK __thread uint32_t g_flag_bridge_eflags, g_flag_bridge_mask;\n\
         static jmp_buf w4_noreturn_jb;\n\
         void recomp_noreturn_returned(uint32_t callee_va, uint32_t site_va) { longjmp(w4_noreturn_jb, 1); }\n\
         recomp_func_t recomp_lookup_manual(uint32_t va) { return 0; }\n\
         recomp_func_t recomp_lookup_kernel(uint32_t va) { return 0; }\n\
         void recomp_icall_unresolved_trap(uint32_t va, const char *kind) { __builtin_trap(); }\n",
    );
    let mut stubs = BTreeSet::new();
    let mut bodies = String::new();
    for &va in &roots {
        let body = extract(&dir, va).unwrap_or_else(|| panic!("lifted sub_{va:08X} not found"));
        let found = callees(&body, va);
        let mut body = body;
        // Rename callees so they cannot collide with replacements already in the archive.
        for callee in &found {
            body = body.replace(&format!("sub_{callee:08X}"), &format!("w4cs_{callee:08X}"));
        }
        stubs.extend(found);
        // T2048: root names are private to this object, so a src/game replacement of the same VA
        // (for example 0x3EA20) cannot collide at link time.
        for root in &roots {
            body = body.replace(&format!("sub_{root:08X}"), &format!("w4r_{root:08X}"));
        }
        bodies.push_str(&body);
    }
    for va in &stubs {
        source.push_str(&format!("void w4cs_{va:08X}(void);\n"));
    }
    for va in &roots {
        source.push_str(&format!("void w4r_{va:08X}(void);\n"));
    }
    source.push_str(&bodies);
    for va in &stubs {
        source.push_str(&format!(
            "void w4cs_{va:08X}(void) {{ recomp_lookup(0x{va:X}u)(); }}\n"
        ));
    }
    source.push_str(
        "int w4_run(uint32_t va, uint32_t r[8])\n{\n    w4_fn fn = 0;\n    switch (va) {\n",
    );
    for va in &roots {
        source.push_str(&format!("    case 0x{va:X}u: fn = w4r_{va:08X}; break;\n"));
    }
    source.push_str(
        "    default: return 0;\n    }\n    g_eax = r[0]; g_ecx = r[1]; g_edx = r[2]; g_ebx = r[3];\n\
         g_esi = r[4]; g_edi = r[5]; g_ebp = r[6]; g_esp = r[7];\n\
         g_seh_ebp = r[6]; g_df = 0; g_fp_top = 0;\n    if (setjmp(w4_noreturn_jb) == 0) fn();\n\
         r[0] = g_eax; r[1] = g_ecx; r[2] = g_edx; r[3] = g_ebx; r[4] = g_esi; r[5] = g_edi;\n\
         r[6] = g_ebp; r[7] = g_esp;\n    return 1;\n}\n",
    );
    let c_file = out.join("w4_lifted_private.c");
    fs::write(&c_file, source).expect("write private lifted extract");
    let object = out.join("w4_lifted_private.o");
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
    println!("cargo:rustc-cfg=w4_lifted");
    println!("cargo:rustc-check-cfg=cfg(w4_lifted)");
    Some(object)
}
