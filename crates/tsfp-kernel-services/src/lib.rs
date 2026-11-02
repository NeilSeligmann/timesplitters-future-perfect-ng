// SPDX-License-Identifier: GPL-3.0-or-later
//! K-3 kernel services. Contract: docs/t-contract-kernel-services.md.
#![forbid(unsafe_code)]
pub mod crypto;
pub mod debug;
pub mod identity;
pub mod services;
pub mod settings;
pub use services::*;

pub const SUCCESS: u32 = 0;
pub const INVALID_PARAMETER: u32 = 0xc000000d;
pub const ACCESS_VIOLATION: u32 = 0xc0000005;
pub const INSUFFICIENT_RESOURCES: u32 = 0xc000009a;

/// Adapter owns mapping permissions and lifetime. No host pointer escapes this boundary.
/// Reject null addresses and ranges beyond 4 GiB, including overflow, before access.
/// Bulk copies may leave a prefix on failure, matching the C accessor contract.
pub trait GuestMemory {
    fn readable(&self, address: u32, length: usize) -> bool;
    fn read(&self, address: u32, output: &mut [u8]) -> bool;
    fn write(&mut self, address: u32, input: &[u8]) -> bool;
    fn byte(&self, address: u32) -> Option<u8> {
        let mut b = [0];
        self.read(address, &mut b).then_some(b[0])
    }
    fn word(&self, address: u32) -> Option<u32> {
        let mut b = [0; 4];
        self.read(address, &mut b).then_some(u32::from_le_bytes(b))
    }
    fn put_word(&mut self, address: u32, value: u32) -> bool {
        self.write(address, &value.to_le_bytes())
    }
}

pub fn add(base: u32, offset: u32) -> u32 {
    if base == 0 {
        0
    } else {
        base.checked_add(offset).unwrap_or(0)
    }
}

/// These are callable exports only; data exports are explicit publication methods.
/// Every listed export is Tier 1 against the C host model on synthetic inputs.
/// No listed export has Tier 2 or newly measured xemu evidence in T2208.
pub const ORDINALS: &[(u32, usize)] = &[
    (1, 0),
    (2, 4),
    (3, 6),
    (4, 1),
    (8, 1),
    (24, 5),
    (47, 2),
    (49, 1),
    (95, 1),
    (139, 1),
    (142, 1),
    (252, 1),
    (253, 2),
    (335, 1),
    (336, 3),
    (337, 2),
    (338, 3),
    (339, 3),
    (340, 7),
    (346, 2),
];

/// Fatal effects are returned to the host, never disguised as a successful reboot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Effect {
    Firmware {
        routine: u32,
        pending_notifications: usize,
    },
    BugCheck {
        code: Option<u32>,
    },
    Debug(Vec<u8>),
    Fabricated(&'static str),
    Refused(&'static str),
    FpAnomaly,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Answer {
    pub eax: u32,
    pub effects: Vec<Effect>,
}
impl Answer {
    pub fn status(eax: u32) -> Self {
        Self {
            eax,
            effects: Vec::new(),
        }
    }
    pub fn effect(eax: u32, effect: Effect) -> Self {
        Self {
            eax,
            effects: vec![effect],
        }
    }
}

/// One extension seam for Rust implementation and the retained C oracle.
/// Caller serializes calls and supplies stable per-thread IDs; callbacks run after return.
pub trait KernelServices {
    fn call(
        &mut self,
        ordinal: u32,
        args: &[u32],
        thread: u64,
        memory: &mut dyn GuestMemory,
    ) -> Option<Answer>;
}

/// Per-ordinal A/B selection is default OFF; both implementations remain available.
/// Changing selection during a stateful session requires replay/reset by the caller.
#[derive(Default, Debug)]
pub struct Selection {
    rust_ordinals: std::collections::BTreeSet<u32>,
}
impl Selection {
    pub fn select(&mut self, ordinal: u32, rust: bool) -> bool {
        if !ORDINALS.iter().any(|&(o, _)| o == ordinal) {
            return false;
        }
        if rust {
            self.rust_ordinals.insert(ordinal);
        } else {
            self.rust_ordinals.remove(&ordinal);
        }
        true
    }
    pub fn rust(&self, ordinal: u32) -> bool {
        self.rust_ordinals.contains(&ordinal)
    }
    pub fn call(
        &self,
        rust: &mut dyn KernelServices,
        oracle: &mut dyn KernelServices,
        ordinal: u32,
        args: &[u32],
        thread: u64,
        memory: &mut dyn GuestMemory,
    ) -> Option<Answer> {
        if self.rust(ordinal) {
            rust.call(ordinal, args, thread, memory)
        } else {
            oracle.call(ordinal, args, thread, memory)
        }
    }
}
