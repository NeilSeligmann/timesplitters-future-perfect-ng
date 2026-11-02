//! Fault-safe Rust snapshot and validation for the bounded DirectSound stream format family.

use tsfp_audio::adapter::GuestMemory;

use crate::frame_guest_memory::FrameGuestMemory;

pub const STREAM_FORMAT_BYTES: usize = 20;

/// Classify the measured SetFormat startup state before the C adapter reads guest format bytes.
/// Returns 1 for stereo, 2 for configured spatial mono, 0 for an unmeasured shape, and 3 when
/// the measured shape has not reached its required startup state.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_stream_format_admit_rust(
    scope_flags: u32,
    cache_mask: u32,
    volume_seen: u32,
    discontinuity_seen: u32,
) -> u32 {
    let shape = match (scope_flags, cache_mask) {
        (0, 0) => 1,
        (0x10, 7) => 2,
        _ => return 0,
    };
    if volume_seen != 0 && discontinuity_seen != 0 {
        shape
    } else {
        3
    }
}

fn decode_format(bytes: &[u8], channels: u32) -> Option<([u8; STREAM_FORMAT_BYTES], u32)> {
    let words: [u8; STREAM_FORMAT_BYTES] = bytes.try_into().ok()?;
    if channels != 1 && channels != 2 {
        return None;
    }
    let read_u32 = |offset: usize| -> Option<u32> {
        Some(u32::from_le_bytes(
            words.get(offset..offset + 4)?.try_into().ok()?,
        ))
    };
    let rate = read_u32(4)?;
    let average = read_u32(8)?;
    let block = 36 * channels;
    if words[0] != 0x69
        || words[1] != 0
        || u32::from(words[2]) != channels
        || words[3] != 0
        || !(8_000..=48_000).contains(&rate)
        || average != (u64::from(rate) * u64::from(block) / 64) as u32
        || u32::from(words[12]) != block
        || words[13] != 0
        || words[14] != 4
        || words[15] != 0
        || words[16] != 2
        || words[17] != 0
        || words[18] != 64
        || words[19] != 0
    {
        return None;
    }
    Some((words, rate))
}

/// Read and validate one 20-byte guest format record. The C adapter retains stream-lease and
/// alias checks before calling this function; guest bytes are copied through process_vm_readv and
/// become visible to C only after the complete measured XADPCM-family check succeeds.
///
/// # Safety
/// `out_format` and `out_sample_rate` must be writable for 20 and 4 bytes respectively and must
/// not overlap. `format_address` is a guest address in `host_pid`'s address space.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_stream_format_decode_rust(
    host_pid: i32,
    format_address: u32,
    channels: u32,
    out_format: *mut u8,
    out_sample_rate: *mut u32,
) -> bool {
    if out_format.is_null() || out_sample_rate.is_null() {
        return false;
    }
    let Some(bytes) =
        FrameGuestMemory::new(host_pid).read_bytes(format_address, STREAM_FORMAT_BYTES)
    else {
        return false;
    };
    let Some((format, rate)) = decode_format(&bytes, channels) else {
        return false;
    };
    // SAFETY: the caller provides the two non-overlapping writable outputs documented above.
    unsafe {
        std::ptr::copy_nonoverlapping(format.as_ptr(), out_format, STREAM_FORMAT_BYTES);
        out_sample_rate.write(rate);
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn format(channels: u8, rate: u32) -> [u8; STREAM_FORMAT_BYTES] {
        let block = 36 * u32::from(channels);
        let mut bytes = [0; STREAM_FORMAT_BYTES];
        bytes[0] = 0x69;
        bytes[2] = channels;
        bytes[4..8].copy_from_slice(&rate.to_le_bytes());
        let average = (u64::from(rate) * u64::from(block) / 64) as u32;
        bytes[8..12].copy_from_slice(&average.to_le_bytes());
        bytes[12] = block as u8;
        bytes[14] = 4;
        bytes[16] = 2;
        bytes[18] = 64;
        bytes
    }

    #[test]
    fn format_admission_accepts_only_started_measured_stream_shapes() {
        assert_eq!(tsfp_audio_stream_format_admit_rust(0, 0, 1, 1), 1);
        assert_eq!(tsfp_audio_stream_format_admit_rust(0x10, 7, 1, 1), 2);
        assert_eq!(tsfp_audio_stream_format_admit_rust(0x10, 3, 1, 1), 0);
        assert_eq!(tsfp_audio_stream_format_admit_rust(0x20, 7, 1, 1), 0);
        assert_eq!(tsfp_audio_stream_format_admit_rust(0, 0, 0, 1), 3);
        assert_eq!(tsfp_audio_stream_format_admit_rust(0, 0, 1, 0), 3);
    }

    #[test]
    fn decodes_measured_stereo_and_mono_adpcm_records() {
        for channels in [1, 2] {
            let source = format(channels, 44_100);
            let (actual, rate) = decode_format(&source, u32::from(channels)).unwrap();
            assert_eq!(actual, source);
            assert_eq!(rate, 44_100);
        }
    }

    #[test]
    fn rejects_out_of_policy_format_fields_without_partial_output() {
        let valid = format(2, 44_100);
        for (offset, value) in [
            (0, 0x68),
            (2, 1),
            (4, 0x40), // low byte changes rate while preserving a readable record
            (12, 35),
            (14, 3),
            (16, 1),
            (18, 63),
        ] {
            let mut invalid = valid;
            invalid[offset] = value;
            assert!(decode_format(&invalid, 2).is_none(), "field at {offset}");
        }
        assert!(decode_format(&valid, 0).is_none());
        assert!(decode_format(&valid, 3).is_none());
    }

    #[test]
    fn guest_ffi_publishes_only_a_complete_valid_snapshot() {
        let (page, address) = crate::test_guest_memory::map_guest_page();
        let valid = format(2, 44_100);
        // SAFETY: this test owns a mapped page and writes exactly one format record.
        unsafe { std::ptr::copy_nonoverlapping(valid.as_ptr(), page, valid.len()) };
        let mut output = [0xA5; STREAM_FORMAT_BYTES];
        let mut rate = 0xDEAD_BEEFu32;
        assert!(unsafe {
            tsfp_audio_stream_format_decode_rust(
                std::process::id() as i32,
                address,
                2,
                output.as_mut_ptr(),
                &mut rate,
            )
        });
        assert_eq!(output, valid);
        assert_eq!(rate, 44_100);

        unsafe { page.add(14).write(3) };
        output.fill(0xA5);
        rate = 0xDEAD_BEEF;
        assert!(!unsafe {
            tsfp_audio_stream_format_decode_rust(
                std::process::id() as i32,
                address,
                2,
                output.as_mut_ptr(),
                &mut rate,
            )
        });
        assert_eq!(output, [0xA5; STREAM_FORMAT_BYTES]);
        assert_eq!(rate, 0xDEAD_BEEF);

        assert!(!unsafe {
            tsfp_audio_stream_format_decode_rust(
                std::process::id() as i32,
                address.wrapping_add(4096),
                2,
                output.as_mut_ptr(),
                &mut rate,
            )
        });
        crate::test_guest_memory::unmap_guest_page(page);
    }
}
