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
