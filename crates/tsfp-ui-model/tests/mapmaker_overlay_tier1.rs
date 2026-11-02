//! T2418 Tier 1: readable Map Maker overlay ramp (0x2FB4C0) and anchor shift (0x30AD30) versus the
//! lifted translations. Memory, constants and callee results are FABRICATED, finite (plus a few
//! NaN and out-of-range cases) and seeded; this is not real editor state (weak evidence, rule 12a).

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use tsfp_ports::abi::{ExtRegisters, Fpu, GuestCall, Xmm};
use tsfp_ports::editor_reference::{f2fb4c0, f30ad30};
use tsfp_ports::mapmaker_overlay::{AnchorShift, OverlayRamp, run_original_entry, shift_anchor};
use tsfp_ports::memory::GuestMemory;

const STACK: u32 = 0x80_000;
const FLAG: u32 = 0x90_000;
const CONTEXT: u32 = 0x91_000;
const LEAF_A: u32 = 0x92_000;
const LEAF_B: u32 = 0x93_000;

#[derive(Clone, Default)]
struct Mem(Rc<RefCell<BTreeMap<u32, u8>>>);

impl Mem {
    fn deep_clone(&self) -> Self {
        Mem(Rc::new(RefCell::new(self.0.borrow().clone())))
    }

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

/// Deterministic callees: `0x774B0` returns the scripted leaf widgets in order, the rest do nothing.
#[derive(Clone)]
struct Host {
    leaves: Vec<u32>,
    next: usize,
    calls: Vec<u32>,
}

impl GuestCall for Host {
    fn call(&mut self, address: u32, registers: &mut ExtRegisters) {
        self.calls.push(address);
        match address {
            0x774b0 => {
                registers.eax = self.leaves[self.next];
                self.next += 1;
            }
            0x7a7b0 | 0x3356a0 => {}
            _ => panic!("unexpected callee {address:#x}"),
        }
    }
}

fn regs() -> ExtRegisters {
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
            control: 0x027f,
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
    m.write32(0x4e8cec, [0u32, 30, 50, 60, 120][(rng.next() % 5) as usize]);
    put_f32(&mut m, 0x475c78, 1.0);
    put_f32(&mut m, 0x475d10, rng.range(0.1, 6.0));
    let alpha = match rng.next() % 12 {
        0 => f32::NAN,
        1 => 1.0,
        2 => 0.0,
        _ => rng.range(-0.5, 1.5),
    };
    put_f32(&mut m, 0x7844ac, alpha);
    m.write32(0x778d34, FLAG);
    m.write32(FLAG, rng.next());
    m.write32(
        0x7842a8,
        [LEAF_A, LEAF_B, 0x94_000][(rng.next() % 3) as usize],
    );
    m.write32(0x7844a8, CONTEXT);
    m.write32(CONTEXT + 0x238, rng.next());
    m.write8(LEAF_A + 0x218, rng.next() as u8);
    m.write8(LEAF_B + 0x218, rng.next() as u8);
    // anchor shift inputs
    put_f32(&mut m, 0x7ba188, rng.range(-300.0, 300.0));
    put_f32(&mut m, 0x4785e4, rng.range(-2.0, 2.0));
    put_f32(&mut m, 0x47db00, rng.range(-50.0, 50.0));
    put_f32(&mut m, 0x7ba038, rng.range(-300.0, 300.0));
    put_f32(&mut m, 0x4760cc, rng.range(-2.0, 2.0));
    put_f32(&mut m, 0x475cd4, rng.range(-2.0, 2.0));
    m
}

fn scripted_host(rng: &mut Rng) -> Host {
    let pick = |rng: &mut Rng| [0, LEAF_A, LEAF_B, 0x94_000][(rng.next() % 4) as usize];
    Host {
        leaves: vec![pick(rng), pick(rng), pick(rng)],
        next: 0,
        calls: Vec::new(),
    }
}

fn diff(lifted: &BTreeMap<u32, u8>, rust: &BTreeMap<u32, u8>) -> Vec<String> {
    lifted
        .keys()
        .chain(rust.keys())
        .filter(|address| lifted.get(address) != rust.get(address))
        .map(|address| {
            format!(
                "{address:#x}: {:?} vs {:?}",
                lifted.get(address),
                rust.get(address)
            )
        })
        .take(8)
        .collect()
}

#[test]
fn overlay_update_matches_lifted() {
    let mut rng = Rng(0x2418_5eed_0001);
    let mut shown = 0;
    let mut hidden = 0;
    for seed in 0..4_000 {
        let start = fabricate(&mut rng);
        let host = scripted_host(&mut rng);
        let (lifted, rust) = (start.deep_clone(), start.deep_clone());
        let (mut lifted_regs, mut rust_regs) = (regs(), regs());
        let (mut lifted_host, mut rust_host) = (host.clone(), host);
        let before = start.read32(0x7844ac);
        let (mut a, mut b) = (lifted.clone(), rust.clone());
        f2fb4c0(&mut a, &mut lifted_host, &mut lifted_regs);
        assert!(run_original_entry(
            0x2fb4c0,
            &mut b,
            &mut rust_host,
            &mut rust_regs
        ));
        let (lifted_live, rust_live) = (lifted.snapshot(), rust.snapshot());
        assert!(!lifted_live.is_empty());
        assert!(
            lifted_live == rust_live,
            "seed {seed}: {:?}",
            diff(&lifted_live, &rust_live)
        );
        assert_eq!(
            lifted_host.calls, rust_host.calls,
            "seed {seed}: callee order"
        );
        assert_eq!(lifted_regs.eax, rust_regs.eax, "seed {seed}: eax");
        assert_eq!(lifted_regs.esp, rust_regs.esp, "seed {seed}: esp");
        for (name, x, y) in [
            ("ebx", lifted_regs.ebx, rust_regs.ebx),
            ("ebp", lifted_regs.ebp, rust_regs.ebp),
            ("esi", lifted_regs.esi, rust_regs.esi),
            ("edi", lifted_regs.edi, rust_regs.edi),
        ] {
            assert_eq!(x, y, "seed {seed}: {name}");
        }
        let after = lifted.read32(0x7844ac);
        if f32::from_bits(after) > f32::from_bits(before) {
            shown += 1;
        } else {
            hidden += 1;
        }
    }
    // the fabricated mix must exercise both ramp directions
    assert!(shown > 100 && hidden > 100, "shown {shown} hidden {hidden}");
    println!("T2418 overlay comparisons: 4000 (up {shown}, down/hold {hidden})");
}

#[test]
fn ramp_clamps_and_keeps_nan() {
    let mut ramp = OverlayRamp {
        alpha: 0.95,
        frame_rate: 60,
        one: 1.0,
        scale: 6.0,
    };
    ramp.step(true);
    assert_eq!(ramp.alpha, 1.0);
    ramp.alpha = 0.05;
    ramp.step(false);
    assert_eq!(ramp.alpha, 0.0);
    ramp.alpha = f32::NAN;
    ramp.step(true);
    assert!(ramp.alpha.is_nan());
}

#[test]
fn anchor_shift_matches_lifted() {
    let mut rng = Rng(0x2418_5eed_0002);
    for seed in 0..4_000 {
        let start = fabricate(&mut rng);
        let (lifted, rust) = (start.deep_clone(), start.deep_clone());
        let (x_io, y_out) = (0xa0_000, 0xa0_100);
        let x_in = match rng.next() % 6 {
            0 => 0x7fff_ffff,
            1 => i32::MIN,
            _ => (rng.next() % 2000) as i32 - 1000,
        } as u32;
        let y_offset = (rng.next() % 2000) as i32 - 1000;
        for memory in [&lifted, &rust] {
            let mut m = memory.clone();
            m.write32(STACK, 0xdead_beef);
            m.write32(STACK + 4, y_out);
            m.write32(STACK + 8, x_io);
            m.write32(STACK + 0xc, 0x1234);
            m.write32(STACK + 0x10, y_offset as u32);
            m.write32(x_io, x_in);
            m.write32(y_out, 0x5a5a_5a5a);
        }
        let (mut lifted_regs, mut rust_regs) = (regs(), regs());
        let mut host = Host {
            leaves: vec![],
            next: 0,
            calls: vec![],
        };
        let mut a = lifted.clone();
        f30ad30(&mut a, &mut host, &mut lifted_regs);
        let mut b = rust.clone();
        assert!(run_original_entry(
            0x30ad30,
            &mut b,
            &mut host,
            &mut rust_regs
        ));
        let (lifted_live, rust_live) = (lifted.snapshot(), rust.snapshot());
        assert!(
            lifted_live == rust_live,
            "seed {seed}: {:?}",
            diff(&lifted_live, &rust_live)
        );
        assert_eq!(lifted.read32(y_out), rust.read32(y_out));
        for (name, x, y) in [
            ("eax", lifted_regs.eax, rust_regs.eax),
            ("ecx", lifted_regs.ecx, rust_regs.ecx),
            ("edx", lifted_regs.edx, rust_regs.edx),
            ("esp", lifted_regs.esp, rust_regs.esp),
        ] {
            assert_eq!(x, y, "seed {seed}: {name}");
        }
        // the pure function agrees with the guest-memory wrapper
        let shift = AnchorShift::read(&start);
        let (px, py) = shift_anchor(&shift, x_in as i32, y_offset);
        assert_eq!(
            (px as u32, py as u32),
            (rust.read32(x_io), rust.read32(y_out))
        );
    }
    println!("T2418 anchor comparisons: 4000");
}
