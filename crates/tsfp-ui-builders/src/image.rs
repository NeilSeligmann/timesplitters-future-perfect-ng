//! Read-only view of the static data of the owner's `default.xbe` (private, read at run time).
//!
//! The ported page builders read constants and tables that live in the image (float constants,
//! settings row descriptors, text id arrays). The tests map them behind the synthetic memory so a
//! builder sees the real static tables without any game bytes entering the repository. Without an
//! image the tests fall back to purely synthetic memory (and report it).

use std::path::Path;

pub use tsfp_ui_model::xbe::XbeImage;

/// The image named by `TSFP_XBE`, or `build/default.xbe` of the repository root (main checkout).
pub fn find_image() -> Option<XbeImage> {
    if let Ok(p) = std::env::var("TSFP_XBE") {
        return XbeImage::load(Path::new(&p));
    }
    XbeImage::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/default.xbe"))
}
