// SPDX-License-Identifier: GPL-3.0-or-later
//! Links the C host (CMake `-DTSFP_RUST_MAIN=ON -DTSFP_RUST_PORTS=ON -DTSFP_RUST_BEVY=ON`, target tsfp_host_bevylink) into
//! the Bevy shell. Same method as crates/tsfp-game/build.rs (T2001): the link line comes from `ninja -t commands`, minus the
//! stub object (src/host/bevy_sink.c: a C main and stand-ins for the symbols this crate defines) and `-o`.
//!
//! Input: TSFP_BEVY_HOST_BUILD_DIR, an existing CMake build dir (relative to the repository root, or absolute), configured as
//! above. Without it (`cargo check`, `cargo test`) the binary is a stub (`cfg(tsfp_no_host)`).
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

/// Hash of the ports crate sources (sorted file list, per-file hashes hashed again), the same stamp as crates/tsfp-game (T2030).
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
    let Some(output) = Command::new("sha256sum")
        .args(&files)
        .output()
        .ok()
        .filter(|o| o.status.success())
    else {
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
    println!("cargo:rerun-if-env-changed=TSFP_BEVY_HOST_BUILD_DIR");
    println!("cargo:rerun-if-env-changed=TSFP_GIT_SHA");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rustc-env=TSFP_BUILD_GIT_SHA={}", git_sha(&root));
    println!(
        "cargo:rustc-env=TSFP_BUILD_PORTS_SRC={}",
        ports_source_hash(&root)
    );
    println!("cargo:rustc-check-cfg=cfg(tsfp_no_host)");
    let Ok(dir) = std::env::var("TSFP_BEVY_HOST_BUILD_DIR") else {
        println!("cargo:rustc-cfg=tsfp_no_host");
        println!("cargo:rustc-env=TSFP_BEVY_HOST_BUILD_DIR=none");
        println!("cargo:rustc-env=TSFP_BUILD_PORTS_ARCHIVE=none");
        return;
    };
    let build_dir = root.join(dir); // an absolute `dir` replaces the root
    let cache = std::fs::read_to_string(build_dir.join("CMakeCache.txt")).unwrap_or_default();
    assert!(
        cache
            .lines()
            .any(|line| line.starts_with("TSFP_RUST_BEVY:BOOL=ON")),
        "{} is not configured with -DTSFP_RUST_BEVY=ON (docs/t-bevy-shell.md)",
        build_dir.display()
    );
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
        .args(["-j", &jobs, "tsfp_host_bevylink"]));
    let commands = run(Command::new("ninja").current_dir(&build_dir).args([
        "-t",
        "commands",
        "tsfp_host_bevylink",
    ]));
    let link = commands
        .lines()
        .rev()
        .find(|line| line.contains("bevy_sink.c.o"))
        .expect("link command");
    let mut tokens = link.split_whitespace().peekable();
    for token in tokens.by_ref() {
        if token == "&&" {
            break; // CMake writes `: && <driver> ... && :`
        }
    }
    tokens.next(); // compiler driver
    let mut arguments: Vec<String> = Vec::new();
    while let Some(token) = tokens.next() {
        if token == "&&" {
            break;
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
        // The stub object: matches `bevy_sink.c.o` but not `audio_sink_bevy.c.o` or `present_sink.c.o`.
        if token.ends_with("/bevy_sink.c.o") || token == "bevy_sink.c.o" {
            continue;
        }
        arguments.push(token.to_string());
    }
    // CMake shell-quotes $ORIGIN. Cargo passes arguments directly to the linker,
    // so remove that quoting and collect fallbacks after our bundled-library path.
    let mut rpaths = vec!["$ORIGIN".to_string()];
    let mut ports_hash = "none".to_string();
    for argument in arguments {
        if let Some(paths) = argument
            .trim_matches(['"', '\''])
            .strip_prefix("-Wl,-rpath,")
        {
            let paths = paths.replace(['\\', '\"', '\''], "");
            for path in paths.split(':').filter(|path| !path.is_empty()) {
                if !rpaths.iter().any(|existing| existing == path) {
                    rpaths.push(path.to_string());
                }
            }
            continue;
        }
        // T2246: the render runtime comes from the tsfp-rt-host rlib (one copy of wgpu in the process), never from the shared object.
        if argument.ends_with("libtsfp_rt_host.so") {
            continue;
        }
        let looks_path =
            (argument.ends_with(".o") || argument.ends_with(".a") || argument.ends_with(".so"))
                && !argument.starts_with('-');
        let value = if looks_path && Path::new(&argument).is_relative() {
            build_dir.join(&argument).display().to_string()
        } else {
            argument
        };
        if looks_path {
            // T2030: every object and archive on the link line is a build input, else a ninja rebuild is invisible to cargo.
            println!("cargo:rerun-if-changed={value}");
            if value.ends_with("libtsfp_ports.a") {
                ports_hash = file_hash(Path::new(&value));
            }
        }
        // T3931: only the fileio capi cdylib references kernel_file_*, so --as-needed would drop
        // the native runtime from NEEDED and the capi would fail to load (undefined symbol).
        let keep = value.ends_with("libtsfp_kernel_file_runtime.so")
            || value == "-ltsfp_kernel_file_runtime";
        if keep {
            println!("cargo:rustc-link-arg-bins=-Wl,--no-as-needed");
        }
        println!("cargo:rustc-link-arg-bins={value}");
        if keep {
            println!("cargo:rustc-link-arg-bins=-Wl,--as-needed");
        }
    }
    // T2417: a -DTSFP_CALL_COUNT=ON host needs its `sub_XXXXXXXX` symbols in the dynamic table, else dladdr names nothing.
    if cache
        .lines()
        .any(|line| line.starts_with("TSFP_CALL_COUNT:BOOL=ON"))
    {
        println!("cargo:rustc-link-arg-bins=-Wl,--export-dynamic");
    }
    println!("cargo:rustc-link-arg-bins=-Wl,-rpath,{}", rpaths.join(":"));
    println!("cargo:rustc-env=TSFP_BUILD_PORTS_ARCHIVE={ports_hash}");
    // libtsfp_ports.a is a no_std staticlib with its own panic handler and eh personality, the two symbols std defines (T2001).
    println!("cargo:rustc-link-arg-bins=-Wl,--allow-multiple-definition");
    println!(
        "cargo:rustc-env=TSFP_BEVY_HOST_BUILD_DIR={}",
        build_dir.display()
    );
}
