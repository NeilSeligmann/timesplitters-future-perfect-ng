// SPDX-License-Identifier: GPL-3.0-or-later
//! Compiles the unmodified src/game C replacement files plus c/shim.c into a static archive.
//! `TSFP_C_OPT` selects the optimisation level (default 2). The harness proves at O0 and O3.
mod bundle;
mod modes_lifted;
mod ref_lifted;
mod sv_lifted;
mod t1900_lifted;
mod w4_lifted;
use std::{env, path::PathBuf, process::Command};

const SOURCES: &[&str] = &[
    "indexed_field_pointer.c",
    "game_t1475_scalar_review.c",
    "game_synth_T1781.c",
    "game_synth_T1784.c",
    "game_synth_T1783.c",
    "game_synth_t1782.c",
    "game_ui_list001.c",
    "game_census_180000_list002.c",
    "game_t1479_appendix_census.c",
    "game_engine_batch6.c",
    "game_scalar_180000_fixed.c",
    "game_t1478_appendix_census.c",
    "game_small_logic_180000.c",
    "game_census_180000_list001.c",
    "game_gameplay_campaign.c",
    "game_census_t1479_list001.c",
    "game_census_t1479_list002.c",
    "game_ui_campaign.c",
    "object_mode_flag_pair.c",
    "game_t1475_stores.c",
    "game_t1477_census001.c",
    "game_t1477_census002.c",
    "game_round4_high_a.c",
    "game_census_180000_list003.c",
    "game_engine_batch2.c",
    "game_engine_campaign.c",
    "game_engine_batch3.c",
    "game_engine_batch4.c",
    "game_scalar_280000_fixed.c",
    "game_network_state_campaign.c",
    "game_network_selectors.c",
    "fixed_record_counter.c",
    "vertex_format_size.c",
    "game_data_campaign.c",
    "game_fesl_crypto_leaves.c",
    "game_physics_ray.c",
    "game_fp_list001.c",
    "game_fp_list002.c",
    "game_fp_step2b.c",
    "game_fp_math.c",
    "game_t1477_appendix_fp.c",
    "game_t1477_vector001.c",
    "game_t1477_vector002.c",
    "game_t1479_vector_census.c",
    "game_vec_list003.c",
    "game_vec_list001b.c",
    "game_engine_batch5.c",
    "game_gameplay_batch2.c",
    "game_vec_list001.c",
    "game_t1477_appendix_census.c",
    "game_t1478_appendix_census.c",
    "game_t1478_vector_census.c",
    "game_t1479_appendix_census.c",
    "game_engine_batch6.c",
    "game_fesl_rc4_key_setup.c",
    "game_fesl_base64_encode.c",
    "game_calls_continuation_batch1.c",
    "game_af_list001.c",
    "game_band_280000_380000_calls.c",
    "game_band_180000_280000_calls.c",
    "game_band_0_80000_calls.c",
    "int_array_search.c",
    "game_calls_list004.c",
    "t1479_signed_table_registration.c",
    "t1479_signed_table_candidates.c",
    "game_band_0_80000.c",
    "game_band_80000_100000_llm.c",
    "game_calls_list003.c",
    "game_round4_low_a.c",
    "game_round4_low_c.c",
    "game_round4_low_d.c",
    "game_scalar_100000_retry.c",
    "game_small_logic_low_c.c",
    "game_store_leaves_low.c",
    "global_gated_object_flag.c",
    "game_calls_continuation_batch4.c",
    "game_calls_continuation_batch2.c",
    "game_calls_seventh_batch.c",
    "game_calls_list001.c",
    "game_band_80000_100000_calls.c",
    "game_band_80000_100000_table_lookup.c",
    "game_band_100000_180000_calls.c",
    "x87_roots.c",
    "game_global_leaves_180000.c",
    "game_pilot_list3.c",
    "game_pilot_list4.c",
    "game_pilot_left.c",
    "low_range_leaves.c",
];

fn run(command: &mut Command) {
    let status = command.status().expect("spawn C toolchain");
    assert!(status.success(), "C toolchain step failed: {command:?}");
}

fn main() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    let game = manifest.join("../../src/game");
    let opt = env::var("TSFP_C_OPT").unwrap_or_else(|_| "2".into());
    // T3501: `tools/wt/cdiff_bundle.py` runs this script standalone to key and build the bundle.
    if env::var_os("TSFP_CDIFF_PRINT_KEY").is_some() {
        println!("{}", bundle::inputs_key(&manifest, SOURCES, &opt));
        return;
    }
    println!("cargo:rerun-if-env-changed=TSFP_CDIFF_LIFT");
    // A verify host without the private lift links the bundle the main host built for this tree.
    if !lift_present(&manifest) {
        if let Ok(id) = env::var("TSFP_CDIFF_LIFT") {
            bundle::consume(&manifest, &out, SOURCES, &opt, &id);
            return;
        }
    }
    let mut objects = Vec::new();
    let mut inputs: Vec<PathBuf> = SOURCES.iter().map(|name| game.join(name)).collect();
    inputs.push(manifest.join("c/shim.c"));
    inputs.push(manifest.join("c/t1697_settings_stub.c"));
    for source in &inputs {
        let object = out.join(source.file_stem().unwrap()).with_extension("o");
        run(Command::new("cc")
            .args(["-std=gnu11", "-fPIC", "-fno-strict-aliasing", "-w"])
            .arg(format!("-O{opt}"))
            .arg("-I")
            .arg(&game)
            .arg("-c")
            .arg(source)
            .arg("-o")
            .arg(&object));
        println!("cargo:rerun-if-changed={}", source.display());
        objects.push(object);
    }
    println!("cargo:rustc-check-cfg=cfg(w4_lifted)");
    // Roots and callee ABI tables live in this imported module. Track it explicitly so
    // added differential roots rebuild the generated lifted dispatcher.
    println!(
        "cargo:rerun-if-changed={}",
        manifest.join("modes_lifted.rs").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        manifest.join("sv_lifted.rs").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        manifest.join("w4_lifted.rs").display()
    );
    if let Some(object) = modes_lifted::build(&manifest, &out, &opt) {
        objects.push(object);
    }
    if let Some(object) = sv_lifted::build(&manifest, &out, &opt) {
        objects.push(object);
    }
    if let Some(object) = w4_lifted::build(&manifest, &out, &opt) {
        objects.push(object);
    }
    println!("cargo:rustc-check-cfg=cfg(t1900_lifted)");
    objects.extend(t1900_lifted::build(&manifest, &out, &opt));
    println!("cargo:rustc-check-cfg=cfg(t1697_lifted)");
    if lifted_root_present(&manifest, 0x002C3AC0) {
        println!("cargo:rustc-cfg=t1697_lifted");
    }
    objects.push(ref_lifted::build(&manifest, &out, &opt));
    build_memshim(&manifest, &out, &opt);
    let archive = out.join("libwave3c.a");
    let _ = std::fs::remove_file(&archive);
    run(Command::new("ar").arg("crs").arg(&archive).args(&objects));
    println!("cargo:rerun-if-env-changed=TSFP_C_OPT");
    println!(
        "cargo:rerun-if-changed={}",
        game.join("game_replace.h").display()
    );
    println!("cargo:rustc-link-search=native={}", out.display());
    println!("cargo:rustc-link-lib=static=wave3c");
}

/// The private lift is readable here (same rule as every `*_lifted::lifted_dir`).
fn lift_present(manifest: &PathBuf) -> bool {
    std::env::var_os("TSFP_LIFTED_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| manifest.join("../../generated/lifted/gen"))
        .join("recomp_types.h")
        .is_file()
}

fn lifted_root_present(manifest: &PathBuf, va: u32) -> bool {
    let dir = std::env::var_os("TSFP_LIFTED_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| manifest.join("../../generated/lifted/gen"));
    let needle = format!("void sub_{va:08X}(void)");
    (0..400).any(|index| {
        std::fs::read_to_string(dir.join(format!("recomp_{index:04}.c")))
            .is_ok_and(|source| source.lines().any(|line| line == needle))
    })
}

/// T2048 slice K (T3935: RTL only): the unmodified src/xbox RTL C files plus c/memshim.c, in their own archive.
fn build_memshim(manifest: &PathBuf, out: &PathBuf, opt: &str) {
    let xbox = manifest.join("../../src/xbox");
    let shim = manifest.join("c/memshim.c");
    let sources = [
        shim,
        xbox.join("kernel_rtl.c"),
        xbox.join("kernel_rtl_string.c"),
    ];
    let mut objects = Vec::new();
    for source in &sources {
        let object = out
            .join(format!(
                "memcdiff_{}",
                source.file_stem().unwrap().to_string_lossy()
            ))
            .with_extension("o");
        run(Command::new("cc")
            .args(["-std=gnu11", "-fPIC", "-fno-strict-aliasing", "-w"])
            .arg(format!("-O{opt}"))
            .arg("-I")
            .arg(&xbox)
            .arg("-c")
            .arg(source)
            .arg("-o")
            .arg(&object));
        println!("cargo:rerun-if-changed={}", source.display());
        objects.push(object);
    }
    for header in [
        "nt_status.h",
        "kernel_call.h",
        "kernel_hle.h",
        "kernel_rtl.h",
        "kernel_rtl_string.h",
        "guest_structs.h",
    ] {
        println!("cargo:rerun-if-changed={}", xbox.join(header).display());
    }
    let archive = out.join("libmemcdiff.a");
    let _ = std::fs::remove_file(&archive);
    run(Command::new("ar").arg("crs").arg(&archive).args(&objects));
    println!("cargo:rustc-link-lib=static=memcdiff");
}
