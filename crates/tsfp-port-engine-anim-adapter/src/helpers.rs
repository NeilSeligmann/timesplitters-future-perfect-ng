// SPDX-License-Identifier: GPL-3.0-or-later
//! Small guest ABI helpers shared by the extracted animation adapter.
use crate::{
    abi::{ExtRegisters, GuestCall},
    memory::GuestMemory,
    start,
};

const CLIP_TABLE: u32 = 0x007e_3954;
const POLICY_TABLE: u32 = 0x007d_e660;
const ID_SLOT_MAP: u32 = 0x007e_03c0;
const KEY_COUNT_OFFSET: u32 = 0x10;

pub(crate) fn arg<M: GuestMemory>(m: &M, entry: &ExtRegisters, index: u32) -> u32 {
    m.read32(entry.esp.wrapping_add(4).wrapping_add(4 * index))
}

pub(crate) fn float<M: GuestMemory>(m: &M, address: u32) -> f32 {
    f32::from_bits(m.read32(address))
}

pub(crate) fn clip_pointer<M: GuestMemory>(m: &M, slot: u32) -> u32 {
    m.read32(m.read32(CLIP_TABLE).wrapping_add(slot.wrapping_mul(4)))
}

pub(crate) fn slot_of<M: GuestMemory>(m: &M, id: u32) -> u32 {
    i16::from_le_bytes([
        m.read8(ID_SLOT_MAP.wrapping_add(id.wrapping_mul(2))),
        m.read8(ID_SLOT_MAP.wrapping_add(id.wrapping_mul(2).wrapping_add(1))),
    ]) as i32 as u32
}

pub(crate) fn clip_key_count<M: GuestMemory>(m: &M, clip: u32) -> u32 {
    m.read32(clip.wrapping_add(KEY_COUNT_OFFSET))
}

pub(crate) fn policy<M: GuestMemory>(m: &M, slot: u32) -> (u32, u32) {
    let base = POLICY_TABLE.wrapping_add(slot.wrapping_mul(8));
    (m.read32(base), m.read32(base.wrapping_add(4)))
}

pub(crate) fn call<M: GuestMemory, C: GuestCall>(
    m: &mut M,
    guest: &mut C,
    entry: &ExtRegisters,
    va: u32,
    args: &[u32],
) -> ExtRegisters {
    let mut cr = *entry;
    cr.esp = entry.esp.wrapping_sub(0x60);
    for (i, value) in args.iter().enumerate() {
        m.write32(cr.esp.wrapping_add(4 * i as u32), *value);
    }
    guest.call(va, &mut cr);
    cr
}

pub(crate) fn write_float<M: GuestMemory>(m: &mut M, address: u32, value: f32) {
    m.write32(address, value.to_bits());
}

pub(crate) fn read_layer<M: GuestMemory>(m: &M, address: u32) -> start::Layer {
    start::Layer(core::array::from_fn(|i| {
        m.read32(address.wrapping_add(4 * i as u32))
    }))
}
