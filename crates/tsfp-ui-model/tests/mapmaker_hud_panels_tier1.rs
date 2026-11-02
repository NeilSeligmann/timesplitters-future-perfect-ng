//! T2415 Tier 1: readable Map Maker HUD panel slides versus the lifted translations of 0x335500,
//! 0x3356A0, 0x3356F0, 0x335790 and 0x335840. Panel records and timing constants are FABRICATED,
//! finite and seeded; this is not real editor state (weak evidence, rule 12a).

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use tsfp_ports::abi::{ExtRegisters, Fpu, GuestCall, Xmm};
use tsfp_ports::editor_reference::{f3356a0, f3356f0, f335500, f335790, f335840};
use tsfp_ports::mapmaker_hud_panels::{PANEL_COUNT, PANEL_STRIDE, PANEL_TABLE, run_original_entry};
use tsfp_ports::memory::GuestMemory;

const STACK: u32 = 0x80_000;
const ENTRIES: [u32; 5] = [0x335500, 0x3356a0, 0x3356f0, 0x335790, 0x335840];

#[derive(Clone, Default)]
struct Mem(Rc<RefCell<BTreeMap<u32, u8>>>);

impl Mem {
    fn deep_clone(&self) -> Self {
        Mem(Rc::new(RefCell::new(self.0.borrow().clone())))
    }

    /// Live state: everything except the dead scratch below the entry stack pointer.
    fn snapshot(&self) -> BTreeMap<u32, u8> {
        let mut live = self.0.borrow().clone();
        live.retain(|&address, _| !(STACK - 0x100..STACK).contains(&address));
        live
    }
}

impl GuestMemory for Mem {
    fn read8(&self, address: u32) -> u8 {
        *self.0.borrow().get(&address).unwrap_or(&0)
    }

    fn write8(&mut self, address: u32, value: u8) {
        self.0.borrow_mut().insert(address, value);
    }
}

/// Routes the lifted bodies' internal calls (0x335500) to the lifted step, on the same memory.
struct Host(Mem);

impl GuestCall for Host {
    fn call(&mut self, address: u32, registers: &mut ExtRegisters) {
        assert_eq!(address, 0x335500, "unexpected callee {address:#x}");
        let mut memory = self.0.clone();
        let mut nested = Host(self.0.clone());
        // The callee returns with esp unchanged (cdecl return already modelled by the caller macro).
        f335500(&mut memory, &mut nested, registers);
    }
}

fn regs(control: u16) -> ExtRegisters {
    ExtRegisters {
        eax: 0,
        ecx: 0,
        edx: 0,
        esp: STACK,
        ebx: 0x1111,
        ebp: 0x2222,
        esi: 0x3333,
        edi: 0x4444,
        eflags: 0,
        fpu: Fpu {
            stack: [0.0; 8],
            top: 0,
            control,
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

    fn unit(&mut self) -> f32 {
        (self.next() % 1_000_000) as f32 / 1_000_000.0
    }

    fn range(&mut self, low: f32, high: f32) -> f32 {
        low + (high - low) * self.unit()
    }
}

fn put_f32(m: &mut Mem, address: u32, value: f32) {
    m.write32(address, value.to_bits());
}

fn fabricate(rng: &mut Rng) -> Mem {
    let mut m = Mem::default();
    m.write32(0x4e8cec, [30u32, 50, 60][(rng.next() % 3) as usize]);
    m.write32(0x4e8cf0, 640);
    put_f32(&mut m, 0x475c78, 1.0);
    put_f32(&mut m, 0x4e7948, rng.range(0.25, 4.0));
    put_f32(&mut m, 0x475cd4, rng.range(0.2, 1.0));
    put_f32(&mut m, 0x4785e0, rng.range(0.2, 1.0));
    put_f32(&mut m, 0x4765e0, rng.range(0.05, 1.5));
    put_f32(&mut m, 0x475cdc, rng.range(0.05, 1.5));
    put_f32(&mut m, 0x47daec, rng.range(-0.5, 0.5));
    for index in 0..PANEL_COUNT {
        let base = PANEL_TABLE + index * PANEL_STRIDE;
        for offset in (0..PANEL_STRIDE).step_by(4) {
            m.write32(base + offset, rng.next());
        }
        m.write32(base, rng.next() % 4);
        for offset in [0x04, 0x08, 0x0c, 0x10, 0x14, 0x18, 0x24, 0x28] {
            put_f32(&mut m, base + offset, rng.range(-200.0, 700.0));
        }
        put_f32(&mut m, base + 0x1c, rng.range(0.0, 400.0));
        let duration = rng.range(0.05, 2.0);
        put_f32(&mut m, base + 0x30, duration);
        put_f32(&mut m, base + 0x2c, rng.range(-0.3, duration * 1.3));
        put_f32(&mut m, base + 0x34, rng.range(-0.2, 1.6));
        put_f32(&mut m, base + 0x38, rng.range(0.0, 1.0));
    }
    m
}

fn call_lifted(va: u32, m: &Mem, r: &mut ExtRegisters) {
    let mut memory = m.clone();
    let mut host = Host(m.clone());
    match va {
        0x335500 => f335500(&mut memory, &mut host, r),
        0x3356a0 => f3356a0(&mut memory, &mut host, r),
        0x3356f0 => f3356f0(&mut memory, &mut host, r),
        0x335790 => f335790(&mut memory, &mut host, r),
        _ => f335840(&mut memory, &mut host, r),
    }
}

/// Runs both implementations of `va` from the same state; returns the compared states.
fn compare(va: u32, index: u32, start: &Mem, control: u16, case: &str) -> (Mem, Mem) {
    let lifted = start.deep_clone();
    let rust = start.deep_clone();
    let mut lifted_regs = regs(control);
    lifted_regs.ecx = index;
    let mut stack = lifted.clone();
    stack.write32(STACK, 0xdead_beef);
    stack.write32(STACK + 4, index);
    let mut rust_regs = lifted_regs;
    let mut rust_stack = rust.clone();
    rust_stack.write32(STACK, 0xdead_beef);
    rust_stack.write32(STACK + 4, index);
    call_lifted(va, &lifted, &mut lifted_regs);
    let mut rust_memory = rust.clone();
    assert!(
        run_original_entry(va, &mut rust_memory, &mut rust_regs),
        "{case}: declined"
    );
    assert!(!lifted.snapshot().is_empty(), "{case}: empty state");
    let (lifted_live, rust_live) = (lifted.snapshot(), rust.snapshot());
    if lifted_live != rust_live {
        let differing: Vec<_> = lifted_live
            .keys()
            .chain(rust_live.keys())
            .filter(|address| lifted_live.get(address) != rust_live.get(address))
            .map(|address| {
                format!(
                    "{address:#x}: {:?} vs {:?}",
                    lifted_live.get(address),
                    rust_live.get(address)
                )
            })
            .take(8)
            .collect();
        panic!("{case}: memory differs (lifted vs rust) {differing:?}");
    }
    assert_eq!(lifted_regs.esp, rust_regs.esp, "{case}: esp");
    if va != 0x335790 {
        assert_eq!(lifted_regs.eax, rust_regs.eax, "{case}: eax");
    }
    for (name, a, b) in [
        ("ebx", lifted_regs.ebx, rust_regs.ebx),
        ("ebp", lifted_regs.ebp, rust_regs.ebp),
        ("esi", lifted_regs.esi, rust_regs.esi),
        ("edi", lifted_regs.edi, rust_regs.edi),
    ] {
        assert_eq!(a, b, "{case}: {name}");
    }
    (lifted, rust)
}

#[test]
fn single_entries_match_lifted() {
    let mut rng = Rng(0x2415_5eed_0001);
    let mut comparisons = 0;
    for seed in 0..1_200 {
        let start = fabricate(&mut rng);
        let control = if seed % 2 == 0 { 0x027f } else { 0x007f };
        let va = ENTRIES[seed % ENTRIES.len()];
        let index = rng.next() % PANEL_COUNT;
        compare(
            va,
            index,
            &start,
            control,
            &format!("seed {seed} va {va:#x} index {index}"),
        );
        comparisons += 1;
    }
    println!("T2415 single-entry comparisons: {comparisons}");
    assert_eq!(comparisons, 1_200);
}

#[test]
fn slide_sequences_match_lifted() {
    let mut rng = Rng(0x2415_5eed_0002);
    let mut comparisons = 0;
    for seed in 0..200 {
        let mut state = fabricate(&mut rng);
        let control = if seed % 2 == 0 { 0x027f } else { 0x007f };
        let panel = rng.next() % PANEL_COUNT;
        let script: Vec<(u32, u32)> = std::iter::once((0x3356f0, panel))
            .chain((0..40).map(|_| (0x3356a0, 0)))
            .chain([(0x335840, panel), (0x335790, 0)])
            .chain((0..40).map(|_| (0x3356a0, 0)))
            .chain([(0x3356f0, panel)])
            .collect();
        for (step, (va, index)) in script.into_iter().enumerate() {
            let (lifted, _) = compare(
                va,
                index,
                &state,
                control,
                &format!("seq {seed} step {step}"),
            );
            state = lifted;
            comparisons += 1;
        }
    }
    println!("T2415 sequence comparisons: {comparisons}");
    assert_eq!(comparisons, 200 * 84);
}
