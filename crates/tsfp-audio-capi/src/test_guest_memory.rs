use std::ffi::c_void;

const PAGE_BYTES: usize = 4096;
const PROT_READ: i32 = 1;
const PROT_WRITE: i32 = 2;
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
        offset: i64,
    ) -> *mut c_void;
    fn munmap(address: *mut c_void, length: usize) -> i32;
}

pub fn map_guest_page() -> (*mut u8, u32) {
    // Linux x86_64's MAP_32BIT gives the decoder a real identity-mapped guest address.
    // SAFETY: mmap owns the returned anonymous page until the test unmaps it.
    let page = unsafe {
        mmap(
            std::ptr::null_mut(),
            PAGE_BYTES,
            PROT_READ | PROT_WRITE,
            MAP_PRIVATE | MAP_ANONYMOUS | MAP_32BIT,
            -1,
            0,
        )
    };
    assert_ne!(page, (-1isize) as *mut c_void);
    assert!(!page.is_null());
    assert!((page as usize) <= u32::MAX as usize);
    (page.cast(), page as usize as u32)
}

pub fn unmap_guest_page(page: *mut u8) {
    // SAFETY: the caller owns this one-page mapping and releases it once.
    assert_eq!(unsafe { munmap(page.cast(), PAGE_BYTES) }, 0);
}
