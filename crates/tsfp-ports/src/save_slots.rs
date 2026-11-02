// SPDX-License-Identifier: GPL-3.0-or-later
// portreg: replaces=tsfp-ports/c2r/leaf_wired
// portreg: replaces=tsfp-ports/c2r/network_session
// portreg: replaces=tsfp-ports/c2r/t2049_nx_006
// portreg: wraps=tsfp-ports/leaf_lane
//! T2091: readable SaveSlots release (0x699D0), release by index (0x69D40) and attach (0x69D90) writers. Contract: docs/t-s3-wiring-leaves.md.
//! The ABI boundary retains scratch-register and stack effects; record operations
//! use the typed owner. Selected only through the existing per-function lane switch.
use crate::abi::{ExtRegisters, GuestCall};
use crate::c2r::rt::RegsExt;
use crate::leaf_wire::{
    self,
    save_text::{AllocationRecord, RECORD_KIND_FRESH, RECORD_KIND_NO_CHANGE_FLAG, SlotHeadCell},
};
use crate::memory::GuestMemory;

pub const ROOTS: [u32; 3] = [0x699d0, 0x69d40, 0x69d90];

/// Slot table base and stride (one 0x1F54-byte slot per index, head cell at +0).
const SLOT_TABLE: u32 = 0x7bedc0;
const SLOT_STRIDE: i32 = 0x1f54;

/// Head cell of slot `index`; the index is signed and unchecked, as in the original.
fn slot_cell(index: u32) -> u32 {
    ((index as i32).wrapping_mul(SLOT_STRIDE) as u32).wrapping_add(SLOT_TABLE)
}

/// Callees of the attach writer; they stay guest calls (allocator and profile code are other systems).
const RELEASE_BY_INDEX: u32 = 0x69d40;
const ALLOC_NAMED_RECORD: u32 = 0x69820;
const INIT_NAME_RECORD: u32 = 0x695c0;
const RESET_PROFILE: u32 = 0x2c7010;
/// Slot layout around the profile reset (INFERRED from the 0x69D90 body): name record at +4, a
/// 0x484-dword profile block at +0x80 and a 0x313-dword block at +0x1290 (no recovered layout).
const SLOT_NAME_RECORD: u32 = 0x4;
const SLOT_PROFILE: u32 = 0x80;
const SLOT_PROFILE_DWORDS: u32 = 0x484;
const SLOT_TAIL: u32 = 0x1290;
const SLOT_TAIL_DWORDS: u32 = 0x313;

fn pop<M: GuestMemory>(m: &M, regs: &mut ExtRegisters) -> u32 {
    let value = m.read32(regs.esp);
    regs.esp = regs.esp.wrapping_add(4);
    value
}

fn fill_zero<M: GuestMemory>(m: &mut M, regs: &mut ExtRegisters, start: u32, dwords: u32) {
    for i in 0..dwords {
        m.write32(start.wrapping_add(i * 4), 0);
    }
    regs.edi = start.wrapping_add(dwords * 4);
    regs.ecx = 0;
}

/// Release the slot cell supplied by the caller (cdecl argument 1).
/// Re-read the head after saving EDI: stack/record aliases are observable in the
/// retained original, so this deliberately does not cache a whole record snapshot.
pub fn release<M: GuestMemory>(m: &mut M, regs: &mut ExtRegisters) {
    regs.edx = m.read32(regs.esp.wrapping_add(4));
    release_cell(m, regs);
}

/// Release slot `index` (cdecl argument 1): the 0x699D0 body on the indexed head cell.
pub fn release_by_index<M: GuestMemory>(m: &mut M, regs: &mut ExtRegisters) {
    regs.edx = slot_cell(m.read32(regs.esp.wrapping_add(4)));
    release_cell(m, regs);
}

/// Shared release body; the slot head cell is in EDX (left there, as the original does).
fn release_cell<M: GuestMemory>(m: &mut M, regs: &mut ExtRegisters) {
    let slot = SlotHeadCell(regs.edx);
    regs.eax = slot.read(m);
    if regs.eax != 0 {
        let record = AllocationRecord::new(regs.eax);
        regs.ecx = record.in_use(m);
        record.set_back_link(m, 0);
        if regs.ecx != 0 {
            regs.push(m, regs.edi);
            regs.edi = slot.read(m);
            if regs.edi != 0 {
                let current = AllocationRecord::new(regs.edi);
                if current.first_word(m) != 0 {
                    regs.eax = current.back_link(m);
                    if regs.eax != 0 {
                        leaf_wire::set_slot_head(m, regs.eax, 0);
                    }
                    current.clear(m);
                    regs.eax = 0;
                    regs.ecx = 0;
                }
            }
            regs.edi = m.read32(regs.esp);
            regs.esp = regs.esp.wrapping_add(4);
        }
    }
    leaf_wire::set_slot_head(m, regs.edx, 0);
}

/// Attach a record to slot `index` (cdecl `(index, name, record)`, returns the slot cell in EAX).
/// `record == 0` allocates a new named record; otherwise the given record is moved here, clearing
/// its old slot cell. A slot that already holds a different record is released first. A fresh
/// record (kind -1) resets the slot profile. Head and back-link stores go through the owner.
pub fn attach<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let index = m.read32(regs.esp.wrapping_add(4));
    regs.eax = index;
    regs.push(m, regs.ebx);
    regs.push(m, regs.esi);
    let slot = slot_cell(index);
    regs.esi = slot;
    regs.ecx = SlotHeadCell(slot).read(m);
    regs.ebx = 0;
    regs.push(m, regs.edi);
    regs.edi = m.read32(regs.esp.wrapping_add(0x18));
    let wanted = regs.edi;
    let already_here = regs.ecx != 0 && wanted != 0 && regs.ecx == wanted;
    if already_here {
        regs.ebx = 1;
    } else if regs.ecx != 0 {
        regs.push(m, index);
        regs.call(guest, RELEASE_BY_INDEX);
        regs.esp = regs.esp.wrapping_add(4);
    }
    if regs.edi == 0 {
        regs.edx = m.read32(regs.esp.wrapping_add(0x14));
        regs.push(m, 0);
        regs.push(m, 0xffff_ffff);
        regs.push(m, regs.edx);
        regs.call(guest, ALLOC_NAMED_RECORD);
        leaf_wire::set_slot_head(m, slot, regs.eax);
        AllocationRecord::new(regs.eax).set_back_link(m, slot);
        regs.eax = SlotHeadCell(slot).read(m);
        regs.esp = regs.esp.wrapping_add(0xc);
        AllocationRecord::new(regs.eax).mark_in_use(m);
    } else {
        let record = AllocationRecord::new(regs.edi);
        regs.eax = record.back_link(m);
        if regs.eax != 0 {
            leaf_wire::set_slot_head(m, regs.eax, 0);
        }
        leaf_wire::set_slot_head(m, slot, regs.edi);
        record.set_back_link(m, slot);
        if regs.ebx == 0 {
            regs.eax = record.kind(m);
            if regs.eax != RECORD_KIND_FRESH && regs.eax != RECORD_KIND_NO_CHANGE_FLAG {
                record.mark_changed(m);
            }
        }
    }
    regs.eax = SlotHeadCell(slot).read(m);
    if regs.eax != 0 && AllocationRecord::new(regs.eax).kind(m) == RECORD_KIND_FRESH {
        regs.ecx = slot.wrapping_add(SLOT_NAME_RECORD);
        regs.push(m, regs.ecx);
        regs.call(guest, INIT_NAME_RECORD);
        regs.edx = slot.wrapping_add(SLOT_PROFILE);
        regs.eax = 0;
        regs.push(m, regs.edx);
        fill_zero(m, regs, regs.edx, SLOT_PROFILE_DWORDS);
        regs.call(guest, RESET_PROFILE);
        regs.esp = regs.esp.wrapping_add(8);
        regs.eax = 0;
        fill_zero(m, regs, slot.wrapping_add(SLOT_TAIL), SLOT_TAIL_DWORDS);
    }
    regs.edi = pop(m, regs);
    regs.eax = slot;
    regs.esi = pop(m, regs);
    regs.ebx = pop(m, regs);
}

pub fn run<M: GuestMemory, C: GuestCall>(
    va: u32,
    m: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
) -> bool {
    match va {
        0x699d0 => release(m, regs),
        0x69d40 => release_by_index(m, regs),
        0x69d90 => attach(m, guest, regs),
        _ => return false,
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Default, PartialEq, Eq, Debug)]
    struct Ram(std::collections::BTreeMap<u32, u8>);
    impl GuestMemory for Ram {
        fn read8(&self, address: u32) -> u8 {
            *self.0.get(&address).unwrap_or(&0)
        }
        fn write8(&mut self, address: u32, value: u8) {
            self.0.insert(address, value);
        }
    }

    #[test]
    fn release_matches_translated_writer_for_2000_fabricated_seeds() {
        for seed in 0..2000u32 {
            let slot = 0x7bedc0 + (seed % 16) * 0x1f54;
            let record = 0x7bc9c0 + (seed % 64) * 0x40;
            let mut expected = Ram::default();
            expected.write32(0x1004, slot);
            expected.write32(slot, if seed % 7 == 0 { 0 } else { record });
            expected.write32(record + 0x34, u32::from(seed % 3 != 0));
            expected.write32(record + 0x3c, slot);
            expected.write32(record, u32::from(seed % 5 != 0));
            for i in 1..16 {
                expected.write32(record + i * 4, seed.rotate_left(i));
            }
            let mut actual = expected.clone();
            let mut a: ExtRegisters = unsafe { core::mem::zeroed() };
            a.esp = 0x1000;
            a.edi = seed ^ 0x12345678;
            let mut b = a;
            crate::c2r::leaf_wired::game_release_table_7bedc0_record_pointer_clear_0x40_bytes_and_null(&mut expected, &mut a);
            release(&mut actual, &mut b);
            assert_eq!(actual, expected, "seed {seed}");
            assert_eq!(b, a, "seed {seed}");
        }
    }

    #[test]
    fn release_by_index_matches_translated_writer_for_2000_fabricated_seeds() {
        let (mut released, mut cleared) = (0, 0);
        for seed in 0..2000u32 {
            // index 16 and -1 fall outside the owned table: the raw-store path of the setter
            let index = [seed % 16, 16, u32::MAX][(seed % 11).min(2) as usize];
            let slot = slot_cell(index);
            let record = 0x7bc9c0 + (seed % 64) * 0x40;
            let other_slot = 0x7bedc0 + ((seed / 5) % 16) * 0x1f54;
            let mut expected = Ram::default();
            expected.write32(0x1004, index);
            expected.write32(slot, if seed % 7 == 0 { 0 } else { record });
            expected.write32(other_slot, record);
            for i in 1..16 {
                expected.write32(record + i * 4, seed.rotate_left(i));
            }
            expected.write32(record + 0x34, u32::from(seed % 3 != 0));
            expected.write32(record + 0x3c, if seed % 4 == 0 { 0 } else { other_slot });
            expected.write32(record, u32::from(seed % 5 != 0));
            let mut actual = expected.clone();
            let mut a: ExtRegisters = unsafe { core::mem::zeroed() };
            a.esp = 0x1000;
            a.edi = seed ^ 0x12345678;
            let mut b = a;
            crate::c2r::t2049_nx_006::game_table_7bedc0_entry_release_by_index_clear_record_and_null(&mut expected, &mut a);
            release_by_index(&mut actual, &mut b);
            assert_eq!(actual, expected, "seed {seed}");
            assert_eq!(b, a, "seed {seed}");
            released += usize::from(expected.read32(slot) == 0 && actual.read32(slot) == 0);
            cleared += usize::from(seed % 7 != 0 && seed % 3 != 0 && seed % 5 != 0);
        }
        assert!(
            released == 2000 && cleared > 300,
            "coverage {released} {cleared}"
        );
    }

    /// Deterministic callee stub: records the call order and returns a seed-derived EAX.
    #[derive(Default, Debug, PartialEq)]
    struct Calls {
        log: Vec<(u32, u32)>,
        next: u32,
    }
    impl GuestCall for Calls {
        fn call(&mut self, va: u32, regs: &mut ExtRegisters) {
            self.log.push((va, regs.esp));
            regs.eax = self.next;
            regs.ecx = va ^ 0x5a5a;
            regs.edx = va.rotate_left(3);
        }
    }

    #[test]
    fn attach_matches_translated_writer_for_2000_fabricated_seeds() {
        let mut fresh_seen = 0;
        let mut moved_seen = 0;
        for seed in 0..2000u32 {
            let index = seed % 17; // index 16 is out of the owned table: raw-store path
            let slot = 0x7bedc0u32.wrapping_add(index * 0x1f54);
            let record = 0x7bc9c0 + (seed % 64) * 0x40;
            let other_slot = 0x7bedc0 + ((seed / 3) % 16) * 0x1f54;
            let mut expected = Ram::default();
            expected.write32(0x1004, index);
            expected.write32(0x1008, 0x2000 + seed);
            let arg_record = match seed % 4 {
                0 => 0,
                1 => record,
                _ => record + 0x40,
            };
            expected.write32(0x100c, arg_record);
            expected.write32(slot, if seed % 5 == 0 { 0 } else { record });
            for r in [record, record + 0x40] {
                expected.write32(r + 0x24, [0xffff_ffff, 0xb, seed][(seed as usize / 7) % 3]);
                expected.write32(r + 0x3c, if seed % 3 == 0 { 0 } else { other_slot });
            }
            expected.write32(slot + 0x80, seed);
            expected.write32(slot + 0x1290 + 0x313 * 4 - 4, seed);
            let mut actual = expected.clone();
            let mut a: ExtRegisters = unsafe { core::mem::zeroed() };
            a.esp = 0x1000;
            a.ebx = seed;
            a.esi = !seed;
            a.edi = seed ^ 0x12345678;
            let mut b = a;
            let alloc = record + 0x80;
            let mut ga = Calls {
                next: alloc,
                ..Calls::default()
            };
            let mut gb = Calls {
                next: alloc,
                ..Calls::default()
            };
            crate::c2r::leaf_wired::game_player_setup_name_slot_alloc_copy_string_max_0x1e(
                &mut expected,
                &mut ga,
                &mut a,
            );
            attach(&mut actual, &mut gb, &mut b);
            assert_eq!(actual, expected, "seed {seed}");
            assert_eq!(b, a, "seed {seed}");
            assert_eq!(gb, ga, "seed {seed}");
            fresh_seen += usize::from(ga.log.iter().any(|c| c.0 == RESET_PROFILE));
            moved_seen += usize::from(arg_record != 0);
        }
        assert!(
            fresh_seen > 100 && moved_seen > 1000,
            "coverage {fresh_seen} {moved_seen}"
        );
    }
}
