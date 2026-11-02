//! Rust-owned SILENT DirectSound device facade and device lease table for the Rust audio host (T2046).
//!
//! Replaces the state and logic of `crates/tsfp-audio-conformance/oracle/dsound_device.c`: `tsfp_audio::lifecycle::Device` owns the
//! owned-device record, the reference count, the lease ledger and the movie references behind one mutex, and
//! this module is its `extern "C"` surface plus a [`GuestWorld`] over the host guest memory and guest heap.
//! C keeps only the Create/Release frame decoding, the fatal handler and the log. Contract:
//! `docs/t-audio-replace.md` (device lease section). The lease ops callbacks run under the device mutex,
//! exactly as in the C oracle (no device reentry).
use crate::frame_guest_memory::FrameGuestMemory;
use std::ffi::c_void;
use std::sync::{LazyLock, Mutex, MutexGuard};
use tsfp_audio::adapter::{GuestMemory, Refusal};
use tsfp_audio::lifecycle::{Child, Device, GuestWorld, Lease, LeaseError, ReleaseError};

unsafe extern "C" {
    fn kernel_guest_at_ro(address: u32, length: usize) -> *const c_void;
    fn kernel_guest_range_readable(address: u32, length: usize) -> bool;
    #[cfg(not(test))]
    fn kmem_heap_create(flags: u32, initial: u32, maximum: u32) -> u32;
    #[cfg(not(test))]
    fn kmem_heap_alloc(heap: u32, bytes: u32) -> u32;
    #[cfg(not(test))]
    fn kmem_heap_valid(heap: u32) -> bool;
    #[cfg(not(test))]
    fn kmem_heap_block_size(heap: u32, address: u32, size_out: *mut u32) -> bool;
    #[cfg(not(test))]
    fn kmem_heap_destroy(heap: u32) -> bool;
}

/// Host guest-memory mapping query (`kernel_guest_at_ro`).
unsafe fn tsfp_audio_dev_mapped(address: u32, len: u32) -> bool {
    // SAFETY: scalar probe.
    !unsafe { kernel_guest_at_ro(address, len as usize) }.is_null()
}

unsafe fn tsfp_audio_dev_readable(address: u32, len: u32) -> bool {
    // SAFETY: scalar probe.
    unsafe { kernel_guest_range_readable(address, len as usize) }
}

// The guest heap services the Rust kmem selector falls back to when it is not active (the kernel memory ABI).
#[cfg(not(test))]
unsafe fn tsfp_audio_dev_heap_create() -> u32 {
    // SAFETY: scalar arguments; GUEST_HEAP_CHUNK_MIN is 0x10000.
    unsafe { kmem_heap_create(0, 0x1_0000, 0) }
}

#[cfg(not(test))]
unsafe fn tsfp_audio_dev_heap_alloc(heap: u32, bytes: u32) -> u32 {
    // SAFETY: scalar arguments.
    unsafe { kmem_heap_alloc(heap, bytes) }
}

#[cfg(not(test))]
unsafe fn tsfp_audio_dev_heap_valid(heap: u32) -> bool {
    // SAFETY: scalar argument.
    unsafe { kmem_heap_valid(heap) }
}

#[cfg(not(test))]
unsafe fn tsfp_audio_dev_heap_destroy(heap: u32) -> bool {
    // SAFETY: scalar argument.
    unsafe { kmem_heap_destroy(heap) }
}

#[cfg(not(test))]
unsafe fn tsfp_audio_dev_heap_block_size(heap: u32, address: u32, size_out: *mut u32) -> bool {
    // SAFETY: `size_out` is a live slot from the caller.
    unsafe { kmem_heap_block_size(heap, address, size_out) }
}

// Unit-test binaries do not link the C audio host. These two services are reached
// by stream-lease tests through HostWorld, so provide inert test-only compatibility
// symbols; production builds continue to resolve the C ABI above.
#[cfg(test)]
unsafe fn tsfp_audio_dev_heap_create() -> u32 {
    0
}

#[cfg(test)]
unsafe fn tsfp_audio_dev_heap_alloc(_heap: u32, _bytes: u32) -> u32 {
    0
}

#[cfg(test)]
unsafe fn tsfp_audio_dev_heap_valid(_heap: u32) -> bool {
    false
}

#[cfg(test)]
unsafe fn tsfp_audio_dev_heap_destroy(_heap: u32) -> bool {
    false
}

#[cfg(test)]
unsafe fn tsfp_audio_dev_heap_block_size(_heap: u32, _address: u32, _size_out: *mut u32) -> bool {
    false
}

#[cfg(feature = "rust-kmem")]
unsafe extern "C" {
    fn tsfp_kmem_audio_heap_active() -> bool;
    fn tsfp_kmem_audio_heap_create(flags: u32, initial: u32, maximum: u32) -> u32;
    fn tsfp_kmem_audio_heap_alloc(heap: u32, bytes: u32) -> u32;
    fn tsfp_kmem_audio_heap_valid(heap: u32) -> bool;
    fn tsfp_kmem_audio_heap_block_size(heap: u32, address: u32, size_out: *mut u32) -> bool;
    fn tsfp_kmem_audio_heap_destroy(heap: u32) -> bool;
}

// Keep this aligned with GUEST_HEAP_CHUNK_MIN in src/xbox/guest_mem.h.
#[cfg(feature = "rust-kmem")]
const PRIVATE_STREAM_HEAP_INITIAL_BYTES: u32 = 0x1_0000;

#[cfg(any(feature = "rust-kmem", test))]
fn prefer_rust_kmem<T>(active: bool, rust: impl FnOnce() -> T, c: impl FnOnce() -> T) -> T {
    if active { rust() } else { c() }
}

/// The host guest memory and guest heap (stateless: C owns the bytes).
pub(crate) struct HostWorld;

fn host_pid() -> i32 {
    i32::try_from(std::process::id()).unwrap_or(0)
}

impl GuestMemory for HostWorld {
    fn read_u32(&self, address: u32) -> Option<u32> {
        let bytes = self.read_bytes(address, 4)?;
        Some(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }
    fn write_u32(&mut self, address: u32, value: u32) -> bool {
        self.write_bytes(address, &value.to_le_bytes())
    }
    fn read_bytes(&self, address: u32, len: usize) -> Option<Vec<u8>> {
        FrameGuestMemory::new(host_pid()).read_bytes(address, len)
    }
    fn range_readable(&self, address: u32, len: u32) -> bool {
        // SAFETY: plain probe, no pointers.
        unsafe { tsfp_audio_dev_readable(address, len) }
    }
}

impl GuestWorld for HostWorld {
    fn mapped(&self, address: u32, len: u32) -> bool {
        // SAFETY: plain probe, no pointers.
        unsafe { tsfp_audio_dev_mapped(address, len) }
    }
    fn write_bytes(&mut self, address: u32, data: &[u8]) -> bool {
        FrameGuestMemory::new(host_pid()).write_bytes(address, data)
    }
    fn heap_create(&mut self) -> u32 {
        PrivateStreamHeap::create(self)
    }
    fn heap_alloc(&mut self, heap: u32, bytes: u32) -> u32 {
        PrivateStreamHeap::allocate(self, heap, bytes)
    }
    fn heap_destroy(&mut self, heap: u32) -> bool {
        PrivateStreamHeap::destroy(self, heap)
    }
    fn heap_valid(&self, heap: u32) -> bool {
        PrivateStreamHeap::valid(self, heap)
    }
    fn heap_block_size(&self, heap: u32, address: u32) -> Option<u32> {
        let mut size = 0u32;
        #[cfg(feature = "rust-kmem")]
        {
            // SAFETY: kmem's selector is scalar; `size` is live for either synchronous backend.
            let active = unsafe { tsfp_kmem_audio_heap_active() };
            let ok = if active {
                // SAFETY: `size` is a live out slot for the synchronous Rust kmem call.
                unsafe { tsfp_kmem_audio_heap_block_size(heap, address, &mut size) }
            } else {
                // SAFETY: `size` is a live out slot for the synchronous C compatibility call.
                unsafe { tsfp_audio_dev_heap_block_size(heap, address, &mut size) }
            };
            return ok.then_some(size);
        }
        #[cfg(not(feature = "rust-kmem"))]
        {
            // SAFETY: `size` is a live out slot.
            unsafe { tsfp_audio_dev_heap_block_size(heap, address, &mut size) }.then_some(size)
        }
    }
}

trait PrivateStreamHeap {
    fn create(&mut self) -> u32;
    fn allocate(&mut self, heap: u32, bytes: u32) -> u32;
    fn valid(&self, heap: u32) -> bool;
    fn destroy(&mut self, heap: u32) -> bool;
}

impl PrivateStreamHeap for HostWorld {
    fn create(&mut self) -> u32 {
        #[cfg(feature = "rust-kmem")]
        {
            // SAFETY: Rust kmem exports the same guest heap service used by the C adapter.
            let active = unsafe { tsfp_kmem_audio_heap_active() };
            return prefer_rust_kmem(
                active,
                || unsafe { tsfp_kmem_audio_heap_create(0, PRIVATE_STREAM_HEAP_INITIAL_BYTES, 0) },
                || unsafe { tsfp_audio_dev_heap_create() },
            );
        }
        #[cfg(not(feature = "rust-kmem"))]
        {
            // SAFETY: no pointers; use the host guest-heap service when Rust kmem is absent.
            unsafe { tsfp_audio_dev_heap_create() }
        }
    }

    fn allocate(&mut self, heap: u32, bytes: u32) -> u32 {
        #[cfg(feature = "rust-kmem")]
        {
            // SAFETY: the heap handle and byte count are scalar guest-heap inputs.
            let active = unsafe { tsfp_kmem_audio_heap_active() };
            return prefer_rust_kmem(
                active,
                || unsafe { tsfp_kmem_audio_heap_alloc(heap, bytes) },
                || unsafe { tsfp_audio_dev_heap_alloc(heap, bytes) },
            );
        }
        #[cfg(not(feature = "rust-kmem"))]
        {
            // SAFETY: no pointers; use the host guest-heap service when Rust kmem is absent.
            unsafe { tsfp_audio_dev_heap_alloc(heap, bytes) }
        }
    }

    fn valid(&self, heap: u32) -> bool {
        #[cfg(feature = "rust-kmem")]
        {
            // SAFETY: the heap handle is a scalar guest-heap input.
            let active = unsafe { tsfp_kmem_audio_heap_active() };
            return prefer_rust_kmem(
                active,
                || unsafe { tsfp_kmem_audio_heap_valid(heap) },
                || unsafe { tsfp_audio_dev_heap_valid(heap) },
            );
        }
        #[cfg(not(feature = "rust-kmem"))]
        {
            // SAFETY: no pointers; use the host guest-heap service when Rust kmem is absent.
            unsafe { tsfp_audio_dev_heap_valid(heap) }
        }
    }

    fn destroy(&mut self, heap: u32) -> bool {
        #[cfg(feature = "rust-kmem")]
        {
            // SAFETY: the heap handle is a scalar guest-heap input.
            let active = unsafe { tsfp_kmem_audio_heap_active() };
            return prefer_rust_kmem(
                active,
                || unsafe { tsfp_kmem_audio_heap_destroy(heap) },
                || unsafe { tsfp_audio_dev_heap_destroy(heap) },
            );
        }
        #[cfg(not(feature = "rust-kmem"))]
        {
            // SAFETY: no pointers; use the host guest-heap service when Rust kmem is absent.
            unsafe { tsfp_audio_dev_heap_destroy(heap) }
        }
    }
}

fn allocate_stream_object(world: &mut impl PrivateStreamHeap, bytes: u32) -> (u32, u32) {
    let heap = world.create();
    let address = if heap == 0 {
        0
    } else {
        world.allocate(heap, bytes)
    };
    (heap, address)
}

/// Allocate the private guest heap and stream object as one Rust-owned operation.
/// On allocation failure the created heap is returned so the caller can apply the
/// existing rollback policy; `false` means heap creation itself failed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_stream_heap_allocate_rust(
    bytes: u32,
    heap_out: *mut u32,
    address_out: *mut u32,
) -> bool {
    if heap_out.is_null() || address_out.is_null() {
        return false;
    }
    let (heap, address) = allocate_stream_object(&mut HostWorld, bytes);
    // SAFETY: both out-pointers are checked and belong to the synchronous C caller.
    unsafe {
        heap_out.write(heap);
        address_out.write(address);
    }
    heap != 0
}

/// Validate or destroy a stream heap on the Rust audio-host path.
/// Active Rust kmem uses its typed guest-region backend; otherwise the established C service
/// remains the compatibility backend. These calls do not change caller rollback ordering.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_stream_heap_valid_rust(heap: u32) -> bool {
    HostWorld.valid(heap)
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_stream_heap_destroy_rust(heap: u32) -> bool {
    HostWorld.destroy(heap)
}

#[cfg(test)]
mod stream_heap_tests {
    use super::{PrivateStreamHeap, allocate_stream_object};
    use std::cell::RefCell;

    struct FakeHeap {
        heap: u32,
        address: u32,
        calls: Vec<(u8, u32, u32)>,
        valid: bool,
        destroy: bool,
    }

    impl PrivateStreamHeap for FakeHeap {
        fn create(&mut self) -> u32 {
            self.calls.push((1, 0, 0));
            self.heap
        }

        fn allocate(&mut self, heap: u32, bytes: u32) -> u32 {
            self.calls.push((2, heap, bytes));
            self.address
        }

        fn valid(&self, _heap: u32) -> bool {
            self.valid
        }

        fn destroy(&mut self, heap: u32) -> bool {
            self.calls.push((3, heap, 0));
            self.destroy
        }
    }

    #[test]
    fn stream_heap_creation_precedes_object_allocation() {
        let mut fake = FakeHeap {
            heap: 7,
            address: 0x1234,
            calls: Vec::new(),
            valid: true,
            destroy: true,
        };
        assert_eq!(allocate_stream_object(&mut fake, 44), (7, 0x1234));
        assert_eq!(fake.calls, [(1, 0, 0), (2, 7, 44)]);
    }

    #[test]
    fn failed_object_allocation_keeps_created_heap_for_rollback() {
        let mut fake = FakeHeap {
            heap: 7,
            address: 0,
            calls: Vec::new(),
            valid: true,
            destroy: true,
        };
        assert_eq!(allocate_stream_object(&mut fake, 44), (7, 0));
        assert_eq!(fake.calls.len(), 2);
    }

    #[test]
    fn failed_heap_creation_skips_object_allocation() {
        let mut fake = FakeHeap {
            heap: 0,
            address: 0x1234,
            calls: Vec::new(),
            valid: false,
            destroy: false,
        };
        assert_eq!(allocate_stream_object(&mut fake, 44), (0, 0));
        assert_eq!(fake.calls, [(1, 0, 0)]);
    }

    #[test]
    fn stream_heap_uses_rust_kmem_only_when_that_backend_is_active() {
        let calls = RefCell::new(Vec::new());
        assert_eq!(
            super::prefer_rust_kmem(
                true,
                || {
                    calls.borrow_mut().push("rust");
                    7
                },
                || {
                    calls.borrow_mut().push("c");
                    9
                }
            ),
            7
        );
        assert_eq!(calls.borrow().as_slice(), ["rust"]);

        calls.borrow_mut().clear();
        assert_eq!(
            super::prefer_rust_kmem(
                false,
                || {
                    calls.borrow_mut().push("rust");
                    7
                },
                || {
                    calls.borrow_mut().push("c");
                    9
                }
            ),
            9
        );
        assert_eq!(calls.borrow().as_slice(), ["c"]);
    }

    #[test]
    fn stream_heap_validate_and_destroy_follow_the_selected_backend() {
        let calls = RefCell::new(Vec::new());
        let valid = super::prefer_rust_kmem(
            true,
            || {
                calls.borrow_mut().push("rust-valid");
                true
            },
            || {
                calls.borrow_mut().push("c-valid");
                false
            },
        );
        let destroyed = super::prefer_rust_kmem(
            false,
            || {
                calls.borrow_mut().push("rust-destroy");
                false
            },
            || {
                calls.borrow_mut().push("c-destroy");
                true
            },
        );
        assert!(valid);
        assert!(destroyed);
        assert_eq!(calls.borrow().as_slice(), ["rust-valid", "c-destroy"]);
    }

    #[test]
    fn heap_block_size_uses_the_selected_backend() {
        let calls = RefCell::new(Vec::new());
        let rust_ok = super::prefer_rust_kmem(
            true,
            || {
                calls.borrow_mut().push("rust-block-size");
                true
            },
            || {
                calls.borrow_mut().push("c-block-size");
                false
            },
        );
        assert!(rust_ok);
        assert_eq!(calls.borrow().as_slice(), ["rust-block-size"]);

        calls.borrow_mut().clear();
        let c_ok = super::prefer_rust_kmem(
            false,
            || {
                calls.borrow_mut().push("rust-block-size");
                false
            },
            || {
                calls.borrow_mut().push("c-block-size");
                true
            },
        );
        assert!(c_ok);
        assert_eq!(calls.borrow().as_slice(), ["c-block-size"]);
    }
}

#[cfg(test)]
mod guest_memory_tests {
    use super::HostWorld;
    use tsfp_audio::adapter::GuestMemory;
    use tsfp_audio::lifecycle::GuestWorld;

    #[test]
    fn host_world_reads_and_writes_guest_memory_through_rust_copy_adapter() {
        let (page, address) = crate::test_guest_memory::map_guest_page();
        let mut world = HostWorld;
        assert!(world.write_bytes(address, &[0x78, 0x56, 0x34, 0x12]));
        assert_eq!(world.read_u32(address), Some(0x1234_5678));
        crate::test_guest_memory::unmap_guest_page(page);
        assert!(!world.write_bytes(address, &[1]));
    }
}

static DEVICE: LazyLock<Mutex<Device>> = LazyLock::new(|| Mutex::new(Device::new()));

fn device() -> MutexGuard<'static, Device> {
    DEVICE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// `dsound_device_lease` (C layout).
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct CLease {
    pub(crate) device_heap: u32,
    pub(crate) internal_address: u32,
    pub(crate) serial: u64,
}

/// `dsound_device_lease_child` (C layout).
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct CChild {
    pub(crate) heap: u32,
    pub(crate) address: u32,
    pub(crate) bytes: u32,
}

/// `dsound_device_lease_ops` (C layout); any null entry is an invalid transaction.
#[repr(C)]
pub struct CLeaseOps {
    pub(crate) prepare:
        Option<unsafe extern "C" fn(*const CLease, *mut c_void, *mut CChild) -> bool>,
    pub(crate) abort: Option<unsafe extern "C" fn(*mut c_void)>,
    pub(crate) finalize: Option<unsafe extern "C" fn(*const CLease, *mut c_void)>,
}

impl From<Lease> for CLease {
    fn from(l: Lease) -> Self {
        Self {
            device_heap: l.device_heap,
            internal_address: l.internal_address,
            serial: l.serial,
        }
    }
}
impl From<CLease> for Lease {
    fn from(l: CLease) -> Self {
        Self {
            device_heap: l.device_heap,
            internal_address: l.internal_address,
            serial: l.serial,
        }
    }
}
impl From<CChild> for Child {
    fn from(c: CChild) -> Self {
        Self {
            heap: c.heap,
            address: c.address,
            bytes: c.bytes,
        }
    }
}

/// `dsound_device_lease_status` values.
const STATUS_OK: i32 = 0;
const STATUS_INVALID: i32 = 1;
const STATUS_REFERENCE_LIMIT: i32 = 2;
const STATUS_SERIAL_LIMIT: i32 = 3;
const STATUS_PREPARE_FAILED: i32 = 5;
const STATUS_WRITE_REFUSED: i32 = 6;

fn status_code(error: LeaseError) -> i32 {
    match error {
        LeaseError::Invalid => STATUS_INVALID,
        LeaseError::ReferenceLimit => STATUS_REFERENCE_LIMIT,
        LeaseError::SerialLimit => STATUS_SERIAL_LIMIT,
        LeaseError::PrepareFailed => STATUS_PREPARE_FAILED,
        LeaseError::WriteRefused => STATUS_WRITE_REFUSED,
    }
}

/// A refusal reason as pointer and length (not NUL terminated); null when there is none.
#[repr(C)]
pub struct CReason {
    text: *const u8,
    len: usize,
}

impl CReason {
    pub(crate) fn none() -> Self {
        Self::NONE
    }
    /// The refusal text (empty when there is none).
    pub(crate) fn as_str(&self) -> &str {
        if self.text.is_null() {
            return "";
        }
        // SAFETY: the pointer and length came from a `&'static str` refusal.
        std::str::from_utf8(unsafe { std::slice::from_raw_parts(self.text, self.len) })
            .unwrap_or("")
    }
    const NONE: Self = Self {
        text: std::ptr::null(),
        len: 0,
    };
    fn of(refusal: Refusal) -> Self {
        Self {
            text: refusal.0.as_ptr(),
            len: refusal.0.len(),
        }
    }
}

/// Result of a Create: `status` is the HRESULT, `announce` asks C to log the one-time facade line.
#[repr(C)]
pub struct CCreateOutcome {
    pub(crate) status: u32,
    pub(crate) announce: bool,
    pub(crate) reason: CReason,
}

/// DirectSoundCreate for a validated frame. `note` is the optional PCM epoch observer. Returns false on a
/// named refusal (`outcome.reason`), which the C caller turns into the fatal stop.
///
/// # Safety
/// `outcome` must be a valid writable pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_device_create(
    guid: u32,
    output: u32,
    outer: u32,
    movie: bool,
    codec_ready: bool,
    note: Option<unsafe extern "C" fn(u32, u32) -> bool>,
    outcome: *mut CCreateOutcome,
) -> bool {
    let mut device = device();
    let announce = !device.announced();
    let result = device.create_with_note(
        &mut HostWorld,
        guid,
        output,
        outer,
        movie,
        codec_ready,
        |heap, address| match note {
            // SAFETY: the registered observer is a plain C function.
            Some(note) => unsafe { note(heap, address) },
            None => true,
        },
    );
    let (ok, value) = match result {
        Ok(status) => (
            true,
            CCreateOutcome {
                status,
                announce,
                reason: CReason::NONE,
            },
        ),
        Err(refusal) => (
            false,
            CCreateOutcome {
                status: 0,
                announce: false,
                reason: CReason::of(refusal),
            },
        ),
    };
    // SAFETY: caller contract.
    unsafe { outcome.write(value) };
    ok
}

/// Device Release from the movie caller. True with `*result` set, false with `*reason` set on refusal.
///
/// # Safety
/// `result` and `reason` must be valid writable pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_device_release_movie(
    interface: u32,
    result: *mut u32,
    reason: *mut CReason,
) -> bool {
    match device().release_movie(&mut HostWorld, interface) {
        Ok(count) => {
            // SAFETY: caller contract.
            unsafe { result.write(count) };
            true
        }
        Err(refusal) => {
            // SAFETY: caller contract.
            unsafe { reason.write(CReason::of(refusal)) };
            false
        }
    }
}

/// `dsound_owned_interface_fn` / `dsound_owned_identity_fn` share this shape for the FFI: the callback gets
/// the owned internal address (and heap) under the device mutex.
pub(crate) type OwnedCallback = unsafe extern "C" fn(*const u32, *mut c_void, *mut u32);

fn with_owned(
    interface: u32,
    callback: Option<OwnedCallback>,
    userdata: *mut c_void,
    result: *mut u32,
    identity: bool,
) -> bool {
    let device = device();
    let (Some(callback), false) = (callback, result.is_null()) else {
        return false;
    };
    if device.reference_count() == u32::MAX || !device.validate_owned(&HostWorld, interface) {
        return false;
    }
    let words = [device.owned_heap(), device.owned_address()];
    // Interface callbacks take the internal address alone, identity callbacks the (heap, address) pair.
    let argument = if identity { words.as_ptr() } else { &words[1] };
    // SAFETY: the callback is the C observer; it runs under the device mutex like the C oracle.
    unsafe { callback(argument, userdata, result) };
    true
}

/// `dsound_device_with_owned_interface`.
///
/// # Safety
/// `callback` receives a pointer to one u32; `result` must be writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_device_with_owned_interface(
    interface: u32,
    callback: Option<OwnedCallback>,
    userdata: *mut c_void,
    result: *mut u32,
) -> bool {
    with_owned(interface, callback, userdata, result, false)
}

/// `dsound_device_with_owned_identity`.
///
/// # Safety
/// `callback` receives a pointer to two u32 (heap, address); `result` must be writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_device_with_owned_identity(
    interface: u32,
    callback: Option<OwnedCallback>,
    userdata: *mut c_void,
    result: *mut u32,
) -> bool {
    with_owned(interface, callback, userdata, result, true)
}

/// `dsound_device_acquire_lease`: runs `ops.prepare` after the reference probe, `ops.finalize` after the
/// commit and `ops.abort` for any refusal after `prepare` ran.
///
/// # Safety
/// `ops` and `output` must be valid pointers for the call.
#[unsafe(export_name = "dsound_device_acquire_lease")]
pub unsafe extern "C" fn tsfp_audio_device_acquire_lease(
    interface: u32,
    ops: *const CLeaseOps,
    userdata: *mut c_void,
    output: *mut CLease,
) -> i32 {
    // SAFETY: caller contract; null is checked.
    let Some(ops) = (unsafe { ops.as_ref() }) else {
        return STATUS_INVALID;
    };
    let (Some(prepare), Some(abort), Some(finalize)) = (ops.prepare, ops.abort, ops.finalize)
    else {
        return STATUS_INVALID;
    };
    if output.is_null() {
        return STATUS_INVALID;
    }
    let mut prepared = false;
    let result = device().acquire_lease(&mut HostWorld, interface, |_, token| {
        prepared = true;
        let c_token = CLease::from(*token);
        let mut child = CChild::default();
        // SAFETY: C callback with a live token and out slot.
        unsafe { prepare(&c_token, userdata, &mut child) }.then(|| Child::from(child))
    });
    match result {
        Ok(token) => {
            let c_token = CLease::from(token);
            // SAFETY: C callback; device mutex is NOT dropped here, matching the oracle.
            unsafe { finalize(&c_token, userdata) };
            // SAFETY: caller contract.
            unsafe { output.write(c_token) };
            STATUS_OK
        }
        Err(error) => {
            if prepared {
                // SAFETY: C callback.
                unsafe { abort(userdata) };
            }
            status_code(error)
        }
    }
}

/// `dsound_device_release_lease`.
///
/// # Safety
/// `lease` and `ops` must be valid pointers for the call.
#[unsafe(export_name = "dsound_device_release_lease")]
pub unsafe extern "C" fn tsfp_audio_device_release_lease(
    lease: *const CLease,
    ops: *const CLeaseOps,
    userdata: *mut c_void,
) -> i32 {
    // SAFETY: caller contract; null is checked.
    let (Some(lease), Some(ops)) = (unsafe { lease.as_ref() }, unsafe { ops.as_ref() }) else {
        return STATUS_INVALID;
    };
    let (Some(prepare), Some(abort), Some(finalize)) = (ops.prepare, ops.abort, ops.finalize)
    else {
        return STATUS_INVALID;
    };
    let token = Lease::from(*lease);
    let c_token = *lease;
    match device().release_lease_with(&mut HostWorld, &token, |_| {
        let mut child = CChild::default();
        // SAFETY: C callback with a live token and out slot.
        unsafe { prepare(&c_token, userdata, &mut child) }.then(|| Child::from(child))
    }) {
        Ok(()) => {
            // SAFETY: C callback.
            unsafe { finalize(&c_token, userdata) };
            STATUS_OK
        }
        Err(ReleaseError::Refused(error)) => status_code(error),
        Err(ReleaseError::AfterPrepare(error)) => {
            // SAFETY: C callback.
            unsafe { abort(userdata) };
            status_code(error)
        }
    }
}

/// `dsound_device_reset_checked`.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_device_reset_checked() -> bool {
    device().reset_checked(&mut HostWorld)
}

/// `dsound_device_set_movie_calls`.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_device_set_movie_calls(enabled: bool) {
    device().movie_calls = enabled;
}

/// `dsound_device_movie_calls_enabled`.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_device_movie_calls_enabled() -> bool {
    device().movie_calls
}

/// `dsound_device_movie_references`.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_device_movie_references() -> u32 {
    device().movie_references()
}
