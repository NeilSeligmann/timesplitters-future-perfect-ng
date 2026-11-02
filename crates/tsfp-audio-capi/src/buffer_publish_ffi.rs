//! Rust-host DirectSound buffer guest header construction and pointer publication.

use crate::effects_guest_copy_ffi::{
    tsfp_effects_guest_copy_read_rust, tsfp_effects_guest_copy_write_rust,
};

const BUFFER_HEADER_BYTES: usize = 36;
const BUFFER_POINTER_OFFSET: u32 = 28;

/// Check that the output word is writable while preserving its current contents.
///
/// # Safety
/// The guest output address must belong to the process identified by `pid`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_buffer_output_probe_rust(pid: i32, output: u32) -> bool {
    if pid <= 0 || output == 0 || output.checked_add(4).is_none() {
        return false;
    }
    let mut value = 0u32;
    // SAFETY: the checked address is copied through the fault-safe process_vm boundary.
    unsafe {
        tsfp_effects_guest_copy_read_rust(pid, output, (&mut value as *mut u32).cast(), 4)
            && tsfp_effects_guest_copy_write_rust(pid, (&value as *const u32).cast(), output, 4)
    }
}

/// Build and write the 36-byte passive DirectSound buffer header from typed words.
///
/// # Safety
/// `header` must be a live guest allocation in the process identified by `pid`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_buffer_header_write_rust(
    pid: i32,
    header: u32,
    vtable: u32,
    references: u32,
    parent_internal: u32,
) -> bool {
    if pid <= 0
        || header == 0
        || u64::from(header) + BUFFER_HEADER_BYTES as u64 > 1u64 << 32
        || parent_internal == 0
    {
        return false;
    }
    let mut bytes = [0u8; BUFFER_HEADER_BYTES];
    bytes[0..4].copy_from_slice(&vtable.to_le_bytes());
    bytes[4..8].copy_from_slice(&references.to_le_bytes());
    bytes[8..12].copy_from_slice(&parent_internal.to_le_bytes());
    // SAFETY: source is a local fixed-size array and guest span was checked above.
    unsafe { tsfp_effects_guest_copy_write_rust(pid, bytes.as_ptr(), header, bytes.len()) }
}

/// Publish the guest buffer interface pointer after its header has been written.
///
/// # Safety
/// The output address must belong to the process identified by `pid`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_buffer_pointer_publish_rust(
    pid: i32,
    output: u32,
    header: u32,
) -> bool {
    let Some(buffer) = header.checked_add(BUFFER_POINTER_OFFSET) else {
        return false;
    };
    if pid <= 0 || output == 0 || output.checked_add(4).is_none() {
        return false;
    }
    // SAFETY: local value and guest address are checked for this 4-byte copy.
    unsafe { tsfp_effects_guest_copy_write_rust(pid, (&buffer as *const u32).cast(), output, 4) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_typed_buffer_header_then_publishes_pointer() {
        let (page, address) = crate::test_guest_memory::map_guest_page();
        // SAFETY: the test owns a readable/writable low-address page until unmap.
        unsafe {
            page.cast::<u32>().write(0x1234_5678);
            assert!(tsfp_audio_buffer_output_probe_rust(
                std::process::id() as i32,
                address + 128
            ));
            assert!(tsfp_audio_buffer_header_write_rust(
                std::process::id() as i32,
                address + 256,
                0x004a_1ce0,
                1,
                0x0050_0000
            ));
            let words = std::slice::from_raw_parts((page.add(256)).cast::<u32>(), 9);
            assert_eq!(&words[..3], &[0x004a_1ce0, 1, 0x0050_0000]);
            assert!(words[3..].iter().all(|word| *word == 0));
            assert!(tsfp_audio_buffer_pointer_publish_rust(
                std::process::id() as i32,
                address + 128,
                address + 256
            ));
            assert_eq!(page.add(128).cast::<u32>().read(), address + 284);
        }
        crate::test_guest_memory::unmap_guest_page(page);
    }

    #[test]
    fn refuses_invalid_guest_spans_without_publishing() {
        let (page, address) = crate::test_guest_memory::map_guest_page();
        // SAFETY: the test owns the output word and verifies it remains unchanged.
        unsafe {
            page.cast::<u32>().write(0xfeed_beef);
            assert!(!tsfp_audio_buffer_header_write_rust(
                std::process::id() as i32,
                u32::MAX - 8,
                1,
                1,
                2
            ));
            assert!(!tsfp_audio_buffer_pointer_publish_rust(
                std::process::id() as i32,
                0,
                address
            ));
            assert_eq!(page.cast::<u32>().read(), 0xfeed_beef);
        }
        crate::test_guest_memory::unmap_guest_page(page);
    }
}
