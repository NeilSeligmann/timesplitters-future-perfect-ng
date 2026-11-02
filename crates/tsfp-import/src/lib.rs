//! `tsfp-import`: the one-time asset importer (T1821).
//!
//! Stage A extracts every file of a source image loose plus a hash manifest. Stage B converts
//! known containers into plain open formats. Sources are abstracted ([`source::Source`]) so the
//! Xbox, PS2 and GameCube releases share one pipeline and manifest schema.
pub mod audio;
pub mod convert;
pub mod inflate;
pub mod json;
mod ktx2;
pub mod manifest;
pub mod names;
pub mod png;
pub mod sha256;
pub mod source;
mod structured;

pub type Result<T> = std::result::Result<T, String>;
