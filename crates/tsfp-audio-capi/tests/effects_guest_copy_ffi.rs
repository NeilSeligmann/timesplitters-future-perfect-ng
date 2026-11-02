#[cfg(target_os = "linux")]
mod linux {
    use std::ffi::c_void;

    pub const PAGE_BYTES: usize = 4096;
    const PROT_READ: i32 = 1;
    const PROT_WRITE: i32 = 2;
    const PROT_NONE: i32 = 0;
    const MAP_PRIVATE: i32 = 2;
    const MAP_ANONYMOUS: i32 = 0x20;
    const MAP_32BIT: i32 = 0x40;

    unsafe extern "C" {
        fn mmap(
            address: *mut c_void,
            length: usize,
            protection: i32,
            flags: i32,
            fd: i32,
            offset: isize,
        ) -> *mut c_void;
        fn mprotect(address: *mut c_void, length: usize, protection: i32) -> i32;
        fn munmap(address: *mut c_void, length: usize) -> i32;
    }

    pub fn map_guest_page() -> (*mut u8, u32) {
        // Linux x86_64's MAP_32BIT gives this test a real identity-mapped 32-bit guest address.
        // SAFETY: mmap owns the returned anonymous page until the test unmaps it.
        let span = unsafe {
            mmap(
                std::ptr::null_mut(),
                PAGE_BYTES,
                PROT_READ | PROT_WRITE,
                MAP_PRIVATE | MAP_ANONYMOUS | MAP_32BIT,
                -1,
                0,
            )
        };
        assert_ne!(span, (-1isize) as *mut c_void);
        assert!(!span.is_null());
        assert!((span as usize) <= u32::MAX as usize);
        (span.cast(), span as usize as u32)
    }

    pub fn protect_none(span: *mut u8) {
        // SAFETY: the test owns one page at this address.
        assert_eq!(unsafe { mprotect(span.cast(), PAGE_BYTES, PROT_NONE) }, 0);
    }

    pub fn unmap(span: *mut u8) {
        // SAFETY: the test owns one page at this address and releases it once.
        assert_eq!(unsafe { munmap(span.cast(), PAGE_BYTES) }, 0);
    }
}

#[cfg(target_os = "linux")]
#[test]
fn copies_between_local_buffers_and_guest_address() {
    use tsfp_audio_capi::effects_guest_copy_ffi::{
        tsfp_effects_guest_copy_read_rust, tsfp_effects_guest_copy_write_rust,
    };

    let (guest_span, guest_address) = linux::map_guest_page();
    let source = [0x35; 32];
    let mut restored = [0; 32];
    let pid = std::process::id() as i32;

    // SAFETY: the local and guest spans remain live and writable/readable for each call.
    assert!(unsafe {
        tsfp_effects_guest_copy_write_rust(pid, source.as_ptr(), guest_address, source.len())
    });
    // SAFETY: the local and guest spans remain live and readable/writable for this call.
    assert!(unsafe {
        tsfp_effects_guest_copy_read_rust(pid, guest_address, restored.as_mut_ptr(), restored.len())
    });

    // SAFETY: this test owns the guest page and initialized its first 32 bytes.
    assert_eq!(
        unsafe { std::slice::from_raw_parts(guest_span, 32) },
        &source
    );
    assert_eq!(restored, source);
    linux::unmap(guest_span);
}

#[cfg(target_os = "linux")]
#[test]
fn inaccessible_guest_address_is_refused_without_local_mutation() {
    use tsfp_audio_capi::effects_guest_copy_ffi::{
        tsfp_effects_guest_copy_read_rust, tsfp_effects_guest_copy_write_rust,
    };

    let (guest_span, guest_address) = linux::map_guest_page();
    let mut local = [0xA5; 16];
    let source = [0x35; 16];
    let pid = std::process::id() as i32;

    // SAFETY: this test owns the writable page and initializes the exact tested span.
    unsafe { std::ptr::copy_nonoverlapping(source.as_ptr(), guest_span, source.len()) };
    linux::protect_none(guest_span);
    // SAFETY: Linux refuses the protected remote span without dereferencing it in this process.
    assert!(!unsafe {
        tsfp_effects_guest_copy_read_rust(pid, guest_address, local.as_mut_ptr(), local.len())
    });
    assert_eq!(local, [0xA5; 16]);
    // SAFETY: the protected guest destination must be refused by the fault-safe syscall.
    assert!(!unsafe {
        tsfp_effects_guest_copy_write_rust(pid, source.as_ptr(), guest_address, source.len())
    });
    linux::unmap(guest_span);
}

#[test]
fn invalid_guest_ranges_are_refused_before_syscall() {
    use tsfp_audio_capi::effects_guest_copy_ffi::{
        tsfp_effects_guest_copy_read_rust, tsfp_effects_guest_copy_write_rust,
    };

    let mut local = [0xA5; 8];
    let source = [0x35; 8];
    let pid = std::process::id() as i32;
    // SAFETY: local buffers are valid; zero and overflowing guest ranges are rejected first.
    assert!(!unsafe { tsfp_effects_guest_copy_read_rust(pid, 0, local.as_mut_ptr(), local.len()) });
    // SAFETY: the guest range runs past 2^32 and is rejected before the syscall.
    assert!(!unsafe {
        tsfp_effects_guest_copy_write_rust(pid, source.as_ptr(), u32::MAX - 3, source.len())
    });
    assert_eq!(local, [0xA5; 8]);
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
