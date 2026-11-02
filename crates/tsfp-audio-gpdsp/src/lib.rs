//! Xbox GP DSP image preparation. Execution and audio routing are not yet enabled.

pub mod dma;
pub mod frame_buffer;
pub mod gp;
pub mod pcm;
pub mod renderer;
pub mod voice_filter;

use tsfp_audio::{effects_cipher, effects_identity, effects_metadata::Metadata};

const FIRMWARE_BYTES: usize = 0x5cc;
const FIRMWARE_FNV: u64 = 0x4327_65cc_cdec_4764;
const STAGE_BYTES: usize = 0x6c000;
const STATE_BASE: usize = 0x27a8;
const STATE_WORDS: usize = 2022;

/// Prepared private image and its validated effect mappings, owned by the host.
pub struct PreparedImage {
    pub stage: Vec<u8>,
    pub metadata: Metadata,
}

/// Prepare the measured game image using the same identity checks as the C adapter.
/// Guest data is supplied by the caller and is never embedded in the executable.
pub fn prepare(firmware: &[u8], image: &[u8]) -> Option<PreparedImage> {
    if firmware.len() != FIRMWARE_BYTES
        || effects_identity::fingerprints(firmware).fnv1a64 != FIRMWARE_FNV
    {
        return None;
    }
    let metadata = effects_identity::validate_known_image(image)?;
    prepare_validated(firmware, image, metadata)
}

fn prepare_validated(firmware: &[u8], image: &[u8], metadata: Metadata) -> Option<PreparedImage> {
    let mut stage = vec![0; STAGE_BYTES];
    stage.get_mut(..image.len())?.copy_from_slice(image);
    stage.get_mut(..firmware.len())?.copy_from_slice(firmware);
    let iv_start = metadata.iv_offset as usize;
    let ivs_encrypted = image.get(iv_start..iv_start.checked_add(metadata.iv_bytes as usize)?)?;
    let mut ivs = vec![0; ivs_encrypted.len()];
    if !effects_cipher::decode(effects_cipher::setup_key(), ivs_encrypted, &mut ivs) {
        return None;
    }
    for (index, map) in metadata
        .maps
        .iter()
        .take(metadata.map_count as usize)
        .enumerate()
    {
        let start = map.code_offset as usize;
        let end = start.checked_add(map.code_bytes as usize)?;
        let key: [u8; 8] = ivs.get(index * 8..index * 8 + 8)?.try_into().ok()?;
        if start < 0x818
            || end > STATE_BASE
            || !effects_cipher::decode(key, image.get(start..end)?, stage.get_mut(start..end)?)
        {
            return None;
        }
        let state_end = (map.state_offset as usize).checked_add(map.state_bytes as usize)?;
        if (map.state_offset as usize) < STATE_BASE || state_end > STATE_BASE + 2022 * 4 {
            return None;
        }
    }
    Some(PreparedImage { stage, metadata })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tsfp_audio::effects_metadata::EffectMap;

    #[test]
    fn prepares_code_with_per_map_iv_and_keeps_state() {
        let mut image = vec![0u8; 0x4900];
        image[0x818..0x81c].copy_from_slice(&[1, 2, 3, 4]);
        image[STATE_BASE] = 42;
        let mut metadata = Metadata {
            iv_offset: 0x4868,
            iv_bytes: 8,
            map_count: 1,
            ..Metadata::default()
        };
        metadata.maps[0] = EffectMap {
            code_offset: 0x818,
            code_bytes: 4,
            state_offset: STATE_BASE as u32,
            state_bytes: 4,
            ..EffectMap::default()
        };
        let mut iv = [0; 8];
        effects_cipher::decode(effects_cipher::setup_key(), &image[0x4868..0x4870], &mut iv);
        let mut expected = [0; 4];
        effects_cipher::decode(iv, &image[0x818..0x81c], &mut expected);
        let prepared = prepare_validated(&[7; 4], &image, metadata).unwrap();
        assert_eq!(&prepared.stage[..4], &[7; 4]);
        assert_eq!(&prepared.stage[0x818..0x81c], &expected);
        assert_eq!(prepared.stage[STATE_BASE], 42);
        assert_eq!(prepared.stage.len(), STAGE_BYTES);
    }

    #[test]
    fn rejects_wrong_identity_and_out_of_bounds_map() {
        assert!(prepare(&[], &[]).is_none());
        let mut metadata = Metadata {
            iv_offset: 0,
            iv_bytes: 8,
            map_count: 1,
            ..Metadata::default()
        };
        metadata.maps[0].code_offset = STATE_BASE as u32;
        metadata.maps[0].code_bytes = 1;
        assert!(prepare_validated(&[], &[0; 16], metadata).is_none());
    }
}
