// SPDX-License-Identifier: GPL-3.0-or-later
//! TEMPORARY-SHIM (removal T2244): owned view/object render metadata inlet
//! for0x9FF10; only original room-assignment compatibility cells are written.
use crate::{
    abi::ExtRegisters,
    memory::GuestMemory,
    scene::scene_collect::{RoomCandidate, choose_room},
};
use alloc::vec::Vec;
pub fn run<M: GuestMemory>(va: u32, m: &mut M, _r: &mut ExtRegisters) -> bool {
    if va != 0x9ff10 {
        return false;
    }
    let count = m.read32(0x7b4fac) as i32;
    if count <= 0 {
        return true;
    }
    let base = m.read32(0x72f5e8);
    let camera = m.read32(0x7b0cc0);
    let viewer = m.read32(camera + 0x214) as i32;
    let views = m.read32(0x7b0ce0) as i32;
    let masks = m.read32(0x7b0ce4);
    // Guarded legacy record decode: no arbitrary unbounded pool walk.
    if count > 0x226 || viewer < 0 {
        return false;
    }
    let mut updates = Vec::new();
    for i in 0..count as u32 {
        let object = base + i * 0x2e8;
        if m.read32(object + 0x28) & 0xc0000000 != 0 {
            continue;
        }
        let n = m.read8(object + 0x30) as i8;
        let fallback = m.read32(object + 0x38);
        let mut candidates = Vec::new();
        if n > 1 {
            for j in 0..n as u32 {
                let room = m.read32(object + 0x38 + j * 4);
                if room > 200 {
                    return false;
                }
                let depth = m.read32(camera + 0x3bc + room * 12) as i32;
                let visible =
                    viewer < views && m.read8(masks + viewer as u32 * 0xffc + 0xd2c + room) != 0;
                candidates.push(RoomCandidate {
                    room,
                    depth,
                    visible,
                });
            }
        }
        updates.push((object, choose_room(fallback, n > 1, &candidates)));
    }
    // Typed decisions are complete before the legacy outlet writes begin.
    for (object, room) in updates {
        m.write32(object + 0x34, room);
    }
    true
}
