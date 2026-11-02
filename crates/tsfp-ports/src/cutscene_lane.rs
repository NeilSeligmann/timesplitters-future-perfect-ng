// SPDX-License-Identifier: GPL-3.0-or-later
// portreg: mirrors=tsfp-data-formats/cutscenes
//! S4 Tier 2 (T2131): hybrid lane adapters for the sixteen cutscene roots of `tsfp-data-formats::cutscenes` (`CutsceneState`,
//! `KeyNodeProvider`), which were semantic-API ports with no register adapter, so `tools.rust_trace` could not switch them in.
//! The bodies restate the readable ports over the guest memory with the original cdecl argument form (`tsfp-ports` cannot
//! depend on the data-formats crate), checked line by line against the lifted originals. Every root runs through
//! [`crate::anim_lane::guarded`]: a null or wild pointer (`0x8E970`/`0x94220` object, the key cache window buffers) is refused,
//! the write footprint is rolled back and the retained original runs instead, so the lane never faults the host.
//!
//! The nine `save_unlock` roots (`0x1CD3E0` ...) are dispatched by `save_lane.rs`, which also exports their entry points.
//!
//! Register contract (documented): EAX carries the result where the original returns one; ECX/EDX/ESI/EDI scratch are volatile
//! in the callers and are not reproduced, ESP is restored to the entry value. `0x60410` is frameless in the original (it
//! saves/restores EBX/EBP/ESI/EDI itself), so callee-saved registers are untouched here as well. Where the original leaves a
//! value in EAX without returning one (`0x58240` the last argument, `0x6F4B0` the first, `0x8E970` the object, `0x94220` its record
//! pointer) the lane leaves the same value.
use crate::{
    abi::{ExtRegisters, GuestCall},
    memory::{GuestMemory, MappedGuest},
};

#[path = "../../tsfp-port-data-cutscene-key-node/src/lib.rs"]
mod key_node;

/// The sixteen cutscene lane roots in registration order (the nine `save_unlock` roots follow them in `lane_inv::INV_ROOTS`).
pub const ROOTS: [u32; 16] = [
    0x3ee20, 0x58240, 0x60390, 0x60410, 0x61680, 0x64d90, 0x64dd0, 0x665b0, 0x6ee00, 0x6f430,
    0x6f4b0, 0x8e970, 0x94220, 0x98da0, 0x98db0, 0x23efc0,
];

fn arg<M: GuestMemory>(m: &M, esp: u32, index: u32) -> u32 {
    m.read32(esp.wrapping_add(4 + 4 * index))
}

/// `0x60390(frame)`: the frame data pointer through the four streaming windows, or the resident header base.
/// Row index of the signed key in the 13-row table at `0x4D1AC0` (stride 0x44, key at +6), -1 when absent.
fn entry_index<M: GuestMemory>(m: &M, key: u32) -> i32 {
    for index in 0..13u32 {
        if m.read16(0x4d1ac6 + index * 0x44) as i16 as i32 as u32 == key {
            return index as i32;
        }
    }
    -1
}

fn entry_pointer<M: GuestMemory>(m: &M, key: u32) -> u32 {
    let index = entry_index(m, key);
    if index < 0 {
        0
    } else {
        0x4d1ac0 + index as u32 * 0x44
    }
}

/// Dispatches a cutscene root; `false` when `va` is not one of them (the nine save_unlock roots belong to `save_lane`).
pub fn run<M: GuestMemory, C: GuestCall>(
    va: u32,
    m: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
) -> bool {
    if !ROOTS.contains(&va) {
        return false;
    }
    let entry_esp = regs.esp;
    let handled = crate::anim_lane::guarded(va, m, guest, regs, |view, guest, regs| {
        run_inner(va, view, guest, regs)
    });
    regs.esp = entry_esp;
    handled
}

fn run_inner<M: GuestMemory, C: GuestCall>(
    va: u32,
    m: &mut M,
    _guest: &mut C,
    regs: &mut ExtRegisters,
) -> bool {
    let esp = regs.esp;
    match va {
        0x3ee20 => {
            m.write32(0x6b83e0, 0);
            m.write32(0x6b83e4, 0);
            regs.eax = 0;
        }
        0x58240 => {
            let mapped = m.read16(0x7e03c0u32.wrapping_add(arg(m, esp, 0).wrapping_mul(2))) as i16
                as i32 as u32;
            let entry = 0x7de660u32.wrapping_add(mapped.wrapping_mul(8));
            let (first, second) = (arg(m, esp, 1), arg(m, esp, 2));
            m.write32(entry, first);
            m.write32(entry.wrapping_add(4), second);
            regs.eax = second;
        }
        0x60390 => regs.eax = key_node::frame_pointer(m, arg(m, esp, 0)),
        0x60410 => {
            let (frame, node) = (arg(m, esp, 0) as i32, arg(m, esp, 1));
            key_node::key_node_with_regs(m, frame, node, regs);
        }
        0x61680 => {
            for offset in (0..48).step_by(4) {
                m.write32(0x6d7840 + offset, 0);
            }
            for address in [0x6d785c, 0x6d7868, 0x6d7860, 0x6d786c, 0x6d7864] {
                m.write32(address, u32::MAX);
            }
            m.write32(0x6d7840, 1);
            m.write32(0x6d7870, 0);
            m.write32(0x6d7874, 0);
            regs.eax = 0;
        }
        0x64d90 => regs.eax = entry_pointer(m, arg(m, esp, 0)),
        0x64dd0 => {
            let (alternate, key) = (arg(m, esp, 0), arg(m, esp, 1));
            let row = entry_pointer(m, key);
            regs.eax = if row == 0 {
                0
            } else {
                u32::from(m.read16(row + if alternate == 0 { 12 } else { 14 }))
            };
        }
        0x665b0 => m.write32(0x6f11bc, 0),
        0x6ee00 => m.write32(0x7bb248, u32::MAX),
        0x6f430 => {
            let index = arg(m, esp, 0) as i32;
            regs.eax = if (0..20).contains(&index) {
                m.read32(0x51fc48 + index as u32 * 36)
            } else {
                u32::MAX
            };
        }
        0x6f4b0 => {
            let (first, second) = (arg(m, esp, 0), arg(m, esp, 1));
            m.write32(0x6f73d0, first);
            m.write32(0x6f73d4, second);
            regs.eax = first;
        }
        0x8e970 => {
            let p = arg(m, esp, 0);
            for offset in [0x1b8u32, 0x1ec, 0x1f0] {
                m.write32(p.wrapping_add(offset), 0);
            }
            for (offset, value) in [(0x1f6u32, 0u8), (0x1f7, 0)] {
                m.write8(p.wrapping_add(offset), value);
            }
            for (offset, value) in [
                (0x1f8u32, 255u8),
                (0x1f9, 255),
                (0x1fa, 0),
                (0x87, 0),
                (0x1fb, 0),
                (0x1fc, 0),
                (0x1fd, 0),
            ] {
                m.write8(p.wrapping_add(offset), value);
            }
            m.write8(p.wrapping_add(0x7c), 0xff);
            m.write8(p.wrapping_add(0x7d), 0xff);
            regs.eax = p;
        }
        0x94220 => {
            // Arguments 1, 3 and 4 are bytes written to +1F8, +1F9 and +1FA (argument 2 is unused); order +1F8, +1FA, +1F9.
            let p = arg(m, esp, 0);
            let (first, second, third) = (
                arg(m, esp, 1) as u8,
                arg(m, esp, 3) as u8,
                arg(m, esp, 4) as u8,
            );
            m.write8(p.wrapping_add(0x1f8), first);
            m.write8(p.wrapping_add(0x1fa), third);
            m.write8(p.wrapping_add(0x1f9), second);
            let record = m.read32(p);
            let flags = m.read32(record.wrapping_add(0x18));
            m.write32(record.wrapping_add(0x18), flags | 4);
            regs.eax = record;
        }
        0x98da0 => regs.eax = m.read32(0x70e164),
        0x98db0 => {
            let record = m.read32(0x70e164);
            regs.eax = record;
            if record != 0 {
                m.write32(record.wrapping_add(0x1c), 1);
                m.write32(0x4d977c, 0);
            }
        }
        0x23efc0 => {
            m.write32(0x78cd58, u32::MAX);
            m.write32(0x75ce58, u32::MAX);
            m.write32(0x78cd54, 0);
            m.write32(0x75ce5c, 0);
            regs.eax = 0;
        }
        _ => return false,
    }
    true
}

macro_rules! export {
    ($name:ident, $va:expr) => {
        /// # Safety
        /// `cpu` is a live C rust_cpu object and offset maps the guest window.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $name(cpu: *mut crate::lane::LaneCpu, offset: usize) {
            let mut memory = unsafe { MappedGuest::new(offset) };
            crate::lane_inv::execute($va, &mut memory, unsafe { &mut *cpu });
        }
    };
}
export!(tsfp_lane_0003ee20, 0x3ee20);
export!(tsfp_lane_00058240, 0x58240);
export!(tsfp_lane_00060390, 0x60390);
export!(tsfp_lane_00060410, 0x60410);
export!(tsfp_lane_00061680, 0x61680);
export!(tsfp_lane_00064d90, 0x64d90);
export!(tsfp_lane_00064dd0, 0x64dd0);
export!(tsfp_lane_000665b0, 0x665b0);
export!(tsfp_lane_0006ee00, 0x6ee00);
export!(tsfp_lane_0006f430, 0x6f430);
export!(tsfp_lane_0006f4b0, 0x6f4b0);
export!(tsfp_lane_0008e970, 0x8e970);
export!(tsfp_lane_00094220, 0x94220);
export!(tsfp_lane_00098da0, 0x98da0);
export!(tsfp_lane_00098db0, 0x98db0);
export!(tsfp_lane_0023efc0, 0x23efc0);

#[cfg(test)]
mod tests {
    use super::key_node::{frame_pointer, key_node};
    use super::*;
    use alloc::{vec, vec::Vec};

    struct Flat(Vec<u8>);
    impl GuestMemory for Flat {
        fn read8(&self, address: u32) -> u8 {
            self.0.get(address as usize).copied().unwrap_or(0)
        }
        fn write8(&mut self, address: u32, value: u8) {
            if let Some(slot) = self.0.get_mut(address as usize) {
                *slot = value;
            }
        }
    }

    #[test]
    fn roots_match_the_dispatch_table() {
        let text = include_str!("cutscene_lane.rs");
        let table = text
            .split("match va {")
            .nth(1)
            .expect("table")
            .split("_ => return false")
            .next()
            .expect("end");
        for va in ROOTS {
            assert!(table.contains(&alloc::format!("{va:#x} =>")), "{va:#x}");
        }
        assert_eq!(text.matches("\nexport!(tsfp_lane_").count(), ROOTS.len());
    }

    #[test]
    fn key_node_cache_replaces_the_smaller_slot() {
        let mut m = Flat(vec![0; 0x800000]);
        // resident (non-streaming) data: header at 0x1000 with base offset +4, three frames of one node, offsets table at 0x2000
        m.write32(0x7de530, 0);
        m.write32(0x7bb260, 3);
        m.write32(0x7bb284, 1);
        m.write32(0x6d7818, 0x1000);
        m.write32(0x1004, 0x100);
        m.write32(0x6d783c, 0x2000);
        m.write32(0x2004, 0x10);
        // frame 0 is clamped to 0, resolves to header + 0x100 + 0x10, goes to slot 2 (first == second == -1000)
        assert_eq!(key_node(&mut m, 5, 1), 0x1000 + 0x100 + 0x10);
        assert_eq!(m.read32(0x4d19cc), 1);
        assert_eq!(m.read32(0x6d7830), 0x1110);
        // the second call hits slot two's frame (clamped 2 != 0), then replaces by the comparison chain
        let again = key_node(&mut m, 5, 1);
        assert_eq!(
            m.read32(0x6d782c) == again || m.read32(0x6d7830) == again,
            true
        );
    }

    #[test]
    fn streaming_window_reads_frame_interval_and_stride() {
        let mut m = Flat(vec![0; 0x800000]);
        m.write32(0x6d7834, 0x1000); // window records
        m.write32(0x7de540, 2); // first slot selects record two
        m.write32(0x1010, 10); // start frame
        m.write32(0x1014, 5); // frame count
        m.write32(0x6d7818, 0x2000); // header
        m.write32(0x2014, 0x100); // slot stride
        m.write32(0x6d7838, 0x3000); // frame offsets
        m.write32(0x302c, 0x20); // frame 11
        m.write32(0x7de530, 0x4000); // streaming base
        assert_eq!(frame_pointer(&mut m, 11), 0x4020);
        // The upper bound is exclusive; fall through to the resident header.
        m.write32(0x2004, 0x30);
        assert_eq!(frame_pointer(&mut m, 15), 0x2030);
    }

    #[test]
    fn table_lookups_and_slot_word() {
        let mut m = Flat(vec![0; 0x800000]);
        for row in 0..13u32 {
            m.write8(0x4d1ac6 + row * 0x44, (row + 1) as u8);
            m.write8(0x4d1ac0 + row * 0x44 + 0xc, 0x21);
            m.write8(0x4d1ac0 + row * 0x44 + 0xe, 0x32);
        }
        assert_eq!(entry_pointer(&m, 3), 0x4d1ac0 + 2 * 0x44);
        assert_eq!(entry_pointer(&m, 99), 0);
        let mut guest = NoCall;
        // SAFETY: ExtRegisters is plain old data (integers and floats), all zero is a valid value.
        let mut regs: ExtRegisters = unsafe { core::mem::zeroed() };
        regs.esp = 0x7000;
        m.write32(0x7004, 1);
        m.write32(0x7008, 5);
        assert!(run_inner(0x64dd0, &mut m, &mut guest, &mut regs));
        assert_eq!(regs.eax, 0x32);
        m.write32(0x7004, 0xffff_ffff);
        assert!(run_inner(0x6f430, &mut m, &mut guest, &mut regs));
        assert_eq!(regs.eax, u32::MAX);
    }

    struct NoCall;
    impl GuestCall for NoCall {
        fn call(&mut self, _va: u32, _regs: &mut ExtRegisters) {
            panic!("no nested call expected");
        }
    }
}
