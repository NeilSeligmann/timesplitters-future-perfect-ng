// SPDX-License-Identifier: GPL-3.0-or-later
//! T1835 wave 11: the x87 model (`tsfp_engine_physics::fp_x87::X87`) against the host FPU. The
//! reference is the real x87 instructions (`shim_hw_x87` in c/shim.c: `fninit`, `fldcw`, the program,
//! `fnstsw`, then `fstpt` of the remaining stack), so the comparison is 80-bit raw results versus
//! the double-backed model. A hardware result that is not exactly an `f64` is **Unrepresented**, it is
//! counted and never forced. See docs/t1835-rust-wave11.md for the measured limits.
use tsfp_engine_physics::fp_x87::{sahf_flags, widen32_raw, NanRule, Op, Quirk, C0, C2, C3, X87};

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct HwX87 {
    status: u16,
    flags: u8,
    depth: u8,
    st32: u32,
    pad: u32,
    st64: u64,
    raw: [[u8; 10]; 8],
}

unsafe extern "C" {
    fn shim_hw_x87(prog: i32, control: u32, a: u64, b: u64, c: u64, single: i32, out: *mut HwX87) -> i32;
}

const PROGRAMS: i32 = 22;
const CASES: usize = 30_000;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: u64) -> u64 {
        (self.next() >> 11) % n
    }
}

fn f32_operand(rng: &mut Rng) -> u64 {
    let sign = if rng.below(2) == 0 { 0 } else { 0x8000_0000u64 };
    sign | match rng.below(14) {
        0 => 0,
        1 => rng.next() & 0x007f_ffff,
        2 => 0x3f80_0000,
        3 => 0x7f80_0000,
        4 => 0x7fc0_0000 | (rng.next() & 0x003f_ffff),
        5 => 0x7f80_0000 | ((rng.next() & 0x003f_ffff) | 1),
        6 => 0x0080_0000,
        7 => 0x7f7f_ffff,
        8 => 0x3f80_0000 + (rng.next() & 0xff),
        9 => 0x4000_0000 + (rng.next() & 0x7f),
        _ => rng.next() & 0x7fff_ffff,
    }
}

fn f64_operand(rng: &mut Rng) -> u64 {
    let sign = if rng.below(2) == 0 { 0 } else { 1u64 << 63 };
    sign | match rng.below(14) {
        0 => 0,
        1 => rng.next() & ((1 << 52) - 1),
        2 => 0x3ff0_0000_0000_0000,
        3 => 0x7ff0_0000_0000_0000,
        4 => 0x7ff8_0000_0000_0000 | (rng.next() & ((1 << 51) - 1)),
        5 => 0x7ff0_0000_0000_0000 | ((rng.next() & ((1 << 51) - 1)) | 1),
        6 => 0x0010_0000_0000_0000,
        7 => 0x7fef_ffff_ffff_ffff,
        8 => 0x3ff0_0000_0000_0000 + (rng.next() & 0xff),
        9 => 0x4000_0000_0000_0000 + (rng.next() & 0x7f),
        10 => 0x3800_0000_0000_0000 + (rng.next() & 0x0fff_ffff_ffff_ffff),
        _ => rng.next() & 0x7fff_ffff_ffff_ffff,
    }
}

/// Model result of one program, same shape as the hardware one.
struct Model {
    status: u16,
    flags: u8,
    st32: u32,
    st64: u64,
    values: Vec<u64>,
}

fn run_model(prog: i32, control: u16, a: u64, b: u64, c: u64, single: bool, quirk: Quirk) -> Model {
    let mut x = X87::new([0.0; 8], 0, control);
    x.nan = NanRule::X87;
    x.quirk = quirk;
    let ld = |x: &mut X87, v: u64| if single { x.fld32(v as u32) } else { x.fld64(v) };
    let src = if single { widen32_raw(b as u32) } else { f64::from_bits(b) };
    let (mut flags, mut st32, mut st64, depth) = (0u8, 0u32, 0u64, [1usize, 1, 1, 1, 1, 1, 1, 0, 0, 2, 1, 1, 1, 1, 1, 1, 3, 1, 0, 1, 1, 3][prog as usize]);
    match prog {
        0..=5 => {
            ld(&mut x, a);
            let op = [Op::Add, Op::Sub, Op::SubR, Op::Mul, Op::Div, Op::DivR][prog as usize];
            x.arith0(op, src);
        }
        6 | 7 => {
            ld(&mut x, a);
            x.fcom(src, prog == 7);
        }
        8 | 18 => {
            ld(&mut x, a);
            ld(&mut x, b);
            x.fcompp();
        }
        9 => {
            ld(&mut x, a);
            ld(&mut x, b);
            x.fxch(1);
        }
        10..=15 => {
            ld(&mut x, a);
            ld(&mut x, b);
            let op = [Op::Sub, Op::Div, Op::Add, Op::Mul, Op::SubR, Op::DivR][(prog - 10) as usize];
            x.arith_i(op, 1, true);
        }
        16 => {
            ld(&mut x, a);
            ld(&mut x, b);
            ld(&mut x, c);
            x.fxch(2);
        }
        17 => {
            ld(&mut x, a);
            x.fcom(src, false);
            flags = sahf_flags((x.fnstsw() >> 8) as u8, quirk);
        }
        19 => {
            ld(&mut x, a);
            st32 = x.fst32();
        }
        20 => {
            ld(&mut x, a);
            st64 = x.st(0).to_bits();
        }
        _ => {
            ld(&mut x, a);
            ld(&mut x, b);
            x.fld_st(1);
        }
    }
    let status = x.status();
    let values = (0..depth).map(|_| x.fstp64()).collect();
    Model { status, flags, st32, st64, values }
}

/// Exact `f64` of a raw 80-bit value, `None` when it has no exact `f64` form (mirror of
/// `tsfp-ports::fpclosure::raw80_to_f64`).
fn raw80_to_f64(raw: &[u8; 10]) -> Option<u64> {
    let mant = u64::from_le_bytes(raw[..8].try_into().unwrap());
    let se = u16::from_le_bytes([raw[8], raw[9]]);
    let sign = u64::from(se >> 15);
    let exp = i64::from(se & 0x7fff);
    if exp == 0x7fff {
        if mant >> 63 == 0 || mant & 0x7ff != 0 {
            return None;
        }
        return Some((sign << 63) | (0x7ff << 52) | ((mant >> 11) & ((1 << 52) - 1)));
    }
    if exp == 0 {
        return if mant == 0 { Some(sign << 63) } else { None };
    }
    if mant >> 63 == 0 {
        return None;
    }
    let unbiased = exp - 16383;
    if unbiased > 1023 {
        return None;
    }
    if unbiased >= -1022 {
        if mant & 0x7ff != 0 {
            return None;
        }
        return Some((sign << 63) | (((unbiased + 1023) as u64) << 52) | ((mant >> 11) & ((1 << 52) - 1)));
    }
    let shift = 11 + (-1022 - unbiased);
    if shift >= 64 || mant & ((1u64 << shift) - 1) != 0 {
        return None;
    }
    Some((sign << 63) | (mant >> shift))
}

#[derive(Default, Debug, Clone, Copy)]
struct Tally {
    matched: usize,
    differ: usize,
    unrepresented: usize,
    first_differ: Option<(i32, u64, u64, u64)>,
}

impl Tally {
    fn add(&mut self, other: Tally) {
        self.matched += other.matched;
        self.differ += other.differ;
        self.unrepresented += other.unrepresented;
        self.first_differ = self.first_differ.or(other.first_differ);
    }
}

/// Runs `cases` random operand sets of program `prog` and tallies the verdicts: Differ if any
/// compared field differs, Unrepresented if the hardware raw values have no `f64` form (and
/// nothing else differs), Match otherwise.
fn compare(prog: i32, control: u16, single: bool, quirk: Quirk, seed: u64, cases: usize) -> Tally {
    let mut rng = Rng(seed);
    let mut t = Tally::default();
    for _ in 0..cases {
        let operand = |rng: &mut Rng| if single { f32_operand(rng) } else { f64_operand(rng) };
        let a = operand(&mut rng);
        let b = if rng.below(6) == 0 { a } else { operand(&mut rng) };
        let c = operand(&mut rng);
        let mut hw = HwX87::default();
        assert_eq!(unsafe { shim_hw_x87(prog, u32::from(control), a, b, c, i32::from(single), &mut hw) }, 1);
        let model = run_model(prog, control, a, b, c, single, quirk);
        let cc_mask = if matches!(prog, 6 | 7 | 8 | 17 | 18) { 0x3800 | C0 | C2 | C3 } else { 0x3800 };
        let mut differ = hw.status & cc_mask != model.status & cc_mask;
        if prog == 17 {
            differ |= hw.flags != model.flags;
        }
        if prog == 19 {
            differ |= hw.st32 != model.st32;
        }
        if prog == 20 {
            differ |= hw.st64 != model.st64;
        }
        let mut unrep = false;
        for (i, expected) in model.values.iter().enumerate() {
            match raw80_to_f64(&hw.raw[i]) {
                Some(bits) => differ |= bits != *expected,
                None => unrep = true,
            }
        }
        if differ {
            t.differ += 1;
            t.first_differ.get_or_insert((prog, a, b, c));
        } else if unrep {
            t.unrepresented += 1;
        } else {
            t.matched += 1;
        }
    }
    t
}

fn sweep(control: u16, single: bool, quirk: Quirk, seed: u64) -> Tally {
    let mut total = Tally::default();
    for prog in 0..PROGRAMS {
        total.add(compare(prog, control, single, quirk, seed ^ (prog as u64 * 0x9e37_79b9), CASES));
    }
    total
}

const PC24: u16 = 0x007f;
const PC53: u16 = 0x027f;
const PC64: u16 = 0x037f;

#[test]
fn pc24_with_float_operands_equals_the_hardware() {
    let t = sweep(PC24, true, Quirk::None, 0x1101);
    eprintln!("pc24/f32: {t:?}");
    assert_eq!(t.differ, 0, "{:?}", t.first_differ);
    assert!(t.matched > 20 * CASES, "too few comparable cases");
}

#[test]
fn pc53_with_double_operands_equals_the_hardware_inside_the_f64_range() {
    let t = sweep(PC53, false, Quirk::None, 0x1102);
    eprintln!("pc53/f64: {t:?}");
    assert_eq!(t.differ, 0, "{:?}", t.first_differ);
    assert!(t.matched > 15 * CASES);
}

#[test]
fn pc64_is_equal_where_representable_and_unrepresented_otherwise() {
    let t = sweep(PC64, false, Quirk::None, 0x1103);
    eprintln!("pc64/f64: {t:?}");
    assert_eq!(t.differ, 0, "{:?}", t.first_differ);
    assert!(t.unrepresented > 0, "PC 64 must produce results with no f64 form");
}

#[test]
fn pc24_with_double_operands_double_rounds_measurably() {
    // The documented precision limit: round-to-24 of a rounded-to-53 result is not the single
    // rounding of the hardware. The measured rate is recorded in docs/t1835-rust-wave11.md.
    let t = sweep(PC24, false, Quirk::None, 0x1104);
    eprintln!("pc24/f64: {t:?}");
    assert!(t.differ > 0, "expected the double rounding limit to show");
}

/// Each quirk must make at least one comparison differ (wrong rounding, stack order, status bits).
#[test]
fn every_quirk_is_detected() {
    let quirks = [
        Quirk::NoRound24,
        Quirk::AlwaysRound24,
        Quirk::TruncateRound24,
        Quirk::PushUp,
        Quirk::NoPop,
        Quirk::FxchNoop,
        Quirk::SwapOperands,
        Quirk::SwapC0C3,
        Quirk::UnorderedC3Only,
        Quirk::SahfParityLost,
    ];
    for quirk in quirks {
        let mut differ = 0;
        for (control, single) in [(PC24, true), (PC53, false)] {
            for prog in 0..PROGRAMS {
                differ += compare(prog, control, single, quirk, 0x1105 + prog as u64, 2_000).differ;
            }
        }
        eprintln!("quirk {quirk:?}: {differ} differing cases");
        assert!(differ > 0, "{quirk:?} was not detected");
    }
}

#[test]
fn sahf_residue_carries_top_bit_one_into_the_auxiliary_flag() {
    // Program 17 starts at TOP = 0 and ends at TOP = 7 (one fld), whose bit 1 reaches AF (0x10).
    let mut hw = HwX87::default();
    assert_eq!(unsafe { shim_hw_x87(17, 0x27f, 1.0f64.to_bits(), 2.0f64.to_bits(), 0, 0, &mut hw) }, 1);
    assert_eq!(hw.flags & 0x10, 0x10);
    let m = run_model(17, 0x27f, 1.0f64.to_bits(), 2.0f64.to_bits(), 0, false, Quirk::None);
    assert_eq!(m.flags, hw.flags);
    assert_eq!(m.flags & 0x01, 0x01, "1.0 < 2.0 sets C0 = CF");
}

