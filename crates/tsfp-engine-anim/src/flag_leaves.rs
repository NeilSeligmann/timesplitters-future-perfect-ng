// SPDX-License-Identifier: GPL-3.0-or-later
//! T1836 wave 3 anim leaves, call-free ports of C replacements, docs/t1836-rust-wave3.md.
use crate::{gprs::Gprs, memory::GuestMemory};

const SLOT_TABLE_POINTER: u32 = 0x007e_3954;
const MASK_TABLE: u32 = 0x007d_e660;

/// 00059F10 (`state_offset` 0xD8) and 00059F50 (0x130) game_anim_obj_*_flag_mask_test.
/// cdecl(3): (object, mask_lo, mask_hi). The animation record at object+4 holds a slot index
/// at `state_offset`. A zero entry in the pointer table at [7E3954] gives 0, else 1 when the
/// 8-byte mask at 7DE660 + 8*slot intersects the 64-bit argument mask.
pub fn obj_flag_mask_test<M: GuestMemory>(m: &M, object: u32, mask_lo: u32, mask_hi: u32, state_offset: u32) -> Gprs {
    let record = m.read32(object.wrapping_add(4));
    let slot = m.read32(record.wrapping_add(state_offset));
    let table = m.read32(SLOT_TABLE_POINTER);
    if m.read32(table.wrapping_add(slot.wrapping_mul(4))) == 0 {
        return Gprs { eax: 0, ecx: slot, edx: table };
    }
    let low = m.read32(slot.wrapping_mul(8).wrapping_add(MASK_TABLE)) & mask_lo;
    let high = m.read32(slot.wrapping_mul(8).wrapping_add(MASK_TABLE + 4)) & mask_hi;
    Gprs { eax: u32::from((low | high) != 0), ecx: high, edx: mask_lo }
}
