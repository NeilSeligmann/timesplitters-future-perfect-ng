// SPDX-License-Identifier: GPL-3.0-or-later
//! Tier 1 differentials (WEAK: fabricated seeded inputs) for the six character and spawn table
//! leaves of `rea_character_leaves`, run through the real `rea_ai_runtime::run` adapter. No
//! lifted Rust body exists for them, so the reference is an independent register-level
//! transliteration of the REA disassembly written in this file (a second reading, not an oracle).

use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

use tsfp_ports::{
    abi::{ExtRegisters, Fpu, GuestCall, Xmm},
    memory::GuestMemory,
    rea_ai_runtime,
};

const STACK: u32 = 0x70000;

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
    fn deep(&self) -> Memory {
        Memory(Rc::new(RefCell::new(self.0.borrow().clone())))
    }
    fn data(&self) -> BTreeMap<u32, u8> {
        self.0
            .borrow()
            .iter()
            .filter(|(address, value)| !(0x6f000..0x71000).contains(*address) && **value != 0)
            .map(|(a, v)| (*a, *v))
            .collect()
    }
}

fn registers(esp: u32) -> ExtRegisters {
    ExtRegisters {
        eax: 0,
        ecx: 0,
        edx: 0,
        esp,
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

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
    fn below(&mut self, n: u32) -> u32 {
        self.next() % n
    }
}

struct NoCalls;
impl GuestCall for NoCalls {
    fn call(&mut self, va: u32, _regs: &mut ExtRegisters) {
        panic!("leaf made a guest call to {va:#x}");
    }
}

fn enable(va: u32) {
    rea_ai_runtime::configure(true);
    assert!(rea_ai_runtime::configure_function(va, true));
}

/// Run the adapter and return EAX plus the register file and all non-zero memory.
fn adapter(va: u32, memory: &Memory, args: &[u32]) -> (u32, ExtRegisters, BTreeMap<u32, u8>) {
    let mut memory = memory.deep();
    let mut regs = registers(STACK - 16);
    for (i, value) in args.iter().enumerate() {
        memory.write32(regs.esp + 4 + 4 * i as u32, *value);
    }
    assert!(rea_ai_runtime::run(
        va,
        &mut memory,
        &mut NoCalls,
        &mut regs
    ));
    (regs.eax, regs, memory.data())
}

fn expected_data(memory: &Memory, args: &[u32]) -> BTreeMap<u32, u8> {
    let mut memory = memory.deep();
    for (i, value) in args.iter().enumerate() {
        memory.write32(STACK - 16 + 4 + 4 * i as u32, *value);
    }
    memory.data()
}

fn check(
    va: u32,
    seeds: u32,
    build: impl Fn(&mut Rng, &mut Memory) -> Vec<u32>,
    reference: impl Fn(&Memory, &[u32]) -> u32,
) {
    enable(va);
    let mut rng = Rng(0x2430_0000_0000_0001 ^ u64::from(va));
    let mut nonzero = 0;
    for seed in 0..seeds {
        let mut memory = Memory::default();
        let args = build(&mut rng, &mut memory);
        let (eax, regs, data) = adapter(va, &memory, &args);
        let want = reference(&memory, &args);
        assert_eq!(eax, want, "{va:#x} seed {seed} args {args:x?}");
        assert_eq!(
            data,
            expected_data(&memory, &args),
            "{va:#x} seed {seed} wrote memory"
        );
        assert_eq!(
            (regs.ebx, regs.esi, regs.edi, regs.ebp, regs.esp),
            (0, 0, 0, 0, STACK - 16),
            "{va:#x} preserved registers"
        );
        nonzero += u32::from(want != 0);
    }
    assert!(
        nonzero > seeds / 20,
        "{va:#x}: reference result was almost always 0 ({nonzero})"
    );
}

fn pick(rng: &mut Rng, specials: &[u32]) -> u32 {
    match rng.below(4) {
        0 => specials[rng.below(specials.len() as u32) as usize],
        1 => rng.below(64),
        _ => rng.next(),
    }
}

#[test]
fn tier1_pair_field_4_12fc0() {
    check(
        0x12fc0,
        3000,
        |rng, m| {
            m.write32(0x74c368, 0x200000);
            let (a, b) = (rng.below(40), rng.below(40));
            // fill the entry the reference reads, wherever the formula puts it
            let hi = a.max(b) as i32;
            let lo = a.min(b) as i32;
            let entry =
                0x200000u32 + ((((hi - 1).wrapping_mul(hi)) >> 1) + lo).wrapping_mul(12) as u32;
            m.write32(entry + 4, rng.next() | 1);
            vec![a, b]
        },
        |m, args| {
            let (a, b) = (args[0] as i32, args[1] as i32);
            let (hi, lo) = if a > b { (a, b) } else { (b, a) };
            let index = ((hi - 1).wrapping_mul(hi)) >> 1;
            m.read32(
                m.read32(0x74c368)
                    .wrapping_add(((index + lo) as u32).wrapping_mul(12))
                    .wrapping_add(4),
            )
        },
    );
}

#[test]
fn tier1_group_index_outside_1c5210() {
    check(
        0x1c5210,
        3000,
        |rng, m| {
            for slot in 0..0x30u32 {
                m.write32(0x4fc99c + 4 * slot, slot * 8 + rng.below(4));
            }
            let group = match rng.below(5) {
                0 => 0,
                1 => rng.next(),
                2 => 0x2e + rng.below(3),
                _ => 1 + rng.below(0x2d),
            };
            let index = if rng.below(2) == 0 {
                rng.below(16)
            } else {
                rng.next()
            };
            vec![group, index]
        },
        |m, args| {
            let (group, index) = (args[0], args[1] as i32);
            if group == 0 {
                return u32::from(index >= m.read32(0x4fc9a0) as i32);
            }
            if (group as i32) < 1 || group >= 0x2f {
                return 0;
            }
            let span = m
                .read32(0x4fc9a0 + group * 4)
                .wrapping_sub(m.read32(0x4fc99c + group * 4)) as i32;
            u32::from(index >= span)
        },
    );
}

#[test]
fn tier1_group_count_1c5250() {
    check(0x1c5250, 3000, |rng, _| vec![rng.next()], |_, _| 0x2f);
}

#[test]
fn tier1_behaviour_table_2aefb0() {
    check(
        0x2aefb0,
        3000,
        |rng, m| {
            let (group, slot) = (rng.below(8), rng.below(8));
            for g in 0..40u32 {
                m.write32(0x520a48 + g * 4, rng.next());
            }
            // also seed the exact cell the formula addresses
            let cell =
                0x520a48u32.wrapping_add(group.wrapping_add(slot.wrapping_mul(4)).wrapping_mul(4));
            m.write32(cell, rng.next() | 1);
            vec![group, slot]
        },
        |m, args| {
            let edx = args[0].wrapping_add(args[1].wrapping_mul(4));
            m.read32(edx.wrapping_mul(4).wrapping_add(0x520a48))
        },
    );
}

#[test]
fn tier1_spawn_table_flags_2b2390() {
    check(
        0x2b2390,
        3000,
        |rng, m| {
            let count = rng.below(12);
            m.write32(0x7a2bcc, count);
            for i in 0..count {
                let descriptor = 0x300000 + i * 0x40;
                m.write32(0x7a2960 + i * 8 + 4, descriptor);
                let flag = if rng.below(8) == 0 { 1 } else { 0 };
                m.write8(descriptor + 0x28, (rng.next() as u8 & 0xfe) | flag);
            }
            if rng.below(10) == 0 {
                m.write32(0x7a2bcc, rng.next() | 0x8000_0000); // negative counts hit the empty path
            }
            vec![]
        },
        |m, _| {
            let count = m.read32(0x7a2bcc) as i32;
            let mut i = 0i32;
            while i < count {
                let descriptor = m.read32(0x7a2960 + (i as u32) * 8 + 4);
                if m.read8(descriptor + 0x28) & 1 != 0 {
                    return 0;
                }
                i += 1;
            }
            1
        },
    );
}

#[test]
fn tier1_type_flag_mask_350e70() {
    check(
        0x350e70,
        3000,
        |rng, m| {
            let object = 0x310000;
            let row = match rng.below(4) {
                0 => 0xffffu32,
                1 => 0x8000,
                _ => rng.below(40),
            };
            m.write8(object + 0x48, row as u8);
            m.write8(object + 0x49, (row >> 8) as u8);
            for r in 0..40u32 {
                m.write32(0x53c508 + r * 20, rng.next());
            }
            for r in [0xffff_u32, 0x8000] {
                let sext = r as u16 as i16 as i32 as u32;
                m.write32(0x53c508u32.wrapping_add(sext.wrapping_mul(20)), rng.next());
            }
            m.write8(
                0x7de455,
                if rng.below(3) == 0 {
                    0x0a
                } else {
                    rng.next() as u8
                },
            );
            m.write32(
                0x7de458,
                if rng.below(3) == 0 {
                    0x80000
                } else {
                    rng.next() & !0x80000
                },
            );
            vec![object, pick(rng, &[1, 0, 0x80000, 0xffff_ffff])]
        },
        |m, args| {
            let (object, mask) = (args[0], args[1]);
            let row = m.read16(object + 0x48) as i16 as i32;
            let word = m.read32(
                (row.wrapping_mul(5) as u32)
                    .wrapping_mul(4)
                    .wrapping_add(0x53c508),
            );
            if mask & 1 != 0 && word & mask != 0 {
                return 1;
            }
            if m.read8(0x7de455) == 0x0a || m.read32(0x7de458) & 0x80000 != 0 {
                word & mask
            } else {
                0
            }
        },
    );
}
