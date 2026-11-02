//! T1867: the effect kind table `0x4DB490` (49 rows of 24 bytes) and the effect definition table `0x4E0FA0` (stride 0x1C),
//! read from an XBE image at run time. No game bytes are committed: the loader takes the image bytes the caller read
//! (tests read `build/default.xbe`, or the path in `TSFP_XBE`, and skip when it is absent). Both tables are
//! statically initialised in the image (section 9, `.data`), so reading them from the file is MEASURED.
//!
//! The XBE header is parsed minimally: image base `[0x104]`, section count `[0x11C]`, section header table VA `[0x120]`;
//! a section header is 0x38 bytes with flags, virtual address, virtual size, raw address and raw size as its first five
//! dwords. A virtual address inside the raw part of a section maps to its file offset.

use super::svc_particle_efbd0::{EmberWorld, KindRow};

/// VA of the kind table and its row count and stride.
pub const KIND_TABLE: u32 = 0x4D_B490;
pub const KIND_ROWS: usize = 49;
pub const KIND_STRIDE: u32 = 24;
/// VA of the effect definition table (first dword non zero = defined), entry count and stride.
///
/// T1868 correction: the table has only [`EFFECT_ENTRIES`] (19) real entries, ids 0 to 18 (the kind rows reference ids up to 18).
/// Entry 19 onward is the data of the neighbouring tables (a pointer value repeats there), so the "537 defined ids" of T1867
/// were an over-read. Nothing writes the first dword at run time (`tools/t1868_scan_refs.py`: zero stores, four loads), so the
/// ids 1, 3 and 12 to 18 stay undefined for the whole run: the default start path skips them exactly as the original does and
/// there is no effect loader. `EFFECT_COUNT` keeps the faithful over-read so ids outside the table read what the original reads.
pub const EFFECT_TABLE: u32 = 0x4E_0FA0;
pub const EFFECT_ENTRIES: usize = 19;
pub const EFFECT_COUNT: usize = 0x400;
pub const EFFECT_STRIDE: u32 = 0x1C;

fn word(image: &[u8], offset: usize) -> Option<u32> {
    let bytes = image.get(offset..offset + 4)?;
    Some(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

/// The bytes at `[va, va + len)` of the image, when they lie in the raw part of one section.
pub fn bytes_at(image: &[u8], va: u32, len: u32) -> Option<&[u8]> {
    let base = word(image, 0x104)?;
    let count = word(image, 0x11C)? as usize;
    let headers = word(image, 0x120)?.checked_sub(base)? as usize;
    for index in 0..count {
        let at = headers + 0x38 * index;
        let (virtual_address, raw_address, raw_size) = (
            word(image, at + 4)?,
            word(image, at + 12)?,
            word(image, at + 16)?,
        );
        let end = u64::from(va) + u64::from(len);
        if va >= virtual_address && end <= u64::from(virtual_address) + u64::from(raw_size) {
            let start = raw_address as usize + (va - virtual_address) as usize;
            return image.get(start..start + len as usize);
        }
    }
    None
}

fn f32_at(bytes: &[u8], offset: usize) -> f32 {
    f32::from_bits(u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ]))
}

fn u32_at(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

/// Decode one 24 byte row (`+0` and `+4` effect ids, `+8` word, `+0xC` light radius, `+0x10` radius, `+0x14` flags).
pub fn decode_row(bytes: &[u8]) -> KindRow {
    KindRow {
        effect_a: u32_at(bytes, 0) as i32,
        effect_b: u32_at(bytes, 4) as i32,
        word_8: u32_at(bytes, 8),
        light_radius: f32_at(bytes, 12),
        radius: f32_at(bytes, 16),
        flags: u32_at(bytes, 20),
    }
}

/// The 49 rows of `0x4DB490`, or `None` when the image does not map the table.
pub fn load_kind_rows(image: &[u8]) -> Option<Vec<KindRow>> {
    let table = bytes_at(image, KIND_TABLE, KIND_STRIDE * KIND_ROWS as u32)?;
    Some(
        table
            .chunks_exact(KIND_STRIDE as usize)
            .map(decode_row)
            .collect(),
    )
}

/// `effect_defined[id]` for the 0x400 entries of `0x4E0FA0` (first dword of the entry non zero).
pub fn load_effect_defined(image: &[u8]) -> Option<Vec<bool>> {
    let table = bytes_at(image, EFFECT_TABLE, EFFECT_STRIDE * EFFECT_COUNT as u32)?;
    Some(
        table
            .chunks_exact(EFFECT_STRIDE as usize)
            .map(|entry| u32_at(entry, 0) != 0)
            .collect(),
    )
}

/// Put the image's kind rows (over `rows[..49]`, rows above stay as they were) and effect definitions into `world`.
/// Returns false and leaves `world` untouched when the image lacks either table.
pub fn install_tables(world: &mut EmberWorld, image: &[u8]) -> bool {
    let (Some(rows), Some(defined)) = (load_kind_rows(image), load_effect_defined(image)) else {
        return false;
    };
    if world.rows.len() < rows.len() {
        world.rows.resize(
            rows.len(),
            KindRow {
                effect_a: -1,
                effect_b: -1,
                ..KindRow::default()
            },
        );
    }
    world.rows[..rows.len()].copy_from_slice(&rows);
    world.effect_defined = defined;
    true
}

/// A synthetic image (no game data) with one section that holds both tables, for tests: the rows are
/// `(kind % 7, -1, 0, kind, kind + 0.5, kind & 1)` and the definitions are defined for odd ids.
pub fn synthetic_image() -> Vec<u8> {
    let base = 0x1_0000u32;
    let virtual_address = 0x4D_B000u32;
    let raw_address = 0x1000u32;
    let raw_size = 0x6_0000u32;
    let mut image = vec![0u8; (raw_address + raw_size) as usize];
    let put = |image: &mut Vec<u8>, at: usize, value: u32| {
        image[at..at + 4].copy_from_slice(&value.to_le_bytes())
    };
    put(&mut image, 0x104, base);
    put(&mut image, 0x11C, 1);
    put(&mut image, 0x120, base + 0x200);
    for (k, value) in [0, virtual_address, raw_size, raw_address, raw_size]
        .into_iter()
        .enumerate()
    {
        put(&mut image, 0x200 + 4 * k, value);
    }
    let offset = |va: u32| (raw_address + va - virtual_address) as usize;
    for kind in 0..KIND_ROWS as u32 {
        let at = offset(KIND_TABLE + KIND_STRIDE * kind);
        put(&mut image, at, kind % 7);
        put(&mut image, at + 4, u32::MAX);
        put(&mut image, at + 12, (kind as f32).to_bits());
        put(&mut image, at + 16, (kind as f32 + 0.5).to_bits());
        put(&mut image, at + 20, kind & 1);
    }
    for id in 0..EFFECT_COUNT as u32 {
        put(
            &mut image,
            offset(EFFECT_TABLE + EFFECT_STRIDE * id),
            id & 1,
        );
    }
    image
}
