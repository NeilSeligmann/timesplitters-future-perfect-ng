// SPDX-License-Identifier: GPL-3.0-or-later
use crate::memory::GuestMemory;

#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-engine-object/src/lib.rs"]
mod object;
struct ObjectReader<'a, M>(&'a M);
impl<M: GuestMemory> object::ObjectMemory for ObjectReader<'_, M> {
    fn read8(&self, address: u32) -> u8 {
        self.0.read8(address)
    }
    fn write8(&mut self, _: u32, _: u8) {
        panic!("read-only object access");
    }
}
struct ObjectAdapter<'a, M>(&'a mut M);
impl<M: GuestMemory> object::ObjectMemory for ObjectAdapter<'_, M> {
    fn read8(&self, address: u32) -> u8 {
        self.0.read8(address)
    }
    fn write8(&mut self, address: u32, value: u8) {
        self.0.write8(address, value);
    }
}
pub fn table_entry<M: GuestMemory>(m: &M, object: u32, table: u32, key: u32) -> u32 {
    object::table_entry_address(&ObjectReader(m), object, table, key)
}
pub fn set_enabled<M: GuestMemory>(m: &mut M, object: u32, enable: u32) -> u32 {
    object::set_enabled(&mut ObjectAdapter(m), object, enable)
}
pub fn request_state<M: GuestMemory>(m: &mut M, object: u32, argument: u32) -> u32 {
    object::request_state(&mut ObjectAdapter(m), object, argument)
}
pub fn record_address<M: GuestMemory>(m: &M, id: u32, offset: u32) -> u32 {
    m.read32(0x74c458)
        .wrapping_add((id & 0xffff) << 9)
        .wrapping_add(offset)
}
// portreg: va=0x001656d0 tier=1
pub fn record_positive<M: GuestMemory>(m: &M, id: u32) -> u32 {
    u32::from((m.read32(record_address(m, id, 0x1d0)) as i32) > 0)
}
// portreg: va=0x00168100 tier=1
pub fn record_set_flags<M: GuestMemory>(m: &mut M, id: u32, bits: u32) -> u32 {
    let address = record_address(m, id, 0x1c0);
    let old = m.read32(address);
    m.write32(address, old | bits);
    old
}
pub fn select_pending<M: GuestMemory>(m: &mut M, extension: u32) -> u32 {
    let state = m.read32(extension.wrapping_add(0x20));
    if matches!(state, 0 | 1 | 2 | 4) {
        m.write32(extension.wrapping_add(0x9d0), 3);
    }
    state
}
pub fn state_allowed<M: GuestMemory>(m: &M, extension: u32) -> u32 {
    object::state_allowed(&ObjectReader(m), extension)
}
pub fn code_weight<M: GuestMemory>(m: &M, object: u32) -> u32 {
    if matches!(m.read16(object.wrapping_add(0x94)), 0x5c7 | 0x5d2 | 0x5e1) {
        100
    } else {
        75
    }
}
pub fn has_bit<M: GuestMemory>(m: &M, object: u32, bit: u32) -> u32 {
    u32::from(m.read32(object.wrapping_add(0xa0)) & 1u32.wrapping_shl(bit) != 0)
}
pub fn add_header<M: GuestMemory>(m: &M, block: u32, value: u32) -> u32 {
    if m.read32(block) != 0 {
        value.wrapping_add(8)
    } else {
        value
    }
}
