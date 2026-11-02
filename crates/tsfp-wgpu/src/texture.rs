// SPDX-License-Identifier: GPL-3.0-or-later
//! NV2A texture decode to RGBA8 (row 0 at top). Pure Rust, no wgpu.
//!
//! Evidence: formats and swizzle follow xemu (`generate_swizzle_masks`, `unswizzle`),
//! xemu-level. INFERRED choices: A8Y8 is read as little-endian u16 with the low byte
//! as luma and the high byte as alpha; YUY2 (0x24 and 0x25) is a row-major packed
//! `Y0 U Y1 V` stream (0x24 sized from the log2 fields, treated as not swizzled)
//! converted with BT.601 full-range; P8 (0x0B) is shown as grey (no palette applied).

/// Decoded texture, row 0 at the top.
pub struct Decoded {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Replay image address of texel data: the Xbox physical alias `0x80000000 | physical` (same rule as
/// `render_trace::texture_data_alias` in tsfp-ports).
pub fn data_address(data: u32) -> u32 {
    (data & 0x0FFF_FFFF) | 0x8000_0000
}

/// Name of a format code = `(format_word >> 8) & 0xFF`.
pub fn format_name(code: u32) -> &'static str {
    match code {
        0x00 => "Y8",
        0x01 => "AY8",
        0x02 => "A1R5G5B5",
        0x03 => "X1R5G5B5",
        0x04 => "A4R4G4B4",
        0x05 => "R5G6B5",
        0x06 => "A8R8G8B8",
        0x07 => "X8R8G8B8",
        0x0B => "P8",
        0x0C => "DXT1",
        0x0E => "DXT3",
        0x0F => "DXT5",
        0x10 => "LIN_A1R5G5B5",
        0x11 => "LIN_R5G6B5",
        0x12 => "LIN_A8R8G8B8",
        0x13 => "LIN_Y8",
        0x19 => "A8",
        0x1A => "A8Y8",
        0x1B => "LIN_AY8",
        0x1C => "LIN_X1R5G5B5",
        0x1D => "LIN_A4R4G4B4",
        0x1E => "LIN_X8R8G8B8",
        0x1F => "LIN_A8",
        0x20 => "LIN_A8Y8",
        0x24 => "YUY2",
        0x25 => "LIN_YUY2",
        _ => "UNKNOWN",
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Y8,
    Ay8,
    A1R5G5B5,
    X1R5G5B5,
    A4R4G4B4,
    R5G6B5,
    A8R8G8B8,
    X8R8G8B8,
    A8,
    A8Y8,
    Yuy2,
    Dxt1,
    Dxt3,
    Dxt5,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Layout {
    Swizzled,
    Linear,
    Block,
}

fn classify(code: u32) -> Option<(Kind, Layout)> {
    use Kind::*;
    use Layout::*;
    Some(match code {
        0x00 | 0x0B => (Y8, Swizzled),
        0x01 => (Ay8, Swizzled),
        0x02 => (A1R5G5B5, Swizzled),
        0x03 => (X1R5G5B5, Swizzled),
        0x04 => (A4R4G4B4, Swizzled),
        0x05 => (R5G6B5, Swizzled),
        0x06 => (A8R8G8B8, Swizzled),
        0x07 => (X8R8G8B8, Swizzled),
        0x0C => (Dxt1, Block),
        0x0E => (Dxt3, Block),
        0x0F => (Dxt5, Block),
        0x19 => (A8, Swizzled),
        0x1A => (A8Y8, Swizzled),
        0x24 => (Yuy2, Swizzled),
        0x10 => (A1R5G5B5, Linear),
        0x11 => (R5G6B5, Linear),
        0x12 => (A8R8G8B8, Linear),
        0x13 => (Y8, Linear),
        0x1B => (Ay8, Linear),
        0x1C => (X1R5G5B5, Linear),
        0x1D => (A4R4G4B4, Linear),
        0x1E => (X8R8G8B8, Linear),
        0x1F => (A8, Linear),
        0x20 => (A8Y8, Linear),
        0x25 => (Yuy2, Linear),
        _ => return None,
    })
}

fn bytes_per_texel(kind: Kind) -> u32 {
    match kind {
        Kind::Y8 | Kind::Ay8 | Kind::A8 => 1,
        Kind::A8R8G8B8 | Kind::X8R8G8B8 => 4,
        Kind::Dxt1 | Kind::Dxt3 | Kind::Dxt5 => 0,
        _ => 2,
    }
}

struct Geometry {
    kind: Kind,
    layout: Layout,
    width: u32,
    height: u32,
    /// Bytes per row for linear and YUY2 data.
    pitch: u32,
    bytes: u32,
}

fn geometry(format_word: u32, size_word: u32) -> Option<Geometry> {
    let (kind, layout) = classify((format_word >> 8) & 0xFF)?;
    let log_w = (format_word >> 20) & 0xF;
    let log_h = (format_word >> 24) & 0xF;
    let (width, height, pitch) = if layout == Layout::Linear {
        let w = (size_word & 0xFFF) + 1;
        let h = ((size_word >> 12) & 0xFFF) + 1;
        (w, h, (((size_word >> 24) & 0xFF) + 1) * 64)
    } else {
        let w = 1u32 << log_w;
        let h = 1u32 << log_h;
        let bpp = if kind == Kind::Yuy2 {
            2
        } else {
            bytes_per_texel(kind)
        };
        (w, h, w * bpp)
    };
    let bytes = match layout {
        Layout::Block => {
            let block = if kind == Kind::Dxt1 { 8 } else { 16 };
            width.div_ceil(4) * height.div_ceil(4) * block
        }
        Layout::Linear | Layout::Swizzled => pitch.checked_mul(height)?,
    };
    Some(Geometry {
        kind,
        layout,
        width,
        height,
        pitch,
        bytes,
    })
}

/// Dimensions and byte length of the base level, `None` for an unknown format.
pub fn base_level(format_word: u32, size_word: u32) -> Option<(u32, u32, u32)> {
    let g = geometry(format_word, size_word)?;
    Some((g.width, g.height, g.bytes))
}

/// True for the linear (non swizzled) colour formats: their texture coordinates are unnormalised texel units.
pub fn is_linear(format_word: u32) -> bool {
    matches!(
        classify((format_word >> 8) & 0xFF),
        Some((_, Layout::Linear))
    )
}

/// Byte length of the base level.
pub fn data_length(format_word: u32, size_word: u32) -> Option<u32> {
    base_level(format_word, size_word).map(|(_, _, bytes)| bytes)
}

/// xemu `generate_swizzle_masks`: returns (x_mask, y_mask).
fn swizzle_masks(width: u32, height: u32) -> (u32, u32) {
    let (mut x_mask, mut y_mask) = (0u32, 0u32);
    let (mut bit, mut mask_bit) = (1u32, 1u32);
    while bit < width || bit < height {
        if bit < width {
            x_mask |= mask_bit;
            mask_bit <<= 1;
        }
        if bit < height {
            y_mask |= mask_bit;
            mask_bit <<= 1;
        }
        bit <<= 1;
    }
    (x_mask, y_mask)
}

/// Deposit the low bits of `value` into the set bits of `mask`, lowest first.
fn pdep(mut value: u32, mut mask: u32) -> u32 {
    let mut out = 0;
    while mask != 0 {
        let low = mask & mask.wrapping_neg();
        if value & 1 != 0 {
            out |= low;
        }
        value >>= 1;
        mask &= mask - 1;
    }
    out
}

/// Texel index of (x, y) in a swizzled surface.
pub fn swizzle_index(x: u32, y: u32, width: u32, height: u32) -> u32 {
    let (xm, ym) = swizzle_masks(width, height);
    pdep(x, xm) | pdep(y, ym)
}

fn e5(v: u32) -> u8 {
    ((v << 3) | (v >> 2)) as u8
}
fn e6(v: u32) -> u8 {
    ((v << 2) | (v >> 4)) as u8
}
fn e4(v: u32) -> u8 {
    (v * 17) as u8
}

fn rgb565(v: u16) -> [u8; 3] {
    let v = u32::from(v);
    [e5((v >> 11) & 31), e6((v >> 5) & 63), e5(v & 31)]
}

fn clamp8(v: i32) -> u8 {
    v.clamp(0, 255) as u8
}

fn yuv(y: u8, u: u8, v: u8) -> [u8; 3] {
    let (y, u, v) = (i32::from(y), i32::from(u) - 128, i32::from(v) - 128);
    [
        clamp8(y + ((359 * v) >> 8)),
        clamp8(y - ((88 * u + 183 * v) >> 8)),
        clamp8(y + ((454 * u) >> 8)),
    ]
}

fn texel(kind: Kind, t: &[u8]) -> [u8; 4] {
    let w16 = || u32::from(u16::from_le_bytes([t[0], t[1]]));
    match kind {
        Kind::Y8 => [t[0], t[0], t[0], 255],
        Kind::Ay8 => [t[0], t[0], t[0], t[0]],
        Kind::A8 => [255, 255, 255, t[0]],
        Kind::A8Y8 => [t[0], t[0], t[0], t[1]],
        Kind::A1R5G5B5 | Kind::X1R5G5B5 => {
            let v = w16();
            let a = if kind == Kind::X1R5G5B5 || v & 0x8000 != 0 {
                255
            } else {
                0
            };
            [e5((v >> 10) & 31), e5((v >> 5) & 31), e5(v & 31), a]
        }
        Kind::A4R4G4B4 => {
            let v = w16();
            [
                e4((v >> 8) & 15),
                e4((v >> 4) & 15),
                e4(v & 15),
                e4((v >> 12) & 15),
            ]
        }
        Kind::R5G6B5 => {
            let [r, g, b] = rgb565(w16() as u16);
            [r, g, b, 255]
        }
        Kind::A8R8G8B8 => [t[2], t[1], t[0], t[3]],
        Kind::X8R8G8B8 => [t[2], t[1], t[0], 255],
        _ => [0, 0, 0, 255],
    }
}

fn decode_block_colors(block: &[u8], dxt1: bool) -> [[u8; 4]; 4] {
    let c0 = u16::from_le_bytes([block[0], block[1]]);
    let c1 = u16::from_le_bytes([block[2], block[3]]);
    let (a, b) = (rgb565(c0), rgb565(c1));
    let mix = |wa: u32, wb: u32, d: u32, i: usize| {
        ((wa * u32::from(a[i]) + wb * u32::from(b[i])) / d) as u8
    };
    let mut p = [[0u8; 4]; 4];
    p[0] = [a[0], a[1], a[2], 255];
    p[1] = [b[0], b[1], b[2], 255];
    if !dxt1 || c0 > c1 {
        p[2] = [mix(2, 1, 3, 0), mix(2, 1, 3, 1), mix(2, 1, 3, 2), 255];
        p[3] = [mix(1, 2, 3, 0), mix(1, 2, 3, 1), mix(1, 2, 3, 2), 255];
    } else {
        p[2] = [mix(1, 1, 2, 0), mix(1, 1, 2, 1), mix(1, 1, 2, 2), 255];
        p[3] = [0, 0, 0, 0];
    }
    p
}

fn bc3_alpha(block: &[u8]) -> [u8; 16] {
    let (a0, a1) = (u32::from(block[0]), u32::from(block[1]));
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
    for (i, b) in block[2..8].iter().enumerate() {
        bits |= u64::from(*b) << (8 * i);
    }
    let mut out = [0u8; 16];
    for (i, o) in out.iter_mut().enumerate() {
        *o = table[((bits >> (3 * i)) & 7) as usize] as u8;
    }
    out
}

fn decode_dxt(g: &Geometry, data: &[u8]) -> Vec<u8> {
    let (w, h) = (g.width as usize, g.height as usize);
    let bw = w.div_ceil(4);
    let block_size = if g.kind == Kind::Dxt1 { 8 } else { 16 };
    let mut rgba = vec![0u8; w * h * 4];
    for by in 0..h.div_ceil(4) {
        for bx in 0..bw {
            let block = &data[(by * bw + bx) * block_size..][..block_size];
            let color_block = if g.kind == Kind::Dxt1 {
                block
            } else {
                &block[8..]
            };
            let pal = decode_block_colors(color_block, g.kind == Kind::Dxt1);
            let alpha: [u8; 16] = match g.kind {
                Kind::Dxt3 => {
                    core::array::from_fn(|i| e4(u32::from(block[i / 2] >> (4 * (i % 2))) & 15))
                }
                Kind::Dxt5 => bc3_alpha(block),
                _ => [255; 16],
            };
            let idx = u32::from_le_bytes([
                color_block[4],
                color_block[5],
                color_block[6],
                color_block[7],
            ]);
            for (i, &a) in alpha.iter().enumerate() {
                let (x, y) = (bx * 4 + i % 4, by * 4 + i / 4);
                if x >= w || y >= h {
                    continue;
                }
                let mut px = pal[((idx >> (2 * i)) & 3) as usize];
                if g.kind != Kind::Dxt1 {
                    px[3] = a;
                }
                rgba[(y * w + x) * 4..][..4].copy_from_slice(&px);
            }
        }
    }
    rgba
}

/// Decode the base level from `data` (at least `bytes` of texel data). `None` if unknown or short.
pub fn decode(format_word: u32, size_word: u32, data: &[u8]) -> Option<Decoded> {
    let g = geometry(format_word, size_word)?;
    if g.width == 0 || g.height == 0 || data.len() < g.bytes as usize {
        return None;
    }
    let (w, h) = (g.width as usize, g.height as usize);
    let rgba = match g.kind {
        Kind::Dxt1 | Kind::Dxt3 | Kind::Dxt5 => decode_dxt(&g, data),
        Kind::Yuy2 => {
            let mut out = vec![0u8; w * h * 4];
            for y in 0..h {
                let row = &data[y * g.pitch as usize..];
                for x in 0..w {
                    let pair = (x & !1) * 2;
                    // A trailing odd column reads past a 2-byte pair, so clamp its chroma.
                    let (u, v) = (
                        row.get(pair + 1).copied().unwrap_or(128),
                        row.get(pair + 3).copied().unwrap_or(128),
                    );
                    let [r, gr, b] = yuv(row[x * 2], u, v);
                    out[(y * w + x) * 4..][..4].copy_from_slice(&[r, gr, b, 255]);
                }
            }
            out
        }
        kind => {
            let bpp = bytes_per_texel(kind) as usize;
            let (xm, ym) = swizzle_masks(g.width, g.height);
            let mut out = vec![0u8; w * h * 4];
            for y in 0..h {
                for x in 0..w {
                    let offset = if g.layout == Layout::Swizzled {
                        (pdep(x as u32, xm) | pdep(y as u32, ym)) as usize * bpp
                    } else {
                        y * g.pitch as usize + x * bpp
                    };
                    let px = texel(kind, &data[offset..offset + bpp]);
                    out[(y * w + x) * 4..][..4].copy_from_slice(&px);
                }
            }
            out
        }
    };
    Some(Decoded {
        width: g.width,
        height: g.height,
        rgba,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fw(code: u32, lu: u32, lv: u32) -> u32 {
        (code << 8) | (lu << 20) | (lv << 24)
    }

    #[test]
    fn address_and_names() {
        assert_eq!(data_address(0x0012_3456), 0x8012_3456);
        assert_eq!(data_address(0x8012_3456), 0x8012_3456);
        assert_eq!(format_name(0x06), "A8R8G8B8");
        assert_eq!(format_name(0x99), "UNKNOWN");
    }

    #[test]
    fn swizzle_4x4() {
        assert_eq!(swizzle_masks(4, 4), (0b0101, 0b1010));
        assert_eq!(swizzle_index(1, 0, 4, 4), 1);
        assert_eq!(swizzle_index(0, 1, 4, 4), 2);
        assert_eq!(swizzle_index(2, 1, 4, 4), 6);
        assert_eq!(swizzle_index(3, 3, 4, 4), 15);
    }

    #[test]
    fn swizzle_8x2_nonsquare() {
        // bit1: x->1, y->2; bit2: x->4 (y exhausted); bit4: x->8.
        assert_eq!(swizzle_masks(8, 2), (0b1101, 0b0010));
        assert_eq!(swizzle_index(1, 0, 8, 2), 1);
        assert_eq!(swizzle_index(0, 1, 8, 2), 2);
        assert_eq!(swizzle_index(2, 0, 8, 2), 4);
        assert_eq!(swizzle_index(7, 1, 8, 2), 15);
        assert_eq!(swizzle_index(5, 1, 8, 2), 0b1011);
    }

    #[test]
    fn a8r8g8b8_swizzled_2x2() {
        // masks x=1,y=2: memory order (0,0),(1,0),(0,1),(1,1); bytes B,G,R,A.
        let data = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
        let d = decode(fw(0x06, 1, 1), 0, &data).unwrap();
        assert_eq!((d.width, d.height), (2, 2));
        assert_eq!(&d.rgba[0..4], &[3, 2, 1, 4]);
        assert_eq!(&d.rgba[4..8], &[7, 6, 5, 8]);
        assert_eq!(&d.rgba[8..12], &[11, 10, 9, 12]);
        assert_eq!(&d.rgba[12..16], &[15, 14, 13, 16]);
    }

    #[test]
    fn small_formats_single_pixel() {
        let one = |code: u32, v: u16| decode(fw(code, 0, 0), 0, &v.to_le_bytes()).unwrap().rgba;
        assert_eq!(one(0x05, 0xF800), [255, 0, 0, 255]);
        assert_eq!(one(0x05, 0x07E0), [0, 255, 0, 255]);
        assert_eq!(one(0x05, 0x001F), [0, 0, 255, 255]);
        // 0x8410: r=16 g=32 b=16 -> 132, 130, 132
        assert_eq!(one(0x05, 0x8410), [132, 130, 132, 255]);
        // A4R4G4B4 0x8421: a=8->136, r=4->68, g=2->34, b=1->17
        assert_eq!(one(0x04, 0x8421), [68, 34, 17, 136]);
        assert_eq!(one(0x02, 0xFC00), [255, 0, 0, 255]);
        assert_eq!(one(0x02, 0x7C00), [255, 0, 0, 0]);
        assert_eq!(one(0x03, 0x7C00), [255, 0, 0, 255]);
        assert_eq!(
            decode(fw(0x00, 0, 0), 0, &[77]).unwrap().rgba,
            [77, 77, 77, 255]
        );
        assert_eq!(
            decode(fw(0x19, 0, 0), 0, &[77]).unwrap().rgba,
            [255, 255, 255, 77]
        );
    }

    #[test]
    fn dxt1_four_colour_and_transparent() {
        // c0 = red 0xF800 > c1 = blue 0x001F: 4 colour mode; row 0 indices 0,1,2,3.
        let mut b = vec![0x00, 0xF8, 0x1F, 0x00];
        b.extend_from_slice(&[0xE4, 0, 0, 0]);
        let d = decode(fw(0x0C, 2, 2), 0, &b).unwrap();
        assert_eq!(&d.rgba[0..4], &[255, 0, 0, 255]);
        assert_eq!(&d.rgba[4..8], &[0, 0, 255, 255]);
        assert_eq!(&d.rgba[8..12], &[170, 0, 85, 255]);
        assert_eq!(&d.rgba[12..16], &[85, 0, 170, 255]);
        // c0 = blue <= c1 = red: 3 colour mode, index 3 transparent.
        let mut b = vec![0x1F, 0x00, 0x00, 0xF8];
        b.extend_from_slice(&[0xE4, 0, 0, 0]);
        let d = decode(fw(0x0C, 2, 2), 0, &b).unwrap();
        assert_eq!(&d.rgba[0..4], &[0, 0, 255, 255]);
        assert_eq!(&d.rgba[8..12], &[127, 0, 127, 255]);
        assert_eq!(&d.rgba[12..16], &[0, 0, 0, 0]);
    }

    #[test]
    fn dxt5_alpha_modes() {
        let colors = [0x00, 0xF8, 0x00, 0xF8, 0, 0, 0, 0];
        // 8 value mode a0=255 a1=0: idx 2 -> 6*255/7=218, idx 7 -> 255/7=36
        let bits: u64 = (1 << 3) | (2 << 6) | (7 << 9);
        let mut blk = vec![255, 0];
        blk.extend_from_slice(&bits.to_le_bytes()[..6]);
        blk.extend_from_slice(&colors);
        let d = decode(fw(0x0F, 2, 2), 0, &blk).unwrap();
        let alphas: Vec<u8> = (0..4).map(|i| d.rgba[i * 4 + 3]).collect();
        assert_eq!(alphas, [255, 0, 218, 36]);
        // 6 value mode a0=0 a1=100: idx2 -> 20, idx6 -> 0, idx7 -> 255
        let bits: u64 = 2 | (6 << 3) | (7 << 6);
        let mut blk = vec![0, 100];
        blk.extend_from_slice(&bits.to_le_bytes()[..6]);
        blk.extend_from_slice(&colors);
        let d = decode(fw(0x0F, 2, 2), 0, &blk).unwrap();
        let alphas: Vec<u8> = (0..3).map(|i| d.rgba[i * 4 + 3]).collect();
        assert_eq!(alphas, [20, 0, 255]);
    }

    #[test]
    fn dxt3_explicit_alpha() {
        let mut blk = vec![0x21, 0xF0, 0, 0, 0, 0, 0, 0];
        blk.extend_from_slice(&[0x00, 0xF8, 0x00, 0xF8, 0, 0, 0, 0]);
        let d = decode(fw(0x0E, 2, 2), 0, &blk).unwrap();
        let alphas: Vec<u8> = (0..4).map(|i| d.rgba[i * 4 + 3]).collect();
        assert_eq!(alphas, [17, 34, 0, 255]);
    }

    #[test]
    fn linear_with_pitch() {
        // 2x2 R5G6B5 linear: w-1=1, h-1=1<<12, pitch=(0+1)*64.
        let size = 1 | (1 << 12);
        let format = 0x11 << 8;
        assert_eq!(base_level(format, size), Some((2, 2, 128)));
        let mut data = vec![0u8; 128];
        data[0..2].copy_from_slice(&0xF800u16.to_le_bytes());
        data[2..4].copy_from_slice(&0x07E0u16.to_le_bytes());
        data[64..66].copy_from_slice(&0x001Fu16.to_le_bytes());
        let d = decode(format, size, &data).unwrap();
        assert_eq!(&d.rgba[0..4], &[255, 0, 0, 255]);
        assert_eq!(&d.rgba[4..8], &[0, 255, 0, 255]);
        assert_eq!(&d.rgba[8..12], &[0, 0, 255, 255]);
    }

    #[test]
    fn base_level_swizzled_and_dxt() {
        assert_eq!(base_level(fw(0x06, 3, 2), 0), Some((8, 4, 128)));
        assert_eq!(base_level(fw(0x0C, 3, 3), 0), Some((8, 8, 32)));
        assert_eq!(base_level(fw(0x0F, 0, 0), 0), Some((1, 1, 16)));
        assert_eq!(data_length(fw(0x0F, 3, 3), 0), Some(64));
    }

    #[test]
    fn unknown_and_short() {
        assert!(decode(fw(0x7F, 2, 2), 0, &[0; 256]).is_none());
        assert!(base_level(fw(0x7F, 2, 2), 0).is_none());
        assert!(decode(fw(0x06, 2, 2), 0, &[0; 63]).is_none());
        assert!(decode(fw(0x0C, 2, 2), 0, &[0; 7]).is_none());
    }

    #[test]
    fn yuy2_grey_linear() {
        // width 2, height 1, pitch 64
        let mut data = vec![0u8; 64];
        data[..4].copy_from_slice(&[100, 128, 200, 128]);
        let d = decode(0x25 << 8, 1, &data).unwrap();
        assert_eq!(&d.rgba[0..8], &[100, 100, 100, 255, 200, 200, 200, 255]);
    }
}
