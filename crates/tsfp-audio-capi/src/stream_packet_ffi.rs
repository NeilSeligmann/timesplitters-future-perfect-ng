//! Rust-host snapshot and decode of the DirectSound stream packet descriptor.
use crate::effects_guest_copy_ffi::tsfp_effects_guest_copy_read_rust;

const PACKET_WORDS: usize = 6;
const PACKET_BYTES: usize = PACKET_WORDS * size_of::<u32>();
const PROBE_CHUNK_BYTES: usize = 4096;
const MAX_MIX_PACKET_BYTES: u32 = 4 * 1024 * 1024;

/// Rust-owned, source-preserving admission for the two measured Xbox ADPCM
/// packet layouts accepted by the optional mixer.
///
/// `kind_out` is 0 for a packet that remains completion-only, 1 for stereo,
/// and 2 for mono. This classifies data only; C retains packet lifecycle and
/// completion policy on the current host adapter.
///
/// # Safety
/// `format` points to `format_bytes` readable bytes; each non-null output points
/// to one writable `u32`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_stream_packet_classify_rust(
    format: *const u8,
    format_bytes: u32,
    packet_bytes: u32,
    kind_out: *mut u32,
    rate_out: *mut u32,
) -> bool {
    if format.is_null() || format_bytes < 20 || kind_out.is_null() || rate_out.is_null() {
        return false;
    }
    // SAFETY: the caller supplies at least the first 20 readable format bytes.
    let bytes = unsafe { std::slice::from_raw_parts(format, 20) };
    let le16 = |offset: usize| u16::from_le_bytes([bytes[offset], bytes[offset + 1]]);
    let rate = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
    let block_bytes = le16(12);
    let channels = le16(2);
    let kind = if le16(0) == 0x69
        && le16(14) == 4
        && le16(16) == 2
        && le16(18) == 64
        && rate != 0
        && packet_bytes != 0
        && packet_bytes <= MAX_MIX_PACKET_BYTES
        && ((channels == 2 && block_bytes == 72 && packet_bytes % 72 == 0)
            || (channels == 1 && block_bytes == 36 && packet_bytes % 36 == 0))
    {
        if channels == 1 { 2 } else { 1 }
    } else {
        0
    };
    // SAFETY: both output pointers were checked above and the caller guarantees
    // each points to one writable u32.
    unsafe {
        kind_out.write(kind);
        rate_out.write(if kind == 0 { 0 } else { rate });
    }
    true
}

/// Read the six little-endian packet fields into a caller-owned array.
///
/// The C adapter retains packet content checks and queue handling. This boundary only moves the
/// guest descriptor read/decode to Rust and publishes no partial result on refusal.
///
/// # Safety
/// `words_out` must point to six writable `u32`s. The guest span must belong to `host_pid`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_stream_packet_decode_rust(
    host_pid: i32,
    packet_address: u32,
    words_out: *mut u32,
) -> bool {
    unsafe { decode_guest_packet(host_pid, packet_address, words_out) }
}

/// Read the six-word XMV packet descriptor through the Rust guest-memory boundary.
///
/// The movie facade still owns packet admission, queue policy, and callbacks; this only moves
/// the fault-safe descriptor snapshot out of C on the Rust-host build.
///
/// # Safety
/// `words_out` must point to six writable `u32`s. The guest span must belong to `host_pid`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_movie_packet_decode_rust(
    host_pid: i32,
    packet_address: u32,
    words_out: *mut u32,
) -> bool {
    unsafe { decode_guest_packet(host_pid, packet_address, words_out) }
}

unsafe fn decode_guest_packet(host_pid: i32, packet_address: u32, words_out: *mut u32) -> bool {
    if host_pid <= 0
        || words_out.is_null()
        || packet_address == 0
        || u64::from(packet_address) + PACKET_BYTES as u64 > (1u64 << 32)
    {
        return false;
    }

    let mut bytes = [0; PACKET_BYTES];
    // SAFETY: the guest-copy ABI checks both spans and uses fault-safe process_vm_readv.
    if !unsafe {
        tsfp_effects_guest_copy_read_rust(host_pid, packet_address, bytes.as_mut_ptr(), bytes.len())
    } {
        return false;
    }

    for (index, chunk) in bytes.chunks_exact(4).enumerate() {
        let word = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        // SAFETY: the caller contract guarantees a writable six-word output array.
        unsafe { words_out.add(index).write(word) };
    }
    true
}

/// Check the complete packet payload span before the adapter publishes completion words.
///
/// Reads bounded 4 KiB chunks through the fault-safe guest-copy ABI. This preserves the
/// C adapter's pre-publication readability check while keeping guest address translation in Rust.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_stream_packet_buffer_probe_rust(
    host_pid: i32,
    buffer_address: u32,
    buffer_bytes: u32,
) -> bool {
    if host_pid <= 0
        || buffer_address == 0
        || buffer_bytes == 0
        || u64::from(buffer_address) + u64::from(buffer_bytes) > (1u64 << 32)
    {
        return false;
    }

    let mut scratch = [0; PROBE_CHUNK_BYTES];
    let mut offset = 0u32;
    while offset < buffer_bytes {
        let chunk_bytes = (buffer_bytes - offset).min(PROBE_CHUNK_BYTES as u32) as usize;
        let chunk_address = buffer_address + offset;
        // SAFETY: the helper validates the remote span and uses fault-safe process_vm_readv.
        if !unsafe {
            tsfp_effects_guest_copy_read_rust(
                host_pid,
                chunk_address,
                scratch.as_mut_ptr(),
                chunk_bytes,
            )
        } {
            return false;
        }
        offset += chunk_bytes as u32;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn adpcm_format(channels: u16, block_bytes: u16, rate: u32) -> [u8; 20] {
        let mut format = [0; 20];
        format[0..2].copy_from_slice(&0x69u16.to_le_bytes());
        format[2..4].copy_from_slice(&channels.to_le_bytes());
        format[4..8].copy_from_slice(&rate.to_le_bytes());
        format[12..14].copy_from_slice(&block_bytes.to_le_bytes());
        format[14..16].copy_from_slice(&4u16.to_le_bytes());
        format[16..18].copy_from_slice(&2u16.to_le_bytes());
        format[18..20].copy_from_slice(&64u16.to_le_bytes());
        format
    }

    #[test]
    fn classifies_measured_mono_and_stereo_adpcm_packets() {
        for (channels, block, bytes, expected_kind) in [(1, 36, 72, 2), (2, 72, 144, 1)] {
            let format = adpcm_format(channels, block, 22_050);
            let (mut kind, mut rate) = (99, 99);
            // SAFETY: all pointers address live local values for this call.
            assert!(unsafe {
                tsfp_audio_stream_packet_classify_rust(
                    format.as_ptr(),
                    format.len() as u32,
                    bytes,
                    &mut kind,
                    &mut rate,
                )
            });
            assert_eq!((kind, rate), (expected_kind, 22_050));
        }
    }

    #[test]
    fn unsupported_or_misaligned_packets_remain_completion_only() {
        let cases = [
            (adpcm_format(1, 36, 22_050), 71),
            (adpcm_format(2, 72, 22_050), 4 * 1024 * 1024 + 1),
            (adpcm_format(1, 36, 0), 72),
            (
                {
                    let mut f = adpcm_format(1, 36, 22_050);
                    f[0..2].copy_from_slice(&1u16.to_le_bytes());
                    f
                },
                72,
            ),
        ];
        for (format, bytes) in cases {
            let (mut kind, mut rate) = (99, 99);
            // SAFETY: all pointers address live local values for this call.
            assert!(unsafe {
                tsfp_audio_stream_packet_classify_rust(
                    format.as_ptr(),
                    format.len() as u32,
                    bytes,
                    &mut kind,
                    &mut rate,
                )
            });
            assert_eq!((kind, rate), (0, 0));
        }
    }

    #[test]
    fn decodes_six_little_endian_words_from_guest_memory() {
        let (page, base) = crate::test_guest_memory::map_guest_page();
        let packet = base + 0x100;
        let expected = [
            0x1020_3040,
            0x5060_7080,
            0x90a0_b0c0,
            0xd0e0_f001,
            0x2345_6789,
            0xabcd_ef01,
        ];
        // SAFETY: the test owns a writable guest page containing the packet descriptor.
        for (index, word) in expected.iter().enumerate() {
            unsafe { ((packet as usize as *mut u32).add(index)).write(*word) };
        }

        let mut actual = [0; PACKET_WORDS];
        // SAFETY: the guest page and output array remain live for the call.
        assert!(unsafe {
            tsfp_audio_stream_packet_decode_rust(
                std::process::id() as i32,
                packet,
                actual.as_mut_ptr(),
            )
        });
        assert_eq!(actual, expected);
        crate::test_guest_memory::unmap_guest_page(page);
    }

    #[test]
    fn refusal_does_not_publish_partial_words() {
        let mut actual = [0xfeed_face; PACKET_WORDS];
        // SAFETY: actual is a writable six-word output; address overflow rejects before reading.
        assert!(!unsafe {
            tsfp_audio_stream_packet_decode_rust(
                std::process::id() as i32,
                u32::MAX - 8,
                actual.as_mut_ptr(),
            )
        });
        assert_eq!(actual, [0xfeed_face; PACKET_WORDS]);
    }

    #[test]
    fn movie_descriptor_uses_the_same_fault_safe_six_word_snapshot() {
        let (page, base) = crate::test_guest_memory::map_guest_page();
        let packet = base + 0x100;
        let expected = [0x1000, 0x2000, 0x3001, 0x4001, 0x5000, 0xfeed_face];
        // SAFETY: the test owns a writable guest page containing the descriptor.
        for (index, word) in expected.iter().enumerate() {
            unsafe { ((packet as usize as *mut u32).add(index)).write(*word) };
        }
        let mut actual = [0; PACKET_WORDS];
        // SAFETY: the guest page and output array remain live for the call.
        assert!(unsafe {
            tsfp_audio_movie_packet_decode_rust(
                std::process::id() as i32,
                packet,
                actual.as_mut_ptr(),
            )
        });
        assert_eq!(actual, expected);
        crate::test_guest_memory::unmap_guest_page(page);
    }

    #[test]
    fn packet_payload_probe_checks_the_entire_span_without_faulting() {
        let (page, base) = crate::test_guest_memory::map_guest_page();
        assert!(tsfp_audio_stream_packet_buffer_probe_rust(
            std::process::id() as i32,
            base + 0x100,
            64,
        ));
        // The first eight bytes are mapped; the suffix crosses into an unmapped page.
        assert!(!tsfp_audio_stream_packet_buffer_probe_rust(
            std::process::id() as i32,
            base + 0x0ff8,
            16,
        ));
        assert!(!tsfp_audio_stream_packet_buffer_probe_rust(
            std::process::id() as i32,
            u32::MAX - 7,
            16,
        ));
        crate::test_guest_memory::unmap_guest_page(page);
    }
}
