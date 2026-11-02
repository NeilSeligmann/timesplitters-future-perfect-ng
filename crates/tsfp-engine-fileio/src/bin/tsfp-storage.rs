//! Native local save storage resolver/migrator (T2319). No original bytes or guest writes.
use std::fs::File;
use std::io;
use std::os::unix::fs::FileExt;
use std::path::PathBuf;
use tsfp_engine_fileio::{
    fatx::{Fatx, FatxError, ReadAt},
    local_storage::{DirectorySource, FatxSource, LocalStorage, user_path},
};

struct Partition {
    file: File,
    offset: u64,
    length: u64,
}
impl ReadAt for Partition {
    fn read_at(&self, out: &mut [u8], offset: u64) -> Result<(), FatxError> {
        if offset
            .checked_add(out.len() as u64)
            .is_none_or(|end| end > self.length)
        {
            return Err(FatxError::Truncated);
        }
        self.file
            .read_exact_at(
                out,
                self.offset
                    .checked_add(offset)
                    .ok_or(FatxError::Truncated)?,
            )
            .map_err(|_| FatxError::Truncated)
    }
}
fn run() -> io::Result<bool> {
    let mut args = std::env::args().skip(1);
    let action = args.next().unwrap_or_else(|| "path".into());
    if action == "path" {
        if args.next().is_some() {
            return Err(io::Error::other("usage: tsfp-storage path"));
        }
        println!("{}", user_path()?.display());
        return Ok(true);
    }
    if action != "migrate" {
        return Err(io::Error::other(
            "usage: tsfp-storage path | migrate SOURCE [--destination DIR] [--offset BYTES --length BYTES]",
        ));
    }
    let source = PathBuf::from(
        args.next()
            .ok_or_else(|| io::Error::other("migrate requires SOURCE"))?,
    );
    let mut destination = None;
    let mut offset = 0u64;
    let mut length = None;
    while let Some(option) = args.next() {
        let value = args
            .next()
            .ok_or_else(|| io::Error::other("option requires a value"))?;
        match option.as_str() {
            "--destination" => destination = Some(PathBuf::from(value)),
            "--offset" => {
                offset = value
                    .parse()
                    .map_err(|_| io::Error::other("offset must be decimal bytes"))?
            }
            "--length" => {
                length = Some(
                    value
                        .parse::<u64>()
                        .map_err(|_| io::Error::other("length must be decimal bytes"))?,
                )
            }
            _ => return Err(io::Error::other(format!("unknown option {option}"))),
        }
    }
    let storage = LocalStorage::open(match destination {
        Some(path) => path,
        None => user_path()?,
    })?;
    let report = if source.is_dir() {
        if offset != 0 || length.is_some() {
            return Err(io::Error::other("offset/length apply only to images"));
        }
        storage.migrate(&DirectorySource(source))?
    } else {
        let file = File::open(source)?;
        let size = file.metadata()?.len();
        let length = length.unwrap_or_else(|| size.saturating_sub(offset));
        if length == 0 || offset.checked_add(length).is_none_or(|end| end > size) {
            return Err(io::Error::other("partition range outside image"));
        }
        let partition = Partition {
            file,
            offset,
            length,
        };
        let fatx = Fatx::open(partition, length)
            .map_err(|e| io::Error::other(format!("FATX open: {e:?}")))?;
        storage.migrate(&FatxSource(fatx))?
    };
    for path in &report.imported {
        println!("imported {}", path.display());
    }
    for path in &report.already_present {
        println!("unchanged {}", path.display());
    }
    for path in &report.conflicts {
        eprintln!("conflict {}; existing save retained", path.display());
    }
    Ok(report.conflicts.is_empty())
}
fn main() {
    match run() {
        Ok(true) => {}
        Ok(false) => std::process::exit(3),
        Err(e) => {
            eprintln!("tsfp-storage: {e}");
            std::process::exit(2);
        }
    }
}
