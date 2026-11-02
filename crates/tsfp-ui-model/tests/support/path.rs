use std::path::PathBuf;

pub fn layout_capture() -> PathBuf {
    if let Ok(path) = std::env::var("TSFP_UI_LAYOUT") {
        return path.into();
    }
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tmp/ui-tree/layout.json")
}
