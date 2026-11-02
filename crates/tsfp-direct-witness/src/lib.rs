// SPDX-License-Identifier: GPL-3.0-or-later
//! T3906 direct-pointer writer witness for Rust `MappedGuest` stores (host side: `guest_mem.c`). One volatile flag
//! load per store when capture is off; when on, a prepare token before the store and the stored bytes reported after it.

unsafe extern "C" {
    static tsfp_hostmem_direct_active: u8;
    fn tsfp_hostmem_direct_prepare() -> u64;
    fn tsfp_hostmem_direct_write(token: u64, address: u32, bytes: *const u8, length: usize);
}

/// Fence token to take before a store, `None` when no capture is recording.
#[inline]
pub fn prepare() -> Option<u64> {
    // SAFETY: a plain flag owned by the host; prepare only reads the recording epoch.
    (unsafe { core::ptr::read_volatile(&raw const tsfp_hostmem_direct_active) } != 0)
        .then(|| unsafe { tsfp_hostmem_direct_prepare() })
}

/// Report the bytes just stored at guest `address`.
#[inline]
pub fn report(token: Option<u64>, address: u32, bytes: &[u8]) {
    if let Some(token) = token {
        // SAFETY: `bytes` is a live slice; the host copies it before returning.
        unsafe { tsfp_hostmem_direct_write(token, address, bytes.as_ptr(), bytes.len()) }
    }
}

/// Witnessed volatile byte store through host pointer `at` for guest `address`.
///
/// # Safety
/// `at` must be a valid writable host pointer.
#[inline]
pub unsafe fn store8(at: usize, address: u32, value: u8) {
    let token = prepare();
    unsafe { core::ptr::write_volatile(at as *mut u8, value) };
    report(token, address, &[value]);
}

/// Witnessed aligned volatile dword store.
///
/// # Safety
/// `at` must be a valid, 4-aligned writable host pointer.
#[inline]
pub unsafe fn store32(at: usize, address: u32, value: u32) {
    let token = prepare();
    unsafe { core::ptr::write_volatile(at as *mut u32, value.to_le()) };
    report(token, address, &value.to_le_bytes());
}

// The host (guest_mem.c) defines these symbols. Unit-test binaries of the crates that include this source run without a
// host, so they get recording-off stand-ins.
#[cfg(test)]
mod test_host_symbols {
    #[unsafe(no_mangle)]
    pub static tsfp_hostmem_direct_active: u8 = 0;
    #[unsafe(no_mangle)]
    pub extern "C" fn tsfp_hostmem_direct_prepare() -> u64 {
        0
    }
    #[unsafe(no_mangle)]
    pub extern "C" fn tsfp_hostmem_direct_write(_: u64, _: u32, _: *const u8, _: usize) {}
}
