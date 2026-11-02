// SPDX-License-Identifier: GPL-3.0-or-later
use crate::wire::{self, Record};
use std::io::{self, Read};

#[derive(Clone, Copy, Debug)]
pub struct Limits {
    /// Includes the length prefix. Checked before reserving the record buffer.
    pub record_bytes: usize,
    /// Includes all records through the present. No frame buffering is needed.
    pub frame_bytes: u64,
    pub records_per_frame: u64,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            record_bytes: 64 * 1024 * 1024,
            frame_bytes: 512 * 1024 * 1024,
            records_per_frame: 4_000_000,
        }
    }
}

/// Pinned by the capture receipt, never inferred from the observed EOF.
#[derive(Clone, Copy, Debug)]
pub struct Window {
    /// Zero means unnamed in the original format and cannot prove continuity.
    pub first_present: u64,
    pub frame_count: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error {
    pub offset: u64,
    pub completed_frames: u64,
    pub reason: String,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "byte {}, after {} frames: {}",
            self.offset, self.completed_frames, self.reason
        )
    }
}
impl std::error::Error for Error {}

#[derive(Debug)]
pub struct LocatedRecord<'a> {
    pub offset: u64,
    /// Encoded byte count, including the length prefix.
    pub bytes: usize,
    /// Zero-based frame index within the pinned window.
    pub frame_index: u64,
    pub record: Record<'a>,
}

/// A failed reader stays failed, including after I/O errors. Consumers must
/// still visit every returned state/call and resolve every referenced resource.
/// This reader never silently skips an unknown record or omitted-draw marker.
pub struct Reader<R> {
    input: R,
    window: Window,
    limits: Limits,
    buffer: Vec<u8>,
    offset: u64,
    completed: u64,
    frame_bytes: u64,
    frame_records: u64,
    failed: bool,
    finished: bool,
}
impl<R: Read> Reader<R> {
    pub fn new(input: R, window: Window, limits: Limits) -> Result<Self, Error> {
        if window.first_present == 0
            || window.frame_count == 0
            || window
                .first_present
                .checked_add(window.frame_count - 1)
                .is_none()
            || limits.record_bytes < 5
            || limits.frame_bytes < 5
            || limits.records_per_frame == 0
        {
            return Err(Error {
                offset: 0,
                completed_frames: 0,
                reason: "invalid window or limits".into(),
            });
        }
        Ok(Self {
            input,
            window,
            limits,
            buffer: Vec::new(),
            offset: 0,
            completed: 0,
            frame_bytes: 0,
            frame_records: 0,
            failed: false,
            finished: false,
        })
    }

    /// Tighten caps before a consumer with a smaller budget starts a read.
    pub(crate) fn tighten_limits(&mut self, limits: Limits) {
        self.limits.record_bytes = self.limits.record_bytes.min(limits.record_bytes);
        self.limits.frame_bytes = self.limits.frame_bytes.min(limits.frame_bytes);
        self.limits.records_per_frame = self.limits.records_per_frame.min(limits.records_per_frame);
    }

    fn error(&self, reason: impl Into<String>) -> Error {
        Error {
            offset: self.offset,
            completed_frames: self.completed,
            reason: reason.into(),
        }
    }

    // Load one envelope only; return false only on clean record-boundary EOF.
    fn load(&mut self) -> Result<bool, Error> {
        let mut header = [0u8; 4];
        loop {
            match self.input.read(&mut header[..1]) {
                Ok(0) => return Ok(false),
                Ok(_) => break,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(self.error(format!("read header: {e}"))),
            }
        }
        if self.completed == self.window.frame_count {
            return Err(self.error("records after declared final present"));
        }
        self.input
            .read_exact(&mut header[1..])
            .map_err(|e| self.error(format!("truncated header: {e}")))?;
        let body = u32::from_le_bytes(header) as u64;
        let total = body + 4;
        if body == 0 || total > self.limits.record_bytes as u64 {
            return Err(self.error("record byte limit or zero length"));
        }
        let frame_bytes = self
            .frame_bytes
            .checked_add(total)
            .ok_or_else(|| self.error("frame byte overflow"))?;
        let frame_records = self
            .frame_records
            .checked_add(1)
            .ok_or_else(|| self.error("frame record overflow"))?;
        if frame_bytes > self.limits.frame_bytes || frame_records > self.limits.records_per_frame {
            return Err(self.error("frame budget exceeded"));
        }
        let total = usize::try_from(total).map_err(|_| self.error("record size overflow"))?;
        self.buffer.clear();
        self.buffer
            .try_reserve_exact(total)
            .map_err(|_| self.error("record allocation failed"))?;
        self.buffer.extend_from_slice(&header);
        self.buffer.resize(total, 0);
        self.input
            .read_exact(&mut self.buffer[4..])
            .map_err(|e| self.error(format!("truncated payload: {e}")))?;
        self.frame_bytes = frame_bytes;
        self.frame_records = frame_records;
        Ok(true)
    }

    pub fn next_record(&mut self) -> Result<Option<LocatedRecord<'_>>, Error> {
        if self.failed {
            return Err(self.error("reader poisoned by previous failure"));
        }
        if self.finished {
            return Ok(None);
        }
        self.failed = true;
        if !self.load()? {
            if self.completed != self.window.frame_count || self.frame_records != 0 {
                return Err(self.error("EOF before declared final present"));
            }
            self.finished = true;
            self.failed = false;
            return Ok(None);
        }
        let record = wire::Reader::new(&self.buffer)
            .next()
            .and_then(Result::ok)
            .ok_or_else(|| self.error("malformed record"))?;
        strict_shape(&record, self.buffer.len() - 4).map_err(|why| self.error(why))?;
        let offset = self.offset;
        let frame_index = self.completed;
        if let Record::Present { number } = record {
            let expected = self.window.first_present + self.completed;
            if number != expected {
                return Err(self.error(format!("present {number}, expected {expected}")));
            }
            self.completed += 1;
            self.frame_bytes = 0;
            self.frame_records = 0;
        }
        self.offset = self
            .offset
            .checked_add(self.buffer.len() as u64)
            .ok_or_else(|| self.error("stream offset overflow"))?;
        self.failed = false;
        Ok(Some(LocatedRecord {
            offset,
            bytes: self.buffer.len(),
            frame_index,
            record,
        }))
    }
}

/// The shared replay parser is extension-tolerant. Evidence admission is strict:
/// every byte must be understood. An exhaustive match makes new tags explicit.
fn strict_shape(record: &Record<'_>, body_bytes: usize) -> Result<(), &'static str> {
    let expected = match record {
        Record::Call { name, words } => {
            if name.is_empty() {
                return Err("empty call name");
            }
            1 + 1 + name.len() as u64 + 4 + words.len() as u64 * 4
        }
        Record::Scissor { count, rects, .. } => {
            if u64::from(*count) * 4 != rects.len() as u64 {
                return Err("scissor count differs from payload");
            }
            1 + 12 + rects.len() as u64 * 4
        }
        Record::Host { words, .. } => 1 + 8 + words.len() as u64 * 4,
        Record::Memory { address, bytes } => {
            if u64::from(*address) + bytes.len() as u64 > 1u64 << 32 {
                return Err("memory address range wraps");
            }
            1 + 8 + bytes.len() as u64
        }
        Record::Surface { .. } => 1 + 32,
        Record::Lib { .. } => 1 + 8,
        Record::Present { .. } => 1 + 8,
        Record::Texture {
            width,
            height,
            rgba,
            ..
        } => {
            let pixels = u64::from(*width) * u64::from(*height);
            if *width == 0 || *height == 0 || pixels.checked_mul(4) != Some(rgba.len() as u64) {
                return Err("texture extent differs from payload");
            }
            1 + 16 + rgba.len() as u64
        }
        Record::Frame { flags } => {
            if flags & wire::FRAME_DRAWS_OMITTED != 0 {
                return Err("draws omitted by producer");
            }
            if *flags != 0 {
                return Err("unknown frame flags");
            }
            1 + 4
        }
        Record::Unknown { .. } => return Err("unknown record tag"),
    };
    if expected != body_bytes as u64 {
        return Err("unconsumed record payload");
    }
    Ok(())
}
