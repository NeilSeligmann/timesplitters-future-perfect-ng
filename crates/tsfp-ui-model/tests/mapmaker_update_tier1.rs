//! T2429 Tier 1: the whole-function Rust port of the Map Maker editor update (0x306460) versus the lifted
//! translation (`editor_reference::f306460`). Memory, widget records, callee results and x87 stick values
//! are FABRICATED and seeded; the callees are a seeded mock that logs every call (address and argument
//! words). This is not real editor state (weak evidence, rule 12a). Compared: callee call log, guest memory
//! (outside the stack window), ESP and the callee-saved registers, FPU top.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use tsfp_ports::abi::{ExtRegisters, Fpu, GuestCall, Xmm};
use tsfp_ports::editor_next_reference::{f2fbf70, f2feb50};
use tsfp_ports::editor_reference::{f2f92e0, f2fc050, f62490, f306460};
use tsfp_ports::mapmaker_update::run_editor_update;
use tsfp_ports::memory::GuestMemory;

const STACK: u32 = 0x80_000;
const CTX: u32 = 0xa0_000;
const PAGE: u32 = 0xc2_000;
const LEAF_A: u32 = 0xc0_000;
const LEAF_B: u32 = 0xc1_000;
const RECORD_A: u32 = 0xb0_000;
const RECORD_B: u32 = 0xb1_000;
const OFFSETS: u32 = 0xb2_000;
const HINT: u32 = 0xb3_000;
const SPRITES: u32 = 0x529510;

thread_local! {
    /// Entries into the ported callees (not in the call log, the lifted shim runs their body).
    static PORTED_ENTRIES: RefCell<BTreeMap<u32, u32>> = RefCell::new(BTreeMap::new());
}

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

#[derive(Clone, Debug, PartialEq)]
struct Call {
    va: u32,
    args: Vec<u32>,
}

/// Argument words logged per callee (the stack words the lifted body pushes).
fn arg_count(va: u32) -> usize {
    match va {
        0x74840 | 0x42760 | 0x427b0 | 0x427f0 | 0x3004e0 | 0x77470 | 0x78d80 | 0x62490
        | 0x32f360 | 0x32f4c0 | 0x337760 | 0x331be0 | 0x331ab0 | 0x31ff30 | 0x2f79e0 | 0x308560
        | 0x22b40 | 0x32d1a0 | 0x32cfe0 => 1,
        0x335890 | 0x335900 | 0x309160 | 0x71140 | 0x42920 | 0x42970 | 0x22b90 | 0x32d440 => 2,
        0x79860 | 0x307e60 | 0x2f8450 => 3,
        0x2f8a20 => 6,
        _ => 0,
    }
}

/// The 6-byte cell structs: logged as two words, the upper half of the second one is stack garbage.
fn cell_words(va: u32) -> Option<usize> {
    match va {
        0x3348c0 => Some(0),
        0x2f8b40 => Some(1),
        _ => None,
    }
}

#[derive(Clone)]
struct Host {
    rng: Rc<RefCell<Rng>>,
    mem: Mem,
    calls: Rc<RefCell<Vec<Call>>>,
}

impl Host {
    fn word(&self, esp: u32, index: usize) -> u32 {
        self.mem.read32(esp.wrapping_add(4 * index as u32))
    }
}

impl GuestCall for Host {
    fn call(&mut self, address: u32, registers: &mut ExtRegisters) {
        // T2432: callees now ported to Rust run as the lifted body here, so their own callees are
        // logged on both sides and the entry itself is not.
        type Lifted = fn(&mut Mem, &mut Host, &mut ExtRegisters);
        let lifted: Option<Lifted> = match address {
            0x62490 => Some(f62490),
            0x2f92e0 => Some(f2f92e0),
            0x2feb50 => Some(f2feb50),
            0x2fc050 => Some(f2fc050),
            0x2fbf70 => Some(f2fbf70),
            _ => None,
        };
        if let Some(body) = lifted {
            PORTED_ENTRIES.with(|e| *e.borrow_mut().entry(address).or_default() += 1);
            // The lifted bodies expect a return address slot below the arguments.
            let mut shared = self.mem.clone();
            let mut inner = *registers;
            inner.esp = inner.esp.wrapping_sub(4);
            shared.write32(inner.esp, 0);
            body(&mut shared, self, &mut inner);
            inner.esp = registers.esp;
            *registers = inner;
            return;
        }
        let mut args: Vec<u32> = (0..arg_count(address))
            .map(|i| self.word(registers.esp, i))
            .collect();
        if let Some(head) = cell_words(address) {
            args = (0..head).map(|i| self.word(registers.esp, i)).collect();
            args.push(self.word(registers.esp, head));
            args.push(self.word(registers.esp, head + 1) & 0xffff);
        }
        self.calls.borrow_mut().push(Call {
            va: address,
            args: args.clone(),
        });
        let mut rng = self.rng.borrow_mut();
        let mut mem = self.mem.clone();
        match address {
            0x42760 => {
                registers.eax =
                    [0, 0x4000, 0x8000, 0x80, 0xc000, 0x7f, 0x8000_0000][rng.below(7) as usize]
            }
            0x427b0 => {
                registers.eax = [
                    0, 0, 0x1, 0x2, 0x100, 0x100, 0x100, 0x200, 0x400, 0x800, 0x201, 0xc00,
                ][rng.below(12) as usize]
            }
            0x427f0 => registers.eax = [0, 0x80, 0x7f, 0xff][rng.below(4) as usize],
            0x3004e0 => registers.eax = rng.below(2),
            0x77470 => {
                registers.eax = [0, LEAF_A, LEAF_B, PAGE, PAGE, PAGE, PAGE][rng.below(7) as usize]
            }
            0x774b0 => registers.eax = [0, LEAF_A, LEAF_B][rng.below(3) as usize],
            0x2f8de0 | 0x31ff30 => registers.eax = rng.below(2),
            0x314d60 => registers.eax = u32::from(rng.chance(4)),
            0x335890 => registers.eax = (rng.below(7) as i32 - 2) as u32,
            0x335900 => registers.eax = (rng.below(5) as i32 - 2) as u32,
            0x307e60 => registers.eax = [0, RECORD_A, RECORD_B][rng.below(3) as usize],
            0x2f8450 => {
                registers.eax = (rng.below(5) as i32 - 1) as u32;
                mem.write32(args[1], rng.next() % 5);
                mem.write32(args[2], rng.next() % 5);
            }
            0x2f8a20 => {
                mem.write32(args[5], (rng.below(5) as i32 - 2) as u32);
                mem.write32(args[4], rng.next() % 5);
                registers.eax = rng.below(2);
            }
            0x2f82e0 => registers.eax = [0, RECORD_A, RECORD_B][rng.below(3) as usize],
            0x2f8610 | 0x2f8b40 => registers.eax = (rng.below(5) as i32 - 1) as u32,
            0x3348c0 => registers.eax = (registers.eax & !0xff) | rng.below(2),
            0x2fc2a0 => registers.eax = rng.below(4),
            0x42970 => {
                let value = if rng.chance(4) {
                    0.0
                } else {
                    f64::from(rng.range(-1.0, 1.0))
                };
                registers.fpu.push(value);
            }
            0x22b90 | 0x22b40 => registers.eax = rng.below(300),
            _ => {
                registers.eax = rng.next();
                // Void callees sometimes change the editor flags like the real ones do.
                if rng.chance(6) {
                    let bit = [0x8, 0x20, 0x80, 0x2000, 0x4_0000, 0x200_0000, 0x2000_0000]
                        [rng.below(7) as usize];
                    let flags = mem.read32(CTX + 0x25c);
                    mem.write32(CTX + 0x25c, flags ^ bit);
                }
            }
        }
    }
}

fn regs() -> ExtRegisters {
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
            control: 0x027f,
        },
        xmm: [Xmm::default(); 5],
        xmm_hi: [Xmm::default(); 3],
    }
}

fn put_f32(m: &mut Mem, address: u32, value: f32) {
    m.write32(address, value.to_bits());
}

/// The constants the image holds (values printed from the ELF, `tmp/t2368-v4/tsfp-analysis-v4.elf`).
const CONSTANTS: [(u32, f32); 17] = [
    (0x475c78, 1.0),
    (0x475d18, 3.0),
    (0x49e084, -0.02),
    (0x4785d4, 0.02),
    (0x475d20, 1.5),
    (0x475d24, 0.1),
    (0x47d780, 1.02),
    (0x47d6dc, 0.98),
    (0x475cd4, 0.5),
    (0x475dc4, 0.0625),
    (0x478394, 0.016_666_668),
    (0x475d50, 0.125),
    (0x4760ac, 60.0),
    (0x47d1e4, 8.0),
    (0x475c88, 360.0),
    (0x47db00, 22.0),
    (0x4760cc, 5.0),
];

fn fabricate(rng: &mut Rng, mutated_constant: Option<usize>) -> Mem {
    let mut m = Mem::default();
    for (index, (address, value)) in CONSTANTS.iter().enumerate() {
        let value = if mutated_constant == Some(index) {
            *value * 1.37
        } else {
            *value
        };
        put_f32(&mut m, *address, value);
    }
    m.write32(0x7844a8, CTX);
    // Object list (T2432): 0 to 4 nodes linked through +0x40 with random flag bytes.
    let mut next = 0u32;
    for index in 0..rng.below(5) {
        let node = 0xe0_000 + index * 0x100;
        m.write32(node + 0x40, next);
        m.write8(node + 0x14, rng.next() as u8);
        next = node;
    }
    m.write32(0x78400c, next);
    m.write32(0x7842a8, PAGE);
    m.write32(0x778d34, HINT);
    m.write32(0x784014, 0xd0_000);
    m.write32(0x7ba9e0, rng.below(3));
    for view in 0..3u32 {
        let base = view * 0x1e0;
        m.write32(0x7ba26c + base, 1 + rng.below(3));
        put_f32(&mut m, 0x7ba280 + base, rng.range(-50.0, 700.0));
        put_f32(&mut m, 0x7ba284 + base, rng.range(-50.0, 500.0));
        m.write8(0x7ba260 + base, rng.below(4) as u8);
    }
    m.write32(0x783ee0, rng.below(40));
    m.write32(0x783dc4, rng.below(40));
    put_f32(&mut m, 0x783dd4, rng.range(0.2, 3.0));
    put_f32(&mut m, 0x783dd8, rng.range(-100.0, 100.0));
    put_f32(&mut m, 0x783f20, rng.range(0.2, 3.0));
    put_f32(&mut m, 0x783cc0, rng.range(-100.0, 100.0));
    let width = [640u32, 800, 1280][rng.below(3) as usize];
    // Scales near the zoom clamps (width/60 and width/8) so both clamp branches are exercised.
    let scale = match rng.below(3) {
        0 => width as f32 / 60.0 * rng.range(0.97, 1.03),
        1 => width as f32 / 8.0 * rng.range(0.97, 1.03),
        _ => rng.range(0.5, 60.0),
    };
    put_f32(&mut m, 0x783f2c, scale);
    put_f32(&mut m, 0x783f28, rng.range(0.5, 60.0));
    put_f32(&mut m, 0x783de0, rng.range(10.0, 500.0));
    put_f32(&mut m, 0x783ddc, rng.range(10.0, 500.0));
    m.write32(0x783de4, width);
    m.write32(0x783ccc, [480, 600, 720][rng.below(3) as usize]);
    m.write32(0x4e8cec, [60, 30, 50][rng.below(3) as usize]);
    put_f32(&mut m, 0x4e7948, rng.range(-2.0, 2.0));
    m.write32(0x4e8cf4, [640, 1280, 1920][rng.below(3) as usize]);
    m.write32(0x4e8cf0, [480, 720, 1080][rng.below(3) as usize]);
    put_f32(&mut m, 0x7ba038, rng.range(0.5, 2.5));
    put_f32(&mut m, 0x7ba188, rng.range(-4.0, 4.0));
    for index in 0..8u32 {
        put_f32(&mut m, 0x528c98 + index * 4, rng.range(-30.0, 400.0));
    }
    for kind in 0..4u32 {
        let value: u16 = if rng.chance(2) { 0x22 } else { 0x10 };
        let address = 0x524f5a + kind * 0xc4;
        m.write8(address, value as u8);
        m.write8(address + 1, (value >> 8) as u8);
    }
    for (record, base) in [(0, RECORD_A), (1, RECORD_B)] {
        let _ = record;
        m.write32(base + 0xc, rng.below(8));
        m.write32(base + 0x2c, SPRITES + 0x140 * rng.below(12));
        put_f32(&mut m, base + 8, rng.range(-3.0, 3.0));
        m.write32(base + 0x40, rng.below(2));
    }
    for leaf in [LEAF_A, LEAF_B, PAGE] {
        m.write32(leaf + 0xc4, rng.next() & 0x1000f0);
        m.write32(leaf + 0xc8, rng.next() & 0x100010);
        m.write8(leaf + 0x218, rng.below(8) as u8);
    }
    for word in 0..3u32 {
        put_f32(&mut m, OFFSETS + word * 4, rng.range(-3.0, 3.0));
    }
    // Hint block: last sprite index, last angle, last shown record.
    m.write32(HINT + 0x90, rng.below(8));
    put_f32(&mut m, HINT + 0x3c, rng.range(0.0, 360.0));
    m.write32(HINT + 0x8c, [0, RECORD_A, RECORD_B][rng.below(3) as usize]);

    let calm = rng.chance(2);
    let mut flags = 0u32;
    for (bit, one_in) in [
        (0x8u32, 14),
        (0x80, 3),
        (0x20, 5),
        (0x2000, 4),
        (0x4_0000, 14),
        (0x200_0000, 20),
        (0x2000_0000, 3),
        (0x800_0000, 12),
        (0x1000_0000, 16),
        (0x400_0000, 18),
        (0x800, 3),
    ] {
        if rng.chance(if calm { one_in * 12 } else { one_in }) {
            flags |= bit;
        }
    }
    m.write32(CTX + 0x25c, flags);
    m.write32(CTX + 0x200, rng.below(2));
    let mode_flags = if calm {
        0
    } else if rng.chance(10) {
        0x2
    } else if rng.chance(10) {
        0x2000
    } else {
        rng.next() & 0x4000
    };
    m.write32(CTX + 0x238, mode_flags);
    m.write32(CTX + 0x254, [0, 0, 1, 1, 2, 3, 4, 4][rng.below(8) as usize]);
    m.write32(CTX + 0x264, (rng.below(5) as i32 - 2) as u32);
    let fade = match rng.below(6) {
        0 => 0.0,
        1 => rng.range(0.0, 0.1),
        2 => -rng.range(0.0, 0.1),
        3 => f32::NAN,
        _ => rng.range(-3.0, 3.0),
    };
    put_f32(&mut m, CTX + 0x260, fade);
    let pick = |rng: &mut Rng, one: u32| [0, RECORD_A, RECORD_B][rng.below(one) as usize];
    m.write32(CTX + 0x2f18, pick(&mut *rng, 3));
    m.write32(CTX + 0x2f1c, [0, OFFSETS][rng.below(2) as usize]);
    m.write32(CTX + 0x2f70, pick(&mut *rng, 3));
    m.write32(CTX + 0x2f74, (rng.below(4) as i32 - 1) as u32);
    m.write32(CTX + 0x2f78, (rng.below(4) as i32 - 1) as u32);
    m.write32(
        CTX + 0x2f7c,
        if rng.chance(4) { pick(&mut *rng, 3) } else { 0 },
    );
    m.write32(CTX + 0x2f80, [0, OFFSETS][rng.below(2) as usize]);
    m.write32(CTX + 0x2f9c, rng.below(2));
    m.write32(
        CTX + 0x2f34,
        if rng.chance(2) {
            u32::MAX
        } else {
            rng.below(4)
        },
    );
    let cell: u16 = if rng.chance(2) {
        0xffff
    } else {
        rng.below(4) as u16
    };
    m.write8(CTX + 0x2f10, cell as u8);
    m.write8(CTX + 0x2f11, (cell >> 8) as u8);
    for (offset, bias) in [(0x2f28u32, 50i32), (0x2f2a, 50), (0x2f2c, 6)] {
        let value = (rng.below(bias as u32 * 2) as i32 - bias) as u16;
        m.write8(CTX + offset, value as u8);
        m.write8(CTX + offset + 1, (value >> 8) as u8);
    }
    m.write32(
        CTX + 0x1185c,
        [
            u32::MAX,
            u32::MAX,
            u32::MAX,
            u32::MAX,
            0,
            1,
            2,
            3,
            4,
            5,
            6,
            7,
        ][rng.below(12) as usize],
    );
    m.write8(CTX + 0x11854, [0, 0, 1, 2][rng.below(4) as usize]);
    // Idle editor (T2432): the cursor-action gate 0x2fbf70 is only true with no pick, cell, hover or substate.
    if rng.chance(4) {
        m.write32(CTX + 0x2f34, u32::MAX);
        m.write8(CTX + 0x2f10, 0xff);
        m.write8(CTX + 0x2f11, 0xff);
        m.write32(CTX + 0x2f7c, 0);
        m.write32(CTX + 0x1185c, u32::MAX);
        let flags = m.read32(CTX + 0x25c);
        m.write32(
            CTX + 0x25c,
            flags & !0x1600_4008 & !if rng.chance(2) { 0x20 } else { 0 },
        );
    }
    m
}

struct Outcome {
    log: Vec<Call>,
    regs: ExtRegisters,
    mem: Mem,
}

fn run_side(start: &Mem, host_seed: u64, lifted: bool) -> Outcome {
    let mem = start.deep_clone();
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut host = Host {
        rng: Rc::new(RefCell::new(Rng(host_seed))),
        mem: mem.clone(),
        calls: calls.clone(),
    };
    let mut r = regs();
    let mut working = mem.clone();
    if lifted {
        f306460(&mut working, &mut host, &mut r);
    } else {
        run_editor_update(&mut working, &mut host, &mut r);
    }
    let log = calls.borrow().clone();
    Outcome { log, regs: r, mem }
}

fn diff(a: &BTreeMap<u32, u8>, b: &BTreeMap<u32, u8>) -> Vec<String> {
    a.keys()
        .chain(b.keys())
        .filter(|address| a.get(address) != b.get(address))
        .map(|address| format!("{address:#x}: {:?} vs {:?}", a.get(address), b.get(address)))
        .take(8)
        .collect()
}

/// `None` when both sides agree, else a description of the first difference.
fn compare(start: &Mem, host_seed: u64) -> Option<String> {
    let lifted = run_side(start, host_seed, true);
    let rust = run_side(start, host_seed, false);
    for (index, (a, b)) in lifted.log.iter().zip(rust.log.iter()).enumerate() {
        if a != b {
            return Some(format!("call {index}: lifted {a:x?} vs rust {b:x?}"));
        }
    }
    if lifted.log.len() != rust.log.len() {
        let (longer, side) = if lifted.log.len() > rust.log.len() {
            (&lifted.log, "lifted")
        } else {
            (&rust.log, "rust")
        };
        let at = lifted.log.len().min(rust.log.len());
        return Some(format!(
            "log length {} vs {}; {side} continues with {:x?}",
            lifted.log.len(),
            rust.log.len(),
            longer[at]
        ));
    }
    let (a, b) = (lifted.mem.snapshot(), rust.mem.snapshot());
    if a != b {
        return Some(format!("memory {:?}", diff(&a, &b)));
    }
    for (name, x, y) in [
        ("esp", lifted.regs.esp, rust.regs.esp),
        ("ebx", lifted.regs.ebx, rust.regs.ebx),
        ("ebp", lifted.regs.ebp, rust.regs.ebp),
        ("esi", lifted.regs.esi, rust.regs.esi),
        ("edi", lifted.regs.edi, rust.regs.edi),
        ("fpu top", lifted.regs.fpu.top, rust.regs.fpu.top),
    ] {
        if x != y {
            return Some(format!("{name} {x:#x} vs {y:#x}"));
        }
    }
    None
}

const SEEDS: u32 = 6_000;

#[test]
fn editor_update_matches_lifted() {
    let mut rng = Rng(0x2429_5eed_0001);
    let mut reached = BTreeMap::<u32, u32>::new();
    let mut failures = Vec::new();
    for seed in 0..SEEDS {
        let start = fabricate(&mut rng, None);
        let host_seed = 0x2429_0000 + u64::from(seed) * 7919 + 1;
        if let Some(why) = compare(&start, host_seed) {
            failures.push(format!("seed {seed}: {why}"));
            if failures.len() >= 5 {
                break;
            }
        }
        PORTED_ENTRIES.with(|e| e.borrow_mut().clear());
        for call in run_side(&start, host_seed, true).log {
            *reached.entry(call.va).or_default() += 1;
        }
        PORTED_ENTRIES.with(|e| {
            for (va, count) in e.borrow().iter() {
                *reached.entry(*va).or_default() += count;
            }
        });
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    // Coverage: the weak fabricated seeds must reach the hard parts, else a pass means little.
    for (va, name) in [
        (0x71140, "zoom"),
        (0x309160, "scroll"),
        (0x79860, "tool mode change"),
        (0x2f8a20, "slot check"),
        (0x2f8b40, "link tool"),
        (0x3348c0, "cell validity"),
        (0x32d440, "hint panel"),
        (0x32cfe0, "hint name"),
        (0x3004e0, "modal"),
        (0x32f360, "alt view"),
        (0x2f9040, "tool 9040"),
        (0x2feb50, "tool eb50"),
        (0x2f92e0, "clear link marks"),
        (0x62490, "cursor mode global"),
        (0x2fc050, "hint refresh gate"),
        (0x2fbf70, "idle gate"),
        (0x314d60, "idle gate inner"),
        (0x3159c0, "nav helper"),
        (0x389b0, "quick action 200"),
        (0x32f4c0, "quick action 100"),
    ] {
        assert!(
            reached.get(&va).copied().unwrap_or(0) > 10,
            "{name} ({va:#x}) reached too rarely: {reached:?}"
        );
    }
    println!("T2429 editor update comparisons: {SEEDS}, reach {reached:x?}");
}

/// Mutation check: perturbing any image constant the Rust port hard-codes (the lifted body reads it
/// from the image) must make the differential fail, so a wrong literal cannot hide.
#[test]
fn constant_mutations_are_detected() {
    for index in 0..CONSTANTS.len() {
        let mut rng = Rng(0x2429_5eed_1000 + index as u64);
        let mut detected = false;
        for seed in 0..1_500u32 {
            let start = fabricate(&mut rng, Some(index));
            if compare(&start, 0x2429_8000 + u64::from(seed) * 31 + 1).is_some() {
                detected = true;
                break;
            }
        }
        let (address, _) = CONSTANTS[index];
        assert!(detected, "mutation of {address:#x} not detected");
    }
}
