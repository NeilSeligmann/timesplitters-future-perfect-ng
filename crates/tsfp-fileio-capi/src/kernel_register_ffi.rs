//! Move the production registration call through Rust so native and legacy
//! builds select the same `kernel_file_register` symbol at link time.

unsafe extern "C" {
    fn kernel_file_register() -> u32;
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_rfio_kernel_file_register() -> u32 {
    // The selected implementation is Rust in TSFP_NATIVE_FILEIO builds and C
    // in the compatibility build. The C registration orchestrator never calls
    // the replaceable symbol directly.
    unsafe { kernel_file_register() }
}
