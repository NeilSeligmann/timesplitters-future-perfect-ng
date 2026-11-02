//! Owned construction of the DirectSound effects descriptor over guest heap views.
//!
//! Parsing and guest allocation live at separate documented boundaries. This module maps each
//! validated image-relative effect range to its Rust-owned guest allocation and emits the exact
//! 296-byte little-endian descriptor consumed by the title/DSP binding.

use crate::effects_metadata::{MAX_MAPS, Metadata};

pub const DESCRIPTOR_BYTES: usize = 296;

fn range_fits(offset: u32, bytes: u32, start: u32, span: u32) -> bool {
    let end = u64::from(offset) + u64::from(bytes);
    let limit = u64::from(start) + u64::from(span);
    u64::from(offset) >= u64::from(start) && end <= limit && limit <= (1u64 << 32)
}

fn overlaps(a: u32, a_bytes: u32, b: u32, b_bytes: u32) -> bool {
    a_bytes != 0
        && b_bytes != 0
        && u64::from(a) + u64::from(a_bytes) > u64::from(b)
        && u64::from(b) + u64::from(b_bytes) > u64::from(a)
}

fn guest_range_fits(base: u32, offset: u32, bytes: u32) -> Option<u32> {
    let address = u64::from(base).checked_add(u64::from(offset))?;
    let end = address.checked_add(u64::from(bytes))?;
    (address <= u64::from(u32::MAX) && end <= (1u64 << 32)).then_some(address as u32)
}

fn put_word(output: &mut [u8], offset: usize, value: u32) {
    output[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

/// Build the title-visible effect descriptor from validated metadata and guest allocations.
///
/// Allocation addresses are the code, state, workspace, and Y views respectively. Every map is
/// checked against its image/workspace span and translated with checked 32-bit guest arithmetic.
/// Refusal returns `None`; callers must publish no partial descriptor.
pub fn build_descriptor(
    metadata: &Metadata,
    code_base: u32,
    state_base: u32,
    workspace_base: u32,
    y_base: u32,
) -> Option<[u8; DESCRIPTOR_BYTES]> {
    let count = usize::try_from(metadata.map_count).ok()?;
    if count == 0
        || count > MAX_MAPS
        || metadata.workspace_bytes == 0
        || [code_base, state_base, workspace_base, y_base].contains(&0)
        || u64::from(metadata.code_offset) + u64::from(metadata.code_bytes) > (1u64 << 32)
        || u64::from(metadata.state_offset) + u64::from(metadata.state_bytes) > (1u64 << 32)
        || guest_range_fits(code_base, 0, metadata.code_bytes).is_none()
        || guest_range_fits(state_base, 0, metadata.state_bytes).is_none()
        || guest_range_fits(workspace_base, 0, metadata.workspace_bytes).is_none()
    {
        return None;
    }

    let mut descriptor = [0u8; DESCRIPTOR_BYTES];
    put_word(&mut descriptor, 0, metadata.map_count);
    put_word(&mut descriptor, 4, metadata.workspace_bytes);

    for (index, map) in metadata.maps[..count].iter().enumerate() {
        if !range_fits(
            map.code_offset,
            map.code_bytes,
            metadata.code_offset,
            metadata.code_bytes,
        ) || !range_fits(
            map.state_offset,
            map.state_bytes,
            metadata.state_offset,
            metadata.state_bytes,
        ) || !range_fits(
            map.workspace_offset,
            map.workspace_bytes,
            0,
            metadata.workspace_bytes,
        ) || map.y_offset != 0
            || map.y_bytes != 0
        {
            return None;
        }
        for previous in &metadata.maps[..index] {
            if overlaps(
                map.code_offset,
                map.code_bytes,
                previous.code_offset,
                previous.code_bytes,
            ) || overlaps(
                map.state_offset,
                map.state_bytes,
                previous.state_offset,
                previous.state_bytes,
            ) || overlaps(
                map.workspace_offset,
                map.workspace_bytes,
                previous.workspace_offset,
                previous.workspace_bytes,
            ) {
                return None;
            }
        }
        let code = guest_range_fits(
            code_base,
            map.code_offset - metadata.code_offset,
            map.code_bytes,
        )?;
        let state = guest_range_fits(
            state_base,
            map.state_offset - metadata.state_offset,
            map.state_bytes,
        )?;
        let workspace =
            guest_range_fits(workspace_base, map.workspace_offset, map.workspace_bytes)?;
        let at = 8 + index * 32;
        for (word, value) in [
            code,
            map.code_bytes,
            state,
            map.state_bytes,
            y_base,
            map.y_bytes,
            workspace,
            map.workspace_bytes,
        ]
        .into_iter()
        .enumerate()
        {
            put_word(&mut descriptor, at + word * 4, value);
        }
    }
    Some(descriptor)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::effects_metadata::EffectMap;

    fn fixture() -> Metadata {
        let mut metadata = Metadata {
            code_offset: 0x818,
            code_bytes: 0x100,
            state_offset: 0x918,
            state_bytes: 0x200,
            map_count: 2,
            workspace_bytes: 0x1000,
            ..Metadata::default()
        };
        metadata.maps[0] = EffectMap {
            code_offset: 0x838,
            code_bytes: 0x20,
            state_offset: 0x938,
            state_bytes: 0x30,
            workspace_offset: 0x100,
            workspace_bytes: 0x80,
            ..EffectMap::default()
        };
        metadata.maps[1] = EffectMap {
            code_offset: 0x878,
            code_bytes: 0x40,
            state_offset: 0x978,
            state_bytes: 0x50,
            workspace_offset: 0x300,
            workspace_bytes: 0x100,
            ..EffectMap::default()
        };
        metadata
    }

    #[test]
    fn maps_validated_image_ranges_into_the_owned_guest_views() {
        let descriptor = build_descriptor(&fixture(), 0x20_000, 0x30_000, 0x40_000, 0x50_000)
            .expect("fixture metadata should bind");
        let words: Vec<u32> = descriptor[..72]
            .chunks_exact(4)
            .map(|word| u32::from_le_bytes(word.try_into().unwrap()))
            .collect();
        assert_eq!(words[0..2], [2, 0x1000]);
        assert_eq!(
            words[2..10],
            [0x20_020, 0x20, 0x30_020, 0x30, 0x50_000, 0, 0x40_100, 0x80]
        );
        assert_eq!(
            words[10..18],
            [0x20_060, 0x40, 0x30_060, 0x50, 0x50_000, 0, 0x40_300, 0x100]
        );
        assert!(descriptor[72..].iter().all(|byte| *byte == 0));
    }

    #[test]
    fn refuses_bad_ranges_and_guest_address_overflow() {
        let mut metadata = fixture();
        metadata.maps[0].code_offset = 0x700;
        assert!(build_descriptor(&metadata, 0x20_000, 0x30_000, 0x40_000, 0x50_000).is_none());

        let metadata = fixture();
        assert!(build_descriptor(&metadata, u32::MAX - 8, 0x30_000, 0x40_000, 0x50_000).is_none());
        assert!(build_descriptor(&metadata, 0x20_000, 0x30_000, 0xffff_f001, 0x50_000).is_none());
        assert!(build_descriptor(&metadata, 0, 0x30_000, 0x40_000, 0x50_000).is_none());
    }
}
