//! Xbox ADPCM (64 frames per block, 36 bytes per channel). Oracle: src/audio/dsound_adpcm.c.

pub const FRAMES_PER_BLOCK: usize = 64;
pub const MONO_BLOCK_BYTES: usize = 36;

/// Decode interleaved `channels` (1 or 2) of Xbox ADPCM. None when the length is not a whole
/// number of blocks or a block header step index is outside 0..=88 (the oracle refuses both).
pub fn decode(encoded: &[u8], channels: usize) -> Option<Vec<i16>> {
    if !(1..=2).contains(&channels) {
        return None;
    }
    let block_bytes = channels * MONO_BLOCK_BYTES;
    if encoded.len() % block_bytes != 0 {
        return None;
    }
    let rd = |b: &[u8], o: usize| i16::from_le_bytes([b[o], b[o + 1]]);
    let blocks: Vec<&[u8]> = encoded.chunks(block_bytes).collect();
    if blocks
        .iter()
        .any(|b| (0..channels).any(|c| !(0..=88).contains(&rd(b, c * 4 + 2))))
    {
        return None;
    }
    // xbadpcm decodes 64 samples per block from the nibbles; the game's decoder emits the header predictor
    // first and drops the last decoded sample, so shift by one around the crate's output.
    let mut planar: Vec<Vec<i16>> = vec![Vec::new(); channels];
    let ok = match channels {
        1 => decode_planar::<1>(encoded, &mut planar),
        _ => decode_planar::<2>(encoded, &mut planar),
    };
    ok?;
    let mut pcm = vec![0i16; blocks.len() * FRAMES_PER_BLOCK * channels];
    for (bi, block) in blocks.iter().enumerate() {
        let base = bi * FRAMES_PER_BLOCK;
        for c in 0..channels {
            pcm[channels * base + c] = rd(block, c * 4);
            for s in 0..FRAMES_PER_BLOCK - 1 {
                pcm[channels * (base + 1 + s) + c] = planar[c][base + s];
            }
        }
    }
    Some(pcm)
}

fn decode_planar<const N: usize>(encoded: &[u8], out: &mut [Vec<i16>]) -> Option<()>
where
    [Vec<i16>; N]: xbadpcm::XboxADPCMDecodeSink<Error = ()>,
{
    let mut sink: [Vec<i16>; N] = std::array::from_fn(|_| Vec::new());
    xbadpcm::XboxADPCMDecoder::new(N, &mut sink)
        .decode(encoded)
        .ok()?;
    for (o, v) in out.iter_mut().zip(sink) {
        *o = v;
    }
    Some(())
}
