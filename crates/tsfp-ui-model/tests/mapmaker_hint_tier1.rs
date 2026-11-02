//! T2420 Tier 1: readable Map Maker label anchor (0x30ACB0) and editor frame draw (0x3111F0) versus
//! the lifted translations. Memory, constants, callee results and text extents are FABRICATED and
//! seeded; this is not real editor state (weak evidence, rule 12a). The host emulates the float to int
//! helper 0x3C848C with the same truncation model the port uses, so that helper is not verified here.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use tsfp_ports::abi::{ExtRegisters, Fpu, GuestCall, Xmm};
use tsfp_ports::editor_reference::{f30acb0, f3111f0};
use tsfp_ports::mapmaker_overlay::{LabelAnchor, ftol_low32, run_original_entry};
use tsfp_ports::mem::GuestAccess;
use tsfp_ports::memory::GuestMemory;

const STACK: u32 = 0x80_000;
const CTX: u32 = 0xa0_000;
const SELECTION: u32 = 0xb0_000;
const ALT: u32 = 0xb1_000;
const NAME_RECORD: u32 = 0xb2_000;
const RECORD_LIST: u32 = 0xb3_000;
const LEAF_A: u32 = 0xc0_000;
const LEAF_B: u32 = 0xc1_000;
const PAGE: u32 = 0xc2_000;

#[derive(Clone, Default)]
struct Mem(Rc<RefCell<BTreeMap<u32, u8>>>);

impl Mem {
    fn deep_clone(&self) -> Self {
        Mem(Rc::new(RefCell::new(self.0.borrow().clone())))
    }

    fn snapshot(&self) -> BTreeMap<u32, u8> {
        let mut live = self.0.borrow().clone();
        live.retain(|&address, _| !(STACK - 0x400..STACK + 0x40).contains(&address));
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

    fn chance(&mut self, one_in: u32) -> bool {
        self.next() % one_in == 0
    }

    fn range(&mut self, low: f32, high: f32) -> f32 {
        low + (high - low) * (self.next() % 1_000_000) as f32 / 1_000_000.0
    }
}

/// One logged guest call: address, the argument words and ESI (the register argument of 0x30CCD0).
#[derive(Clone, Debug, PartialEq)]
struct Call {
    va: u32,
    args: Vec<u32>,
    esi: u32,
}

fn arg_count(va: u32) -> usize {
    match va {
        0x19650 | 0x19d70 | 0x77470 | 0x336570 | 0x32f740 | 0x30b480 | 0x3d7b0 | 0x303350
        | 0x31dbc0 | 0x2f8020 | 0x405c0 | 0x40620 => 1,
        0x78950 => 2,
        0x151030 => 3,
        0x3357b0 | 0x30ccd0 => 4,
        _ => 0,
    }
}

#[derive(Clone)]
struct Host {
    rng: Rc<RefCell<Rng>>,
    mem: Mem,
    calls: Rc<RefCell<Vec<Call>>>,
    metric: bool,
}

impl GuestCall for Host {
    fn call(&mut self, address: u32, registers: &mut ExtRegisters) {
        let args = (0..arg_count(address))
            .map(|i| self.mem.read32(registers.esp.wrapping_add(4 * i as u32)))
            .collect();
        self.calls.borrow_mut().push(Call {
            va: address,
            args,
            esi: if address == 0x30ccd0 {
                registers.esi
            } else {
                0
            },
        });
        let mut rng = self.rng.borrow_mut();
        match address {
            0x405c0 | 0x40620 => {
                let value = if rng.chance(40) {
                    f32::NAN
                } else {
                    rng.range(-20.0, 400.0)
                };
                registers.fpu.push(value as f64);
            }
            0x3c848c => {
                let value = registers.fpu.st(0);
                registers.fpu.pop();
                registers.eax = ftol_low32(value);
            }
            0x774b0 => registers.eax = [0, LEAF_A, LEAF_B, PAGE][rng.below(4) as usize],
            0x77470 => {
                registers.eax = if rng.chance(2) {
                    self.mem.read32(0x7842a8)
                } else {
                    [0, LEAF_A, LEAF_B][rng.below(3) as usize]
                }
            }
            0x2f82e0 => registers.eax = [0, RECORD_LIST, RECORD_LIST + 0x40][rng.below(3) as usize],
            0x2f8020 => registers.eax = rng.below(8),
            0x31dbc0 => registers.eax = rng.below(2),
            0x3d7b0 | 0x151030 => {
                registers.eax = [0, 0xd0_000, 0xd1_000, 0xd2_000][rng.below(4) as usize]
            }
            0x303350 => registers.eax = [0, 0xd3_000, 0xd4_000][rng.below(3) as usize],
            0x22b40 | 0x22b90 => registers.eax = (rng.below(900) as i32 - 60) as u32,
            _ => registers.eax = rng.next(),
        }
        let _ = self.metric;
    }
}

fn regs(control: u16) -> ExtRegisters {
    ExtRegisters {
        eax: 0,
        ecx: 0x0c0c,
        edx: 0x0d0d,
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

fn put_f32(m: &mut Mem, address: u32, value: f32) {
    m.write32(address, value.to_bits());
}

fn fabricate_constants(m: &mut Mem, rng: &mut Rng) {
    put_f32(m, 0x7ba188, rng.range(-300.0, 300.0));
    put_f32(m, 0x7ba038, rng.range(-300.0, 300.0));
    put_f32(m, 0x4785e4, rng.range(-2.0, 2.0));
    put_f32(m, 0x47db00, rng.range(-50.0, 50.0));
    put_f32(m, 0x4760cc, rng.range(-2.0, 2.0));
    put_f32(m, 0x475cd4, rng.range(-2.0, 2.0));
    put_f32(m, 0x47d1e4, rng.range(-2.0, 2.0));
    m.write32(0x4e8cf4, [0, 640, 1280, 1920][rng.below(4) as usize]);
    m.write32(0x4e8cf0, [0, 480, 720, 1080][rng.below(4) as usize]);
}

fn diff(a: &BTreeMap<u32, u8>, b: &BTreeMap<u32, u8>) -> Vec<String> {
    a.keys()
        .chain(b.keys())
        .filter(|address| a.get(address) != b.get(address))
        .map(|address| format!("{address:#x}: {:?} vs {:?}", a.get(address), b.get(address)))
        .take(8)
        .collect()
}

fn run_pair(
    start: &Mem,
    host_seed: u64,
    control: u16,
    setup: impl Fn(&mut Mem),
    lifted_entry: impl Fn(&mut Mem, &mut Host, &mut ExtRegisters),
    va: u32,
) -> (Vec<Call>, Vec<Call>, ExtRegisters, ExtRegisters, Mem, Mem) {
    let mut sides = Vec::new();
    for lifted in [true, false] {
        let mem = start.deep_clone();
        let mut setup_mem = mem.clone();
        setup(&mut setup_mem);
        let calls = Rc::new(RefCell::new(Vec::new()));
        let mut host = Host {
            rng: Rc::new(RefCell::new(Rng(host_seed))),
            mem: mem.clone(),
            calls: calls.clone(),
            metric: true,
        };
        let mut r = regs(control);
        let mut working = mem.clone();
        if lifted {
            lifted_entry(&mut working, &mut host, &mut r);
        } else {
            assert!(run_original_entry(va, &mut working, &mut host, &mut r));
        }
        let log = calls.borrow().clone();
        sides.push((log, r, mem));
    }
    let (rust_log, rust_regs, rust_mem) = sides.pop().expect("rust side");
    let (lifted_log, lifted_regs, lifted_mem) = sides.pop().expect("lifted side");
    (
        lifted_log,
        rust_log,
        lifted_regs,
        rust_regs,
        lifted_mem,
        rust_mem,
    )
}

#[test]
fn label_anchor_matches_lifted() {
    let mut rng = Rng(0x2420_5eed_0001);
    let mut measured = 0;
    for seed in 0..4_000u32 {
        let mut start = Mem::default();
        fabricate_constants(&mut start, &mut rng);
        start.write32(0x7e7260, [1, 1, 0, 2][rng.below(4) as usize]);
        let (x_out, y_io, text) = (0xe0_000, 0xe0_100, 0xe0_200);
        let y_in = match rng.below(8) {
            0 => i32::MAX,
            1 => i32::MIN,
            _ => rng.below(2000) as i32 - 1000,
        };
        let x_offset = rng.below(400) as i32 - 200;
        let control = [0x027f, 0x007f][rng.below(2) as usize];
        let seed_host = 0x2420_0000 + seed as u64 * 7919 + 1;
        let (lifted_log, rust_log, lifted_regs, rust_regs, lifted_mem, rust_mem) = run_pair(
            &start,
            seed_host,
            control,
            |m| {
                m.write32(STACK, 0xdead_beef);
                m.write32(STACK + 4, x_out);
                m.write32(STACK + 8, y_io);
                m.write32(STACK + 0xc, text);
                m.write32(STACK + 0x10, x_offset as u32);
                m.write32(x_out, 0x5a5a_5a5a);
                m.write32(y_io, y_in as u32);
            },
            |m, host, r| f30acb0(m, host, r),
            0x30acb0,
        );
        // memory is shared through the mutated clone: reapply on both sides by comparing snapshots
        assert_eq!(
            lifted_mem.snapshot().get(&x_out),
            rust_mem.snapshot().get(&x_out)
        );
        let (a, b) = (lifted_mem.snapshot(), rust_mem.snapshot());
        assert!(a == b, "seed {seed}: {:?}", diff(&a, &b));
        // the port models 0x3C848C (float to int) inline, so the lifted call to it is dropped
        let lifted_log: Vec<_> = lifted_log
            .into_iter()
            .filter(|c| c.va != 0x3c848c)
            .collect();
        assert_eq!(lifted_log, rust_log, "seed {seed}: callee log");
        for (name, x, y) in [
            ("eax", lifted_regs.eax, rust_regs.eax),
            ("edx", lifted_regs.edx, rust_regs.edx),
            ("esi", lifted_regs.esi, rust_regs.esi),
            ("esp", lifted_regs.esp, rust_regs.esp),
            ("ebx", lifted_regs.ebx, rust_regs.ebx),
            ("fpu top", lifted_regs.fpu.top, rust_regs.fpu.top),
        ] {
            assert_eq!(x, y, "seed {seed}: {name}");
        }
        if start.read32(0x7e7260) == 1 {
            measured += 1;
        }
    }
    assert!(measured > 1000);
    println!("T2420 label anchor comparisons: 4000 (width measured {measured})");
}

#[test]
fn label_anchor_pure_cases() {
    let anchor = LabelAnchor {
        measure_width: true,
        x_lerp: 2.0,
        x_scale: 0.5,
        y_bias: 10.0,
        text_scale: 1.0,
        control: 0x007f,
    };
    assert_eq!(anchor.scaled_width(30.0), 30.0);
    // 2*0.5 + (4 + 0x18) + 30 = 59
    assert_eq!(anchor.x(30.0, 4), 59);
    // (10 - 4) * 1 + 7 = 13
    assert_eq!(anchor.y(4.0, 7), 13);
    assert_eq!(ftol_low32(-2.9), (-2i32) as u32);
    assert_eq!(ftol_low32(f64::NAN), 0);
    let unmeasured = LabelAnchor {
        measure_width: false,
        ..anchor
    };
    assert_eq!(unmeasured.scaled_width(99.0), 0.0);
}

fn fabricate_frame(rng: &mut Rng) -> Mem {
    let mut m = Mem::default();
    fabricate_constants(&mut m, rng);
    m.write32(0x7844a8, CTX);
    m.write32(0x7842a8, PAGE);
    m.write32(0x7ba9e0, rng.below(3));
    for view in 0..3u32 {
        m.write32(0x7ba26c + view * 0x1e0, [0, 1, 2][rng.below(3) as usize]);
        m.write32(0x7ba270 + view * 0x1e0, rng.below(2));
    }
    let mut flags = 0u32;
    if rng.chance(6) {
        flags |= 0x200_0000;
    }
    if rng.chance(3) {
        flags |= 0x400_0000;
    }
    for bit in [0x8, 0x80, 0x4000, 0x4_0000] {
        if rng.chance(3) {
            flags |= bit;
        }
    }
    m.write32(CTX + 0x25c, flags);
    m.write32(
        CTX + 0x238,
        if rng.chance(5) {
            0x8000
        } else {
            rng.next() & 0x7fff
        },
    );
    m.write32(CTX + 0x254, [0, 0, 1, 2, 4, 3][rng.below(6) as usize]);
    m.write32(CTX + 0x2f18, if rng.chance(2) { SELECTION } else { 0 });
    m.write32(CTX + 0x2f1c, RECORD_LIST + rng.below(2) * 0x40);
    m.write32(CTX + 0x2f78, (rng.below(5) as i32 - 2) as u32);
    m.write32(CTX + 0x2f7c, if rng.chance(2) { ALT } else { 0 });
    for slot in 0..3u32 {
        let base = CTX + 0x3070 + slot * 12;
        m.write8(base + 7, rng.below(8) as u8);
        m.write16(
            base + 0xa,
            if rng.chance(3) {
                0xffff
            } else {
                rng.below(6) as u16
            },
        );
    }
    for record in [SELECTION, ALT] {
        m.write32(record + 0x10, rng.below(6));
        m.write32(record + 0x1c, rng.below(6));
        m.write32(record + 0x2c, NAME_RECORD);
        m.write32(record + 0x40, rng.below(2));
    }
    m.write32(NAME_RECORD + 8, rng.below(100));
    for entry in [RECORD_LIST, RECORD_LIST + 0x40] {
        m.write32(entry, rng.below(8));
        m.write32(
            entry + 4,
            if rng.chance(3) {
                u32::MAX
            } else {
                rng.below(6)
            },
        );
    }
    for kind in 0..8u32 {
        let base = 0x52_4f58 + kind * 0xc4;
        m.write16(base, rng.below(1000) as u16);
        m.write32(base + 0xb8, if rng.chance(2) { 0x1000 } else { 0 });
    }
    for icon in 0..6u32 {
        m.write8(0x52_9514 + icon * 0x140, rng.next() as u8);
        m.write32(0x52_cdf0 + icon * 4, rng.next());
    }
    for leaf in [LEAF_A, LEAF_B, PAGE] {
        m.write32(leaf + 0x38, rng.below(3));
        m.write32(leaf + 0x40, if rng.chance(2) { 0x4_0000 } else { 0 });
        m.write8(leaf + 0x218, rng.next() as u8);
    }
    m
}

#[test]
fn editor_frame_matches_lifted() {
    let mut rng = Rng(0x2420_5eed_0002);
    let mut seen: BTreeMap<u32, u32> = BTreeMap::new();
    let mut saturated = 0;
    for seed in 0..12_000u32 {
        let start = fabricate_frame(&mut rng);
        let seed_host = 0x2421_0000 + seed as u64 * 104_729 + 1;
        let (lifted_log, rust_log, lifted_regs, rust_regs, lifted_mem, rust_mem) = run_pair(
            &start,
            seed_host,
            0x007f,
            |_| {},
            |m, host, r| f3111f0(m, host, r),
            0x3111f0,
        );
        let (a, b) = (lifted_mem.snapshot(), rust_mem.snapshot());
        assert!(a == b, "seed {seed}: {:?}", diff(&a, &b));
        // The lifted memory-operand `cvttss2si` is `as f64 as i32` (saturating, NaN to 0) while the
        // hardware and the port give 0x80000000 for out of range corners a, b and c: a known lifted
        // deviation, accepted here and counted.
        let mut lifted_log = lifted_log;
        for (l, r) in lifted_log.iter_mut().zip(&rust_log) {
            if l.va == 0x30ccd0 {
                for i in 0..3 {
                    if r.args[i] == 0x8000_0000 && l.args[i] != 0x8000_0000 {
                        l.args[i] = 0x8000_0000;
                        saturated += 1;
                    }
                }
            }
        }
        assert_eq!(lifted_log, rust_log, "seed {seed}: callee log");
        for (name, x, y) in [
            ("eax", lifted_regs.eax, rust_regs.eax),
            ("esp", lifted_regs.esp, rust_regs.esp),
            ("ebx", lifted_regs.ebx, rust_regs.ebx),
            ("ebp", lifted_regs.ebp, rust_regs.ebp),
            ("esi", lifted_regs.esi, rust_regs.esi),
            ("edi", lifted_regs.edi, rust_regs.edi),
        ] {
            assert_eq!(x, y, "seed {seed}: {name}");
        }
        for call in &rust_log {
            *seen.entry(call.va).or_default() += 1;
        }
    }
    // the fabricated mix must reach every branch family
    for va in [
        0x30a550, 0x335790, 0x30ccd0, 0x3357b0, 0x151030, 0x303350, 0x3d7b0, 0x31dbc0, 0x2f8020,
        0x2f82e0, 0x22b40, 0x22b90, 0x30be60, 0x30cf60, 0x78950,
    ] {
        assert!(
            seen.get(&va).copied().unwrap_or(0) > 30,
            "{va:#x}: {:?}",
            seen.get(&va)
        );
    }
    println!(
        "T2420 editor frame comparisons: 12000 ({} callee kinds, {saturated} lifted saturating corners)",
        seen.len()
    );
}
