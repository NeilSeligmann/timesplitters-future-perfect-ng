//! Lossless format facade; see docs/t-rust-data-formats.md.
pub use tsfp_format_core as core;
pub use tsfp_format_pak as pak;
pub use tsfp_format_xbr as xbr;
pub use tsfp_format_xbt as xbt;
pub use tsfp_format_bs as bs;
pub use tsfp_format_csinfo as csinfo;
pub use tsfp_format_igcs as igcs;
pub use tsfp_format_anim as anim;
pub use tsfp_format_profile as profile;
pub use tsfp_format_script as script;

/// Opt-in runtime attachment for cutscene formats (T1908, Tier 0).
pub mod cutscenes;
