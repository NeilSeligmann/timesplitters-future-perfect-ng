// SPDX-License-Identifier: GPL-3.0-or-later
//! Minimal XBE section map: just enough to slice file-backed initialised data by guest address.
//! Header offsets are those of `tools/xbe/parser.py`. The image is borrowed, never copied.
const MAGIC: &[u8; 4] = b"XBEH";
const BASE_ADDRESS: usize = 0x104;
const SECTION_COUNT: usize = 0x11c;
const SECTION_HEADERS: usize = 0x120;
const SECTION_HEADER_SIZE: usize = 0x38;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum XbeError {
    BadMagic,
    Truncated,
}

#[derive(Clone, Copy, Debug)]
struct Section {
    virtual_addr: u32,
    raw_addr: u32,
    raw_size: u32,
}

pub struct XbeImage<'a> {
    data: &'a [u8],
    sections: [Option<Section>; 32],
}

fn word(data: &[u8], at: usize) -> Result<u32, XbeError> {
    data.get(at..at + 4)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .ok_or(XbeError::Truncated)
}

impl<'a> XbeImage<'a> {
    pub fn parse(data: &'a [u8]) -> Result<Self, XbeError> {
        if data.get(0..4) != Some(MAGIC.as_slice()) {
            return Err(XbeError::BadMagic);
        }
        let base = word(data, BASE_ADDRESS)?;
        let count = word(data, SECTION_COUNT)? as usize;
        let headers = word(data, SECTION_HEADERS)?.wrapping_sub(base) as usize;
        if count > 32 {
            return Err(XbeError::Truncated);
        }
        let mut sections = [None; 32];
        for (i, slot) in sections.iter_mut().enumerate().take(count) {
            let at = headers + i * SECTION_HEADER_SIZE;
            *slot = Some(Section {
                virtual_addr: word(data, at + 4)?,
                raw_addr: word(data, at + 12)?,
                raw_size: word(data, at + 16)?,
            });
        }
        Ok(Self { data, sections })
    }

    /// The `len` bytes at guest address `va`, only when they lie inside one section's file-backed
    /// part (the zero-filled tail of a section has no bytes in the file).
    pub fn file_bytes(&self, va: u32, len: u32) -> Option<&'a [u8]> {
        let end = va.checked_add(len)?;
        self.sections.iter().flatten().find_map(|s| {
            let section_end = s.virtual_addr.checked_add(s.raw_size)?;
            if va < s.virtual_addr || end > section_end {
                return None;
            }
            let start = s.raw_addr as usize + (va - s.virtual_addr) as usize;
            self.data.get(start..start + len as usize)
        })
    }
}
