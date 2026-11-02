//! C adapter boundary for the Rust-owned DirectSound stream lease/header predicate.

use std::ffi::c_void;
use std::sync::Mutex;

use tsfp_audio::adapter::GuestMemory;
#[cfg(feature = "rust-audio-host")]
use tsfp_audio::stream_lease::RollbackCleanupFailure;
#[cfg(test)]
use tsfp_audio::stream_lease::STREAM_CONTROL_ACCEPTED;
use tsfp_audio::stream_lease::{
    DeviceLeaseIdentity, STREAM_ALLOCATION_BYTES, STREAM_HEADER_WORDS, StreamControlRequest,
    StreamControlState, StreamFormatState, StreamLeaseGuestSpan, StreamLeaseHeader,
    StreamLeaseProbes, StreamLeaseRegistry, StreamLeaseScope, StreamSpatialState,
    StreamVolumeRequest, StreamVolumeState, validate_stream_lease,
};

use crate::frame_guest_memory::FrameGuestMemory;

#[cfg(feature = "rust-kmem")]
unsafe extern "C" {
    fn tsfp_kmem_audio_heap_active() -> bool;
    fn tsfp_kmem_audio_heap_valid(heap: u32) -> bool;
    fn tsfp_kmem_audio_heap_block_size(heap: u32, address: u32, size: *mut u32) -> bool;
}

pub const PROBE_DEVICE_HEAP: u32 = 0;
pub const PROBE_STREAM_HEAP: u32 = 2;
pub const PROBE_STREAM_BLOCK_SIZE: u32 = 3;

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CDeviceLease {
    pub device_heap: u32,
    pub internal_address: u32,
    pub serial: u64,
}

static STREAM_LEASES: Mutex<StreamLeaseRegistry> = Mutex::new(StreamLeaseRegistry::new());

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct OrphanRollback {
    stream_address: u32,
    stream_heap: u32,
}

#[derive(Default)]
struct OrphanRollbacks(Vec<OrphanRollback>);

impl OrphanRollbacks {
    fn retain(&mut self, record: OrphanRollback) -> bool {
        if self.0.try_reserve(1).is_err() {
            return false;
        }
        self.0.push(record);
        true
    }

    fn overlaps_guest_span(&self, address: u32, bytes: u32) -> bool {
        self.0.iter().any(|record| {
            record.stream_address != 0
                && overlaps(
                    address,
                    bytes,
                    record.stream_address,
                    STREAM_ALLOCATION_BYTES,
                )
        })
    }

    fn first(&self) -> Option<OrphanRollback> {
        self.0.first().copied()
    }

    fn remove(&mut self, record: OrphanRollback) -> bool {
        let Some(index) = self.0.iter().position(|candidate| *candidate == record) else {
            return false;
        };
        self.0.remove(index);
        true
    }
}

static ORPHAN_ROLLBACKS: Mutex<OrphanRollbacks> = Mutex::new(OrphanRollbacks(Vec::new()));

fn orphan_rollbacks() -> std::sync::MutexGuard<'static, OrphanRollbacks> {
    ORPHAN_ROLLBACKS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn retain_orphan_rollback(stream_address: u32, stream_heap: u32) -> bool {
    orphan_rollbacks().retain(OrphanRollback {
        stream_address,
        stream_heap,
    })
}

pub(crate) fn overlaps(address: u32, bytes: u32, other_address: u32, other_bytes: u32) -> bool {
    u64::from(address) < u64::from(other_address) + u64::from(other_bytes)
        && u64::from(other_address) < u64::from(address) + u64::from(bytes)
}

pub(crate) fn stream_leases() -> std::sync::MutexGuard<'static, StreamLeaseRegistry> {
    STREAM_LEASES
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl From<CDeviceLease> for DeviceLeaseIdentity {
    fn from(value: CDeviceLease) -> Self {
        Self {
            device_heap: value.device_heap,
            internal_address: value.internal_address,
            serial: value.serial,
        }
    }
}

/// Reserve a private stream address before the device lease is published to guest memory.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_stream_lease_reserve_rust(
    stream_address: u32,
    stream_heap: u32,
    publication_address: u32,
    header: *const u32,
    scope: *const StreamLeaseScope,
) -> bool {
    if header.is_null() || scope.is_null() {
        return false;
    }
    // SAFETY: the C adapter supplies live fixed-size snapshots synchronously.
    let (header, scope) = unsafe {
        (
            std::slice::from_raw_parts(header, STREAM_HEADER_WORDS)
                .try_into()
                .expect("fixed stream header length"),
            *scope,
        )
    };
    stream_leases().reserve_scoped(
        stream_address,
        stream_heap,
        publication_address,
        header,
        scope,
    )
}

/// Copy an active stream's validated creation scope for its exact lease generation.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_stream_lease_scope_rust(
    stream_address: u32,
    lease: *const CDeviceLease,
    scope_out: *mut StreamLeaseScope,
) -> bool {
    if lease.is_null() || scope_out.is_null() {
        return false;
    }
    // SAFETY: the C adapter supplies live synchronous lease and output records.
    let lease = unsafe { (*lease).into() };
    let Some(scope) = stream_leases().scope(stream_address, lease) else {
        return false;
    };
    // SAFETY: checked non-null and exclusively borrowed for this synchronous call.
    unsafe { *scope_out = scope };
    true
}

/// Copy mutable SetFormat bytes/count for the exact active lease generation.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_stream_lease_format_state_rust(
    stream_address: u32,
    lease: *const CDeviceLease,
    state_out: *mut StreamFormatState,
) -> bool {
    if lease.is_null() || state_out.is_null() {
        return false;
    }
    // SAFETY: the C adapter supplies live synchronous lease and output records.
    let lease = unsafe { (*lease).into() };
    let Some(state) = stream_leases().format_state(stream_address, lease) else {
        return false;
    };
    // SAFETY: checked non-null and exclusively borrowed for this synchronous call.
    unsafe { *state_out = state };
    true
}

/// Compare and commit one admitted SetFormat update for the exact active generation.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_stream_lease_format_commit_rust(
    stream_address: u32,
    lease: *const CDeviceLease,
    expected: *const StreamFormatState,
    bytes: *const u8,
) -> bool {
    if lease.is_null() || expected.is_null() || bytes.is_null() {
        return false;
    }
    // SAFETY: C supplies live fixed-size snapshots synchronously under its stream lock.
    let (lease, expected, bytes) = unsafe {
        (
            (*lease).into(),
            *expected,
            std::slice::from_raw_parts(bytes, tsfp_audio::scope::FORMAT_BYTES)
                .try_into()
                .expect("fixed stream format length"),
        )
    };
    stream_leases().commit_format_state(stream_address, lease, expected, bytes)
}

/// Copy the accepted SetVolume projection for the exact active stream generation.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_stream_lease_volume_state_rust(
    stream_address: u32,
    lease: *const CDeviceLease,
    state_out: *mut StreamVolumeState,
) -> bool {
    if lease.is_null() || state_out.is_null() {
        return false;
    }
    // SAFETY: the C adapter supplies live synchronous lease and output records.
    let lease = unsafe { (*lease).into() };
    let Some(state) = stream_leases().volume_state(stream_address, lease) else {
        return false;
    };
    // SAFETY: checked non-null and exclusively borrowed for this synchronous call.
    unsafe { *state_out = state };
    true
}

/// Compare and commit one accepted SetVolume projection for the exact active generation.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_stream_lease_volume_commit_rust(
    stream_address: u32,
    lease: *const CDeviceLease,
    expected: *const StreamVolumeState,
    next: *const StreamVolumeState,
) -> bool {
    if lease.is_null() || expected.is_null() || next.is_null() {
        return false;
    }
    // SAFETY: C supplies live fixed-size snapshots synchronously under its stream lock.
    let (lease, expected, next) = unsafe { ((*lease).into(), *expected, *next) };
    stream_leases().commit_volume_state(stream_address, lease, expected, next)
}

/// Plan the measured SetVolume decision for one exact active generation.
/// The C adapter commits the returned state only after routing accepts the update.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_stream_lease_volume_apply_rust(
    stream_address: u32,
    lease: *const CDeviceLease,
    request: *const StreamVolumeRequest,
    next_out: *mut StreamVolumeState,
) -> u32 {
    if lease.is_null() || request.is_null() || next_out.is_null() {
        return tsfp_audio::stream_lease::STREAM_VOLUME_BAD_REQUEST;
    }
    // SAFETY: C supplies live synchronous lease/request records and an output record.
    let (lease, request) = unsafe { ((*lease).into(), *request) };
    match stream_leases().plan_volume_request(stream_address, lease, request) {
        Ok(next) => {
            // SAFETY: checked non-null and exclusively borrowed for this synchronous call.
            unsafe { *next_out = next };
            tsfp_audio::stream_lease::STREAM_VOLUME_ACCEPTED
        }
        Err(status) => status,
    }
}

/// Copy accepted Pause/FlushEx/Discontinuity observations for the exact active generation.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_stream_lease_control_state_rust(
    stream_address: u32,
    lease: *const CDeviceLease,
    state_out: *mut StreamControlState,
) -> bool {
    if lease.is_null() || state_out.is_null() {
        return false;
    }
    // SAFETY: the C adapter supplies live synchronous lease and output records.
    let lease = unsafe { (*lease).into() };
    let Some(state) = stream_leases().control_state(stream_address, lease) else {
        return false;
    };
    // SAFETY: checked non-null and exclusively borrowed for this synchronous call.
    unsafe { *state_out = state };
    true
}

/// Compare and commit one accepted control observation for the exact active generation.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_stream_lease_control_commit_rust(
    stream_address: u32,
    lease: *const CDeviceLease,
    expected: *const StreamControlState,
    next: *const StreamControlState,
) -> bool {
    if lease.is_null() || expected.is_null() || next.is_null() {
        return false;
    }
    // SAFETY: C supplies live fixed-size snapshots synchronously under its stream lock.
    let (lease, expected, next) = unsafe { ((*lease).into(), *expected, *next) };
    stream_leases().commit_control_state(stream_address, lease, expected, next)
}

/// Evaluate and commit one measured startup stream-control operation in Rust.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_stream_lease_control_apply_rust(
    stream_address: u32,
    lease: *const CDeviceLease,
    request: *const StreamControlRequest,
) -> u32 {
    if lease.is_null() || request.is_null() {
        return tsfp_audio::stream_lease::STREAM_CONTROL_INVALID_REQUEST;
    }
    // SAFETY: C supplies live synchronous lease and request records under the stream lock.
    let (lease, request) = unsafe { ((*lease).into(), *request) };
    stream_leases().apply_control_request(stream_address, lease, request)
}

/// Copy accepted spatial observations for the exact active stream generation.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_stream_lease_spatial_state_rust(
    stream_address: u32,
    lease: *const CDeviceLease,
    state_out: *mut StreamSpatialState,
) -> bool {
    if lease.is_null() || state_out.is_null() {
        return false;
    }
    // SAFETY: the C adapter supplies live synchronous lease and output records.
    let lease = unsafe { (*lease).into() };
    let Some(state) = stream_leases().spatial_state(stream_address, lease) else {
        return false;
    };
    // SAFETY: checked non-null and exclusively borrowed for this synchronous call.
    unsafe { *state_out = state };
    true
}

/// Compare and commit one accepted spatial observation for the exact active generation.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_stream_lease_spatial_commit_rust(
    stream_address: u32,
    lease: *const CDeviceLease,
    expected: *const StreamSpatialState,
    next: *const StreamSpatialState,
) -> bool {
    if lease.is_null() || expected.is_null() || next.is_null() {
        return false;
    }
    // SAFETY: C supplies live fixed-size snapshots synchronously under its stream lock.
    let (lease, expected, next) = unsafe { ((*lease).into(), *expected, *next) };
    stream_leases().commit_spatial_state(stream_address, lease, expected, next)
}

/// Apply the bounded SetMaxDistance state transition in the Rust lease owner.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_stream_lease_max_distance_rust(
    stream_address: u32,
    lease: *const CDeviceLease,
    apply: u32,
    distance_bits: u32,
) -> bool {
    if lease.is_null() {
        return false;
    }
    // SAFETY: C supplies the active lease identity synchronously under its stream lock.
    let lease = unsafe { (*lease).into() };
    stream_leases().record_max_distance(stream_address, lease, apply, distance_bits)
}

/// Own ordered I3DL2/minimum-distance/startup-curve admission and state in Rust.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_stream_lease_startup_spatial_rust(
    stream_address: u32,
    lease: *const CDeviceLease,
    action: u32,
    argument: u32,
    count: u32,
    i3dl2_words: *const u32,
) -> u32 {
    if lease.is_null() || (action == 1 && i3dl2_words.is_null()) {
        return tsfp_audio::stream_lease::STREAM_SPATIAL_BAD_REQUEST;
    }
    // SAFETY: C holds the stream lock and supplies the previously guarded fixed-size read.
    let lease = unsafe { (*lease).into() };
    let words = if action == 1 {
        unsafe {
            std::slice::from_raw_parts(i3dl2_words, 9)
                .try_into()
                .expect("fixed I3DL2 word count")
        }
    } else {
        [0; 9]
    };
    stream_leases().apply_startup_spatial(stream_address, lease, action, argument, count, words)
}

/// Commit the exact device generation after its lease transaction succeeds.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_stream_lease_bind_rust(
    stream_address: u32,
    lease: *const CDeviceLease,
) -> bool {
    if lease.is_null() {
        return false;
    }
    // SAFETY: the C adapter supplies a live lease record for this synchronous call.
    let lease: DeviceLeaseIdentity = unsafe { (*lease).into() };
    stream_leases().bind(stream_address, lease)
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_stream_lease_cancel_pending_rust(stream_address: u32) -> bool {
    stream_leases().cancel_pending(stream_address)
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_stream_lease_retain_rollback_rust(stream_address: u32) -> bool {
    stream_leases().retain_rollback(stream_address)
}

/// Retain an allocation whose heap cleanup failed before the normal lease was reserved.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_stream_lease_retain_unreserved_rollback_rust(
    stream_address: u32,
    stream_heap: u32,
    publication_address: u32,
) -> bool {
    let mut leases = stream_leases();
    if !leases.reserve(
        stream_address,
        stream_heap,
        publication_address,
        [0; STREAM_HEADER_WORDS],
    ) {
        drop(leases);
        return retain_orphan_rollback(stream_address, stream_heap);
    }
    if leases.retain_rollback(stream_address) {
        true
    } else {
        let _ = leases.cancel_pending(stream_address);
        drop(leases);
        retain_orphan_rollback(stream_address, stream_heap)
    }
}

/// Resolve an aborted stream creation in the Rust lease registry.
///
/// Returns true when C may release its transient node. A false result means
/// the caller must preserve that node as an untracked rollback record.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_stream_create_abort_rust(
    stream_address: u32,
    stream_heap: u32,
    publication_address: u32,
    lease_reserved: bool,
    routing_reserved: bool,
) -> bool {
    #[cfg(not(feature = "rust-audio-host"))]
    let _ = routing_reserved;
    // Preserve the adapter's rollback order: cancel the optional routing
    // sidecar before attempting private heap cleanup and the lease transition.
    #[cfg(feature = "rust-audio-host")]
    if routing_reserved {
        let _ = crate::routing_ffi::tsfp_audio_stream_routing_cancel_rust(stream_address);
    }
    #[cfg(feature = "rust-audio-host")]
    let heap_cleaned = stream_heap == 0
        || !crate::device_ffi::tsfp_audio_stream_heap_valid_rust(stream_heap)
        || crate::device_ffi::tsfp_audio_stream_heap_destroy_rust(stream_heap);
    #[cfg(not(feature = "rust-audio-host"))]
    let heap_cleaned = stream_heap == 0;
    let resolved = resolve_stream_create_abort(
        stream_address,
        stream_heap,
        publication_address,
        lease_reserved,
        heap_cleaned,
    );
    resolved || retain_orphan_rollback(stream_address, stream_heap)
}

fn resolve_stream_create_abort(
    stream_address: u32,
    stream_heap: u32,
    publication_address: u32,
    lease_reserved: bool,
    heap_cleaned: bool,
) -> bool {
    let mut leases = stream_leases();
    if lease_reserved {
        if heap_cleaned {
            leases.cancel_pending(stream_address)
        } else {
            leases.retain_rollback(stream_address)
        }
    } else if heap_cleaned {
        true
    } else if leases.reserve(
        stream_address,
        stream_heap,
        publication_address,
        [0; STREAM_HEADER_WORDS],
    ) {
        if leases.retain_rollback(stream_address) {
            true
        } else {
            let _ = leases.cancel_pending(stream_address);
            false
        }
    } else {
        false
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_stream_lease_detach_rust(
    stream_address: u32,
    lease: *const CDeviceLease,
) -> bool {
    if lease.is_null() {
        return false;
    }
    // SAFETY: the C adapter supplies a live lease record for this synchronous call.
    let lease: DeviceLeaseIdentity = unsafe { (*lease).into() };
    stream_leases().detach(stream_address, lease)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_stream_lease_begin_retire_detached_rust(
    stream_address: u32,
    lease: *const CDeviceLease,
) -> bool {
    if lease.is_null() {
        return false;
    }
    // SAFETY: the C adapter supplies a live lease record for this synchronous call.
    let lease: DeviceLeaseIdentity = unsafe { (*lease).into() };
    stream_leases().begin_retire_detached(stream_address, lease)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_stream_lease_restore_detached_rust(
    stream_address: u32,
    lease: *const CDeviceLease,
) -> bool {
    if lease.is_null() {
        return false;
    }
    // SAFETY: the C adapter supplies a live lease record for this synchronous call.
    let lease: DeviceLeaseIdentity = unsafe { (*lease).into() };
    stream_leases().restore_detached(stream_address, lease)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_stream_lease_retire_detached_rust(
    stream_address: u32,
    lease: *const CDeviceLease,
) -> bool {
    if lease.is_null() {
        return false;
    }
    // SAFETY: the C adapter supplies a live lease record for this synchronous call.
    let lease: DeviceLeaseIdentity = unsafe { (*lease).into() };
    stream_leases().retire_detached(stream_address, lease)
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_stream_lease_begin_retire_rollback_rust(stream_address: u32) -> bool {
    stream_leases().begin_retire_rollback(stream_address)
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_stream_lease_restore_rollback_rust(stream_address: u32) -> bool {
    stream_leases().restore_rollback(stream_address)
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_stream_lease_retire_rollback_rust(stream_address: u32) -> bool {
    stream_leases().retire_rollback(stream_address)
}

/// Return the newest active stream in the same order as the legacy C head list.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_stream_lease_first_active_rust(
    stream_address_out: *mut u32,
) -> bool {
    if stream_address_out.is_null() {
        return false;
    }
    let Some(stream_address) = stream_leases().first_active() else {
        return false;
    };
    // SAFETY: the output pointer was checked and belongs to the synchronous C caller.
    unsafe { *stream_address_out = stream_address };
    true
}

/// Return the next detached payload to clean up in legacy C-list order.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_stream_lease_first_detached_rust(
    stream_address_out: *mut u32,
) -> bool {
    if stream_address_out.is_null() {
        return false;
    }
    let Some(stream_address) = stream_leases().first_detached() else {
        return false;
    };
    // SAFETY: the output pointer was checked and belongs to the synchronous C caller.
    unsafe { *stream_address_out = stream_address };
    true
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_stream_lease_is_active_rust(stream_address: u32) -> bool {
    stream_leases().is_active(stream_address)
}

/// Return one Rust-owned failed-create allocation for the serialized cleanup loop.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_stream_lease_first_rollback_rust(
    stream_address_out: *mut u32,
    stream_heap_out: *mut u32,
) -> bool {
    if stream_address_out.is_null() || stream_heap_out.is_null() {
        return false;
    }
    let Some((stream_address, stream_heap)) = stream_leases().first_rollback() else {
        return false;
    };
    // SAFETY: both output pointers were checked and are owned by the synchronous C caller.
    unsafe {
        *stream_address_out = stream_address;
        *stream_heap_out = stream_heap;
    }
    true
}

/// Clean Rust-owned failed-create records in registry order. Status 2 identifies an invariant failure.
#[cfg(feature = "rust-audio-host")]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_stream_lease_cleanup_rollbacks_rust() -> u32 {
    let records = stream_leases().cleanup_rollback_records(
        |heap| crate::device_ffi::tsfp_audio_stream_heap_valid_rust(heap),
        |heap| crate::device_ffi::tsfp_audio_stream_heap_destroy_rust(heap),
    );
    match records {
        Err(RollbackCleanupFailure::Refused) => return 1,
        Err(RollbackCleanupFailure::RetireInvariant) => return 2,
        Ok(()) => {}
    }
    loop {
        let Some(record) = orphan_rollbacks().first() else {
            return 0;
        };
        let cleaned = !crate::device_ffi::tsfp_audio_stream_heap_valid_rust(record.stream_heap)
            || crate::device_ffi::tsfp_audio_stream_heap_destroy_rust(record.stream_heap);
        if !cleaned || !orphan_rollbacks().remove(record) {
            return 1;
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_stream_lease_overlaps_guest_span_rust(
    address: u32,
    bytes: u32,
) -> bool {
    stream_leases().overlaps_guest_span(address, bytes)
        || orphan_rollbacks().overlaps_guest_span(address, bytes)
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_stream_lease_publication_address_rust(
    stream_address: u32,
    output: *mut u32,
) -> bool {
    if output.is_null() {
        return false;
    }
    let leases = stream_leases();
    let Some(record) = leases.record(stream_address) else {
        return false;
    };
    unsafe { *output = record.publication_address };
    true
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_stream_lease_overlaps_i3dl2_span_rust(
    address: u32,
    bytes: u32,
) -> bool {
    stream_leases().overlaps_i3dl2_span(address, bytes)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_stream_lease_registered_rust(
    stream_address: u32,
    stream_heap: u32,
    header: *const u32,
    lease: *const CDeviceLease,
) -> bool {
    if lease.is_null() || header.is_null() {
        return false;
    }
    // SAFETY: the C adapter supplies live synchronous lease and ten-word header records.
    let (lease, header): (DeviceLeaseIdentity, [u32; STREAM_HEADER_WORDS]) = unsafe {
        (
            (*lease).into(),
            std::slice::from_raw_parts(header, STREAM_HEADER_WORDS)
                .try_into()
                .expect("fixed stream header length"),
        )
    };
    stream_leases().owns_live_stream(&StreamLeaseHeader {
        lease,
        stream_address,
        stream_heap,
        header,
    })
}

/// Copy the registry's active, detached, and rollback guest spans into a caller buffer.
/// The C adapter holds its stream lock across sizing and fill, so lifecycle membership
/// cannot change between the two calls.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_stream_lease_guest_spans_rust(
    spans: *mut StreamLeaseGuestSpan,
    capacity: u32,
    count_out: *mut u32,
) -> bool {
    if count_out.is_null() {
        return false;
    }
    let registry = stream_leases();
    let Some(count) = registry.guest_span_count() else {
        return false;
    };
    let Ok(count) = u32::try_from(count) else {
        return false;
    };
    // SAFETY: count_out was checked non-null and is synchronously owned by the C caller.
    unsafe { *count_out = count };
    if spans.is_null() {
        return capacity == 0;
    }
    if capacity < count {
        return false;
    }
    if count == 0 {
        return true;
    }
    // SAFETY: the C adapter provides count writable 8-byte slots for this call.
    let output = unsafe { std::slice::from_raw_parts_mut(spans, count as usize) };
    registry.append_guest_spans(output) == Some(count as usize)
}

/// Compare ownership identity fields without interpreting the lease as a capability.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_stream_lease_identity_matches_rust(
    left: *const CDeviceLease,
    right: *const CDeviceLease,
) -> bool {
    if left.is_null() || right.is_null() {
        return false;
    }
    // SAFETY: the C adapter supplies live lease records for this synchronous comparison.
    let (left, right): (DeviceLeaseIdentity, DeviceLeaseIdentity) =
        unsafe { ((*left).into(), (*right).into()) };
    left == right
}

/// Copy the canonical identity of an active or detached stream lease from Rust state.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_stream_lease_identity_rust(
    stream_address: u32,
    identity_out: *mut CDeviceLease,
) -> bool {
    if identity_out.is_null() {
        return false;
    }
    let Some(identity) = stream_leases().live_identity(stream_address) else {
        return false;
    };
    // SAFETY: checked non-null and the C adapter owns this synchronous output slot.
    unsafe {
        *identity_out = CDeviceLease {
            device_heap: identity.device_heap,
            internal_address: identity.internal_address,
            serial: identity.serial,
        }
    };
    true
}

/// Copy the canonical header of an exact active or detached stream lease.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_stream_lease_header_rust(
    stream_address: u32,
    identity: *const CDeviceLease,
    header_out: *mut u32,
) -> bool {
    if stream_address == 0 || identity.is_null() || header_out.is_null() {
        return false;
    }
    // SAFETY: the adapter supplies a live identity for this synchronous call.
    let identity = DeviceLeaseIdentity::from(unsafe { *identity });
    let leases = stream_leases();
    if !leases.owns_live_lease(stream_address, identity) {
        return false;
    }
    let Some(stream) = leases.validation_header(stream_address) else {
        return false;
    };
    if stream.lease != identity {
        return false;
    }
    // SAFETY: the caller provides room for the fixed ten-word C stream header.
    unsafe {
        std::ptr::copy_nonoverlapping(stream.header.as_ptr(), header_out, STREAM_HEADER_WORDS);
    }
    true
}

/// Copy the heap recorded by the active or detached Rust stream lease.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_stream_lease_heap_rust(
    stream_address: u32,
    heap_out: *mut u32,
) -> bool {
    if heap_out.is_null() {
        return false;
    }
    let leases = stream_leases();
    let Some(record) = leases.record(stream_address) else {
        return false;
    };
    // SAFETY: checked non-null output is live for this synchronous FFI call.
    unsafe { heap_out.write(record.stream_heap) };
    true
}

pub type StreamLeaseProbe = unsafe extern "C" fn(
    context: *mut c_void,
    stage: u32,
    heap: u32,
    address: u32,
    block_bytes: *mut u32,
) -> bool;

struct FfiProbes {
    context: *mut c_void,
    probe: StreamLeaseProbe,
    guest_memory: FrameGuestMemory,
}

impl FfiProbes {
    unsafe fn call(&mut self, stage: u32, heap: u32, address: u32, block_bytes: *mut u32) -> bool {
        // SAFETY: the callback and context are supplied by the synchronous C adapter call.
        unsafe { (self.probe)(self.context, stage, heap, address, block_bytes) }
    }
}

impl StreamLeaseProbes for FfiProbes {
    fn device_heap_valid(&mut self, heap: u32) -> bool {
        #[cfg(feature = "rust-kmem")]
        // SAFETY: kmem's process-wide backend selector takes no pointers and is linked with this feature.
        if unsafe { tsfp_kmem_audio_heap_active() } {
            // SAFETY: the handle is an opaque guest heap ID, not a host pointer.
            return unsafe { tsfp_kmem_audio_heap_valid(heap) };
        }
        // SAFETY: this probe stage has no output pointers and runs synchronously.
        unsafe { self.call(PROBE_DEVICE_HEAP, heap, 0, std::ptr::null_mut()) }
    }

    fn read_header(&mut self, address: u32) -> Option<[u32; STREAM_HEADER_WORDS]> {
        let bytes = self
            .guest_memory
            .read_bytes(address, STREAM_HEADER_WORDS * std::mem::size_of::<u32>())?;
        if bytes.len() != STREAM_HEADER_WORDS * std::mem::size_of::<u32>() {
            return None;
        }
        let mut header = [0; STREAM_HEADER_WORDS];
        for (word, bytes) in header.iter_mut().zip(bytes.chunks_exact(4)) {
            *word = u32::from_le_bytes(bytes.try_into().ok()?);
        }
        Some(header)
    }

    fn stream_heap_valid(&mut self, heap: u32) -> bool {
        #[cfg(feature = "rust-kmem")]
        // SAFETY: kmem's process-wide backend selector takes no pointers and is linked with this feature.
        if unsafe { tsfp_kmem_audio_heap_active() } {
            // SAFETY: the handle is an opaque guest heap ID, not a host pointer.
            return unsafe { tsfp_kmem_audio_heap_valid(heap) };
        }
        // SAFETY: this probe stage has no output pointers and runs synchronously.
        unsafe { self.call(PROBE_STREAM_HEAP, heap, 0, std::ptr::null_mut()) }
    }

    fn stream_block_size(&mut self, heap: u32, address: u32) -> Option<u32> {
        let mut bytes = 0;
        #[cfg(feature = "rust-kmem")]
        // SAFETY: kmem's process-wide backend selector takes no pointers and is linked with this feature.
        if unsafe { tsfp_kmem_audio_heap_active() } {
            // SAFETY: bytes is a live writable word for this synchronous ABI call.
            return unsafe { tsfp_kmem_audio_heap_block_size(heap, address, &mut bytes) }
                .then_some(bytes);
        }
        // SAFETY: the local byte count remains writable for this synchronous callback.
        unsafe { self.call(PROBE_STREAM_BLOCK_SIZE, heap, address, &mut bytes) }.then_some(bytes)
    }
}

/// Run the stream's ordered typed validation, reading the guest header through Rust's
/// fault-safe process_vm boundary and delegating heap probes to C.
/// The pointers are borrowed only for this call; no guest address crosses as a Rust reference.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_stream_lease_validate_rust(
    owned_lease: *const CDeviceLease,
    identity: *const CDeviceLease,
    required_internal_address: u32,
    stream_address: u32,
    stream_heap: u32,
    expected_header: *const u32,
    context: *mut c_void,
    probe: Option<StreamLeaseProbe>,
) -> bool {
    if owned_lease.is_null() || identity.is_null() || expected_header.is_null() {
        return false;
    }
    let Some(probe) = probe else {
        return false;
    };

    // SAFETY: the C adapter passes live synchronous records and a ten-word header snapshot.
    let (owned_lease, identity, expected_header) = unsafe {
        (
            *owned_lease,
            *identity,
            std::slice::from_raw_parts(expected_header, STREAM_HEADER_WORDS)
                .try_into()
                .expect("fixed stream header length"),
        )
    };
    let stream = StreamLeaseHeader {
        lease: owned_lease.into(),
        stream_address,
        stream_heap,
        header: expected_header,
    };
    let identity = DeviceLeaseIdentity::from(identity);
    let mut probes = FfiProbes {
        context,
        probe,
        guest_memory: FrameGuestMemory::new(std::process::id() as i32),
    };
    validate_stream_lease(
        Some(&stream),
        &identity,
        required_internal_address,
        &mut probes,
    )
}

/// Validate an active or detached stream using identity copied from the Rust-owned record.
/// C supplies only the current device identity and heap-probe service, never mirrored header data.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_stream_lease_validate_record_rust(
    stream_address: u32,
    identity: *const CDeviceLease,
    required_internal_address: u32,
    context: *mut c_void,
    probe: Option<StreamLeaseProbe>,
) -> bool {
    if stream_address == 0 || identity.is_null() {
        return false;
    }
    let Some(probe) = probe else {
        return false;
    };
    // SAFETY: the caller provides a live identity record for this synchronous validation.
    let identity = DeviceLeaseIdentity::from(unsafe { *identity });
    let Some(stream) = stream_leases().validation_header(stream_address) else {
        return false;
    };
    let mut probes = FfiProbes {
        context,
        probe,
        guest_memory: FrameGuestMemory::new(std::process::id() as i32),
    };
    validate_stream_lease(
        Some(&stream),
        &identity,
        required_internal_address,
        &mut probes,
    )
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "rust-kmem")]
    use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

    use super::*;

    // These FFI tests share the process-wide registry. Serialize their multi-call
    // lifecycle assertions just as the host adapter serializes them with its stream lock.
    static STREAM_LEASE_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn lease_scope(descriptor_address: u32, format_address: u32) -> StreamLeaseScope {
        let mut descriptor = [0; tsfp_audio::scope::DESCRIPTOR_BYTES];
        descriptor[4..8].copy_from_slice(&3u32.to_le_bytes());
        descriptor[8..12].copy_from_slice(&format_address.to_le_bytes());
        let mut format = [0; tsfp_audio::scope::FORMAT_BYTES];
        format[0..2].copy_from_slice(&0x69u16.to_le_bytes());
        format[2..4].copy_from_slice(&2u16.to_le_bytes());
        format[4..8].copy_from_slice(&44_100u32.to_le_bytes());
        format[8..12].copy_from_slice(&49_612u32.to_le_bytes());
        format[12..14].copy_from_slice(&72u16.to_le_bytes());
        format[14..16].copy_from_slice(&4u16.to_le_bytes());
        format[16..18].copy_from_slice(&2u16.to_le_bytes());
        format[18..20].copy_from_slice(&64u16.to_le_bytes());
        StreamLeaseScope {
            descriptor_address,
            format_address,
            flags: 0,
            max_packets: 3,
            sample_rate: 44_100,
            average_bytes_per_second: 49_612,
            channels: 2,
            block_align: 72,
            descriptor,
            format,
        }
    }

    #[cfg(feature = "rust-kmem")]
    static RUST_HEAP_ACTIVE: AtomicBool = AtomicBool::new(false);
    #[cfg(feature = "rust-kmem")]
    static RUST_HEAP_VALID_CALLS: AtomicU32 = AtomicU32::new(0);
    #[cfg(feature = "rust-kmem")]
    static RUST_HEAP_BLOCK_SIZE_CALLS: AtomicU32 = AtomicU32::new(0);

    #[cfg(feature = "rust-kmem")]
    #[unsafe(no_mangle)]
    extern "C" fn tsfp_kmem_audio_heap_active() -> bool {
        RUST_HEAP_ACTIVE.load(Ordering::SeqCst)
    }

    #[cfg(feature = "rust-kmem")]
    #[unsafe(no_mangle)]
    extern "C" fn tsfp_kmem_audio_heap_valid(_heap: u32) -> bool {
        RUST_HEAP_VALID_CALLS.fetch_add(1, Ordering::SeqCst);
        true
    }

    #[cfg(feature = "rust-kmem")]
    #[unsafe(no_mangle)]
    unsafe extern "C" fn tsfp_kmem_audio_heap_block_size(
        _heap: u32,
        _address: u32,
        size: *mut u32,
    ) -> bool {
        if size.is_null() {
            return false;
        }
        RUST_HEAP_BLOCK_SIZE_CALLS.fetch_add(1, Ordering::SeqCst);
        // SAFETY: the test passes a live output word from FfiProbes.
        unsafe { size.write(40) };
        true
    }
    use std::vec::Vec;

    struct ProbeFixture {
        calls: Vec<u32>,
        block_bytes: u32,
        device_heap_valid: bool,
        stream_heap_valid: bool,
    }

    unsafe extern "C" fn test_probe(
        context: *mut c_void,
        stage: u32,
        _heap: u32,
        _address: u32,
        block_bytes: *mut u32,
    ) -> bool {
        // SAFETY: tests pass a unique pointer to a live ProbeFixture for this synchronous call.
        let state = unsafe { &mut *context.cast::<ProbeFixture>() };
        state.calls.push(stage);
        match stage {
            PROBE_DEVICE_HEAP => state.device_heap_valid,
            PROBE_STREAM_HEAP => state.stream_heap_valid,
            PROBE_STREAM_BLOCK_SIZE => {
                if block_bytes.is_null() {
                    return false;
                }
                // SAFETY: the caller supplies a valid output word for this synchronous call.
                unsafe { *block_bytes = state.block_bytes };
                true
            }
            _ => false,
        }
    }

    #[test]
    fn c_abi_owns_ordered_lease_and_header_validation() {
        let _guard = STREAM_LEASE_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        assert_eq!(std::mem::size_of::<CDeviceLease>(), 16);
        let lease = CDeviceLease {
            device_heap: 7,
            internal_address: 0x1234,
            serial: 0x1_0000_0002,
        };
        let header = [1, 2, 3, lease.internal_address, 0, 0, 0, 0, 0, 0];
        let (page, stream_address) = crate::test_guest_memory::map_guest_page();
        // SAFETY: this test owns a mapped writable page large enough for the ten-word header.
        unsafe {
            std::ptr::copy_nonoverlapping(header.as_ptr(), page.cast::<u32>(), STREAM_HEADER_WORDS)
        };
        let mut fixture = ProbeFixture {
            calls: Vec::new(),
            block_bytes: 40,
            device_heap_valid: true,
            stream_heap_valid: true,
        };
        // SAFETY: all record/header/context pointers are live for the synchronous FFI call.
        assert!(unsafe {
            tsfp_audio_stream_lease_validate_rust(
                &lease,
                &lease,
                lease.internal_address,
                stream_address,
                8,
                header.as_ptr(),
                (&mut fixture as *mut ProbeFixture).cast(),
                Some(test_probe),
            )
        });
        assert_eq!(
            fixture.calls,
            [
                PROBE_DEVICE_HEAP,
                PROBE_STREAM_HEAP,
                PROBE_STREAM_BLOCK_SIZE,
            ]
        );

        let scope = lease_scope(0xf000_6000, 0xf000_7000);
        // SAFETY: the mapped stream header, scope, and lease remain live for these calls.
        assert!(unsafe {
            tsfp_audio_stream_lease_reserve_rust(
                stream_address,
                8,
                0xf000_5000,
                header.as_ptr(),
                &scope,
            )
        });
        assert!(unsafe { tsfp_audio_stream_lease_bind_rust(stream_address, &lease) });
        let mut canonical_identity = CDeviceLease {
            device_heap: 0,
            internal_address: 0,
            serial: 0,
        };
        // SAFETY: the bound stream is active and the identity output is exclusively borrowed.
        assert!(unsafe {
            tsfp_audio_stream_lease_identity_rust(stream_address, &mut canonical_identity)
        });
        assert_eq!(canonical_identity, lease);
        let mut canonical_heap = 0;
        // SAFETY: the stream is active and the heap output is exclusively borrowed.
        assert!(unsafe { tsfp_audio_stream_lease_heap_rust(stream_address, &mut canonical_heap) });
        assert_eq!(canonical_heap, 8);
        let mut canonical_header = [0; STREAM_HEADER_WORDS];
        // SAFETY: the header output has the fixed ABI length and the lease is active.
        assert!(unsafe {
            tsfp_audio_stream_lease_header_rust(
                stream_address,
                &canonical_identity,
                canonical_header.as_mut_ptr(),
            )
        });
        assert_eq!(canonical_header, header);
        let mut stale_header = [0; STREAM_HEADER_WORDS];
        let mut stale_identity = canonical_identity;
        stale_identity.serial += 1;
        // SAFETY: the stale identity and output remain live for the synchronous call.
        assert!(!unsafe {
            tsfp_audio_stream_lease_header_rust(
                stream_address,
                &stale_identity,
                stale_header.as_mut_ptr(),
            )
        });
        let mut publication_address = 0;
        assert!(tsfp_audio_stream_lease_publication_address_rust(
            stream_address,
            &mut publication_address,
        ));
        assert_eq!(publication_address, 0xf000_5000);
        assert!(!tsfp_audio_stream_lease_publication_address_rust(
            stream_address,
            std::ptr::null_mut(),
        ));
        assert!(!unsafe {
            tsfp_audio_stream_lease_identity_rust(stream_address, std::ptr::null_mut())
        });
        fixture.calls.clear();
        // The record-backed entry point must use Rust's stored header and heap, not C copies.
        // SAFETY: identity and probe context are live for this synchronous validation.
        assert!(unsafe {
            tsfp_audio_stream_lease_validate_record_rust(
                stream_address,
                &lease,
                lease.internal_address,
                (&mut fixture as *mut ProbeFixture).cast(),
                Some(test_probe),
            )
        });
        assert_eq!(
            fixture.calls,
            [
                PROBE_DEVICE_HEAP,
                PROBE_STREAM_HEAP,
                PROBE_STREAM_BLOCK_SIZE
            ]
        );
        let mut stale_record_identity = lease;
        stale_record_identity.serial += 1;
        fixture.calls.clear();
        // SAFETY: pointers remain live; a stale generation must be rejected before probing.
        assert!(!unsafe {
            tsfp_audio_stream_lease_validate_record_rust(
                stream_address,
                &stale_record_identity,
                stale_record_identity.internal_address,
                (&mut fixture as *mut ProbeFixture).cast(),
                Some(test_probe),
            )
        });
        assert!(fixture.calls.is_empty());
        assert!(unsafe { tsfp_audio_stream_lease_detach_rust(stream_address, &lease) });
        assert!(unsafe {
            tsfp_audio_stream_lease_begin_retire_detached_rust(stream_address, &lease)
        });
        assert!(unsafe { tsfp_audio_stream_lease_retire_detached_rust(stream_address, &lease) });
        assert!(!unsafe {
            tsfp_audio_stream_lease_identity_rust(stream_address, &mut canonical_identity)
        });
        assert!(!tsfp_audio_stream_lease_publication_address_rust(
            stream_address,
            &mut publication_address,
        ));

        #[cfg(feature = "rust-kmem")]
        {
            fixture.calls.clear();
            RUST_HEAP_VALID_CALLS.store(0, Ordering::SeqCst);
            RUST_HEAP_BLOCK_SIZE_CALLS.store(0, Ordering::SeqCst);
            RUST_HEAP_ACTIVE.store(true, Ordering::SeqCst);
            // SAFETY: all lease records, expected header, page, and fixture remain live.
            assert!(unsafe {
                tsfp_audio_stream_lease_validate_rust(
                    &lease,
                    &lease,
                    lease.internal_address,
                    stream_address,
                    8,
                    header.as_ptr(),
                    (&mut fixture as *mut ProbeFixture).cast(),
                    Some(test_probe),
                )
            });
            assert!(fixture.calls.is_empty());
            assert_eq!(RUST_HEAP_VALID_CALLS.load(Ordering::SeqCst), 2);
            assert_eq!(RUST_HEAP_BLOCK_SIZE_CALLS.load(Ordering::SeqCst), 1);
        }

        fixture.calls.clear();
        // An unreadable header refuses after device-heap validation and before stream-heap probes.
        // SAFETY: all records and context are live; this address is outside the mapped test page.
        assert!(!unsafe {
            tsfp_audio_stream_lease_validate_rust(
                &lease,
                &lease,
                lease.internal_address,
                u32::MAX - 7,
                8,
                header.as_ptr(),
                (&mut fixture as *mut ProbeFixture).cast(),
                Some(test_probe),
            )
        });
        #[cfg(feature = "rust-kmem")]
        {
            assert!(fixture.calls.is_empty());
            assert_eq!(RUST_HEAP_VALID_CALLS.load(Ordering::SeqCst), 3);
            assert_eq!(RUST_HEAP_BLOCK_SIZE_CALLS.load(Ordering::SeqCst), 1);
            RUST_HEAP_ACTIVE.store(false, Ordering::SeqCst);
        }
        #[cfg(not(feature = "rust-kmem"))]
        assert_eq!(fixture.calls, [PROBE_DEVICE_HEAP]);

        let mut stale = lease;
        stale.serial += 1;
        fixture.calls.clear();
        // SAFETY: pointers remain valid; the stale serial should be rejected before probing.
        assert!(!unsafe {
            tsfp_audio_stream_lease_validate_rust(
                &lease,
                &stale,
                stale.internal_address,
                stream_address,
                8,
                header.as_ptr(),
                (&mut fixture as *mut ProbeFixture).cast(),
                Some(test_probe),
            )
        });
        assert!(fixture.calls.is_empty());

        // SAFETY: both lease records are live for this synchronous identity comparison.
        assert!(unsafe { tsfp_audio_stream_lease_identity_matches_rust(&lease, &lease) });
        assert!(!unsafe { tsfp_audio_stream_lease_identity_matches_rust(&lease, &stale) });
        crate::test_guest_memory::unmap_guest_page(page);
    }

    #[test]
    fn ffi_registry_commits_detaches_and_retires_exact_generations() {
        let _guard = STREAM_LEASE_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let address = 0xf001_1000;
        let heap = 0xf001_2000;
        let lease = CDeviceLease {
            device_heap: 0xf001_3000,
            internal_address: 0xf001_4000,
            serial: 0x1_0000_0002,
        };
        let header = [1, 2, 3, lease.internal_address, 0, 0, 0, 0, 0, 0];
        let scope = lease_scope(0xf001_6000, 0xf001_7000);

        // SAFETY: the header, scope and lease stay live for each synchronous FFI call.
        assert!(unsafe {
            tsfp_audio_stream_lease_reserve_rust(
                address,
                heap,
                0xf001_5000,
                header.as_ptr(),
                &scope,
            )
        });
        assert!(!unsafe {
            tsfp_audio_stream_lease_registered_rust(address, heap, header.as_ptr(), &lease)
        });
        assert!(unsafe { tsfp_audio_stream_lease_bind_rust(address, &lease) });
        let mut lifecycle_address = 0u32;
        // SAFETY: the output slot is live for this synchronous FFI call.
        assert!(unsafe { tsfp_audio_stream_lease_first_active_rust(&mut lifecycle_address) });
        assert_eq!(lifecycle_address, address);
        assert!(tsfp_audio_stream_lease_is_active_rust(address));
        assert!(unsafe {
            tsfp_audio_stream_lease_registered_rust(address, heap, header.as_ptr(), &lease)
        });

        let mut stale = lease;
        stale.serial += 1;
        assert!(!unsafe { tsfp_audio_stream_lease_detach_rust(address, &stale) });
        assert!(unsafe { tsfp_audio_stream_lease_detach_rust(address, &lease) });
        // SAFETY: the output slot is live for this synchronous FFI call.
        let has_active =
            unsafe { tsfp_audio_stream_lease_first_active_rust(&mut lifecycle_address) };
        assert!(!has_active || lifecycle_address != address);
        assert!(!tsfp_audio_stream_lease_is_active_rust(address));
        assert!(unsafe { tsfp_audio_stream_lease_first_detached_rust(&mut lifecycle_address) });
        assert_eq!(lifecycle_address, address);
        assert!(unsafe {
            tsfp_audio_stream_lease_registered_rust(address, heap, header.as_ptr(), &lease)
        });
        assert!(!unsafe { tsfp_audio_stream_lease_retire_detached_rust(address, &stale) });
        assert!(unsafe { tsfp_audio_stream_lease_begin_retire_detached_rust(address, &lease) });
        assert!(unsafe { tsfp_audio_stream_lease_retire_detached_rust(address, &lease) });
        // SAFETY: no pointer is retained by the registry after retirement.
        let has_detached =
            unsafe { tsfp_audio_stream_lease_first_detached_rust(&mut lifecycle_address) };
        assert!(!has_detached || lifecycle_address != address);
        assert!(!unsafe {
            tsfp_audio_stream_lease_registered_rust(address, heap, header.as_ptr(), &lease)
        });
    }

    #[test]
    fn ffi_protected_spans_follow_rust_lease_lifecycle() {
        let _guard = STREAM_LEASE_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let address = 0xf002_1000;
        let heap = 0xf002_2000;
        let publication = 0xf002_3000;
        let lease = CDeviceLease {
            device_heap: 0xf002_4000,
            internal_address: 0xf002_5000,
            serial: 0x2_0000_0003,
        };
        let header = [1, 2, 3, lease.internal_address, 0, 0, 0, 0, 0, 0];
        let scope = lease_scope(0xf002_6000, 0xf002_7000);
        let mut count = u32::MAX;
        let mut spans = [StreamLeaseGuestSpan::default(); 4];

        // SAFETY: pointers refer to live data for each synchronous FFI call.
        assert!(unsafe {
            tsfp_audio_stream_lease_reserve_rust(
                address,
                heap,
                publication,
                header.as_ptr(),
                &scope,
            )
        });
        assert!(unsafe {
            tsfp_audio_stream_lease_guest_spans_rust(std::ptr::null_mut(), 0, &mut count)
        });
        assert_eq!(count, 0);
        assert!(unsafe { tsfp_audio_stream_lease_bind_rust(address, &lease) });
        assert!(unsafe {
            tsfp_audio_stream_lease_guest_spans_rust(spans.as_mut_ptr(), 4, &mut count)
        });
        // Create snapshots descriptor/format bytes; only Rust-owned stream state
        // remains protected after the lease becomes active.
        assert_eq!(count, 2);
        assert_eq!(
            spans[0],
            StreamLeaseGuestSpan {
                address,
                bytes: tsfp_audio::stream_lease::STREAM_ALLOCATION_BYTES,
            }
        );
        assert_eq!(
            spans[1],
            StreamLeaseGuestSpan {
                address: publication,
                bytes: 4
            }
        );
        let mut copied_scope = StreamLeaseScope {
            descriptor_address: 0,
            format_address: 0,
            flags: 0,
            max_packets: 0,
            sample_rate: 0,
            average_bytes_per_second: 0,
            channels: 0,
            block_align: 0,
            descriptor: [0; tsfp_audio::scope::DESCRIPTOR_BYTES],
            format: [0; tsfp_audio::scope::FORMAT_BYTES],
        };
        // SAFETY: lease and output scope remain live for this synchronous query.
        assert!(unsafe { tsfp_audio_stream_lease_scope_rust(address, &lease, &mut copied_scope) });
        assert_eq!(copied_scope, scope);
        let mut stale = lease;
        stale.serial += 1;
        let before_stale_read = copied_scope;
        assert!(!unsafe { tsfp_audio_stream_lease_scope_rust(address, &stale, &mut copied_scope) });
        assert_eq!(copied_scope, before_stale_read);

        assert!(unsafe { tsfp_audio_stream_lease_detach_rust(address, &lease) });
        assert!(unsafe {
            tsfp_audio_stream_lease_guest_spans_rust(spans.as_mut_ptr(), 2, &mut count)
        });
        assert_eq!(count, 1);
        assert_eq!(spans[0].address, address);
        assert!(unsafe { tsfp_audio_stream_lease_begin_retire_detached_rust(address, &lease) });
        assert!(unsafe { tsfp_audio_stream_lease_retire_detached_rust(address, &lease) });
    }

    #[test]
    fn ffi_applies_control_policy_in_rust_for_the_exact_generation() {
        let _guard = STREAM_LEASE_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let address = 0xf006_1000;
        let lease = CDeviceLease {
            device_heap: 0xf006_2000,
            internal_address: 0xf006_3000,
            serial: 0x6_0000_0007,
        };
        let header = [1, 2, 3, lease.internal_address, 0, 0, 0, 0, 0, 0];
        let scope = lease_scope(0xf006_4000, 0xf006_5000);
        // SAFETY: all records remain live across these synchronous FFI calls.
        assert!(unsafe {
            tsfp_audio_stream_lease_reserve_rust(
                address,
                0xf006_6000,
                0xf006_7000,
                header.as_ptr(),
                &scope,
            )
        });
        assert!(unsafe { tsfp_audio_stream_lease_bind_rust(address, &lease) });

        let apply = |request: &StreamControlRequest, identity: &CDeviceLease| unsafe {
            tsfp_audio_stream_lease_control_apply_rust(address, identity, request)
        };
        let pause = StreamControlRequest {
            action: tsfp_audio::stream_lease::STREAM_CONTROL_PAUSE,
            argument: 1,
            ..StreamControlRequest::default()
        };
        assert_eq!(apply(&pause, &lease), STREAM_CONTROL_ACCEPTED);
        let flush = StreamControlRequest {
            action: tsfp_audio::stream_lease::STREAM_CONTROL_FLUSH_EX,
            apply: 1,
            ..StreamControlRequest::default()
        };
        assert_eq!(apply(&flush, &lease), STREAM_CONTROL_ACCEPTED);
        let discontinuity = StreamControlRequest {
            action: tsfp_audio::stream_lease::STREAM_CONTROL_DISCONTINUITY,
            ..StreamControlRequest::default()
        };
        assert_eq!(apply(&discontinuity, &lease), STREAM_CONTROL_ACCEPTED);
        let resume = StreamControlRequest {
            action: tsfp_audio::stream_lease::STREAM_CONTROL_PAUSE,
            completion_available: 1,
            ..StreamControlRequest::default()
        };
        assert_eq!(apply(&resume, &lease), STREAM_CONTROL_ACCEPTED);

        let mut stale = lease;
        stale.serial += 1;
        assert_eq!(
            apply(&pause, &stale),
            tsfp_audio::stream_lease::STREAM_CONTROL_STALE_LEASE
        );
        let mut state = StreamControlState::default();
        assert!(unsafe { tsfp_audio_stream_lease_control_state_rust(address, &lease, &mut state) });
        assert_eq!(state.pause_seen, 1);
        assert_eq!(state.pause_mode, 0);
        assert_eq!(state.flush_seen, 1);
        assert_eq!(state.discontinuity_seen, 1);
        assert_eq!(state.revision, 4);
        assert!(unsafe { tsfp_audio_stream_lease_detach_rust(address, &lease) });
        assert!(unsafe { tsfp_audio_stream_lease_begin_retire_detached_rust(address, &lease) });
        assert!(unsafe { tsfp_audio_stream_lease_retire_detached_rust(address, &lease) });
    }

    #[test]
    fn ffi_plans_stream_volume_without_committing_before_routing() {
        let _guard = STREAM_LEASE_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let address = 0xf007_1000;
        let lease = CDeviceLease {
            device_heap: 0xf007_2000,
            internal_address: 0xf007_3000,
            serial: 0x7_0000_0005,
        };
        let header = [1, 2, 3, lease.internal_address, 0, 0, 0, 0, 0, 0];
        let scope = lease_scope(0xf007_4000, 0xf007_5000);
        // SAFETY: reserve receives live synchronous header/scope snapshots.
        assert!(unsafe {
            tsfp_audio_stream_lease_reserve_rust(
                address,
                0xf007_6000,
                0xf007_7000,
                header.as_ptr(),
                &scope,
            )
        });
        assert!(unsafe { tsfp_audio_stream_lease_bind_rust(address, &lease) });

        let request = StreamVolumeRequest {
            argument: 0xFFFF_D8F0,
            ..StreamVolumeRequest::default()
        };
        let mut planned = StreamVolumeState {
            seen: 99,
            value: 123,
        };
        // SAFETY: all request, lease, and output records live for this synchronous call.
        assert_eq!(
            unsafe {
                tsfp_audio_stream_lease_volume_apply_rust(address, &lease, &request, &mut planned)
            },
            tsfp_audio::stream_lease::STREAM_VOLUME_ACCEPTED
        );
        assert_eq!(
            planned,
            StreamVolumeState {
                seen: 1,
                value: -10_000
            }
        );
        let mut volume = StreamVolumeState::default();
        assert!(unsafe { tsfp_audio_stream_lease_volume_state_rust(address, &lease, &mut volume) });
        assert_eq!(volume, StreamVolumeState::default());

        let missing_format = StreamVolumeRequest {
            count: 1,
            ..request
        };
        assert_eq!(
            unsafe {
                tsfp_audio_stream_lease_volume_apply_rust(
                    address,
                    &lease,
                    &missing_format,
                    &mut planned,
                )
            },
            tsfp_audio::stream_lease::STREAM_VOLUME_NEEDS_FORMAT
        );
        let mut stale = lease;
        stale.serial += 1;
        assert_eq!(
            unsafe {
                tsfp_audio_stream_lease_volume_apply_rust(address, &stale, &request, &mut planned)
            },
            tsfp_audio::stream_lease::STREAM_VOLUME_STALE_LEASE
        );
        assert!(unsafe {
            tsfp_audio_stream_lease_volume_commit_rust(address, &lease, &volume, &planned)
        });
        assert!(unsafe { tsfp_audio_stream_lease_detach_rust(address, &lease) });
        assert!(unsafe { tsfp_audio_stream_lease_begin_retire_detached_rust(address, &lease) });
        assert!(unsafe { tsfp_audio_stream_lease_retire_detached_rust(address, &lease) });
    }

    #[test]
    fn ffi_format_state_reads_and_commits_exact_active_generation() {
        let _guard = STREAM_LEASE_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let address = 0xf004_1000;
        let lease = CDeviceLease {
            device_heap: 0xf004_2000,
            internal_address: 0xf004_3000,
            serial: 0x4_0000_0005,
        };
        let header = [1, 2, 3, lease.internal_address, 0, 0, 0, 0, 0, 0];
        let scope = lease_scope(0xf004_4000, 0xf004_5000);
        // SAFETY: snapshots and leases remain live for synchronous FFI calls.
        assert!(unsafe {
            tsfp_audio_stream_lease_reserve_rust(
                address,
                0xf004_6000,
                0xf004_7000,
                header.as_ptr(),
                &scope,
            )
        });
        assert!(unsafe { tsfp_audio_stream_lease_bind_rust(address, &lease) });

        let mut state = StreamFormatState {
            bytes: [0xa5; tsfp_audio::scope::FORMAT_BYTES],
            sets: 99,
        };
        assert!(unsafe { tsfp_audio_stream_lease_format_state_rust(address, &lease, &mut state) });
        assert_eq!(state, StreamFormatState::default());

        let next_bytes = [0x69; tsfp_audio::scope::FORMAT_BYTES];
        assert!(unsafe {
            tsfp_audio_stream_lease_format_commit_rust(address, &lease, &state, next_bytes.as_ptr())
        });
        let previous = state;
        assert!(!unsafe {
            tsfp_audio_stream_lease_format_commit_rust(
                address,
                &lease,
                &previous,
                [0x11; tsfp_audio::scope::FORMAT_BYTES].as_ptr(),
            )
        });
        assert!(unsafe { tsfp_audio_stream_lease_format_state_rust(address, &lease, &mut state) });
        assert_eq!(state.bytes, next_bytes);
        assert_eq!(state.sets, 1);

        let mut volume = StreamVolumeState {
            seen: 7,
            value: 1234,
        };
        assert!(unsafe { tsfp_audio_stream_lease_volume_state_rust(address, &lease, &mut volume) });
        assert_eq!(volume, StreamVolumeState::default());
        let accepted_volume = StreamVolumeState {
            seen: 1,
            value: -10_000,
        };
        assert!(unsafe {
            tsfp_audio_stream_lease_volume_commit_rust(address, &lease, &volume, &accepted_volume)
        });
        assert!(!unsafe {
            tsfp_audio_stream_lease_volume_commit_rust(address, &lease, &volume, &accepted_volume)
        });
        assert!(unsafe { tsfp_audio_stream_lease_volume_state_rust(address, &lease, &mut volume) });
        assert_eq!(volume, accepted_volume);

        let mut spatial = StreamSpatialState {
            position_bits: [u32::MAX; 3],
            position_sets: u32::MAX,
            max_distance_seen: u32::MAX,
            revision: u32::MAX,
            ..StreamSpatialState::default()
        };
        assert!(unsafe {
            tsfp_audio_stream_lease_spatial_state_rust(address, &lease, &mut spatial)
        });
        assert_eq!(spatial, StreamSpatialState::default());
        let accepted_i3dl2 = StreamSpatialState {
            startup_cache_mask: 1,
            i3dl2_address: 0xf003_6000,
            i3dl2_words: [0, 0, 0xffff_f448, 0, 0, 0, 0, 0, 0],
            revision: 1,
            ..spatial
        };
        assert!(unsafe {
            tsfp_audio_stream_lease_spatial_commit_rust(address, &lease, &spatial, &accepted_i3dl2)
        });
        let accepted_position = StreamSpatialState {
            position_bits: [0x3f80_0000, 0xbf00_0000, 0x3e80_0000],
            position_sets: 1,
            revision: 2,
            ..accepted_i3dl2
        };
        assert!(unsafe {
            tsfp_audio_stream_lease_spatial_commit_rust(
                address,
                &lease,
                &accepted_i3dl2,
                &accepted_position,
            )
        });
        assert!(!unsafe {
            tsfp_audio_stream_lease_spatial_commit_rust(
                address,
                &lease,
                &spatial,
                &accepted_position,
            )
        });
        let accepted_spatial = StreamSpatialState {
            max_distance_seen: 1,
            max_distance_bits: 0x4120_0000,
            curve_address: 0x4400,
            curve_count: 4,
            revision: 3,
            ..accepted_position
        };
        assert!(unsafe {
            tsfp_audio_stream_lease_spatial_commit_rust(
                address,
                &lease,
                &accepted_position,
                &accepted_spatial,
            )
        });
        assert!(unsafe {
            tsfp_audio_stream_lease_spatial_state_rust(address, &lease, &mut spatial)
        });
        assert_eq!(spatial, accepted_spatial);

        let mut stale = lease;
        stale.serial += 1;
        let before_stale_read = state;
        assert!(!unsafe { tsfp_audio_stream_lease_format_state_rust(address, &stale, &mut state) });
        assert_eq!(state, before_stale_read);
        assert!(!unsafe {
            tsfp_audio_stream_lease_format_commit_rust(address, &stale, &state, next_bytes.as_ptr())
        });
        let before_stale_volume_read = volume;
        assert!(!unsafe {
            tsfp_audio_stream_lease_volume_state_rust(address, &stale, &mut volume)
        });
        assert_eq!(volume, before_stale_volume_read);
        assert!(!unsafe {
            tsfp_audio_stream_lease_volume_commit_rust(
                address,
                &stale,
                &volume,
                &StreamVolumeState {
                    seen: 1,
                    value: -2000,
                },
            )
        });
        let before_stale_spatial_read = spatial;
        assert!(!unsafe {
            tsfp_audio_stream_lease_spatial_state_rust(address, &stale, &mut spatial)
        });
        assert_eq!(spatial, before_stale_spatial_read);
        assert!(!unsafe {
            tsfp_audio_stream_lease_spatial_commit_rust(
                address,
                &stale,
                &spatial,
                &StreamSpatialState {
                    revision: spatial.revision + 1,
                    ..spatial
                },
            )
        });
        assert!(unsafe { tsfp_audio_stream_lease_detach_rust(address, &lease) });
        assert!(unsafe { tsfp_audio_stream_lease_begin_retire_detached_rust(address, &lease) });
        assert!(unsafe { tsfp_audio_stream_lease_retire_detached_rust(address, &lease) });
    }

    #[test]
    fn ffi_startup_spatial_policy_commits_ordered_values_for_lease() {
        let _guard = STREAM_LEASE_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let address = 0xf005_3000;
        let lease = CDeviceLease {
            device_heap: 0xf005_4000,
            internal_address: 0xf005_5000,
            serial: 991,
        };
        let mut scope = lease_scope(0xf005_9000, 0xf005_a000);
        scope.flags = 0x10;
        scope.descriptor[0..4].copy_from_slice(&scope.flags.to_le_bytes());
        scope.format[2..4].copy_from_slice(&1u16.to_le_bytes());
        scope.format[8..12].copy_from_slice(&24_806u32.to_le_bytes());
        scope.format[12..14].copy_from_slice(&36u16.to_le_bytes());
        scope.channels = 1;
        scope.block_align = 36;
        scope.average_bytes_per_second = 24_806;
        let header = [0, 0, 0, lease.internal_address, 0, 0, 0, 0, 0, 0];
        assert!(unsafe {
            tsfp_audio_stream_lease_reserve_rust(
                address,
                0xf005_6000,
                0xf005_7000,
                header.as_ptr(),
                &scope,
            )
        });
        assert!(unsafe { tsfp_audio_stream_lease_bind_rust(address, &lease) });
        let words = [0, 0, 0xffff_f448, 0, 0, 0, 0, 0, 0];
        assert_eq!(
            unsafe {
                tsfp_audio_stream_lease_startup_spatial_rust(
                    address,
                    &lease,
                    1,
                    0xf005_8000,
                    0,
                    words.as_ptr(),
                )
            },
            tsfp_audio::stream_lease::STREAM_SPATIAL_ACCEPTED
        );
        assert_eq!(
            unsafe {
                tsfp_audio_stream_lease_startup_spatial_rust(
                    address,
                    &lease,
                    2,
                    0x3f80_0000,
                    0,
                    std::ptr::null(),
                )
            },
            tsfp_audio::stream_lease::STREAM_SPATIAL_ACCEPTED
        );
        assert_eq!(
            unsafe {
                tsfp_audio_stream_lease_startup_spatial_rust(
                    address,
                    &lease,
                    3,
                    0x004b_914c,
                    4,
                    std::ptr::null(),
                )
            },
            tsfp_audio::stream_lease::STREAM_SPATIAL_ACCEPTED
        );
        assert!(unsafe { tsfp_audio_stream_lease_detach_rust(address, &lease) });
        assert!(unsafe { tsfp_audio_stream_lease_begin_retire_detached_rust(address, &lease) });
        assert!(unsafe { tsfp_audio_stream_lease_retire_detached_rust(address, &lease) });
    }

    fn ffi_control_state_is_exact_generation_and_compare_committed() {
        let _guard = STREAM_LEASE_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let address = 0xf005_1000;
        let lease = CDeviceLease {
            device_heap: 0xf005_2000,
            internal_address: 0xf005_3000,
            serial: 0x5_0000_0006,
        };
        let header = [1, 2, 3, lease.internal_address, 0, 0, 0, 0, 0, 0];
        let scope = lease_scope(0xf005_4000, 0xf005_5000);
        // SAFETY: snapshots and leases remain live for synchronous FFI calls.
        assert!(unsafe {
            tsfp_audio_stream_lease_reserve_rust(
                address,
                0xf005_6000,
                0xf005_7000,
                header.as_ptr(),
                &scope,
            )
        });
        assert!(unsafe { tsfp_audio_stream_lease_bind_rust(address, &lease) });

        assert_eq!(std::mem::size_of::<StreamControlState>(), 32);
        let mut state = StreamControlState::default();
        assert!(unsafe { tsfp_audio_stream_lease_control_state_rust(address, &lease, &mut state) });
        let paused = StreamControlState {
            pause_seen: 1,
            pause_mode: 1,
            revision: 1,
            ..state
        };
        assert!(unsafe {
            tsfp_audio_stream_lease_control_commit_rust(address, &lease, &state, &paused)
        });
        let flushed = StreamControlState {
            flush_seen: 1,
            flush_flags: 1,
            revision: 2,
            ..paused
        };
        assert!(unsafe {
            tsfp_audio_stream_lease_control_commit_rust(address, &lease, &paused, &flushed)
        });
        let started = StreamControlState {
            discontinuity_seen: 1,
            revision: 3,
            ..flushed
        };
        assert!(unsafe {
            tsfp_audio_stream_lease_control_commit_rust(address, &lease, &flushed, &started)
        });

        let mut stale = lease;
        stale.serial += 1;
        let before_stale_read = state;
        assert!(!unsafe {
            tsfp_audio_stream_lease_control_state_rust(address, &stale, &mut state)
        });
        assert_eq!(state, before_stale_read);
        assert!(!unsafe {
            tsfp_audio_stream_lease_control_commit_rust(address, &stale, &started, &started)
        });
        assert!(unsafe { tsfp_audio_stream_lease_detach_rust(address, &lease) });
        assert!(!unsafe {
            tsfp_audio_stream_lease_control_state_rust(address, &lease, &mut state)
        });
        assert!(unsafe { tsfp_audio_stream_lease_begin_retire_detached_rust(address, &lease) });
        assert!(unsafe { tsfp_audio_stream_lease_retire_detached_rust(address, &lease) });
    }

    #[test]
    fn ffi_stream_create_abort_transitions_and_rollback_view() {
        let _guard = STREAM_LEASE_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let address = 0xf003_1000;
        let heap = 0xf003_2000;
        let header = [1, 2, 3, 4, 0, 0, 0, 0, 0, 0];
        let scope = lease_scope(0xf003_4000, 0xf003_5000);
        let mut found_address = u32::MAX;
        let mut found_heap = u32::MAX;

        // SAFETY: the header and output slots stay live across synchronous FFI calls.
        assert!(unsafe {
            tsfp_audio_stream_lease_reserve_rust(
                address,
                heap,
                0xf003_3000,
                header.as_ptr(),
                &scope,
            )
        });
        assert!(resolve_stream_create_abort(
            address,
            heap,
            0xf003_3000,
            true,
            true
        ));
        assert!(unsafe {
            tsfp_audio_stream_lease_reserve_rust(
                address,
                heap,
                0xf003_3000,
                header.as_ptr(),
                &scope,
            )
        });
        assert!(!unsafe {
            tsfp_audio_stream_lease_first_rollback_rust(&mut found_address, &mut found_heap)
        });
        assert!(resolve_stream_create_abort(
            address,
            heap,
            0xf003_3000,
            true,
            false
        ));
        assert!(unsafe {
            tsfp_audio_stream_lease_first_rollback_rust(&mut found_address, &mut found_heap)
        });
        assert_eq!((found_address, found_heap), (address, heap));
        assert!(tsfp_audio_stream_lease_overlaps_guest_span_rust(
            address + 8,
            4
        ));
        assert!(!tsfp_audio_stream_lease_overlaps_guest_span_rust(
            0xf003_3000,
            4
        ));
        assert!(!unsafe {
            tsfp_audio_stream_lease_first_rollback_rust(std::ptr::null_mut(), &mut found_heap)
        });
        assert!(tsfp_audio_stream_lease_begin_retire_rollback_rust(address));
        assert!(!tsfp_audio_stream_lease_overlaps_guest_span_rust(
            address, 4
        ));
        assert!(tsfp_audio_stream_lease_restore_rollback_rust(address));
        assert!(tsfp_audio_stream_lease_begin_retire_rollback_rust(address));
        assert!(tsfp_audio_stream_lease_retire_rollback_rust(address));

        let unreserved_address = 0xf003_6000;
        let unreserved_heap = 0xf003_7000;
        assert!(resolve_stream_create_abort(
            unreserved_address,
            unreserved_heap,
            0xf003_8000,
            false,
            false,
        ));
        assert!(unsafe {
            tsfp_audio_stream_lease_first_rollback_rust(&mut found_address, &mut found_heap)
        });
        assert_eq!(
            (found_address, found_heap),
            (unreserved_address, unreserved_heap)
        );
        assert!(tsfp_audio_stream_lease_begin_retire_rollback_rust(
            unreserved_address
        ));
        assert!(tsfp_audio_stream_lease_retire_rollback_rust(
            unreserved_address
        ));
        // C treats an already-invalid heap as cleaned; no rollback lease remains.
        assert!(resolve_stream_create_abort(
            0xf003_b000,
            0xf003_c000,
            0xf003_d000,
            false,
            true,
        ));
        assert!(tsfp_audio_stream_create_abort_rust(
            0xf003_9000,
            0,
            0xf003_a000,
            false,
            false,
        ));
    }

    #[test]
    fn orphan_rollback_storage_protects_stream_spans() {
        let mut records = OrphanRollbacks::default();
        let record = OrphanRollback {
            stream_address: 0xf100_0000,
            stream_heap: 0xf100_1000,
        };
        assert!(records.retain(record));
        assert!(records.overlaps_guest_span(0xf100_0008, 4));
        assert!(!records.overlaps_guest_span(0xf100_2000, 4));
        assert!(!records.overlaps_guest_span(0xf100_3000, 4));
        assert_eq!(records.first(), Some(record));
        assert!(records.remove(record));
        assert_eq!(records.first(), None);
    }
}
