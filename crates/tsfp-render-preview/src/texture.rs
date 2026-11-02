// SPDX-License-Identifier: GPL-3.0-or-later
//! Bounded offline decoders for texture payloads captured at a live draw.
//!
//! The format mapping follows `src/gpu/live_texture.c`. This first slice only
//! accepts the measured linear A8R8G8B8 and measured DXT1 formats; it refuses
//! inferred formats, mip chains, cube maps, and malformed extents.

const MAX_DIMENSION: u32 = 2048;
const MAX_DECODED_BYTES: usize = 16 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextureDecodeError {
    UnsupportedFormat,
    UnsupportedLayout,
    InvalidExtent,
    SourceLength,
    SizeLimit,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecodedTexture {
    pub width: u32,
    pub height: u32,
    /// Tight RGBA8 texels, top row first.
    pub rgba: Vec<u8>,
}

/// Decode one captured Xbox texture payload into tight RGBA8.
pub fn decode_texture_rgba(
    format: u32,
    size_word: u32,
    source: &[u8],
) -> Result<DecodedTexture, TextureDecodeError> {
    let colour = (format >> 8) & 0xff;
    let levels = (format >> 16) & 0xf;
    let exp_width = (format >> 20) & 0xf;
    let exp_height = (format >> 24) & 0xf;
    if ((format >> 4) & 0xf) != 2 || ((format >> 28) & 0xf) != 0 || levels != 1 {
        return Err(TextureDecodeError::UnsupportedFormat);
    }
    if format & 4 != 0 {
        return Err(TextureDecodeError::UnsupportedLayout);
    }

    let (width, height) = match colour {
        // DXT1 dimensions are exponents in the format word. Its blocks are
        // row-major even though the NV2A format class is marked swizzled.
        0x0c => (1u32.checked_shl(exp_width), 1u32.checked_shl(exp_height)),
        // The only supported linear, uncompressed layout in this slice.
        0x12 => (
            Some((size_word & 0xfff) + 1),
            Some(((size_word >> 12) & 0xfff) + 1),
        ),
        _ => return Err(TextureDecodeError::UnsupportedFormat),
    };
    let (width, height) = (
        width.ok_or(TextureDecodeError::InvalidExtent)?,
        height.ok_or(TextureDecodeError::InvalidExtent)?,
    );
    if width == 0 || height == 0 || width > MAX_DIMENSION || height > MAX_DIMENSION {
        return Err(TextureDecodeError::InvalidExtent);
    }
    let pixel_bytes = (width as usize)
        .checked_mul(height as usize)
        .and_then(|pixels| pixels.checked_mul(4))
        .filter(|bytes| *bytes <= MAX_DECODED_BYTES)
        .ok_or(TextureDecodeError::SizeLimit)?;

    let rgba = match colour {
        0x12 => decode_linear_argb(size_word, width, height, source, pixel_bytes)?,
        0x0c => decode_dxt1(width, height, source, pixel_bytes)?,
        _ => unreachable!("format was checked above"),
    };
    Ok(DecodedTexture {
        width,
        height,
        rgba,
    })
}

fn decode_linear_argb(
    size_word: u32,
    width: u32,
    height: u32,
    source: &[u8],
    decoded_bytes: usize,
) -> Result<Vec<u8>, TextureDecodeError> {
    let pitch = (((size_word >> 24) & 0xff) + 1) as usize * 64;
    let row_bytes = (width as usize)
        .checked_mul(4)
        .ok_or(TextureDecodeError::SizeLimit)?;
    let source_bytes = pitch
        .checked_mul(height as usize)
        .ok_or(TextureDecodeError::SizeLimit)?;
    if pitch < row_bytes || source.len() != source_bytes {
        return Err(TextureDecodeError::SourceLength);
    }
    let mut rgba = Vec::with_capacity(decoded_bytes);
    for row in 0..height as usize {
        for texel in source[row * pitch..row * pitch + row_bytes].chunks_exact(4) {
            // LiveTexture's measured A8R8G8B8 decoder reads little-endian
            // ARGB words, producing RGBA bytes for wgpu.
            rgba.extend_from_slice(&[texel[2], texel[1], texel[0], texel[3]]);
        }
    }
    Ok(rgba)
}

fn decode_dxt1(
    width: u32,
    height: u32,
    source: &[u8],
    decoded_bytes: usize,
) -> Result<Vec<u8>, TextureDecodeError> {
    let blocks_x = width.div_ceil(4) as usize;
    let blocks_y = height.div_ceil(4) as usize;
    let source_bytes = blocks_x
        .checked_mul(blocks_y)
        .and_then(|blocks| blocks.checked_mul(8))
        .ok_or(TextureDecodeError::SizeLimit)?;
    if source.len() != source_bytes {
        return Err(TextureDecodeError::SourceLength);
    }
    let mut rgba = vec![0; decoded_bytes];
    for block_y in 0..blocks_y {
        for block_x in 0..blocks_x {
            let offset = (block_y * blocks_x + block_x) * 8;
            let block = &source[offset..offset + 8];
            let c0 = u16::from_le_bytes([block[0], block[1]]);
            let c1 = u16::from_le_bytes([block[2], block[3]]);
            let mut palette = [[0u8; 4]; 4];
            palette[0] = expand_565(c0);
            palette[1] = expand_565(c1);
            if c0 > c1 {
                for channel in 0..3 {
                    palette[2][channel] = ((2 * u16::from(palette[0][channel])
                        + u16::from(palette[1][channel]))
                        / 3) as u8;
                    palette[3][channel] = ((u16::from(palette[0][channel])
                        + 2 * u16::from(palette[1][channel]))
                        / 3) as u8;
                }
                palette[2][3] = 255;
                palette[3][3] = 255;
            } else {
                for channel in 0..3 {
                    palette[2][channel] = ((u16::from(palette[0][channel])
                        + u16::from(palette[1][channel]))
                        / 2) as u8;
                }
                palette[2][3] = 255;
                palette[3] = [0, 0, 0, 0];
            }
            let indices = u32::from_le_bytes(block[4..8].try_into().expect("four bytes"));
            for texel in 0..16 {
                let x = block_x * 4 + texel % 4;
                let y = block_y * 4 + texel / 4;
                if x >= width as usize || y >= height as usize {
                    continue;
                }
                let colour = palette[((indices >> (texel * 2)) & 3) as usize];
                let dst = (y * width as usize + x) * 4;
                rgba[dst..dst + 4].copy_from_slice(&colour);
            }
        }
    }
    Ok(rgba)
}

fn expand_565(value: u16) -> [u8; 4] {
    let red = ((value >> 11) & 31) as u8;
    let green = ((value >> 5) & 63) as u8;
    let blue = (value & 31) as u8;
    [
        (red << 3) | (red >> 2),
        (green << 2) | (green >> 4),
        (blue << 3) | (blue >> 2),
        255,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    const FORMAT_PREFIX: u32 = 0x21 | (2 << 4) | (1 << 16);

    #[test]
    fn decodes_measured_linear_a8r8g8b8_and_checks_pitch() {
        let size = 0; // 64-byte pitch
        let mut source = vec![0; 64];
        source[..4].copy_from_slice(&[0x33, 0x22, 0x11, 0x44]);
        let image = decode_texture_rgba(FORMAT_PREFIX | (0x12 << 8), size, &source).unwrap();
        assert_eq!((image.width, image.height), (1, 1));
        assert_eq!(image.rgba, [0x11, 0x22, 0x33, 0x44]);
        assert_eq!(
            decode_texture_rgba(FORMAT_PREFIX | (0x12 << 8), size, &source[..4]),
            Err(TextureDecodeError::SourceLength)
        );
    }

    #[test]
    fn decodes_measured_dxt1_colour_and_transparent_modes() {
        let format = FORMAT_PREFIX | (0x0c << 8) | (2 << 20) | (2 << 24);
        let mut opaque = [0u8; 8];
        opaque[..2].copy_from_slice(&0xf800u16.to_le_bytes());
        opaque[2..4].copy_from_slice(&0x07e0u16.to_le_bytes());
        let red = decode_texture_rgba(format, 0, &opaque).unwrap();
        assert_eq!((red.width, red.height), (4, 4));
        assert_eq!(&red.rgba[..4], &[255, 0, 0, 255]);

        let mut transparent = [0u8; 8];
        transparent[4..8].copy_from_slice(&3u32.to_le_bytes());
        let image = decode_texture_rgba(format, 0, &transparent).unwrap();
        assert_eq!(&image.rgba[..4], &[0, 0, 0, 0]);
    }

    #[test]
    fn refuses_inferred_layouts_and_oversized_extents() {
        let dxt1 = FORMAT_PREFIX | (0x0c << 8) | (2 << 20) | (2 << 24);
        assert_eq!(
            decode_texture_rgba(dxt1 | (1 << 2), 0, &[0; 8]),
            Err(TextureDecodeError::UnsupportedLayout)
        );
        let too_large = FORMAT_PREFIX | (0x0c << 8) | (12 << 20) | (12 << 24);
        assert_eq!(
            decode_texture_rgba(too_large, 0, &[0; 8]),
            Err(TextureDecodeError::InvalidExtent)
        );
    }
}
