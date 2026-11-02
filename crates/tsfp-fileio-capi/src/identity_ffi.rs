//! T3879 early identity orchestration ABI. Actual backing resources remain in
//! the TEMPORARY-SHIM callbacks until FIO-3 removes the C physical state.
use std::ffi::c_void;
use tsfp_host_fileio_setup::identity::{self, Backend};

#[repr(C)]
struct DeviceBacking {
    root_fd: i32,
    filename: [u8; 256],
    capacity: u64,
}

unsafe extern "C" {
    fn kernel_file_mount_host_device(
        prefix: *const std::ffi::c_char,
        root: *const std::ffi::c_char,
        image: *const std::ffi::c_char,
        capacity: u64,
    ) -> bool;
    fn kernel_file_device_backing_acquire(
        prefix: *const std::ffi::c_char,
        out: *mut DeviceBacking,
    ) -> bool;
    fn disk_identity_xemu_acquire_backing(backing: *const DeviceBacking, snapshot: *mut c_void) -> i32;
}

struct RuntimeIdentity {
    host_root: *const std::ffi::c_char,
    snapshot: *mut c_void,
}

impl Backend for RuntimeIdentity {
    type Backing = DeviceBacking;

    fn mount(&mut self) -> bool {
        unsafe {
            kernel_file_mount_host_device(
                c"\\Device\\Harddisk0\\partition0".as_ptr(),
                self.host_root,
                c".tsfp-partition0.bin".as_ptr(),
                0x80000,
            )
        }
    }
    fn acquire(&mut self) -> Option<Self::Backing> {
        let mut backing = DeviceBacking {
            root_fd: -1,
            filename: [0; 256],
            capacity: 0,
        };
        unsafe {
            kernel_file_device_backing_acquire(c"\\Device\\Harddisk0\\partition0".as_ptr(), &mut backing)
                .then_some(backing)
        }
    }
    fn probe(&mut self, backing: &Self::Backing) -> u32 {
        unsafe { disk_identity_xemu_acquire_backing(backing, self.snapshot) as u32 }
    }
    fn release(&mut self, backing: Self::Backing) {
        unsafe { libc::close(backing.root_fd) };
    }
}

/// Owns mount/acquire/release in Rust while retaining the C xemu identity probe.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_rfio_host_identity_prepare(
    host_root: *const std::ffi::c_char,
    snapshot: *mut c_void,
    no_backing: u32,
    status: *mut u32,
) -> bool {
    if host_root.is_null() || snapshot.is_null() || status.is_null() {
        return false;
    }
    let mut backend = RuntimeIdentity { host_root, snapshot };
    let Ok(value) = identity::prepare(&mut backend, no_backing) else {
        return false;
    };
    unsafe { status.write(value) };
    true
}

type Mount = unsafe extern "C" fn(*mut c_void) -> bool;
type Acquire = unsafe extern "C" fn(*mut c_void) -> bool;
type Probe = unsafe extern "C" fn(*mut c_void) -> u32;
type Release = unsafe extern "C" fn(*mut c_void);

#[repr(C)]
pub struct IdentityBackend {
    mount: Option<Mount>,
    acquire: Option<Acquire>,
    probe: Option<Probe>,
    release: Option<Release>,
    no_backing: u32,
}

struct Adapter {
    context: *mut c_void,
    mount: Mount,
    acquire: Acquire,
    probe: Probe,
    release: Release,
}

impl Backend for Adapter {
    type Backing = ();

    fn mount(&mut self) -> bool {
        unsafe { (self.mount)(self.context) }
    }
    fn acquire(&mut self) -> Option<()> {
        unsafe { (self.acquire)(self.context) }.then_some(())
    }
    fn probe(&mut self, _: &()) -> u32 {
        unsafe { (self.probe)(self.context) }
    }
    fn release(&mut self, _: ()) {
        unsafe { (self.release)(self.context) }
    }
}

/// # Safety
/// Nonnull `raw` and `status` point to the declared C ABI structures and writable
/// status storage respectively. Each callback accepts `context` and must not
/// unwind. A successful acquire retains one backing until release; probe borrows
/// it. Callbacks and context remain valid throughout this synchronous call.
/// All required pointers are checked before any callback; failed mount leaves
/// status unchanged, while unavailable backing writes the supplied status code.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_rfio_host_identity(
    raw: *const IdentityBackend,
    context: *mut c_void,
    status: *mut u32,
) -> bool {
    if status.is_null() {
        return false;
    }
    let Some(raw) = (unsafe { raw.as_ref() }) else {
        return false;
    };
    let (Some(mount), Some(acquire), Some(probe), Some(release)) =
        (raw.mount, raw.acquire, raw.probe, raw.release)
    else {
        return false;
    };
    let mut adapter = Adapter {
        context,
        mount,
        acquire,
        probe,
        release,
    };
    let Ok(value) = identity::prepare(&mut adapter, raw.no_backing) else {
        return false;
    };
    unsafe {
        status.write(value);
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    struct State {
        calls: Vec<u8>,
        mount: bool,
        acquire: bool,
        probe: u32,
    }
    unsafe fn state<'a>(context: *mut c_void) -> &'a mut State {
        unsafe { &mut *context.cast::<State>() }
    }
    unsafe extern "C" fn mount(context: *mut c_void) -> bool {
        let s = unsafe { state(context) };
        s.calls.push(0);
        s.mount
    }
    unsafe extern "C" fn acquire(context: *mut c_void) -> bool {
        let s = unsafe { state(context) };
        s.calls.push(1);
        s.acquire
    }
    unsafe extern "C" fn probe(context: *mut c_void) -> u32 {
        let s = unsafe { state(context) };
        s.calls.push(2);
        s.probe
    }
    unsafe extern "C" fn release(context: *mut c_void) {
        unsafe { state(context) }.calls.push(3);
    }
    fn backend() -> IdentityBackend {
        IdentityBackend {
            mount: Some(mount),
            acquire: Some(acquire),
            probe: Some(probe),
            release: Some(release),
            no_backing: 79,
        }
    }
    fn fresh() -> State {
        State {
            calls: Vec::new(),
            mount: true,
            acquire: true,
            probe: 3,
        }
    }

    #[test]
    fn identity_ffi_validates_all_callbacks_before_side_effects() {
        for missing in 0..4 {
            let mut backend = backend();
            match missing {
                0 => backend.mount = None,
                1 => backend.acquire = None,
                2 => backend.probe = None,
                _ => backend.release = None,
            }
            let mut state = fresh();
            let mut status = 0xfeed;
            assert!(!unsafe {
                tsfp_rfio_host_identity(&backend, (&mut state as *mut State).cast(), &mut status)
            });
            assert!(state.calls.is_empty());
            assert_eq!(status, 0xfeed);
        }
    }

    #[test]
    fn identity_ffi_rejects_null_struct_and_output_without_callbacks() {
        let mut state = fresh();
        let mut status = 0xfeed;
        let context = (&mut state as *mut State).cast();
        assert!(!unsafe { tsfp_rfio_host_identity(std::ptr::null(), context, &mut status) });
        assert!(!unsafe { tsfp_rfio_host_identity(&backend(), context, std::ptr::null_mut()) });
        assert!(state.calls.is_empty());
        assert_eq!(status, 0xfeed);
    }

    #[test]
    fn identity_ffi_preserves_failure_outputs_and_release_order() {
        for mounted in [false, true] {
            for acquired in [false, true] {
                for probe_status in [0, 3, u32::MAX] {
                    let mut state = State {
                        calls: Vec::new(),
                        mount: mounted,
                        acquire: acquired,
                        probe: probe_status,
                    };
                    let mut status = 0xfeed;
                    let ok = unsafe {
                        tsfp_rfio_host_identity(
                            &backend(),
                            (&mut state as *mut State).cast(),
                            &mut status,
                        )
                    };
                    assert_eq!(ok, mounted);
                    if !mounted {
                        assert_eq!(status, 0xfeed);
                        assert_eq!(state.calls, [0]);
                    } else if !acquired {
                        assert_eq!(status, 79);
                        assert_eq!(state.calls, [0, 1]);
                    } else {
                        assert_eq!(status, probe_status);
                        assert_eq!(state.calls, [0, 1, 2, 3]);
                    }
                }
            }
        }
    }
}
