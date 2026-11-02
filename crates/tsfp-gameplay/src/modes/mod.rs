// SPDX-License-Identifier: GPL-3.0-or-later
//! gameplay/modes system (T1896, owner rule 11): game mode, match state and mode setup
//! functions ported as readable Rust from the private lifted original. Every function here is
//! **Tier 0** (translated, compiles, smoke-tested on hand-built fixtures) unless a doc line says
//! otherwise; none has a Tier 1 differential against the original. See docs/t1896-modes.md and
//! the attachment-point report docs/t-attach-modes.md.
//!
//! Boundary: the whole system talks to the rest of the game through [`ModeHost`], which is guest
//! memory plus a recorded way to call functions that are not part of this system (network
//! session, text, HUD, player records, the other mode systems). Globals the system owns are
//! named constants below, so a later owner of that state can replace the reads and writes.
use crate::memory::GuestMemory;

pub mod challenge_setup;
pub mod match_flow;
#[cfg(feature = "motion")]
pub mod motion;
pub mod predicates;
pub mod setup;
pub mod story_activation;
pub mod story_menu;
pub mod story_sequencer;
pub mod tables;

/// Guest memory plus calls out of the system. `call` runs the guest (or Rust) function at
/// `target` with the cdecl stack arguments `args` (first argument first) and returns EAX.
pub trait ModeHost: GuestMemory {
    fn call(&mut self, target: u32, args: &[u32]) -> u32;
    /// Like [`ModeHost::call`] for a callee that returns its result as a float in ST0 (x87).
    /// The integer registers it leaves are not observable through this method.
    fn call_f32(&mut self, target: u32, args: &[u32]) -> f32;
    /// A callee that takes its single argument in ST0 and returns in ST0 (x87 library functions).
    fn call_x87_unary(&mut self, target: u32, argument: f32) -> f32;
    /// Guest stack pointer at function entry: the return address is at `[stack_pointer]` and the
    /// caller's arguments follow it. Functions that pass pointers to their own stack locals build
    /// those addresses from it with [`Esp`].
    fn stack_pointer(&self) -> u32;
}

/// x86 `cvttss2si`: truncates toward zero, and yields the "integer indefinite" 0x80000000 for NaN
/// and out-of-range values (Rust's `as i32` would saturate or give 0 instead).
pub fn cvttss2si(value: f32) -> i32 {
    if value.is_nan() || value >= 2_147_483_648.0 || value < -2_147_483_648.0 {
        i32::MIN
    } else {
        value as i32
    }
}

/// The x87 model's 24-bit precision narrowing: the significand is rounded to 24 bits (nearest
/// even) while the f64 exponent range is kept, as the lifter's `RECOMP_FP_PC` does. Mirrors
/// `round24` of `tsfp_engine_physics::fp_x87` (kept local, the crates do not depend on each other).
pub fn x87_round24(value: f64) -> f64 {
    let magnitude = value.abs();
    if (magnitude <= 3.402_823_466_385_288_6e38 && magnitude >= 1.175_494_350_822_287_5e-38)
        || magnitude == 0.0
        || value.is_nan()
        || magnitude == f64::INFINITY
    {
        return f64::from(value as f32);
    }
    let bits = value.to_bits();
    let scale = |v: f64| -> (f64, i32) {
        let raw = ((v.to_bits() >> 52) & 0x7ff) as i32;
        (
            f64::from_bits((v.to_bits() & !(0x7ff << 52)) | (1022 << 52)),
            raw - 1022,
        )
    };
    let (mantissa, exponent) = if (bits >> 52) & 0x7ff == 0 {
        let (m, e) = scale(value * f64::from_bits(0x4350_0000_0000_0000));
        (m, e - 54)
    } else {
        scale(value)
    };
    let mut result = f64::from(mantissa as f32);
    let mut exponent = exponent;
    while exponent > 1000 {
        result *= f64::from_bits(((1000 + 1023) as u64) << 52);
        exponent -= 1000;
    }
    while exponent < -1000 {
        result *= f64::from_bits(((-1000 + 1023) as u64) << 52);
        exponent += 1000;
    }
    result * f64::from_bits(((exponent + 1023) as u64) << 52)
}

/// Reads a float from guest memory.
pub fn read_f32<H: ModeHost>(h: &H, address: u32) -> f32 {
    f32::from_bits(h.read32(address))
}

/// Tracks the ESP of a function body that hands out addresses of its own stack locals (T1910
/// frame-aware host). It mirrors the original's `sub esp`, `push` and `pop` sequence, so the
/// addresses a callee sees (and the differential records in the call trace) are the original's.
#[derive(Clone, Copy, Debug)]
pub struct Esp(pub u32);

impl Esp {
    /// ESP of the entry of a function body: `host.stack_pointer()`.
    pub fn at_entry<H: ModeHost>(host: &H) -> Esp {
        Esp(host.stack_pointer())
    }
    /// `sub esp, bytes`.
    pub fn sub(&mut self, bytes: u32) {
        self.0 = self.0.wrapping_sub(bytes);
    }
    /// `add esp, bytes`.
    pub fn add(&mut self, bytes: u32) {
        self.0 = self.0.wrapping_add(bytes);
    }
    /// `push` of one dword: returns the address it was stored at.
    pub fn push(&mut self) -> u32 {
        self.0 = self.0.wrapping_sub(4);
        self.0
    }
    /// Address `esp + offset`.
    pub fn at(&self, offset: u32) -> u32 {
        self.0.wrapping_add(offset)
    }
}

/// Game mode globals (byte and dword views of the same state).
pub mod globals {
    /// Selected game mode id (dword), for example 0x64 and 0x65 are special single player modes.
    pub const GAME_MODE: u32 = 0x0079_094c;
    /// State word cleared by the mode refresh gate; its broader purpose is unresolved.
    pub const MODE_STATE_WORD: u32 = 0x0079_0958;
    /// Mode class byte: 3 and 7 skip the match-end side effects, 0xA is the multiplayer pair mode.
    pub const MODE_BYTE: u32 = 0x007d_e455;
    /// Sub-mode byte used by the mode predicates.
    pub const SUBMODE_BYTE: u32 = 0x007d_e456;
    /// Mode flag word (bit 6 and bit 27 select the "keep running" end-of-round path, bit 23 blocks
    /// the 6B98A4 transition).
    pub const MODE_FLAGS: u32 = 0x007d_e458;
    /// Per-player count of setup entries.
    pub const SETUP_ENTRY_COUNT_BYTE: u32 = 0x007d_e402;
    /// Match state machine state (dword) and its float companion.
    pub const MATCH_STATE: u32 = 0x007d_e30c;
    pub const MATCH_STATE_TIMER: u32 = 0x007d_e300;
    /// Transition state machine (6B98A4 family).
    pub const TRANSITION_STATE: u32 = 0x006b_98a4;
    pub const TRANSITION_ARG: u32 = 0x006b_98a8;
    pub const TRANSITION_FLOAT: u32 = 0x006b_98b0;
    pub const TRANSITION_FLOAT_SOURCE: u32 = 0x0047_5d18;
    pub const TRANSITION_RESULT_A: u32 = 0x006b_989c;
    pub const TRANSITION_RESULT_B: u32 = 0x006b_98a0;
    /// Pointer to the active mode setup record (see [`super::setup`]).
    pub const SETUP_RECORD_POINTER: u32 = 0x006f_1e10;
    pub const MATCH_END_PENDING: u32 = 0x006f_11a4;
}

/// Calls out of the system (VAs of the other systems' functions), named once.
pub mod callees {
    pub const NET_SESSION_ACTIVE_A: u32 = 0x0035_5f10;
    pub const NET_SESSION_ACTIVE_B: u32 = 0x0035_7690;
    pub const PLAYER_COUNT: u32 = 0x001c_dae0;
    pub const MODE_SETUP_LOOKUP: u32 = 0x0006_a850;
    pub const MATCH_END_ENTER_STATE_8: u32 = 0x0006_6fc0;
    pub const MATCH_END_NOTIFY: u32 = 0x0006_6e90;
    pub const STATE_DESCRIPTOR_COMPLETE: u32 = 0x0023_1a20;
    pub const TRANSITION_DISPATCH: u32 = 0x0004_2510;
    pub const TRANSITION_SET: u32 = 0x0004_31f0;
    pub const SLOT_PLAYER: u32 = 0x0006_4160;
    pub const PRELOAD_BY_ID: u32 = 0x000d_7780;
    pub const TEXT_BY_ID: u32 = 0x0003_d7b0;
    pub const MODE_REFRESH: u32 = 0x001c_d170;
}

#[cfg(test)]
pub(crate) mod test_host {
    use super::ModeHost;
    use crate::memory::GuestMemory;
    use std::collections::HashMap;

    /// Sparse memory plus a scripted call log, for the Tier 0 smoke tests.
    #[derive(Default)]
    pub struct TestHost {
        pub bytes: HashMap<u32, u8>,
        pub calls: Vec<(u32, Vec<u32>)>,
        /// Return values by target, default 0.
        pub returns: HashMap<u32, u32>,
        /// ST0 results by target, default 0.0.
        pub float_returns: HashMap<u32, f32>,
    }
    impl GuestMemory for TestHost {
        fn read8(&self, address: u32) -> u8 {
            self.bytes.get(&address).copied().unwrap_or(0)
        }
        fn write8(&mut self, address: u32, value: u8) {
            self.bytes.insert(address, value);
        }
    }
    impl ModeHost for TestHost {
        fn call(&mut self, target: u32, args: &[u32]) -> u32 {
            self.calls.push((target, args.to_vec()));
            self.returns.get(&target).copied().unwrap_or(0)
        }
        fn call_f32(&mut self, target: u32, args: &[u32]) -> f32 {
            self.calls.push((target, args.to_vec()));
            self.float_returns.get(&target).copied().unwrap_or(0.0)
        }
        fn call_x87_unary(&mut self, target: u32, argument: f32) -> f32 {
            self.calls.push((target, vec![argument.to_bits()]));
            self.float_returns.get(&target).copied().unwrap_or(0.0)
        }
        fn stack_pointer(&self) -> u32 {
            0x0030_0000
        }
    }
}
