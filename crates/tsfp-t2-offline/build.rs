// SPDX-License-Identifier: GPL-3.0-or-later
//! Builds `c/t2o_glue.c` (sigsetjmp guard, weak host API lookup) into a static library linked
//! with the crate. No environment or repo inputs (the crate is in the tsfp-bevy closure); the
//! lifted-vs-port integration test lives in crates/tsfp-t2-offline-lifted.
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

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

fn main() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let out = PathBuf::from(env::var("OUT_DIR").expect("out dir"));
    println!("cargo:rerun-if-changed=c/t2o_glue.c");
    let glue = out.join("t2o_glue.o");
    compile(&manifest.join("c/t2o_glue.c"), &glue, None);
    archive(&out, "t2o_glue", &[glue]);
    println!("cargo:rustc-link-search=native={}", out.display());
    println!("cargo:rustc-link-lib=static=t2o_glue");
}
