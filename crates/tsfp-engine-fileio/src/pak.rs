//! Streaming PAK (P5CK) access over any [`Vfs`] handle: reads only the 12-byte header and the
//! TOC, then ranged payload reads, so a 1 GB archive is never loaded. TOC rules are identical to
//! `tsfp_format_pak::Pak::parse` (checked by the unit tests); XBR/XBT payloads feed the format
//! crates. Compressed payloads are decoded with a strict output bound; `stored` still returns
//! the original bitstream.
use std::io::Read;
use flate2::read::GzDecoder;
use crate::vfs::{Handle, NtStatus, Vfs, status};
use tsfp_format_core::{range, word, words};
pub use tsfp_format_pak::Entry;

#[derive(Debug)]
pub struct PakIndex { pub entries: Vec<Entry>, pub file_size: u64, handle: Handle }

fn bad<T>(_: &'static str) -> Result<T, NtStatus> { Err(status::UNRECOGNIZED_VOLUME) }

impl PakIndex {
    /// `handle` must be an open PAK file (disc or host). The index owns the handle id only.
    pub fn open(vfs: &mut Vfs, handle: Handle) -> Result<Self, NtStatus> {
        let file_size = vfs.size(handle)?;
        let mut head = [0u8; 12];
        if vfs.read_at(handle, 0, &mut head)? != 12 { return bad("short header"); }
        if range(&head, 0, 4).map_err(|_| status::UNRECOGNIZED_VOLUME)? != b"P5CK" { return bad("magic"); }
        let toc = word(&head, 4).map_err(|_| status::UNRECOGNIZED_VOLUME)? as u64;
        let size = word(&head, 8).map_err(|_| status::UNRECOGNIZED_VOLUME)? as u64;
        if size % 16 != 0 { return bad("partial TOC"); }
        // Validate the claimed TOC against the file before allocating it. A damaged header
        // can otherwise request nearly 4 GiB from a tiny archive.
        if size != 0 && (toc < 12 || toc.checked_add(size) != Some(file_size)) { return bad("TOC not at EOF"); }
        let mut raw = vec![0u8; size as usize];
        if size != 0 && vfs.read_at(handle, toc, &mut raw)? != raw.len() { return bad("short TOC"); }
        let mut entries = Vec::new();
        for i in 0..(size / 16) as usize {
            let w = words(&raw, i * 16, 4).map_err(|_| status::UNRECOGNIZED_VOLUME)?;
            let n = u64::from(if w[3] == 0 { w[2] } else { w[3] });
            if n != 0 && (u64::from(w[1]) < 12 || u64::from(w[1]) + n > toc) { return bad("payload overlaps"); }
            entries.push(Entry { key: w[0], offset: w[1], decoded_size: w[2], stored_size: w[3] });
        }
        Ok(Self { entries, file_size, handle })
    }
    pub fn stored_len(&self, index: usize) -> Option<u64> {
        self.entries.get(index).map(|e| u64::from(if e.stored_size == 0 { e.decoded_size } else { e.stored_size }))
    }
    pub fn find(&self, key: u32) -> impl Iterator<Item = usize> + '_ {
        self.entries.iter().enumerate().filter(move |(_, e)| e.key == key).map(|(i, _)| i)
    }
    /// Ranged read of the stored bytes of entry `index`.
    pub fn read_stored(&self, vfs: &mut Vfs, index: usize, offset: u64, buf: &mut [u8]) -> Result<usize, NtStatus> {
        let (e, len) = (self.entries.get(index).ok_or(status::INVALID_PARAMETER)?, self.stored_len(index).unwrap());
        if offset >= len { return Ok(0); }
        let n = buf.len().min((len - offset) as usize);
        vfs.read_at(self.handle, u64::from(e.offset) + offset, &mut buf[..n])
    }
    pub fn stored(&self, vfs: &mut Vfs, index: usize) -> Result<Vec<u8>, NtStatus> {
        let mut v = vec![0u8; self.stored_len(index).ok_or(status::INVALID_PARAMETER)? as usize];
        let mut n = 0;
        while n < v.len() {
            let got = self.read_stored(vfs, index, n as u64, &mut v[n..])?;
            if got == 0 { return bad("short payload"); }
            n += got;
        }
        Ok(v)
    }
    /// Return decoded bytes, requiring the advertised size and a complete gzip stream.
    pub fn payload(&self, vfs: &mut Vfs, index: usize) -> Result<Vec<u8>, NtStatus> {
        let entry = self.entries.get(index).ok_or(status::INVALID_PARAMETER)?;
        let stored = self.stored(vfs, index)?;
        if entry.stored_size == 0 { return Ok(stored); }
        let expected = entry.decoded_size as usize;
        let decoder = GzDecoder::new(stored.as_slice());
        let mut output = Vec::new();
        decoder.take(expected as u64 + 1).read_to_end(&mut output)
            .map_err(|_| status::UNRECOGNIZED_VOLUME)?;
        if output.len() != expected { return bad("decoded size mismatch"); }
        Ok(output)
    }
    /// Parse an entry as an XBT texture envelope.
    pub fn texture(&self, vfs: &mut Vfs, index: usize) -> Result<tsfp_format_xbt::Texture, NtStatus> {
        tsfp_format_xbt::Texture::parse(&self.payload(vfs, index)?).map_err(|_| status::UNRECOGNIZED_VOLUME)
    }
    /// Parse an entry as an XBR level envelope.
    pub fn level(&self, vfs: &mut Vfs, index: usize) -> Result<tsfp_format_xbr::Level, NtStatus> {
        tsfp_format_xbr::Level::parse(&self.payload(vfs, index)?).map_err(|_| status::UNRECOGNIZED_VOLUME)
    }
}
