// SPDX-License-Identifier: GPL-3.0-or-later
// Sparse recovered fields only; these are prefixes, not allocation-size claims.
// docs/t1743-character-fields.md and docs/t-combat-damage-model.md.
#[repr(C)]
pub struct ObjectPrefix {
    pub reserved_00: [u8; 0x0c],
    pub flags_0c: u32,
    pub reserved_10: [u8; 0x18],
    pub mode_flags_28: u32,
    pub reserved_2c: [u8; 0x38],
    pub position_64: [f32; 3],
    pub reserved_70: [u8; 0x0c],
    pub extension_7c: u32,
    pub reserved_80: [u8; 0x14],
    pub model_id_94: u16,
    pub reserved_96: [u8; 0x136],
    pub health_1cc: f32,
    pub reserved_1d0: [u8; 8],
    pub max_health_1d8: f32,
}
#[repr(C)]
pub struct CharacterTransitionPrefix {
    pub reserved_00: [u8; 0x20],
    pub state_20: u32,
    pub reserved_24: [u8; 0x9ac],
    pub pending_state_9d0: i32,
    pub previous_state_9d4: u32,
    pub transition_time_9d8: f32,
}
const _: () = {
    assert!(core::mem::offset_of!(ObjectPrefix, extension_7c) == 0x7c);
    assert!(core::mem::offset_of!(ObjectPrefix, health_1cc) == 0x1cc);
    assert!(core::mem::offset_of!(ObjectPrefix, max_health_1d8) == 0x1d8);
    assert!(core::mem::offset_of!(CharacterTransitionPrefix, pending_state_9d0) == 0x9d0);
};
