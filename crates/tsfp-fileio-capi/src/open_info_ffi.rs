//! Rust ABI for reading the selected kernel_file open record.
use std::ffi::c_void;

unsafe extern "C" {
    fn kernel_file_open_info(handle: u32, out: *mut c_void) -> bool;
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_rfio_open_info(handle: u32, out: *mut c_void) -> bool {
    if out.is_null() {
        return false;
    }
    unsafe { kernel_file_open_info(handle, out) }
}
