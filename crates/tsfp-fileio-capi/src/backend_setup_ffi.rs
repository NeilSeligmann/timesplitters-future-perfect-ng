//! Select the Rust file-I/O backend tables from the Rust host boundary.
//!
//! Keeping registration here means the host setup C code does not call back
//! into the replaceable `kernel_file_*` ABI directly.
use libc::c_void;

unsafe extern "C" {
    fn tsfp_rfio_hostio_backend() -> *const c_void;
    fn tsfp_rfio_hostdir_backend() -> *const c_void;
    fn tsfp_rfio_handle_backend() -> *const c_void;
    fn tsfp_rfio_dir_backend() -> *const c_void;
    fn tsfp_rfio_disposition_backend() -> *const c_void;
    fn tsfp_rfio_commit_backend() -> *const c_void;
    fn tsfp_rfio_namespace_backend() -> *const c_void;
    fn tsfp_rfio_view_backend() -> *const c_void;
    fn tsfp_rfio_glue_backend() -> *const c_void;
    fn tsfp_rfio_payload_backend() -> *const c_void;
    fn tsfp_rfio_desc_backend() -> *const c_void;
    fn kernel_file_set_hostio_backend(backend: *const c_void);
    fn kernel_file_set_hostdir_backend(backend: *const c_void);
    fn kernel_file_set_handle_backend(backend: *const c_void);
    fn kernel_file_set_dir_backend(backend: *const c_void);
    fn kernel_file_set_disposition_backend(backend: *const c_void);
    fn kernel_file_set_commit_backend(backend: *const c_void);
    fn kernel_file_set_namespace_backend(backend: *const c_void);
    fn kernel_file_set_view_backend(backend: *const c_void);
    fn kernel_file_set_glue_backend(backend: *const c_void);
    fn kernel_file_set_payload_backend(backend: *const c_void);
    fn kernel_file_set_desc_backend(backend: *const c_void);
    fn kernel_file_set_payload_mirror(enabled: bool);
}

fn payload_mirror_enabled() -> bool {
    std::env::var("TSFP_RUST_FILEIO_PAYLOAD_MIRROR").is_ok_and(|value| value == "1")
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_rfio_install_backends() {
    // SAFETY: backend pointers refer to static Rust-owned tables, and the
    // selected C ABI setters copy only those stable pointers.
    unsafe {
        kernel_file_set_hostio_backend(tsfp_rfio_hostio_backend());
        kernel_file_set_hostdir_backend(tsfp_rfio_hostdir_backend());
        kernel_file_set_handle_backend(tsfp_rfio_handle_backend());
        kernel_file_set_dir_backend(tsfp_rfio_dir_backend());
        kernel_file_set_disposition_backend(tsfp_rfio_disposition_backend());
        kernel_file_set_commit_backend(tsfp_rfio_commit_backend());
        kernel_file_set_namespace_backend(tsfp_rfio_namespace_backend());
        kernel_file_set_view_backend(tsfp_rfio_view_backend());
        kernel_file_set_glue_backend(tsfp_rfio_glue_backend());
        kernel_file_set_payload_backend(tsfp_rfio_payload_backend());
        kernel_file_set_desc_backend(tsfp_rfio_desc_backend());
        kernel_file_set_payload_mirror(payload_mirror_enabled());
    }
}
