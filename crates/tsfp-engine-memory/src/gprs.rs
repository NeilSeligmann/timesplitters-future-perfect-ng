// SPDX-License-Identifier: GPL-3.0-or-later
//! Final EAX/ECX/EDX of a register-exact guest leaf (T1836 wave 3). Included by path into the
//! ports crates, like `leaf.rs`, so no crate gains a new dependency.

/// The three volatile registers a register-exact C replacement leaves defined.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Gprs {
    pub eax: u32,
    pub ecx: u32,
    pub edx: u32,
}
