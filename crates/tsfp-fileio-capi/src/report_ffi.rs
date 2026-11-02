//! Host report aggregation for T3879 FIO-1.
//!
//! TEMPORARY-SHIM: the callback reads the still-C-owned counters. FIO-3 must
//! replace that state before runtime retirement. This is a sequential snapshot,
//! with the same read order as the previous host report; it is not atomic.
use std::ffi::{c_uint, c_ulonglong};

/// Matches `host_report_file_counts` in `src/host/host_report.h`.
#[derive(Debug, Default, PartialEq, Eq)]
#[repr(C)]
pub struct HostReportCounts {
    total: c_uint,
    refused: c_uint,
    relative_refused: c_uint,
    fabricated: c_uint,
    escape_refused: c_uint,
    disc_opened: c_uint,
    disc_bytes_read: c_ulonglong,
    host_opened: c_uint,
    host_bytes_read: c_ulonglong,
    created: c_uint,
    bytes_written: c_ulonglong,
    write_count: c_uint,
    write_refused: c_uint,
}

pub type Counter = unsafe extern "C" fn(u32) -> u64;

/// Write the complete snapshot only after validating the output and callback.
///
/// # Safety
/// A nonnull `out` must point to writable, aligned `host_report_file_counts`
/// storage. `counter`, when present, must accept IDs 0 through 12 without
/// unwinding. The u32 fields originate in C `unsigned` counters; byte counts
/// remain u64 throughout this boundary.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_rfio_host_report_snapshot(
    out: *mut HostReportCounts,
    counter: Option<Counter>,
) -> bool {
    let Some(counter) = counter else { return false };
    if out.is_null() {
        return false;
    }
    // Explicit statements preserve the legacy read order across the callback.
    let total = unsafe { counter(0) } as c_uint;
    let refused = unsafe { counter(1) } as c_uint;
    let relative_refused = unsafe { counter(2) } as c_uint;
    let fabricated = unsafe { counter(3) } as c_uint;
    let escape_refused = unsafe { counter(4) } as c_uint;
    let disc_opened = unsafe { counter(5) } as c_uint;
    let disc_bytes_read = unsafe { counter(6) };
    let host_opened = unsafe { counter(7) } as c_uint;
    let host_bytes_read = unsafe { counter(8) };
    let created = unsafe { counter(9) } as c_uint;
    let bytes_written = unsafe { counter(10) };
    let write_count = unsafe { counter(11) } as c_uint;
    let write_refused = unsafe { counter(12) } as c_uint;
    unsafe {
        out.write(HostReportCounts {
            total,
            refused,
            relative_refused,
            fabricated,
            escape_refused,
            disc_opened,
            disc_bytes_read,
            host_opened,
            host_bytes_read,
            created,
            bytes_written,
            write_count,
            write_refused,
        });
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::mem::{align_of, offset_of, size_of};

    thread_local! {
        static READS: RefCell<Vec<u32>> = const { RefCell::new(Vec::new()) };
    }

    unsafe extern "C" fn sentinel(field: u32) -> u64 {
        READS.with(|reads| reads.borrow_mut().push(field));
        match field {
            6 => 0x1_0000_0006,
            8 => 0x2_0000_0008,
            10 => 0x3_0000_000a,
            _ => 101 + u64::from(field),
        }
    }

    #[test]
    fn report_snapshot_preserves_field_identity_order_and_wide_counts() {
        READS.with(|reads| reads.borrow_mut().clear());
        let mut out = HostReportCounts::default();
        assert!(unsafe { tsfp_rfio_host_report_snapshot(&mut out, Some(sentinel)) });
        assert_eq!(
            out,
            HostReportCounts {
                total: 101,
                refused: 102,
                relative_refused: 103,
                fabricated: 104,
                escape_refused: 105,
                disc_opened: 106,
                disc_bytes_read: 0x1_0000_0006,
                host_opened: 108,
                host_bytes_read: 0x2_0000_0008,
                created: 110,
                bytes_written: 0x3_0000_000a,
                write_count: 112,
                write_refused: 113,
            }
        );
        READS.with(|reads| assert_eq!(*reads.borrow(), (0..13).collect::<Vec<_>>()));
    }

    #[test]
    fn report_snapshot_invalid_arguments_do_not_read_or_overwrite() {
        READS.with(|reads| reads.borrow_mut().clear());
        let mut out = HostReportCounts {
            total: 73,
            ..Default::default()
        };
        assert!(!unsafe { tsfp_rfio_host_report_snapshot(&mut out, None) });
        assert_eq!(out.total, 73);
        assert!(!unsafe { tsfp_rfio_host_report_snapshot(std::ptr::null_mut(), Some(sentinel)) });
        READS.with(|reads| assert!(reads.borrow().is_empty()));
    }

    #[test]
    fn report_snapshot_layout_matches_host_report_c_abi() {
        assert_eq!(size_of::<c_uint>(), 4);
        assert_eq!(size_of::<c_ulonglong>(), 8);
        assert_eq!(offset_of!(HostReportCounts, total), 0);
        assert_eq!(offset_of!(HostReportCounts, refused), 4);
        assert_eq!(offset_of!(HostReportCounts, relative_refused), 8);
        assert_eq!(offset_of!(HostReportCounts, fabricated), 12);
        assert_eq!(offset_of!(HostReportCounts, escape_refused), 16);
        assert_eq!(offset_of!(HostReportCounts, disc_opened), 20);
        assert_eq!(offset_of!(HostReportCounts, disc_bytes_read), 24);
        assert_eq!(offset_of!(HostReportCounts, host_opened), 32);
        // Current execution hosts are 64-bit. Preserve the standard 32-bit C
        // layout too rather than requiring an unnecessary pointer-width ABI.
        let wide_align = align_of::<c_ulonglong>();
        let host_bytes = 36usize.next_multiple_of(wide_align);
        let created = host_bytes + 8;
        let written = (created + 4).next_multiple_of(wide_align);
        assert_eq!(offset_of!(HostReportCounts, host_bytes_read), host_bytes);
        assert_eq!(offset_of!(HostReportCounts, created), created);
        assert_eq!(offset_of!(HostReportCounts, bytes_written), written);
        assert_eq!(offset_of!(HostReportCounts, write_count), written + 8);
        assert_eq!(offset_of!(HostReportCounts, write_refused), written + 12);
        assert_eq!(size_of::<HostReportCounts>(), written + 16);
    }
}
