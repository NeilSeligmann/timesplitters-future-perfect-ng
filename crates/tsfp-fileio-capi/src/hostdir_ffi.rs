//! C ABI of the Rust host-dir walk (T2074 slice 2). `src/xbox/kernel_file.c` keeps its C walk (`c_hostdir_*`) and routes
//! `hostdir_walk_locked` / `hostdir_real_name_locked` through the `kernel_file_hostdir_backend` table when the host runs
//! with `--rust-fileio`. The OS primitives stay in C and arrive as [`COps`] (mirror of `kernel_file_hostdir_ops`).
use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::panic::{AssertUnwindSafe, catch_unwind};
use tsfp_engine_fileio::hostdir::{self as h, HostOps, Kind, WalkError};

/// Mirror of `kernel_file_hostdir_ops` in `src/xbox/kernel_file.h`.
#[repr(C)]
pub struct COps {
    dup_fd: unsafe extern "C" fn(c_int) -> c_int,
    close_fd: unsafe extern "C" fn(c_int),
    /// -1 missing, 0 other, 1 directory, 2 symbolic link; fills the `struct stat` behind `info`.
    stat_at: unsafe extern "C" fn(c_int, *const c_char, *mut c_void) -> c_int,
    open_dir: unsafe extern "C" fn(c_int, *const c_char) -> c_int,
    scan_open: unsafe extern "C" fn(c_int) -> *mut c_void,
    scan_next: unsafe extern "C" fn(*mut c_void) -> *const c_char,
    scan_close: unsafe extern "C" fn(*mut c_void),
}

struct Bridge<'a> {
    ops: &'a COps,
    info: *mut c_void,
}

impl HostOps for Bridge<'_> {
    fn dup(&mut self, fd: i32) -> Option<i32> {
        let r = unsafe { (self.ops.dup_fd)(fd) };
        (r >= 0).then_some(r)
    }
    fn close(&mut self, fd: i32) {
        unsafe { (self.ops.close_fd)(fd) }
    }
    fn stat_at(&mut self, dir: i32, name: &[u8]) -> Option<Kind> {
        let c = CString::new(name).ok()?;
        match unsafe { (self.ops.stat_at)(dir, c.as_ptr(), self.info) } {
            0 => Some(Kind::Other),
            1 => Some(Kind::Dir),
            2 => Some(Kind::Link),
            _ => None,
        }
    }
    fn open_dir(&mut self, dir: i32, name: &[u8]) -> Option<i32> {
        let c = CString::new(name).ok()?;
        let r = unsafe { (self.ops.open_dir)(dir, c.as_ptr()) };
        (r >= 0).then_some(r)
    }
    fn scan(&mut self, dir: i32, visit: &mut dyn FnMut(&[u8]) -> bool) -> bool {
        let it = unsafe { (self.ops.scan_open)(dir) };
        if it.is_null() {
            return false;
        }
        loop {
            let p = unsafe { (self.ops.scan_next)(it) };
            if p.is_null() || visit(unsafe { CStr::from_ptr(p) }.to_bytes()) {
                break;
            }
        }
        unsafe { (self.ops.scan_close)(it) };
        true
    }
}

/// Error codes of `walk`: >= 0 is the descriptor, the rest mirror `kernel_file_hostdir_err` in the C header.
const E_NO_START: c_int = -1;
const E_NO_DESCRIPTOR: c_int = -2;
const E_TOO_LONG: c_int = -3;
const E_DOT: c_int = -4;
const E_PATH_NOT_FOUND: c_int = -5;
const E_LINK: c_int = -6;
const E_INTERNAL: c_int = -7;

unsafe fn put(out: *mut c_char, cap: usize, bytes: &[u8]) {
    if cap == 0 {
        return;
    }
    let n = bytes.len().min(cap - 1);
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), out.cast::<u8>(), n);
        *out.add(n) = 0;
    }
}

unsafe extern "C" fn c_walk(
    ops: *const COps,
    info: *mut c_void,
    start_fd: c_int,
    rest: *const c_char,
    pending_cap: usize,
    leaf: *mut c_char,
    leaf_bytes: usize,
    detail: *mut usize,
) -> c_int {
    crate::counters::bump(crate::counters::HOSTDIR_WALK);
    if ops.is_null() || rest.is_null() || leaf.is_null() {
        return E_INTERNAL;
    }
    unsafe { *leaf = 0 };
    catch_unwind(AssertUnwindSafe(|| {
        let mut bridge = Bridge {
            ops: unsafe { &*ops },
            info,
        };
        match h::walk(
            &mut bridge,
            start_fd,
            unsafe { CStr::from_ptr(rest) }.to_bytes(),
            pending_cap,
            leaf_bytes,
        ) {
            Ok((fd, name)) => {
                unsafe { put(leaf, leaf_bytes, &name) };
                fd
            }
            Err(WalkError::NoStart) => E_NO_START,
            Err(WalkError::NoDescriptor) => E_NO_DESCRIPTOR,
            Err(WalkError::TooLong(n)) => {
                if !detail.is_null() {
                    unsafe { *detail = n };
                }
                E_TOO_LONG
            }
            Err(WalkError::Dot) => E_DOT,
            Err(WalkError::PathNotFound) => E_PATH_NOT_FOUND,
            Err(WalkError::Link(real)) => {
                unsafe { put(leaf, leaf_bytes, &real) };
                E_LINK
            }
        }
    }))
    .unwrap_or(E_INTERNAL)
}

unsafe extern "C" fn c_real_name(
    ops: *const COps,
    info: *mut c_void,
    dir: c_int,
    wanted: *const c_char,
    out: *mut c_char,
    out_bytes: usize,
) -> c_int {
    crate::counters::bump(crate::counters::HOSTDIR_NAME);
    if ops.is_null() || wanted.is_null() || out.is_null() {
        return 0;
    }
    catch_unwind(AssertUnwindSafe(|| {
        let mut bridge = Bridge {
            ops: unsafe { &*ops },
            info,
        };
        match h::real_name(
            &mut bridge,
            dir,
            unsafe { CStr::from_ptr(wanted) }.to_bytes(),
            out_bytes,
        ) {
            Some((name, _)) => {
                unsafe { put(out, out_bytes, &name) };
                1
            }
            None => 0,
        }
    }))
    .unwrap_or(0)
}

#[repr(C)]
pub struct HostdirBackend {
    walk: unsafe extern "C" fn(
        *const COps,
        *mut c_void,
        c_int,
        *const c_char,
        usize,
        *mut c_char,
        usize,
        *mut usize,
    ) -> c_int,
    real_name: unsafe extern "C" fn(
        *const COps,
        *mut c_void,
        c_int,
        *const c_char,
        *mut c_char,
        usize,
    ) -> c_int,
}
static BACKEND: HostdirBackend = HostdirBackend {
    walk: c_walk,
    real_name: c_real_name,
};

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_rfio_hostdir_backend() -> *const HostdirBackend {
    &BACKEND
}
