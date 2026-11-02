// SPDX-License-Identifier: GPL-3.0-or-later
//! T3501: the c-diff lifted bundle. The private lift exists only on the main host, so a remote
//! verify host cannot run the lifted extracts of this build script. The main host therefore runs
//! this same build script once per (inputs key, lift hash) and keeps its two static archives plus
//! the cargo directives it printed as a bundle (`tools/wt/cdiff_bundle.py`), ships the bundle to
//! the verify host, and the build script there links it instead of compiling anything.
//!
//! The inputs key hashes every file this build script reads except the lift itself (the lift hash
//! is the second half of the bundle id), so a bundle can only be used by the tree it was built
//! from. The bundle holds compiled archives and directives only, never lifted C text.
use std::{
    collections::BTreeSet,
    env, fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

const KEY_VERSION: &str = "t3501-v1";
/// The archives the normal build leaves in OUT_DIR and links.
pub const ARCHIVES: [&str; 2] = ["libwave3c.a", "libmemcdiff.a"];
pub const DIRECTIVES: &str = "directives.txt";

fn names_in(dir: &Path, extensions: &[&str]) -> Vec<String> {
    let mut found: Vec<String> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|entry| entry.path().is_file())
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| {
            Path::new(name)
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| extensions.contains(&ext))
        })
        .collect();
    found.sort();
    found
}

/// Every non-lift input of the build script, as paths relative to the repository root (built
/// lexically, so the same tree gives the same names on every host and in every canonical copy).
pub fn inputs(manifest: &Path, sources: &[&str]) -> Vec<String> {
    let root = manifest.join("../..");
    let krate = manifest
        .file_name()
        .expect("crate dir")
        .to_string_lossy()
        .into_owned();
    let mut paths: BTreeSet<String> = BTreeSet::new();
    for name in names_in(manifest, &["rs", "toml"]) {
        paths.insert(format!("crates/{krate}/{name}"));
    }
    for name in names_in(&manifest.join("c"), &["c", "h", "inc"]) {
        paths.insert(format!("crates/{krate}/c/{name}"));
    }
    for name in sources
        .iter()
        .map(|name| name.to_string())
        .chain(names_in(&root.join("src/game"), &["h", "inc"]))
    {
        paths.insert(format!("src/game/{name}"));
    }
    let xbox_sources = ["kernel_rtl.c", "kernel_rtl_string.c"];
    for name in xbox_sources
        .iter()
        .map(|name| name.to_string())
        .chain(names_in(&root.join("src/xbox"), &["h", "inc"]))
    {
        paths.insert(format!("src/xbox/{name}"));
    }
    paths.into_iter().collect()
}

/// sha256 (first 32 hex digits) of the inputs' names and contents plus the optimisation level.
pub fn inputs_key(manifest: &Path, sources: &[&str], opt: &str) -> String {
    let root = manifest.join("../..");
    let files = inputs(manifest, sources);
    let listing = Command::new("sha256sum")
        .arg("--")
        .args(&files)
        .current_dir(&root)
        .output()
        .expect("spawn sha256sum");
    assert!(
        listing.status.success(),
        "sha256sum of the c-diff build inputs failed: {}",
        String::from_utf8_lossy(&listing.stderr)
    );
    let mut child = Command::new("sha256sum")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn sha256sum");
    {
        let mut stdin = child.stdin.take().expect("sha256sum stdin");
        writeln!(stdin, "{KEY_VERSION}\nopt={opt}").expect("hash header");
        stdin.write_all(&listing.stdout).expect("hash listing");
    }
    let output = child.wait_with_output().expect("sha256sum");
    String::from_utf8_lossy(&output.stdout)[..32].to_string()
}

/// Link a prebuilt bundle instead of compiling: `id` is `<inputs key>-<lift hash>`.
pub fn consume(manifest: &Path, out: &Path, sources: &[&str], opt: &str, id: &str) {
    for name in ["TSFP_CDIFF_BUNDLE_DIR", "TSFP_LIFTED_DIR", "TSFP_C_OPT"] {
        println!("cargo:rerun-if-env-changed={name}");
    }
    let base = env::var_os("TSFP_CDIFF_BUNDLE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            panic!("TSFP_CDIFF_LIFT={id} is set but TSFP_CDIFF_BUNDLE_DIR is not (T3501)")
        });
    let key = inputs_key(manifest, sources, opt);
    assert!(
        id.split('-').next() == Some(key.as_str()),
        "c-diff lifted bundle {id} was built for other inputs than this tree ({key}, T3501)"
    );
    let dir = base.join(id);
    let directives = fs::read_to_string(dir.join(DIRECTIVES)).unwrap_or_else(|error| {
        panic!(
            "c-diff lifted bundle {} is missing ({error}, T3501)",
            dir.display()
        )
    });
    for name in ARCHIVES {
        fs::copy(dir.join(name), out.join(name))
            .unwrap_or_else(|error| panic!("copy {name} from {}: {error}", dir.display()));
    }
    for line in directives.lines().filter(|line| !line.is_empty()) {
        println!("cargo:{line}");
    }
    let root = manifest.join("../..");
    for file in inputs(manifest, sources) {
        println!("cargo:rerun-if-changed={}", root.join(file).display());
    }
    println!("cargo:rerun-if-changed={}", dir.join(DIRECTIVES).display());
    println!("cargo:rustc-link-search=native={}", out.display());
}
