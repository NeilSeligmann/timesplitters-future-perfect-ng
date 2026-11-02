// SPDX-License-Identifier: GPL-3.0-or-later
//! Runtime support for the modules written by `python3 -m tools.c2rust` (T1878): sub-word and
//! 64-bit guest accesses, the x87 helpers of the lifted model (double-backed stack, 24-bit
//! precision control), SSE register helpers and the libm routines the lifted C calls.
//! Everything here mirrors a macro or helper of the lifted C runtime (`recomp_types.h`), named
//! after it, so a generated function reads like the original instruction sequence.
#![allow(dead_code, clippy::all)]
pub use crate::abi::{ExtRegisters, Fpu, GuestCall, Xmm};
pub use crate::memory::GuestMemory;

/// Edge coverage hook of the differential (a no-op unless built with `--cfg tsfp_cover`).
#[cfg(tsfp_cover)]
pub use crate::pose::cover;

// ---- guest memory -------------------------------------------------------------------------------

/// 16-bit store (the shared `GuestMemory` leaf trait has 8 and 32 bit stores).
pub fn write16<M: GuestMemory>(m: &mut M, address: u32, value: u16) {
    m.write8(address, value as u8);
    m.write8(address.wrapping_add(1), (value >> 8) as u8);
}

/// 64-bit load (little endian), used by the double-precision x87 forms.
pub fn read64<M: GuestMemory>(m: &M, address: u32) -> u64 {
    u64::from(m.read32(address)) | (u64::from(m.read32(address.wrapping_add(4))) << 32)
}

/// 64-bit store.
pub fn write64<M: GuestMemory>(m: &mut M, address: u32, value: u64) {
    m.write32(address, value as u32);
    m.write32(address.wrapping_add(4), (value >> 32) as u32);
}

/// x86 parity flag of the low byte (`true` when PF is set: an even number of one bits).
pub fn parity8(value: u32) -> bool {
    (value as u8).count_ones() % 2 == 0
}

// ---- libm (the lifted C calls the C library, so the Rust side calls the same routines) -----------

unsafe extern "C" {
    #[link_name = "sin"]
    fn c_sin(x: f64) -> f64;
    #[link_name = "cos"]
    fn c_cos(x: f64) -> f64;
    #[link_name = "tan"]
    fn c_tan(x: f64) -> f64;
    #[link_name = "atan2"]
    fn c_atan2(y: f64, x: f64) -> f64;
    #[link_name = "log2"]
    fn c_log2(x: f64) -> f64;
    #[link_name = "exp2"]
    fn c_exp2(x: f64) -> f64;
    #[link_name = "ldexp"]
    fn c_ldexp(x: f64, e: i32) -> f64;
    #[link_name = "frexp"]
    fn c_frexp(x: f64, e: *mut i32) -> f64;
    #[link_name = "floor"]
    fn c_floor(x: f64) -> f64;
    #[link_name = "ceil"]
    fn c_ceil(x: f64) -> f64;
    #[link_name = "trunc"]
    fn c_trunc(x: f64) -> f64;
    #[link_name = "fmod"]
    fn c_fmod(x: f64, y: f64) -> f64;
    #[link_name = "sqrt"]
    fn c_sqrt(x: f64) -> f64;
    #[link_name = "sqrtf"]
    fn c_sqrtf(x: f32) -> f32;
}

pub fn sin(x: f64) -> f64 {
    unsafe { c_sin(x) }
}
pub fn cos(x: f64) -> f64 {
    unsafe { c_cos(x) }
}
pub fn tan(x: f64) -> f64 {
    unsafe { c_tan(x) }
}
pub fn atan2(y: f64, x: f64) -> f64 {
    unsafe { c_atan2(y, x) }
}
pub fn log2(x: f64) -> f64 {
    unsafe { c_log2(x) }
}
pub fn exp2(x: f64) -> f64 {
    unsafe { c_exp2(x) }
}
pub fn ldexp(x: f64, e: i32) -> f64 {
    unsafe { c_ldexp(x, e) }
}
pub fn floor(x: f64) -> f64 {
    unsafe { c_floor(x) }
}
pub fn ceil(x: f64) -> f64 {
    unsafe { c_ceil(x) }
}
pub fn trunc(x: f64) -> f64 {
    unsafe { c_trunc(x) }
}
pub fn fmod(x: f64, y: f64) -> f64 {
    unsafe { c_fmod(x, y) }
}
pub fn sqrt(x: f64) -> f64 {
    unsafe { c_sqrt(x) }
}
pub fn sqrtf(x: f32) -> f32 {
    unsafe { c_sqrtf(x) }
}
pub fn fabs(x: f64) -> f64 {
    f64::from_bits(x.to_bits() & !(1 << 63))
}

// ---- x87 (double-backed model of the lifted C) ----------------------------------------------------

/// Extension of the shared `Fpu` for stack-relative stores.
pub trait FpuExt {
    /// `ST(i) = v` (the lifted `fp_top()` / `fp_st1()` / `g_fp_stack[..]` lvalues).
    fn set_st(&mut self, i: u32, v: f64);
}
impl FpuExt for Fpu {
    fn set_st(&mut self, i: u32, v: f64) {
        self.stack[((self.top + i) & 7) as usize] = v;
    }
}

/// `recomp_fp_round24`: round the significand to 24 bits, keep the exponent range.
fn round24(x: f64) -> f64 {
    let magnitude = fabs(x);
    if (magnitude <= 3.402_823_466_385_288_6e38 && magnitude >= 1.175_494_350_822_287_5e-38)
        || magnitude == 0.0
        || x != x
        || magnitude == f64::INFINITY
    {
        return x as f32 as f64;
    }
    let mut exponent = 0i32;
    let mantissa = unsafe { c_frexp(x, &mut exponent) };
    ldexp(mantissa as f32 as f64, exponent)
}

/// `RECOMP_FP_PC`: results are narrowed to 24 bits unless precision control selects wider.
pub fn fp_pc(fpu: &Fpu, x: f64) -> f64 {
    if fpu.control & 0x300 != 0 {
        x
    } else {
        round24(x)
    }
}

/// `RECOMP_FCMP`: -1 less, 0 equal, 1 greater, 2 unordered.
pub fn fcmp(a: f64, b: f64) -> i32 {
    if a != a || b != b {
        2
    } else if a < b {
        -1
    } else if a > b {
        1
    } else {
        0
    }
}

/// `RECOMP_FCMP_CC`: the C0/C2/C3 condition bits of a compare result.
pub fn fcmp_cc(c: i32) -> u32 {
    match c {
        2 => 0x4500,
        c if c < 0 => 0x0100,
        c if c > 0 => 0,
        _ => 0x4000,
    }
}

/// `recomp_fxam` condition bits (sign C1, class C0/C2/C3).
pub fn fxam(value: f64) -> u32 {
    let sign = if value.is_sign_negative() { 0x0200 } else { 0 };
    sign | if value.is_nan() {
        0x0100
    } else if value.is_infinite() {
        0x0500
    } else if value == 0.0 {
        0x4000
    } else {
        0x0400
    }
}

/// `recomp_frndint`: round to an integral value under the control word RC bits.
pub fn frndint(value: f64, control: u16) -> f64 {
    if !value.is_finite() {
        return value;
    }
    let rounded = match (control >> 10) & 3 {
        1 => floor(value),
        2 => ceil(value),
        3 => trunc(value),
        _ => {
            let low = floor(value);
            let fraction = value - low;
            if fraction > 0.5 || (fraction == 0.5 && fmod(low, 2.0) != 0.0) {
                low + 1.0
            } else {
                low
            }
        }
    };
    if rounded == 0.0 {
        if value.is_sign_negative() { -0.0 } else { 0.0 }
    } else {
        rounded
    }
}

/// `recomp_fist`: integer store under the control word rounding, indefinite value on overflow.
pub fn fist(value: f64, control: u16, bits: u32) -> i64 {
    let rounded = frndint(value, control);
    let limit = ldexp(1.0, bits as i32 - 1);
    if !rounded.is_finite() || rounded < -limit || rounded >= limit {
        return if bits == 64 {
            i64::MIN
        } else {
            -(1i64 << (bits - 1))
        };
    }
    rounded as i64
}

/// x86 `cvttss2si`: truncate, 0x8000_0000 (the integer indefinite) on NaN or overflow.
pub fn cvttss2si(value: f32) -> u32 {
    if value != value || value >= 2147483648.0 || value < -2147483648.0 {
        0x8000_0000
    } else {
        value as i32 as u32
    }
}

/// x86 `cvttsd2si`.
pub fn cvttsd2si(value: f64) -> u32 {
    if value != value || value >= 2147483648.0 || value < -2147483648.0 {
        0x8000_0000
    } else {
        value as i32 as u32
    }
}

// ---- SSE ----------------------------------------------------------------------------------------

/// Lane views of an `Xmm`.
pub trait XmmExt {
    fn f(&self, lane: usize) -> f32;
    fn set_f(&mut self, lane: usize, value: f32);
    fn d(&self, lane: usize) -> f64;
    fn set_d(&mut self, lane: usize, value: f64);
}
impl XmmExt for Xmm {
    fn f(&self, lane: usize) -> f32 {
        f32::from_bits(self.lanes[lane])
    }
    fn set_f(&mut self, lane: usize, value: f32) {
        self.lanes[lane] = value.to_bits();
    }
    fn d(&self, lane: usize) -> f64 {
        f64::from_bits(
            u64::from(self.lanes[lane * 2]) | (u64::from(self.lanes[lane * 2 + 1]) << 32),
        )
    }
    fn set_d(&mut self, lane: usize, value: f64) {
        let bits = value.to_bits();
        self.lanes[lane * 2] = bits as u32;
        self.lanes[lane * 2 + 1] = (bits >> 32) as u32;
    }
}

pub fn xmm_scalar_double(value: f64) -> Xmm {
    let mut r = Xmm::default();
    r.set_d(0, value);
    r
}

pub fn xmm_load<M: GuestMemory>(m: &M, address: u32) -> Xmm {
    Xmm {
        lanes: [
            m.read32(address),
            m.read32(address.wrapping_add(4)),
            m.read32(address.wrapping_add(8)),
            m.read32(address.wrapping_add(12)),
        ],
    }
}

pub fn xmm_store<M: GuestMemory>(m: &mut M, address: u32, value: Xmm) {
    for (i, lane) in value.lanes.into_iter().enumerate() {
        m.write32(address.wrapping_add(4 * i as u32), lane);
    }
}

/// `movlps` / `movhps` load: replace one 64-bit half.
pub fn xmm_load_half<M: GuestMemory>(m: &M, dst: &mut Xmm, address: u32, high: bool) {
    let base = if high { 2 } else { 0 };
    dst.lanes[base] = m.read32(address);
    dst.lanes[base + 1] = m.read32(address.wrapping_add(4));
}

/// `movlps` / `movhps` store.
pub fn xmm_store_half<M: GuestMemory>(m: &mut M, address: u32, src: Xmm, high: bool) {
    let base = if high { 2 } else { 0 };
    m.write32(address, src.lanes[base]);
    m.write32(address.wrapping_add(4), src.lanes[base + 1]);
}

fn lanewise(a: Xmm, b: Xmm, op: impl Fn(f32, f32) -> f32) -> Xmm {
    Xmm {
        lanes: core::array::from_fn(|i| {
            op(f32::from_bits(a.lanes[i]), f32::from_bits(b.lanes[i])).to_bits()
        }),
    }
}
fn bitwise(a: Xmm, b: Xmm, op: impl Fn(u32, u32) -> u32) -> Xmm {
    Xmm {
        lanes: core::array::from_fn(|i| op(a.lanes[i], b.lanes[i])),
    }
}
fn mask(c: bool) -> u32 {
    if c { u32::MAX } else { 0 }
}

pub fn xmm_add(a: Xmm, b: Xmm) -> Xmm {
    lanewise(a, b, |x, y| x + y)
}
pub fn xmm_sub(a: Xmm, b: Xmm) -> Xmm {
    lanewise(a, b, |x, y| x - y)
}
pub fn xmm_mul(a: Xmm, b: Xmm) -> Xmm {
    lanewise(a, b, |x, y| x * y)
}
pub fn xmm_div(a: Xmm, b: Xmm) -> Xmm {
    lanewise(a, b, |x, y| x / y)
}
/// MINPS/MAXPS return the second operand when the lanes are unordered or equal.
pub fn xmm_min(a: Xmm, b: Xmm) -> Xmm {
    lanewise(a, b, |x, y| if x < y { x } else { y })
}
pub fn xmm_max(a: Xmm, b: Xmm) -> Xmm {
    lanewise(a, b, |x, y| if x > y { x } else { y })
}
pub fn xmm_and(a: Xmm, b: Xmm) -> Xmm {
    bitwise(a, b, |x, y| x & y)
}
pub fn xmm_or(a: Xmm, b: Xmm) -> Xmm {
    bitwise(a, b, |x, y| x | y)
}
pub fn xmm_xor(a: Xmm, b: Xmm) -> Xmm {
    bitwise(a, b, |x, y| x ^ y)
}
/// ANDNPS is `!dst & src`.
pub fn xmm_andn(a: Xmm, b: Xmm) -> Xmm {
    bitwise(a, b, |x, y| !x & y)
}
pub fn xmm_cmp_eq(a: Xmm, b: Xmm) -> Xmm {
    lanewise_mask(a, b, |x, y| x == y)
}
pub fn xmm_cmp_lt(a: Xmm, b: Xmm) -> Xmm {
    lanewise_mask(a, b, |x, y| x < y)
}
pub fn xmm_cmp_le(a: Xmm, b: Xmm) -> Xmm {
    lanewise_mask(a, b, |x, y| x <= y)
}
pub fn xmm_cmp_neq(a: Xmm, b: Xmm) -> Xmm {
    lanewise_mask(a, b, |x, y| !(x == y))
}
fn lanewise_mask(a: Xmm, b: Xmm, op: impl Fn(f32, f32) -> bool) -> Xmm {
    Xmm {
        lanes: core::array::from_fn(|i| {
            mask(op(f32::from_bits(a.lanes[i]), f32::from_bits(b.lanes[i])))
        }),
    }
}
/// SSE compare by CMPPS immediate (0 EQ, 1 LT, 2 LE, 3 UNORD, 4 NEQ, 5 NLT, 6 NLE, 7 ORD).
pub fn xmm_cmp_pred(a: Xmm, b: Xmm, predicate: u32) -> Xmm {
    lanewise_mask(a, b, |x, y| match predicate & 7 {
        0 => x == y,
        1 => x < y,
        2 => x <= y,
        3 => x != x || y != y,
        4 => !(x == y),
        5 => !(x < y),
        6 => !(x <= y),
        _ => !(x != x || y != y),
    })
}
/// Packed unary ops take their operand from the second argument (the first is unused).
pub fn xmm_sqrt(_a: Xmm, b: Xmm) -> Xmm {
    lanewise(b, b, |x, _| sqrtf(x))
}
pub fn xmm_rsqrt(_a: Xmm, b: Xmm) -> Xmm {
    lanewise(b, b, |x, _| 1.0 / sqrtf(x))
}
pub fn xmm_rcp(_a: Xmm, b: Xmm) -> Xmm {
    lanewise(b, b, |x, _| 1.0 / x)
}
pub fn xmm_movemask(a: Xmm) -> u32 {
    (0..4).fold(0, |acc, i| acc | ((a.lanes[i] >> 31) << i))
}
/// `shufps`: lanes 0-1 from `a`, lanes 2-3 from `b`.
pub fn xmm_shuffle(a: Xmm, b: Xmm, imm: u32) -> Xmm {
    Xmm {
        lanes: [
            a.lanes[(imm & 3) as usize],
            a.lanes[((imm >> 2) & 3) as usize],
            b.lanes[((imm >> 4) & 3) as usize],
            b.lanes[((imm >> 6) & 3) as usize],
        ],
    }
}
pub fn xmm_unpack_low(a: Xmm, b: Xmm) -> Xmm {
    Xmm {
        lanes: [a.lanes[0], b.lanes[0], a.lanes[1], b.lanes[1]],
    }
}
pub fn xmm_unpack_high(a: Xmm, b: Xmm) -> Xmm {
    Xmm {
        lanes: [a.lanes[2], b.lanes[2], a.lanes[3], b.lanes[3]],
    }
}
/// `movlhps`: destination high half = source low half.
pub fn xmm_move_low_to_high(a: Xmm, b: Xmm) -> Xmm {
    Xmm {
        lanes: [a.lanes[0], a.lanes[1], b.lanes[0], b.lanes[1]],
    }
}
/// `movhlps`: destination low half = source high half.
pub fn xmm_move_high_to_low(a: Xmm, b: Xmm) -> Xmm {
    Xmm {
        lanes: [b.lanes[2], b.lanes[3], a.lanes[2], a.lanes[3]],
    }
}

// ---- partial registers ----------------------------------------------------------------------------

/// `mov al, v`: replace the low byte.
pub fn set_lo8(reg: u32, value: u32) -> u32 {
    (reg & 0xffff_ff00) | (value & 0xff)
}
/// `mov ah, v`: replace bits 8..16.
pub fn set_hi8(reg: u32, value: u32) -> u32 {
    (reg & 0xffff_00ff) | ((value & 0xff) << 8)
}
/// `mov ax, v`: replace the low word.
pub fn set_lo16(reg: u32, value: u32) -> u32 {
    (reg & 0xffff_0000) | (value & 0xffff)
}

/// Local extension over the existing ABI. The production GuestCall trampoline owns
/// the nested return slot; translated code pushes only arguments. No ABI file edits.
pub trait CoreRegisters {
    fn push<M: GuestMemory>(&mut self, m: &mut M, value: u32);
    fn call<C: GuestCall>(&mut self, guest: &mut C, target: u32);
    fn tail_call<C: GuestCall>(&mut self, guest: &mut C, target: u32);
}
impl CoreRegisters for ExtRegisters {
    fn push<M: GuestMemory>(&mut self, m: &mut M, value: u32) {
        self.esp = self.esp.wrapping_sub(4);
        m.write32(self.esp, value);
    }
    fn call<C: GuestCall>(&mut self, guest: &mut C, target: u32) {
        guest.call(target, self);
    }
    fn tail_call<C: GuestCall>(&mut self, guest: &mut C, target: u32) {
        // Skip the root's live return slot: the trampoline supplies the callee's slot.
        self.esp = self.esp.wrapping_add(4);
        guest.call(target, self);
        self.esp = self.esp.wrapping_sub(4);
    }
}
