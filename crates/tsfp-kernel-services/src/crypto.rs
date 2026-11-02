// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{GuestMemory, INVALID_PARAMETER, SUCCESS};

/// Standard SHA-1 state; guest serialization is the host model's own layout.
#[derive(Clone, Debug)]
pub struct Sha1 {
    chain: [u32; 5],
    bits: u64,
    pending: usize,
    block: [u8; 64],
}
impl Default for Sha1 {
    fn default() -> Self {
        Self {
            chain: [0x67452301, 0xefcdab89, 0x98badcfe, 0x10325476, 0xc3d2e1f0],
            bits: 0,
            pending: 0,
            block: [0; 64],
        }
    }
}
impl Sha1 {
    fn compress(&mut self, block: &[u8; 64]) {
        let mut words = [0u32; 80];
        for (i, chunk) in block.chunks_exact(4).enumerate() {
            words[i] = u32::from_be_bytes(chunk.try_into().unwrap());
        }
        for i in 16..80 {
            words[i] = (words[i - 3] ^ words[i - 8] ^ words[i - 14] ^ words[i - 16]).rotate_left(1);
        }
        let [mut a, mut b, mut c, mut d, mut e] = self.chain;
        for (i, word) in words.into_iter().enumerate() {
            let (f, k) = match i {
                0..20 => ((b & c) | (!b & d), 0x5a827999u32),
                20..40 => (b ^ c ^ d, 0x6ed9eba1),
                40..60 => ((b & c) | (b & d) | (c & d), 0x8f1bbcdc),
                _ => (b ^ c ^ d, 0xca62c1d6),
            };
            let t = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(word);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = t;
        }
        for (h, v) in self.chain.iter_mut().zip([a, b, c, d, e]) {
            *h = h.wrapping_add(v);
        }
    }
    pub fn update(&mut self, mut input: &[u8]) {
        self.bits = self.bits.wrapping_add((input.len() as u64).wrapping_mul(8));
        if self.pending != 0 {
            let count = (64 - self.pending).min(input.len());
            self.block[self.pending..self.pending + count].copy_from_slice(&input[..count]);
            self.pending += count;
            input = &input[count..];
            if self.pending == 64 {
                self.compress(&self.block.clone());
                self.pending = 0;
            }
        }
        while input.len() >= 64 {
            self.compress(input[..64].try_into().unwrap());
            input = &input[64..];
        }
        if !input.is_empty() {
            self.block[..input.len()].copy_from_slice(input);
            self.pending = input.len();
        }
    }
    pub fn finish(mut self) -> [u8; 20] {
        let bits = self.bits;
        self.update(&[0x80]);
        while self.pending != 56 {
            let want = if self.pending < 56 {
                56 - self.pending
            } else {
                120 - self.pending
            };
            self.update(&[0; 64][..want]);
        }
        self.update(&bits.to_be_bytes());
        let mut digest = [0; 20];
        for (chunk, word) in digest.chunks_exact_mut(4).zip(self.chain) {
            chunk.copy_from_slice(&word.to_be_bytes());
        }
        digest
    }
    pub fn load(memory: &dyn GuestMemory, address: u32) -> Option<Self> {
        let mut bytes = [0; 100];
        if !memory.readable(address, 100) || !memory.read(address, &mut bytes) {
            return None;
        }
        let word = |i| u32::from_le_bytes(bytes[i..i + 4].try_into().unwrap());
        if word(0) != 0x41485354 || word(32) >= 64 {
            return None;
        }
        let mut chain = [0; 5];
        for (i, h) in chain.iter_mut().enumerate() {
            *h = word(4 + i * 4);
        }
        Some(Self {
            chain,
            bits: u64::from(word(24)) | (u64::from(word(28)) << 32),
            pending: word(32) as usize,
            block: bytes[36..].try_into().unwrap(),
        })
    }
    pub fn store(&self, memory: &mut dyn GuestMemory, address: u32) -> bool {
        if !memory.readable(address, 100) {
            return false;
        }
        // C's write order matters on partially writable guest ranges.
        for (offset, value) in [
            (0, 0x41485354),
            (24, self.bits as u32),
            (28, (self.bits >> 32) as u32),
            (32, self.pending as u32),
        ] {
            if !memory.put_word(address.wrapping_add(offset), value) {
                return false;
            }
        }
        for (i, value) in self.chain.iter().enumerate() {
            if !memory.put_word(address.wrapping_add(4 + i as u32 * 4), *value) {
                return false;
            }
        }
        for (i, byte) in self.block.iter().enumerate() {
            if !memory.write(address.wrapping_add(36 + i as u32), &[*byte]) {
                return false;
            }
        }
        true
    }
}
pub fn sha1(input: &[u8]) -> [u8; 20] {
    let mut state = Sha1::default();
    state.update(input);
    state.finish()
}

/// Xbox export truncates oversized keys; RFC callers explicitly choose hash_long_key.
pub fn hmac(key: &[u8], first: &[u8], second: &[u8], hash_long_key: bool) -> [u8; 20] {
    let mut padded = [0; 64];
    if hash_long_key && key.len() > 64 {
        padded[..20].copy_from_slice(&sha1(key));
    } else {
        let n = key.len().min(64);
        padded[..n].copy_from_slice(&key[..n]);
    }
    let mut inner = Sha1::default();
    inner.update(&padded.map(|v| v ^ 0x36));
    inner.update(first);
    inner.update(second);
    let mut outer = Sha1::default();
    outer.update(&padded.map(|v| v ^ 0x5c));
    outer.update(&inner.finish());
    outer.finish()
}

pub fn rc4_span(length: u32) -> usize {
    let n = length as u8;
    if n == 0 { 256 } else { n as usize }
}

/// Operate in guest memory so key/state overlap is preserved, including counter aliases.
pub fn rc4_key(memory: &mut dyn GuestMemory, state: u32, length: u32, key: u32) -> u32 {
    let span = rc4_span(length);
    if !memory.readable(state, 258) || !memory.readable(key, span) {
        return INVALID_PARAMETER;
    }
    if !memory.write(state + 256, &[0, 0]) {
        return INVALID_PARAMETER;
    }
    for i in 0..256u32 {
        if !memory.write(state + i, &[i as u8]) {
            return INVALID_PARAMETER;
        }
    }
    let mut j = 0u8;
    for i in 0..256u32 {
        let Some(first) = memory.byte(state + i) else {
            return INVALID_PARAMETER;
        };
        let Some(k) = memory.byte(key + (i as usize % span) as u32) else {
            return INVALID_PARAMETER;
        };
        j = j.wrapping_add(first).wrapping_add(k);
        let Some(other) = memory.byte(state + u32::from(j)) else {
            return INVALID_PARAMETER;
        };
        if !memory.write(state + i, &[other]) || !memory.write(state + u32::from(j), &[first]) {
            return INVALID_PARAMETER;
        }
    }
    SUCCESS
}
pub fn rc4_crypt(memory: &mut dyn GuestMemory, state: u32, length: u32, data: u32) -> u32 {
    if !memory.readable(state, 258) || (length != 0 && !memory.readable(data, length as usize)) {
        return INVALID_PARAMETER;
    }
    let (Some(mut i), Some(mut j)) = (memory.byte(state + 256), memory.byte(state + 257)) else {
        return INVALID_PARAMETER;
    };
    let mut n = 0u32;
    while n < length {
        let group = if data & 3 == 0 && length - n >= 4 {
            4
        } else {
            1
        };
        let mut input = [0; 4];
        // Aligned fast path prefetches before the permutation writes.
        if group == 4 && !memory.read(data + n, &mut input[..group]) {
            return INVALID_PARAMETER;
        }
        for byte in &mut input[..group] {
            i = i.wrapping_add(1);
            let Some(first) = memory.byte(state + u32::from(i)) else {
                return INVALID_PARAMETER;
            };
            j = j.wrapping_add(first);
            let Some(other) = memory.byte(state + u32::from(j)) else {
                return INVALID_PARAMETER;
            };
            if !memory.write(state + u32::from(i), &[other])
                || !memory.write(state + u32::from(j), &[first])
            {
                return INVALID_PARAMETER;
            }
            let (Some(a), Some(b)) = (
                memory.byte(state + u32::from(i)),
                memory.byte(state + u32::from(j)),
            ) else {
                return INVALID_PARAMETER;
            };
            let Some(stream) = memory.byte(state + u32::from(a.wrapping_add(b))) else {
                return INVALID_PARAMETER;
            };
            if group == 1 {
                let Some(value) = memory.byte(data + n) else {
                    return INVALID_PARAMETER;
                };
                *byte = value;
            }
            *byte ^= stream;
        }
        if !memory.write(data + n, &input[..group]) {
            return INVALID_PARAMETER;
        }
        n += group as u32;
    }
    if memory.write(state + 256, &[i, j]) {
        SUCCESS
    } else {
        INVALID_PARAMETER
    }
}

pub fn parity(byte: u8) -> u8 {
    let high = byte & 0xfe;
    high | u8::from(high.count_ones() % 2 == 0)
}

/// Validate the complete span before allocating. Zero lengths accept a null source.
fn fetch(memory: &dyn GuestMemory, address: u32, length: u32) -> Option<Vec<u8>> {
    if length == 0 {
        return Some(Vec::new());
    }
    if !memory.readable(address, length as usize) {
        return None;
    }
    let mut out = Vec::new();
    out.try_reserve_exact(length as usize).ok()?;
    out.resize(length as usize, 0);
    memory.read(address, &mut out).then_some(out)
}
#[derive(Debug, Default)]
pub struct CryptoCounters {
    pub sha_init: u32,
    pub sha_update: u32,
    pub sha_final: u32,
    pub hmac: u32,
    pub des: u32,
    pub stale: u32,
    pub oversize: u32,
    pub absorbed: u64,
}
pub fn call(
    ordinal: u32,
    args: &[u32],
    memory: &mut dyn GuestMemory,
    counters: &mut CryptoCounters,
) -> u32 {
    match ordinal {
        335 => {
            if Sha1::default().store(memory, args[0]) {
                counters.sha_init = counters.sha_init.wrapping_add(1);
                SUCCESS
            } else {
                INVALID_PARAMETER
            }
        }
        336 => {
            let Some(mut state) = Sha1::load(memory, args[0]) else {
                counters.stale = counters.stale.wrapping_add(1);
                return INVALID_PARAMETER;
            };
            let Some(data) = fetch(memory, args[1], args[2]) else {
                return INVALID_PARAMETER;
            };
            state.update(&data);
            counters.absorbed = counters.absorbed.wrapping_add(data.len() as u64);
            if state.store(memory, args[0]) {
                counters.sha_update = counters.sha_update.wrapping_add(1);
                SUCCESS
            } else {
                INVALID_PARAMETER
            }
        }
        337 => {
            let Some(state) = Sha1::load(memory, args[0]) else {
                counters.stale = counters.stale.wrapping_add(1);
                return INVALID_PARAMETER;
            };
            if !memory.readable(args[1], 20)
                || !memory.write(args[1], &state.finish())
                || !memory.put_word(args[0], 0)
            {
                INVALID_PARAMETER
            } else {
                counters.sha_final = counters.sha_final.wrapping_add(1);
                SUCCESS
            }
        }
        338 => rc4_key(memory, args[0], args[1], args[2]),
        339 => rc4_crypt(memory, args[0], args[1], args[2]),
        340 => {
            let Some(key) = fetch(memory, args[0], args[1]) else {
                return INVALID_PARAMETER;
            };
            let Some(first) = fetch(memory, args[2], args[3]) else {
                return INVALID_PARAMETER;
            };
            let Some(second) = fetch(memory, args[4], args[5]) else {
                return INVALID_PARAMETER;
            };
            if !memory.readable(args[6], 20) {
                return INVALID_PARAMETER;
            }
            if key.len() > 64 {
                counters.oversize = counters.oversize.wrapping_add(1);
            }
            if !memory.write(args[6], &hmac(&key, &first, &second, false)) {
                INVALID_PARAMETER
            } else {
                counters.hmac = counters.hmac.wrapping_add(1);
                SUCCESS
            }
        }
        346 => {
            // Zero length returns success without translating the pointer.
            if args[1] == 0 {
                counters.des = counters.des.wrapping_add(1);
                return SUCCESS;
            }
            if !memory.readable(args[0], args[1] as usize) {
                return INVALID_PARAMETER;
            }
            for i in 0..args[1] {
                let Some(byte) = memory.byte(args[0] + i) else {
                    return INVALID_PARAMETER;
                };
                if !memory.write(args[0] + i, &[parity(byte)]) {
                    return INVALID_PARAMETER;
                }
            }
            counters.des = counters.des.wrapping_add(1);
            SUCCESS
        }
        _ => INVALID_PARAMETER,
    }
}
