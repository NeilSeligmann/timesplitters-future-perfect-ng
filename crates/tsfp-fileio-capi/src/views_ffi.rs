//! C ABI of the Rust volume view choice (T2106, `kernel_file_view_backend`). With `--rust-fileio` the raw-vs-directory
//! choice and the FATX gate come from Rust, cross-checked against the C original (kernel_file_view_shadow_mismatches).
use std::ffi::{CStr, c_char, c_int};
use tsfp_engine_fileio::views::{VolumeFacts, choose, image_formatted};

// KERNEL_FILE_VIEW_* for `rest` on a volume with (has_device, cache_partition); -1 for a NULL rest.
unsafe extern "C" fn c_choose(has_device: c_int, cache_partition: u32, rest: *const c_char) -> c_int {
    if rest.is_null() {
        return -1;
    }
    crate::counters::bump(crate::counters::VIEWS);
    let rest = unsafe { CStr::from_ptr(rest) }.to_bytes();
    choose(VolumeFacts { has_device: has_device != 0, cache_partition }, rest) as c_int
}

// 1 when the `length` bytes read from the image's offset 0 start with "FATX".
unsafe extern "C" fn c_formatted(head: *const u8, length: usize) -> c_int {
    if head.is_null() {
        return 0;
    }
    c_int::from(image_formatted(unsafe { std::slice::from_raw_parts(head, length) }))
}

#[repr(C)]
pub struct ViewBackend {
    choose: unsafe extern "C" fn(c_int, u32, *const c_char) -> c_int,
    formatted: unsafe extern "C" fn(*const u8, usize) -> c_int,
}
static BACKEND: ViewBackend = ViewBackend { choose: c_choose, formatted: c_formatted };

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_rfio_view_backend() -> *const ViewBackend {
    &BACKEND
}
