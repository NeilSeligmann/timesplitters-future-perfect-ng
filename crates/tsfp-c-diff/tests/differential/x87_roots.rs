// SPDX-License-Identifier: GPL-3.0-or-later
//! T1835 x87 roots (000CB3B0, 001B7D00, 001B7D70): the Rust ports of
//! `tsfp-engine-physics/src/fp_x87.rs` against the unmodified C replacements of
//! `src/game/x87_roots.inc`, compiled with the production (double-backed) backend of
//! `x87_replace.h`. The driver compares, on top of the registers and memory of wave 9, the whole x87
//! state the lift keeps: all eight physical slots by bit pattern, TOP and the control word, in and
//! out. Negative controls: a perturbation of every kind (registers, memory, one slot bit, TOP,
//! the control word) and five model quirks (wrong rounding three ways, wrong stack order two ways)
//! that the roots' own arithmetic must expose. See docs/t1835-rust-x87.md.
use super::wave9::{POOL_A, POOL_B, POOL_C, S9, float_bits, s9};
use super::*;
use tsfp_engine_physics::{
    fp_stack::Cpu,
    fp_x87::{self as x87, Quirk, X87},
};

unsafe extern "C" {
    fn shim_run(va: u32, regs: *mut u32) -> i32;
    fn shim_x87_set(stack: *const u64, top: u32, control: u32);
    fn shim_x87_get(stack: *mut u64, top: *mut u32, control: *mut u32);
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Mode {
    Equal,
    Perturb,
    Quirk(Quirk),
}

pub(super) struct Stats {
    pub(super) detected: usize,
    pub(super) total: usize,
    /// Cases whose C run left a non-trivial x87 state (TOP moved or a slot changed).
    pub(super) x87_active: usize,
}

const CONTROLS: [u16; 6] = [0x027f, 0x037f, 0x007f, 0x0000, 0x017f, 0x1f7f];

fn initial_stack(rng: &mut Rng) -> [u64; 8] {
    let mut stack = [0u64; 8];
    for slot in &mut stack {
        *slot = match rng.below(3) {
            0 => f64::from(f32::from_bits(float_bits(rng))).to_bits(),
            1 => (u64::from(rng.next()) << 32) | u64::from(rng.next()),
            _ => 0,
        };
    }
    stack
}

pub(super) fn x87_drive(
    seed: u64,
    mode: Mode,
    mut build: impl FnMut(&mut Rng, &mut Pair) -> S9,
    port: impl Fn(&mut View, &mut Cpu, &mut X87),
) -> Stats {
    let mut guard = pair().lock().unwrap_or_else(|e| e.into_inner());
    let pair = &mut *guard;
    let mut rng = Rng(seed);
    let mut stats = Stats { detected: 0, total: 0, x87_active: 0 };
    for index in 0..CASES {
        let mut s = build(&mut rng, pair);
        let logged: Vec<u32> = pair.log.drain(..).collect();
        let slots = 4 * s.case.args.len().max(1) as u32;
        s.case.watch.insert(0, (STACK - 64, 64 + 4 + slots));
        s.case.watch.extend(logged.into_iter().map(|address| (address, 4)));
        pair.put32(STACK, 0);
        for (i, arg) in s.case.args.iter().enumerate() {
            pair.put32(STACK + 4 + 4 * i as u32, *arg);
        }
        pair.log.clear();
        let stack0 = initial_stack(&mut rng);
        let top0 = rng.below(8);
        let control0 = u32::from(CONTROLS[rng.below(CONTROLS.len() as u32) as usize]);
        let case = &s.case;
        let mut regs = [case.regs[0], case.regs[1], case.regs[2], s.extra[0], case.esi, s.extra[1], s.extra[2], STACK];
        unsafe { shim_x87_set(stack0.as_ptr(), top0, control0) };
        assert_eq!(unsafe { shim_run(case.va, regs.as_mut_ptr()) }, 1, "unknown root");
        let (mut c_stack, mut c_top, mut c_control) = ([0u64; 8], 0u32, 0u32);
        unsafe { shim_x87_get(c_stack.as_mut_ptr(), &mut c_top, &mut c_control) };
        stats.x87_active += usize::from(c_top != top0 || c_stack != stack0);

        let mut view = View(pair.rust);
        let mut cpu = Cpu::entry(STACK, case.regs[0], case.regs[1], case.regs[2], case.esi, [s.extra[0], s.extra[1], s.extra[2], 0]);
        let mut x = X87::new(stack0.map(f64::from_bits), top0 as u8, control0 as u16);
        if let Mode::Quirk(quirk) = mode {
            x.quirk = quirk;
        }
        port(&mut view, &mut cpu, &mut x);
        if mode == Mode::Perturb {
            match index % 10 {
                0 => cpu.eax ^= 1,
                1 => cpu.ecx ^= 1,
                2 => cpu.edx ^= 1,
                3 => {
                    let (start, _) = case.watch[0];
                    let byte = view.byte(start);
                    view.set(start, byte ^ 0x80);
                }
                4 => cpu.esi ^= 1 << (rng.next() % 32),
                5 => match rng.next() % 3 {
                    0 => cpu.ebx ^= 1,
                    1 => cpu.edi ^= 1,
                    _ => cpu.ebp ^= 1,
                },
                6 => cpu.esp = cpu.esp.wrapping_add(4),
                7 => {
                    let slot = (rng.next() % 8) as usize;
                    x.stack[slot] = f64::from_bits(x.stack[slot].to_bits() ^ (1 << (rng.next() % 64)));
                }
                8 => x.top = (x.top + 1) & 7,
                _ => x.control ^= 0x0100,
            }
        }
        let x_stack = x.stack.map(f64::to_bits);
        let mut mismatch = lifted_policy::mismatch(
            case.va,
            u8::from(regs[0] != cpu.eax)
                | u8::from(regs[1] != cpu.ecx) << 1
                | u8::from(regs[2] != cpu.edx) << 2
                | u8::from(regs[3] != cpu.ebx) << 3
                | u8::from(regs[4] != cpu.esi) << 4
                | u8::from(regs[5] != cpu.edi) << 5
                | u8::from(regs[6] != cpu.ebp) << 6
                | u8::from(regs[7] != cpu.esp.wrapping_add(4)) << 7,
        )
            || c_stack != x_stack
            || c_top != u32::from(x.top)
            || c_control != u32::from(x.control);
        for (start, len) in &case.watch {
            for offset in 0..*len {
                let address = start.wrapping_add(offset);
                mismatch |= lifted_policy::mem(case.va, address, pair.get(false, address) != pair.get(true, address));
            }
        }
        if mode != Mode::Equal {
            stats.total += 1;
            stats.detected += usize::from(mismatch);
            // Equal the arenas again so the next case starts from the same memory.
            for (start, len) in &case.watch {
                for offset in 0..*len {
                    let address = start.wrapping_add(offset);
                    let byte = pair.get(false, address);
                    unsafe { *pair.rust.add(address as usize) = byte };
                }
            }
        } else {
            assert!(
                !mismatch,
                "root {:08X} case {index}: C regs {regs:x?} x87 {c_stack:x?} top {c_top} cw {c_control:x}, Rust {cpu:x?} x87 {x_stack:x?} top {} cw {:x}, args {:x?}",
                case.va, x.top, x.control, case.args
            );
        }
    }
    stats
}

/// Quirks a root with an fmul and an fstp must expose.
const ARITH: [Quirk; 5] = [Quirk::NoRound24, Quirk::AlwaysRound24, Quirk::TruncateRound24, Quirk::PushUp, Quirk::NoPop];

macro_rules! x87_suite {
    ($name:ident, $mutant:ident, $quirks:ident, $list:expr, $seed:expr, $build:expr, $port:expr) => {
        #[test]
        fn $name() {
            let stats = x87_drive($seed, Mode::Equal, $build, $port);
            eprintln!("{}: {} cases changed the x87 state", stringify!($name), stats.x87_active);
            assert!(stats.x87_active > CASES / 4, "the root hardly touched the x87 stack");
        }
        #[test]
        fn $mutant() {
            let stats = x87_drive($seed, Mode::Perturb, $build, $port);
            eprintln!("mutant control {}: {}/{} perturbations detected", stringify!($name), stats.detected, stats.total);
            assert!(stats.total > 0);
            assert_eq!(stats.detected, stats.total);
        }
        #[test]
        fn $quirks() {
            for quirk in $list {
                let stats = x87_drive($seed, Mode::Quirk(quirk), $build, $port);
                eprintln!("quirk {quirk:?} {}: {}/{} cases detected", stringify!($name), stats.detected, stats.total);
                assert!(stats.detected > 0, "{quirk:?} went undetected");
            }
        }
    };
}

/// A NaN float word (quiet or signalling, any sign).
fn is_nan32(bits: u32) -> bool {
    bits & 0x7fff_ffff > 0x7f80_0000
}

/// A float that is not a NaN. NaN times NaN is excluded from the proof: the C compiler may commute
/// `st0 * m32`, so which NaN `mulsd` returns depends on the optimisation level (O0 returns ST(0), O2
/// and O3 may return the memory operand, MEASURED). A NaN times a number is compared, as is a number
/// times a number. docs/t1835-rust-x87.md, finding 4.
fn plain_float(rng: &mut Rng) -> u32 {
    quiet_float(loop {
        let bits = float_bits(rng);
        if !is_nan32(bits) {
            break bits;
        }
    })
}

/// Signalling NaN floats are excluded: the C replacement's `(float)(double)f` round trip is folded to
/// the identity at O2 and O3 (an SNaN word survives) but quieted at O0 and by the hardware `fld`
/// (MEASURED, case 61661 of 001B7D70). The Rust model quiets, like the hardware.
/// docs/t1835-rust-x87.md, finding 5.
fn quiet_float(bits: u32) -> u32 {
    if is_nan32(bits) { bits | 0x0040_0000 } else { bits }
}

thread_local! {
    /// Whether the constants of the current case may be NaN (then the factors never are).
    static CONSTANTS_NAN: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Constants shared by all three roots. They are NaN in one case of eight, the table factors then are not.
fn zoom_constants(rng: &mut Rng, pair: &mut Pair) {
    let nan = rng.below(8) == 0;
    CONSTANTS_NAN.set(nan);
    for address in [0x0047_8900, 0x0047_85cc] {
        pair.put32(address, if nan { quiet_float(float_bits(rng)) } else { plain_float(rng) });
    }
}

/// An object with the zoom fields: +254 flag (mostly 0), +258 mode (mostly 1), the three outputs scribbled.
fn zoom_object(rng: &mut Rng, pair: &mut Pair, object: u32) {
    pair.put32(object + 0x254, if rng.below(3) == 0 { rng.next() } else { 0 });
    pair.put32(object + 0x258, if rng.below(3) == 0 { rng.below(4) } else { 1 });
    for offset in [0x38c, 0x390, 0x394] {
        pair.put32(object + offset, rng.next() | 1);
    }
}

fn region(rng: &mut Rng, pool: u32, k: u32) -> u32 {
    pool + k * 0x2000 + rng.below(0x40) * 4
}

/// A table entry of 0x29C bytes at `[4F9BAC] + index * 0x29C`, factors +5C +60 +64, flag byte +0C.
fn entry_table(rng: &mut Rng, pair: &mut Pair, indices: u32, flag_dword: bool) {
    let table = region(rng, POOL_C, 1);
    pair.put32(0x004f_9bac, table);
    for index in 0..indices {
        let entry = table + index * 0x29c;
        for offset in [0x5c, 0x60, 0x64] {
            pair.put32(entry + offset, if CONSTANTS_NAN.get() { plain_float(rng) } else { quiet_float(float_bits(rng)) });
        }
        let flags = if flag_dword && rng.below(2) == 0 { rng.next() | 0x40 } else { rng.next() & !0x40 };
        pair.put32(entry + 0xc, flags);
    }
}

fn build_cb3b0(rng: &mut Rng, pair: &mut Pair) -> S9 {
    zoom_constants(rng, pair);
    let object = region(rng, POOL_A, 0);
    zoom_object(rng, pair, object);
    let mut s = s9(rng, 0xcb3b0, vec![object], false);
    s.case.watch.push((object + 0x254, 8));
    s
}

/// Where the record's object pointer points: a separate object, null, or an alias that makes a store
/// of the root land on `[record+8]` itself (the original re-reads it after every store).
fn record_object(rng: &mut Rng, pair: &mut Pair, record: u32) -> u32 {
    let object = match rng.below(10) {
        0 => 0,
        1 => record.wrapping_add(8).wrapping_sub(0x38c),
        2 => record.wrapping_add(8).wrapping_sub(0x390),
        3 => record.wrapping_add(8).wrapping_sub(0x394),
        4 => record.wrapping_sub(0x254) & !3,
        _ => region(rng, POOL_B, 0),
    };
    pair.put32(record + 8, object);
    if object != 0 {
        zoom_object(rng, pair, object);
        // The aliasing placements above clobber their own field writes, which is the point.
    }
    object
}

fn build_1b7d00(rng: &mut Rng, pair: &mut Pair) -> S9 {
    zoom_constants(rng, pair);
    entry_table(rng, pair, 4, false);
    let record = region(rng, POOL_A, 0);
    let object = record_object(rng, pair, record);
    let index = rng.below(4);
    let mut s = s9(rng, 0x1b7d00, vec![record, index], false);
    s.case.watch.push((record + 8, 4));
    if object != 0 {
        s.case.watch.push((object.wrapping_add(0x38c), 12));
    }
    s
}

fn build_1b7d70(rng: &mut Rng, pair: &mut Pair) -> S9 {
    zoom_constants(rng, pair);
    entry_table(rng, pair, 4, true);
    let record = region(rng, POOL_A, 0);
    let object = record_object(rng, pair, record);
    let other = region(rng, POOL_C, 4);
    pair.put32(other + 0x30, if rng.below(6) == 0 { 0xffff_ffff } else { rng.below(4) });
    pair.put32(other + 0x38, if rng.below(2) == 0 { 1 } else { rng.below(3) });
    let mut s = s9(rng, 0x1b7d70, vec![record, other], false);
    s.case.watch.push((record + 8, 4));
    if object != 0 {
        s.case.watch.push((object.wrapping_add(0x38c), 12));
    }
    s
}

x87_suite!(x87_r_000cb3b0, x87_mutant_000cb3b0, x87_quirks_000cb3b0, [Quirk::PushUp], 0x1835_b001, build_cb3b0, |m, c, x| {
    x87::camera_default_zoom_scale(m, c, x)
});
x87_suite!(x87_r_001b7d00, x87_mutant_001b7d00, x87_quirks_001b7d00, ARITH, 0x1835_b002, build_1b7d00, |m, c, x| {
    x87::object_scale_floats(m, c, x)
});
x87_suite!(x87_r_001b7d70, x87_mutant_001b7d70, x87_quirks_001b7d70, ARITH, 0x1835_b003, build_1b7d70, |m, c, x| {
    x87::object_scale_child_floats(m, c, x)
});
