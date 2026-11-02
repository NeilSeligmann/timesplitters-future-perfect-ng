// SPDX-License-Identifier: GPL-3.0-or-later
// T1510: faithful x87 closure state and fail-closed comparison. Mirror of
// tools/harness/x87_closure.py, proven against the same vectors
// (docs/evidence/t1510/fpclosure_vectors.txt). Contract: docs/replace-x87-t1510.md.
// Slots compare by f64 bit pattern when occupied. Empty slots are never compared. A raw80
// the f64 cannot hold exactly, non-contiguous tags or unmodelled status bits are
// Unrepresented, never Match. Tolerances: none.
pub const STATUS_TOP: u16 = 0x3800;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClosureFp { pub slots: [u64; 8], pub top: u8, pub control: u16, pub depth: u8 }
impl ClosureFp {
    pub fn valid(&self) -> bool { self.top < 8 && self.depth <= 8 }
    pub fn logical(&self, i: u8) -> u64 { self.slots[((self.top + i) & 7) as usize] }
}
/// Original x87 register file, raw80 per physical slot (low 80 bits of the u128).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RawX87 { pub physical: [u128; 8], pub tags: u16, pub control: u16, pub status: u16 }
impl RawX87 {
    pub fn tag(&self, i: usize) -> u16 { (self.tags >> (i * 2)) & 3 }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind { Match, Differ, Unrepresented, Missing }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Verdict { pub kind: Kind, pub diffs: u32, pub ignored_status: u16 }
pub const DIFF_DEPTH: u32 = 1;
pub const DIFF_TOP: u32 = 2;
pub const DIFF_CONTROL: u32 = 4;
/// Logical slot `i` differs: bit `8 + i`.
pub const DIFF_ST0: u32 = 8;

/// Reasons a raw x87 state cannot be represented by the f64 closure model.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RawConversionError {
    IgnoredTop,
    UnmodelledStatus,
    SparseStack,
    UnrepresentableSlot { physical: usize },
    TagMismatch { physical: usize },
}

pub fn f64_to_raw80(bits: u64) -> u128 {
    let sign = (bits >> 63) as u128;
    let exp = ((bits >> 52) & 0x7FF) as u128;
    let frac = (bits & ((1 << 52) - 1)) as u128;
    if exp == 0x7FF { return (sign << 79) | (0x7FFF << 64) | (1 << 63) | (frac << 11); }
    if exp == 0 {
        if frac == 0 { return sign << 79; }
        let width = 128 - frac.leading_zeros() as u128;
        return (sign << 79) | ((width - 1 + 16383 - 1074) << 64) | (frac << (64 - width));
    }
    (sign << 79) | ((exp + 16383 - 1023) << 64) | (1 << 63) | (frac << 11)
}
pub fn raw80_to_f64(raw: u128) -> Option<u64> {
    let sign = ((raw >> 79) & 1) as u64;
    let exp = ((raw >> 64) & 0x7FFF) as i64;
    let mant = raw as u64;
    if exp == 0x7FFF {
        if mant >> 63 == 0 || mant & 0x7FF != 0 { return None; }
        return Some((sign << 63) | (0x7FF << 52) | ((mant >> 11) & ((1 << 52) - 1)));
    }
    if exp == 0 { return if mant == 0 { Some(sign << 63) } else { None }; }
    if mant >> 63 == 0 { return None; }
    let unbiased = exp - 16383;
    if unbiased > 1023 { return None; }
    if unbiased >= -1022 {
        if mant & 0x7FF != 0 { return None; }
        return Some((sign << 63) | (((unbiased + 1023) as u64) << 52) | ((mant >> 11) & ((1 << 52) - 1)));
    }
    let shift = 11 + (-1022 - unbiased);
    if shift >= 64 || mant & ((1u64 << shift) - 1) != 0 { return None; }
    Some((sign << 63) | (mant >> shift))
}
/// Hardware tag of an occupied raw80: 0 valid, 1 zero, 2 special.
pub fn raw80_tag(raw: u128) -> u16 {
    let exp = (raw >> 64) & 0x7FFF;
    let mant = raw as u64;
    if exp == 0x7FFF { 2 } else if exp == 0 { if mant == 0 { 1 } else { 2 } }
    else if mant >> 63 != 0 { 0 } else { 2 }
}
/// Missing observations are `None` and fail closed even when both sides are missing.
pub fn differences(expected: Option<&ClosureFp>, actual: Option<&ClosureFp>) -> Option<u32> {
    let (e, a) = (expected?, actual?);
    let mut out = 0;
    if e.depth != a.depth { out |= DIFF_DEPTH; }
    if e.top != a.top { out |= DIFF_TOP; }
    if e.control != a.control { out |= DIFF_CONTROL; }
    if e.depth == a.depth && e.top == a.top {
        for i in 0..e.depth { if e.logical(i) != a.logical(i) { out |= DIFF_ST0 << i; } }
    }
    Some(out)
}
/// Lift an original raw state, retaining the reason when it cannot be represented.
pub fn from_raw(s: &RawX87, ignore_status: u16) -> Result<(ClosureFp, u16), RawConversionError> {
    if ignore_status & STATUS_TOP != 0 { return Err(RawConversionError::IgnoredTop); }
    let extra = s.status & !STATUS_TOP;
    if extra & !ignore_status != 0 { return Err(RawConversionError::UnmodelledStatus); }
    let top = ((s.status >> 11) & 7) as usize;
    let mut depth = 0usize;
    let mut occupied = 0u8;
    for i in 0..8 { if s.tag(i) != 3 { depth += 1; occupied |= 1 << i; } }
    let mut run = 0u8;
    for k in 0..depth { run |= 1 << ((top + k) & 7); }
    if run != occupied { return Err(RawConversionError::SparseStack); }
    let mut slots = [0u64; 8];
    for i in 0..8 {
        if s.tag(i) == 3 { continue; }
        slots[i] = raw80_to_f64(s.physical[i])
            .ok_or(RawConversionError::UnrepresentableSlot { physical: i })?;
        if raw80_tag(s.physical[i]) != s.tag(i) {
            return Err(RawConversionError::TagMismatch { physical: i });
        }
    }
    Ok((ClosureFp { slots, top: top as u8, control: s.control, depth: depth as u8 }, extra & ignore_status))
}
pub fn compare_raw(original: Option<&RawX87>, lifted: Option<&ClosureFp>, ignore_status: u16) -> Verdict {
    let (Some(o), Some(l)) = (original, lifted) else {
        return Verdict { kind: Kind::Missing, diffs: 0, ignored_status: 0 };
    };
    match from_raw(o, ignore_status) {
        Err(_) => Verdict { kind: Kind::Unrepresented, diffs: 0, ignored_status: 0 },
        Ok((lowered, ignored)) => {
            let diffs = differences(Some(&lowered), Some(l)).unwrap_or(0);
            Verdict { kind: if diffs == 0 { Kind::Match } else { Kind::Differ }, diffs, ignored_status: ignored }
        }
    }
}
