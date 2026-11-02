// SPDX-License-Identifier: GPL-3.0-or-later
// FABRICATED synthetic fixtures (patterned bytes); no retail data.
use tsfp_state::GuestMemory;
use tsfp_state::generated_views::TsfpVoiceBankOwned;
use tsfp_state::save_text::*;
use tsfp_state::values::GuestPtr;

struct Image(Vec<u8>);
const BASE: u32 = 0x7b_c000;
impl GuestMemory for Image {
    fn read8(&self, a: u32) -> u8 {
        self.0[(a - BASE) as usize]
    }
    fn write8(&mut self, a: u32, v: u8) {
        self.0[(a - BASE) as usize] = v;
    }
}
fn image() -> Image {
    // covers 0x7bc000..0x7ec000 (alloc records, slot table, voice banks, a sample area)
    Image((0..0x30000usize).map(|i| (i * 13 + 5) as u8).collect())
}

/// Transcription of the lifted 0x461C0 body (c2r data_text.rs), the Tier 1 reference.
fn lifted_lookup(m: &Image, id: u32) -> u32 {
    if id != 0 {
        let edx = m.read32(0x7e7208);
        let eax = id.wrapping_sub(edx);
        if (eax as i32) >= 0 && (eax as i32) < (m.read32(0x7e7204) as i32) {
            let v = m.read32(m.read32(0x7e7200).wrapping_add(eax.wrapping_mul(4)));
            if v != 0xffff_ffff {
                return v;
            }
        }
        let ecx = id.wrapping_sub(m.read32(0x7e7218));
        if (ecx as i32) >= 0 && (ecx as i32) < (m.read32(0x7e7214) as i32) {
            return m.read32(m.read32(0x7e7210).wrapping_add(ecx.wrapping_mul(4)));
        }
    }
    0xffff_ffff
}

fn text_fixture() -> (Image, TextBank) {
    let mut m = image();
    let mut bank = TextBank::EMPTY;
    bank.set_bank(
        &mut m,
        0,
        TsfpVoiceBankOwned {
            samples: GuestPtr::new(0x7e_0000),
            count: 40,
            label_base: 100,
        },
    );
    bank.set_bank(
        &mut m,
        1,
        TsfpVoiceBankOwned {
            samples: GuestPtr::new(0x7e_1000),
            count: 30,
            label_base: 130,
        },
    );
    for i in 0..40u32 {
        m.write32(
            0x7e_0000 + i * 4,
            if i % 5 == 0 { u32::MAX } else { 1000 + i },
        );
    }
    for i in 0..30u32 {
        m.write32(0x7e_1000 + i * 4, 2000 + i);
    }
    (m, bank)
}

#[test]
fn text_bank_write_through_and_round_trip() {
    let (m, bank) = text_fixture();
    assert_eq!(bank.diff(&m), None);
    assert_eq!(TextBank::load(&m), bank);
    assert_eq!(m.read32(0x7e7208), 100);
    assert_eq!(m.read32(0x7e7214), 30);
    let mut copy = image();
    bank.store(&mut copy);
    assert_eq!(TextBank::load(&copy), bank);
}

#[test]
fn text_lookup_matches_lifted_transcription() {
    let (m, bank) = text_fixture();
    let mut hit = 0;
    for id in (0..400u32).chain([u32::MAX, 0x8000_0000, 99, 100, 139, 140, 159, 160]) {
        let expect = lifted_lookup(&m, id);
        assert_eq!(bank.lookup(&m, id), expect, "id {id}");
        hit += (expect != u32::MAX) as u32;
    }
    assert!(hit > 30, "fixture must exercise real hits, got {hit}"); // non-empty before equality
}

#[test]
fn text_sample_cells_keep_live_dword_reads_and_wrapping_addresses() {
    use std::cell::RefCell;
    struct Trace(RefCell<Vec<u32>>);
    impl GuestMemory for Trace {
        fn read8(&self, _: u32) -> u8 {
            panic!("unexpected byte read")
        }
        fn write8(&mut self, _: u32, _: u8) {
            panic!("unexpected byte write")
        }
        fn read32(&self, address: u32) -> u32 {
            self.0.borrow_mut().push(address);
            0x1234_5678
        }
    }
    let memory = Trace(RefCell::new(Vec::new()));
    let mut bank = TextBank::EMPTY;
    bank.banks[0] = TsfpVoiceBankOwned {
        samples: GuestPtr::new(u32::MAX - 1),
        count: 2,
        label_base: 40,
    };
    assert_eq!(bank.lookup(&memory, 41), 0x1234_5678);
    assert_eq!(*memory.0.borrow(), [2]);
}

#[test]
fn text_mutation_is_detected() {
    let (m, mut bank) = text_fixture();
    bank.banks[1].count += 1;
    assert_eq!(bank.diff(&m), Some((1, "count")));
}

#[test]
fn slots_attach_release_round_trip() {
    let mut m = image();
    let mut slots = SaveSlots::EMPTY;
    for i in 0..SLOT_COUNT {
        m.write32(slot_address(i), 0);
    }
    let rec = |n: u32| 0x7b_c9c0 + n * 0x40;
    m.write32(rec(7) + RECORD_BACK_LINK, 0);
    slots.attach(&mut m, 3, rec(7));
    assert_eq!(m.read32(slot_address(3)), rec(7));
    assert_eq!(m.read32(rec(7) + RECORD_BACK_LINK), slot_address(3));
    assert_eq!(slots.diff(&m), None);
    // re-attach the same record to another slot clears the old cell
    slots.attach(&mut m, 5, rec(7));
    assert_eq!(m.read32(slot_address(3)), 0);
    assert_eq!(slots.heads[3], 0);
    assert_eq!(slots.diff(&m), None);
    assert!(slots.is_active(5) && !slots.is_active(3));
    // release with the in-use flag set zeroes the 0x40 byte record
    m.write32(rec(7), 0xdead);
    m.write32(rec(7) + RECORD_IN_USE, 1);
    slots.release(&mut m, 5);
    assert_eq!(m.read32(slot_address(5)), 0);
    for off in (0..0x40).step_by(4) {
        assert_eq!(m.read32(rec(7) + off), 0);
    }
    assert_eq!(slots.diff(&m), None);
    assert_eq!(SaveSlots::load(&m), slots);
}

#[test]
fn slots_never_touch_the_unrecovered_body() {
    let mut m = image();
    let before = m.0.clone();
    let slots = SaveSlots::load(&m);
    slots.store(&mut m);
    assert_eq!(m.0, before);
    let mut changed = slots;
    changed.heads[9] ^= 0x10;
    assert_eq!(changed.diff(&m), Some(9));
    changed.store(&mut m);
    let mut expect = before;
    let o = (slot_address(9) - BASE) as usize;
    expect[o..o + 4].copy_from_slice(&changed.heads[9].to_le_bytes());
    assert_eq!(m.0, expect);
}
