//! C ABI of the Rust disc reader for the hybrid host (T2056, step S1 of docs/decisions/rust-only-build.md).
//!
//! `src/xbox/xdvdfs.c` keeps its C implementation and, when the host is started with
//! `--rust-fileio` (or `TSFP_RUST_FILEIO=1`), routes `xdvdfs_open/close/lookup/read/walk` here
//! through the `xdvdfs_backend` table installed by [`tsfp_rfio_install`]. The struct layouts below
//! mirror `xdvdfs_entry` in `src/xbox/xdvdfs.h`; the result codes mirror `xdvdfs_result`.
mod counters;
mod setup_ffi;
mod report_ffi;
mod host_report_ffi;
mod identity_ffi;
mod desc_ffi;
mod dir_ffi;
mod commit_ffi;
mod disposition_ffi;
mod create_ffi;
mod rename_ffi;
mod lifecycle_ffi;
mod activity_ffi;
mod fatx_ffi;
mod info_ffi;
mod handles_ffi;
mod namespace_ffi;
mod views_ffi;
mod glue_ffi;
mod payload_ffi;
mod hostdir_ffi;
mod xdvdfs_ffi;
mod kernel_register_ffi;
mod kernel_io_ops_ffi;
// Hybrid mode installs Rust backend tables into the retained C oracle. The
// native file-I/O build has no C kernel_file backend setters to install into.
#[cfg(not(feature = "native-fileio"))]
mod backend_setup_ffi;
mod share_ffi;

use std::collections::HashMap;
use std::ffi::{CStr, c_char, c_void};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use tsfp_engine_fileio::xiso::{Entry, Xiso, XisoError};

pub const OK: i32 = 0;
pub const ERR_IMAGE: i32 = 1;
pub const ERR_MAGIC: i32 = 2;
pub const ERR_TRUNCATED: i32 = 3;
pub const ERR_MALFORMED: i32 = 4;
pub const ERR_WALK_LIMIT: i32 = 5;
pub const ERR_NOT_FOUND: i32 = 6;

/// Same layout as the C `xdvdfs_entry`.
#[repr(C)]
pub struct CEntry {
    pub name: [u8; 256],
    pub name_length: u8,
    pub start_sector: u32,
    pub size: u32,
    pub attributes: u8,
    pub is_directory: bool,
}

fn code(e: &XisoError) -> i32 {
    match e {
        XisoError::Image(_) => ERR_IMAGE,
        XisoError::Magic => ERR_MAGIC,
        XisoError::Truncated => ERR_TRUNCATED,
        XisoError::Malformed(_) => ERR_MALFORMED,
        XisoError::WalkLimit => ERR_WALK_LIMIT,
        XisoError::NotFound => ERR_NOT_FOUND,
    }
}

fn to_c(e: &Entry, out: &mut CEntry) {
    out.name = [0; 256];
    let n = e.name.len().min(255);
    out.name[..n].copy_from_slice(&e.name[..n]);
    out.name_length = n as u8;
    out.start_sector = e.start_sector;
    out.size = e.size;
    out.attributes = e.attributes;
    out.is_directory = e.is_directory();
}

fn from_c(c: &CEntry) -> Entry {
    Entry { name: c.name[..usize::from(c.name_length)].to_vec(), start_sector: c.start_sector, size: c.size, attributes: c.attributes }
}

#[derive(Default)]
/// T2143: images are shared (`Arc`) so that `read` and `lookup` do not hold the table lock across a disc read. Before, one guest
/// thread waiting on a cold disc block kept every other thread (the movie stream reader, the loader, the audio packet reader) from
/// even starting its own read, which serialised reads the C reader (`pread` without a lock) overlaps.
struct Table { next: u32, images: HashMap<u32, Arc<Image>> }

unsafe extern "C" {
    fn posix_fadvise(fd: i32, offset: i64, length: i64, advice: i32) -> i32;
}
const FADV_SEQUENTIAL: i32 = 2;
const FADV_WILLNEED: i32 = 3;
/// How far ahead of a sequential read the kernel is asked to fetch.
const READAHEAD_BYTES: u64 = 1 << 20;

/// T2143: an open image plus the readahead state. A read that starts where the previous one ended is sequential (the movie stream, a
/// PAK being loaded), so the next `READAHEAD_BYTES` are requested from the kernel with `posix_fadvise(WILLNEED)`: advice only, it
/// never blocks and never changes the bytes returned. The C reader does no readahead of its own, so on a cold disc this is where the
/// Rust reader can be faster than the C one instead of slower.
struct Image { xiso: Xiso, last_read_end: AtomicU64 }
impl Image {
    fn advise_after(&self, start: u64, bytes: usize) {
        let end = start + bytes as u64;
        if self.last_read_end.swap(end, Ordering::Relaxed) == start && bytes >= 4096 {
            let ahead = READAHEAD_BYTES.min(self.xiso.image_bytes().saturating_sub(end));
            if ahead > 0 {
                // SAFETY: the descriptor lives as long as the image; posix_fadvise only advises the page cache.
                unsafe { posix_fadvise(self.xiso.raw_fd(), end as i64, ahead as i64, FADV_WILLNEED) };
            }
        }
    }
}
fn table() -> &'static Mutex<Table> {
    static T: OnceLock<Mutex<Table>> = OnceLock::new();
    T.get_or_init(|| Mutex::new(Table { next: 1, images: HashMap::new() }))
}

/// The image of `handle`, the table lock released before the caller touches the disc.
fn image(handle: u32) -> Option<Arc<Image>> { table().lock().unwrap().images.get(&handle).map(Arc::clone) }

/// Open an image. Returns the result code; `*handle` is non-zero on success and also reports the image size.
///
/// # Safety
/// `path` is a NUL-terminated string, `handle` and `image_bytes` are valid for writes.
pub unsafe extern "C" fn open(path: *const c_char, handle: *mut u32, image_bytes: *mut u64) -> i32 {
    if path.is_null() || handle.is_null() { return ERR_IMAGE; }
    let Ok(p) = unsafe { CStr::from_ptr(path) }.to_str() else { return ERR_IMAGE };
    match Xiso::open(Path::new(p)) {
        Ok(x) => {
            let mut t = table().lock().unwrap();
            let id = t.next;
            t.next += 1;
            if !image_bytes.is_null() { unsafe { *image_bytes = x.image_bytes() } }
            // SAFETY: as above, advice for the whole file.
            unsafe { posix_fadvise(x.raw_fd(), 0, 0, FADV_SEQUENTIAL) };
            t.images.insert(id, Arc::new(Image { xiso: x, last_read_end: AtomicU64::new(u64::MAX) }));
            unsafe { *handle = id };
            OK
        }
        Err(e) => code(&e),
    }
}
/// # Safety
/// Plain value argument.
pub unsafe extern "C" fn close(handle: u32) { table().lock().unwrap().images.remove(&handle); }
/// # Safety
/// `path` is NUL-terminated, `out` valid for writes.
pub unsafe extern "C" fn lookup(handle: u32, path: *const c_char, out: *mut CEntry) -> i32 {
    if path.is_null() || out.is_null() { return ERR_MALFORMED; }
    let Some(x) = image(handle) else { return ERR_IMAGE };
    let Ok(p) = unsafe { CStr::from_ptr(path) }.to_str() else { return ERR_NOT_FOUND };
    match x.xiso.lookup(p) {
        Ok(e) => { to_c(&e, unsafe { &mut *out }); OK }
        Err(e) => code(&e),
    }
}
/// # Safety
/// `entry` valid for reads, `buffer` valid for `length` writes, `out_read` valid for writes.
pub unsafe extern "C" fn read(handle: u32, entry: *const CEntry, offset: u64, buffer: *mut u8, length: u32, out_read: *mut u32) -> i32 {
    if entry.is_null() || out_read.is_null() || (buffer.is_null() && length > 0) { return ERR_IMAGE; }
    unsafe { *out_read = 0 };
    let Some(x) = image(handle) else { return ERR_IMAGE };
    let e = from_c(unsafe { &*entry });
    let buf: &mut [u8] = if length == 0 { &mut [] } else { unsafe { std::slice::from_raw_parts_mut(buffer, length as usize) } };
    match x.xiso.read(&e, offset, buf) {
        Ok(n) => {
            x.advise_after(Xiso::image_offset(&e, offset), n);
            unsafe { *out_read = n as u32 };
            OK
        }
        Err(e) => code(&e),
    }
}
type Visit = unsafe extern "C" fn(*mut c_void, *const c_char, *const CEntry) -> bool;
/// # Safety
/// `visit` is a valid callback, `context` is passed through.
pub unsafe extern "C" fn walk(handle: u32, visit: Visit, context: *mut c_void) -> i32 {
    let all = {
        let Some(x) = image(handle) else { return ERR_IMAGE };
        match x.xiso.walk() { Ok(v) => v, Err(e) => return code(&e) }
    };
    for (path, e) in all {
        let mut c = CEntry { name: [0; 256], name_length: 0, start_sector: 0, size: 0, attributes: 0, is_directory: false };
        to_c(&e, &mut c);
        let Ok(p) = std::ffi::CString::new(path) else { continue };
        if !unsafe { visit(context, p.as_ptr(), &c) } { break; }
    }
    OK
}

/// The function table `src/xbox/xdvdfs.c` accepts (`xdvdfs_backend`).
#[repr(C)]
pub struct Backend {
    pub open: unsafe extern "C" fn(*const c_char, *mut u32, *mut u64) -> i32,
    pub close: unsafe extern "C" fn(u32),
    pub lookup: unsafe extern "C" fn(u32, *const c_char, *mut CEntry) -> i32,
    pub read: unsafe extern "C" fn(u32, *const CEntry, u64, *mut u8, u32, *mut u32) -> i32,
    pub walk: unsafe extern "C" fn(u32, Visit, *mut c_void) -> i32,
}
static BACKEND: Backend = Backend { open, close, lookup, read, walk };

/// Pointer to the static backend table, to hand to `xdvdfs_set_backend`.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_rfio_backend() -> *const Backend { &BACKEND }

mod hostio_ffi;

mod async_ffi;

mod file_object_ffi;

mod kernel_io_ffi;

mod open_info_ffi;
