//! The oracle is the unchanged C mixer, never a second Rust implementation. The C moved from src/audio to oracle/ when
//! the host stopped linking it (T3959): it is a test oracle only.
use std::{env, fs, path::PathBuf, process::Command};

fn run(command: &mut Command) {
    assert!(
        command.status().expect("start C oracle tool").success(),
        "C oracle build failed"
    );
}

fn main() {
    let source = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("oracle");
    let headers = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("../../src/audio");
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let cc = env::var("CC").unwrap_or_else(|_| "cc".into());
    let ar = env::var("AR").unwrap_or_else(|_| "ar".into());
    println!("cargo:rerun-if-env-changed=CC");
    println!("cargo:rerun-if-env-changed=AR");
    for entry in fs::read_dir(&source).expect("C audio sources must be staged") {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|ext| ext == "h") {
            println!("cargo:rerun-if-changed={}", path.display());
        }
    }
    let mut objects = Vec::new();
    for name in ["dsound_mixer", "dsound_adpcm", "dsound_stream_frequency"] {
        let input = source.join(format!("{name}.c"));
        let object = out.join(format!("{name}.o"));
        println!("cargo:rerun-if-changed={}", input.display());
        run(Command::new(&cc)
            .args(["-std=c11", "-O2", "-fPIC", "-c"])
            .arg(&input)
            .arg("-I")
            .arg(&source)
            .arg("-I")
            .arg(&headers)
            .arg("-o")
            .arg(&object));
        objects.push(object);
    }
    run(Command::new(ar)
        .arg("crs")
        .arg(out.join("libaudio_oracle.a"))
        .args(objects));
    println!("cargo:rustc-link-search=native={}", out.display());
    println!("cargo:rustc-link-lib=static=audio_oracle");
    println!("cargo:rustc-link-lib=m");
}
