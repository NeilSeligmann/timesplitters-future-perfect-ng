// SPDX-License-Identifier: GPL-3.0-or-later
//! T2433 Tier 1 (WEAK): readable owner/adapter of 0xB0770, 0x160F50 and 0x162520 versus references.
//!
//! References: `0x162520` is the machine translation `c2r::engine_object_3` of the lifted body
//! (independent of the owner, but a transliteration); `0xB0770` and `0x160F50` have no c2r body, so
//! the reference is a register-level transliteration of the disassembly written in this file
//! (the author's reading of the same bytes, hence weak). Memory is FABRICATED (seeded sparse words
//! with biased values so every branch is hit), not real game state. Each suite also runs a mutation
//! check: a deliberately wrong reference must be caught by the same seeds.
use std::collections::BTreeMap;
use tsfp_ports::abi::{ExtRegisters, Fpu, GuestCall, Xmm};
use tsfp_ports::c2r::engine_object_3;
use tsfp_ports::memory::GuestMemory;
use tsfp_ports::rea_object_flags as owner;
use tsfp_ports::rea_object_runtime as adapter;

#[derive(Clone, Default, PartialEq, Debug)]
struct Mem {
    bytes: BTreeMap<u32, u8>,
}
impl GuestMemory for Mem {
    fn read8(&self, a: u32) -> u8 {
        *self.bytes.get(&a).unwrap_or(&0)
    }
    fn write8(&mut self, a: u32, v: u8) {
        self.bytes.insert(a, v);
    }
}
impl Mem {
    fn put(&mut self, a: u32, v: u32) {
        self.write32(a, v);
    }
}
struct NoCalls;
impl GuestCall for NoCalls {
    fn call(&mut self, a: u32, _: &mut ExtRegisters) {
        panic!("unexpected guest call {a:#x}");
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
    fn pick(&mut self, values: &[u32]) -> u32 {
        values[self.below(values.len() as u32) as usize]
    }
}

const STACK: u32 = 0x0010_0000;
const OBJ: u32 = 0x0020_0000;
const EXT: u32 = 0x0021_0000;
const TAGREC: u32 = 0x0022_0000;

fn blank() -> ExtRegisters {
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
            control: 0x027f,
        },
        xmm: [Xmm::default(); 5],
        xmm_hi: [Xmm::default(); 3],
    }
}

fn regs(rng: &mut Rng, arg: u32, mem: &mut Mem) -> ExtRegisters {
    let mut r = blank();
    r.esp = STACK;
    r.eax = rng.next();
    r.ecx = rng.next();
    r.edx = rng.next();
    r.ebx = rng.next();
    r.esi = rng.next();
    r.edi = rng.next();
    r.ebp = rng.next();
    mem.put(STACK + 4, arg);
    r
}

fn same(a: &ExtRegisters, b: &ExtRegisters) -> bool {
    (a.eax, a.ecx, a.edx, a.ebx, a.esi, a.edi, a.ebp, a.esp)
        == (b.eax, b.ecx, b.edx, b.ebx, b.esi, b.edi, b.ebp, b.esp)
}

// ---- 0xB0770 reference: transliteration of the disassembly ----
fn ref_b0770(m: &Mem, r: &mut ExtRegisters, mutate: bool) {
    r.eax = m.read32(0x007d_e318);
    if r.eax != 0 {
        r.ecx = m.read32(r.esp + 4);
        r.ecx = m.read32(r.ecx.wrapping_add(0x90));
        if r.ecx != 0 {
            if r.ecx == r.eax {
                r.eax = 1;
                return;
            }
            let delta = m.read32(0x004e_7944);
            r.eax = r
                .eax
                .wrapping_sub(if mutate { delta.wrapping_add(1) } else { delta });
            if r.ecx == r.eax {
                r.eax = 1;
                return;
            }
        }
    }
    r.eax = 0;
}

fn build_b0770(rng: &mut Rng) -> (Mem, ExtRegisters) {
    let mut m = Mem::default();
    let current = rng.pick(&[0, 0x1000, 0x2000, 0x40, 0x8000_0000, 4]);
    let delta = rng.pick(&[0, 0x10, 0x1000, 0xffff_fff0, 4]);
    m.put(0x007d_e318, current);
    m.put(0x004e_7944, delta);
    let stamp = match rng.below(6) {
        0 => 0,
        1 => current,
        2 => current.wrapping_sub(delta),
        3 => current.wrapping_sub(delta).wrapping_add(1),
        _ => rng.next(),
    };
    m.put(OBJ + 0x90, stamp);
    let r = regs(rng, OBJ, &mut m);
    (m, r)
}

// ---- 0x160F50 reference ----
fn ref_160f50(m: &Mem, r: &mut ExtRegisters, mutate: bool) {
    let obj = m.read32(r.esp + 4);
    r.eax = obj;
    let kind = m.read32(obj.wrapping_add(0x20));
    r.ecx = m.read32(obj.wrapping_add(0x7c));
    if kind == 8 {
        r.eax = m.read32(r.ecx.wrapping_add(0xbf0));
        r.eax = m.read32(r.eax.wrapping_add(8));
        if r.eax == 0x4543_4956 || r.eax == if mutate { 0x4f43_4957 } else { 0x4f43_4956 } {
            r.eax = 1;
            return;
        }
    }
    r.eax = 0;
}

fn build_160f50(rng: &mut Rng) -> (Mem, ExtRegisters) {
    let mut m = Mem::default();
    m.put(OBJ + 0x20, rng.pick(&[8, 8, 8, 7, 9, 0]));
    m.put(OBJ + 0x7c, EXT);
    m.put(EXT + 0xbf0, TAGREC);
    let noise = rng.next();
    m.put(
        TAGREC + 8,
        rng.pick(&[0x4543_4956, 0x4f43_4956, 0x4543_4957, 0, noise]),
    );
    let r = regs(rng, OBJ, &mut m);
    (m, r)
}

// ---- 0x162520 / 0x162450 / 0x163220 reference: transliteration of the 20-bit loop ----
fn ref_entry_flag(
    m: &mut Mem,
    r: &mut ExtRegisters,
    mask_off: u32,
    table: u32,
    value: u32,
    mutate: bool,
) {
    let index = m.read32(r.esp + 4);
    let owner_rec = m.read32(0x0074_c36c);
    let record = m
        .read32(owner_rec.wrapping_add(0x74))
        .wrapping_add(index.wrapping_mul(0x98));
    let entry = m.read32(0x007a_2820).wrapping_add(index.wrapping_mul(24));
    m.put(entry + 0x10, 0);
    let mut esi = table;
    let mut edx = 2u32;
    loop {
        let offsets: [i32; 5] = [-0x60, 0, 0x60, 0xc0, 0x120];
        for (j, off) in offsets.iter().enumerate() {
            let bit = edx - 2 + j as u32;
            let hit_mask = m.read32(record.wrapping_add(mask_off)) & (1u32 << bit) != 0;
            if hit_mask && m.read32(esi.wrapping_add(*off as u32)) != 0 {
                m.put(entry + 0x10, if mutate { value + 1 } else { value });
            }
        }
        edx += 5;
        esi = esi.wrapping_add(0x1e0);
        if edx - 2 >= 0x14 {
            break;
        }
    }
    r.eax = record;
    r.ecx = edx - 2;
    r.edx = edx;
}

fn build_flag(rng: &mut Rng, mask_off: u32) -> (Mem, ExtRegisters) {
    let mut m = Mem::default();
    let index = rng.below(6);
    m.put(0x0074_c36c, 0x0030_0000);
    m.put(0x0030_0000 + 0x74, 0x0031_0000);
    m.put(0x007a_2820, 0x0032_0000);
    let record = 0x0031_0000 + index * 0x98;
    let mask = match rng.below(5) {
        0 => 0,
        1 => 1 << rng.below(32),
        2 => rng.next(),
        3 => 0xfff0_0000,
        _ => rng.next() & rng.next() & rng.next(),
    };
    m.put(record + mask_off, mask);
    for slot in 0..20u32 {
        let row = 0x007a_2d84 + slot * 0x60;
        m.put(row, if rng.below(4) == 0 { rng.next() | 1 } else { 0 });
        m.put(row + 4, if rng.below(4) == 0 { rng.next() | 1 } else { 0 });
    }
    m.put(0x0032_0000 + index * 24 + 0x10, rng.next());
    let r = regs(rng, index, &mut m);
    (m, r)
}

const SEEDS: u32 = 4000;

fn drive_stamp(mutate: bool) -> u32 {
    let mut rng = Rng(0x2433_b077_0001);
    let mut diverged = 0;
    for _ in 0..SEEDS {
        let (m, r0) = build_b0770(&mut rng);
        let (mut rr, mut ra) = (r0, r0);
        ref_b0770(&m, &mut rr, mutate);
        let mut mm = m.clone();
        adapter::run_original_entry(0xb0770, &mut mm, &mut ra);
        if !same(&rr, &ra) || mm != m {
            diverged += 1;
        }
    }
    diverged
}

fn drive_tag(mutate: bool) -> u32 {
    let mut rng = Rng(0x2433_160f_5001);
    let mut diverged = 0;
    for _ in 0..SEEDS {
        let (m, r0) = build_160f50(&mut rng);
        let (mut rr, mut ra) = (r0, r0);
        ref_160f50(&m, &mut rr, mutate);
        let mut mm = m.clone();
        adapter::run_original_entry(0x160f50, &mut mm, &mut ra);
        if !same(&rr, &ra) || mm != m {
            diverged += 1;
        }
    }
    diverged
}

fn drive_flag(
    mask_off: u32,
    table: u32,
    value: u32,
    rule: owner::EntryFlagRule,
    mutate: bool,
) -> u32 {
    let mut rng = Rng(0x2433_1625_2001 + u64::from(mask_off));
    let mut diverged = 0;
    for _ in 0..SEEDS {
        let (m, r0) = build_flag(&mut rng, mask_off);
        let (mut rr, mut ra) = (r0, r0);
        let mut mr = m.clone();
        ref_entry_flag(&mut mr, &mut rr, mask_off, table, value, mutate);
        let mut ma = m.clone();
        adapter::set_entry_flag(&mut ma, &mut ra, rule);
        if !same(&rr, &ra) || mr != ma {
            diverged += 1;
        }
    }
    diverged
}

#[test]
fn stamp_matches_reference_and_mutant_is_caught() {
    assert_eq!(drive_stamp(false), 0);
    assert!(
        drive_stamp(true) > 0,
        "mutation check: wrong delta must diverge"
    );
}

#[test]
fn vice_vico_matches_reference_and_mutant_is_caught() {
    assert_eq!(drive_tag(false), 0);
    assert!(
        drive_tag(true) > 0,
        "mutation check: wrong VICO tag must diverge"
    );
}

#[test]
fn entry_flag_family_matches_transliteration_and_mutant_is_caught() {
    assert_eq!(
        drive_flag(0x70, 0x007a_2de8, 1, owner::RULE_162520, false),
        0
    );
    assert_eq!(
        drive_flag(0x70, 0x007a_2de4, 1, owner::RULE_162450, false),
        0
    );
    assert_eq!(
        drive_flag(0x38, 0x007a_2de4, 3, owner::RULE_163220, false),
        0
    );
    assert!(
        drive_flag(0x70, 0x007a_2de8, 1, owner::RULE_162520, true) > 0,
        "mutation check: hit value"
    );
}

/// 0x162520 against the machine translation of the lifted body (c2r), same seeds.
#[test]
fn entry_flag_162520_matches_c2r_translation() {
    let mut rng = Rng(0x2433_c2c2_0001);
    let mut compared = 0;
    for _ in 0..SEEDS {
        let (m, r0) = build_flag(&mut rng, 0x70);
        let (mut rc, mut ra) = (r0, r0);
        let mut mc = m.clone();
        let mut guest = NoCalls;
        assert!(engine_object_3::run(0x162520, &mut mc, &mut guest, &mut rc));
        let mut ma = m.clone();
        adapter::set_entry_flag(&mut ma, &mut ra, owner::RULE_162520);
        // the c2r body returns with ESP on the return slot, the adapter keeps the entry ESP
        rc.esp = ra.esp;
        // the lifted body restores EBX/EBP/ESI/EDI and so does the adapter
        assert!(same(&rc, &ra), "register divergence: {rc:?} vs {ra:?}");
        // pushes of the callee-saved registers land below the entry ESP, that residue is not state
        mc.bytes
            .retain(|&a, _| !(STACK - 0x100..STACK).contains(&a));
        assert_eq!(mc, ma, "memory divergence");
        compared += 1;
    }
    assert_eq!(compared, SEEDS);
}

#[test]
fn every_root_is_a_lane_root_and_dispatches() {
    for va in adapter::ROOTS {
        assert!(tsfp_ports::lane_inv::INV_ROOTS.contains(&va));
        let mut m = Mem::default();
        let mut rng = Rng(7);
        let mut r = regs(&mut rng, OBJ, &mut m);
        assert!(adapter::run_original_entry(va, &mut m, &mut r));
    }
    let mut m = Mem::default();
    let mut r = blank();
    assert!(!adapter::run_original_entry(0x1234, &mut m, &mut r));
}
