// SPDX-License-Identifier: GPL-3.0-or-later
//! Private-disc sample reader shared by the real-pose integration suites.

pub fn stream(args: &[&str]) -> Option<Vec<u8>> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let python = std::env::var("TSFP_TEST_PYTHON").unwrap_or("python3".into());
    let output = std::process::Command::new(python)
        .current_dir(root)
        .args(["-m", "tools.rust_anim_sample"])
        .args(args)
        .output()
        .expect("Python reader");
    if output.status.code() == Some(77) {
        eprintln!("SKIP anim: private ISO absent");
        return None;
    }
    assert!(
        output.status.success(),
        "reader failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Some(output.stdout)
}
