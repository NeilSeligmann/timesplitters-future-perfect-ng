//! T2189 UI-6 slice 1: runs the ported (c2r) menu page builders against a recording toolkit.
//! `toolkit` is the `GuestCall` backend (the page-building calls become `PageBuilder` calls),
//! `compare` compares a recorded page with a page loaded from a `tools.ui_tree` dump.
pub mod compare;
pub mod image;
pub mod native;
pub mod tables;
pub mod toolkit;

/// Link stub: a few translated modules reached through `c2r::run` call the host's time stamp
/// counter. The offline builders never read it, so a constant is correct for them.
#[unsafe(no_mangle)]
pub extern "C" fn xbox_ReadTimeStampCounter() -> u64 {
    0
}
