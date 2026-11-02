// SPDX-License-Identifier: GPL-3.0-or-later
//! T2422 Tier 1 (weak) differentials: `point_tables` owners against the lifted 0x9CC40, 0x59D20 and
//! 0x9A420 on seeded, fabricated tables. Fabricated inputs, so weak evidence by policy.

use std::{cell::RefCell, collections::HashMap, rc::Rc};

use tsfp_ai::point_tables::{
    RecordSlot, key_to_record_id, owner_has_either_slot, record_address, select_record,
};
use tsfp_ports::{
    abi::{ExtRegisters, Fpu, GuestCall, Xmm},
    c2r::{
        data_missions::game_table_715f34_binary_search_key_to_id,
        engine_object::game_point_entry_is_valid_by_flags_and_id_range,
        t2049_exec_001::{
            game_owner_has_either_slot, game_table_715f2c_family_select_by_id_range_and_lookup,
        },
        t2049_exec_002::game_table_715f2c_binary_search_key_to_id,
    },
    memory::GuestMemory,
};

const STACK: u32 = 0x0007_0000;
const PRIMARY: u32 = 0x10000;
const SECONDARY: u32 = 0x20000;
const TERTIARY: u32 = 0x30000;

/// Shared handle: `clone` aliases the same bytes (so the callee sees pushed arguments),
/// `fork` copies them.
#[derive(Default, Clone)]
struct Memory(Rc<RefCell<HashMap<u32, u8>>>);
impl GuestMemory for Memory {
    fn read8(&self, a: u32) -> u8 {
        self.0.borrow().get(&a).copied().unwrap_or(0)
    }
    fn write8(&mut self, a: u32, v: u8) {
        self.0.borrow_mut().insert(a, v);
    }
}
impl Memory {
    fn fork(&self) -> Memory {
        Memory(Rc::new(RefCell::new(self.0.borrow().clone())))
    }
    fn put(&mut self, a: u32, v: u32) {
        self.write32(a, v);
    }
}

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 33) as u32
    }
}

fn registers() -> ExtRegisters {
    ExtRegisters {
        eax: 0,
        ecx: 0,
        edx: 0,
        esp: STACK,
        ebx: 0,
        ebp: 0,
        esi: 0,
        edi: 0,
        eflags: 0,
        fpu: Fpu {
            stack: [0.0; 8],
            top: 0,
            control: 0,
        },
        xmm: [Xmm::default(); 5],
        xmm_hi: [Xmm::default(); 3],
    }
}

/// Lifted callees the originals reach: 0x9A3C0 and 0x9A330 run for real on the shared memory.
struct Callees(Memory);
impl GuestCall for Callees {
    fn call(&mut self, va: u32, regs: &mut ExtRegisters) {
        // The lane pushes a return slot before entry; the lifted bodies read args at esp+4.
        regs.esp = regs.esp.wrapping_sub(4);
        match va {
            0x9a3c0 => game_table_715f34_binary_search_key_to_id(&mut self.0, regs),
            0x9a330 => game_point_entry_is_valid_by_flags_and_id_range(&mut self.0, regs),
            other => panic!("unexpected callee {other:#x}"),
        }
        regs.esp = regs.esp.wrapping_add(4);
    }
}

fn build_tables(rng: &mut Rng, m: &mut Memory) -> (i32, i32) {
    let primary_count = (rng.next() % 40) as i32;
    let secondary_count = (rng.next() % 40) as i32;
    m.put(0x715f2c, PRIMARY);
    m.put(0x715f34, SECONDARY);
    m.put(0x715f38, TERTIARY);
    m.put(0x7b6664, primary_count as u32);
    m.put(0x7b65c0, secondary_count as u32);
    for (base, count, first) in [
        (PRIMARY, primary_count, 0x3e9),
        (SECONDARY, secondary_count, 0x2329),
    ] {
        let mut key = first + (rng.next() % 5) as i32;
        for i in 0..count {
            m.put(record_address(base, i as u32), key as u32);
            m.put(
                record_address(base, i as u32) + 4,
                (rng.next() & 0x40_0f00) | u32::from(rng.next() % 4 != 0),
            );
            key += 1 + (rng.next() % 4) as i32;
        }
    }
    for i in 0..0x20u32 {
        m.put(record_address(TERTIARY, i), rng.next() % 0x3000);
        m.put(
            record_address(TERTIARY, i) + 4,
            (rng.next() & 0x40_0f00) | u32::from(rng.next() % 4 != 0),
        );
    }
    (primary_count, secondary_count)
}

#[test]
fn key_lookup_matches_lifted_0x9cc40() {
    let mut rng = Rng(0x2422);
    let mut hits = 0;
    for _ in 0..3000 {
        let mut m = Memory::default();
        let (pc, sc) = build_tables(&mut rng, &mut m);
        let key = if rng.next() % 2 == 0 {
            // an existing key half the time so the found branch is exercised
            let table = if rng.next() % 2 == 0 {
                (PRIMARY, pc)
            } else {
                (SECONDARY, sc)
            };
            if table.1 > 0 {
                m.read32(record_address(table.0, rng.next() % table.1 as u32)) as i32
            } else {
                7
            }
        } else {
            (rng.next() % 0x3000) as i32 - 5
        };
        let mut lifted = m.fork();
        let mut regs = registers();
        lifted.put(STACK + 4, key as u32);
        let mut guest = Callees(lifted.clone());
        game_table_715f2c_binary_search_key_to_id(&mut lifted, &mut guest, &mut regs);
        let owner = key_to_record_id(
            key,
            pc,
            |i| m.read32(record_address(PRIMARY, i as u32)) as i32,
            sc,
            |i| m.read32(record_address(SECONDARY, i as u32)) as i32,
        );
        hits += usize::from(owner.is_some());
        assert_eq!(
            owner.map_or(u32::MAX, |i| i as u32),
            regs.eax,
            "key {key:#x} pc {pc} sc {sc}"
        );
    }
    assert!(hits > 500, "found branch under-exercised: {hits}");
}

#[test]
fn slot_test_matches_lifted_0x59d20() {
    let mut rng = Rng(0x59d20);
    for _ in 0..3000 {
        let mut m = Memory::default();
        let (owner, record) = (0x4000u32, 0x5000u32);
        let wanted = rng.next() % 4;
        let (a, b) = (rng.next() % 4, rng.next() % 4);
        m.put(STACK + 4, owner);
        m.put(STACK + 8, wanted);
        m.put(owner + 4, record);
        m.put(record + 0xd4, a);
        m.put(record + 0x12c, b);
        let mut regs = registers();
        game_owner_has_either_slot(&mut m, &mut regs);
        assert_eq!(u32::from(owner_has_either_slot(a, b, wanted)), regs.eax);
    }
}

#[test]
fn record_select_matches_lifted_0x9a420() {
    let mut rng = Rng(0x9a420);
    let mut nonzero = 0;
    for _ in 0..3000 {
        let mut m = Memory::default();
        build_tables(&mut rng, &mut m);
        let level = if rng.next() % 2 == 0 {
            0x64
        } else {
            rng.next() % 0x80
        };
        m.put(0x79094c, level);
        let id = match rng.next() % 6 {
            0 => -1,
            1 => 0x1f40 + (rng.next() % 0x600) as i32,
            2 => 0x209e + (rng.next() % 0x20) as i32 - 4,
            _ => (rng.next() % 0x28) as i32,
        };
        m.put(STACK + 4, id as u32);
        let mut lifted = m.fork();
        let mut regs = registers();
        let mut guest = Callees(lifted.clone());
        game_table_715f2c_family_select_by_id_range_and_lookup(&mut lifted, &mut guest, &mut regs);
        let expected = match select_record(id, level) {
            None => 0,
            Some(slot) => {
                let record = match slot {
                    RecordSlot::Primary(i) => record_address(PRIMARY, i),
                    RecordSlot::Secondary(i) => record_address(SECONDARY, i),
                    RecordSlot::Tertiary(i) => record_address(TERTIARY, i),
                };
                let mut check = registers();
                let mut probe = m.fork();
                probe.put(STACK + 4, record);
                game_point_entry_is_valid_by_flags_and_id_range(&mut probe, &mut check);
                if check.eax != 0 { record } else { 0 }
            }
        };
        nonzero += usize::from(expected != 0);
        assert_eq!(expected, regs.eax, "id {id:#x} level {level:#x}");
    }
    assert!(nonzero > 100, "valid branch under-exercised: {nonzero}");
}
