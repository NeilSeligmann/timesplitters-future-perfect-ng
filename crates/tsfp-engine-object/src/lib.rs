// SPDX-License-Identifier: GPL-3.0-or-later
#![no_std]

//! Shared original-verified object leaves and exact integer additions.
//! See docs/t1835-rust-wave2.md for direct and transitive evidence boundaries.

pub mod closure;
pub mod exact;
#[cfg(feature = "object-handle-stamp")]
pub mod handle_stamp;

#[path = "../../tsfp-engine-memory/src/gprs.rs"]
pub mod gprs;
pub mod object_leaves;
pub mod w4_object;
pub mod w4b_object;

/// Guest byte memory with 32-bit wrapping addresses (little endian).
///
/// Multi-byte accesses wrap at the end of the 32-bit address space.
///
/// ```
/// use std::collections::HashMap;
/// use tsfp_engine_object::ObjectMemory;
/// struct Memory(HashMap<u32, u8>);
/// impl ObjectMemory for Memory {
///     fn read8(&self, address: u32) -> u8 { *self.0.get(&address).unwrap_or(&0) }
///     fn write8(&mut self, address: u32, value: u8) { self.0.insert(address, value); }
/// }
/// let mut memory = Memory(HashMap::new());
/// memory.write32(u32::MAX - 1, 0x1234_5678);
/// assert_eq!(memory.read32(u32::MAX - 1), 0x1234_5678);
/// assert_eq!(memory.read16(0), 0x1234);
/// ```
pub trait ObjectMemory {
    fn read8(&self, address: u32) -> u8;
    fn write8(&mut self, address: u32, value: u8);
    fn read16(&self, address: u32) -> u16 {
        u16::from_le_bytes([self.read8(address), self.read8(address.wrapping_add(1))])
    }
    fn read32(&self, address: u32) -> u32 {
        u32::from_le_bytes(core::array::from_fn(|i| {
            self.read8(address.wrapping_add(i as u32))
        }))
    }
    fn write32(&mut self, address: u32, value: u32) {
        for (i, byte) in value.to_le_bytes().into_iter().enumerate() {
            self.write8(address.wrapping_add(i as u32), byte);
        }
    }
}

const FLAGS_0C: u32 = 0x0c;
const ENABLE_FLAGS_28: u32 = 0x28;
const PREVIOUS_STATE_110: u32 = 0x110;
const CURRENT_STATE_114: u32 = 0x114;
const STATE_TIMER_118: u32 = 0x118;
const STATE_LOCKED: u32 = 0x10;
const CLEARED_ON_REQUEST: u32 = 0x20400;

/// 00012820 game_table_entry_address: base(table+4) + (u16 at key+6 << 3 or 4).
pub fn table_entry_address<M: ObjectMemory>(m: &M, object: u32, table: u32, key: u32) -> u32 {
    let shift = if m.read8(object.wrapping_add(FLAGS_0C)) & 0x10 != 0 {
        3
    } else {
        4
    };
    m.read32(table.wrapping_add(4))
        .wrapping_add(u32::from(m.read16(key.wrapping_add(6))) << shift)
}

/// 00071C70 game_object_set_enabled: bit0 of O+0x28. Returns the stored flags.
// portreg: va=0x00071c70
pub fn set_enabled<M: ObjectMemory>(m: &mut M, object: u32, enable: u32) -> u32 {
    let address = object.wrapping_add(ENABLE_FLAGS_28);
    let flags = (m.read32(address) & !1) | u32::from(enable != 0);
    m.write32(address, flags);
    flags
}

// portreg: va=0x000d2ce0
/// 000D2CE0 game_object_request_state. The requested value is read AFTER the
/// previous-state store, so an argument aliasing O+0x110 observes that store.
pub fn request_state<M: ObjectMemory>(m: &mut M, object: u32, argument: u32) -> u32 {
    let current = m.read32(object.wrapping_add(CURRENT_STATE_114));
    if current == STATE_LOCKED {
        return current;
    }
    m.write32(object.wrapping_add(PREVIOUS_STATE_110), current);
    let requested = m.read32(argument);
    m.write32(object.wrapping_add(CURRENT_STATE_114), requested);
    let flags = m.read32(object.wrapping_add(FLAGS_0C)) & !CLEARED_ON_REQUEST;
    m.write32(object.wrapping_add(STATE_TIMER_118), 0);
    m.write32(object.wrapping_add(FLAGS_0C), flags);
    flags
}

/// 00243040 game_state_not_7_or_9: state word at extension+0x0c.
pub fn state_allowed<M: ObjectMemory>(m: &M, extension: u32) -> u32 {
    u32::from(!matches!(m.read32(extension.wrapping_add(FLAGS_0C)), 7 | 9))
}
pub mod synth_leaves;
