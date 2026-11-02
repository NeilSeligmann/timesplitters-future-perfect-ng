//! RFC 1951 inflate and RFC 1952 gzip unwrapping (dependency-free, bounds-checked).
use crate::Result;

struct Bits<'a> {
    d: &'a [u8],
    pos: usize,
    bit: u32,
    n: u32,
}
impl Bits<'_> {
    fn need(&mut self, n: u32) -> Result<()> {
        while self.n < n {
            let b = *self.d.get(self.pos).ok_or("inflate: truncated")?;
            self.pos += 1;
            self.bit |= (b as u32) << self.n;
            self.n += 8;
        }
        Ok(())
    }
    fn get(&mut self, n: u32) -> Result<u32> {
        if n == 0 {
            return Ok(0);
        }
        self.need(n)?;
        let v = self.bit & ((1u32 << n) - 1);
        self.bit >>= n;
        self.n -= n;
        Ok(v)
    }
}

struct Huff {
    count: [u16; 16],
    symbol: Vec<u16>,
}
impl Huff {
    fn new(lengths: &[u8]) -> Result<Self> {
        let mut count = [0u16; 16];
        for l in lengths {
            count[*l as usize] += 1;
        }
        count[0] = 0;
        let mut left = 1i32;
        for c in &count[1..] {
            left = (left << 1) - *c as i32;
            if left < 0 {
                return Err("inflate: over-subscribed code".into());
            }
        }
        let mut offs = [0u16; 16];
        for i in 1..15 {
            offs[i + 1] = offs[i] + count[i];
        }
        let mut symbol = vec![0u16; lengths.len()];
        for (s, l) in lengths.iter().enumerate() {
            if *l != 0 {
                symbol[offs[*l as usize] as usize] = s as u16;
                offs[*l as usize] += 1;
            }
        }
        Ok(Self { count, symbol })
    }
    fn decode(&self, b: &mut Bits) -> Result<u16> {
        let (mut code, mut first, mut index) = (0i32, 0i32, 0i32);
        for len in 1..16 {
            code |= b.get(1)? as i32;
            let c = self.count[len] as i32;
            if code - c < first {
                return Ok(self.symbol[(index + (code - first)) as usize]);
            }
            index += c;
            first = (first + c) << 1;
            code <<= 1;
        }
        Err("inflate: bad code".into())
    }
}

const LBASE: [u16; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131,
    163, 195, 227, 258,
];
const LEXT: [u8; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];
const DBASE: [u16; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];
const DEXT: [u8; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13,
    13,
];

/// Inflate a raw deflate stream; returns the data and the bytes consumed. `limit` caps output.
pub fn inflate(d: &[u8], limit: usize) -> Result<(Vec<u8>, usize)> {
    let mut b = Bits {
        d,
        pos: 0,
        bit: 0,
        n: 0,
    };
    let mut out = Vec::new();
    loop {
        let last = b.get(1)?;
        match b.get(2)? {
            0 => {
                b.bit = 0;
                b.n = 0;
                let h = d.get(b.pos..b.pos + 4).ok_or("inflate: truncated stored")?;
                let len = u16::from_le_bytes([h[0], h[1]]) as usize;
                if len as u16 != !u16::from_le_bytes([h[2], h[3]]) {
                    return Err("inflate: stored length check".into());
                }
                let s = d
                    .get(b.pos + 4..b.pos + 4 + len)
                    .ok_or("inflate: truncated stored")?;
                out.extend_from_slice(s);
                b.pos += 4 + len;
            }
            t @ (1 | 2) => {
                let (lit, dist) = if t == 1 {
                    let mut l = [8u8; 288];
                    l[144..256].fill(9);
                    l[256..280].fill(7);
                    (Huff::new(&l)?, Huff::new(&[5u8; 30])?)
                } else {
                    let (nl, nd, nc) = (
                        b.get(5)? as usize + 257,
                        b.get(5)? as usize + 1,
                        b.get(4)? as usize + 4,
                    );
                    const ORD: [usize; 19] = [
                        16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
                    ];
                    let mut cl = [0u8; 19];
                    for o in ORD.iter().take(nc) {
                        cl[*o] = b.get(3)? as u8;
                    }
                    let ch = Huff::new(&cl)?;
                    let mut lens = vec![0u8; nl + nd];
                    let mut i = 0;
                    while i < nl + nd {
                        let s = ch.decode(&mut b)?;
                        if s < 16 {
                            lens[i] = s as u8;
                            i += 1;
                        } else {
                            let (v, r) = match s {
                                16 => (
                                    *lens.get(i.wrapping_sub(1)).ok_or("inflate: repeat first")?,
                                    3 + b.get(2)?,
                                ),
                                17 => (0, 3 + b.get(3)?),
                                _ => (0, 11 + b.get(7)?),
                            };
                            if i + r as usize > nl + nd {
                                return Err("inflate: repeat overflow".into());
                            }
                            lens[i..i + r as usize].fill(v);
                            i += r as usize;
                        }
                    }
                    (Huff::new(&lens[..nl])?, Huff::new(&lens[nl..])?)
                };
                loop {
                    let s = lit.decode(&mut b)? as usize;
                    if s < 256 {
                        out.push(s as u8);
                    } else if s == 256 {
                        break;
                    } else {
                        let s = s - 257;
                        if s >= 29 {
                            return Err("inflate: bad length".into());
                        }
                        let len = LBASE[s] as usize + b.get(LEXT[s] as u32)? as usize;
                        let ds = dist.decode(&mut b)? as usize;
                        if ds >= 30 {
                            return Err("inflate: bad distance".into());
                        }
                        let dd = DBASE[ds] as usize + b.get(DEXT[ds] as u32)? as usize;
                        if dd > out.len() {
                            return Err("inflate: distance too far".into());
                        }
                        for _ in 0..len {
                            out.push(out[out.len() - dd]);
                        }
                    }
                    if out.len() > limit {
                        return Err("inflate: output exceeds limit".into());
                    }
                }
            }
            _ => return Err("inflate: reserved block type".into()),
        }
        if out.len() > limit {
            return Err("inflate: output exceeds limit".into());
        }
        if last == 1 {
            return Ok((out, b.pos));
        }
    }
}

/// Unwrap a gzip member, verifying CRC32 and ISIZE.
pub fn gunzip(d: &[u8], limit: usize) -> Result<Vec<u8>> {
    if d.len() < 18 || d[0] != 0x1f || d[1] != 0x8b || d[2] != 8 {
        return Err("gzip: bad header".into());
    }
    let flg = d[3];
    let mut p = 10;
    if flg & 4 != 0 {
        p += 2 + u16::from_le_bytes([d[p], d[p + 1]]) as usize;
    }
    for bit in [8u8, 16] {
        if flg & bit != 0 {
            p += d
                .get(p..)
                .ok_or("gzip: truncated")?
                .iter()
                .position(|x| *x == 0)
                .ok_or("gzip: unterminated")?
                + 1;
        }
    }
    if flg & 2 != 0 {
        p += 2;
    }
    let (out, used) = inflate(d.get(p..).ok_or("gzip: truncated")?, limit)?;
    let t = d
        .get(p + used..p + used + 8)
        .ok_or("gzip: truncated trailer")?;
    if u32::from_le_bytes(t[..4].try_into().unwrap()) != tsfp_format_core::crc32(&out)
        || u32::from_le_bytes(t[4..].try_into().unwrap()) != out.len() as u32
    {
        return Err("gzip: crc/size mismatch".into());
    }
    Ok(out)
}
