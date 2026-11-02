// SPDX-License-Identifier: GPL-3.0-or-later
use std::{env, path::PathBuf, process::Command};
fn main() {
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_C_ORACLE");
    if env::var_os("CARGO_FEATURE_C_ORACLE").is_none() {
        return;
    }
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("../..");
    let output = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let mut objects = Vec::new();
    for name in [
        "hal",
        "config",
        "identity",
        "crypto",
        "disk_identity",
        "av",
        "dbg",
        "fpstate",
        "eeprom",
        "adapter",
    ] {
        let source = match name {
            "eeprom" => root.join("src/xbox/xnet_eeprom.c"),
            "adapter" => root.join("crates/tsfp-kernel-services/tests/oracle.c"),
            _ => root.join(format!("src/xbox/kernel_{name}.c")),
        };
        println!("cargo:rerun-if-changed={}", source.display());
        let object = output.join(format!("{name}.o"));
        let status = Command::new("cc")
            .args([
                "-std=c11",
                "-D_GNU_SOURCE",
                "-O1",
                "-Wall",
                "-Wextra",
                "-Werror",
                "-c",
            ])
            .arg(&source)
            .arg("-I")
            .arg(root.join("src/xbox"))
            .arg("-o")
            .arg(&object)
            .status()
            .unwrap();
        assert!(status.success(), "C oracle compilation failed: {name}");
        objects.push(object);
    }
    // Headers are also oracle inputs (constants/contracts), not just the .c files.
    for entry in std::fs::read_dir(root.join("src/xbox")).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|x| x == "h") {
            println!("cargo:rerun-if-changed={}", path.display());
        }
    }
    let archive = output.join("libkernel_services_oracle.a");
    let status = Command::new("ar")
        .arg("crs")
        .arg(archive)
        .args(objects)
        .status()
        .unwrap();
    assert!(status.success());
    println!("cargo:rustc-link-search=native={}", output.display());
    println!("cargo:rustc-link-lib=static=kernel_services_oracle");
    println!("cargo:rustc-link-lib=pthread");
}
