//! T1877: how a spawned character receives its behaviour class mask. See `docs/t-character-anim-selection.md` section 12.
//!
//! In story mode A the spawn routine `0x15B830` copies the first 8 bytes of the SETU spawn record (stride `0x98`) into the
//! extension mask `C[0x10..0x18]` (`0x15B9A5`). `0x15F9BA..0x15FA9E` then splits the mask: the deferred classes `0x800000` and
//! `0x100000` move to the pending slot, and a record without the "activate now" bit `0x10000000` starts as the dormant class
//! `0x4000000` with the whole record mask parked in `C[0xAB8]`. Bit exact against the original (1500 Unicorn cases).

use crate::char_anim::dispatch_ticks;

/// Class that carries a character before it is activated (tick `0x26C5E0`).
pub const DORMANT_CLASS: u64 = 0x400_0000;
/// Record bit: start in the record class instead of the dormant class.
pub const ACTIVATE_NOW: u32 = 0x1000_0000;
/// Deferred classes (`0x15F9C2`) taken out of the mask first.
pub const DEFERRED_A: u32 = 0x80_0000;
pub const DEFERRED_B: u32 = 0x10_0000;

/// The mask related dwords of the character extension `C` (offsets `0x10/0x14`, `0xAA8/AAC`, `0xAB8/ABC`, `0xAC0/AC4`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct SpawnMaskState {
    pub mask: u64,
    pub aa8: u32,
    pub aac: u32,
    pub ab8: u32,
    pub abc: u32,
    pub ac0: u32,
    pub ac4: u32,
}

/// Port of `0x15F9BA..0x15FA9E`: only the fields the original writes change.
pub fn init_spawn_mask(state: &mut SpawnMaskState) {
    let (low, high) = (state.mask as u32, (state.mask >> 32) as u32);
    let removed = if low & DEFERRED_A != 0 {
        Some(low & 0xEF7F_FFFF)
    } else if low & DEFERRED_B != 0 {
        Some(low & 0xEFEF_FFFF)
    } else {
        None
    };
    let (mut low, mut high) = (low, high);
    if let Some(kept) = removed {
        state.ac0 = kept;
        state.ac4 = high;
        low &= !kept;
        high = 0;
    }
    if low & ACTIVATE_NOW != 0 {
        low &= !ACTIVATE_NOW;
        state.ab8 = 0;
        state.abc = 0;
        state.aac = high;
        state.aa8 = low;
    } else {
        state.ab8 = low;
        state.abc = high;
        state.aa8 = DORMANT_CLASS as u32;
        state.aac = 0;
        low = DORMANT_CLASS as u32;
        high = 0;
    }
    if low == 0 && high == 0 {
        low = 4;
    }
    state.mask = u64::from(low) | u64::from(high) << 32;
}

/// What a spawn record's mask turns into: the class the character starts in and the tick that runs it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpawnClass {
    pub initial_mask: u64,
    /// Parked record mask (`C[0xAB8]`), the class entered when the dormant class is left.
    pub parked_mask: u64,
    pub ticks: Option<&'static [u32]>,
}

/// Record mask (first qword of the `0x98` byte SETU record) to initial class and class tick (story mode A).
pub fn spawn_class(record_mask: u64) -> SpawnClass {
    let mut state = SpawnMaskState {
        mask: record_mask,
        ..SpawnMaskState::default()
    };
    init_spawn_mask(&mut state);
    SpawnClass {
        initial_mask: state.mask,
        parked_mask: u64::from(state.ab8) | u64::from(state.abc) << 32,
        ticks: dispatch_ticks(true, state.mask),
    }
}

/// The class mask word of the record at `index` (`None` when out of range). `records` is the `0x98` byte stride table.
pub fn record_mask(records: &[u8], index: usize) -> Option<u64> {
    let raw = records.get(index * 0x98..index * 0x98 + 8)?;
    Some(u64::from_le_bytes(raw.try_into().ok()?))
}

/// Mode-selected behaviour mask application, original 0x15C700.
/// Hooks may change the mode or requested mask, as original callees can.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct ClassTransition {
    pub mode: u8,
    pub story_mask: u64,
    pub other_mask: u64,
    pub requested: u64,
    pub previous: u64,
    pub transition_time: f32,
    pub clock: f32,
    pub auxiliary_time: f32,
    pub auxiliary_state: u32,
    pub flags: u32,
}

impl ClassTransition {
    pub fn active(&self) -> u64 {
        if self.mode == 0xA {
            self.story_mask
        } else {
            self.other_mask
        }
    }
}

pub trait ClassTransitionWorld {
    /// 0x158D40(object, requested_lo, requested_hi, old_lo, old_hi).
    fn leaving(&mut self, state: &mut ClassTransition, requested: u64, old: u64);
    /// 0x15BCC0(object) on entering a story dormant mask.
    fn dormant(&mut self, state: &mut ClassTransition);
    /// Append the character extension to 0x7A3580 and increment 0x74C37C.
    fn wake(&mut self, state: &mut ClassTransition);
    /// 0x27B8B0(object), called after every actual change.
    fn entered(&mut self, state: &mut ClassTransition);
}

/// Apply pending C+0xAA8/AAC to C+0x10/14 (story) or C+0x18/1C (other).
/// Equality returns without callbacks or timer writes. Mode and request are reread
/// after the leave hook, preserving the original ordering rather than caching them.
pub fn apply_class_transition(state: &mut ClassTransition, world: &mut impl ClassTransitionWorld) {
    let old = state.active();
    let requested = state.requested;
    if old == requested {
        return;
    }
    world.leaving(state, requested, old);
    state.previous = state.active();
    if state.mode == 0xA {
        state.story_mask = state.requested;
    } else {
        state.other_mask = state.requested;
    }
    state.transition_time = 0.0;
    state.clock = 0.0;
    state.auxiliary_time = 0.0;
    state.auxiliary_state = 0;
    if state.mode == 0xA {
        if state.story_mask & DORMANT_CLASS != 0 {
            state.flags |= 0x80;
            world.dormant(state);
            world.entered(state);
            return;
        }
        if state.previous == DORMANT_CLASS {
            world.wake(state);
        }
    }
    world.entered(state);
}
