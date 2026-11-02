//! Rust-owned host reporting bridge for the selected file-I/O ABI.
//!
//! This keeps the report's formatter and output stable while moving collection
//! of file-I/O state out of a production C caller. The selected `kernel_file_*`
//! symbols still choose the Rust default or the explicit legacy fallback.
use crate::report_ffi::{HostReportCounts, tsfp_rfio_host_report_snapshot};
use libc::{FILE, c_char, c_uint, c_ulonglong, c_void};

type AttemptAt = unsafe extern "C" fn(c_uint) -> *const c_void;
type LinkTarget = unsafe extern "C" fn(*const c_char) -> *const c_char;

unsafe extern "C" {
    fn kernel_file_volume_count() -> c_uint;
    fn kernel_file_open_count() -> c_uint;
    fn kernel_file_handle_shadow_mismatches() -> c_uint;
    fn kernel_file_share_conflicts_seen() -> c_uint;
    fn kernel_file_share_refusals() -> c_uint;
    fn kernel_file_payload_shadow_mismatches() -> c_uint;
    fn kernel_file_disposition_shadow_mismatches() -> c_uint;
    fn kernel_file_commit_shadow_mismatches() -> c_uint;
    fn kernel_file_namespace_shadow_mismatches() -> c_uint;
    fn kernel_file_view_shadow_mismatches() -> c_uint;
    fn kernel_file_glue_shadow_mismatches() -> c_uint;
    fn kernel_file_attempt_count() -> c_uint;
    fn kernel_file_refused_count() -> c_uint;
    fn kernel_file_relative_refused_count() -> c_uint;
    fn kernel_file_fabricated_count() -> c_uint;
    fn kernel_file_escape_refused_count() -> c_uint;
    fn kernel_file_disc_opened_count() -> c_uint;
    fn kernel_file_disc_bytes_read() -> c_ulonglong;
    fn kernel_file_host_opened_count() -> c_uint;
    fn kernel_file_host_bytes_read() -> c_ulonglong;
    fn kernel_file_created_count() -> c_uint;
    fn kernel_file_attempt_at(index: c_uint) -> *const c_void;
    fn kernel_file_symlink_count() -> c_uint;
    fn kernel_file_symlink_target(name: *const c_char) -> *const c_char;
    fn kernel_io_bytes_written() -> c_ulonglong;
    fn kernel_io_write_count() -> c_uint;
    fn kernel_io_write_refused_count() -> c_uint;
    fn host_report_file_attempts(
        out: *mut FILE,
        counts: *const HostReportCounts,
        attempt_at: AttemptAt,
    );
    fn host_report_symbolic_links(
        out: *mut FILE,
        total: c_uint,
        target_of: LinkTarget,
    );
}

/// Read a file-I/O diagnostic counter through the Rust host boundary.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_rfio_fileio_report_counter(index: u32) -> c_uint {
    match index {
        0 => unsafe { kernel_file_volume_count() },
        1 => unsafe { kernel_file_open_count() },
        2 => unsafe { kernel_file_handle_shadow_mismatches() },
        3 => unsafe { kernel_file_share_conflicts_seen() },
        4 => unsafe { kernel_file_share_refusals() },
        5 => unsafe { kernel_file_payload_shadow_mismatches() },
        6 => unsafe { kernel_file_disposition_shadow_mismatches() },
        7 => unsafe { kernel_file_commit_shadow_mismatches() },
        8 => unsafe { kernel_file_namespace_shadow_mismatches() },
        9 => unsafe { kernel_file_view_shadow_mismatches() },
        10 => unsafe { kernel_file_glue_shadow_mismatches() },
        _ => std::process::abort(),
    }
}

unsafe extern "C" fn fileio_counter(field: u32) -> u64 {
    match field {
        0 => unsafe { kernel_file_attempt_count() as u64 },
        1 => unsafe { kernel_file_refused_count() as u64 },
        2 => unsafe { kernel_file_relative_refused_count() as u64 },
        3 => unsafe { kernel_file_fabricated_count() as u64 },
        4 => unsafe { kernel_file_escape_refused_count() as u64 },
        5 => unsafe { kernel_file_disc_opened_count() as u64 },
        6 => unsafe { kernel_file_disc_bytes_read() as u64 },
        7 => unsafe { kernel_file_host_opened_count() as u64 },
        8 => unsafe { kernel_file_host_bytes_read() as u64 },
        9 => unsafe { kernel_file_created_count() as u64 },
        10 => unsafe { kernel_io_bytes_written() as u64 },
        11 => unsafe { kernel_io_write_count() as u64 },
        12 => unsafe { kernel_io_write_refused_count() as u64 },
        _ => std::process::abort(),
    }
}

unsafe extern "C" fn attempt_at(index: c_uint) -> *const c_void {
    unsafe { kernel_file_attempt_at(index) }
}

unsafe extern "C" fn link_target(name: *const c_char) -> *const c_char {
    unsafe { kernel_file_symlink_target(name) }
}

/// Collect and print the same report through the currently selected file-I/O ABI.
///
/// # Safety
/// `out` must be a valid writable C `FILE*`, as required by the existing host API.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn host_fileio_report_attempts(out: *mut FILE, _rust_setup: bool) {
    let mut counts = HostReportCounts::default();
    if !unsafe { tsfp_rfio_host_report_snapshot(&mut counts, Some(fileio_counter)) } {
        std::process::abort();
    }
    unsafe { host_report_file_attempts(out, &counts, attempt_at) };
}

/// Print the selected file-I/O ABI's symbolic-link report.
///
/// # Safety
/// `out` must be a valid writable C `FILE*`, as required by the existing host API.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn host_fileio_report_links(out: *mut FILE) {
    let count = unsafe { kernel_file_symlink_count() };
    unsafe { host_report_symbolic_links(out, count, link_target) };
}
