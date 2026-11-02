// SPDX-License-Identifier: GPL-3.0-or-later
//! The C modules the MU device still calls: FATX volume code, the guest accessors, the file-system namespace and
//! the XInput dispatcher. One `pub fn` per line: `cmake/rust-input-hle.cmake` reads this list to force the members in.
use std::ffi::{c_char, c_int, c_void};

#[repr(C)]
pub struct Volume {
    pub image: *mut u8,
    pub size: usize,
    pub volume_id: u32,
    pub sectors_per_cluster: u32,
    pub root_cluster: u32,
    pub cluster_count: u32,
    pub fat_offset: usize,
    pub fat_size: usize,
    pub data_offset: usize,
    pub fat32: bool,
}

pub type Job = unsafe extern "C" fn(*mut c_void);
pub type Handler = unsafe extern "C" fn(*mut c_void) -> u32;
pub type LogFn = unsafe extern "C" fn(*const c_char, ...) -> c_int;

#[repr(C)]
pub struct Mutex {
    pub storage: [u64; 8],
}

unsafe extern "C" {
    pub fn fatx_format(image: *mut u8, size: usize, volume_id: u32) -> u32;
    pub fn fatx_open(volume: *mut Volume, image: *mut u8, size: usize) -> u32;
    pub fn fatx_mkdir_path(volume: *mut Volume, path: *const c_char) -> u32;
    pub fn fatx_stat_path(
        volume: *const Volume,
        path: *const c_char,
        directory: *mut bool,
        size: *mut u32,
    ) -> u32;
    pub fn kernel_guest_at_ro(address: u32, length: usize) -> *const c_void;
    pub fn kernel_guest_write_bytes(address: u32, source: *const c_void, length: usize) -> bool;
    pub fn kernel_guest_read_u32(address: u32, out: *mut u32) -> bool;
    pub fn kernel_guest_write_u8(address: u32, value: u8) -> bool;
    pub fn kernel_frame_arg(frame: *const c_void, index: u32, out: *mut u32) -> bool;
    pub fn tsfp_rfio_mount_fatx(
        prefix: *const c_char,
        image: *mut Volume,
        flush: unsafe extern "C" fn(*mut c_void) -> bool,
        context: *mut c_void,
    ) -> bool;
    pub fn tsfp_rfio_unmount_fatx(prefix: *const c_char) -> bool;
    pub fn tsfp_rfio_add_symlink(name: *const c_char, target: *const c_char) -> bool;
    pub fn tsfp_rfio_remove_symlink(name: *const c_char) -> bool;
    pub fn tsfp_rfio_run_locked(job: Job, context: *mut c_void);
    pub fn xinput_hle_register_identified(address: u32, handler: Handler, id: u32) -> bool;
    pub fn xinput_hle_log() -> LogFn;
    pub fn xinput_devices_run_locked(job: Job, context: *mut c_void);
    pub fn xinput_devices_set_mu_mask(mask: u32);
    pub fn pthread_mutex_lock(mutex: *mut Mutex) -> c_int;
    pub fn pthread_mutex_unlock(mutex: *mut Mutex) -> c_int;
}
