// SPDX-License-Identifier: GPL-3.0-or-later
//! Tier 1 WEAK differential for the 0x26CE20 AI action scalar. The reference is the retained
//! translated lifted body (`c2r::t1926_characters`) with its 0x59D20 owner-slot callee modeled.
//! All memory and table contents are fabricated; this is not an original runtime observation.

use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

use tsfp_ports::{
    abi::{ExtRegisters, Fpu, GuestCall, Xmm},
    c2r::t1926_characters::game_char_action_by_weapon_record_4f9bac_26ce20,
    memory::GuestMemory,
    rea_ai_runtime,
};

const ACTOR: u32 = 0x10000;
const OWNER: u32 = 0x20000;
const EXT: u32 = 0x30000;
const RECORDS: u32 = 0x40000;
const STACK: u32 = 0x70000;
const TABLES: [u32; 7] = [
    0x520308, 0x5202f8, 0x5202e8, 0x520338, 0x520328, 0x520318, 0x5203b8,
];
const RESULTS: [u32; 7] = [
    0xc2b4_0000,
    0x42b4_0000,
    0x4334_0000,
    0x4334_0000,
    0x42b4_0000,
    0xc2b4_0000,
    0x4334_0000,
];

fn write_constants(m: &mut Memory) {
    m.write32(0x478634, 0xc2b4_0000);
    m.write32(0x478628, 0x42b4_0000);
    m.write32(0x475c84, 0x4334_0000);
}

#[derive(Clone, Default)]
struct Memory(Rc<RefCell<BTreeMap<u32, u8>>>);

impl GuestMemory for Memory {
    fn read8(&self, address: u32) -> u8 {
        self.0.borrow().get(&address).copied().unwrap_or(0)
    }
    fn write8(&mut self, address: u32, value: u8) {
        self.0.borrow_mut().insert(address, value);
    }
}

impl Memory {
    fn data(&self) -> BTreeMap<u32, u8> {
        self.0.borrow().iter().map(|(a, v)| (*a, *v)).collect()
    }
    fn deep(&self) -> Self {
        Self(Rc::new(RefCell::new(self.0.borrow().clone())))
    }
}

fn registers() -> ExtRegisters {
    ExtRegisters {
        eax: 0x1111,
        ecx: 0x2222,
        edx: 0x3333,
        esp: STACK,
        ebx: 0x4444,
        ebp: 0x5555,
        esi: 0x6666,
        edi: ACTOR,
        eflags: 0,
        fpu: Fpu {
            stack: [0.0; 8],
            top: 0,
            control: 0,
        },
        xmm: [Xmm {
            lanes: [0x1234_5678, 0xaaaa_aaaa, 0xbbbb_bbbb, 0xcccc_cccc],
        }; 5],
        xmm_hi: [Xmm::default(); 3],
    }
}

struct Calls(Memory);

impl GuestCall for Calls {
    fn call(&mut self, va: u32, regs: &mut ExtRegisters) {
        assert_eq!(va, 0x59d20, "unexpected lifted helper");
        let actor = self.0.read32(regs.esp + 4);
        let wanted = self.0.read32(regs.esp + 8);
        let owner = self.0.read32(actor + 4);
        let record = self.0.read32(owner + 4);
        let hit = self.0.read32(record + 0xd4) == wanted || self.0.read32(record + 0x12c) == wanted;
        regs.eax = u32::from(hit);
    }
}

fn seed_world(seed: u32) -> (Memory, usize, u32) {
    let mut rng = seed.wrapping_mul(0x9e37_79b9).wrapping_add(0x26ce20);
    let next = |state: &mut u32| {
        *state ^= *state << 13;
        *state ^= *state >> 17;
        *state ^= *state << 5;
        *state
    };
    let mut m = Memory::default();
    write_constants(&mut m);
    let row = (next(&mut rng) % 4) as i32 - 1;
    m.write32(ACTOR + 4, OWNER);
    m.write32(OWNER + 4, OWNER);
    m.write32(ACTOR + 0x7c, EXT);
    m.write32(EXT + 0x204, row as u32);
    m.write32(0x4f9bac, RECORDS);
    if row >= 0 {
        m.write8(
            RECORDS + row as u32 * 0x29c + 0xc,
            (next(&mut rng) & 1) as u8,
        );
    }
    let column = if row < 0 || m.read8(RECORDS + (row.max(0) as u32) * 0x29c + 0xc) & 1 != 0 {
        1
    } else {
        0
    };
    for (i, table) in TABLES.iter().enumerate() {
        m.write32(*table, 0x0a00_0000 + i as u32 * 0x100);
        m.write32(*table + 4, 0x0b00_0000 + i as u32 * 0x100);
    }
    let selected = (next(&mut rng) as usize) % TABLES.len();
    let wanted = m.read32(TABLES[selected] + column * 4);
    match next(&mut rng) % 3 {
        0 => m.write32(OWNER + 0xd4 + 4, wanted),
        1 => m.write32(OWNER + 0x12c + 4, wanted),
        _ => {
            m.write32(OWNER + 0xd4 + 4, 0xdead_0001);
            m.write32(OWNER + 0x12c + 4, wanted);
        }
    }
    (m, selected, RESULTS[selected])
}

fn adapter(m: &Memory) -> ExtRegisters {
    let mut m = m.deep();
    let mut regs = registers();
    rea_ai_runtime::configure(true);
    assert!(rea_ai_runtime::configure_function(0x26ce20, true));
    let mut guest = Calls(m.clone());
    // This binary has process-global adapter state shared with other integration tests.
    // Reassert the global switch immediately before dispatch so parallel test execution
    // cannot leave this case observing another test's configure(false).
    rea_ai_runtime::configure(true);
    assert!(rea_ai_runtime::configure_function(0x26ce20, true));
    assert!(rea_ai_runtime::run(0x26ce20, &mut m, &mut guest, &mut regs));
    regs
}

fn lifted(m: &Memory) -> ExtRegisters {
    let mut m = m.deep();
    let mut regs = registers();
    let mut guest = Calls(m.clone());
    game_char_action_by_weapon_record_4f9bac_26ce20(&mut m, &mut guest, &mut regs);
    regs
}

#[test]
fn tier1_weak_lifted_differential_3000_fabricated_worlds() {
    for seed in 0..3000 {
        let (m, _, _) = seed_world(seed);
        let before = m.data();
        let actual = adapter(&m);
        let expected = lifted(&m);
        assert_eq!(
            actual.xmm[0].lanes[0], expected.xmm[0].lanes[0],
            "seed {seed}"
        );
        assert_eq!(m.data(), before, "owner is read-only, seed {seed}");
    }
}

#[test]
fn mutation_sentinels_cover_order_column_and_all_return_values() {
    // The ordered scan must choose the first table when both owner slots match.
    for i in 0..TABLES.len() {
        let mut m = Memory::default();
        write_constants(&mut m);
        m.write32(ACTOR + 4, OWNER);
        m.write32(OWNER + 4, OWNER);
        m.write32(ACTOR + 0x7c, EXT);
        m.write32(EXT + 0x204, u32::MAX);
        m.write32(0x4f9bac, RECORDS);
        for (j, table) in TABLES.iter().enumerate() {
            m.write32(*table, 0x1000 + j as u32);
            m.write32(*table + 4, 0x2000 + j as u32);
        }
        m.write32(OWNER + 0xd4, m.read32(TABLES[i] + 4));
        m.write32(OWNER + 0x12c, 0xf000 + i as u32);
        assert_eq!(adapter(&m).xmm[0].lanes[0], RESULTS[i], "table {i}");
    }

    // Simultaneous matches distinguish table-order mutation; valid index with clear flag selects
    // column 0; no match distinguishes the required zero fallback from any unconditional return.
    let mut both = Memory::default();
    write_constants(&mut both);
    both.write32(ACTOR + 4, OWNER);
    both.write32(OWNER + 4, OWNER);
    both.write32(ACTOR + 0x7c, EXT);
    both.write32(EXT + 0x204, u32::MAX);
    both.write32(0x4f9bac, RECORDS);
    both.write32(0x520308 + 4, 0x1111);
    both.write32(0x5202f8 + 4, 0x2222);
    both.write32(OWNER + 0xd4, 0x1111);
    both.write32(OWNER + 0x12c, 0x2222);
    assert_eq!(adapter(&both).xmm[0].lanes[0], 0xc2b4_0000);

    let mut no_match = both.deep();
    no_match.write32(OWNER + 0xd4, 0xeeee);
    no_match.write32(OWNER + 0x12c, 0xffff);
    assert_eq!(adapter(&no_match).xmm[0].lanes[0], 0);

    let mut clear_column = both.deep();
    clear_column.write32(EXT + 0x204, 0);
    clear_column.write8(RECORDS + 0xc, 0);
    clear_column.write32(0x520308, 0x3333);
    clear_column.write32(0x520308 + 4, 0x1111);
    clear_column.write32(OWNER + 0xd4, 0x3333);
    assert_eq!(adapter(&clear_column).xmm[0].lanes[0], 0xc2b4_0000);
}
