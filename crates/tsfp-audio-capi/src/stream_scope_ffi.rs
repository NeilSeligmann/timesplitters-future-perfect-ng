//! Rust-owned validation for the measured DirectSound stream creation scope.

use tsfp_audio::adapter::GuestMemory;
use tsfp_audio::scope::{
    FORMAT_BYTES, StreamScope, stream_create_fixed_state_overlap, stream_scope_format_address_ok,
    stream_scope_snapshot, stream_scope_validate,
};
use tsfp_audio::stream_lease::StreamLeaseScope;

use crate::frame_guest_memory::FrameGuestMemory;

const DESCRIPTOR_BYTES: usize = 24;

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct DsoundStreamScope {
    descriptor_address: u32,
    format_address: u32,
    flags: u32,
    max_packets: u32,
    sample_rate: u32,
    average_bytes_per_second: u32,
    channels: u16,
    block_align: u16,
    descriptor: [u8; DESCRIPTOR_BYTES],
    format: [u8; FORMAT_BYTES],
}
const _: () = assert!(std::mem::size_of::<DsoundStreamScope>() == 72);

impl From<StreamScope> for DsoundStreamScope {
    fn from(scope: StreamScope) -> Self {
        Self {
            descriptor_address: scope.descriptor_address,
            format_address: scope.format_address,
            flags: scope.flags,
            max_packets: scope.max_packets,
            sample_rate: scope.sample_rate,
            average_bytes_per_second: scope.average_bytes_per_second,
            channels: scope.channels,
            block_align: scope.block_align,
            descriptor: scope.descriptor,
            format: scope.format,
            ..Self::default()
        }
    }
}

/// Validate immutable creation bytes for the Rust-audio-host path.
///
/// # Safety
/// Non-null pointers must reference `descriptor_bytes`, `format_bytes`, and a writable
/// `DsoundStreamScope` output respectively for the duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_stream_scope_validate_rust(
    descriptor: *const u8,
    descriptor_bytes: usize,
    format: *const u8,
    format_bytes: usize,
    output: *mut DsoundStreamScope,
) -> bool {
    if descriptor.is_null()
        || format.is_null()
        || output.is_null()
        || descriptor_bytes != DESCRIPTOR_BYTES
        || format_bytes != FORMAT_BYTES
    {
        return false;
    }
    // SAFETY: pointer validity and exact bounded lengths are the caller's FFI contract above.
    let descriptor = unsafe { std::slice::from_raw_parts(descriptor, descriptor_bytes) };
    // SAFETY: pointer validity and exact bounded lengths are the caller's FFI contract above.
    let format = unsafe { std::slice::from_raw_parts(format, format_bytes) };
    let Some(scope) = stream_scope_validate(descriptor, format) else {
        return false;
    };
    let mut result = DsoundStreamScope::from(scope);
    result.descriptor.copy_from_slice(descriptor);
    // SAFETY: output is writable for one DsoundStreamScope by the caller's FFI contract.
    unsafe { output.write(result) };
    true
}

/// Check the measured descriptor/format address relationship before guest format access.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_stream_format_address_ok_rust(descriptor: u32, address: u32) -> bool {
    stream_scope_format_address_ok(descriptor, address)
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_stream_create_fixed_overlap_rust(
    address: u32,
    bytes: u32,
    internal: u32,
    include_curve: bool,
) -> bool {
    stream_create_fixed_state_overlap(address, bytes, internal, include_curve)
}

/// Validate the Create output word and the source descriptor/format against the new stream span.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_stream_create_output_overlap_rust(
    output: u32,
    internal: u32,
    descriptor: u32,
    format: u32,
) -> bool {
    fn overlaps(a: u32, a_len: u32, b: u32, b_len: u32) -> bool {
        u64::from(a) < u64::from(b) + u64::from(b_len)
            && u64::from(b) < u64::from(a) + u64::from(a_len)
    }
    overlaps(output, 4, internal, 44)
        || overlaps(output, 4, descriptor, 24)
        || overlaps(output, 4, format, FORMAT_BYTES as u32)
        || overlaps(descriptor, 24, internal, 44)
        || overlaps(format, FORMAT_BYTES as u32, internal, 44)
}

/// Check a freshly allocated private stream object against Create's retained input ranges.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_stream_create_allocation_overlap_rust(
    address: u32,
    output: u32,
    descriptor: u32,
    format: u32,
) -> bool {
    fn overlaps(a: u32, a_len: u32, b: u32, b_len: u32) -> bool {
        u64::from(a) < u64::from(b) + u64::from(b_len)
            && u64::from(b) < u64::from(a) + u64::from(a_len)
    }
    let bytes = 40;
    overlaps(address, bytes, output, 4)
        || overlaps(address, bytes, descriptor, 24)
        || overlaps(address, bytes, format, FORMAT_BYTES as u32)
}

fn snapshot_with_memory<M: GuestMemory + ?Sized>(
    memory: &M,
    descriptor: u32,
) -> Option<DsoundStreamScope> {
    stream_scope_snapshot(memory, descriptor).map(DsoundStreamScope::from)
}

/// Snapshot the immutable Create inputs directly into the Rust-owned lease record.
pub(crate) fn snapshot_lease_scope<M: GuestMemory + ?Sized>(
    memory: &M,
    descriptor: u32,
) -> Option<StreamLeaseScope> {
    let scope = stream_scope_snapshot(memory, descriptor)?;
    Some(StreamLeaseScope {
        descriptor_address: scope.descriptor_address,
        format_address: scope.format_address,
        flags: scope.flags,
        max_packets: scope.max_packets,
        sample_rate: scope.sample_rate,
        average_bytes_per_second: scope.average_bytes_per_second,
        channels: scope.channels,
        block_align: scope.block_align,
        descriptor: scope.descriptor,
        format: scope.format,
    })
}

/// Read and validate the ordered creation snapshot through the shared Rust guest-memory service.
///
/// # Safety
/// `output` must point to writable storage for one 72-byte `DsoundStreamScope`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_stream_scope_snapshot_rust(
    host_pid: i32,
    descriptor: u32,
    output: *mut DsoundStreamScope,
) -> bool {
    if output.is_null() {
        return false;
    }
    let Some(scope) = snapshot_with_memory(&FrameGuestMemory::new(host_pid), descriptor) else {
        return false;
    };
    // SAFETY: output validity is the caller's FFI contract.
    unsafe { output.write(scope) };
    true
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use tsfp_audio::adapter::GuestMemory;

    use super::*;

    struct Memory {
        descriptor: [u8; 24],
        format: [u8; 20],
        reads: RefCell<Vec<(u32, usize)>>,
    }

    impl GuestMemory for Memory {
        fn read_u32(&self, _: u32) -> Option<u32> {
            None
        }
        fn write_u32(&mut self, _: u32, _: u32) -> bool {
            false
        }
        fn read_bytes(&self, address: u32, len: usize) -> Option<Vec<u8>> {
            self.reads.borrow_mut().push((address, len));
            match (address, len) {
                (0x1000, 24) => Some(self.descriptor.to_vec()),
                (0x2000, 20) => Some(self.format.to_vec()),
                _ => None,
            }
        }
        fn range_readable(&self, _: u32, _: u32) -> bool {
            false
        }
    }

    fn fixture(flags: u32) -> ([u8; 24], [u8; 20]) {
        let mut descriptor = [0; 24];
        descriptor[0..4].copy_from_slice(&flags.to_le_bytes());
        descriptor[4..8].copy_from_slice(&3u32.to_le_bytes());
        descriptor[8..12].copy_from_slice(&0x2000u32.to_le_bytes());
        let channels = if flags == 0 { 2u16 } else { 1 };
        let block = 36 * u32::from(channels);
        let mut format = [0; 20];
        format[0..2].copy_from_slice(&0x69u16.to_le_bytes());
        format[2..4].copy_from_slice(&channels.to_le_bytes());
        format[4..8].copy_from_slice(&44100u32.to_le_bytes());
        format[8..12].copy_from_slice(&((block * 44100) >> 6).to_le_bytes());
        format[12..14].copy_from_slice(&(block as u16).to_le_bytes());
        format[14..16].copy_from_slice(&4u16.to_le_bytes());
        format[16..18].copy_from_slice(&2u16.to_le_bytes());
        format[18..20].copy_from_slice(&64u16.to_le_bytes());
        (descriptor, format)
    }

    #[test]
    fn validates_measured_stereo_and_spatial_scopes_and_preserves_snapshots() {
        for flags in [0, 0x10] {
            let (descriptor, format) = fixture(flags);
            let mut output = DsoundStreamScope::default();
            // SAFETY: pointers refer to live stack objects with exact declared lengths.
            assert!(unsafe {
                tsfp_audio_stream_scope_validate_rust(
                    descriptor.as_ptr(),
                    descriptor.len(),
                    format.as_ptr(),
                    format.len(),
                    &mut output,
                )
            });
            assert_eq!(output.flags, flags);
            assert_eq!(output.descriptor, descriptor);
            assert_eq!(output.format, format);
            assert_eq!(output.channels, if flags == 0 { 2 } else { 1 });
        }
    }

    #[test]
    fn refuses_unmeasured_scope_and_wrong_snapshot_lengths() {
        let (mut descriptor, format) = fixture(0);
        descriptor[0..4].copy_from_slice(&0x20u32.to_le_bytes());
        let mut output = DsoundStreamScope::default();
        // SAFETY: pointers are valid; the content is outside the measured scope.
        assert!(!unsafe {
            tsfp_audio_stream_scope_validate_rust(
                descriptor.as_ptr(),
                descriptor.len(),
                format.as_ptr(),
                format.len(),
                &mut output,
            )
        });
        let (descriptor, format) = fixture(0);
        // SAFETY: the declared short descriptor is rejected before it is read.
        assert!(!unsafe {
            tsfp_audio_stream_scope_validate_rust(
                descriptor.as_ptr(),
                descriptor.len() - 1,
                format.as_ptr(),
                format.len(),
                &mut output,
            )
        });
    }

    #[test]
    fn stream_address_guard_accepts_adjacent_and_rejects_overlap_or_wrap() {
        assert!(tsfp_audio_stream_format_address_ok_rust(0x1000, 0x1018));
        assert!(tsfp_audio_stream_format_address_ok_rust(0x1000, 0x0fec));
        assert!(!tsfp_audio_stream_format_address_ok_rust(0x1000, 0x1017));
        assert!(!tsfp_audio_stream_format_address_ok_rust(0x1000, 0));
        assert!(!tsfp_audio_stream_format_address_ok_rust(
            0xffff_fff0,
            0xffff_fff8
        ));
    }

    #[test]
    fn create_fixed_state_overlap_matches_guest_ranges_and_curve_policy() {
        assert!(tsfp_audio_stream_create_fixed_overlap_rust(
            0x1000, 1, 0x1000, true
        ));
        assert!(tsfp_audio_stream_create_fixed_overlap_rust(
            0x00412B30, 1, 0, false
        ));
        assert!(tsfp_audio_stream_create_fixed_overlap_rust(
            0x004A1CF0, 60, 0, false
        ));
        assert!(tsfp_audio_stream_create_fixed_overlap_rust(
            0x004124A8, 4, 0, false
        ));
        assert!(!tsfp_audio_stream_create_fixed_overlap_rust(
            0x004B914C, 4, 0, false
        ));
        assert!(tsfp_audio_stream_create_fixed_overlap_rust(
            0x004B914C, 4, 0, true
        ));
        assert!(!tsfp_audio_stream_create_fixed_overlap_rust(
            0x004A1CEC, 4, 0, false
        ));
        assert!(!tsfp_audio_stream_create_fixed_overlap_rust(
            0xFFFF_FFFF,
            2,
            0,
            false
        ));
    }

    #[test]
    fn create_output_guard_rejects_intersections_and_accepts_adjacent_spans() {
        assert!(tsfp_audio_stream_create_output_overlap_rust(
            0x1000, 0x1000, 0x2000, 0x3000
        ));
        assert!(tsfp_audio_stream_create_output_overlap_rust(
            0x1000, 0x2000, 0x1000, 0x3000
        ));
        assert!(tsfp_audio_stream_create_output_overlap_rust(
            0x1000, 0x2000, 0x3000, 0x0ffc
        ));
        assert!(!tsfp_audio_stream_create_output_overlap_rust(
            0x1000, 0x2000, 0x3000, 0x4000
        ));
        assert!(tsfp_audio_stream_create_output_overlap_rust(
            0xffff_fffe,
            0xffff_fffe,
            0x1000,
            0x2000
        ));
    }

    #[test]
    fn create_allocation_guard_checks_guest_inputs_and_fixed_state() {
        assert!(tsfp_audio_stream_create_allocation_overlap_rust(
            0x1000, 0x1010, 0x2000, 0x3000
        ));
        assert!(tsfp_audio_stream_create_allocation_overlap_rust(
            0x1000, 0x3000, 0x1010, 0x4000
        ));
        assert!(!tsfp_audio_stream_create_allocation_overlap_rust(
            0x1000, 0x2000, 0x3000, 0x4000
        ));
        assert!(tsfp_audio_stream_create_allocation_overlap_rust(
            0xffff_fffe,
            0xffff_fffc,
            0,
            0
        ));
    }

    #[test]
    fn snapshot_reads_descriptor_then_format_once_and_returns_both_exact_snapshots() {
        let (mut descriptor, format) = fixture(0);
        descriptor[8..12].copy_from_slice(&0x2000u32.to_le_bytes());
        let memory = Memory {
            descriptor,
            format,
            reads: RefCell::new(Vec::new()),
        };
        let result = snapshot_with_memory(&memory, 0x1000).unwrap();
        assert_eq!(&*memory.reads.borrow(), &[(0x1000, 24), (0x2000, 20)]);
        assert_eq!(result.descriptor_address, 0x1000);
        assert_eq!(result.descriptor, descriptor);
        assert_eq!(result.format, memory.format);
    }

    #[test]
    fn lease_scope_snapshot_preserves_the_ordered_rust_guest_snapshot() {
        let (mut descriptor, format) = fixture(0);
        descriptor[8..12].copy_from_slice(&0x2000u32.to_le_bytes());
        let memory = Memory {
            descriptor,
            format,
            reads: RefCell::new(Vec::new()),
        };
        let result = snapshot_lease_scope(&memory, 0x1000).unwrap();
        assert_eq!(&*memory.reads.borrow(), &[(0x1000, 24), (0x2000, 20)]);
        assert_eq!(result.descriptor_address, 0x1000);
        assert_eq!(result.format_address, 0x2000);
        assert_eq!(result.descriptor, descriptor);
        assert_eq!(result.format, memory.format);
        assert_eq!(result.sample_rate, 44_100);
        assert_eq!(result.channels, 2);
    }

    #[test]
    fn snapshot_refuses_overlapping_format_before_a_second_guest_read() {
        let (mut descriptor, format) = fixture(0);
        descriptor[8..12].copy_from_slice(&0x1017u32.to_le_bytes());
        let memory = Memory {
            descriptor,
            format,
            reads: RefCell::new(Vec::new()),
        };
        assert!(snapshot_with_memory(&memory, 0x1000).is_none());
        assert_eq!(&*memory.reads.borrow(), &[(0x1000, 24)]);
    }
}
