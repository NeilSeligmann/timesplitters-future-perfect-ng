//! T3879: opt-in native attachment ABI. Distinct symbols keep the C oracle linkable.
use super::{Backend, CEntry, Visit, BACKEND, ERR_IMAGE, ERR_MALFORMED, OK};
use std::ffi::{CStr, c_char, c_void};
use std::fs::File;
use std::io::Write;
use std::os::unix::ffi::OsStrExt;
use std::sync::{Mutex, OnceLock};

/// Exact `xdvdfs_reader` layout from src/xbox/xdvdfs.h.
#[repr(C)]
pub struct Reader {
    fd: i32,
    image_bytes: u64,
    root_sector: u32,
    root_size: u32,
    open: bool,
    backend_handle: u32,
}
impl Default for Reader {
    fn default() -> Self {
        Self { fd: -1, image_bytes: 0, root_sector: 0, root_size: 0, open: false, backend_handle: 0 }
    }
}
fn backend() -> &'static Mutex<Backend> {
    static B: OnceLock<Mutex<Backend>> = OnceLock::new();
    B.get_or_init(|| Mutex::new(copy_backend(&BACKEND)))
}
fn copy_backend(b: &Backend) -> Backend {
    Backend { open: b.open, close: b.close, lookup: b.lookup, read: b.read, walk: b.walk }
}
fn current() -> Backend { copy_backend(&backend().lock().unwrap()) }
fn log() -> &'static Mutex<Option<File>> {
    static L: OnceLock<Mutex<Option<File>>> = OnceLock::new();
    L.get_or_init(|| Mutex::new(None))
}
fn record(bytes: &[u8]) {
    if let Some(file) = log().lock().unwrap().as_mut() {
        let _ = file.write_all(bytes);
        let _ = file.flush();
    }
}
unsafe fn path_bytes<'a>(path: *const c_char) -> &'a [u8] {
    if path.is_null() { b"" } else { unsafe { CStr::from_ptr(path) }.to_bytes() }
}

/// The backend must remain valid for this call; its function pointers must remain valid
/// while readers use it. NULL restores the native Rust backend.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_native_xdvdfs_set_backend(b: *const Backend) {
    *backend().lock().unwrap() = copy_backend(if b.is_null() { &BACKEND } else { unsafe { &*b } });
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_native_xdvdfs_result_str(result: i32) -> *const c_char {
    match result {
        0 => c"ok",
        1 => c"image could not be opened or stat'd",
        2 => c"no MICROSOFT*XBOX*MEDIA at byte offset 0x10000",
        3 => c"read fell outside the image or returned short",
        4 => c"directory blob or entry is malformed",
        5 => c"directory walk hit its node or pending limit",
        6 => c"path is not on the disc",
        _ => c"unknown xdvdfs result",
    }.as_ptr()
}

/// `path` is NULL (close logging) or a valid NUL-terminated OS path.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_native_xdvdfs_log_open(path: *const c_char) -> bool {
    let mut target = log().lock().unwrap();
    *target = None;
    if path.is_null() { return true; }
    match File::create(std::ffi::OsStr::from_bytes(unsafe { path_bytes(path) })) {
        Ok(file) => { *target = Some(file); true }
        Err(_) => false,
    }
}

/// Pointers must reference writable reader storage and a terminated path, or be NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_native_xdvdfs_open(reader: *mut Reader, path: *const c_char) -> i32 {
    let rc = if reader.is_null() || path.is_null() { ERR_IMAGE } else {
        // Write without reading: C callers may pass uninitialized stack storage.
        unsafe { reader.write(Reader::default()) };
        let r = unsafe { &mut *reader };
        let rc = unsafe { (current().open)(path, &mut r.backend_handle, &mut r.image_bytes) };
        if rc == OK { r.open = true; }
        rc
    };
    record(format!("open rc={rc}\n").as_bytes());
    rc
}

/// Reader must be initialized by open or zero initialized, or NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_native_xdvdfs_close(reader: *mut Reader) {
    if let Some(r) = unsafe { reader.as_mut() } {
        if r.backend_handle != 0 { unsafe { (current().close)(r.backend_handle) }; }
        *r = Reader::default();
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_native_xdvdfs_image_bytes(reader: *const Reader) -> u64 {
    unsafe { reader.as_ref() }.filter(|r| r.open).map_or(0, |r| r.image_bytes)
}

/// Non-NULL pointers have the same validity requirements as xdvdfs_lookup.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_native_xdvdfs_lookup(reader: *const Reader, path: *const c_char, out: *mut CEntry) -> i32 {
    let rc = match unsafe { reader.as_ref() } {
        Some(r) if r.open && !out.is_null() => {
            if path.is_null() { ERR_MALFORMED } else { unsafe { (current().lookup)(r.backend_handle, path, out) } }
        }
        _ => ERR_IMAGE,
    };
    let mut line = b"lookup ".to_vec();
    line.extend_from_slice(unsafe { path_bytes(path) });
    if rc == OK {
        let e = unsafe { &*out };
        line.extend_from_slice(format!(" rc=0 sector={} size={} attr={:02x}\n", e.start_sector, e.size, e.attributes).as_bytes());
    } else { line.extend_from_slice(format!(" rc={rc}\n").as_bytes()); }
    record(&line);
    rc
}

/// Non-NULL pointers have the same validity requirements as xdvdfs_read.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_native_xdvdfs_read(reader: *const Reader, entry: *const CEntry, offset: u64, buffer: *mut c_void, length: u32, out_read: *mut u32) -> i32 {
    if !out_read.is_null() { unsafe { *out_read = 0 }; }
    let rc = match unsafe { reader.as_ref() } {
        Some(r) if r.open && !entry.is_null() && !out_read.is_null() && (!buffer.is_null() || length == 0) => {
            unsafe { (current().read)(r.backend_handle, entry, offset, buffer.cast(), length, out_read) }
        }
        _ => ERR_IMAGE,
    };
    if let Some(e) = unsafe { entry.as_ref() } {
        let got = if out_read.is_null() { 0 } else { unsafe { *out_read } };
        let hash = if !buffer.is_null() && got > 0 {
            unsafe { std::slice::from_raw_parts(buffer.cast::<u8>(), got as usize) }.iter()
                .fold(0xcbf29ce484222325u64, |h, b| (h ^ u64::from(*b)).wrapping_mul(0x100000001b3))
        } else { 0 };
        record(format!("read sector={} off={offset} len={length} rc={rc} got={got} fnv={hash:016x}\n", e.start_sector).as_bytes());
    }
    rc
}

/// Reader and callback must remain valid throughout the synchronous walk.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_native_xdvdfs_walk(reader: *const Reader, visit: Option<Visit>, context: *mut c_void) -> i32 {
    match (unsafe { reader.as_ref() }, visit) {
        (Some(r), Some(v)) if r.open => unsafe { (current().walk)(r.backend_handle, v, context) },
        _ => ERR_IMAGE,
    }
}

#[cfg(test)]
#[path = "xdvdfs_ffi_tests.rs"]
mod tests;
