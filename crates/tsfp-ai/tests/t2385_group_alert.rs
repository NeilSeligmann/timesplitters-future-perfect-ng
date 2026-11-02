// SPDX-License-Identifier: GPL-3.0-or-later
//! T2385 Tier 1 differential: Rust group alert sweep (`0x267FF0`, adapter
//! `rea_ai_runtime::run` over `rea_group_alert`) against the lifted original.
//! Inputs are fabricated by a seeded generator and callee services are mocked,
//! so this is WEAK evidence (Tier 1, not Tier 2).

use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

use tsfp_ports::{
    abi::{ExtRegisters, Fpu, GuestCall, Xmm},
    c2r::ai_behavior,
    memory::GuestMemory,
    rea_ai_runtime,
};

const STACK: u32 = 0x70000;
const TABLE: u32 = 0x7a2d88;

/// Shared storage: clones alias, `deep` copies.
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
    fn put(&mut self, address: u32, value: u32) {
        self.write32(address, value);
    }
    /// Memory outside the call stack, which the lifted code scribbles on.
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

#[derive(Default)]
struct Mock {
    seed: u32,
    calls: Vec<(u32, Vec<u32>)>,
}

/// The mock is driven through a memory-aware wrapper because `GuestCall::call`
/// does not receive memory: arguments are captured from a shared stack copy.
struct MemMock {
    memory: Memory,
    inner: Mock,
}

impl GuestCall for MemMock {
    fn call(&mut self, va: u32, regs: &mut ExtRegisters) {
        let memory = self.memory.clone();
        let args: Vec<u32> = (0..3).map(|i| memory.read32(regs.esp + i * 4)).collect();
        let kept = match va {
            0x59e00 => args.clone(),
            0x25faf0 | 0x9cc40 | 0xd6dd0 => args[..1].to_vec(),
            0x17570 if args[1] != 0x200 => args.clone(),
            _ => args[..2].to_vec(),
        };
        let hash = va
            .wrapping_mul(0x9e37_79b1)
            .wrapping_add(args[0].wrapping_mul(31))
            .wrapping_add(args[1])
            .wrapping_add(self.inner.seed);
        regs.eax = match va {
            0xd6dd0 | 0x15e120 | 0x59e00 => (hash >> 7) & 1,
            _ => 0,
        };
        self.inner.calls.push((va, kept));
    }
}

fn build(rng: &mut Rng) -> (Memory, u32, [u32; 3]) {
    let mut m = Memory::default();
    let mut next_block = 0x100000u32;
    let mut block = || {
        let b = next_block;
        next_block += 0x1000;
        b
    };
    m.put(0x4788fc, 0x4170_0000);
    m.write8(0x7de455, if rng.below(8) == 0 { 9 } else { 10 });
    m.put(0x7de458, if rng.below(8) == 0 { 0x4000_0000 } else { 0 });
    for group in 0..20u32 {
        let base = TABLE + group * 0x60;
        let count = if rng.below(3) == 0 { 0 } else { rng.below(4) };
        m.put(base - 8, count);
        for index in 0..count {
            let object = block();
            let ext = block();
            let behavior = block();
            let linked = block();
            let linked_ext = block();
            m.put(base + 8 + index * 4, object);
            m.put(object + 0x7c, ext);
            m.put(ext, object);
            m.put(ext + 4, linked);
            m.put(ext + 8, rng.below(4));
            m.put(
                ext + 0x10,
                if rng.below(2) == 0 { 0x0100_0000 } else { 0 } | rng.below(4),
            );
            m.put(ext + 0x14, rng.below(2));
            m.put(
                ext + 0x28,
                if rng.below(2) == 0 { 0x400 } else { 0 } | (rng.below(4) << 4),
            );
            {
                let v = rng.below(0x1_0000);
                m.write8(ext + 0x4a, v as u8);
                m.write8(ext + 0x4b, (v >> 8) as u8);
            }
            m.put(ext + 0xbec, rng.next() & 0x1f | (rng.below(2) << 7));
            m.put(ext + 0xbf0, behavior);
            m.put(behavior + 0x54, rng.next() & 0x4009);
            m.put(ext + 0xac8, 0xdead_0000 + rng.below(16));
            m.put(linked + 0x20, if rng.below(2) == 0 { 8 } else { 3 });
            m.put(linked + 0x7c, linked_ext);
            m.put(linked_ext + 0x678, 0x1000 + rng.below(1000));
            m.put(linked + 0x110, 0x2000 + rng.below(1000));
        }
    }
    let kind = rng.below(4);
    let subject = block();
    let ext = block();
    let behavior = block();
    let source = block();
    let source_ext = block();
    let mask = rng.next() & 0xf_ffff;
    let mut args = [kind, subject, 0x3000 + rng.below(64)];
    match kind {
        0 => {
            m.put(subject + 0x7c, ext);
            m.put(ext + 0xbf0, behavior);
            m.put(behavior + 0x54, rng.below(2));
            m.put(ext + 0xbec, mask);
            m.put(ext + 8, rng.below(4));
        }
        1 => {
            m.put(subject + 4, mask);
            m.put(subject + 0x10, if rng.below(4) == 0 { 0 } else { source });
            m.put(source + 0x20, if rng.below(2) == 0 { 8 } else { 3 });
            m.put(source + 0x7c, source_ext);
            m.put(source_ext + 0x678, 0x4000 + rng.below(100));
            m.put(source + 0x110, 0x5000 + rng.below(100));
        }
        _ => {
            m.put(subject + 0xc, mask);
            m.put(subject + 0x10, rng.below(500));
        }
    }
    args[1] = subject;
    (m, 0, args)
}

fn run_side(
    mut memory: Memory,
    args: [u32; 3],
    seed: u32,
    lifted: bool,
) -> (Memory, Vec<(u32, Vec<u32>)>) {
    let mut guest = MemMock {
        memory: memory.clone(),
        inner: Mock {
            seed,
            calls: Vec::new(),
        },
    };
    let mut regs = registers(STACK - 16);
    // cdecl frame: [esp] return slot, then three arguments.
    memory.write32(regs.esp, 0);
    for (i, value) in args.iter().enumerate() {
        memory.write32(regs.esp + 4 + 4 * i as u32, *value);
    }
    if lifted {
        ai_behavior::game_char_alert_group_members_set_target_and_wake_by_group_mask(
            &mut memory,
            &mut guest,
            &mut regs,
        );
    } else {
        assert!(rea_ai_runtime::run(
            0x267ff0,
            &mut memory,
            &mut guest,
            &mut regs
        ));
    }
    (memory, guest.inner.calls)
}

#[test]
fn tier1_group_alert_matches_lifted_267ff0() {
    rea_ai_runtime::configure(true);
    let mut rng = Rng(0x2385_267f_f0aa_5501);
    let mut sweeps = 0usize;
    let mut raised = 0usize;
    for seed in 0..3000u32 {
        let (memory, _, args) = build(&mut rng);
        // Both sides' stack frames are laid out identically before the call.
        let (lifted_memory, lifted_calls) = run_side(memory.deep(), args, seed, true);
        let (native_memory, native_calls) = run_side(memory, args, seed, false);
        let context = format!("seed {seed} args {args:x?}");
        assert_eq!(lifted_calls, native_calls, "{context}");
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
        sweeps += usize::from(!lifted_calls.is_empty());
        raised += lifted_calls.iter().filter(|(va, _)| *va == 0x17570).count();
    }
    assert!(sweeps > 500, "{sweeps}");
    assert!(raised > 100, "{raised}");
}
