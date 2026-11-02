//! T1877: which animation a character plays. See `docs/t-character-anim-selection.md`.
//!
//! The game keeps ONE global animation id namespace (`0x7E03C0`, id to clip slot, see `tsfp_format_anim::propset`) and every
//! character class tick requests ids it hard codes (`0x5F460(object, id, blend, speed)`); the clip an id resolves to is
//! whatever the level's anim sets loaded last for that id. This module ports the pure selection pieces:
//!
//! * [`Lcg64`], the `0x153B40` generator the selectors draw from;
//! * [`playable_id`], the id gate of the play routine `0x5F460`;
//! * [`select_armed_pose`], the armed actor pose selector `0x1C91C0` (bit exact against the original under Unicorn);
//! * [`CLASS_TICKS`] and [`class_tick_for_mask`], the class mask to tick map of the dispatchers `0x27D590` / `0x27F8E0`;
//! * [`TICK_IDS`], the ids each class tick requests (static immediates of the 634 `0x5F460` call sites).

pub use crate::char_anim_tables::{CLASS_TICKS_MODE_A, CLASS_TICKS_OTHER, TICK_IDS};

// non story-mode-A high classes, dispatcher 0x27DF60 (mask at C+0x18), tools/t1877_class_map.py
pub const CLASS_TICKS_OTHER_HIGH: &[(u64, &[u32])] = &[
    (0x2000, &[0x2b5b90]),
    (0x4000, &[0x2b6170]),
    (0x8000, &[0x2b1e90]),
    (0x10000, &[0x2b2050]),
    (0x20000, &[0x2b66a0, 0x1594e0]),
    (0x40000, &[0x2b55d0, 0x1594e0]),
    (0x100000, &[0x273a20]),
    (0x200000, &[0x272800]),
    (0x400000, &[0x2b1120, 0x1594e0]),
    (0x800000, &[0x2b11b0, 0x1594e0]),
    (0x1000000, &[0x2b2750]),
    (0x2000000, &[0x274ba0]),
    (0x4000000, &[0x275050]),
    (0x8000000, &[0x274690]),
    (0x10000000, &[0x2b68b0]),
    (0x4000000000000000, &[0x2b4060]),
];

/// Ids the play routine `0x5F460` accepts (`0x25 <= id < 0xBFE`, plus a loaded clip in the slot table).
pub const PLAY_ID_MIN: u32 = 0x25;
pub const PLAY_ID_END: u32 = 0xBFE;

/// `0x5F460` plays only ids inside the range (the clip must also be resident, which the caller checks).
pub fn playable_id(id: i32) -> bool {
    id >= PLAY_ID_MIN as i32 && id < PLAY_ID_END as i32
}

/// `0x153B40` (`game_rng_advance_lcg64`): `state = state * 0x343FD + 0x269EC3` (64 bit), returns the low dword of the new state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lcg64 {
    pub state: u64,
}

impl Lcg64 {
    pub fn next_u32(&mut self) -> u32 {
        self.state = self.state.wrapping_mul(0x343FD).wrapping_add(0x269EC3);
        self.state as u32
    }
}

/// The selectors turn the dword into a fraction with `fild` (+2^32 when negative) and `fmul 2^-32`: exactly `raw / 2^32`.
pub fn unit_fraction(raw: u32) -> f64 {
    f64::from(raw) / 4_294_967_296.0
}

/// Inputs of `0x1C91C0` (state record `S`, object `O = S[0]`, extension `C = O[0x7C]`).
#[derive(Clone, Copy, Debug, Default)]
pub struct ArmedPoseInputs {
    /// `S[0xA0]` before the call (the previous pose id: the selector is a hysteresis).
    pub previous_id: u32,
    /// `C[0x80]`.
    pub c80: u32,
    /// `C[0x94]` (signed compares: `< 0xE`).
    pub c94: u32,
    /// The 64 bit class mask `C[0x10]` / `C[0x14]` (tested against `0x60000` by `0x17510`).
    pub class_mask: u64,
    /// `C[0x2BC]` and `C[0x18C]`.
    pub c2bc: u32,
    pub c18c: u32,
    /// `[0x4F9BAC + C[0x204] * 0x29C + 0xC]`: bit 0 and bit 8 steer the branches (roles UNRESOLVED).
    pub weapon_flags: u32,
    /// `P[0x54]` of the record `P = C[0xBF0]`, `None` when the record pointer is null.
    pub record_flags: Option<u32>,
    /// The dword `[C[0] + 0x150]` read on the one handed branch.
    pub object_150: u32,
}

/// `S[0x9C]` / `S[0xA0]` as written through `0x58C50`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PoseChoice {
    pub mask: u32,
    pub id: u32,
}

const MASK_ALL: u32 = 0x1FF8;
const MASK_EXPLICIT: u32 = 0x1F8;

/// Port of `0x1C91C0`. `rng` is drawn only on the two hysteresis branches, exactly like the original.
pub fn select_armed_pose(inputs: &ArmedPoseInputs, rng: &mut Lcg64) -> PoseChoice {
    let pick = |mask, id| PoseChoice { mask, id };
    if inputs.c80 != 0 && inputs.c94 != 0 {
        return pick(MASK_ALL, 0x6E);
    }
    if matches!(inputs.c94, 0x38 | 0x2B | 0x2C | 0x2E | 0x2F | 0x28) {
        return pick(MASK_ALL, 0x71);
    }
    if inputs.class_mask & 0x60000 != 0 {
        return pick(MASK_EXPLICIT, 0x6B);
    }
    if inputs.c2bc != 0 && inputs.c18c != 0 {
        return pick(
            MASK_ALL,
            if (inputs.c94 as i32) < 0xE {
                0x6E
            } else {
                0x6D
            },
        );
    }
    let record_bit = inputs.record_flags.is_some_and(|flags| flags & 0x800 != 0);
    if inputs.weapon_flags & 1 != 0 {
        if inputs.weapon_flags & 0x100 != 0 {
            return pick(MASK_ALL, 0x6B);
        }
        if record_bit || inputs.previous_id == 0x69 {
            return pick(MASK_ALL, 0x69);
        }
        let fraction = unit_fraction(rng.next_u32());
        if fraction > f64::from(0.7f32) && inputs.previous_id != 0x6B {
            return pick(MASK_ALL, 0x69);
        }
        return pick(MASK_ALL, if inputs.object_150 == 0 { 0x6B } else { 0x69 });
    }
    if inputs.weapon_flags & 0x100 != 0 {
        return pick(MASK_ALL, 0x6C);
    }
    if record_bit || inputs.previous_id == 0x6A {
        return pick(MASK_ALL, 0x6A);
    }
    let fraction = unit_fraction(rng.next_u32());
    if fraction <= 0.5 || inputs.previous_id == 0x6C {
        pick(MASK_ALL, 0x6C)
    } else {
        pick(MASK_ALL, 0x6A)
    }
}

/// Callees (the class ticks) the dispatcher picks for an exact class mask: `0x27D590` in story mode A (mask at `C[0x10]`),
/// `0x27F8E0` otherwise (mask at `C[0x18]`). Only single bit masks select a tick, anything else falls to the default arm.
pub fn class_ticks(mode_a: bool, mask: u64) -> Option<&'static [u32]> {
    let table = if mode_a {
        CLASS_TICKS_MODE_A
    } else {
        CLASS_TICKS_OTHER
    };
    table
        .iter()
        .find(|(bit, _)| *bit == mask)
        .map(|(_, ticks)| *ticks)
}

/// The ticks the top level class dispatcher `0x27FE30` runs for the active mask (`C[0x10]` in story mode A, `C[0x18]`
/// otherwise): a mask with a bit in `0x1FFF` goes to the low dispatcher `0x27F8E0` (all modes), else mode A uses
/// `0x27D590` and every other mode `0x27DF60`. `None` is the default arm.
pub fn dispatch_ticks(mode_a: bool, mask: u64) -> Option<&'static [u32]> {
    let table = if mask & 0x1FFF != 0 {
        CLASS_TICKS_OTHER
    } else if mode_a {
        CLASS_TICKS_MODE_A
    } else {
        CLASS_TICKS_OTHER_HIGH
    };
    table
        .iter()
        .find(|(bit, _)| *bit == mask)
        .map(|(_, ticks)| *ticks)
}

/// The animation ids a tick function requests with immediate arguments (`None` when it never calls `0x5F460` directly).
pub fn tick_ids(tick: u32) -> Option<&'static [u16]> {
    TICK_IDS
        .iter()
        .find(|(owner, _, _)| *owner == tick)
        .map(|(_, ids, _)| *ids)
}

/// Every id the class selected by `mask` can request directly (union over its callees, sorted, deduplicated).
pub fn class_ids(mode_a: bool, mask: u64) -> Vec<u16> {
    let mut ids: Vec<u16> = class_ticks(mode_a, mask)
        .into_iter()
        .flatten()
        .filter_map(|tick| tick_ids(*tick))
        .flatten()
        .copied()
        .collect();
    ids.sort_unstable();
    ids.dedup();
    ids
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lcg_matches_the_original_constants() {
        let mut rng = Lcg64 { state: 0 };
        assert_eq!(rng.next_u32(), 0x0026_9EC3);
        assert_eq!(
            rng.next_u32(),
            (0x0026_9EC3u64 * 0x343FD + 0x0026_9EC3) as u32
        );
    }

    #[test]
    fn class_dispatch_tables_are_consistent() {
        // 0x2000 is the "jbcoweridle" class of 0x2784D0, 0x4000000000 the 0x2806B0 amble class (MEASURED by the oracle)
        assert_eq!(class_ticks(true, 0x2000), Some(&[0x2784d0u32][..]));
        assert_eq!(class_ticks(true, 0x4000000000), Some(&[0x2806b0u32][..]));
        assert_eq!(class_ticks(true, 0x3), None);
        assert!(class_ids(true, 0x4000000000).contains(&275));
        assert!(CLASS_TICKS_MODE_A.len() >= 40 && CLASS_TICKS_OTHER.len() >= 10);
        assert!(
            TICK_IDS
                .iter()
                .map(|(_, ids, dynamic)| ids.len() + usize::from(*dynamic))
                .sum::<usize>()
                > 400
        );
        assert!(TICK_IDS.windows(2).all(|pair| pair[0].0 < pair[1].0));
    }

    #[test]
    fn id_gate() {
        assert!(
            !playable_id(0x24)
                && playable_id(0x25)
                && playable_id(0xBFD)
                && !playable_id(0xBFE)
                && !playable_id(-1)
        );
    }

    #[test]
    fn two_handed_walks_use_the_hysteresis() {
        let mut rng = Lcg64 { state: 1 };
        let inputs = ArmedPoseInputs {
            weapon_flags: 0x101,
            ..Default::default()
        };
        assert_eq!(
            select_armed_pose(&inputs, &mut rng),
            PoseChoice {
                mask: 0x1FF8,
                id: 0x6B
            }
        );
        assert_eq!(rng.state, 1, "no draw on the fixed branch");
    }
}
