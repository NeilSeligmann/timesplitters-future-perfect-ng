//! Recovered 32-bit guest layouts, not host-owned game state.
//! Offsets observed; semantic field names INFERRED (T1807).

pub mod generated_layouts;

/// Absolute little-endian guest virtual address; never a host pointer.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GuestAddr(pub u32);

impl GuestAddr {
    /// Add a byte offset with the guest CPU's 32-bit address wrap.
    ///
    /// This only computes an address; it does not validate a guest memory range.
    ///
    /// ```
    /// use tsfp_types::GuestAddr;
    /// assert_eq!(GuestAddr(0xffff_fffc).wrapping_add(8), GuestAddr(4));
    /// ```
    pub const fn wrapping_add(self, bytes: u32) -> Self {
        Self(self.0.wrapping_add(bytes))
    }

    /// Return `None` if adding a byte offset crosses the 32-bit address limit.
    ///
    /// ```
    /// use tsfp_types::GuestAddr;
    /// assert_eq!(GuestAddr(0xffff_fffc).checked_add(8), None);
    /// assert_eq!(GuestAddr(0x1000).checked_add(4), Some(GuestAddr(0x1004)));
    /// ```
    pub const fn checked_add(self, bytes: u32) -> Option<Self> {
        match self.0.checked_add(bytes) {
            Some(address) => Some(Self(address)),
            None => None,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct TsfpVec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct TsfpObject {
    pub unknown_000: [u8; 0x20],
    pub class_id: u32,
    pub unknown_024: [u8; 0x40],
    pub position: TsfpVec3,
    pub yaw: f32,
    pub unknown_074: [u8; 8],
    pub character: GuestAddr,
    pub unknown_080: [u8; 0xd0],
    pub pending_effect: u32,
    pub unknown_154: [u8; 0x78],
    pub health: f32,
    pub unknown_1d0: [u8; 8],
    pub max_health: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct TsfpCharacter {
    pub object: GuestAddr,
    pub melee_target: GuestAddr,
    pub unknown_008: [u8; 0x70],
    pub armor: f32,
    pub unknown_07c: [u8; 0xac8],
    pub seconds_since_damage: f32,
    pub last_health_damage: f32,
    pub unknown_b4c: [u8; 0x1c],
    pub melee_stamp: u32,
    pub melee_hit_count: u16,
    pub unknown_b6e: [u8; 0x332],
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct TsfpPlayerPrefix {
    pub unknown_000: [u8; 0xa8],
    pub pose: TsfpVec3,
    pub unknown_0b4: [u8; 0xc],
    pub facing: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct TsfpDamage {
    pub victim: GuestAddr,
    pub attacker: u32,
    pub source: u32,
    pub unknown_00c: [u8; 0xc],
    pub weapon_id: u32,
    pub unknown_01c: [u8; 4],
    pub damage: f32,
    pub damage_type: u32,
    pub unknown_028: [u8; 0x14],
    pub flags: u32,
    pub flags2: u32,
    pub unknown_044: [u8; 4],
    pub reaction_id: u32,
    pub reaction_a: f32,
    pub reaction_b: f32,
    pub extra_id: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct TsfpScriptSetup {
    pub list_kind: u32,
    pub key: u32,
    pub subtype: u32,
    pub setup_index: i16,
    pub unknown_00e: [u8; 0x96],
    pub rule_index: u32,
    pub unknown_0a8: [u8; 4],
    pub identifier: u32,
    pub flags: u8,
    pub unknown_0b1: [u8; 3],
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct TsfpScriptRule {
    pub unknown_000: [u8; 4],
    pub direct_action_enabled: u8,
    pub unknown_005: [u8; 3],
    pub group_threshold: u32,
    pub unknown_00c: [u8; 0x44],
    pub condition_count: u32,
    pub condition_indices: GuestAddr,
    pub unknown_058: [u8; 0x30],
    pub action_count: u32,
    pub action_indices: GuestAddr,
    pub unknown_090: [u8; 0x18c],
    pub event_selector: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct TsfpVoiceDefinition {
    pub unknown_000: [u8; 8],
    pub kind: u32,
    pub group: u32,
    pub label: u32,
    pub object_ref: u32,
    pub fallback_ref: u32,
    pub unknown_01c: [u8; 0x94],
    pub controls: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct TsfpVoiceInstance {
    pub definition: GuestAddr,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct TsfpVoiceBank {
    pub samples: GuestAddr,
    pub count: i32,
    pub label_base: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct TsfpSubtitle {
    pub start: f32,
    pub duration: f32,
    pub unknown_008: [u8; 8],
    pub label: u32,
    pub flags: u8,
    pub unknown_015: [u8; 3],
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct TsfpModePrefix {
    pub unknown_000: [u8; 0x12c],
    pub setup_count: u32,
    pub setups: GuestAddr,
    pub rule_count: u32,
    pub rules: GuestAddr,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct TsfpScriptEntry {
    pub own_setup_index: i32,
    pub key: u32,
    pub resolved_index: i32,
    pub flags: u8,
    pub unknown_00d: [u8; 3],
    pub callback: u32,
    pub payload: u32,
    pub state: u32,
    pub unknown_01c: [u8; 4],
    pub auxiliary: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::mem::{align_of, offset_of, size_of};

    #[test]
    fn guest_address_arithmetic_boundaries() {
        for base in [0, 1, 0x7fff_ffff, 0xffff_ff00, u32::MAX] {
            for bytes in 0..=512u32 {
                let address = GuestAddr(base);
                assert_eq!(address.wrapping_add(bytes).0, base.wrapping_add(bytes));
                assert_eq!(
                    address.checked_add(bytes).map(|a| a.0),
                    base.checked_add(bytes)
                );
            }
        }
    }

    #[test]
    fn recovered_layouts() {
        assert_eq!(offset_of!(TsfpVec3, x), 0x0);
        assert_eq!(offset_of!(TsfpVec3, y), 0x4);
        assert_eq!(offset_of!(TsfpVec3, z), 0x8);
        assert_eq!(size_of::<TsfpVec3>(), 0xc);
        assert_eq!(align_of::<TsfpVec3>(), 4);
        assert_eq!(offset_of!(TsfpObject, unknown_000), 0x0);
        assert_eq!(offset_of!(TsfpObject, class_id), 0x20);
        assert_eq!(offset_of!(TsfpObject, unknown_024), 0x24);
        assert_eq!(offset_of!(TsfpObject, position), 0x64);
        assert_eq!(offset_of!(TsfpObject, yaw), 0x70);
        assert_eq!(offset_of!(TsfpObject, unknown_074), 0x74);
        assert_eq!(offset_of!(TsfpObject, character), 0x7c);
        assert_eq!(offset_of!(TsfpObject, unknown_080), 0x80);
        assert_eq!(offset_of!(TsfpObject, pending_effect), 0x150);
        assert_eq!(offset_of!(TsfpObject, unknown_154), 0x154);
        assert_eq!(offset_of!(TsfpObject, health), 0x1cc);
        assert_eq!(offset_of!(TsfpObject, unknown_1d0), 0x1d0);
        assert_eq!(offset_of!(TsfpObject, max_health), 0x1d8);
        assert_eq!(size_of::<TsfpObject>(), 0x1dc);
        assert_eq!(align_of::<TsfpObject>(), 4);
        assert_eq!(offset_of!(TsfpCharacter, object), 0x0);
        assert_eq!(offset_of!(TsfpCharacter, melee_target), 0x4);
        assert_eq!(offset_of!(TsfpCharacter, unknown_008), 0x8);
        assert_eq!(offset_of!(TsfpCharacter, armor), 0x78);
        assert_eq!(offset_of!(TsfpCharacter, unknown_07c), 0x7c);
        assert_eq!(offset_of!(TsfpCharacter, seconds_since_damage), 0xb44);
        assert_eq!(offset_of!(TsfpCharacter, last_health_damage), 0xb48);
        assert_eq!(offset_of!(TsfpCharacter, unknown_b4c), 0xb4c);
        assert_eq!(offset_of!(TsfpCharacter, melee_stamp), 0xb68);
        assert_eq!(offset_of!(TsfpCharacter, melee_hit_count), 0xb6c);
        assert_eq!(offset_of!(TsfpCharacter, unknown_b6e), 0xb6e);
        assert_eq!(size_of::<TsfpCharacter>(), 0xea0);
        assert_eq!(align_of::<TsfpCharacter>(), 4);
        assert_eq!(offset_of!(TsfpPlayerPrefix, unknown_000), 0x0);
        assert_eq!(offset_of!(TsfpPlayerPrefix, pose), 0xa8);
        assert_eq!(offset_of!(TsfpPlayerPrefix, unknown_0b4), 0xb4);
        assert_eq!(offset_of!(TsfpPlayerPrefix, facing), 0xc0);
        assert_eq!(size_of::<TsfpPlayerPrefix>(), 0xc4);
        assert_eq!(align_of::<TsfpPlayerPrefix>(), 4);
        assert_eq!(offset_of!(TsfpDamage, victim), 0x0);
        assert_eq!(offset_of!(TsfpDamage, attacker), 0x4);
        assert_eq!(offset_of!(TsfpDamage, source), 0x8);
        assert_eq!(offset_of!(TsfpDamage, unknown_00c), 0xc);
        assert_eq!(offset_of!(TsfpDamage, weapon_id), 0x18);
        assert_eq!(offset_of!(TsfpDamage, unknown_01c), 0x1c);
        assert_eq!(offset_of!(TsfpDamage, damage), 0x20);
        assert_eq!(offset_of!(TsfpDamage, damage_type), 0x24);
        assert_eq!(offset_of!(TsfpDamage, unknown_028), 0x28);
        assert_eq!(offset_of!(TsfpDamage, flags), 0x3c);
        assert_eq!(offset_of!(TsfpDamage, flags2), 0x40);
        assert_eq!(offset_of!(TsfpDamage, unknown_044), 0x44);
        assert_eq!(offset_of!(TsfpDamage, reaction_id), 0x48);
        assert_eq!(offset_of!(TsfpDamage, reaction_a), 0x4c);
        assert_eq!(offset_of!(TsfpDamage, reaction_b), 0x50);
        assert_eq!(offset_of!(TsfpDamage, extra_id), 0x54);
        assert_eq!(size_of::<TsfpDamage>(), 0x58);
        assert_eq!(align_of::<TsfpDamage>(), 4);
        assert_eq!(offset_of!(TsfpScriptSetup, list_kind), 0x0);
        assert_eq!(offset_of!(TsfpScriptSetup, key), 0x4);
        assert_eq!(offset_of!(TsfpScriptSetup, subtype), 0x8);
        assert_eq!(offset_of!(TsfpScriptSetup, setup_index), 0xc);
        assert_eq!(offset_of!(TsfpScriptSetup, unknown_00e), 0xe);
        assert_eq!(offset_of!(TsfpScriptSetup, rule_index), 0xa4);
        assert_eq!(offset_of!(TsfpScriptSetup, unknown_0a8), 0xa8);
        assert_eq!(offset_of!(TsfpScriptSetup, identifier), 0xac);
        assert_eq!(offset_of!(TsfpScriptSetup, flags), 0xb0);
        assert_eq!(offset_of!(TsfpScriptSetup, unknown_0b1), 0xb1);
        assert_eq!(size_of::<TsfpScriptSetup>(), 0xb4);
        assert_eq!(align_of::<TsfpScriptSetup>(), 4);
        assert_eq!(offset_of!(TsfpScriptRule, unknown_000), 0x0);
        assert_eq!(offset_of!(TsfpScriptRule, direct_action_enabled), 0x4);
        assert_eq!(offset_of!(TsfpScriptRule, unknown_005), 0x5);
        assert_eq!(offset_of!(TsfpScriptRule, group_threshold), 0x8);
        assert_eq!(offset_of!(TsfpScriptRule, unknown_00c), 0xc);
        assert_eq!(offset_of!(TsfpScriptRule, condition_count), 0x50);
        assert_eq!(offset_of!(TsfpScriptRule, condition_indices), 0x54);
        assert_eq!(offset_of!(TsfpScriptRule, unknown_058), 0x58);
        assert_eq!(offset_of!(TsfpScriptRule, action_count), 0x88);
        assert_eq!(offset_of!(TsfpScriptRule, action_indices), 0x8c);
        assert_eq!(offset_of!(TsfpScriptRule, unknown_090), 0x90);
        assert_eq!(offset_of!(TsfpScriptRule, event_selector), 0x21c);
        assert_eq!(size_of::<TsfpScriptRule>(), 0x220);
        assert_eq!(align_of::<TsfpScriptRule>(), 4);
        assert_eq!(offset_of!(TsfpVoiceDefinition, unknown_000), 0x0);
        assert_eq!(offset_of!(TsfpVoiceDefinition, kind), 0x8);
        assert_eq!(offset_of!(TsfpVoiceDefinition, group), 0xc);
        assert_eq!(offset_of!(TsfpVoiceDefinition, label), 0x10);
        assert_eq!(offset_of!(TsfpVoiceDefinition, object_ref), 0x14);
        assert_eq!(offset_of!(TsfpVoiceDefinition, fallback_ref), 0x18);
        assert_eq!(offset_of!(TsfpVoiceDefinition, unknown_01c), 0x1c);
        assert_eq!(offset_of!(TsfpVoiceDefinition, controls), 0xb0);
        assert_eq!(size_of::<TsfpVoiceDefinition>(), 0xb4);
        assert_eq!(align_of::<TsfpVoiceDefinition>(), 4);
        assert_eq!(offset_of!(TsfpVoiceInstance, definition), 0x0);
        assert_eq!(size_of::<TsfpVoiceInstance>(), 0x4);
        assert_eq!(align_of::<TsfpVoiceInstance>(), 4);
        assert_eq!(offset_of!(TsfpVoiceBank, samples), 0x0);
        assert_eq!(offset_of!(TsfpVoiceBank, count), 0x4);
        assert_eq!(offset_of!(TsfpVoiceBank, label_base), 0x8);
        assert_eq!(size_of::<TsfpVoiceBank>(), 0xc);
        assert_eq!(align_of::<TsfpVoiceBank>(), 4);
        assert_eq!(offset_of!(TsfpSubtitle, start), 0x0);
        assert_eq!(offset_of!(TsfpSubtitle, duration), 0x4);
        assert_eq!(offset_of!(TsfpSubtitle, unknown_008), 0x8);
        assert_eq!(offset_of!(TsfpSubtitle, label), 0x10);
        assert_eq!(offset_of!(TsfpSubtitle, flags), 0x14);
        assert_eq!(offset_of!(TsfpSubtitle, unknown_015), 0x15);
        assert_eq!(size_of::<TsfpSubtitle>(), 0x18);
        assert_eq!(align_of::<TsfpSubtitle>(), 4);
        assert_eq!(offset_of!(TsfpModePrefix, unknown_000), 0x0);
        assert_eq!(offset_of!(TsfpModePrefix, setup_count), 0x12c);
        assert_eq!(offset_of!(TsfpModePrefix, setups), 0x130);
        assert_eq!(offset_of!(TsfpModePrefix, rule_count), 0x134);
        assert_eq!(offset_of!(TsfpModePrefix, rules), 0x138);
        assert_eq!(size_of::<TsfpModePrefix>(), 0x13c);
        assert_eq!(align_of::<TsfpModePrefix>(), 4);
        assert_eq!(offset_of!(TsfpScriptEntry, own_setup_index), 0x0);
        assert_eq!(offset_of!(TsfpScriptEntry, key), 0x4);
        assert_eq!(offset_of!(TsfpScriptEntry, resolved_index), 0x8);
        assert_eq!(offset_of!(TsfpScriptEntry, flags), 0xc);
        assert_eq!(offset_of!(TsfpScriptEntry, unknown_00d), 0xd);
        assert_eq!(offset_of!(TsfpScriptEntry, callback), 0x10);
        assert_eq!(offset_of!(TsfpScriptEntry, payload), 0x14);
        assert_eq!(offset_of!(TsfpScriptEntry, state), 0x18);
        assert_eq!(offset_of!(TsfpScriptEntry, unknown_01c), 0x1c);
        assert_eq!(offset_of!(TsfpScriptEntry, auxiliary), 0x20);
        assert_eq!(size_of::<TsfpScriptEntry>(), 0x24);
        assert_eq!(align_of::<TsfpScriptEntry>(), 4);
        assert_eq!(size_of::<GuestAddr>(), 4);
    }
}
