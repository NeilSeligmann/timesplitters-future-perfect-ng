// SPDX-License-Identifier: GPL-3.0-or-later
//! Name-field specs read at run time from the original page builders: each builder pushes the
//! arguments of the text-field constructor 0x7D090 as immediates, and this module decodes them
//! from the owner's image so limits, flags, width scale and confirm/cancel handlers are never
//! hardcoded. See docs/t2197-text-field.md (argument order measured from 0x7D090).

use super::keyboard::FieldFlags;

/// Original text-field constructor.
pub const CONSTRUCTOR: u32 = 0x7_d090;

/// Name fields this slice covers, by original page builder.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NameField {
    /// "Create New Persona" page (0x19E810), the profile name.
    ProfileName,
    /// Map Maker map name page (0x2D0310).
    MapName,
    /// Map Maker map description page (0x2D03F0).
    MapDescription,
    /// Save name page (0x32AD70).
    SaveName,
}

impl NameField {
    pub const ALL: [Self; 4] = [
        Self::ProfileName,
        Self::MapName,
        Self::MapDescription,
        Self::SaveName,
    ];
    /// Entry of the page builder that constructs the field.
    pub fn builder(self) -> u32 {
        match self {
            Self::ProfileName => 0x19_e810,
            Self::MapName => 0x2d_0310,
            Self::MapDescription => 0x2d_03f0,
            Self::SaveName => 0x32_ad70,
        }
    }
}

/// Typed constructor arguments (0x7D090: page, source, limit, width scale, selection,
/// flags, done handler, cancel handler).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FieldSpec {
    /// Address of the call to the constructor.
    pub call_site: u32,
    /// Guest buffer the confirmed text is written back to (None when passed in a register).
    pub source: Option<u32>,
    /// Character limit before the 255 clamp.
    pub limit: u32,
    /// Font-width scale (+0x238); zero or negative disables the width check.
    pub width_scale: f32,
    /// Non-zero selects the whole initial text (constructor sets flag 0x10).
    pub select_all: bool,
    pub flags: FieldFlags,
    /// Handler stored at +0x1CC, called by the page's done path (INFERRED name).
    pub done: u32,
    /// Handler stored at +0x1D0, called by the page's cancel path (INFERRED name).
    pub cancel: u32,
}

/// One `push imm` decoded backwards from `end` (exclusive). `6A ib` is sign-extended.
fn push_before(read: &impl Fn(u32) -> Option<u8>, end: u32) -> Option<(u32, u32)> {
    if read(end - 2)? == 0x6a {
        return Some((end - 2, read(end - 1)? as i8 as i32 as u32));
    }
    if read(end - 5)? == 0x68 {
        let v = (0..4).try_fold(0u32, |acc, i| {
            Some(acc | (u32::from(read(end - 4 + i)?) << (8 * i)))
        })?;
        return Some((end - 5, v));
    }
    None
}

/// Decodes the pushes before `call_site` (`E8 rel32` to the constructor). The builders push the
/// page pointer from a register (one byte, 0x50..0x57) right before the call.
pub fn decode_call(read: impl Fn(u32) -> Option<u8>, call_site: u32) -> Option<FieldSpec> {
    if read(call_site)? != 0xe8 {
        return None;
    }
    let rel = (0..4).try_fold(0u32, |acc, i| {
        Some(acc | (u32::from(read(call_site + 1 + i)?) << (8 * i)))
    })?;
    if call_site.wrapping_add(5).wrapping_add(rel) != CONSTRUCTOR {
        return None;
    }
    let page = call_site - 1;
    if !(0x50..=0x57).contains(&read(page)?) {
        return None;
    }
    let (at, source) = match read(page - 1)? {
        0x50..=0x57 => (page - 1, None),
        _ => {
            let (at, v) = push_before(&read, page)?;
            (at, Some(v))
        }
    };
    let mut args = [0u32; 6];
    let mut end = at;
    for slot in &mut args {
        let (at, v) = push_before(&read, end)?;
        *slot = v;
        end = at;
    }
    let [limit, width, select, flags, done, cancel] = args;
    Some(FieldSpec {
        call_site,
        source,
        limit,
        width_scale: f32::from_bits(width),
        select_all: select != 0,
        flags: FieldFlags(flags),
        done,
        cancel,
    })
}

/// Finds the first constructor call within `scan` bytes of `builder` and decodes it.
pub fn find_in_builder(
    read: impl Fn(u32) -> Option<u8>,
    builder: u32,
    scan: u32,
) -> Option<FieldSpec> {
    (builder..builder + scan).find_map(|at| decode_call(&read, at))
}

/// The spec of a known name field, read from the image.
pub fn load(field: NameField, read: impl Fn(u32) -> Option<u8>) -> Option<FieldSpec> {
    find_in_builder(read, field.builder(), 0x100)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn decodes_a_synthetic_builder() {
        // push cancel; push done; push 0 (flags); push 0 (select); push 50.0f; push 0x20;
        // push buffer; push eax; call constructor.
        let mut code = vec![0x90u8; 4];
        code.extend([
            0x68, 0x00, 0x02, 0x2d, 0x00, 0x68, 0x10, 0x02, 0x2d, 0x00, 0x6a, 0x00, 0x6a,
        ]);
        code.extend([
            0x00, 0x68, 0x00, 0x00, 0x48, 0x42, 0x6a, 0x20, 0x68, 0x00, 0x3f, 0x78, 0x00, 0x50,
        ]);
        let base = 0x2d_0300u32;
        let call = base + code.len() as u32;
        let rel = CONSTRUCTOR.wrapping_sub(call + 5);
        code.push(0xe8);
        code.extend(rel.to_le_bytes());
        let read = |a: u32| code.get(a.checked_sub(base)? as usize).copied();
        let spec = find_in_builder(read, base, 0x40).unwrap();
        assert_eq!(spec.call_site, call);
        assert_eq!(spec.source, Some(0x78_3f00));
        assert_eq!(spec.limit, 0x20);
        assert_eq!(spec.width_scale, 50.0);
        assert!(!spec.select_all);
        assert_eq!(spec.flags, FieldFlags(0));
        assert_eq!((spec.done, spec.cancel), (0x2d_0210, 0x2d_0200));
    }
}
