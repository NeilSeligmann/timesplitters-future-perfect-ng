//! Host descriptor I/O of the guest file layer (T2074 slice 8): positional read, positional write and truncate on a host
//! descriptor that `kernel_file.c` still opens and closes. Results follow the C contract of `kernel_file_desc_backend`:
//! a byte count (or 0 for truncate) on success, `-errno` on failure, so the C retry loops (EINTR) and the NTSTATUS mapping
//! stay unchanged. The descriptor is borrowed, never closed here.
//! The C ABI borrows the raw descriptor as a `File` (this crate forbids `unsafe`).
use std::fs::File;
use std::io;
use std::os::unix::fs::FileExt;

fn errno(error: io::Error) -> i64 {
    -i64::from(error.raw_os_error().unwrap_or(libc_eio()))
}

const fn libc_eio() -> i32 {
    5
}

/// One `pread`: bytes read (0 at end of file) or `-errno`.
pub fn read_at(file: &File, buffer: &mut [u8], offset: u64) -> i64 {
    match file.read_at(buffer, offset) {
        Ok(got) => got as i64,
        Err(error) => errno(error),
    }
}

/// One `pwrite`: bytes written or `-errno`.
pub fn write_at(file: &File, buffer: &[u8], offset: u64) -> i64 {
    match file.write_at(buffer, offset) {
        Ok(put) => put as i64,
        Err(error) => errno(error),
    }
}

/// `ftruncate` to `length`: 0 or `-errno`.
pub fn truncate(file: &File, length: u64) -> i64 {
    match file.set_len(length) {
        Ok(()) => 0,
        Err(error) => errno(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Seek, SeekFrom, Write};

    #[test]
    fn read_write_truncate_round_trip() {
        let path = std::env::temp_dir().join(format!("t2074-desc-{}", std::process::id()));
        let file = File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&path)
            .unwrap();
        let fd = &file;
        assert_eq!(write_at(fd, b"hello world", 4), 11);
        let mut out = [0u8; 15];
        assert_eq!(read_at(fd, &mut out, 0), 15);
        assert_eq!(&out, b"\0\0\0\0hello world");
        assert_eq!(truncate(fd, 9), 0);
        assert_eq!(read_at(fd, &mut out, 0), 9);
        assert_eq!(read_at(fd, &mut out, 9), 0);
        let read_only = File::open(&path).unwrap();
        assert_eq!(
            write_at(&read_only, b"x", 0),
            -9,
            "read-only descriptor reports EBADF"
        );
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn positional_io_keeps_the_borrowed_file_cursor_unchanged() {
        let path = std::env::temp_dir().join(format!("t2074-desc-cursor-{}", std::process::id()));
        let mut file = File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&path)
            .unwrap();
        file.write_all(b"0123456789").unwrap();
        file.seek(SeekFrom::Start(3)).unwrap();

        assert_eq!(write_at(&file, b"XY", 7), 2);
        let mut out = [0u8; 2];
        assert_eq!(read_at(&file, &mut out, 7), 2);
        assert_eq!(&out, b"XY");
        assert_eq!(file.stream_position().unwrap(), 3);

        std::fs::remove_file(path).unwrap();
    }
}
