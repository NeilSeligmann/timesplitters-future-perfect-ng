use std::{env, path::PathBuf, process::Command};
fn main() {
    if env::var_os("CARGO_FEATURE_C_ORACLE").is_none() {
        return;
    }
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("../..");
    let source = root.join("crates/tsfp-audio-conformance/oracle/audio_stretch.c");
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let obj = out.join("stretch.o");
    println!("cargo:rerun-if-changed={}", source.display());
    println!("cargo:rerun-if-changed={}", root.join("src/host").display());
    assert!(
        Command::new(env::var_os("CC").unwrap_or_else(|| "cc".into()))
            .args(["-std=c11", "-c"])
            .arg(source)
            .arg("-o")
            .arg(&obj)
            .status()
            .unwrap()
            .success()
    );
    assert!(
        Command::new(env::var_os("AR").unwrap_or_else(|| "ar".into()))
            .arg("crs")
            .arg(out.join("libstretch.a"))
            .arg(obj)
            .status()
            .unwrap()
            .success()
    );
    println!("cargo:rustc-link-search=native={}", out.display());
    println!("cargo:rustc-link-lib=static=stretch");
}
