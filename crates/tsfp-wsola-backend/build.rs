use std::{env, path::PathBuf, process::Command};
fn main() {
    if env::var_os("CARGO_FEATURE_C_ORACLE").is_none() {
        return;
    }
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("../..");
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    println!("cargo:rerun-if-changed={}", root.join("src/host").display());
    println!("cargo:rerun-if-changed=tests/facade.c");
    let mut objects = Vec::new();
    for (i, source) in [
        root.join("crates/tsfp-audio-conformance/oracle/audio_stretch.c"),
        root.join("crates/tsfp-wsola-backend/tests/facade.c"),
    ]
    .iter()
    .enumerate()
    {
        let object = out.join(format!("part{i}.o"));
        assert!(
            Command::new(env::var_os("CC").unwrap_or_else(|| "cc".into()))
                .args(["-std=c11", "-c"])
                .arg(format!(
                    "-I{}",
                    root.join("crates/tsfp-audio-conformance/oracle").display()
                ))
                .arg(source)
                .arg("-o")
                .arg(&object)
                .status()
                .unwrap()
                .success()
        );
        objects.push(object);
    }
    let mut ar = Command::new(env::var_os("AR").unwrap_or_else(|| "ar".into()));
    assert!(
        ar.arg("crs")
            .arg(out.join("libwsola_reference.a"))
            .args(objects)
            .status()
            .unwrap()
            .success()
    );
    println!("cargo:rustc-link-search=native={}", out.display());
    println!("cargo:rustc-link-lib=static:+whole-archive=wsola_reference");
}
