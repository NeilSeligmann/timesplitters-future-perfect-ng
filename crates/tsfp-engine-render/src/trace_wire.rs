// SPDX-License-Identifier: GPL-3.0-or-later
//! T2136d: binary form of the render trace stream (the text grammar of `crates/tsfp-wgpu/src/stream.rs` stays the default for
//! dumps and the offline replay tools). Shared source: `crates/tsfp-ports` includes it with `#[path]` to ENCODE on the guest thread,
//! `crates/tsfp-wgpu` decodes it on the render thread. Pure `core` and `alloc`, no I/O. Format: docs/t-rust-hosted-dispatcher.md.
//!
//! A buffer is a sequence of records, all integers little endian:
//!
//! ```text
//! u32 length   bytes that follow this field (tag included)
//! u8  tag
//! payload
//! ```
//!
//! A reader that meets an unknown tag skips the record by its length, so tags can be added without breaking old readers.
use alloc::vec::Vec;

pub const TAG_CALL: u8 = 1;
pub const TAG_SCISSOR: u8 = 2;
pub const TAG_HOST: u8 = 3;
pub const TAG_MEMORY: u8 = 4;
pub const TAG_SURFACE: u8 = 5;
pub const TAG_LIB: u8 = 6;
pub const TAG_PRESENT: u8 = 7;
pub const TAG_TEXTURE: u8 = 8;
/// `u32 flags`, bit 0: the guest omits the draws of the frames that follow (it is behind), until the next `Frame` record.
pub const TAG_FRAME: u8 = 9;
/// [`TAG_FRAME`] flag: draw calls and their vertex data are left out of the following frames.
pub const FRAME_DRAWS_OMITTED: u32 = 1;

/// Start a record: returns the offset of its length field, to be passed to [`finish`].
#[inline]
fn begin(out: &mut Vec<u8>, tag: u8) -> usize {
    let at = out.len();
    out.extend_from_slice(&[0, 0, 0, 0, tag]);
    at
}

/// Patch the length field of the record started at `at`.
#[inline]
fn finish(out: &mut Vec<u8>, at: usize) {
    let length = (out.len() - at - 4) as u32;
    out[at..at + 4].copy_from_slice(&length.to_le_bytes());
}

#[inline]
fn put32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

/// Stage words on the stack and append their wire payload in one slice copy.
/// Shared no_std source keeps the host hook allocation-free after buffer warmup.
#[inline]
fn put_words(out: &mut Vec<u8>, words: &[u32]) {
    let mut bytes = [0u8; 256];
    for chunk in words.chunks(64) {
        for (word, dest) in chunk.iter().zip(bytes.chunks_exact_mut(4)) {
            dest.copy_from_slice(&word.to_le_bytes());
        }
        out.extend_from_slice(&bytes[..chunk.len() * 4]);
    }
}

/// `call <name> <hex words>`: `u8 name length, name, u32 word count, words`.
pub fn call(out: &mut Vec<u8>, name: &str, words: &[u32]) {
    let at = begin(out, TAG_CALL);
    out.push(name.len() as u8);
    out.extend_from_slice(name.as_bytes());
    put32(out, words.len() as u32);
    put_words(out, words);
    finish(out, at);
}

/// A [`call`] whose first word is `first` and the rest `words` (the constant uploads: register index, then the data), without
/// building the joined list.
pub fn call_with_first(out: &mut Vec<u8>, name: &str, first: u32, words: &[u32]) {
    let at = begin(out, TAG_CALL);
    out.push(name.len() as u8);
    out.extend_from_slice(name.as_bytes());
    put32(out, words.len() as u32 + 1);
    put32(out, first);
    put_words(out, words);
    finish(out, at);
}

/// `call SetScissor`: `u32 count, u32 flags, u32 rectangle count, 4 i32 per rectangle`.
pub fn scissor(out: &mut Vec<u8>, count: u32, flags: u32, rects: &[[i32; 4]]) {
    let at = begin(out, TAG_SCISSOR);
    put32(out, count);
    put32(out, flags);
    put32(out, rects.len() as u32);
    for rect in rects {
        for value in rect {
            put32(out, *value as u32);
        }
    }
    finish(out, at);
}

/// `host <function> <hex words>`: `u32 function, u32 word count, words`.
pub fn host(out: &mut Vec<u8>, function: u32, words: &[u32]) {
    let at = begin(out, TAG_HOST);
    put32(out, function);
    put32(out, words.len() as u32);
    put_words(out, words);
    finish(out, at);
}

/// Header of a `mem` record whose `length` bytes the caller appends next with [`memory_bytes`]; returns the [`finish`] cookie.
pub fn memory_begin(out: &mut Vec<u8>, address: u32, length: u32) -> usize {
    let at = begin(out, TAG_MEMORY);
    put32(out, address);
    put32(out, length);
    at
}

/// Append payload bytes of the open `mem` record.
#[inline]
pub fn memory_bytes(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend_from_slice(bytes);
}

/// Close a record opened by [`memory_begin`].
pub fn memory_end(out: &mut Vec<u8>, cookie: usize) {
    finish(out, cookie);
}

/// `mem <address> <bytes>` in one go.
pub fn memory(out: &mut Vec<u8>, address: u32, bytes: &[u8]) {
    let at = memory_begin(out, address, bytes.len() as u32);
    memory_bytes(out, bytes);
    memory_end(out, at);
}

/// `call SetSurface`: eight words (target, depth, data, format, size, pitch, depth data, flags).
pub fn surface(out: &mut Vec<u8>, words: &[u32; 8]) {
    let at = begin(out, TAG_SURFACE);
    for word in words {
        put32(out, *word);
    }
    finish(out, at);
}

/// `lib <method> <value>`.
pub fn lib(out: &mut Vec<u8>, method: u32, value: u32) {
    let at = begin(out, TAG_LIB);
    put32(out, method);
    put32(out, value);
    finish(out, at);
}

/// `# present <number>` plus `present`: `u64 number`.
pub fn present(out: &mut Vec<u8>, number: u64) {
    let at = begin(out, TAG_PRESENT);
    out.extend_from_slice(&number.to_le_bytes());
    finish(out, at);
}

/// `texture <id> <width> <height> <rgba>`: `u32 id, u32 width, u32 height, u32 length, bytes`.
pub fn texture(out: &mut Vec<u8>, id: u32, width: u32, height: u32, rgba: &[u8]) {
    let at = begin(out, TAG_TEXTURE);
    put32(out, id);
    put32(out, width);
    put32(out, height);
    put32(out, rgba.len() as u32);
    out.extend_from_slice(rgba);
    finish(out, at);
}

/// `Frame` flags for the frames that follow.
pub fn frame(out: &mut Vec<u8>, flags: u32) {
    let at = begin(out, TAG_FRAME);
    put32(out, flags);
    finish(out, at);
}

/// One decoded record, borrowing the buffer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Record<'a> {
    Call {
        name: &'a str,
        words: Words<'a>,
    },
    Scissor {
        count: u32,
        flags: u32,
        rects: Words<'a>,
    },
    Host {
        function: u32,
        words: Words<'a>,
    },
    Memory {
        address: u32,
        bytes: &'a [u8],
    },
    Surface {
        words: Words<'a>,
    },
    Lib {
        method: u32,
        value: u32,
    },
    Present {
        number: u64,
    },
    Texture {
        id: u32,
        width: u32,
        height: u32,
        rgba: &'a [u8],
    },
    Frame {
        flags: u32,
    },
    /// A tag this reader does not know (skipped by length).
    Unknown {
        tag: u8,
    },
}

/// A run of little endian u32 in a buffer (the buffer is not aligned, so words are read by copy).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Words<'a>(&'a [u8]);

impl Words<'_> {
    pub fn len(&self) -> usize {
        self.0.len() / 4
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    pub fn get(&self, index: usize) -> u32 {
        let at = index * 4;
        u32::from_le_bytes([self.0[at], self.0[at + 1], self.0[at + 2], self.0[at + 3]])
    }
    pub fn to_vec(&self) -> Vec<u32> {
        (0..self.len()).map(|index| self.get(index)).collect()
    }
}

fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

fn words_at(bytes: &[u8], at: usize, count: usize) -> Option<Words<'_>> {
    Some(Words(
        bytes.get(at..at.checked_add(count.checked_mul(4)?)?)?,
    ))
}

/// Decode the record whose payload (tag byte first) is `body`.
fn record(body: &[u8]) -> Option<Record<'_>> {
    let tag = *body.first()?;
    let p = &body[1..];
    Some(match tag {
        TAG_CALL => {
            let name_length = *p.first()? as usize;
            let name = core::str::from_utf8(p.get(1..1 + name_length)?).ok()?;
            let count = u32_at(p, 1 + name_length)? as usize;
            Record::Call {
                name,
                words: words_at(p, 5 + name_length, count)?,
            }
        }
        TAG_SCISSOR => {
            let count = u32_at(p, 0)?;
            let flags = u32_at(p, 4)?;
            let rects = u32_at(p, 8)? as usize;
            Record::Scissor {
                count,
                flags,
                rects: words_at(p, 12, rects.checked_mul(4)?)?,
            }
        }
        TAG_HOST => {
            let function = u32_at(p, 0)?;
            let count = u32_at(p, 4)? as usize;
            Record::Host {
                function,
                words: words_at(p, 8, count)?,
            }
        }
        TAG_MEMORY => {
            let address = u32_at(p, 0)?;
            let length = u32_at(p, 4)? as usize;
            Record::Memory {
                address,
                bytes: p.get(8..8usize.checked_add(length)?)?,
            }
        }
        TAG_SURFACE => Record::Surface {
            words: words_at(p, 0, 8)?,
        },
        TAG_LIB => Record::Lib {
            method: u32_at(p, 0)?,
            value: u32_at(p, 4)?,
        },
        TAG_PRESENT => Record::Present {
            number: u64::from_le_bytes(p.get(0..8)?.try_into().ok()?),
        },
        TAG_TEXTURE => {
            let length = u32_at(p, 12)? as usize;
            Record::Texture {
                id: u32_at(p, 0)?,
                width: u32_at(p, 4)?,
                height: u32_at(p, 8)?,
                rgba: p.get(16..16usize.checked_add(length)?)?,
            }
        }
        TAG_FRAME => Record::Frame {
            flags: u32_at(p, 0)?,
        },
        other => Record::Unknown { tag: other },
    })
}

/// Iterator over the records of a buffer. A truncated or malformed record yields `Err(offset)` once and ends the iteration.
pub struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, at: 0 }
    }
}

impl<'a> Iterator for Reader<'a> {
    type Item = Result<Record<'a>, usize>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.at >= self.bytes.len() {
            return None;
        }
        let start = self.at;
        let decoded = u32_at(self.bytes, start).and_then(|length| {
            let end = (start + 4).checked_add(length as usize)?;
            let body = self.bytes.get(start + 4..end)?;
            self.at = end;
            record(body)
        });
        if decoded.is_none() {
            self.at = self.bytes.len();
        }
        Some(decoded.ok_or(start))
    }
}
