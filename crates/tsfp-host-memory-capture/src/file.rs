// SPDX-License-Identifier: GPL-3.0-or-later
//! Publish only a fully sealed stream; leave incomplete files visibly partial.
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::Writer;

/// Temporary sink for a framed writer. Call `publish` only after successful seal.
pub(crate) struct AtomicFile {
    file: File,
    partial: PathBuf,
    destination: PathBuf,
}
impl AtomicFile {
    pub(crate) fn create(destination: &Path) -> io::Result<Self> {
        let mut partial_name = destination.as_os_str().to_os_string();
        partial_name.push(".part");
        let partial = PathBuf::from(partial_name);
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&partial)?;
        Ok(Self {
            file,
            partial,
            destination: destination.to_owned(),
        })
    }
    pub(crate) fn publish(self) -> io::Result<()> {
        self.file.sync_all()?;
        // Fails if destination exists; never overwrite another recording.
        fs::hard_link(&self.partial, &self.destination)?;
        fs::remove_file(&self.partial)?;
        Ok(())
    }
}
impl Write for AtomicFile {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        self.file.write(data)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}

pub struct CaptureFile {
    writer: Writer<AtomicFile>,
}
impl CaptureFile {
    pub fn create(destination: &Path, limit: u64) -> io::Result<Self> {
        Ok(Self {
            writer: Writer::new(AtomicFile::create(destination)?, limit)?,
        })
    }
    pub fn record(&mut self, kind: u32, data: &[u8]) -> io::Result<()> {
        self.writer.record(kind, data)
    }
    pub fn incomplete(&mut self) {
        self.writer.incomplete();
    }
    pub fn stop(&mut self) {
        self.writer.stop();
    }
    pub fn seal(self) -> io::Result<()> {
        self.writer.seal()?.publish()
    }
}
