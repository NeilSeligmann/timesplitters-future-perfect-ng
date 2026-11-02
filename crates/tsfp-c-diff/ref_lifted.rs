// SPDX-License-Identifier: GPL-3.0-or-later
//! T2007 lifted reference for the whole T1836 differential suite: compiles the private lifted
//! original (gitignored, never committed) of EVERY root in the `ROOTS` X-macro of c/shim.c into one
//! static object at build time. Function text goes to OUT_DIR only. Every guest call a root makes
//! lands in a generated `t20cs_*` stub that forwards to the harness callee stub (`recomp_lookup`
//! in c/shim.c), exactly like the calls of the C replacements, so the Rust port and the lifted
//! original see the same opaque callees. The object defines `t20_lifted_lookup`, which shim.c
//! consults when `TSFP_REF_LIFTED` is set. Without the lifted tree nothing is built and shim.c
//! keeps its weak default (the C replacement is the reference).
use std::{collections::BTreeSet, env, fs, path::{Path, PathBuf}, process::Command};

fn lifted_dir(manifest: &Path) -> Option<PathBuf> {
    println!("cargo:rerun-if-env-changed=TSFP_LIFTED_DIR");
    let dir = env::var("TSFP_LIFTED_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| manifest.join("../../generated/lifted/gen"));
    dir.join("recomp_types.h").is_file().then_some(dir)
}

/// Every `X(VA)` entry of the shim's ROOTS macro.
fn roots(shim: &str) -> Vec<u32> {
    let start = shim.find("#define ROOTS(X)").expect("ROOTS macro");
    let mut found = Vec::new();
    for line in shim[start..].lines().skip(1) {
        let Some(hex) = line.trim().strip_prefix("X(").and_then(|rest| rest.split(')').next()) else {
            break;
        };
        found.push(u32::from_str_radix(hex, 16).expect("root VA"));
        if !line.trim_end().ends_with('\\') {
            break;
        }
    }
    found
}

/// `sub_VA` function texts of every lifted source file, keyed by VA.
fn index(dir: &Path, wanted: &BTreeSet<u32>) -> std::collections::BTreeMap<u32, String> {
    let mut bodies = std::collections::BTreeMap::new();
    for number in 0..400 {
        let Ok(text) = fs::read_to_string(dir.join(format!("recomp_{number:04}.c"))) else {
            continue;
        };
        let mut current: Option<(u32, String)> = None;
        for line in text.lines() {
            if let Some(hex) = line.strip_prefix("void sub_").and_then(|rest| rest.strip_suffix("(void)")) {
                if let Ok(va) = u32::from_str_radix(hex, 16) {
                    if wanted.contains(&va) {
                        current = Some((va, String::new()));
                    }
                }
            }
            if let Some((va, body)) = current.as_mut() {
                body.push_str(line);
                body.push('\n');
                if line == "}" {
                    bodies.insert(*va, std::mem::take(body));
                    current = None;
                }
            }
        }
    }
    bodies
}

/// Roots called by other x87 roots (000CB3B0 returns in ST0; 001B7D70 composes 001B7D00, T1835).
const REAL_CALLEES: [u32; 2] = [0x000C_B3B0, 0x001B_7D00];

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

/// Always yields an object: without the lifted tree it only defines a lookup that finds nothing.
pub fn build(manifest: &Path, out: &Path, opt: &str) -> PathBuf {
    let c_file = out.join("ref_lifted_private.c");
    let object = out.join("ref_lifted_private.o");
    let compile = |include: Option<&Path>| {
        let mut command = Command::new("cc");
        command.args(["-std=gnu11", "-fPIC", "-fno-strict-aliasing", "-w"]).arg(format!("-O{opt}"));
        if let Some(dir) = include {
            command.arg("-I").arg(dir);
        }
        let status = command.arg("-c").arg(&c_file).arg("-o").arg(&object).status().expect("spawn cc");
        assert!(status.success(), "private ref lifted extract failed to compile");
    };
    let Some(dir) = lifted_dir(manifest) else {
        fs::write(&c_file, "typedef unsigned int uint32_t;\nvoid (*t20_lifted_lookup(uint32_t va))(void) { return 0; }\nvoid t20_seed_ebp(uint32_t v) { (void)v; }\n")
            .expect("write empty lookup");
        compile(None);
        return object;
    };
    let shim = fs::read_to_string(manifest.join("c/shim.c")).expect("read shim.c");
    let roots = roots(&shim);
    let wanted: BTreeSet<u32> = roots.iter().copied().collect();
    let bodies = index(&dir, &wanted);
    let mut source = String::from(
        "#define RECOMP_GENERATED_CODE\n#include \"recomp_funcs.h\"\n#include <math.h>\n\
         typedef void (*t20_fn)(void);\nextern t20_fn recomp_lookup(uint32_t va);\n\
         #define T20_WEAK __attribute__((weak))\n\
         T20_WEAK int g_force_return; T20_WEAK __thread uint32_t g_seh_ebp;\n\
         T20_WEAK __thread int g_fp_cmp; T20_WEAK __thread uint16_t g_fp_cc;\n\
         T20_WEAK __thread uint32_t g_flag_bridge_eflags, g_flag_bridge_mask;\n\
         T20_WEAK recomp_func_t recomp_lookup_manual(uint32_t va) { return 0; }\n\
         T20_WEAK recomp_func_t recomp_lookup_kernel(uint32_t va) { return 0; }\n\
         T20_WEAK void recomp_icall_unresolved_trap(uint32_t va, const char *kind) { __builtin_trap(); }\n",
    );
    let mut stubs = BTreeSet::new();
    let mut text = String::new();
    let mut present = Vec::new();
    for &va in &roots {
        let Some(body) = bodies.get(&va) else { continue };
        let found = callees(body, va);
        let mut body = body.clone();
        for callee in &found {
            // T2015: a callee that returns its result on the x87 stack must run for real, the opaque
            // stub pushes nothing and would leave the FP stack depth of the lifted body short.
            let real = REAL_CALLEES.contains(callee) && bodies.contains_key(callee);
            let prefix = if real { "t20r" } else { "t20cs" };
            body = body.replace(&format!("sub_{callee:08X}"), &format!("{prefix}_{callee:08X}"));
        }
        body = body.replace(&format!("sub_{va:08X}"), &format!("t20r_{va:08X}"));
        stubs.extend(found);
        present.push(va);
        text.push_str(&body);
    }
    for va in &stubs {
        source.push_str(&format!("void t20cs_{va:08X}(void);\n"));
    }
    for va in &present {
        source.push_str(&format!("void t20r_{va:08X}(void);\n"));
    }
    source.push_str(&text);
    for va in &stubs {
        source.push_str(&format!("void t20cs_{va:08X}(void) {{ recomp_lookup(0x{va:X}u)(); }}\n"));
    }
    source.push_str("void (*t20_lifted_lookup(uint32_t va))(void)\n{\n    switch (va) {\n");
    for va in &present {
        source.push_str(&format!("    case 0x{va:X}u: return t20r_{va:08X};\n"));
    }
    source.push_str("    default: return 0;\n    }\n}\n");
    // T2015: a frameless lifted body re-seeds EBP from g_seh_ebp at entry; hand it the harness EBP so
    // the body sees the same register the Rust port does (it otherwise always saw 0).
    source.push_str("void t20_seed_ebp(uint32_t v) { g_seh_ebp = v; }\n");
    fs::write(&c_file, source).expect("write private lifted extract");
    compile(Some(&dir));
    println!("cargo:warning=T2007 lifted reference: {} of {} roots", present.len(), roots.len());
    object
}
