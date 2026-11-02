// SPDX-License-Identifier: GPL-3.0-or-later
//! T1835 wave 11: the x87 model of the Rust ports and the first x87 roots (docs/t1835-rust-wave11.md).
//!
//! # Model decision: the T1510 `ClosureFp` widening contract, not 80 bits
//!
//! [`X87`] is the lifted code's own double-backed register file (`g_fp_stack[8]` of `f64`, `g_fp_top`,
//! `g_fp_control_word`, see `src/game/x87_replace.h` and `tsfp-ports::fpclosure::ClosureFp`) plus the
//! four condition-code bits C0..C3 of the status word that the lift does not keep. Every slot is
//! an `f64`. The original runs at 80 bits, so this model is exact only where the 80-bit result is
//! exactly an `f64` (precision limits, MEASURED against the host FPU by the `wave11` tests):
//!
//! * PC = 24 (control bits 8-9 zero): the arithmetic result is computed in `f64` and its significand
//!   is narrowed to 24 bits with the exponent range kept ([`round24`], the lifter's `recomp_fp_round24`).
//!   Exact against the hardware when both operands are `f32` values (53 >= 2 * 24 + 2, so the double
//!   rounding is innocuous for + - * /). With `f64` operands the double rounding differs from the
//!   hardware in a measurable fraction of the cases (recorded, not hidden).
//! * PC = 53 or 64 (control bits 8-9 non-zero): the result is the plain `f64` result. Exact for PC = 53
//!   when the hardware result stays inside the `f64` exponent range. For PC = 64 (the
//!   power-on value 0x37F) a hardware result needing more than 53 bits is **unrepresentable**.
//!   Whenever the hardware result *is* exactly an `f64` the model equals it (the model is the
//!   nearest `f64` to the exact result and the hardware 64-bit result lies within a quarter ulp of it).
//! * Exponent range: the hardware exponent is 15 bits, so overflow, underflow and `f64` denormals
//!   differ. Such hardware results have no `f64` form and are reported Unrepresented.
//! * Not modelled: tag word, stack overflow/underflow (depth), the sticky exception flags (IE, DE, ZE, OE,
//!   UE, PE, SF), C1, rounding control other than nearest, the instruction and data pointers.
//!
//! NaN handling has two selectable rules ([`NanRule`]). `Host` (what the C lift computes with SSE
//! doubles, the first NaN operand wins) is used by the roots' C-vs-Rust proof. `X87` (larger
//! significand wins, a signalling NaN is quieted, a tie prefers the positive sign) matches the
//! hardware and is used by the hardware validation.
//!
//! The `quirk` field injects one deliberate defect so the tests can show that wrong rounding, a wrong
//! stack order or wrong status bits are detected. It is [`Quirk::None`] in every port.
use crate::memory::GuestMemory;
use crate::fp_stack::{cdecl_arg as arg, Cpu};

/// Condition code bits of the status word: C0 = 0x100, C1 = 0x200, C2 = 0x400, C3 = 0x4000.
pub const C0: u16 = 0x0100;
pub const C2: u16 = 0x0400;
pub const C3: u16 = 0x4000;
/// C0, C1, C2, C3 and the TOP field: the part of the status word this model represents.
pub const STATUS_MODELLED: u16 = C0 | 0x0200 | C2 | C3 | 0x3800;

/// NaN selection rule of the arithmetic instructions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NanRule {
    /// Host `f64` arithmetic (the lifted C): the NaN of the first operand, else of the second.
    Host,
    /// Hardware: the NaN with the larger significand, quieted, ties go to the positive one.
    X87,
}

/// A deliberately wrong behaviour for the negative controls.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Quirk {
    None,
    /// Wrong rounding: PC = 24 results keep the full `f64` significand.
    NoRound24,
    /// Wrong rounding: results are narrowed to 24 bits whatever the precision control says.
    AlwaysRound24,
    /// Wrong rounding: narrowing truncates instead of rounding to nearest even.
    TruncateRound24,
    /// Wrong stack order: `fld` raises TOP instead of lowering it.
    PushUp,
    /// Wrong stack order: `fstp` does not pop.
    NoPop,
    /// Wrong stack order: `fxch` is a no-op.
    FxchNoop,
    /// Wrong operand order: the reversed forms (`fsubr`, `fdivr`, `fsubp`...) swap.
    SwapOperands,
    /// Wrong status bits: C0 and C3 exchanged.
    SwapC0C3,
    /// Wrong status bits: an unordered compare reports C3 only.
    UnorderedC3Only,
    /// Wrong status bits: `sahf` maps C2 to the sign flag instead of the parity flag.
    SahfParityLost,
}

/// Arithmetic operations of the `fadd`/`fsub`/`fmul`/`fdiv` families.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    Add,
    /// `a - b`
    Sub,
    /// `b - a`
    SubR,
    Mul,
    /// `a / b`
    Div,
    /// `b / a`
    DivR,
}

/// x87 register file of the double-backed model (see the module documentation).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct X87 {
    /// Physical slots. Slot `top` is ST(0).
    pub stack: [f64; 8],
    pub top: u8,
    /// Control word. Only bits 8-9 (precision control) change the arithmetic.
    pub control: u16,
    /// Condition codes C0..C3 (bits 8, 9, 10, 14) set by the compare family.
    pub cc: u16,
    pub nan: NanRule,
    pub quirk: Quirk,
}

/// The lifter's PC = 24 narrowing: the significand to 24 bits (nearest even), the `f64` exponent
/// range kept. Mirrors `x87_round24` of `src/game/x87_replace.h` including the denormal path.
pub fn round24(value: f64) -> f64 {
    let magnitude = value.abs();
    if (magnitude <= 3.402_823_466_385_288_6e38 && magnitude >= 1.175_494_350_822_287_5e-38)
        || magnitude == 0.0
        || value.is_nan()
        || magnitude == f64::INFINITY
    {
        return f64::from(value as f32);
    }
    let (mantissa, exponent) = frexp(value);
    ldexp(f64::from(mantissa as f32), exponent)
}

/// `frexp` for a finite non-zero `f64`: mantissa in [0.5, 1) (sign kept) and the exponent.
fn frexp(value: f64) -> (f64, i32) {
    let bits = value.to_bits();
    let raw = ((bits >> 52) & 0x7ff) as i32;
    if raw == 0 {
        // Denormal: scale into the normal range first (2^54 is exact).
        let (mantissa, exponent) = frexp(value * f64::from_bits(0x4350_0000_0000_0000));
        return (mantissa, exponent - 54);
    }
    (f64::from_bits((bits & !(0x7ff << 52)) | (1022 << 52)), raw - 1022)
}

/// `ldexp` with a single rounding of the final step (the intermediate scalings are exact).
fn ldexp(mantissa: f64, mut exponent: i32) -> f64 {
    let mut value = mantissa;
    while exponent > 1000 {
        value *= f64::from_bits(((1000 + 1023) as u64) << 52);
        exponent -= 1000;
    }
    while exponent < -1000 {
        value *= f64::from_bits(((-1000 + 1023) as u64) << 52);
        exponent += 1000;
    }
    value * f64::from_bits(((exponent + 1023) as u64) << 52)
}

/// A float widened to `f64` keeping a signalling NaN signalling (a memory operand of an arithmetic
/// instruction reaches the NaN selection unquieted, only loads quiet).
pub fn widen32_raw(bits: u32) -> f64 {
    if bits & 0x7fff_ffff > 0x7f80_0000 {
        return f64::from_bits((u64::from(bits >> 31) << 63) | (0x7ff << 52) | (u64::from(bits & 0x007f_ffff) << 29));
    }
    f64::from(f32::from_bits(bits))
}

/// `round24` with truncation instead of nearest even (the `TruncateRound24` quirk).
fn truncate24(value: f64) -> f64 {
    if !value.is_finite() || value == 0.0 {
        return value;
    }
    f64::from_bits(value.to_bits() & !((1u64 << 29) - 1))
}

fn quiet(bits: u64) -> u64 {
    bits | (1 << 51)
}

/// Hardware NaN pick (Intel SDM table 4-7) for two operands, at least one of them NaN.
fn pick_nan(first: f64, second: f64) -> f64 {
    let (a, b) = (first.to_bits(), second.to_bits());
    let (fa, fb) = (first.is_nan(), second.is_nan());
    let result = match (fa, fb) {
        (true, false) => quiet(a),
        (false, true) => quiet(b),
        _ => {
            // Both NaN: a quiet one beats a signalling one, else the larger significand, else the positive.
            let (qa, qb) = (a >> 51 & 1, b >> 51 & 1);
            let (ma, mb) = (a & ((1 << 52) - 1), b & ((1 << 52) - 1));
            if qa != qb {
                if qa == 1 { quiet(a) } else { quiet(b) }
            } else if ma > mb || (ma == mb && (a >> 63) == 0) {
                quiet(a)
            } else {
                quiet(b)
            }
        }
    };
    f64::from_bits(result)
}

impl X87 {
    /// A register file with the given contents and no condition codes.
    pub fn new(stack: [f64; 8], top: u8, control: u16) -> X87 {
        X87 { stack, top: top & 7, control, cc: 0, nan: NanRule::Host, quirk: Quirk::None }
    }

    /// ST(i) by logical index.
    pub fn st(&self, i: u8) -> f64 {
        self.stack[((self.top + i) & 7) as usize]
    }

    fn set_st(&mut self, i: u8, value: f64) {
        self.stack[((self.top + i) & 7) as usize] = value;
    }

    /// Status word: TOP field plus the condition codes.
    pub fn status(&self) -> u16 {
        (u16::from(self.top) << 11) | self.cc
    }

    fn push(&mut self, value: f64) {
        self.top = if self.quirk == Quirk::PushUp { (self.top + 1) & 7 } else { (self.top + 7) & 7 };
        self.stack[usize::from(self.top)] = value;
    }

    fn pop(&mut self) {
        if self.quirk != Quirk::NoPop {
            self.top = (self.top + 1) & 7;
        }
    }

    /// The significand narrowing after an arithmetic result.
    fn narrow(&self, value: f64) -> f64 {
        if self.nan == NanRule::X87 && value.is_nan() {
            return value;
        }
        match self.quirk {
            Quirk::NoRound24 => value,
            Quirk::AlwaysRound24 => round24(value),
            Quirk::TruncateRound24 if self.control & 0x300 == 0 => truncate24(value),
            _ if self.control & 0x300 == 0 => round24(value),
            _ => value,
        }
    }

    /// `fld m32`: the float widened exactly, pushed.
    pub fn fld32(&mut self, bits: u32) {
        // `f64::from(f32)` quiets a signalling NaN, as the hardware load does.
        self.push(f64::from(f32::from_bits(bits)));
    }

    /// `fld m64`.
    pub fn fld64(&mut self, bits: u64) {
        let value = f64::from_bits(bits);
        self.push(if value.is_nan() { f64::from_bits(quiet(bits)) } else { value });
    }

    /// `fld st(i)`.
    pub fn fld_st(&mut self, i: u8) {
        let value = self.st(i);
        self.push(value);
    }

    /// `fstp m32`: ST(0) narrowed to a float, popped.
    pub fn fstp32(&mut self) -> u32 {
        let bits = (self.st(0) as f32).to_bits();
        self.pop();
        bits
    }

    /// `fst m32`.
    pub fn fst32(&self) -> u32 {
        (self.st(0) as f32).to_bits()
    }

    /// `fstp m64`.
    pub fn fstp64(&mut self) -> u64 {
        let bits = self.st(0).to_bits();
        self.pop();
        bits
    }

    /// `fxch st(i)`.
    pub fn fxch(&mut self, i: u8) {
        if self.quirk == Quirk::FxchNoop {
            return;
        }
        let (a, b) = (self.st(0), self.st(i));
        self.set_st(0, b);
        self.set_st(i, a);
    }

    /// `fstp st(i)` (the CRT `fstp st(1)`): copies ST(0) to ST(i), pops.
    pub fn fstp_st(&mut self, i: u8) {
        let value = self.st(0);
        self.set_st(i, value);
        self.pop();
    }

    fn compute(&self, op: Op, a: f64, b: f64) -> f64 {
        let (a, b) = if self.quirk == Quirk::SwapOperands && matches!(op, Op::SubR | Op::DivR) { (b, a) } else { (a, b) };
        if a.is_nan() || b.is_nan() {
            return match self.nan {
                NanRule::X87 => pick_nan(a, b),
                // SSE `mulsd st0, m`: the destination (ST(0)) operand wins when both are NaN. The C
                // compiler may commute the multiplication, so the roots' proof is measured per level.
                NanRule::Host => if a.is_nan() { a } else { b },
            };
        }
        match op {
            Op::Add => a + b,
            Op::Sub => a - b,
            Op::SubR => b - a,
            Op::Mul => a * b,
            Op::Div => a / b,
            Op::DivR => b / a,
        }
    }

    /// `f<op> m32/m64/st(i)` with ST(0) as destination: ST(0) = ST(0) op `source` (`SubR` is `source - ST(0)`).
    pub fn arith0(&mut self, op: Op, source: f64) {
        let value = self.compute(op, self.st(0), source);
        let narrowed = self.narrow(value);
        self.set_st(0, narrowed);
    }

    /// `f<op>p st(i), st` and the non-popping `f<op> st(i), st`: ST(i) = ST(i) op ST(0)
    /// (`SubR`: ST(0) - ST(i)), then optionally pop. The `p` forms are `DE xx`.
    pub fn arith_i(&mut self, op: Op, i: u8, pop: bool) {
        let value = self.compute(op, self.st(i), self.st(0));
        let narrowed = self.narrow(value);
        self.set_st(i, narrowed);
        if pop {
            self.pop();
        }
    }

    /// `fmul m32`, the operation the roots use (alias of [`X87::arith0`]).
    pub fn fmul32(&mut self, bits: u32) {
        self.arith0(Op::Mul, f64::from(f32::from_bits(bits)));
    }

    /// Condition codes of comparing `a` with `b`: unordered 0b111, a < b C0, a == b C3, a > b none.
    fn compare(&mut self, a: f64, b: f64) {
        let (mut c0, mut c2, mut c3) = (false, false, false);
        if a.is_nan() || b.is_nan() {
            c0 = true;
            c2 = true;
            c3 = true;
            if self.quirk == Quirk::UnorderedC3Only {
                c0 = false;
                c2 = false;
            }
        } else if a < b {
            c0 = true;
        } else if a == b {
            c3 = true;
        }
        if self.quirk == Quirk::SwapC0C3 {
            core::mem::swap(&mut c0, &mut c3);
        }
        self.cc = (if c0 { C0 } else { 0 }) | (if c2 { C2 } else { 0 }) | (if c3 { C3 } else { 0 });
    }

    /// `fcom`/`fcomp` against `source`: ST(0) is compared, popped when `pop`.
    pub fn fcom(&mut self, source: f64, pop: bool) {
        self.compare(self.st(0), source);
        if pop {
            self.pop();
        }
    }

    /// `fcompp` (and `fucompp`): ST(0) against ST(1), both popped.
    pub fn fcompp(&mut self) {
        self.compare(self.st(0), self.st(1));
        self.pop();
        self.pop();
    }

    /// `fnstsw ax`.
    pub fn fnstsw(&self) -> u16 {
        self.status()
    }
}

/// The flags `sahf` loads from AH, then read back by `lahf`: SF, ZF, AF, PF, CF, bit 1 set.
pub fn sahf_flags(ah: u8, quirk: Quirk) -> u8 {
    let mut ah = ah;
    if quirk == Quirk::SahfParityLost {
        ah &= !0x04;
    }
    (ah & 0xd5) | 0x02
}

// ---- ports of the x87 roots (docs/t1835-rust-wave11.md) ----

const SCALE_ONE: u32 = 0x0047_8900;
const SCALE_DEFAULT: u32 = 0x0047_85cc;
const TABLE_BASE: u32 = 0x004f_9bac;
const ENTRY_SIZE: u32 = 0x29c;

fn r<M: GuestMemory>(m: &M, base: u32, off: u32) -> u32 {
    m.read32(base.wrapping_add(off))
}

fn w<M: GuestMemory>(m: &mut M, base: u32, off: u32, value: u32) {
    m.write32(base.wrapping_add(off), value);
}


/// Body of 000CB3B0 without the frame: EAX = object, ECX = [object+254], pushes the zoom float.
fn zoom_scale<M: GuestMemory>(m: &M, c: &mut Cpu, x: &mut X87, object: u32) {
    c.eax = object;
    c.ecx = r(m, object, 0x254);
    let plain = c.ecx == 0 && r(m, object, 0x258) == 1;
    x.fld32(m.read32(if plain { SCALE_ONE } else { SCALE_DEFAULT }));
}

/// 000CB3B0 cdecl(1). Pushes the camera zoom scale float on the x87 stack: constant 478900 when
/// [object+254] is 0 and [object+258] is 1, else 4785CC. Leaves EAX = object, ECX = [object+254].
pub fn camera_default_zoom_scale<M: GuestMemory>(m: &mut M, c: &mut Cpu, x: &mut X87) {
    let object = arg(m, c, 0);
    zoom_scale(m, c, x, object);
}

/// 001B7D00 body: three factors of the 0x29C-byte table entry times the object zoom scale into the
/// object's +38C/+390/+394. `[record+8]` is re-read where the original re-reads it.
fn scale_floats<M: GuestMemory>(m: &mut M, c: &mut Cpu, x: &mut X87, record: u32, index: u32) {
    c.eax = r(m, record, 8);
    if c.eax == 0 {
        return;
    }
    let entry = m.read32(TABLE_BASE).wrapping_add(index.wrapping_mul(ENTRY_SIZE));
    zoom_scale(m, c, x, c.eax);
    c.eax = r(m, record, 8);
    x.fmul32(r(m, entry, 0x5c));
    let bits = x.fstp32();
    w(m, c.eax, 0x38c, bits);
    c.ecx = r(m, record, 8);
    zoom_scale(m, c, x, c.ecx);
    x.fmul32(r(m, entry, 0x60));
    c.edx = r(m, record, 8);
    let bits = x.fstp32();
    w(m, c.edx, 0x390, bits);
    c.eax = r(m, record, 8);
    zoom_scale(m, c, x, c.eax);
    x.fmul32(r(m, entry, 0x64));
    c.ecx = r(m, record, 8);
    let bits = x.fstp32();
    w(m, c.ecx, 0x394, bits);
}

/// 001B7D00 cdecl(2) (record, table index).
pub fn object_scale_floats<M: GuestMemory>(m: &mut M, c: &mut Cpu, x: &mut X87) {
    let (record, index) = (arg(m, c, 0), arg(m, c, 1));
    scale_floats(m, c, x, record, index);
}

/// 001B7D70 cdecl(2) (record, other). Scales through 001B7D00, or (entry flag 0x40 set and
/// [other+38] != 1) stores one scaled value into Y, copies it to Z and then X.
pub fn object_scale_child_floats<M: GuestMemory>(m: &mut M, c: &mut Cpu, x: &mut X87) {
    let (record, other) = (arg(m, c, 0), arg(m, c, 1));
    c.ecx = r(m, record, 8);
    if c.ecx == 0 {
        return;
    }
    c.edx = other;
    c.eax = r(m, other, 0x30);
    if c.eax == 0xffff_ffff {
        return;
    }
    let index = c.eax;
    let flags = m.read8(m.read32(TABLE_BASE).wrapping_add(index.wrapping_mul(ENTRY_SIZE)).wrapping_add(0xc));
    if flags & 0x40 == 0 || r(m, other, 0x38) == 1 {
        scale_floats(m, c, x, record, index);
        return;
    }
    zoom_scale(m, c, x, c.ecx);
    c.eax = r(m, record, 8);
    let bits = x.fstp32();
    w(m, c.eax, 0x390, bits);
    c.eax = r(m, record, 8);
    c.ecx = r(m, c.eax, 0x390);
    w(m, c.eax, 0x394, c.ecx);
    let target = r(m, record, 8);
    c.edx = r(m, target, 0x394);
    w(m, target, 0x38c, c.edx);
}
