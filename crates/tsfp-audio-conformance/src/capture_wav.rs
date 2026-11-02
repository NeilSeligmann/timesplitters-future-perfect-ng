//! Fallible, bounded-memory evidence sink (classic RIFF, stereo i16).
use std::{
    fs::File,
    io::{self, BufWriter, Seek, SeekFrom, Write},
    path::Path,
};

pub(crate) struct PcmWav {
    file: BufWriter<File>,
    bytes: u32,
}
impl PcmWav {
    pub(crate) fn new(path: &Path, rate: u32) -> io::Result<Self> {
        let mut file = BufWriter::new(File::create_new(path)?);
        file.write_all(b"RIFF\0\0\0\0WAVEfmt \x10\0\0\0\x01\0\x02\0")?;
        file.write_all(&rate.to_le_bytes())?;
        file.write_all(&(rate * 4).to_le_bytes())?;
        file.write_all(b"\x04\0\x10\0data\0\0\0\0")?;
        Ok(Self { file, bytes: 0 })
    }
    pub(crate) fn write(&mut self, pcm: &[i16]) -> io::Result<()> {
        let bytes = u32::try_from(pcm.len())
            .ok()
            .and_then(|n| n.checked_mul(2))
            .and_then(|n| self.bytes.checked_add(n))
            .filter(|&n| n <= u32::MAX - 36)
            .ok_or_else(|| io::Error::other("PCM exceeds RIFF size bound"))?;
        for sample in pcm {
            self.file.write_all(&sample.to_le_bytes())?;
        }
        self.bytes = bytes;
        Ok(())
    }
    pub(crate) fn finish(mut self) -> io::Result<()> {
        self.file.seek(SeekFrom::Start(4))?;
        self.file.write_all(&(self.bytes + 36).to_le_bytes())?;
        self.file.seek(SeekFrom::Start(40))?;
        self.file.write_all(&self.bytes.to_le_bytes())?;
        self.file.flush()?;
        self.file.get_ref().sync_all()
    }
}
