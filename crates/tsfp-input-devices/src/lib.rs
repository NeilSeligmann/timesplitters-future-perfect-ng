// SPDX-License-Identifier: GPL-3.0-or-later
//! Rust-owned state behind the old `devices_mutex` of `src/input/xinput_devices.c` (T3926).
//!
//! Guest memory stays host-owned: this crate keeps the lock, the pad lifecycle, the host handle
//! table and the pending feedback addresses, and answers the decisions the C adapter used to make
//! from its static variables. The C adapter keeps the guest-memory copies and refusals.

pub mod host;
pub mod host_source;
pub mod hotkey;
pub mod lock;
pub mod mu;
pub mod pad;
pub mod poll;
pub mod script;

/// Guest status of a feedback transfer still in flight.
pub const ERROR_IO_PENDING: u32 = 997;
/// Guest status of a feedback transfer cut short by a disconnect or close.
pub const FEEDBACK_CANCELLED: u32 = 0x1F;
