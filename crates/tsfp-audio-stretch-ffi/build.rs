use std::{env, path::PathBuf, process::Command};

fn main() {
    if env::var_os("CARGO_FEATURE_ORACLE_TEST").is_none() {
        return;
    }
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("../..");
    // Track C oracle include search roots, including transitive local headers.
    for dir in ["src/host", "src/gpu", "src/xbox"] {
        println!("cargo:rerun-if-changed={}", root.join(dir).display());
    }
    let source = root.join("crates/tsfp-audio-conformance/oracle/audio_stretch.c");
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let object = out.join("audio_stretch.o");
    let fill_source = root.join("crates/tsfp-audio-stretch-ffi/tests/fill_oracle.c");
    let fill_object = out.join("fill_oracle.o");
    let archive = out.join("libstretch_oracle.a");
    println!("cargo:rerun-if-changed={}", source.display());
    println!("cargo:rerun-if-changed={}", fill_source.display());
    println!(
        "cargo:rerun-if-changed={}",
        root.join("src/host/present_sink.c").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        root.join("crates/tsfp-audio-conformance/oracle/audio_stretch.h")
            .display()
    );
    assert!(
        Command::new(env::var_os("CC").unwrap_or_else(|| "cc".into()))
            .args(["-std=c11", "-c"])
            .arg(&source)
            .arg("-o")
            .arg(&object)
            .status()
            .unwrap()
            .success()
    );
    assert!(
        Command::new(env::var_os("CC").unwrap_or_else(|| "cc".into()))
            .args(["-std=c11", "-ffunction-sections", "-fdata-sections", "-c"])
            .arg("-DTSFP_WSOLA_C_ORACLE=1")
            .arg(format!(
                "-I{}",
                root.join("crates/tsfp-audio-conformance/oracle").display()
            ))
            .arg(format!("-I{}", root.join("src/gpu").display()))
            .arg(format!("-I{}", root.join("src/xbox").display()))
            .arg(&fill_source)
            .arg("-o")
            .arg(&fill_object)
            .status()
            .unwrap()
            .success()
    );
    assert!(
        Command::new(env::var_os("AR").unwrap_or_else(|| "ar".into()))
            .arg("crs")
            .arg(&archive)
            .arg(&object)
            .arg(&fill_object)
            .status()
            .unwrap()
            .success()
    );
    println!("cargo:rustc-link-search=native={}", out.display());
    println!("cargo:rustc-link-lib=static=stretch_oracle");
}
