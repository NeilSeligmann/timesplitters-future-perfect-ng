// SPDX-License-Identifier: GPL-3.0-or-later
//! Linear base of the fs segment (the guest thread's KPCR/TIB), `XBOX_FS_BASE` of the lifted C (T2128).
//! The host sets it with `set_fs_base` when it creates the guest thread, 0 until then like the lifted
//! `g_fs_base`. One global stands in for the per-thread value: a port that runs on several guest threads
//! must set it on entry. T3605: hosted here so the `tsfp-ports` facade and all family crates share one value.
use core::sync::atomic::{AtomicU32, Ordering};

static FS_BASE: AtomicU32 = AtomicU32::new(0);

pub fn fs_base() -> u32 {
    FS_BASE.load(Ordering::Relaxed)
}

pub fn set_fs_base(base: u32) {
    FS_BASE.store(base, Ordering::Relaxed)
}
