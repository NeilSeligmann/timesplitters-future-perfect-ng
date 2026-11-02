use std::ffi::c_void;
use std::ptr;
use tsfp_audio_capi::effects_publish_ffi::tsfp_effects_binding_publish_rust;

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod linux_tests {
    use super::*;

    const PROT_NONE: i32 = 0;
    const PROT_READ: i32 = 1;
    const PROT_WRITE: i32 = 2;
    const MAP_PRIVATE: i32 = 2;
    const MAP_ANONYMOUS: i32 = 0x20;
    const MAP_32BIT: i32 = 0x40;
    const WORKSPACE_BYTES: usize = 393_216;
    const STATE_BYTES: usize = 8088;

    unsafe extern "C" {
        fn mmap(
            address: *mut c_void,
            length: usize,
            protection: i32,
            flags: i32,
            fd: i32,
            offset: i64,
        ) -> *mut c_void;
        fn munmap(address: *mut c_void, length: usize) -> i32;
    }

    unsafe fn low_mapping(length: usize, protection: i32) -> *mut c_void {
        // SAFETY: caller owns the returned mapping and releases it with munmap.
        let mapping = unsafe {
            mmap(
                ptr::null_mut(),
                length,
                protection,
                MAP_PRIVATE | MAP_ANONYMOUS | MAP_32BIT,
                -1,
                0,
            )
        };
        assert_ne!(mapping, (-1isize) as *mut c_void);
        let base = mapping as usize;
        assert!(base != 0 && base + length <= (1usize << 32));
        mapping
    }

    fn views(base: u32) -> [u32; 6] {
        [
            base,
            base + 296,
            base + 296 + 8080,
            base + 296 + 8080 + STATE_BYTES as u32,
            base + 296 + 8080 + STATE_BYTES as u32 + WORKSPACE_BYTES as u32,
            base + 296 + 8080 + STATE_BYTES as u32 + WORKSPACE_BYTES as u32 + 8,
        ]
    }

    #[test]
    fn publication_writes_guest_views_and_commits_the_output_last() {
        let length = 0x80000usize;
        let mapping = unsafe { low_mapping(length, PROT_READ | PROT_WRITE) };
        // SAFETY: mapping is writable for the requested range.
        unsafe { ptr::write_bytes(mapping.cast::<u8>(), 0xA5, length) };
        let addresses = views(mapping as usize as u32);
        let output = addresses[5] + STATE_BYTES as u32;
        let descriptor = [0x11u8; 296];
        let code = [0x22u8; 8080];
        let state = [0x33u8; STATE_BYTES];
        let accepted = unsafe {
            tsfp_effects_binding_publish_rust(
                std::process::id() as i32,
                descriptor.as_ptr(),
                code.as_ptr(),
                state.as_ptr(),
                addresses.as_ptr(),
                output,
            )
        };
        assert!(accepted);

        // SAFETY: all inspected ranges are in the live writable mapping.
        unsafe {
            assert_eq!(
                std::slice::from_raw_parts(addresses[0] as usize as *const u8, descriptor.len()),
                descriptor.as_slice()
            );
            assert_eq!(
                std::slice::from_raw_parts(addresses[1] as usize as *const u8, code.len()),
                code.as_slice()
            );
            assert_eq!(
                std::slice::from_raw_parts(addresses[2] as usize as *const u8, state.len()),
                state.as_slice()
            );
            assert!(
                std::slice::from_raw_parts(addresses[3] as usize as *const u8, WORKSPACE_BYTES)
                    .iter()
                    .all(|byte| *byte == 0)
            );
            assert_eq!(
                std::slice::from_raw_parts(addresses[4] as usize as *const u8, 4),
                &[0; 4]
            );
            assert_eq!(
                std::slice::from_raw_parts(addresses[5] as usize as *const u8, state.len()),
                state.as_slice()
            );
            assert_eq!(ptr::read(output as usize as *const u32), addresses[0]);
            assert_eq!(munmap(mapping, length), 0);
        }
    }

    #[test]
    fn invalid_address_refuses_before_mutating_destinations() {
        let length = 0x80000usize;
        let mapping = unsafe { low_mapping(length, PROT_READ | PROT_WRITE) };
        // SAFETY: mapping is writable for the requested range.
        unsafe { ptr::write_bytes(mapping.cast::<u8>(), 0xA5, length) };
        let mut addresses = views(mapping as usize as u32);
        let output = addresses[5] + STATE_BYTES as u32;
        addresses[0] = 0;
        let accepted = unsafe {
            tsfp_effects_binding_publish_rust(
                std::process::id() as i32,
                [0x11; 296].as_ptr(),
                [0x22; 8080].as_ptr(),
                [0x33; STATE_BYTES].as_ptr(),
                addresses.as_ptr(),
                output,
            )
        };
        assert!(!accepted);
        // SAFETY: mapping remains live and writable.
        unsafe {
            assert!(
                std::slice::from_raw_parts(mapping.cast::<u8>(), length)
                    .iter()
                    .all(|byte| *byte == 0xA5)
            );
            assert_eq!(munmap(mapping, length), 0);
        }
    }

    #[test]
    fn inaccessible_guest_view_refuses_without_publishing_title_pointer() {
        let length = 0x80000usize;
        let mapping = unsafe { low_mapping(length, PROT_READ | PROT_WRITE) };
        // SAFETY: mapping is writable for the requested range.
        unsafe { ptr::write_bytes(mapping.cast::<u8>(), 0xA5, length) };
        let protected = unsafe { low_mapping(8192, PROT_NONE) };
        let mut addresses = views(mapping as usize as u32);
        addresses[1] = protected as usize as u32;
        let output = addresses[5] + STATE_BYTES as u32;
        let accepted = unsafe {
            tsfp_effects_binding_publish_rust(
                std::process::id() as i32,
                [0x11; 296].as_ptr(),
                [0x22; 8080].as_ptr(),
                [0x33; STATE_BYTES].as_ptr(),
                addresses.as_ptr(),
                output,
            )
        };
        assert!(!accepted);
        // SAFETY: the output mapping is still readable; publication failed before the final write.
        unsafe {
            assert_eq!(ptr::read(output as usize as *const u32), 0xA5A5_A5A5);
            assert_eq!(munmap(protected, 8192), 0);
            assert_eq!(munmap(mapping, length), 0);
        }
    }
}

// T3906 direct witness: the host (guest_mem.c) owns these; integration tests run without a host (recording off).
#[unsafe(no_mangle)]
pub static tsfp_hostmem_direct_active: u8 = 0;
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_hostmem_direct_prepare() -> u64 {
    0
}
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_hostmem_direct_write(_: u64, _: u32, _: *const u8, _: usize) {}
