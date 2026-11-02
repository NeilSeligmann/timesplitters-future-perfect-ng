// SPDX-License-Identifier: GPL-3.0-or-later
//! Bounded, sealed host-boundary event stream. A stream is evidence only after
//! its producer has established complete instrumentation and supplied all events.

pub mod events;
pub mod ffi;
pub mod file;
pub mod host_write;
pub mod producer;
pub mod recording;
pub mod writer_coverage;

use std::io::{self, Read, Write};

const MAGIC: &[u8; 8] = b"TSFPHC1\n";
const FOOTER: u32 = u32::MAX;
const MAX_RECORD: usize = 1 << 20;
const MAX_STREAM: u64 = 4 << 30;
const MAX_RECORDS: u64 = 50_000_000;
const HEADER_SIZE: u64 = 8;
const FRAME_SIZE: u64 = 16;
const FOOTER_SIZE: u64 = 24;
const FNV_OFFSET: u64 = 0xcbf29ce484222325;
const FNV_PRIME: u64 = 0x100000001b3;

fn hash(mut state: u64, bytes: &[u8]) -> u64 {
    for byte in bytes {
        state = (state ^ u64::from(*byte)).wrapping_mul(FNV_PRIME);
    }
    state
}

/// Writes ordered events to an already-created temporary stream. A poisoned or
/// over-budget stream cannot be sealed; callers must not publish its temporary file.
pub struct Writer<W> {
    output: W,
    limit: u64,
    size: u64,
    count: u64,
    checksum: u64,
    complete: bool,
    stopped: bool,
}

impl<W: Write> Writer<W> {
    pub fn new(mut output: W, limit: u64) -> io::Result<Self> {
        if !(HEADER_SIZE + FOOTER_SIZE..=MAX_STREAM).contains(&limit) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "capture limit too small",
            ));
        }
        output.write_all(MAGIC)?;
        Ok(Self {
            output,
            limit,
            size: HEADER_SIZE,
            count: 0,
            checksum: hash(FNV_OFFSET, MAGIC),
            complete: true,
            stopped: false,
        })
    }

    /// Records accepted so far; the stream position a host-write fence is checked against.
    pub fn count(&self) -> u64 {
        self.count
    }

    pub fn record(&mut self, kind: u32, data: &[u8]) -> io::Result<()> {
        let required = FRAME_SIZE
            .checked_add(data.len() as u64)
            .and_then(|n| n.checked_add(FOOTER_SIZE));
        if self.stopped
            || self.count >= MAX_RECORDS
            || kind == FOOTER
            || data.len() > MAX_RECORD
            || required.is_none_or(|n| self.size.checked_add(n).is_none_or(|v| v > self.limit))
        {
            self.complete = false;
            self.stopped = true;
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "incomplete or over-budget capture",
            ));
        }
        let mut frame = [0u8; 16];
        frame[..8].copy_from_slice(&self.count.to_le_bytes());
        frame[8..12].copy_from_slice(&kind.to_le_bytes());
        frame[12..].copy_from_slice(&(data.len() as u32).to_le_bytes());
        if let Err(error) = self
            .output
            .write_all(&frame)
            .and_then(|_| self.output.write_all(data))
        {
            self.complete = false;
            self.stopped = true;
            return Err(error);
        }
        self.checksum = hash(hash(self.checksum, &frame), data);
        self.size += FRAME_SIZE + data.len() as u64;
        self.count += 1;
        Ok(())
    }

    /// Mark unsupported, omitted or failed capture work; sealing then fails.
    pub fn incomplete(&mut self) {
        self.complete = false;
    }

    /// A structural error stops further writes as well as preventing a seal.
    pub fn stop(&mut self) {
        self.complete = false;
        self.stopped = true;
    }

    pub fn seal(mut self) -> io::Result<W> {
        if !self.complete {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "capture incomplete",
            ));
        }
        let mut footer = [0u8; 24];
        footer[..8].copy_from_slice(&self.count.to_le_bytes());
        footer[8..12].copy_from_slice(&FOOTER.to_le_bytes());
        footer[16..].copy_from_slice(&self.checksum.to_le_bytes());
        self.output.write_all(&footer)?;
        self.output.flush()?;
        Ok(self.output)
    }
}

/// Validate the complete stream before accepting any events as evidence.
/// The rolling checksum detects accidental corruption; retainers must additionally
/// use a cryptographic content digest for evidence identity.
pub fn read<R: Read>(mut input: R, limit: u64) -> io::Result<Vec<(u32, Vec<u8>)>> {
    let bad = || io::Error::new(io::ErrorKind::InvalidData, "invalid or incomplete capture");
    let mut magic = [0u8; 8];
    input.read_exact(&mut magic)?;
    if &magic != MAGIC || !(HEADER_SIZE + FOOTER_SIZE..=MAX_STREAM).contains(&limit) {
        return Err(bad());
    }
    let mut size = HEADER_SIZE;
    let mut checksum = hash(FNV_OFFSET, &magic);
    let mut result = Vec::new();
    loop {
        let mut frame = [0u8; 16];
        input.read_exact(&mut frame)?;
        size = size.checked_add(FRAME_SIZE).ok_or_else(bad)?;
        let sequence = u64::from_le_bytes(frame[..8].try_into().unwrap());
        let kind = u32::from_le_bytes(frame[8..12].try_into().unwrap());
        let len = u32::from_le_bytes(frame[12..].try_into().unwrap()) as usize;
        if sequence != result.len() as u64 || sequence > MAX_RECORDS || size > limit {
            return Err(bad());
        }
        if kind == FOOTER {
            if len != 0 || size.checked_add(8).is_none_or(|v| v > limit) {
                return Err(bad());
            }
            let mut digest = [0u8; 8];
            input.read_exact(&mut digest)?;
            if u64::from_le_bytes(digest) != checksum {
                return Err(bad());
            }
            let mut extra = [0u8; 1];
            if input.read(&mut extra)? != 0 {
                return Err(bad());
            }
            return Ok(result);
        }
        if sequence >= MAX_RECORDS
            || len > MAX_RECORD
            || size.checked_add(len as u64).is_none_or(|v| v > limit)
        {
            return Err(bad());
        }
        let mut data = vec![0; len];
        input.read_exact(&mut data)?;
        size += len as u64;
        checksum = hash(hash(checksum, &frame), &data);
        result.push((kind, data));
    }
}
