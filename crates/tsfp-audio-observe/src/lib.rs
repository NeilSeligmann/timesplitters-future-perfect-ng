//! Passive command capture. The caller holds its mixer lock across operation and
//! observation. Errors invalidate the capture, never alter the mixer verdict.
#![forbid(unsafe_code)]

use std::io::{self, Write};
use tsfp_audio::{contract::Command, replay::fnv1a};

mod encode;

pub const MAX_LINE_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_RENDER_FRAMES: usize = 1_048_576;
const FNV_OFFSET: u64 = 1469598103934665603;

/// A bounded, streaming recorder. No END is emitted on Drop or after a record error;
/// even truncation exactly at a record boundary must fail offline verification.
pub struct Recorder<W> {
    writer: W,
    hash: u64,
    records: u64,
    failed: bool,
}

impl<W: Write> Recorder<W> {
    pub fn new(writer: W, rate: u32, ticks_per_second: u64) -> io::Result<Self> {
        if rate == 0 || rate > 384_000 || ticks_per_second == 0 {
            return Err(invalid("invalid clock or output rate"));
        }
        let mut recorder = Self {
            writer,
            hash: FNV_OFFSET,
            records: 0,
            failed: false,
        };
        recorder.record(|w| writeln!(w, "CAPTURE 1"))?;
        recorder.record(|w| writeln!(w, "START {rate} {ticks_per_second}"))?;
        Ok(recorder)
    }

    /// PB must retain the original ADPCM bytes, independently decoded by the
    /// offline C and Rust hosts. Other commands must not carry a PB payload.
    pub fn command(
        &mut self,
        command: &Command,
        accepted: bool,
        encoded_buffer: Option<&[u8]>,
    ) -> io::Result<()> {
        self.record(|w| encode::command(w, command, accepted, encoded_buffer))
    }

    /// Record empty renders too: the timeline operation itself is observable.
    pub fn render(&mut self, ticks: u64, capacity: usize, pcm: &[i16]) -> io::Result<()> {
        self.record(|w| {
            if capacity > MAX_RENDER_FRAMES || pcm.len() % 2 != 0 || pcm.len() / 2 > capacity {
                return Err(invalid("invalid render shape"));
            }
            writeln!(
                w,
                "R {ticks} {capacity} {} {:016x}",
                pcm.len() / 2,
                fnv1a(pcm)
            )
        })
    }

    fn record(&mut self, emit: impl FnOnce(&mut dyn Write) -> io::Result<()>) -> io::Result<()> {
        if self.failed {
            return Err(invalid("capture already failed"));
        }
        let mut bounded = Bounded {
            writer: &mut self.writer,
            hash: &mut self.hash,
            remaining: MAX_LINE_BYTES,
        };
        let result = emit(&mut bounded);
        if result.is_err() {
            self.failed = true;
        } else {
            self.records += 1;
        }
        result
    }

    /// Only an explicitly completed, flushed recording has a valid END seal.
    /// File owners must additionally sync the returned file before publication.
    pub fn finish(mut self) -> io::Result<W> {
        if self.failed {
            return Err(invalid("capture already failed"));
        }
        writeln!(self.writer, "END {} {:016x}", self.records, self.hash)?;
        self.writer.flush()?;
        Ok(self.writer)
    }
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

struct Bounded<'a, W> {
    writer: &'a mut W,
    hash: &'a mut u64,
    remaining: usize,
}

impl<W: Write> Write for Bounded<'_, W> {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        if data.len() > self.remaining {
            return Err(invalid("record exceeds bound"));
        }
        let count = self.writer.write(data)?;
        for &byte in &data[..count] {
            *self.hash = (*self.hash ^ u64::from(byte)).wrapping_mul(1099511628211);
        }
        self.remaining -= count;
        Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }
}
