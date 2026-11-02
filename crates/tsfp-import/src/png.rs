//! Minimal PNG writer (8-bit, stored deflate blocks) and the Xbox Morton unswizzle.
//! Stored deflate keeps the encoder tiny and byte-deterministic. Size is not a goal of stage B.
use tsfp_format_core::crc32;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Color {
    Gray = 0,
    Rgb = 2,
    GrayAlpha = 4,
    Rgba = 6,
}

impl Color {
    pub fn channels(self) -> usize {
        match self {
            Color::Gray => 1,
            Color::GrayAlpha => 2,
            Color::Rgb => 3,
            Color::Rgba => 4,
        }
    }
}

fn adler32(b: &[u8]) -> u32 {
    let (mut a, mut s) = (1u32, 0u32);
    for x in b {
        a = (a + *x as u32) % 65521;
        s = (s + a) % 65521;
    }
    (s << 16) | a
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend((data.len() as u32).to_be_bytes());
    let mut body = kind.to_vec();
    body.extend(data);
    out.extend(&body);
    out.extend(crc32(&body).to_be_bytes());
}

/// Encode `pixels` (row-major, `color.channels()` bytes per pixel) as a PNG.
/// Each scanline uses filter 0 and the zlib stream uses stored deflate blocks.
///
/// ```
/// use tsfp_import::png::{encode, Color};
/// let bytes = encode(1, 1, Color::Rgba, &[10, 20, 30, 255]);
/// assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
/// ```
pub fn encode(w: usize, h: usize, color: Color, pixels: &[u8]) -> Vec<u8> {
    let stride = w * color.channels();
    assert_eq!(pixels.len(), stride * h);
    let mut raw = Vec::with_capacity((stride + 1) * h);
    for row in pixels.chunks(stride) {
        raw.push(0);
        raw.extend(row);
    }
    let mut z = vec![0x78, 0x01];
    let mut blocks = raw.chunks(65535).peekable();
    while let Some(b) = blocks.next() {
        z.push(u8::from(blocks.peek().is_none()));
        z.extend((b.len() as u16).to_le_bytes());
        z.extend((!(b.len() as u16)).to_le_bytes());
        z.extend(b);
    }
    z.extend(adler32(&raw).to_be_bytes());
    let mut ihdr = Vec::new();
    ihdr.extend((w as u32).to_be_bytes());
    ihdr.extend((h as u32).to_be_bytes());
    ihdr.extend([8, color as u8, 0, 0, 0]);
    let mut o = b"\x89PNG\r\n\x1a\n".to_vec();
    chunk(&mut o, b"IHDR", &ihdr);
    chunk(&mut o, b"IDAT", &z);
    chunk(&mut o, b"IEND", &[]);
    o
}

/// Decode the PNG emitted by [`encode`] and prove its texels match the source buffer.
/// This intentionally accepts the importer's deterministic filter-0/stored-deflate subset.
pub fn verify_encoded_pixels(
    bytes: &[u8],
    width: usize,
    height: usize,
    color: Color,
    pixels: &[u8],
) -> crate::Result<()> {
    if bytes.get(..8) != Some(b"\x89PNG\r\n\x1a\n") {
        return Err("png: bad signature".into());
    }
    let stride = width
        .checked_mul(color.channels())
        .ok_or_else(|| "png: row size overflow".to_string())?;
    let raw_size = stride
        .checked_add(1)
        .and_then(|row| row.checked_mul(height))
        .ok_or_else(|| "png: decoded size overflow".to_string())?;
    if pixels.len() != stride * height {
        return Err("png: source pixel size mismatch".into());
    }

    let mut cursor = 8usize;
    let mut header_seen = false;
    let mut end_seen = false;
    let mut compressed = Vec::new();
    while cursor < bytes.len() {
        let chunk_header = bytes
            .get(cursor..cursor + 8)
            .ok_or("png: truncated chunk header")?;
        let len = u32::from_be_bytes(chunk_header[..4].try_into().unwrap()) as usize;
        let kind = &chunk_header[4..8];
        let data_start = cursor + 8;
        let data_end = data_start
            .checked_add(len)
            .ok_or_else(|| "png: chunk size overflow".to_string())?;
        let crc_end = data_end
            .checked_add(4)
            .ok_or_else(|| "png: chunk size overflow".to_string())?;
        let data = bytes
            .get(data_start..data_end)
            .ok_or("png: truncated chunk")?;
        let expected_crc = u32::from_be_bytes(
            bytes
                .get(data_end..crc_end)
                .ok_or("png: missing chunk CRC")?
                .try_into()
                .unwrap(),
        );
        if crc32(&bytes[cursor + 4..data_end]) != expected_crc {
            return Err("png: chunk CRC mismatch".into());
        }
        match kind {
            b"IHDR" if !header_seen && data.len() == 13 => {
                let got_width = u32::from_be_bytes(data[..4].try_into().unwrap()) as usize;
                let got_height = u32::from_be_bytes(data[4..8].try_into().unwrap()) as usize;
                if got_width != width
                    || got_height != height
                    || data[8..] != [8, color as u8, 0, 0, 0]
                {
                    return Err("png: unexpected image header".into());
                }
                header_seen = true;
            }
            b"IDAT" if header_seen && !end_seen => compressed.extend_from_slice(data),
            b"IEND" if data.is_empty() && header_seen => end_seen = true,
            _ => return Err("png: unexpected chunk".into()),
        }
        cursor = crc_end;
        if end_seen {
            break;
        }
    }
    if !end_seen || cursor != bytes.len() || compressed.len() < 6 {
        return Err("png: incomplete image".into());
    }
    if compressed[..2] != [0x78, 0x01] {
        return Err("png: unexpected zlib header".into());
    }
    let deflate_end = compressed.len() - 4;
    let (raw, consumed) = crate::inflate::inflate(&compressed[2..deflate_end], raw_size)?;
    if consumed != deflate_end - 2 || raw.len() != raw_size {
        return Err("png: decoded stream length mismatch".into());
    }
    if u32::from_be_bytes(compressed[deflate_end..].try_into().unwrap()) != adler32(&raw) {
        return Err("png: Adler-32 mismatch".into());
    }
    for row in 0..height {
        let raw_start = row * (stride + 1);
        if raw[raw_start] != 0
            || raw[raw_start + 1..raw_start + stride + 1]
                != pixels[row * stride..(row + 1) * stride]
        {
            return Err(format!("png: decoded texels differ on row {row}"));
        }
    }
    Ok(())
}

/// Byte offset-free Morton index of texel (x, y) in a `w` x `h` power-of-two surface. The NV2A
/// interleaves x and y bits (x in the even positions) while both dimensions have bits left,
/// then the remaining high bits of the longer side continue linearly.
pub fn morton(x: usize, y: usize, w: usize, h: usize) -> usize {
    let (mut idx, mut out_bit, mut xb, mut yb) = (0usize, 0, 0, 0);
    let (wbits, hbits) = (w.trailing_zeros() as usize, h.trailing_zeros() as usize);
    while xb < wbits || yb < hbits {
        if xb < wbits {
            idx |= ((x >> xb) & 1) << out_bit;
            out_bit += 1;
            xb += 1;
        }
        if yb < hbits {
            idx |= ((y >> yb) & 1) << out_bit;
            out_bit += 1;
            yb += 1;
        }
    }
    idx
}

/// Unswizzle a `w` x `h` surface of `bpp`-byte texels into row-major order.
pub fn unswizzle(src: &[u8], w: usize, h: usize, bpp: usize) -> Vec<u8> {
    let mut out = vec![0u8; w * h * bpp];
    for y in 0..h {
        for x in 0..w {
            let s = morton(x, y, w, h) * bpp;
            let d = (y * w + x) * bpp;
            out[d..d + bpp].copy_from_slice(&src[s..s + bpp]);
        }
    }
    out
}
