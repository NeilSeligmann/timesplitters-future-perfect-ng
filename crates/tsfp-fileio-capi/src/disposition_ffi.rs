//! C ABI of the Rust CreateDisposition decision (T2074 slice 7). `kernel_file.c` keeps resolution, truncation, creation and
//! the IO_STATUS block; with `--rust-fileio` NtCreateFile takes the plan from here (`kernel_file_disposition_backend`).
use std::ffi::c_int;
use tsfp_engine_fileio::disposition::{self, Plan, Target};

/// Mirrors `kernel_file_disposition_facts` in src/xbox/kernel_file.h.
#[repr(C)]
pub struct Facts {
    pub exists: bool,
    pub name_not_found: bool,
    pub fatx: bool,
    pub host_dir: bool,
    pub is_directory: bool,
    pub writable: bool,
    pub has_host_fd: bool,
    pub device: bool,
}

// Mirrors the KERNEL_FILE_DISPOSITION_* plan codes.
const COLLISION: c_int = 0;
const TRUNCATE_FATX: c_int = 1;
const TRUNCATE_HOST: c_int = 2;
const REFUSE_OVERWRITE: c_int = 3;
const OPEN: c_int = 4;
const TRY_CREATE: c_int = 5;
const REFUSE: c_int = 6;

unsafe extern "C" fn c_plan(disposition: u32, facts: *const Facts, information: *mut u32) -> c_int {
    if facts.is_null() {
        return REFUSE;
    }
    crate::counters::bump(crate::counters::DISPOSITION);
    let f = unsafe { &*facts };
    let target = Target {
        exists: f.exists,
        name_not_found: f.name_not_found,
        fatx: f.fatx,
        host_dir: f.host_dir,
        is_directory: f.is_directory,
        writable: f.writable,
        has_host_fd: f.has_host_fd,
        device: f.device,
    };
    let (code, info) = match disposition::plan(disposition, &target) {
        Plan::Collision => (COLLISION, disposition::INFO_EXISTS),
        Plan::TruncateFatx { information } => (TRUNCATE_FATX, information),
        Plan::TruncateHost { information } => (TRUNCATE_HOST, information),
        Plan::RefuseOverwrite => (REFUSE_OVERWRITE, 0),
        Plan::Open => (OPEN, disposition::INFO_OPENED),
        Plan::TryCreate => (TRY_CREATE, disposition::INFO_CREATED),
        Plan::Refuse => (REFUSE, 0),
    };
    if !information.is_null() {
        unsafe { *information = info };
    }
    code
}

#[repr(C)]
pub struct DispositionBackend {
    plan: unsafe extern "C" fn(u32, *const Facts, *mut u32) -> c_int,
}
static BACKEND: DispositionBackend = DispositionBackend { plan: c_plan };

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_rfio_disposition_backend() -> *const DispositionBackend {
    &BACKEND
}
