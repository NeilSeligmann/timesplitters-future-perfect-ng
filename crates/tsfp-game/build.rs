// SPDX-License-Identifier: GPL-3.0-or-later
//! Links the C host (built by CMake with -DTSFP_RUST_MAIN=ON) into the Rust binary.
//!
//! Inputs (environment, all paths relative to the repository root or absolute):
//!   TSFP_GAME_HOST_BUILD_DIR  an existing CMake build dir configured with TSFP_RUST_MAIN=ON and
//!                             TSFP_RUST_PORTS=ON (built here with `ninja tsfp_host_rustlink`), or
//!   TSFP_LIFTED_DIR           the lifted `gen` dir, to configure a fresh build under OUT_DIR/cmake.
//! The link line is taken from `ninja -t commands tsfp_host_rustlink` minus the stub C main and `-o`.
use std::path::{Path, PathBuf};
use std::process::Command;

fn run(command: &mut Command) -> String {
    let output = command
        .output()
        .unwrap_or_else(|error| panic!("{command:?}: {error}"));
    if !output.status.success() {
        panic!(
            "{command:?} failed:\n{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// First 12 hex digits of the sha256 of a file, or "unknown" when `sha256sum` or the file is missing.
fn file_hash(path: &Path) -> String {
    Command::new("sha256sum")
        .arg(path)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| {
            String::from_utf8_lossy(&o.stdout)
                .get(..12)
                .map(str::to_string)
        })
        .unwrap_or_else(|| "unknown".into())
}

/// Hash of the ports crate sources (sorted file list, then per-file hashes hashed again via sha256sum).
fn ports_source_hash(root: &Path) -> String {
    // T3605: the facade and every family crate it re-exports (`crates/tsfp-port-*`, T2399/T3433): the moved C2R
    // batches are part of the ports archive, so the stamp and the rebuild trigger cover them too.
    let mut crate_dirs: Vec<PathBuf> = vec![root.join("crates/tsfp-ports")];
    let mut families: Vec<PathBuf> = std::fs::read_dir(root.join("crates"))
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("tsfp-port-"))
        })
        .collect();
    families.sort();
    crate_dirs.extend(families);
    let mut files: Vec<PathBuf> = Vec::new();
    let mut pending: Vec<PathBuf> = Vec::new();
    for dir in &crate_dirs {
        files.push(dir.join("Cargo.toml"));
        pending.push(dir.join("src"));
        println!("cargo:rerun-if-changed={}", dir.join("src").display());
        println!(
            "cargo:rerun-if-changed={}",
            dir.join("Cargo.toml").display()
        );
    }
    while let Some(current) = pending.pop() {
        for entry in std::fs::read_dir(&current).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path)
            } else {
                files.push(path)
            }
        }
    }
    files.sort();
    let mut command = Command::new("sha256sum");
    command.args(&files);
    let Some(output) = command.output().ok().filter(|o| o.status.success()) else {
        return "unknown".into();
    };
    let listing: String = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|l| l.split_whitespace().next())
        .collect();
    let mut again = Command::new("sha256sum");
    again
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped());
    let Ok(mut child) = again.spawn() else {
        return "unknown".into();
    };
    use std::io::Write;
    let _ = child.stdin.take().unwrap().write_all(listing.as_bytes());
    child
        .wait_with_output()
        .ok()
        .and_then(|o| {
            String::from_utf8_lossy(&o.stdout)
                .get(..12)
                .map(str::to_string)
        })
        .unwrap_or_else(|| "unknown".into())
}

fn git_sha(root: &Path) -> String {
    // T3760 (rule 47): a remote exec tree has no repository; its commit arrives as data.
    println!("cargo:rerun-if-env-changed=TSFP_SOURCE_COMMIT");
    if let Ok(commit) = std::env::var("TSFP_SOURCE_COMMIT") {
        if let Some(short) = commit.get(..12) {
            return short.to_string();
        }
    }
    let from_git = Command::new("git")
        .current_dir(root)
        .args(["rev-parse", "--short=12", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());
    // Rerun when HEAD moves (the git dir of a worktree is a file, so ask git for it).
    if let Some(head) = Command::new("git")
        .current_dir(root)
        .args(["rev-parse", "--git-path", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
    {
        println!(
            "cargo:rerun-if-changed={}",
            root.join(String::from_utf8_lossy(&head.stdout).trim())
                .display()
        );
    }
    from_git
        .or_else(|| std::env::var("TSFP_GIT_SHA").ok())
        .unwrap_or_else(|| "unknown".into())
}

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let root = manifest.parent().unwrap().parent().unwrap().to_path_buf();
    for name in ["TSFP_GAME_HOST_BUILD_DIR", "TSFP_LIFTED_DIR"] {
        println!("cargo:rerun-if-env-changed={name}");
    }
    println!("cargo:rerun-if-env-changed=TSFP_GIT_SHA");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rustc-env=TSFP_BUILD_GIT_SHA={}", git_sha(&root));
    println!(
        "cargo:rustc-env=TSFP_BUILD_PORTS_SRC={}",
        ports_source_hash(&root)
    );
    println!("cargo:rustc-check-cfg=cfg(tsfp_no_host)");
    println!("cargo:rustc-check-cfg=cfg(tsfp_rust_window)");
    if std::env::var_os("TSFP_GAME_HOST_BUILD_DIR").is_none()
        && std::env::var_os("TSFP_LIFTED_DIR").is_none()
    {
        println!("cargo:rustc-cfg=tsfp_no_host");
        println!("cargo:rustc-env=TSFP_GAME_HOST_BUILD_DIR=none");
        println!("cargo:rustc-env=TSFP_BUILD_PORTS_ARCHIVE=none");
        return;
    }
    let build_dir = match std::env::var("TSFP_GAME_HOST_BUILD_DIR") {
        Ok(dir) => root.join(dir),
        Err(_) => {
            let lifted = std::env::var("TSFP_LIFTED_DIR")
                .expect("set TSFP_GAME_HOST_BUILD_DIR or TSFP_LIFTED_DIR (docs/t-rust-main.md)");
            let dir = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("cmake");
            run(Command::new("cmake")
                .current_dir(&root)
                .args(["-S", ".", "-B"])
                .arg(&dir)
                .args([
                    "-G",
                    "Ninja",
                    "-DCMAKE_C_COMPILER=clang",
                    "-DCMAKE_BUILD_TYPE=Release",
                    "-DTSFP_LIFTED_OPT=-O0",
                    "-DTSFP_RUST_PORTS=ON",
                    "-DTSFP_RUST_MAIN=ON",
                ])
                .arg(format!("-DTSFP_LIFTED_DIR={}", root.join(lifted).display())));
            dir
        }
    };
    // T2003: the C host was configured with -DTSFP_RUST_WINDOW=ON, so it expects the tsfp_wi_* symbols from sdl_window.rs.
    let cache = std::fs::read_to_string(build_dir.join("CMakeCache.txt")).unwrap_or_default();
    if cache
        .lines()
        .any(|line| line.starts_with("TSFP_RUST_WINDOW:BOOL=ON"))
    {
        println!("cargo:rustc-cfg=tsfp_rust_window");
    }
    println!(
        "cargo:rerun-if-changed={}",
        build_dir.join("CMakeCache.txt").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        build_dir.join("build.ninja").display()
    );
    let jobs = std::thread::available_parallelism()
        .map_or(4, |n| n.get().min(16))
        .to_string();
    run(Command::new("ninja")
        .current_dir(&build_dir)
        .args(["-j", &jobs, "tsfp_host_rustlink"]));
    let commands = run(Command::new("ninja").current_dir(&build_dir).args([
        "-t",
        "commands",
        "tsfp_host_rustlink",
    ]));
    let link = commands
        .lines()
        .rev()
        .find(|line| line.contains("rust_main_link_stub.c.o"))
        .expect("link command");
    let mut tokens = link.split_whitespace().peekable();
    // CMake writes `: && <driver> ... && :`; skip to the driver, stop at the trailing `&&`.
    for token in tokens.by_ref() {
        if token == "&&" {
            break;
        }
    }
    tokens.next(); // compiler driver
    let mut arguments: Vec<String> = Vec::new();
    while let Some(token) = tokens.next() {
        if token == "&&" {
            break; // CMake appends `&& :` (post-link commands).
        }
        if token == "-o"
            || token == "-Xlinker"
                && tokens
                    .peek()
                    .is_some_and(|next| next.starts_with("--dependency-file"))
        {
            tokens.next();
            continue;
        }
        if token.contains("_link_stub") && token.ends_with(".c.o") {
            continue;
        }
        // CMake quotes the rpath as one shell token and escapes $ORIGIN for the shell.
        // Cargo passes each link arg directly to the compiler, so remove that shell layer.
        arguments.push(token.trim_matches('"').replace("\\$", "$"));
    }
    let mut ports_hash = "none".to_string();
    for argument in arguments {
        // Relative object and archive paths in the ninja line are relative to the build dir.
        let looks_path =
            (argument.ends_with(".o") || argument.ends_with(".a") || argument.ends_with(".so"))
                && !argument.starts_with('-');
        let value = if looks_path && Path::new(&argument).is_relative() {
            build_dir.join(&argument).display().to_string()
        } else {
            argument
        };
        // T2030: every object and archive on the link line is a build input, else a ninja rebuild is invisible to cargo.
        if looks_path {
            println!("cargo:rerun-if-changed={value}");
            if value.ends_with("libtsfp_ports.a") {
                ports_hash = file_hash(Path::new(&value));
            }
        }
        println!("cargo:rustc-link-arg-bins={value}");
    }
    println!("cargo:rustc-env=TSFP_BUILD_PORTS_ARCHIVE={ports_hash}");
    // libtsfp_ports.a is a no_std staticlib that carries its own panic handler and eh personality, the
    // same two symbols std defines. The C link has no std, so this is the only duplicate (T2001 finding):
    // std's definitions come first and win. Panics are `panic=abort` on both sides.
    println!("cargo:rustc-link-arg-bins=-Wl,--allow-multiple-definition");
    println!(
        "cargo:rustc-env=TSFP_GAME_HOST_BUILD_DIR={}",
        build_dir.display()
    );
}
