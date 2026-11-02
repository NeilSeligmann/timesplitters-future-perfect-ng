//! Rust C ABI for the remaining kernel I/O operations.
//!
//! These imports resolve to the selected kernel-file runtime. Production callers
//! enter through this Rust boundary, while the C implementation remains available
//! as the explicit compatibility fallback until retirement is complete.
use std::ffi::{c_char, c_void};

unsafe extern "C" {
    fn kernel_file_volume_space(
        handle: u32,
        total: *mut u64,
        available: *mut u64,
        sectors: *mut u32,
        bytes_per_sector: *mut u32,
    ) -> bool;
    fn kernel_file_query_attributes(address: u32, out: *mut c_void) -> u32;
    fn kernel_file_open_set_offset(handle: u32, offset: u64) -> bool;
    fn kernel_file_set_allocation(handle: u32, length: u64, status: *mut u32) -> bool;
    fn kernel_file_set_end_of_file(handle: u32, length: u64, status: *mut u32) -> bool;
    fn kernel_file_rename_from_guest(handle: u32, address: u32, length: u32) -> u32;
    fn kernel_file_set_disposition(handle: u32, delete: bool) -> u32;
    fn kernel_file_read_backing(
        handle: u32,
        offset: u64,
        buffer: *mut c_void,
        length: u32,
        read: *mut u32,
    ) -> bool;
    fn kernel_file_read_backing_stored(
        handle: u32,
        offset: u64,
        buffer: *mut c_void,
        length: u32,
        read: *mut u32,
    ) -> bool;
    fn kernel_file_write_backing(
        handle: u32,
        offset: u64,
        buffer: *const c_void,
        length: u32,
        written: *mut u32,
        status: *mut u32,
    ) -> bool;
    fn kernel_file_flush_fatx(handle: u32, status: *mut u32) -> bool;
    fn kernel_file_dir_next(
        handle: u32,
        restart: bool,
        mask: *const c_char,
        max_name: u32,
        out: *mut c_void,
        status: *mut u32,
    ) -> bool;
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_rfio_volume_space(
    handle: u32,
    total: *mut u64,
    available: *mut u64,
    sectors: *mut u32,
    bytes_per_sector: *mut u32,
) -> bool {
    unsafe { kernel_file_volume_space(handle, total, available, sectors, bytes_per_sector) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_rfio_query_attributes(address: u32, out: *mut c_void) -> u32 {
    unsafe { kernel_file_query_attributes(address, out) }
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_rfio_open_set_offset(handle: u32, offset: u64) -> bool {
    unsafe { kernel_file_open_set_offset(handle, offset) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_rfio_set_allocation(
    handle: u32,
    length: u64,
    status: *mut u32,
) -> bool {
    unsafe { kernel_file_set_allocation(handle, length, status) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_rfio_set_end_of_file(
    handle: u32,
    length: u64,
    status: *mut u32,
) -> bool {
    unsafe { kernel_file_set_end_of_file(handle, length, status) }
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_rfio_rename_from_guest(handle: u32, address: u32, length: u32) -> u32 {
    unsafe { kernel_file_rename_from_guest(handle, address, length) }
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_rfio_set_disposition(handle: u32, delete: bool) -> u32 {
    unsafe { kernel_file_set_disposition(handle, delete) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_rfio_read_backing(
    handle: u32,
    offset: u64,
    buffer: *mut c_void,
    length: u32,
    read: *mut u32,
) -> bool {
    unsafe { kernel_file_read_backing(handle, offset, buffer, length, read) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_rfio_read_backing_stored(
    handle: u32,
    offset: u64,
    buffer: *mut c_void,
    length: u32,
    read: *mut u32,
) -> bool {
    unsafe { kernel_file_read_backing_stored(handle, offset, buffer, length, read) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_rfio_write_backing(
    handle: u32,
    offset: u64,
    buffer: *const c_void,
    length: u32,
    written: *mut u32,
    status: *mut u32,
) -> bool {
    unsafe { kernel_file_write_backing(handle, offset, buffer, length, written, status) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_rfio_flush_fatx(handle: u32, status: *mut u32) -> bool {
    unsafe { kernel_file_flush_fatx(handle, status) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_rfio_dir_next(
    handle: u32,
    restart: bool,
    mask: *const c_char,
    max_name: u32,
    out: *mut c_void,
    status: *mut u32,
) -> bool {
    unsafe { kernel_file_dir_next(handle, restart, mask, max_name, out, status) }
}
