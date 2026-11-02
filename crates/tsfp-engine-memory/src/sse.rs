// SPDX-License-Identifier: GPL-3.0-or-later
//! T1836 wave 8: portable model of the scalar SSE subset the proven FP replacements use
//! (addss, subss, mulss, divss, comiss/ucomiss, cvtsi2ss, cvttss2si). Included by path into the
//! ports crates, like `gprs.rs`, so no crate gains a dependency. Operates on raw bit patterns.
//!
//! Contract (MXCSR control 0x1F80: round to nearest, FTZ 0, DAZ 0, all exceptions masked):
//! * two NaN inputs give the FIRST (destination) operand's payload, quieted. One NaN input gives
//!   that NaN quieted. An invalid operation without NaN input gives the default NaN 0xFFC00000.
//! * otherwise the IEEE single result (the host f32 arithmetic is the SSE instruction).
//! Limits: MXCSR sticky status flags are not modelled (the proof masks them, mask 0x3f), FTZ/DAZ
//! and non-nearest rounding are out of contract, only lane 0 arithmetic is modelled.

/// One XMM register as four little-endian 32-bit lanes (lane 0 is the scalar).
pub type Xmm = [u32; 4];

pub fn float(bits: u32) -> f32 {
    f32::from_bits(bits)
}

fn is_nan(bits: u32) -> bool {
    bits & 0x7fff_ffff > 0x7f80_0000
}

fn ss_result(dest: u32, source: u32, arithmetic: f32) -> u32 {
    if is_nan(dest) {
        dest | 0x0040_0000
    } else if is_nan(source) {
        source | 0x0040_0000
    } else if arithmetic.is_nan() {
        0xffc0_0000
    } else {
        arithmetic.to_bits()
    }
}

/// addss dest, source: dest = dest + source.
pub fn addss(dest: u32, source: u32) -> u32 {
    ss_result(dest, source, float(dest) + float(source))
}
pub fn subss(dest: u32, source: u32) -> u32 {
    ss_result(dest, source, float(dest) - float(source))
}
pub fn mulss(dest: u32, source: u32) -> u32 {
    ss_result(dest, source, float(dest) * float(source))
}
pub fn divss(dest: u32, source: u32) -> u32 {
    ss_result(dest, source, float(dest) / float(source))
}

/// cvtsi2ss lane 0 (round to nearest).
pub fn cvtsi2ss(value: u32) -> u32 {
    (value as i32 as f32).to_bits()
}

/// cvttss2si: truncate, NaN or out of range gives the integer indefinite 0x80000000.
pub fn cvttss2si(bits: u32) -> u32 {
    let value = float(bits);
    if !(value > -2147483648.0 && value < 2147483648.0) {
        0x8000_0000
    } else {
        value as i32 as u32
    }
}

/// The ZF, PF and CF that comiss/ucomiss leave (OF, SF, AF are cleared). Unordered sets all three.
/// comiss and ucomiss differ only in the invalid flag of MXCSR, which is not modelled.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Flags {
    pub zf: bool,
    pub pf: bool,
    pub cf: bool,
}

pub fn ucomiss(a: u32, b: u32) -> Flags {
    let (x, y) = (float(a), float(b));
    if x.is_nan() || y.is_nan() {
        Flags { zf: true, pf: true, cf: true }
    } else {
        Flags { zf: x == y, pf: false, cf: x < y }
    }
}
pub use ucomiss as comiss;

impl Flags {
    /// AH after `lahf`: SF:ZF:0:AF:0:PF:1:CF with SF = AF = 0.
    pub fn lahf_ah(self) -> u32 {
        (u32::from(self.zf) << 6) | (u32::from(self.pf) << 2) | 2 | u32::from(self.cf)
    }
    /// EAX after `lahf` keeps bits 0..7 and 16..31 and replaces AH.
    pub fn lahf_into(self, eax: u32) -> u32 {
        (eax & !0xff00) | (self.lahf_ah() << 8)
    }
    pub fn a(self) -> bool {
        !self.cf && !self.zf
    }
    pub fn ae(self) -> bool {
        !self.cf
    }
    pub fn b(self) -> bool {
        self.cf
    }
    pub fn be(self) -> bool {
        self.cf || self.zf
    }
    pub fn z(self) -> bool {
        self.zf
    }
    pub fn nz(self) -> bool {
        !self.zf
    }
    pub fn p(self) -> bool {
        self.pf
    }
    pub fn np(self) -> bool {
        !self.pf
    }
    /// `ucomiss; lahf; test ah, 0x44; jnp`: ordered equal.
    pub fn ordered_equal(self) -> bool {
        self.zf && !self.pf
    }
}
