// SPDX-License-Identifier: GPL-3.0-or-later
//! T2425 Tier 1 differentials (WEAK: fabricated seeded inputs, mocked callees) for the four
//! awareness and route leaves ported in `rea_awareness_leaves`, run through the real
//! `rea_ai_runtime::run` adapter.
//!
//! * `0x349E20`, `0xA0640`, `0x15D470` compare against the lifted c2r bodies (calls, EAX, memory).
//! * `0x3797B0` has no lifted body, so the reference is an independent transliteration of the
//!   REA disassembly written in this file (a second reading, not an oracle run).

use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

use tsfp_ports::{
    abi::{ExtRegisters, Fpu, GuestCall, Xmm},
    c2r::{data_missions, engine_object, ui_hud_2},
    memory::GuestMemory,
    rea_ai_runtime,
};

const STACK: u32 = 0x70000;
const LEVEL_RECORD: u32 = 0x100000;
const ENTRIES: u32 = 0x101000;
const COUNTERS: u32 = 0x102000;
const OUT: u32 = 0x103000;
const OBJECT: u32 = 0x104000;
const EXT: u32 = 0x105000;
const SOURCE: u32 = 0x106000;
const TABLE: u32 = 0x7a38a0;

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

type Calls = Vec<(u32, Vec<u32>)>;

struct Mock {
    memory: Memory,
    seed: u32,
    calls: Calls,
}

impl GuestCall for Mock {
    fn call(&mut self, va: u32, regs: &mut ExtRegisters) {
        let args: Vec<u32> = (0..3)
            .map(|i| self.memory.read32(regs.esp + i * 4))
            .collect();
        let used = match va {
            0x159e50 => &args[..3],
            0x6a850 | 0x25fa10 | 0xa05c0 => &args[..1],
            _ => &args[..0],
        };
        let hash = va
            .wrapping_mul(0x9e37_79b1)
            .wrapping_add(
                used.iter()
                    .fold(0u32, |a, v| a.wrapping_mul(31).wrapping_add(*v)),
            )
            .wrapping_add(self.seed);
        let hash = (hash ^ (hash >> 15)).wrapping_mul(0x2c1b_3c6d);
        regs.eax = match va {
            0x6a850 => {
                if (hash >> 9) % 5 == 0 {
                    0
                } else {
                    LEVEL_RECORD
                }
            }
            0x25fa10 => (hash >> 9) % 4,
            _ => hash,
        };
        self.calls.push((va, used.to_vec()));
    }
}

fn run_side(
    mut memory: Memory,
    va: u32,
    args: &[u32],
    seed: u32,
    lifted: bool,
) -> (Memory, Calls, u32) {
    let mut guest = Mock {
        memory: memory.clone(),
        seed,
        calls: Vec::new(),
    };
    let mut regs = registers(STACK - 16);
    memory.write32(regs.esp, 0);
    for (i, value) in args.iter().enumerate() {
        memory.write32(regs.esp + 4 + 4 * i as u32, *value);
    }
    if lifted {
        match va {
            0x349e20 => {
                ui_hud_2::game_set_field_0x3c_of_all_four_stride_0x68_records_from_arg(
                    &mut memory,
                    &mut regs,
                )
            }
            0xa0640 => data_missions::game_level_spawn_entries_type_1_register_counts_for_found_table_78b2e0_ids(
                &mut memory,
                &mut guest,
                &mut regs,
            ),
            0x15d470 => engine_object::game_object_update_nav_node_from_link_query_with_zero_args(
                &mut memory,
                &mut guest,
                &mut regs,
            ),
            _ => unreachable!(),
        }
    } else {
        assert!(rea_ai_runtime::run(va, &mut memory, &mut guest, &mut regs));
    }
    (memory, guest.calls, regs.eax)
}

fn assert_same(lifted: &(Memory, Calls, u32), native: &(Memory, Calls, u32), context: &str) {
    assert_eq!(lifted.1, native.1, "{context}");
    let (a, b) = (lifted.0.data(), native.0.data());
    let diff: Vec<_> = a
        .keys()
        .chain(b.keys())
        .filter(|k| a.get(k) != b.get(k))
        .map(|k| (format!("{k:#x}"), a.get(k).copied(), b.get(k).copied()))
        .take(12)
        .collect();
    assert!(diff.is_empty(), "{context} diff {diff:?}");
}

fn enable(va: u32) {
    rea_ai_runtime::configure(true);
    assert!(rea_ai_runtime::configure_function(va, true));
}

#[test]
fn tier1_group_alert_value_matches_lifted_349e20() {
    enable(0x349e20);
    let mut rng = Rng(0x2425_349e_20aa_0001);
    for seed in 0..3000u32 {
        let mut memory = Memory::default();
        for cell in [0x77883c, 0x7788a4, 0x77890c, 0x778974, 0x778840] {
            memory.write32(cell, rng.next());
        }
        let args = [rng.next()];
        let lifted = run_side(memory.deep(), 0x349e20, &args, seed, true);
        let native = run_side(memory, 0x349e20, &args, seed, false);
        assert_same(&lifted, &native, &format!("seed {seed}"));
        assert_eq!(lifted.2, native.2, "eax seed {seed}");
    }
}

#[test]
fn tier1_route_nav_update_matches_lifted_15d470() {
    enable(0x15d470);
    let mut rng = Rng(0x2425_15d4_70aa_0002);
    for seed in 0..3000u32 {
        let memory = Memory::default();
        let args = [rng.next()];
        let lifted = run_side(memory.deep(), 0x15d470, &args, seed, true);
        let native = run_side(memory, 0x15d470, &args, seed, false);
        assert_same(&lifted, &native, &format!("seed {seed}"));
        assert_eq!(lifted.2, native.2, "eax seed {seed}");
        assert_eq!(native.1.len(), 1);
    }
}

#[test]
fn tier1_spawn_register_matches_lifted_a0640() {
    enable(0xa0640);
    let mut rng = Rng(0x2425_a064_00aa_0003);
    let (mut registered, mut skipped_counter, mut missing_record) = (0, 0, 0);
    for seed in 0..3000u32 {
        let mut memory = Memory::default();
        memory.write32(0x79094c, rng.below(30));
        memory.write32(0x72f5ec, COUNTERS);
        let count = rng.below(7) as i32 - 1;
        memory.write32(LEVEL_RECORD + 0xbc, ENTRIES);
        memory.write32(LEVEL_RECORD + 0xc0, count as u32);
        for slot in 0..6u32 {
            let entry = ENTRIES + slot * 0x34;
            memory.write32(entry + 0x24, rng.below(3));
            memory.write32(entry + 0x28, rng.next());
            memory.write32(COUNTERS + slot * 4, rng.below(4).wrapping_sub(1));
        }
        let args = [rng.below(4)];
        let lifted = run_side(memory.deep(), 0xa0640, &args, seed, true);
        let native = run_side(memory, 0xa0640, &args, seed, false);
        assert_same(&lifted, &native, &format!("seed {seed}"));
        registered += lifted.1.iter().filter(|(va, _)| *va == 0xa05c0).count();
        skipped_counter += usize::from(lifted.1.iter().all(|(va, _)| *va != 0x25fa10));
        missing_record += usize::from(lifted.1.len() == 1);
    }
    eprintln!("cov registered {registered} nogroup {skipped_counter} norecord {missing_record}");
    assert!(registered > 100, "registered {registered}");
    assert!(skipped_counter > 100, "no group lookups {skipped_counter}");
    assert!(missing_record > 50, "missing record runs {missing_record}");
}

/// Independent transliteration of the `0x3797B0` disassembly (REA, MEASURED static).
fn reference_direction(memory: &Memory, actor: u32) -> [u32; 3] {
    let tab = |i: i32| f32::from_bits(memory.read32(TABLE + ((i & 0x7ff) as u32) * 4));
    let cvt = |v: f32| -> i32 {
        if v.is_nan() || v >= 2147483648.0 || v < -2147483648.0 {
            i32::MIN
        } else {
            v as i32
        }
    };
    let source = memory.read32(memory.read32(actor + 0x7c) + 0xaa0);
    let (mut x, mut y, mut z);
    if source == 0 {
        (x, y, z) = (0.0f32, 0.0f32, 1.0f32);
    } else {
        let yaw = f32::from_bits(memory.read32(actor + 0x70));
        let off = f32::from_bits(memory.read32(source + 0x24));
        let pitch = f32::from_bits(memory.read32(source + 0x20));
        let k = (
            f32::from_bits(0x3c8efa35),
            f32::from_bits(0x43a2f983),
            f32::from_bits(0x3f000000),
        );
        let a = cvt((yaw + off) * k.0 * k.1 + k.2);
        let b = cvt(pitch * k.0 * k.1 + k.2);
        x = tab(a) * tab(b.wrapping_add(0x200));
        y = tab(b);
        z = tab(a.wrapping_add(0x200)) * tab(b.wrapping_add(0x200));
    }
    let len2 = x * x + y * y + z * z;
    if len2 > f32::from_bits(0x33d6bf95) {
        let s = 1.0 / (f64::from(len2).sqrt() as f32) * 1.0;
        x *= s;
        y *= s;
        z *= s;
    }
    [x.to_bits(), y.to_bits(), z.to_bits()]
}

fn float_pick(rng: &mut Rng) -> f32 {
    match rng.below(8) {
        0 => f32::NAN,
        1 => 1.0e12,
        2 => -1.0e12,
        3 => 0.0,
        _ => (rng.next() % 20000) as f32 / 10.0 - 1000.0,
    }
}

#[test]
fn tier1_view_direction_matches_reference_3797b0() {
    enable(0x3797b0);
    let mut rng = Rng(0x2425_3797_b0aa_0004);
    let (mut sourced, mut normalized, mut forward) = (0, 0, 0);
    for seed in 0..3000u32 {
        let mut memory = Memory::default();
        for i in 0..2048u32 {
            let value = match rng.below(10) {
                0 => 0.0f32,
                1 => f32::NAN,
                _ => (rng.next() % 2001) as f32 / 1000.0 - 1.0,
            };
            memory.write32(TABLE + i * 4, value.to_bits());
        }
        memory.write32(OBJECT + 0x7c, EXT);
        memory.write32(OBJECT + 0x70, float_pick(&mut rng).to_bits());
        let has_source = rng.below(4) != 0;
        memory.write32(EXT + 0xaa0, if has_source { SOURCE } else { 0 });
        memory.write32(SOURCE + 0x20, float_pick(&mut rng).to_bits());
        memory.write32(SOURCE + 0x24, float_pick(&mut rng).to_bits());
        for slot in 0..3 {
            memory.write32(OUT + slot * 4, 0xdead_beef);
        }
        let expected = reference_direction(&memory, OBJECT);
        let (after, calls, _) = run_side(memory, 0x3797b0, &[OBJECT, OUT], seed, false);
        let got = [0, 4, 8].map(|offset| after.read32(OUT + offset));
        let context = format!("seed {seed} source {has_source}");
        assert!(calls.is_empty(), "{context}");
        let same =
            |a: u32, b: u32| a == b || (f32::from_bits(a).is_nan() && f32::from_bits(b).is_nan());
        assert!(
            (0..3).all(|i| same(got[i], expected[i])),
            "{context} got {got:x?} expected {expected:x?}"
        );
        sourced += usize::from(has_source);
        forward += usize::from(!has_source);
        normalized += usize::from(f32::from_bits(got[0]).abs() <= 1.0 && has_source);
    }
    eprintln!("cov sourced {sourced} forward {forward} bounded {normalized}");
    assert!(sourced > 1500 && forward > 400 && normalized > 500);
}
