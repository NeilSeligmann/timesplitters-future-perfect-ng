// SPDX-License-Identifier: GPL-3.0-or-later
//! T3742 test harness build: when the PRIVATE lifted tree is present (`TSFP_LIFTED_DIR`, else
//! `generated/lifted/gen`; gitignored, never committed), extract the lifted originals the
//! integration test compares against into `libt2o_lifted_test.a` in OUT_DIR and set
//! `cfg(t2o_lifted)`. The archive defines the lifted register globals, so only the test links it
//! (`#[link]` in tests/lifted_math.rs). Kept out of tsfp-t2-offline so the crate the host binary
//! links has no environment-dependent build input.
use std::{
    collections::BTreeSet,
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

/// Lifted originals of the integration test (engine/math ports with c2r Rust versions).
const LIFTED: &[u32] = &[0x1531D0, 0x1515E0, 0x5F430];

fn compile(source: &Path, object: &Path, include: Option<&Path>) {
    let mut command = Command::new(env::var("CC").unwrap_or_else(|_| "cc".into()));
    command.args([
        "-std=gnu11",
        "-fPIC",
        "-O1",
        "-fno-strict-aliasing",
        "-w",
        "-c",
    ]);
    if let Some(dir) = include {
        command.arg("-I").arg(dir);
    }
    let status = command
        .arg(source)
        .arg("-o")
        .arg(object)
        .status()
        .expect("spawn cc");
    assert!(status.success(), "cc {} failed", source.display());
}

fn archive(out: &Path, name: &str, objects: &[PathBuf]) {
    let library = out.join(format!("lib{name}.a"));
    let _ = fs::remove_file(&library);
    let status = Command::new(env::var("AR").unwrap_or_else(|_| "ar".into()))
        .arg("crs")
        .arg(&library)
        .args(objects)
        .status()
        .expect("spawn ar");
    assert!(status.success(), "ar {name} failed");
}

fn lifted_dir(manifest: &Path) -> Option<PathBuf> {
    println!("cargo:rerun-if-env-changed=TSFP_LIFTED_DIR");
    let dir = env::var("TSFP_LIFTED_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| manifest.join("../../generated/lifted/gen"));
    dir.join("recomp_types.h").is_file().then_some(dir)
}

fn extract(dir: &Path, va: u32) -> String {
    let header = format!("void sub_{va:08X}(void)");
    for index in 0..400 {
        let Ok(text) = fs::read_to_string(dir.join(format!("recomp_{index:04}.c"))) else {
            continue;
        };
        let Some(start) = text.find(&format!("\n{header}\n")) else {
            continue;
        };
        let body = &text[start + 1..];
        let end = body.find("\n}\n").expect("function end") + 3;
        return body[..end].to_string();
    }
    panic!("lifted sub_{va:08X} not found");
}

/// The lifted test object: each root renamed `t2or_<VA>`, every other `sub_<VA>` it names
/// renamed `t2ocs_<VA>` and forwarded to the harness callee stub `t2o_test_callee`.
fn lifted_source(dir: &Path) -> String {
    let mut stubs = BTreeSet::new();
    let mut bodies = String::new();
    for &va in LIFTED {
        let mut body = extract(dir, va);
        let mut rest = body.clone();
        while let Some(at) = rest.find("sub_") {
            if let Ok(callee) = u32::from_str_radix(rest.get(at + 4..at + 12).unwrap_or(""), 16) {
                if callee != va {
                    stubs.insert(callee);
                }
            }
            rest = rest[at + 4..].to_string();
        }
        for callee in &stubs {
            body = body.replace(&format!("sub_{callee:08X}"), &format!("t2ocs_{callee:08X}"));
        }
        bodies.push_str(&body.replace(&format!("sub_{va:08X}"), &format!("t2or_{va:08X}")));
    }
    let mut source = String::from(
        "#define RECOMP_GENERATED_CODE\n#include \"recomp_funcs.h\"\n#include <math.h>\n#include <setjmp.h>\n\
         ptrdiff_t g_xbox_mem_offset;\n\
         __thread uint32_t g_eax, g_ecx, g_edx, g_esp, g_esi, g_ebx, g_edi, g_ebp, g_fs_base;\n\
         __thread int g_df, g_fp_top, g_fp_cmp; __thread uint16_t g_fp_control_word, g_fp_cc;\n\
         __thread double g_fp_stack[8];\n\
         __thread RecompXmm g_xmm0, g_xmm1, g_xmm2, g_xmm3, g_xmm4, g_xmm5, g_xmm6, g_xmm7;\n\
         __attribute__((weak)) int g_force_return; __attribute__((weak)) __thread uint32_t g_seh_ebp;\n\
         __attribute__((weak)) __thread uint32_t g_flag_bridge_eflags, g_flag_bridge_mask;\n\
         static jmp_buf t2o_noreturn;\n\
         void recomp_noreturn_returned(uint32_t callee_va, uint32_t site_va) { longjmp(t2o_noreturn, 1); }\n\
         recomp_func_t recomp_lookup(uint32_t va) { return 0; }\n\
         recomp_func_t recomp_lookup_manual(uint32_t va) { return 0; }\n\
         recomp_func_t recomp_lookup_kernel(uint32_t va) { return 0; }\n\
         void recomp_icall_unresolved_trap(uint32_t va, const char *kind) { __builtin_trap(); }\n\
         typedef struct { double stack[8]; uint32_t top; uint16_t control; } t2o_fpu;\n\
         typedef struct { uint32_t eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags; t2o_fpu fpu; RecompXmm xmm[8]; } t2o_regs;\n\
         _Static_assert(sizeof(t2o_regs) == 240, \"ExtRegisters\");\n\
         static void t2o_load(const t2o_regs *r) {\n\
           g_eax = r->eax; g_ecx = r->ecx; g_edx = r->edx; g_esp = r->esp; g_ebx = r->ebx; g_ebp = r->ebp;\n\
           g_esi = r->esi; g_edi = r->edi; g_seh_ebp = r->ebp; g_df = (r->eflags & 0x400u) != 0;\n\
           memcpy(g_fp_stack, r->fpu.stack, sizeof g_fp_stack); g_fp_top = (int)r->fpu.top; g_fp_control_word = r->fpu.control;\n\
           g_xmm0 = r->xmm[0]; g_xmm1 = r->xmm[1]; g_xmm2 = r->xmm[2]; g_xmm3 = r->xmm[3];\n\
           g_xmm4 = r->xmm[4]; g_xmm5 = r->xmm[5]; g_xmm6 = r->xmm[6]; g_xmm7 = r->xmm[7];\n\
         }\n\
         static void t2o_store(t2o_regs *r) {\n\
           r->eax = g_eax; r->ecx = g_ecx; r->edx = g_edx; r->esp = g_esp; r->ebx = g_ebx; r->ebp = g_ebp;\n\
           r->esi = g_esi; r->edi = g_edi; r->eflags = g_df ? 0x400u : 0u;\n\
           memcpy(r->fpu.stack, g_fp_stack, sizeof g_fp_stack); r->fpu.top = (uint32_t)g_fp_top & 7u; r->fpu.control = g_fp_control_word;\n\
           r->xmm[0] = g_xmm0; r->xmm[1] = g_xmm1; r->xmm[2] = g_xmm2; r->xmm[3] = g_xmm3;\n\
           r->xmm[4] = g_xmm4; r->xmm[5] = g_xmm5; r->xmm[6] = g_xmm6; r->xmm[7] = g_xmm7;\n\
         }\n\
         /* The harness callee stub (Rust): the lifted caller already pushed the return PC. */\n\
         static void (*t2o_callee_hook)(uint32_t va, t2o_regs *regs);\n\
         void t2o_test_set_callee_hook(void (*hook)(uint32_t, t2o_regs *)) { t2o_callee_hook = hook; }\n\
         static void t2o_test_callee(uint32_t va) {\n\
           t2o_regs r; t2o_store(&r); uint32_t ebp = g_ebp;\n\
           if (t2o_callee_hook) t2o_callee_hook(va, &r); else __builtin_trap();\n\
           t2o_load(&r); g_ebp = ebp;\n\
         }\n",
    );
    for va in &stubs {
        source.push_str(&format!(
            "void t2ocs_{va:08X}(void) {{ t2o_test_callee(0x{va:X}u); }}\n"
        ));
    }
    for va in LIFTED {
        source.push_str(&format!("void t2or_{va:08X}(void);\n"));
    }
    source.push_str(&bodies);
    source.push_str("int t2o_test_run_lifted(uint32_t va, t2o_regs *r, ptrdiff_t base)\n{\n    void (*fn)(void) = 0;\n    switch (va) {\n");
    for va in LIFTED {
        source.push_str(&format!("    case 0x{va:X}u: fn = t2or_{va:08X}; break;\n"));
    }
    source.push_str(
        "    default: return 0;\n    }\n    g_xbox_mem_offset = base;\n    t2o_load(r);\n\
         if (setjmp(t2o_noreturn) == 0) fn();\n    t2o_store(r);\n    return 1;\n}\n",
    );
    source
}

fn main() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let out = PathBuf::from(env::var("OUT_DIR").expect("out dir"));
    println!("cargo:rustc-check-cfg=cfg(t2o_lifted)");
    if let Some(dir) = lifted_dir(&manifest) {
        let source = out.join("t2o_lifted_private.c");
        fs::write(&source, lifted_source(&dir)).expect("write lifted extract");
        let object = out.join("t2o_lifted_private.o");
        compile(&source, &object, Some(&dir));
        archive(&out, "t2o_lifted_test", &[object]);
        println!("cargo:rustc-link-search=native={}", out.display());
        println!("cargo:rustc-cfg=t2o_lifted");
    }
}
