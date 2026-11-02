// SPDX-License-Identifier: GPL-3.0-or-later
//! T2385 Tier 1 differential: Rust nav link advance (`0x15D500`, adapter
//! `rea_ai_runtime::run` over `rea_nav_link`) against the lifted original.
//! Inputs are fabricated by a seeded generator and callee services are mocked,
//! so this is WEAK evidence (Tier 1, not Tier 2).

use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

use tsfp_ports::{
    abi::{ExtRegisters, Fpu, GuestCall, Xmm},
    c2r::engine_object,
    memory::GuestMemory,
    rea_ai_runtime,
};

// The integration-test binary links tsfp-ports as a normal dependency, so its
// crate-level `cfg(test)` host callback stub is not included. The host hook is
// diagnostic-only for this fabricated-input differential.
const STACK: u32 = 0x70000;
const OBJECT: u32 = 0x100000;
const EXT: u32 = 0x101000;
const NODES: u32 = 0x110000;
const RECORDS: u32 = 0x120000;
const PLAYER: u32 = 0x130000;
const PLAYER_EXT: u32 = 0x131000;
const LIST: u32 = 0x132000;
const FIRST: u32 = 0x133000;
const FIRST_EXT: u32 = 0x134000;
const RECORD_COUNT: u32 = 3;
const NODE_COUNT: u32 = 8;

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

/// Callee services keyed by a hash of the call so both sides see equal answers.
struct Mock {
    memory: Memory,
    seed: u32,
    calls: Calls,
    search_results: Vec<u32>,
}

impl GuestCall for Mock {
    fn call(&mut self, va: u32, regs: &mut ExtRegisters) {
        let args: Vec<u32> = (0..3)
            .map(|i| self.memory.read32(regs.esp + i * 4))
            .collect();
        let used = match va {
            0x1596f0 | 0x15a180 => &args[..3],
            0x15be70 | 0x15d490 | 0x15a050 | 0x9a420 | 0x9cc40 | 0x9a4c0 | 0x160b00 => &args[..1],
            _ => &args[..0],
        };
        let hash = va
            .wrapping_mul(0x9e37_79b1)
            .wrapping_add(
                used.iter()
                    .fold(0u32, |a, v| a.wrapping_mul(31).wrapping_add(*v)),
            )
            .wrapping_add(self.seed)
            .wrapping_add((self.calls.len() as u32).wrapping_mul(0x85eb_ca6b));
        let hash = (hash ^ (hash >> 15)).wrapping_mul(0x2c1b_3c6d);
        regs.eax = match va {
            0x9a420 => match (hash >> 9) % 3 {
                0 => 0,
                n => RECORDS + (n - 1) * 0x40,
            },
            0x9cc40 => ((hash >> 9) % (NODE_COUNT + 2)).wrapping_sub(1),
            0x9a4c0 => (hash >> 9) % 4,
            0x1596f0 => match (hash >> 9) % 6 {
                0 => 0xffff_fffe,
                1 => 0xffff_ffff,
                2 => 0xffff_fffd,
                n => n - 3,
            },
            0x15a180 => (hash >> 9) & 1,
            _ => 0,
        };
        if va == 0x1596f0 {
            self.search_results.push(regs.eax);
        }
        self.calls.push((va, used.to_vec()));
    }
}

fn grid(rng: &mut Rng) -> u32 {
    [0.0f32, 0.5, 1.0, 2.0, -1.0][rng.below(5) as usize].to_bits()
}

fn build(rng: &mut Rng) -> (Memory, [u32; 2]) {
    let mut m = Memory::default();
    m.write32(0x475d24, 0.1f32.to_bits());
    m.write32(0x47d568, 0.45f32.to_bits());
    m.write32(
        0x47dd90,
        [1.21f32, 0.25, 1.0, 2.25][rng.below(4) as usize].to_bits(),
    );
    m.write32(0x715f2c, NODES);
    m.write32(0x7356d8, PLAYER);
    m.write32(PLAYER + 0x7c, PLAYER_EXT);
    m.write32(PLAYER_EXT + 0x678, rng.below(NODE_COUNT));
    m.write32(0x7b0c48, LIST);
    m.write32(LIST + 0x14, FIRST);
    m.write32(FIRST + 0x7c, FIRST_EXT);
    m.write32(FIRST_EXT + 0x678, rng.below(NODE_COUNT));
    for node in 0..NODE_COUNT {
        let base = NODES + node * 32;
        m.write32(base, rng.below(4));
        for axis in 0..3 {
            m.write32(base + 0xc + axis * 4, grid(rng));
        }
    }
    for record in 0..RECORD_COUNT {
        let base = RECORDS + record * 0x40;
        m.write8(base + 8, rng.below(4) as u8);
        for axis in 0..3 {
            m.write32(base + 0xc + axis * 4, grid(rng));
        }
    }
    m.write32(OBJECT + 0x7c, EXT);
    for axis in 0..3 {
        m.write32(OBJECT + 0x64 + axis * 4, grid(rng));
    }
    let held = match rng.below(4) {
        0 | 1 => 0,
        n => RECORDS + (n - 2) * 0x40,
    };
    m.write32(EXT + 0x24, rng.next());
    m.write32(EXT + 0x28, rng.next());
    m.write32(EXT + 0x2c, rng.next());
    m.write32(EXT + 0x678, rng.below(NODE_COUNT));
    m.write32(EXT + 0x680, rng.next());
    m.write32(EXT + 0x684, rng.below(NODE_COUNT));
    m.write32(EXT + 0x9a8, rng.below(3));
    m.write32(EXT + 0x9ac, if rng.below(2) == 0 { 0 } else { 1 });
    m.write32(
        EXT + 0x9b0,
        [0.0f32, 0.001, 0.04, 0.3, f32::NAN][rng.below(5) as usize].to_bits(),
    );
    m.write32(EXT + 0xacc, held);
    m.write32(EXT + 0xad0, rng.next());
    m.write32(EXT + 0xb14, rng.next());
    let requested = match rng.below(4) {
        0 => rng.below(NODE_COUNT),
        1 => rng.next() | 0x8000_0000,
        2 => 0x1f40 + rng.below(0x172),
        _ => [0x1f3f, 0x1f40, 0x20b1, 0x20b2, 0x209d, 0x209e][rng.below(6) as usize],
    };
    (m, [OBJECT, requested])
}

fn run_side(
    mut memory: Memory,
    args: [u32; 2],
    seed: u32,
    lifted: bool,
) -> (Memory, Calls, Vec<u32>, u32) {
    let mut guest = Mock {
        memory: memory.clone(),
        seed,
        calls: Vec::new(),
        search_results: Vec::new(),
    };
    let mut regs = registers(STACK - 16);
    memory.write32(regs.esp, 0);
    for (i, value) in args.iter().enumerate() {
        memory.write32(regs.esp + 4 + 4 * i as u32, *value);
    }
    if lifted {
        engine_object::game_object_advance_nav_link_with_reverse_link_trace_slot_assign_and_message_7_by_id_range(
            &mut memory,
            &mut guest,
            &mut regs,
        );
    } else {
        assert!(rea_ai_runtime::run(
            0x15d500,
            &mut memory,
            &mut guest,
            &mut regs
        ));
    }
    (memory, guest.calls, guest.search_results, regs.eax)
}

#[test]
fn tier1_nav_link_matches_lifted_15d500() {
    rea_ai_runtime::configure(true);
    assert!(rea_ai_runtime::configure_function(0x15d500, true));
    let mut rng = Rng(0x2385_15d5_00aa_5501);
    let (mut reached, mut committed, mut retried, mut held_hits, mut hold) = (0, 0, 0, 0, 0);
    for seed in 0..6000u32 {
        let (memory, args) = build(&mut rng);
        let (lifted_memory, lifted_calls, lifted_searches, lifted_result) =
            run_side(memory.deep(), args, seed, true);
        let (native_memory, native_calls, native_searches, native_result) =
            run_side(memory, args, seed, false);
        let context = format!(
            "seed {seed} args {args:x?} lifted searches {lifted_searches:x?} native searches {native_searches:x?} lifted link {:#x} native link {:#x}",
            lifted_memory.read32(EXT + 0x680),
            native_memory.read32(EXT + 0x680),
        );
        assert_eq!(lifted_calls, native_calls, "{context}");
        assert_eq!(lifted_searches, native_searches, "{context}");
        assert_eq!(lifted_result, native_result, "{context}");
        let (a, b) = (lifted_memory.data(), native_memory.data());
        let diff: Vec<_> = a
            .keys()
            .chain(b.keys())
            .filter(|k| a.get(k) != b.get(k))
            .map(|k| (format!("{k:#x}"), a.get(k).copied(), b.get(k).copied()))
            .take(12)
            .collect();
        assert!(
            diff.is_empty(),
            "{context} diff (addr, lifted, native) {diff:?}"
        );
        reached += usize::from(lifted_result == 1);
        committed += lifted_calls
            .iter()
            .filter(|(va, _)| *va == 0x15a180)
            .count();
        retried += lifted_calls
            .iter()
            .filter(|(va, _)| *va == 0x160b00)
            .count();
        held_hits += usize::from(lifted_calls.is_empty());
        hold += usize::from(lifted_calls.len() == 0 && lifted_result == 0);
    }
    eprintln!(
        "cov reached {reached} committed {committed} retried {retried} nocall {held_hits} hold {hold}"
    );
    assert!(reached > 50, "reached {reached}");
    assert!(committed > 200, "committed {committed}");
    assert!(retried > 200, "retried {retried}");
    assert!(held_hits > 100, "no-callee runs {held_hits}");
    assert!(hold > 100, "hold runs {hold}");
}
