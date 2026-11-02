//! C ABI for selecting file sharing policy from the Rust host setup path.

unsafe extern "C" {
    fn kernel_file_set_share_enforcement(enabled: bool);
}

/// # Safety
/// The C setter is process-global and may only be called during host setup.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_rfio_set_share_enforcement(enabled: bool) {
    unsafe { kernel_file_set_share_enforcement(enabled) };
}
