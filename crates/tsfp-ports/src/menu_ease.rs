// SPDX-License-Identifier: GPL-3.0-or-later
//! 0x2E28A0 `game_ui_curved_menu_ease_cell_values_toward_targets` (T2042), hand port.
//!
//! The record holds 19 cells of `{ current: f32, target: i32 }` (8 bytes) interleaved with one spare dword per
//! cell (stride 0xc): two groups of eight cells (record + 0x00 and + 0x60, cell stride 0xc) and a tail of three
//! cells at + 0xc0. Every call moves each `current` towards `(float)target` by
//! `[0x4e7948] * [0x789648] * 0.1` (frame time times a speed global) and snaps to the target when the step
//! would pass it. The function returns 1 when every cell already sat on its target and 0 otherwise.
//!
//! Verification tier: 1 (T2069, weak, FABRICATED inputs): observable-only differential against the lifted original
//! from a fresh relift (`C2R_OBSERVABLE=1`, docs/t2069-live-frame-roots.md) plus the spec unit tests below.
use crate::abi::ExtRegisters;
use crate::memory::GuestMemory;
use crate::views::generated_views::TsfpMenuEaseCellView;

const FRAME_TIME: u32 = 0x004e_7948;
const SPEED: u32 = 0x0078_9648;
const GROUP_BASE: [u32; 2] = [0x00, 0x60];
const GROUP_CELLS: u32 = 8;
const TAIL_BASE: u32 = 0xc0;
const TAIL_CELLS: u32 = 3;
const CELL_STRIDE: u32 = TsfpMenuEaseCellView::SIZE;

/// Live cell boundary. Named offsets come from the generated layout, while
/// dword operations preserve the original GuestMemory access contract. The
/// generated scalar getters use byte operations and are intentionally not used
/// here: memory adapters may observe widths and accesses. No snapshot is cached.
#[derive(Clone, Copy, Debug)]
struct EaseCellView(u32);

impl EaseCellView {
    fn current<M: GuestMemory>(self, m: &M) -> f32 {
        f32::from_bits(m.read32(self.0.wrapping_add(TsfpMenuEaseCellView::OFFSET_CURRENT)))
    }

    fn target<M: GuestMemory>(self, m: &M) -> i32 {
        m.read32(self.0.wrapping_add(TsfpMenuEaseCellView::OFFSET_TARGET)) as i32
    }

    fn set_current<M: GuestMemory>(self, m: &mut M, value: f32) {
        m.write32(
            self.0.wrapping_add(TsfpMenuEaseCellView::OFFSET_CURRENT),
            value.to_bits(),
        );
    }
}
/// Read-only float constant read through memory like the original (0.1 per the earlier decompile, INFERRED, not read from the retail image here).
const STEP_SCALE: u32 = 0x0047_5d24;

/// One ease step towards `target`: `Some(new value)` when the cell moves, `None` when it already sits on the
/// target. Mirrors the machine code (`comiss` + `jbe`, which also takes the branch on unordered operands): a NaN
/// `current` leaves the cell alone (no store, still counted as settled), and a NaN stepped value is kept, not
/// snapped (T2069: the differential against the lifted original, 20000 seeds, found the earlier "NaN ends on the
/// target" reading wrong).
fn ease(current: f32, target: f32, step: f32) -> Option<f32> {
    if current > target {
        let next = current - step;
        Some(if next.is_nan() || target <= next {
            next
        } else {
            target
        })
    } else if target > current {
        let next = current + step;
        Some(if next.is_nan() || next <= target {
            next
        } else {
            target
        })
    } else {
        None // equal, or either operand is NaN
    }
}

/// Hybrid hook for `c2r::run`: cdecl(record) at `esp + 4`, EAX = 1 when every cell was already settled.
pub fn try_run<M: GuestMemory>(va: u32, m: &mut M, r: &mut ExtRegisters) -> bool {
    if va != 0x002e_28a0 {
        return false;
    }
    let record = m.read32(r.esp.wrapping_add(4));
    r.eax = u32::from(ease_cells(m, record));
    true
}

/// Advance every cell of the record at `record`; true when all of them were already at their targets.
pub fn ease_cells<M: GuestMemory>(m: &mut M, record: u32) -> bool {
    let cells = GROUP_BASE
        .iter()
        .flat_map(|&base| (0..GROUP_CELLS).map(move |i| base + i * CELL_STRIDE))
        .chain((0..TAIL_CELLS).map(|i| TAIL_BASE + i * CELL_STRIDE));
    let mut settled = true;
    for offset in cells {
        let cell = EaseCellView(record.wrapping_add(offset));
        let step = f32::from_bits(m.read32(FRAME_TIME))
            * f32::from_bits(m.read32(SPEED))
            * f32::from_bits(m.read32(STEP_SCALE));
        let target = cell.target(m) as f32;
        let current = cell.current(m);
        if let Some(value) = ease(current, target, step) {
            cell.set_current(m, value);
            settled = false;
        }
    }
    settled
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use std::collections::BTreeMap;

    #[derive(Default)]
    struct Memory(BTreeMap<u32, u8>);
    impl GuestMemory for Memory {
        fn read8(&self, a: u32) -> u8 {
            *self.0.get(&a).unwrap_or(&0)
        }
        fn write8(&mut self, a: u32, v: u8) {
            self.0.insert(a, v);
        }
    }

    const RECORD: u32 = 0x5000;

    fn setup(step_time: f32, speed: f32) -> Memory {
        let mut m = Memory::default();
        m.write32(FRAME_TIME, step_time.to_bits());
        m.write32(SPEED, speed.to_bits());
        m.write32(STEP_SCALE, 0.1f32.to_bits());
        m
    }
    fn cell_offsets() -> Vec<u32> {
        (0..8)
            .map(|i| i * 0xc)
            .chain((0..8).map(|i| 0x60 + i * 0xc))
            .chain((0..3).map(|i| 0xc0 + i * 0xc))
            .collect()
    }
    fn current(m: &Memory, offset: u32) -> f32 {
        f32::from_bits(m.read32(RECORD + offset))
    }

    #[test]
    fn live_views_preserve_dword_order_wrap_and_spare_words() {
        use std::cell::RefCell;
        struct Observed {
            words: BTreeMap<u32, u32>,
            accesses: RefCell<Vec<(bool, u32)>>,
        }
        impl GuestMemory for Observed {
            fn read8(&self, _: u32) -> u8 {
                panic!("unexpected byte read")
            }
            fn write8(&mut self, _: u32, _: u8) {
                panic!("unexpected byte write")
            }
            fn read32(&self, a: u32) -> u32 {
                self.accesses.borrow_mut().push((false, a));
                *self.words.get(&a).expect("unexpected read")
            }
            fn write32(&mut self, a: u32, v: u32) {
                self.accesses.borrow_mut().push((true, a));
                self.words.insert(a, v);
            }
        }
        for base in [RECORD, 0xffff_fffcu32] {
            let mut m = Observed {
                words: BTreeMap::new(),
                accesses: RefCell::new(Vec::new()),
            };
            for (a, v) in [(FRAME_TIME, 1.0f32), (SPEED, 1.0), (STEP_SCALE, 1.0)] {
                m.words.insert(a, v.to_bits());
            }
            let mut expected = Vec::new();
            for (i, offset) in cell_offsets().into_iter().enumerate() {
                let a = base.wrapping_add(offset);
                // Include unsettled, settled and NaN cells. Only unsettled cells store.
                let current = [0.0f32, 2.0, f32::from_bits(0x7fc01234)][i % 3];
                m.words.insert(a, current.to_bits());
                m.words.insert(a.wrapping_add(4), 2);
                m.words.insert(a.wrapping_add(8), 0xa5a5a5a5);
                expected.extend([
                    (false, FRAME_TIME),
                    (false, SPEED),
                    (false, STEP_SCALE),
                    (false, a.wrapping_add(4)),
                    (false, a),
                ]);
                if i % 3 == 0 {
                    expected.push((true, a));
                }
            }
            assert!(!ease_cells(&mut m, base));
            assert_eq!(*m.accesses.borrow(), expected);
            for (i, offset) in cell_offsets().into_iter().enumerate() {
                let a = base.wrapping_add(offset);
                let expected_bits = [1.0f32.to_bits(), 2.0f32.to_bits(), 0x7fc01234][i % 3];
                assert_eq!(m.words[&a], expected_bits);
                assert_eq!(m.words[&a.wrapping_add(4)], 2);
                assert_eq!(m.words[&a.wrapping_add(8)], 0xa5a5a5a5);
            }
        }
    }

    #[test]
    fn settled_record_reports_true_and_writes_nothing() {
        let mut m = setup(1.0, 1.0);
        for o in cell_offsets() {
            m.write32(RECORD + o, 5.0f32.to_bits());
            m.write32(RECORD + o + 4, 5);
        }
        assert!(ease_cells(&mut m, RECORD));
    }

    #[test]
    fn cells_step_by_a_tenth_of_frame_time_times_speed_then_snap() {
        let mut m = setup(2.0, 5.0); // step 1.0
        for o in cell_offsets() {
            m.write32(RECORD + o, 0.0f32.to_bits());
            m.write32(RECORD + o + 4, 2);
        }
        m.write32(RECORD + 0xc, 3.0f32.to_bits()); // above its target: moves down
        assert!(!ease_cells(&mut m, RECORD));
        assert_eq!(current(&m, 0), 1.0);
        assert_eq!(current(&m, 0xc), 2.0);
        assert!(!ease_cells(&mut m, RECORD));
        assert_eq!(current(&m, 0), 2.0);
        assert!(ease_cells(&mut m, RECORD)); // every cell sits on 2.0 now
    }

    #[test]
    fn overshoot_snaps_to_target_both_directions() {
        assert_eq!(ease(0.0, 0.25, 1.0), Some(0.25));
        assert_eq!(ease(1.0, -0.25, 3.0), Some(-0.25));
        assert_eq!(ease(2.0, 2.0, 1.0), None);
        assert_eq!(ease(f32::NAN, 4.0, 1.0), None);
        assert!(ease(1.0, 4.0, f32::NAN).unwrap().is_nan());
    }

    #[test]
    fn target_is_a_signed_integer() {
        let mut m = setup(10.0, 1.0); // step 1.0
        for o in cell_offsets() {
            m.write32(RECORD + o, 0.0f32.to_bits());
            m.write32(RECORD + o + 4, (-3i32) as u32);
        }
        ease_cells(&mut m, RECORD);
        assert_eq!(current(&m, 0xc0 + 0xc), -1.0);
    }
}
