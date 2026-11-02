// SPDX-License-Identifier: GPL-3.0-or-later
//! T1907 shared constructors; Tier 0 translations with guest record boundaries.
use tsfp_engine_object::closure::{
    callee::{Cpu, stack_arg},
    memory::GuestMemory,
};
/// 00227100 owns three adjacent eleven-dword arrays (0x84 bytes).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlotArrays {
    pub values: [u32; 11],
    pub pending: [u32; 11],
    pub completed: [u32; 11],
}
impl SlotArrays {
    pub fn new(value: u32) -> Self {
        Self {
            values: [value; 11],
            pending: [0; 11],
            completed: [0; 11],
        }
    }
    pub fn write<M: GuestMemory>(&self, m: &mut M, base: u32) {
        for (i, value) in self
            .values
            .iter()
            .chain(self.pending.iter())
            .chain(self.completed.iter())
            .enumerate()
        {
            m.write32(base.wrapping_add(i as u32 * 4), *value);
        }
    }
}
/// 00227100 cdecl(record,value): exactly 33 dwords, including +0x7C and +0x80. Tier 0.
// portreg: va=0x00227100 tier=0
pub fn init_slot_arrays<M: GuestMemory>(m: &mut M, cpu: &Cpu) {
    SlotArrays::new(stack_arg(m, cpu, 1)).write(m, stack_arg(m, cpu, 0));
}
/// 00032550 builds ten key/value pointer pairs for network/live; no string copies. Tier 0.
pub fn init_flag_addresses<M: GuestMemory>(m: &mut M, cpu: &mut Cpu) {
    let base = stack_arg(m, cpu, 0);
    let flags = stack_arg(m, cpu, 1);
    let keys = [
        0x0047_6890,
        0x0047_6898,
        0x0047_689C,
        0x0047_68A8,
        0x0047_68B0,
        0x0047_68B8,
        0x0047_68C0,
        0x0047_68CC,
        0x0047_68D4,
    ];
    let bits = [1, 2, 4, 8, 0x10, 0x20, 0x40, 0x200, 0x100];
    for (i, (&key, &bit)) in keys.iter().zip(bits.iter()).enumerate() {
        m.write32(base.wrapping_add(i as u32 * 8), key);
        m.write32(
            base.wrapping_add(i as u32 * 8 + 4),
            if flags & bit != 0 {
                0x0047_68E8
            } else {
                0x0047_68EC
            },
        );
    }
    m.write32(base.wrapping_add(0x48), 0x0047_68DC);
    m.write32(base.wrapping_add(0x4C), 0x0047_68F8);
    cpu.eax = base;
}
/// The render seam stores three thresholds, three interleaved masks and three zero words.
/// Each power-of-two level below a threshold consumes the next shared mask bit,
/// visiting channels in array order. Thresholds of zero or one consume no bits.
///
/// ```
/// use tsfp_engine_core::initializers::ThresholdMasks;
/// let value = ThresholdMasks::new([4, 2, 1]);
/// assert_eq!(value.masks, [0b101, 0b010, 0]);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ThresholdMasks {
    pub thresholds: [u32; 3],
    pub masks: [u32; 3],
}
impl ThresholdMasks {
    pub fn new(thresholds: [u32; 3]) -> Self {
        let mut result = Self {
            thresholds,
            masks: [0; 3],
        };
        let mut level = 1u32;
        let mut bit = 1u32;
        loop {
            let mut residue = 0;
            for i in 0..3 {
                if level < thresholds[i] {
                    result.masks[i] |= bit;
                    bit = bit.wrapping_shl(1);
                    residue = bit;
                }
            }
            level = level.wrapping_shl(1);
            if residue == 0 {
                break;
            }
        }
        result
    }
}
/// 0002A200 thiscall(record,a,b,c): strict unsigned threshold tests; six trailing dwords. Tier 0.
// portreg: va=0x0002a200 tier=0
pub fn init_threshold_masks<M: GuestMemory>(m: &mut M, cpu: &Cpu) {
    let result = ThresholdMasks::new([
        stack_arg(m, cpu, 0),
        stack_arg(m, cpu, 1),
        stack_arg(m, cpu, 2),
    ]);
    for (i, value) in result
        .thresholds
        .into_iter()
        .chain(result.masks)
        .chain([0; 3])
        .enumerate()
    {
        m.write32(cpu.ecx.wrapping_add(i as u32 * 4), value);
    }
}
