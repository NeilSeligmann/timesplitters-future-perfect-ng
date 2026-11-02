// SPDX-License-Identifier: GPL-3.0-or-later
//! Canonical DrawList file v1: fixed header then u64-length-prefixed frames.
//! A semantic adapter must resolve the original device stream before writing.
//! This format is not the NV2A stream and never silently treats it as one.
use crate::codec::{Codec, Decoder, invalid};
use crate::{Capture, Frame, FrameSource, Side};
use std::io::{self, Read, Write};

pub const MAGIC: [u8; 8] = *b"TSDRAW01";
/// Capture (204 bytes), side (1), producer revision (20). All integers LE.
pub const HEADER_BYTES: usize = 225;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Header {
    pub capture: Capture,
    pub side: Side,
    pub producer_revision: [u8; 20],
}
impl Header {
    fn validate(&self) -> io::Result<()> {
        if !self.capture.valid()
            || self.producer_revision == [0; 20]
            || (self.side == Side::Oracle
                && self.producer_revision != self.capture.original_revision)
        {
            return Err(invalid("invalid canonical capture header"));
        }
        Ok(())
    }
}

pub struct Reader<R> {
    input: R,
    header: Header,
    failed: bool,
}
impl<R: Read> Reader<R> {
    pub fn new(mut input: R) -> io::Result<Self> {
        let mut magic = [0; 8];
        input.read_exact(&mut magic)?;
        if magic != MAGIC {
            return Err(invalid("not canonical DrawList v1"));
        }
        let mut decoder = Decoder {
            input: &mut input,
            remaining: HEADER_BYTES as u64,
            allocations: HEADER_BYTES,
        };
        let capture = Capture::decode(&mut decoder)?;
        let side = match u8::decode(&mut decoder)? {
            0 => Side::Oracle,
            1 => Side::Native,
            _ => return Err(invalid("unknown capture side")),
        };
        let producer_revision = Codec::decode(&mut decoder)?;
        let header = Header {
            capture,
            side,
            producer_revision,
        };
        header.validate()?;
        Ok(Self {
            input,
            header,
            failed: false,
        })
    }
    pub fn header(&self) -> &Header {
        &self.header
    }

    fn read_frame(&mut self, limit: usize) -> io::Result<Option<Frame>> {
        // Only EOF before the first length byte is an end of stream.
        let mut length = [0u8; 8];
        loop {
            match self.input.read(&mut length[..1]) {
                Ok(0) => return Ok(None),
                Ok(_) => break,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(e),
            }
        }
        self.input.read_exact(&mut length[1..])?;
        let length = u64::from_le_bytes(length);
        if length == 0 || length > limit as u64 {
            return Err(invalid("canonical frame byte limit"));
        }
        // Decode directly from the reader. No frame-sized input buffer is created.
        let mut decoder = Decoder {
            input: &mut self.input,
            remaining: length,
            allocations: limit,
        };
        let frame = Frame::decode(&mut decoder)?;
        if decoder.remaining != 0 {
            return Err(invalid("trailing bytes in canonical frame"));
        }
        Ok(Some(frame))
    }
}
impl<R: Read> FrameSource for Reader<R> {
    fn next_frame(&mut self, max_frame_bytes: usize) -> Result<Option<Frame>, String> {
        if self.failed {
            return Err("canonical reader failed previously".into());
        }
        self.read_frame(max_frame_bytes).map_err(|e| {
            self.failed = true;
            e.to_string()
        })
    }
}

struct Count {
    bytes: u64,
    limit: u64,
}
impl Write for Count {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.bytes = self
            .bytes
            .checked_add(bytes.len() as u64)
            .ok_or_else(|| invalid("frame length overflow"))?;
        if self.bytes > self.limit {
            return Err(invalid("canonical frame byte limit"));
        }
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub struct Writer<W> {
    output: W,
    header: Header,
    frames: u64,
    failed: bool,
}
impl<W: Write> Writer<W> {
    pub fn new(mut output: W, header: Header) -> io::Result<Self> {
        header.validate()?;
        output.write_all(&MAGIC)?;
        header.capture.encode(&mut output)?;
        match header.side {
            Side::Oracle => 0u8,
            Side::Native => 1u8,
        }
        .encode(&mut output)?;
        header.producer_revision.encode(&mut output)?;
        Ok(Self {
            output,
            header,
            frames: 0,
            failed: false,
        })
    }
    pub fn frame(&mut self, frame: &Frame, max_frame_bytes: usize) -> io::Result<()> {
        if self.failed {
            return Err(invalid("canonical writer failed previously"));
        }
        self.failed = true;
        if self.frames >= self.header.capture.frame_count
            || frame.present != self.header.capture.first_present + self.frames
        {
            return Err(invalid("frame outside declared capture extent"));
        }
        let mut count = Count {
            bytes: 0,
            limit: max_frame_bytes as u64,
        };
        frame.encode(&mut count)?;
        count.bytes.encode(&mut self.output)?;
        frame.encode(&mut self.output)?;
        self.frames += 1;
        self.failed = false;
        Ok(())
    }
    pub fn finish(mut self) -> io::Result<W> {
        if self.failed || self.frames != self.header.capture.frame_count {
            return Err(invalid("incomplete canonical capture"));
        }
        self.output.flush()?;
        Ok(self.output)
    }
}
