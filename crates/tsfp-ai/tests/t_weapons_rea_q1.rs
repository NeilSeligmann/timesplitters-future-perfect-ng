// SPDX-License-Identifier: GPL-3.0-or-later
//! Tier 1 WEAK differentials for the weapon-cycle selector. Inputs are fabricated;
//! the reference is a register-level transliteration of original x86 disassembly
//! collected with Rizin (not a lifted C2R body).

use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

use tsfp_ports::{
    abi::{ExtRegisters, Fpu, GuestCall, Xmm},
    memory::GuestMemory,
    rea_ai_runtime,
};

const ORDER: u32 = 0x004f_c888;
const ENTRY_FLAGS: u32 = 0x004f_a128;
const EXT: u32 = 0x90000;
const OWNED: u32 = 0x54c;
const STACK: u32 = 0x70000;
const COUNT: usize = 70;

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
    fn put32(&mut self, address: u32, value: u32) {
        for (offset, byte) in value.to_le_bytes().into_iter().enumerate() {
            self.write8(address.wrapping_add(offset as u32), byte);
        }
    }
    fn snapshot(&self) -> BTreeMap<u32, u8> {
        self.0
            .borrow()
            .iter()
            .filter(|(_, byte)| **byte != 0)
            .map(|(a, b)| (*a, *b))
            .collect()
    }
}

struct NoCalls;
impl GuestCall for NoCalls {
    fn call(&mut self, va: u32, _regs: &mut ExtRegisters) {
        panic!("weapon-cycle leaf called guest function {va:#x}");
    }
}

fn regs() -> ExtRegisters {
    ExtRegisters {
        eax: 0,
        ecx: 0x1111,
        edx: 0x2222,
        esp: STACK,
        ebx: 0x3333,
        ebp: 0x4444,
        esi: 0x5555,
        edi: 0x6666,
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

#[derive(Clone, Copy)]
struct Candidate {
    id: u32,
    flags: u8,
    owned: bool,
}

fn reference_x86_transliteration(rows: &[Candidate; COUNT], previous: u32) -> u32 {
    // The x86 body first searches the order vector for previous. EAX starts at
    // zero and remains zero when the search misses; the second loop increments
    // before checking, wraps at 70, and returns on bit 0x10 plus owned byte.
    let mut edx = 0usize;
    for (eax, candidate) in rows.iter().enumerate() {
        if candidate.id == previous {
            edx = eax;
            break;
        }
    }
    let mut ecx = edx;
    loop {
        ecx += 1;
        if ecx >= COUNT {
            ecx = 0;
        }
        let candidate = rows[ecx];
        if candidate.flags & 0x10 != 0 && candidate.owned {
            return candidate.id;
        }
        if ecx == edx {
            return candidate.id;
        }
    }
}

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
}

fn seeded_case(seed: u32) -> (Memory, u32, [Candidate; COUNT]) {
    let mut rng = Rng(0x2440_0000_0000_0001 ^ u64::from(seed));
    let mut memory = Memory::default();
    let rows = core::array::from_fn(|slot| {
        let id = slot as u32;
        let flags = (rng.next() & 0xff) as u8;
        let owned = rng.next() & 3 == 0;
        memory.put32(ORDER + slot as u32 * 4, id);
        memory.write8(ENTRY_FLAGS + id * 0x3c, flags);
        memory.write8(EXT + OWNED + id, u8::from(owned));
        Candidate { id, flags, owned }
    });
    let previous = match seed % 5 {
        0 => rng.next() % COUNT as u32,
        1 => u32::MAX,
        _ => rng.next() % COUNT as u32,
    };
    memory.put32(STACK + 4, EXT);
    memory.put32(STACK + 8, previous);
    (memory, previous, rows)
}

#[test]
fn adapter_matches_weak_disassembly_reference_for_3000_fabricated_cases() {
    rea_ai_runtime::configure(true);
    assert!(rea_ai_runtime::configure_function(0x1b6bf0, true));
    for seed in 0..3000 {
        let (memory, previous, rows) = seeded_case(seed);
        let before = memory.snapshot();
        let mut live = memory.clone();
        let mut cpu = regs();
        assert!(rea_ai_runtime::run(
            0x1b6bf0,
            &mut live,
            &mut NoCalls,
            &mut cpu
        ));
        assert_eq!(
            cpu.eax,
            reference_x86_transliteration(&rows, previous),
            "seed {seed}"
        );
        assert_eq!(cpu.esi, 0x5555);
        assert_eq!(cpu.edi, 0x6666);
        assert_eq!(cpu.esp, STACK);
        assert_eq!(
            live.snapshot(),
            before,
            "selector must not mutate guest state"
        );
    }
}

#[test]
fn no_eligible_entry_falls_back_to_previous_or_order_zero() {
    rea_ai_runtime::configure(true);
    assert!(rea_ai_runtime::configure_function(0x1b6bf0, true));
    for previous in [17, u32::MAX] {
        let mut memory = Memory::default();
        let rows = core::array::from_fn(|slot| {
            memory.put32(ORDER + slot as u32 * 4, slot as u32);
            Candidate {
                id: slot as u32,
                flags: 0,
                owned: false,
            }
        });
        memory.put32(STACK + 4, EXT);
        memory.put32(STACK + 8, previous);
        let mut cpu = regs();
        assert!(rea_ai_runtime::run(
            0x1b6bf0,
            &mut memory,
            &mut NoCalls,
            &mut cpu
        ));
        assert_eq!(cpu.eax, reference_x86_transliteration(&rows, previous));
    }
}

#[test]
fn candidate_after_slot_69_wraps_to_slot_zero() {
    rea_ai_runtime::configure(true);
    assert!(rea_ai_runtime::configure_function(0x1b6bf0, true));
    let mut memory = Memory::default();
    let rows = core::array::from_fn(|slot| {
        memory.put32(ORDER + slot as u32 * 4, slot as u32);
        let row = Candidate {
            id: slot as u32,
            flags: if slot == 0 { 0x10 } else { 0 },
            owned: slot == 0,
        };
        memory.write8(ENTRY_FLAGS + row.id * 0x3c, row.flags);
        memory.write8(EXT + OWNED + row.id, u8::from(row.owned));
        row
    });
    memory.put32(STACK + 4, EXT);
    memory.put32(STACK + 8, 69);
    let mut cpu = regs();
    assert!(rea_ai_runtime::run(
        0x1b6bf0,
        &mut memory,
        &mut NoCalls,
        &mut cpu
    ));
    assert_eq!(cpu.eax, reference_x86_transliteration(&rows, 69));
    assert_eq!(cpu.eax, 0);
}
