//! Minimal KTX2 writer for Xbox BC textures.
//!
//! The XBT payload is already BC-compressed, so this module only writes the KTX2
//! container, level index and Khronos Data Format Descriptor. No pixel blocks are
//! decoded or recompressed.
use crate::Result;

const IDENTIFIER: [u8; 12] = [
    0xab, b'K', b'T', b'X', b' ', b'2', b'0', 0xbb, b'\r', b'\n', 0x1a, b'\n',
];
const HEADER_SIZE: usize = 80;

fn align_up(value: usize, alignment: usize) -> Result<usize> {
    value
        .checked_add((alignment - value % alignment) % alignment)
        .ok_or_else(|| "ktx2: file offset overflow".into())
}

fn push_u16(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn push_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn push_u64(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn dfd(color_model: u8, block_bytes: usize) -> Vec<u8> {
    // Khronos Basic Data Format Descriptor fields for BC1A, BC2 or BC3.
    // These values follow the Khronos Data Format Specification §5.6.
    let samples: &[(u16, u8)] = match color_model {
        128 => &[(0, 1)],                 // BC1A: special selector denotes transparency.
        129 | 130 => &[(0, 15), (64, 0)], // BC2/BC3: alpha block, then color block.
        _ => unreachable!("only BC1A, BC2 and BC3 are supported"),
    };
    let descriptor_size = 24 + samples.len() * 16;
    let mut out = Vec::with_capacity(4 + descriptor_size);
    push_u32(&mut out, (4 + descriptor_size) as u32); // dfdTotalSize
    push_u16(&mut out, 0); // Khronos vendor id
    push_u16(&mut out, 0); // basic descriptor type
    push_u16(&mut out, 2); // descriptor version
    push_u16(&mut out, descriptor_size as u16);
    out.extend_from_slice(&[
        color_model, // BC1A=128, BC2=129, BC3=130
        1,           // BT.709 color primaries
        1,           // linear transfer function; XBT carries no sRGB tag
        0,           // no premultiplied-alpha flag
        3,
        3,
        0,
        0, // 4x4x1 texel block dimensions, stored minus one
        block_bytes as u8,
        0,
        0,
        0,
        0,
        0,
        0,
        0, // bytesPlane[0..7]
    ]);
    for &(bit_offset, channel_id) in samples {
        push_u16(&mut out, bit_offset);
        out.push(63); // sample bit length is stored minus one (64 bits)
        out.push(channel_id);
        out.extend_from_slice(&[0; 4]); // sample position
        push_u32(&mut out, 0); // sample lower bound
        push_u32(&mut out, 0); // sample upper bound
    }
    out
}

/// Encode already-compressed BC mip levels in a KTX2 file.
///
/// `levels` is ordered from the largest image to the smallest, as required by
/// KTX2. The function checks that each level has the byte size implied by its
/// dimensions and BC block size before writing any output.
pub fn encode_bc(
    width: u32,
    height: u32,
    vk_format: u32,
    color_model: u8,
    block_bytes: usize,
    levels: &[&[u8]],
) -> Result<Vec<u8>> {
    if width == 0 || height == 0 || levels.is_empty() || levels.len() > 32 {
        return Err("ktx2: invalid dimensions or mip count".into());
    }
    if !matches!((color_model, block_bytes), (128, 8) | (129, 16) | (130, 16)) {
        return Err("ktx2: unsupported BC descriptor".into());
    }

    let mut w = width as usize;
    let mut h = height as usize;
    for (index, level) in levels.iter().enumerate() {
        let blocks_w = w.div_ceil(4);
        let blocks_h = h.div_ceil(4);
        let expected = blocks_w
            .checked_mul(blocks_h)
            .and_then(|blocks| blocks.checked_mul(block_bytes))
            .ok_or_else(|| "ktx2: mip level size overflow".to_string())?;
        if level.len() != expected {
            return Err(format!(
                "ktx2: mip {index} has {} bytes; expected {expected}",
                level.len()
            ));
        }
        if index + 1 < levels.len() && w == 1 && h == 1 {
            return Err("ktx2: mip count exceeds the complete chain".into());
        }
        w = (w / 2).max(1);
        h = (h / 2).max(1);
    }

    let descriptor = dfd(color_model, block_bytes);
    let index_size = levels
        .len()
        .checked_mul(24)
        .ok_or_else(|| "ktx2: level index size overflow".to_string())?;
    let dfd_offset = HEADER_SIZE
        .checked_add(index_size)
        .ok_or_else(|| "ktx2: DFD offset overflow".to_string())?;
    let after_dfd = dfd_offset
        .checked_add(descriptor.len())
        .ok_or_else(|| "ktx2: DFD end overflow".to_string())?;
    let alignment = block_bytes.max(4);
    let mut next_offset = align_up(after_dfd, alignment)?;
    let mut level_offsets = Vec::with_capacity(levels.len());
    for level in levels {
        level_offsets.push(next_offset);
        next_offset = next_offset
            .checked_add(level.len())
            .ok_or_else(|| "ktx2: mip level end overflow".to_string())?;
        next_offset = align_up(next_offset, alignment)?;
    }

    let mut out = Vec::new();
    out.try_reserve(next_offset)
        .map_err(|error| format!("ktx2: output allocation failed: {error}"))?;
    out.extend_from_slice(&IDENTIFIER);
    for field in [
        vk_format,
        1, // typeSize for block-compressed formats
        width,
        height,
        0, // pixelDepth
        0, // layerCount (zero means non-array)
        1, // faceCount
        levels.len() as u32,
        0, // no supercompression
        dfd_offset as u32,
        descriptor.len() as u32,
        0, // no key/value data
        0,
    ] {
        push_u32(&mut out, field);
    }
    push_u64(&mut out, 0); // no supercompression global data
    push_u64(&mut out, 0);
    for (level, &offset) in levels.iter().zip(&level_offsets) {
        push_u64(&mut out, offset as u64);
        push_u64(&mut out, level.len() as u64);
        push_u64(&mut out, level.len() as u64); // uncompressed size, no supercompression
    }
    out.extend_from_slice(&descriptor);
    for (level, &offset) in levels.iter().zip(&level_offsets) {
        out.resize(offset, 0); // KTX2 mip padding is zero-filled.
        out.extend_from_slice(level);
    }
    Ok(out)
}

fn read_u32(b: &[u8], at: usize) -> Result<u32> {
    b.get(at..at + 4)
        .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
        .ok_or_else(|| "ktx2: truncated header".to_string())
}

fn read_u64(b: &[u8], at: usize) -> Result<u64> {
    let lo = read_u32(b, at)? as u64;
    let hi = read_u32(b, at + 4)? as u64;
    Ok(lo | (hi << 32))
}

fn rgb565(v: u16) -> [u8; 3] {
    let r = ((v >> 11) & 31) as u32;
    let g = ((v >> 5) & 63) as u32;
    let b = (v & 31) as u32;
    [
        ((r * 527 + 23) >> 6) as u8,
        ((g * 259 + 33) >> 6) as u8,
        ((b * 527 + 23) >> 6) as u8,
    ]
}

/// Colour endpoints and 2-bit indices of one BC1-style block. `four` forces the 4-colour mode (BC2/BC3).
fn color_block(block: &[u8], four: bool) -> [[u8; 4]; 16] {
    let c0 = u16::from_le_bytes([block[0], block[1]]);
    let c1 = u16::from_le_bytes([block[2], block[3]]);
    let (a, b) = (rgb565(c0), rgb565(c1));
    let mix = |x: u8, y: u8, wx: u32, wy: u32, d: u32| ((x as u32 * wx + y as u32 * wy) / d) as u8;
    let mut palette = [[0u8; 4]; 4];
    palette[0] = [a[0], a[1], a[2], 255];
    palette[1] = [b[0], b[1], b[2], 255];
    if four || c0 > c1 {
        for k in 0..3 {
            palette[2][k] = mix(a[k], b[k], 2, 1, 3);
            palette[3][k] = mix(a[k], b[k], 1, 2, 3);
        }
        palette[2][3] = 255;
        palette[3][3] = 255;
    } else {
        for k in 0..3 {
            palette[2][k] = mix(a[k], b[k], 1, 1, 2);
        }
        palette[2][3] = 255;
        palette[3] = [0, 0, 0, 0];
    }
    let bits = u32::from_le_bytes([block[4], block[5], block[6], block[7]]);
    let mut out = [[0u8; 4]; 16];
    for (i, px) in out.iter_mut().enumerate() {
        *px = palette[((bits >> (2 * i)) & 3) as usize];
    }
    out
}

fn bc3_alpha(block: &[u8]) -> [u8; 16] {
    let (a0, a1) = (block[0] as u32, block[1] as u32);
    let mut table = [0u32; 8];
    table[0] = a0;
    table[1] = a1;
    if a0 > a1 {
        for i in 1..7 {
            table[i + 1] = ((7 - i as u32) * a0 + i as u32 * a1) / 7;
        }
    } else {
        for i in 1..5 {
            table[i + 1] = ((5 - i as u32) * a0 + i as u32 * a1) / 5;
        }
        table[6] = 0;
        table[7] = 255;
    }
    let mut bits = 0u64;
    for (i, byte) in block[2..8].iter().enumerate() {
        bits |= (*byte as u64) << (8 * i);
    }
    let mut out = [0u8; 16];
    for (i, a) in out.iter_mut().enumerate() {
        *a = table[((bits >> (3 * i)) & 7) as usize] as u8;
    }
    out
}

/// Decode the largest mip level of a BC1/BC2/BC3 KTX2 file (as written by [`encode_bc`]) to RGBA8.
/// Returns `(width, height, pixels)`.
pub fn decode_rgba(file: &[u8]) -> Result<(u32, u32, Vec<u8>)> {
    if file.get(..12) != Some(&IDENTIFIER[..]) {
        return Err("ktx2: not a KTX2 file".into());
    }
    let vk = read_u32(file, 12)?;
    let (width, height) = (read_u32(file, 20)?, read_u32(file, 24)?);
    if read_u32(file, 44)? != 0 {
        return Err("ktx2: supercompressed files are not supported".into());
    }
    let kind = match vk {
        131..=134 => 1,
        135 | 136 => 2,
        137 | 138 => 3,
        _ => return Err(format!("ktx2: format {vk} is not BC1/BC2/BC3")),
    };
    let offset = read_u64(file, HEADER_SIZE)? as usize;
    let length = read_u64(file, HEADER_SIZE + 8)? as usize;
    let data = file
        .get(offset..offset.saturating_add(length))
        .ok_or("ktx2: level 0 outside the file")?;
    let (w, h) = (width as usize, height as usize);
    let block_bytes = if kind == 1 { 8 } else { 16 };
    let (bw, bh) = (w.div_ceil(4), h.div_ceil(4));
    if data.len() < bw * bh * block_bytes {
        return Err("ktx2: level 0 is too short".into());
    }
    let mut rgba = vec![0u8; w * h * 4];
    for by in 0..bh {
        for bx in 0..bw {
            let block = &data[(by * bw + bx) * block_bytes..][..block_bytes];
            let (color, alpha) = match kind {
                1 => (color_block(block, false), None),
                2 => {
                    let mut a = [0u8; 16];
                    for (i, v) in a.iter_mut().enumerate() {
                        *v = ((block[i / 2] >> (4 * (i % 2))) & 15) * 17;
                    }
                    (color_block(&block[8..], true), Some(a))
                }
                _ => (color_block(&block[8..], true), Some(bc3_alpha(block))),
            };
            for i in 0..16 {
                let (x, y) = (bx * 4 + i % 4, by * 4 + i / 4);
                if x < w && y < h {
                    let mut px = color[i];
                    if let Some(a) = alpha {
                        px[3] = a[i];
                    }
                    rgba[(y * w + x) * 4..][..4].copy_from_slice(&px);
                }
            }
        }
    }
    Ok((width, height, rgba))
}

#[cfg(test)]
mod decode_tests {
    use super::*;

    #[test]
    fn bc1_solid_and_transparent_blocks_round_trip() {
        // Block 1: both endpoints pure red (0xF800), all indices 0 -> opaque red.
        // Block 2: c0 < c1 enables 3-colour mode; index 3 is transparent black.
        let mut level = vec![0x00, 0xF8, 0x00, 0xF8, 0, 0, 0, 0];
        level.extend([0x00, 0x00, 0x1F, 0x00, 0xFF, 0xFF, 0xFF, 0xFF]);
        let file = encode_bc(8, 4, 131, 128, 8, &[&level]).unwrap();
        let (w, h, px) = decode_rgba(&file).unwrap();
        assert_eq!((w, h), (8, 4));
        assert_eq!(&px[0..4], &[255, 0, 0, 255]);
        assert_eq!(&px[4 * 4..4 * 4 + 4], &[0, 0, 0, 0]);
    }

    #[test]
    fn bc3_alpha_ramp_and_colour() {
        // Alpha endpoints 255/0 (8-value mode), indices 0 then 1: first pixel 255, second 0. Colour pure blue.
        let mut block = vec![255u8, 0, 0b1000, 0, 0, 0, 0, 0];
        block.extend([0x1F, 0x00, 0x1F, 0x00, 0, 0, 0, 0]);
        let file = encode_bc(4, 4, 137, 130, 16, &[&block]).unwrap();
        let (_, _, px) = decode_rgba(&file).unwrap();
        assert_eq!(&px[0..4], &[0, 0, 255, 255]);
        assert_eq!(px[7], 0);
    }

    #[test]
    fn rejects_non_ktx2_and_other_formats() {
        assert!(decode_rgba(b"nope").is_err());
        let file = encode_bc(4, 4, 131, 128, 8, &[&[0u8; 8]]).unwrap();
        let mut other = file.clone();
        other[12] = 37; // VK_FORMAT_R8G8B8A8_UNORM
        assert!(decode_rgba(&other).is_err());
    }
}
