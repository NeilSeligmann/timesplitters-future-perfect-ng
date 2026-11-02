//! Read-only view of the static data of an XBE image by virtual address (T2189 UI-6 slice 15,
//! moved here from `tsfp-ui-builders` so the run-time UI can read the font tables without the
//! ported builders). The owner's `default.xbe` is read at run time, never stored.

use std::path::Path;

#[derive(Clone, Debug)]
struct Section {
    virtual_address: u32,
    virtual_size: u32,
    raw_address: u32,
    raw_size: u32,
}

/// Sections of an XBE file with their file bytes.
///
/// Missing or malformed addresses return `None`; bytes in a section's virtual tail
/// beyond its stored raw bytes read as zero.
///
/// ```
/// use tsfp_ui_model::xbe::XbeImage;
/// let mut bytes = vec![0; 0x200];
/// bytes[..4].copy_from_slice(b"XBEH");
/// bytes[0x104..0x108].copy_from_slice(&0x10000u32.to_le_bytes());
/// bytes[0x120..0x124].copy_from_slice(&0x10000u32.to_le_bytes());
/// let image = XbeImage::parse(bytes).unwrap();
/// assert_eq!(image.byte(0x20000), None);
/// ```
#[derive(Clone, Debug)]
pub struct XbeImage {
    bytes: Vec<u8>,
    sections: Vec<Section>,
}

fn word(bytes: &[u8], at: usize) -> Option<u32> {
    bytes
        .get(at..at.checked_add(4)?)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

impl XbeImage {
    /// Parses the XBE header: base address, section table.
    pub fn parse(bytes: Vec<u8>) -> Option<Self> {
        if bytes.get(0..4)? != b"XBEH" {
            return None;
        }
        let base = word(&bytes, 0x104)?;
        let count = word(&bytes, 0x11c)? as usize;
        let table = word(&bytes, 0x120)?.checked_sub(base)? as usize;
        let mut sections = Vec::new();
        for i in 0..count.min(64) {
            let at = table.checked_add(i * 0x38)?;
            sections.push(Section {
                virtual_address: word(&bytes, at + 4)?,
                virtual_size: word(&bytes, at + 8)?,
                raw_address: word(&bytes, at + 12)?,
                raw_size: word(&bytes, at + 16)?,
            });
        }
        Some(Self { bytes, sections })
    }
    /// Loads `path` (None when missing or not an XBE).
    pub fn load(path: &Path) -> Option<Self> {
        Self::parse(std::fs::read(path).ok()?)
    }
    /// Byte at virtual address `a`; None outside every section. Section tails past the raw size
    /// read as zero (uninitialised data).
    pub fn byte(&self, a: u32) -> Option<u8> {
        for s in &self.sections {
            if a >= s.virtual_address && a - s.virtual_address < s.virtual_size {
                let offset = a - s.virtual_address;
                return Some(if offset < s.raw_size {
                    *self
                        .bytes
                        .get(s.raw_address.checked_add(offset)? as usize)?
                } else {
                    0
                });
            }
        }
        None
    }
}

impl crate::font::StaticImage for XbeImage {
    fn byte(&self, address: u32) -> Option<u8> {
        XbeImage::byte(self, address)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_minimal_synthetic_header() {
        let mut b = vec![0u8; 0x400];
        b[0..4].copy_from_slice(b"XBEH");
        b[0x104..0x108].copy_from_slice(&0x10000u32.to_le_bytes());
        b[0x11c..0x120].copy_from_slice(&1u32.to_le_bytes());
        b[0x120..0x124].copy_from_slice(&(0x10000u32 + 0x200).to_le_bytes());
        let s = 0x200;
        b[s + 4..s + 8].copy_from_slice(&0x20000u32.to_le_bytes()); // virtual address
        b[s + 8..s + 12].copy_from_slice(&0x10u32.to_le_bytes()); // virtual size
        b[s + 12..s + 16].copy_from_slice(&0x300u32.to_le_bytes()); // raw address
        b[s + 16..s + 20].copy_from_slice(&0x08u32.to_le_bytes()); // raw size
        b[0x300] = 0xAB;
        let image = XbeImage::parse(b).unwrap();
        assert_eq!(image.byte(0x20000), Some(0xAB));
        assert_eq!(
            image.byte(0x2000c),
            Some(0),
            "tail beyond the raw size is zero"
        );
        assert_eq!(image.byte(0x20010), None);
        assert_eq!(image.byte(0x1ffff), None);
    }

    #[test]
    fn section_boundaries_and_malformed_offsets() {
        for raw_size in 0..=16u32 {
            let mut bytes = vec![0u8; 0x400];
            bytes[..4].copy_from_slice(b"XBEH");
            bytes[0x104..0x108].copy_from_slice(&0x10000u32.to_le_bytes());
            bytes[0x11c..0x120].copy_from_slice(&1u32.to_le_bytes());
            bytes[0x120..0x124].copy_from_slice(&0x10200u32.to_le_bytes());
            bytes[0x204..0x208].copy_from_slice(&0x20000u32.to_le_bytes());
            bytes[0x208..0x20c].copy_from_slice(&16u32.to_le_bytes());
            bytes[0x20c..0x210].copy_from_slice(&0x300u32.to_le_bytes());
            bytes[0x210..0x214].copy_from_slice(&raw_size.to_le_bytes());
            for i in 0..16 {
                bytes[0x300 + i] = i as u8 + 1;
            }
            let image = XbeImage::parse(bytes.clone()).unwrap();
            for offset in 0..16 {
                assert_eq!(
                    image.byte(0x20000 + offset),
                    Some(if offset < raw_size {
                        offset as u8 + 1
                    } else {
                        0
                    })
                );
            }
            assert_eq!(image.byte(0x20010), None);

            bytes[0x20c..0x210].copy_from_slice(&u32::MAX.to_le_bytes());
            bytes[0x210..0x214].copy_from_slice(&16u32.to_le_bytes());
            let image = XbeImage::parse(bytes).unwrap();
            assert_eq!(image.byte(0x20001), None);
        }
    }
}
