//! C ABI of the Rust host descriptor I/O (T2074 slice 8). `kernel_file.c` retains descriptor ownership; the separate hostio backend opens and closes them. With
//! `--rust-fileio` the read, write and truncate calls of open guest files go through `kernel_file_desc_backend`.
use std::ffi::{c_int, c_void};
use std::fs::File;
use std::mem::ManuallyDrop;
use std::os::fd::FromRawFd;
use tsfp_engine_fileio::desc;

const EBADF: i64 = -9;

/// Borrows `fd` as a `File` that is never closed here; None for a negative descriptor.
fn borrow(fd: c_int) -> Option<ManuallyDrop<File>> {
    // SAFETY: kernel_file.c owns `fd` for the duration of the call, ManuallyDrop keeps it open.
    (fd >= 0).then(|| ManuallyDrop::new(unsafe { File::from_raw_fd(fd) }))
}

unsafe extern "C" fn c_pread(fd: c_int, buffer: *mut c_void, length: usize, offset: u64) -> i64 {
    crate::counters::bump(crate::counters::DESC);
    if buffer.is_null() && length != 0 {
        return -14; // EFAULT
    }
    let slice = if length == 0 {
        &mut [][..]
    } else {
        unsafe { std::slice::from_raw_parts_mut(buffer.cast::<u8>(), length) }
    };
    borrow(fd).map_or(EBADF, |file| desc::read_at(&file, slice, offset))
}

unsafe extern "C" fn c_pwrite(fd: c_int, buffer: *const c_void, length: usize, offset: u64) -> i64 {
    crate::counters::bump(crate::counters::DESC);
    if buffer.is_null() && length != 0 {
        return -14;
    }
    let slice = if length == 0 {
        &[][..]
    } else {
        unsafe { std::slice::from_raw_parts(buffer.cast::<u8>(), length) }
    };
    borrow(fd).map_or(EBADF, |file| desc::write_at(&file, slice, offset))
}

unsafe extern "C" fn c_truncate(fd: c_int, length: u64) -> i64 {
    crate::counters::bump(crate::counters::DESC);
    borrow(fd).map_or(EBADF, |file| desc::truncate(&file, length))
}

/// Mirrors `kernel_file_desc_backend` in src/xbox/kernel_file.h.
#[repr(C)]
pub struct DescBackend {
    pread: unsafe extern "C" fn(c_int, *mut c_void, usize, u64) -> i64,
    pwrite: unsafe extern "C" fn(c_int, *const c_void, usize, u64) -> i64,
    truncate: unsafe extern "C" fn(c_int, u64) -> i64,
}
static BACKEND: DescBackend = DescBackend {
    pread: c_pread,
    pwrite: c_pwrite,
    truncate: c_truncate,
};

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_rfio_desc_backend() -> *const DescBackend {
    &BACKEND
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Seek, SeekFrom, Write};
    use std::os::fd::AsRawFd;

    #[test]
    fn exported_backend_preserves_cursor_and_reports_bad_descriptors() {
        let path = std::env::temp_dir().join(format!("t2225-desc-ffi-{}", std::process::id()));
        let mut file = std::fs::File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&path)
            .unwrap();
        file.write_all(b"0123456789").unwrap();
        file.seek(SeekFrom::Start(3)).unwrap();
        let fd = file.as_raw_fd();
        let backend = unsafe { &*tsfp_rfio_desc_backend() };
        let mut out = [0u8; 2];

        assert_eq!(
            unsafe { (backend.pread)(fd, out.as_mut_ptr().cast(), out.len(), 7) },
            2
        );
        assert_eq!(&out, b"78");
        assert_eq!(file.stream_position().unwrap(), 3);
        assert_eq!(
            unsafe { (backend.pwrite)(fd, b"XY".as_ptr().cast(), 2, 7) },
            2
        );
        assert_eq!(file.stream_position().unwrap(), 3);
        assert_eq!(unsafe { (backend.truncate)(fd, 8) }, 0);
        assert_eq!(file.metadata().unwrap().len(), 8);
        assert_eq!(
            unsafe { (backend.pread)(-1, out.as_mut_ptr().cast(), 2, 0) },
            EBADF
        );

        std::fs::remove_file(path).unwrap();
    }
}
