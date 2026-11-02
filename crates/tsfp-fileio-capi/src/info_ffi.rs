//! C ABI of the Rust file-information decision table (T2074 slice 2). `kernel_io.c` keeps guest memory access, the IO_STATUS
//! block and the backing mutations; with `--rust-fileio` it takes the class decisions from this table (`kernel_io_info_backend`).
use std::ffi::c_int;
use std::panic::{AssertUnwindSafe, catch_unwind};
use tsfp_engine_fileio::info::{self, FileFacts, SetPlan};

/// Mirrors `kernel_io_file_facts` in src/xbox/kernel_io.h.
#[repr(C)]
pub struct Facts {
    offset: u64,
    size: u64,
    is_directory: bool,
    host_dir: bool,
    disc_file: bool,
    disc_sector: u32,
}

/// Mirrors `kernel_io_info_reply`.
#[repr(C)]
pub struct Reply {
    status: u32,
    information: u32,
    payload: [u8; info::NETWORK_OPEN_BYTES],
    payload_bytes: u32,
    fabricated_timestamps: bool,
    unknown_class: bool,
}

unsafe extern "C" fn c_query(class: u32, length: u32, file: *const Facts, out: *mut Reply) {
    if file.is_null() || out.is_null() {
        return;
    }
    crate::counters::bump(crate::counters::INFO_QUERY);
    let f = unsafe { &*file };
    let facts = FileFacts {
        offset: f.offset,
        size: f.size,
        is_directory: f.is_directory,
        host_dir: f.host_dir,
        disc_file: f.disc_file,
        disc_sector: f.disc_sector,
    };
    let Ok(reply) = catch_unwind(AssertUnwindSafe(|| info::query(class, length, &facts))) else {
        unsafe { (*out).status = info::STATUS_INVALID_INFO_CLASS };
        return;
    };
    let out = unsafe { &mut *out };
    out.status = reply.status;
    out.information = reply.information;
    let n = reply.payload.len().min(out.payload.len());
    out.payload[..n].copy_from_slice(&reply.payload[..n]);
    out.payload_bytes = n as u32;
    out.fabricated_timestamps = reply.fabricated_timestamps;
    out.unknown_class = reply.unknown_class;
}

const REFUSE_UNKNOWN: c_int = -1;
const REFUSE_LENGTH: c_int = 0;
const SEEK: c_int = 1;
const ALLOCATION: c_int = 2;
const END_OF_FILE: c_int = 3;
const BASIC: c_int = 4;

unsafe extern "C" fn c_set_plan(
    class: u32,
    length: u32,
    have_value: c_int,
    value: u64,
    planned: *mut u64,
) -> c_int {
    crate::counters::bump(crate::counters::INFO_SET);
    let plan = catch_unwind(|| info::set_plan(class, length, (have_value != 0).then_some(value)));
    let (code, amount) = match plan {
        Ok(SetPlan::Seek(v)) => (SEEK, v),
        Ok(SetPlan::Allocation(v)) => (ALLOCATION, v),
        Ok(SetPlan::EndOfFile(v)) => (END_OF_FILE, v),
        Ok(SetPlan::Basic) => (BASIC, 0),
        Ok(SetPlan::Refuse {
            unknown_class: false,
            ..
        }) => (REFUSE_LENGTH, 0),
        _ => (REFUSE_UNKNOWN, 0),
    };
    if !planned.is_null() {
        unsafe { *planned = amount };
    }
    code
}

#[repr(C)]
pub struct InfoBackend {
    query: unsafe extern "C" fn(u32, u32, *const Facts, *mut Reply),
    set_plan: unsafe extern "C" fn(u32, u32, c_int, u64, *mut u64) -> c_int,
}
static BACKEND: InfoBackend = InfoBackend {
    query: c_query,
    set_plan: c_set_plan,
};

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_rfio_info_backend() -> *const InfoBackend {
    &BACKEND
}
