//! Rust-owned DirectSound stream-create HLE frame callback for the Rust audio host.
use std::alloc::{Layout, alloc_zeroed, dealloc};
use std::ffi::{c_char, c_void};
use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};
use tsfp_audio::adapter::GuestMemory;
use tsfp_audio::device_frames::CallFrame;
use tsfp_audio::frames::{CREATE, FrameCall, decode_frame};
use tsfp_audio::lifecycle::{
    SINGLETON, STREAM_SECONDARY, STREAM_VTABLE, stream_tables_are_stopped,
};
use tsfp_audio::stream_lease::STREAM_HEADER_WORDS;

use crate::frame_guest_memory::FrameGuestMemory;
use crate::stream_lease_ffi::CDeviceLease;
use tsfp_audio::stream_lease::StreamLeaseScope;

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct StreamCreatePrepareResult {
    pub code: u32,
    pub stream_heap: u32,
    pub stream_address: u32,
    pub lease_reserved: u32,
    pub routing_reserved: u32,
}

#[repr(C)]
struct StreamCreateFinalizeContext {
    host_pid: i32,
    stream_address: u32,
    output_address: u32,
    routing_reserved: u32,
    failures: u32,
    announce: u32,
}
const _: () = assert!(std::mem::size_of::<StreamCreateFinalizeContext>() == 24);

#[repr(C)]
struct StreamCreateRequestContext {
    finalize: StreamCreateFinalizeContext,
    scope: StreamLeaseScope,
    output: u32,
    status: u32,
    node: *mut StreamCreateRollbackNode,
    error: *const c_char,
    announce: bool,
    routing_reserved: bool,
    lease_reserved: bool,
}

#[repr(C)]
struct StreamCreateRollbackNode {
    stream_address: u32,
    stream_heap: u32,
    next: *mut StreamCreateRollbackNode,
}

static UNTRACKED_ROLLBACK_HEAD: AtomicPtr<StreamCreateRollbackNode> =
    AtomicPtr::new(std::ptr::null_mut());

#[repr(C)]
struct StreamCreateLeaseChild {
    heap: u32,
    address: u32,
    bytes: u32,
}

const _: () = assert!(std::mem::size_of::<StreamCreateRequestContext>() == 128);

unsafe extern "C" {
    fn tsfp_audio_stream_create_policy_error_host_c() -> *const c_char;
    fn tsfp_audio_stream_create_policy_error_snapshot_host_c() -> *const c_char;
    fn tsfp_audio_stream_create_acquire_host_c(
        device: u32,
        request: *mut c_void,
        status: *mut u32,
        lease: *mut CDeviceLease,
    );
    fn tsfp_audio_stream_create_cleanup_host_c(request: *mut c_void);
    fn tsfp_audio_stream_create_report_host_c(
        request: *const c_void,
        lease: *const CDeviceLease,
        status: u32,
    );
    fn tsfp_audio_stream_create_refuse_host_c(reason: *const c_char) -> !;
}

fn allocate_rollback_node() -> *mut StreamCreateRollbackNode {
    // Keep allocation fallible so Create reports the same OOM result as the C host.
    unsafe { alloc_zeroed(Layout::new::<StreamCreateRollbackNode>()).cast() }
}

unsafe fn release_rollback_node(node: *mut StreamCreateRollbackNode) {
    if !node.is_null() {
        // SAFETY: the node is uniquely owned and came from allocate_rollback_node.
        unsafe { dealloc(node.cast(), Layout::new::<StreamCreateRollbackNode>()) };
    }
}

/// Release the transient rollback node allocated by the Rust Create callback.
/// The C HLE wrapper must not own the lifetime of Rust's request storage.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_stream_create_release_request_rust(pointer: *mut c_void) {
    // SAFETY: only the synchronous Create wrapper releases nodes allocated by this module.
    unsafe { release_rollback_node(pointer.cast()) };
}

fn retain_untracked_rollback(node: *mut StreamCreateRollbackNode) {
    let head = UNTRACKED_ROLLBACK_HEAD.load(Ordering::Acquire);
    // SAFETY: callers hold the shared stream mutex, and the C node remains allocated until
    // cleanup succeeds. The field is the documented intrusive link in the C/Rust ABI.
    unsafe { (*node).next = head };
    UNTRACKED_ROLLBACK_HEAD.store(node, Ordering::Release);
}

unsafe fn drain_untracked_rollback_with(
    mut head: *mut StreamCreateRollbackNode,
    mut cleanup_heap: impl FnMut(u32) -> bool,
    mut release_node: impl FnMut(*mut StreamCreateRollbackNode),
) -> (*mut StreamCreateRollbackNode, u32) {
    while !head.is_null() {
        // SAFETY: the linked nodes are retained until their heap cleanup succeeds.
        let node = unsafe { &*head };
        if !cleanup_heap(node.stream_heap) {
            return (head, 1);
        }
        let next = node.next;
        release_node(head);
        head = next;
    }
    (std::ptr::null_mut(), 0)
}

fn untracked_rollback_overlaps(
    mut node: *mut StreamCreateRollbackNode,
    address: u32,
    bytes: u32,
) -> bool {
    while !node.is_null() {
        // SAFETY: callers hold the stream mutex and retained nodes remain allocated.
        let current = unsafe { &*node };
        if current.stream_address != 0
            && crate::stream_lease_ffi::overlaps(address, bytes, current.stream_address, 40)
        {
            return true;
        }
        node = current.next;
    }
    false
}

/// Retry emergency failed-Create heap cleanup and release its Rust-owned intrusive records.
/// The caller holds the stream mutex, matching the Create-abort insertion boundary.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_stream_create_cleanup_untracked_rust() -> u32 {
    let head = UNTRACKED_ROLLBACK_HEAD.load(Ordering::Acquire);
    // SAFETY: the caller serializes this list with the stream mutex; heap services and free
    // are the same host operations used by the ordinary Rust Create cleanup path.
    let (remaining, status) = unsafe {
        drain_untracked_rollback_with(
            head,
            |heap| {
                !crate::device_ffi::tsfp_audio_stream_heap_valid_rust(heap)
                    || crate::device_ffi::tsfp_audio_stream_heap_destroy_rust(heap)
            },
            |node| unsafe { release_rollback_node(node) },
        )
    };
    UNTRACKED_ROLLBACK_HEAD.store(remaining, Ordering::Release);
    status
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_stream_create_untracked_overlaps_rust(
    address: u32,
    bytes: u32,
) -> bool {
    untracked_rollback_overlaps(
        UNTRACKED_ROLLBACK_HEAD.load(Ordering::Acquire),
        address,
        bytes,
    )
}

fn prepare_error(code: u32) -> *const c_char {
    match code {
        4 => std::ptr::null(),
        1 => c"original vtable unreadable or contains an unprotected target".as_ptr(),
        2 => c"unproven output/input alias with owned state".as_ptr(),
        3 => c"output is not writable".as_ptr(),
        5 | 6 => c"new private stream allocation/alias is invalid".as_ptr(),
        7 => c"Rust stream lease lifecycle reservation failed".as_ptr(),
        8 => c"Rust routing sidecar reservation failed".as_ptr(),
        _ => c"Rust stream-create preparation refused".as_ptr(),
    }
}

fn rollback_error(lease_reserved: bool) -> *const c_char {
    if lease_reserved {
        c"Rust stream lease abort transition failed".as_ptr()
    } else {
        c"private allocation rollback failed before Rust reservation".as_ptr()
    }
}

/// Run Rust-owned Create preparation directly from the synchronous lease callback.
/// The callback retains the stream mutex until the matching Rust abort or finalize callback.
///
/// # Safety
/// `candidate`, `userdata`, and `child` remain valid for this synchronous callback.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_stream_create_prepare_callback_rust(
    candidate: *const CDeviceLease,
    userdata: *mut c_void,
    child: *mut StreamCreateLeaseChild,
) -> bool {
    if userdata.is_null() {
        return false;
    }
    crate::stream_mutex_ffi::tsfp_audio_stream_mutex_lock_rust();
    // SAFETY: the device lease adapter keeps the request context alive until callback return.
    let request = unsafe { &mut *userdata.cast::<StreamCreateRequestContext>() };
    let policy_error = unsafe { tsfp_audio_stream_create_policy_error_host_c() };
    if !policy_error.is_null() {
        request.error = policy_error;
        return false;
    }
    if candidate.is_null() || child.is_null() {
        request.error = c"Rust stream-create preparation refused".as_ptr();
        return false;
    }
    // Keep an emergency C-compatible cleanup node available if Rust rollback cannot be recorded.
    let node = allocate_rollback_node();
    if node.is_null() {
        request.status = 0x8007_000e;
        return false;
    }
    request.node = node;
    let mut result = StreamCreatePrepareResult::default();
    let code = unsafe {
        tsfp_audio_stream_create_prepare_rust(
            request.finalize.host_pid,
            candidate,
            &request.scope,
            request.output,
            &mut result,
        )
    };
    unsafe {
        request.node.write(StreamCreateRollbackNode {
            stream_address: result.stream_address,
            stream_heap: result.stream_heap,
            next: std::ptr::null_mut(),
        });
    }
    request.lease_reserved = result.lease_reserved != 0;
    request.routing_reserved = result.routing_reserved != 0;
    request.finalize.stream_address = result.stream_address;
    request.finalize.output_address = request.output;
    request.finalize.routing_reserved = u32::from(request.routing_reserved);
    if code != 0 {
        if code == 4 {
            request.status = 0x8007_000e;
        }
        request.error = prepare_error(code);
        return false;
    }
    unsafe {
        child.write(StreamCreateLeaseChild {
            heap: result.stream_heap,
            address: result.stream_address,
            bytes: 0x28,
        });
    }
    true
}

/// Finish a failed Rust-host Create after the device-lease callback releases its lock.
/// Rust owns heap cleanup, lease rollback, error selection and request-node disposition;
/// C only links the emergency node if both Rust rollback stores refuse it.
///
/// # Safety
/// `userdata` must point to the live, layout-compatible synchronous C request.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_stream_create_cleanup_aborted_rust(userdata: *mut c_void) {
    if userdata.is_null() {
        return;
    }
    // SAFETY: the C caller retains its stack request through this synchronous cleanup.
    let request = unsafe { &mut *userdata.cast::<StreamCreateRequestContext>() };
    if request.node.is_null() {
        return;
    }
    crate::stream_mutex_ffi::tsfp_audio_stream_mutex_lock_rust();
    // SAFETY: the node is allocated by allocate_rollback_node during prepare.
    let node = unsafe { &mut *request.node };
    let released = crate::stream_lease_ffi::tsfp_audio_stream_create_abort_rust(
        node.stream_address,
        node.stream_heap,
        request.output,
        request.lease_reserved,
        request.routing_reserved,
    );
    if released {
        // SAFETY: the request node is uniquely owned until it is retained or released.
        unsafe { release_rollback_node(request.node) };
        request.node = std::ptr::null_mut();
    } else {
        retain_untracked_rollback(request.node);
        request.error = rollback_error(request.lease_reserved);
        request.node = std::ptr::null_mut();
    }
    crate::stream_mutex_ffi::tsfp_audio_stream_mutex_unlock_rust();
}

/// Finalize a successful Rust-host Create and release its retained stream lock.
///
/// # Safety
/// Both pointers must refer to the live lease and first-member callback context
/// retained by the synchronous C request.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_stream_create_finalize_callback_rust(
    lease: *const CDeviceLease,
    userdata: *mut c_void,
) {
    if lease.is_null() || userdata.is_null() {
        return;
    }
    // SAFETY: the synchronous callback retains this context and lease.
    let context = unsafe { &mut *userdata.cast::<StreamCreateFinalizeContext>() };
    context.failures = unsafe {
        tsfp_audio_stream_create_finalize_rust(
            context.host_pid,
            context.stream_address,
            context.output_address,
            lease,
            (*lease).serial,
            context.routing_reserved != 0,
        )
    };
    context.announce = u32::from(tsfp_audio_stream_create_claim_announcement_rust());
    crate::stream_mutex_ffi::tsfp_audio_stream_mutex_unlock_rust();
}

const _: () = assert!(std::mem::size_of::<StreamLeaseScope>() == 72);
const _: () = assert!(std::mem::size_of::<StreamCreatePrepareResult>() == 20);
const _: () = assert!(
    std::mem::size_of::<StreamCreateRollbackNode>() == std::mem::size_of::<*mut c_void>() + 8
);

#[cfg(test)]
mod prepare_callback_tests {
    use super::{prepare_error, rollback_error};

    #[test]
    fn prepare_callback_keeps_oom_error_empty_and_maps_refusals() {
        assert!(prepare_error(4).is_null());
        for code in [1, 2, 3, 5, 6, 7, 8, 9] {
            assert!(
                !prepare_error(code).is_null(),
                "missing refusal for code {code}"
            );
        }
    }

    #[test]
    fn rollback_error_distinguishes_reserved_and_unreserved_failures() {
        // SAFETY: both pointers are static C strings returned by rollback_error.
        let reserved = unsafe { std::ffi::CStr::from_ptr(rollback_error(true)) };
        // SAFETY: both pointers are static C strings returned by rollback_error.
        let unreserved = unsafe { std::ffi::CStr::from_ptr(rollback_error(false)) };
        assert_eq!(
            reserved.to_bytes(),
            b"Rust stream lease abort transition failed"
        );
        assert_eq!(
            unreserved.to_bytes(),
            b"private allocation rollback failed before Rust reservation"
        );
    }
}

/// Run the ordered Rust-host portion of DirectSound stream creation preparation.
///
/// # Safety
/// The candidate, scope, and result pointers must reference readable/readable/writable
/// values for this synchronous call. `scope` uses the matching C adapter layout.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_stream_create_prepare_rust(
    host_pid: i32,
    candidate: *const CDeviceLease,
    scope: *const StreamLeaseScope,
    output: u32,
    result: *mut StreamCreatePrepareResult,
) -> u32 {
    if result.is_null() {
        return 1;
    }
    if host_pid <= 0 || candidate.is_null() || scope.is_null() {
        let state = StreamCreatePrepareResult {
            code: 1,
            ..Default::default()
        };
        // SAFETY: result is non-null and caller-provided writable storage.
        unsafe { result.write(state) };
        return state.code;
    }
    // SAFETY: pointer validity is the synchronous ABI contract above.
    let candidate = unsafe { *candidate };
    let scope = unsafe { *scope };
    let mut state = StreamCreatePrepareResult::default();
    let finish = |state: StreamCreatePrepareResult, out: *mut StreamCreatePrepareResult| {
        // SAFETY: caller provided writable result storage and it remains live for this call.
        unsafe { out.write(state) };
        state.code
    };
    if !tsfp_audio_stream_create_tables_stopped_rust(host_pid) {
        state.code = 1;
        return finish(state, result);
    }
    if crate::stream_scope_ffi::tsfp_audio_stream_create_fixed_overlap_rust(
        output,
        4,
        candidate.internal_address,
        true,
    ) || crate::stream_lease_ffi::tsfp_audio_stream_lease_overlaps_guest_span_rust(output, 4)
        || crate::stream_lease_ffi::tsfp_audio_stream_lease_overlaps_i3dl2_span_rust(output, 4)
        || crate::stream_scope_ffi::tsfp_audio_stream_create_output_overlap_rust(
            output,
            candidate.internal_address,
            scope.descriptor_address,
            scope.format_address,
        )
    {
        state.code = 2;
        return finish(state, result);
    }
    if !tsfp_audio_stream_create_probe_output_rust(host_pid, output) {
        state.code = 3;
        return finish(state, result);
    }
    // SAFETY: both output pointers refer to writable local words.
    if !unsafe {
        crate::device_ffi::tsfp_audio_stream_heap_allocate_rust(
            tsfp_audio::stream_lease::STREAM_ALLOCATION_BYTES,
            &mut state.stream_heap,
            &mut state.stream_address,
        )
    } {
        state.code = 4;
        return finish(state, result);
    }
    if state.stream_address == 0 {
        state.code = 4;
        return finish(state, result);
    }
    if crate::stream_scope_ffi::tsfp_audio_stream_create_fixed_overlap_rust(
        state.stream_address,
        tsfp_audio::stream_lease::STREAM_ALLOCATION_BYTES,
        candidate.internal_address,
        true,
    ) || crate::stream_lease_ffi::tsfp_audio_stream_lease_overlaps_guest_span_rust(
        state.stream_address,
        tsfp_audio::stream_lease::STREAM_ALLOCATION_BYTES,
    ) || crate::stream_lease_ffi::tsfp_audio_stream_lease_overlaps_i3dl2_span_rust(
        state.stream_address,
        tsfp_audio::stream_lease::STREAM_ALLOCATION_BYTES,
    ) || crate::stream_scope_ffi::tsfp_audio_stream_create_allocation_overlap_rust(
        state.stream_address,
        output,
        scope.descriptor_address,
        scope.format_address,
    ) {
        state.code = 5;
        return finish(state, result);
    }
    if !tsfp_audio_stream_create_write_header_rust(
        host_pid,
        state.stream_address,
        candidate.internal_address,
    ) {
        state.code = 6;
        return finish(state, result);
    }
    // SAFETY: the C adapter's scope uses the same repr(C) 72-byte layout.
    if !unsafe {
        tsfp_audio_stream_create_reserve_rust(
            state.stream_address,
            state.stream_heap,
            output,
            candidate.internal_address,
            (&scope as *const StreamLeaseScope).cast(),
        )
    } {
        state.code = 7;
        return finish(state, result);
    }
    state.lease_reserved = 1;
    if !crate::routing_ffi::tsfp_audio_stream_routing_prepare_rust(state.stream_address) {
        state.code = 8;
        return finish(state, result);
    }
    state.routing_reserved = 1;
    finish(state, result)
}

fn passive_stream_header(internal_address: u32) -> [u32; STREAM_HEADER_WORDS] {
    let mut words = [0; STREAM_HEADER_WORDS];
    words[..4].copy_from_slice(&[STREAM_VTABLE, STREAM_SECONDARY, 1, internal_address]);
    words
}

type DsoundHandler = unsafe extern "C" fn(*mut c_void) -> u32;

const REFUSE_DECODE: u32 = 1;
const CREATE_POLICY_ALLOWED: u32 = 0;
const CREATE_POLICY_DISABLED: u32 = 1;
const CREATE_POLICY_IRQL: u32 = 2;
const CREATE_POLICY_STATE_UNREADABLE: u32 = 3;
const CREATE_POLICY_STATE_NONZERO: u32 = 4;
const CREATE_OUTCOME_READY: u32 = 0;
const CREATE_OUTCOME_OOM: u32 = 1;
const CREATE_OUTCOME_REFUSE: u32 = 2;
const CREATE_BIND_LEASE_FAILED: u32 = 1;
const CREATE_BIND_ROUTING_FAILED: u32 = 2;
const CREATE_PUBLISH_FAILED: u32 = 4;
const GLOBAL_AUDIO_STATE: u32 = 0x0041_24a8;

static STREAM_CREATE_ENABLED: AtomicBool = AtomicBool::new(false);

/// Claim the Rust-host's once-per-reset Create policy announcement.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_stream_create_claim_announcement_rust() -> bool {
    crate::stream_create_announcement::claim()
}

/// Reset the Rust-host's Create announcement alongside stream state.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_stream_create_reset_announcement_rust() {
    crate::stream_create_announcement::reset()
}

/// Store the explicit headless-stream policy used by Rust-host handlers.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_stream_create_set_enabled_rust(enabled: bool) {
    STREAM_CREATE_ENABLED.store(enabled, Ordering::Release);
}

/// Read the explicit headless-stream policy without entering the C stream lock.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_stream_create_enabled_rust() -> bool {
    STREAM_CREATE_ENABLED.load(Ordering::Acquire)
}
const LEASE_OK: u32 = 0;
const LEASE_OUT_OF_MEMORY: u32 = 4;
const LEASE_PREPARE_FAILED: u32 = 5;
const CREATE_OOM_STATUS: u32 = 0x8007_000e;

fn stream_tables_stopped(memory: &impl GuestMemory) -> bool {
    let Some(bytes) = memory.read_bytes(tsfp_audio::lifecycle::STREAM_SECONDARY, 15 * 4) else {
        return false;
    };
    let words: Vec<u32> = bytes
        .chunks_exact(4)
        .map(|word| u32::from_le_bytes([word[0], word[1], word[2], word[3]]))
        .collect();
    stream_tables_are_stopped(&words)
}

fn read_global_audio_state(memory: &impl GuestMemory) -> Option<u32> {
    memory.read_u32(GLOBAL_AUDIO_STATE)
}

/// Snapshot and validate the original 15-word secondary table through Rust guest memory.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_stream_create_tables_stopped_rust(host_pid: i32) -> bool {
    if host_pid <= 0 {
        return false;
    }
    stream_tables_stopped(&FrameGuestMemory::new(host_pid))
}

/// Validate a caller-owned snapshot of the fifteen secondary-table entries.
/// The C host supplies the snapshot only on its compatibility path; the Rust
/// audio host reads the table through `FrameGuestMemory` above.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_stream_create_table_snapshot_stopped_rust(
    words: *const u32,
    word_count: usize,
) -> bool {
    if words.is_null() || word_count != 15 {
        return false;
    }
    // SAFETY: caller guarantees a readable array of `word_count` u32 entries.
    let words = unsafe { std::slice::from_raw_parts(words, word_count) };
    stream_tables_are_stopped(words)
}

/// Build the passive stream header using the Rust lifecycle contract.
///
/// # Safety
/// `header` must point to writable storage for `STREAM_HEADER_WORDS` words.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_stream_create_header_rust(
    internal_address: u32,
    header: *mut u32,
) -> bool {
    if header.is_null() {
        return false;
    }
    let words = passive_stream_header(internal_address);
    // SAFETY: the caller provides writable storage for the documented fixed word count.
    unsafe { header.copy_from_nonoverlapping(words.as_ptr(), words.len()) };
    true
}

/// Evaluate the passive stream policy after the host snapshots its synchronized inputs.
/// The host remains responsible for lock ownership, IRQL observation and the guarded guest read.
fn create_policy_decision(
    enabled: u32,
    irql_known: u32,
    irql: u32,
    state_known: u32,
    state: u32,
) -> u32 {
    if enabled == 0 {
        CREATE_POLICY_DISABLED
    } else if irql_known == 0 || irql != 0 {
        CREATE_POLICY_IRQL
    } else if state_known == 0 {
        CREATE_POLICY_STATE_UNREADABLE
    } else if state != 0 {
        CREATE_POLICY_STATE_NONZERO
    } else {
        CREATE_POLICY_ALLOWED
    }
}

fn policy_error_message(reason: u32) -> *const c_char {
    match reason {
        CREATE_POLICY_ALLOWED => std::ptr::null(),
        CREATE_POLICY_DISABLED => c"explicit headless-streams policy is disabled".as_ptr(),
        CREATE_POLICY_IRQL => c"only known IRQL0 is supported".as_ptr(),
        CREATE_POLICY_STATE_UNREADABLE | CREATE_POLICY_STATE_NONZERO => {
            c"original global audio state must be zero".as_ptr()
        }
        _ => c"original global audio state must be zero".as_ptr(),
    }
}

/// Evaluate and format the Rust-host stream-create policy refusal.
/// The C host snapshots its non-reentrant IRQL and PID inputs; Rust preserves
/// short-circuit ordering and performs the guarded guest-state read itself.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_stream_create_policy_error_rust(
    enabled: u32,
    irql_known: u32,
    irql: u32,
    host_pid: i32,
) -> *const c_char {
    let state = if enabled != 0 && irql_known != 0 && irql == 0 && host_pid > 0 {
        read_global_audio_state(&FrameGuestMemory::new(host_pid))
    } else {
        None
    };
    let reason = create_policy_decision(
        enabled,
        irql_known,
        irql,
        u32::from(state.is_some()),
        state.unwrap_or_default(),
    );
    policy_error_message(reason)
}

/// Classify a cleaned device-lease result for stream creation.
/// C retains rollback callbacks and the host fatal-refusal boundary.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_stream_create_outcome_rust(
    lease_status: u32,
    create_status: u32,
    has_error: u32,
) -> u32 {
    if lease_status == LEASE_OK {
        CREATE_OUTCOME_READY
    } else if lease_status == LEASE_OUT_OF_MEMORY
        || (lease_status == LEASE_PREPARE_FAILED
            && create_status == CREATE_OOM_STATUS
            && has_error == 0)
    {
        CREATE_OUTCOME_OOM
    } else {
        CREATE_OUTCOME_REFUSE
    }
}

/// Bind the committed lease and optional routing sidecar in the stream-create order.
/// The caller holds the stream lock and inserts its cleanup payload after this returns.
/// A bitmask preserves the adapter's independent invariant diagnostics.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_stream_create_bind_state_rust(
    stream_address: u32,
    lease: *const crate::stream_lease_ffi::CDeviceLease,
    routing_serial: u64,
    bind_routing: bool,
) -> u32 {
    if lease.is_null() {
        return CREATE_BIND_LEASE_FAILED
            | if bind_routing {
                CREATE_BIND_ROUTING_FAILED
            } else {
                0
            };
    }
    // SAFETY: caller holds the stream lock and passes the committed lease synchronously.
    let lease_bound = unsafe {
        crate::stream_lease_ffi::tsfp_audio_stream_lease_bind_rust(stream_address, lease)
    };
    let routing_bound = !bind_routing
        || crate::routing_ffi::tsfp_audio_stream_routing_bind_rust(stream_address, routing_serial);
    u32::from(!lease_bound) * CREATE_BIND_LEASE_FAILED
        | u32::from(!routing_bound) * CREATE_BIND_ROUTING_FAILED
}

fn finalize_stream_create(bind: impl FnOnce() -> u32, publish: impl FnOnce() -> bool) -> u32 {
    let binding = bind();
    let publication = publish();
    binding | u32::from(!publication) * CREATE_PUBLISH_FAILED
}

/// Bind committed Rust lease state before publishing the guest stream pointer.
/// A bind invariant failure is reported independently; as in the adapter oracle,
/// publication still follows the committed device lease.
///
/// # Safety
/// `lease` must point to a live `dsound_device_lease` for this synchronous call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_stream_create_finalize_rust(
    host_pid: i32,
    stream_address: u32,
    output_address: u32,
    lease: *const crate::stream_lease_ffi::CDeviceLease,
    routing_serial: u64,
    bind_routing: bool,
) -> u32 {
    let binding = || {
        // SAFETY: the caller guarantees a live lease for this synchronous callback.
        unsafe {
            tsfp_audio_stream_create_bind_state_rust(
                stream_address,
                lease,
                routing_serial,
                bind_routing,
            )
        }
    };
    let publish =
        || tsfp_audio_stream_create_publish_rust(host_pid, output_address, stream_address);
    finalize_stream_create(binding, publish)
}

#[cfg(test)]
mod create_finalize_tests {
    use super::finalize_stream_create;
    use super::{CREATE_BIND_LEASE_FAILED, CREATE_BIND_ROUTING_FAILED, CREATE_PUBLISH_FAILED};
    use std::cell::RefCell;

    #[test]
    fn finalization_binds_before_publication_and_preserves_independent_failures() {
        let order = RefCell::new(Vec::new());
        let result = finalize_stream_create(
            || {
                order.borrow_mut().push("bind");
                CREATE_BIND_LEASE_FAILED | CREATE_BIND_ROUTING_FAILED
            },
            || {
                order.borrow_mut().push("publish");
                false
            },
        );
        assert_eq!(*order.borrow(), ["bind", "publish"]);
        assert_eq!(
            result,
            CREATE_BIND_LEASE_FAILED | CREATE_BIND_ROUTING_FAILED | CREATE_PUBLISH_FAILED
        );
    }
}

#[cfg(test)]
mod create_bind_state_tests {
    use super::*;

    #[test]
    fn bind_state_rejects_null_lease_and_reports_each_requested_sidecar() {
        // SAFETY: null is explicitly rejected before dereference.
        let no_routing =
            unsafe { tsfp_audio_stream_create_bind_state_rust(0x1234, std::ptr::null(), 7, false) };
        // SAFETY: null is explicitly rejected before dereference.
        let with_routing =
            unsafe { tsfp_audio_stream_create_bind_state_rust(0x1234, std::ptr::null(), 7, true) };
        assert_eq!(no_routing, CREATE_BIND_LEASE_FAILED);
        assert_eq!(
            with_routing,
            CREATE_BIND_LEASE_FAILED | CREATE_BIND_ROUTING_FAILED
        );
    }
}

#[cfg(test)]
mod untracked_rollback_tests {
    use super::*;

    #[test]
    fn emergency_cleanup_keeps_the_failed_node_and_its_tail_for_retry() {
        let tail = Box::into_raw(Box::new(StreamCreateRollbackNode {
            stream_address: 2,
            stream_heap: 22,
            next: std::ptr::null_mut(),
        }));
        let head = Box::into_raw(Box::new(StreamCreateRollbackNode {
            stream_address: 1,
            stream_heap: 11,
            next: tail,
        }));
        let mut cleaned = Vec::new();

        // SAFETY: both nodes are uniquely owned by this test until the release callback.
        let (remaining, status) = unsafe {
            drain_untracked_rollback_with(
                head,
                |heap| {
                    cleaned.push(heap);
                    heap != 22
                },
                |node| drop(Box::from_raw(node)),
            )
        };
        assert_eq!(cleaned, [11, 22]);
        assert_eq!(status, 1);
        assert_eq!(remaining, tail);

        // Retry succeeds and drains the retained tail exactly once.
        let mut retried = Vec::new();
        // SAFETY: the failed head is still uniquely owned and intact.
        let (remaining, status) = unsafe {
            drain_untracked_rollback_with(
                remaining,
                |heap| {
                    retried.push(heap);
                    true
                },
                |node| drop(Box::from_raw(node)),
            )
        };
        assert!(remaining.is_null());
        assert_eq!(status, 0);
        assert_eq!(retried, [22]);
    }

    #[test]
    fn emergency_rollback_spans_remain_protected_until_cleanup() {
        let node = Box::into_raw(Box::new(StreamCreateRollbackNode {
            stream_address: 0x1000,
            stream_heap: 22,
            next: std::ptr::null_mut(),
        }));
        // SAFETY: the single node remains allocated for the duration of the checks.
        assert!(untracked_rollback_overlaps(node, 0x1010, 4));
        assert!(!untracked_rollback_overlaps(node, 0x1028, 4));
        assert!(!untracked_rollback_overlaps(node, 0x1000, 0));
        // SAFETY: this test still uniquely owns the Box allocation.
        unsafe { drop(Box::from_raw(node)) };
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
struct KernelCallFramePrefix {
    stack_pointer: u32,
    stack_limit: u32,
}

trait CreateHleHost {
    fn extension(&mut self, context: *mut c_void) -> Option<u32>;
    fn decode(&mut self, context: *mut c_void) -> Option<FrameCall>;
    fn create(&mut self, call: &FrameCall) -> u32;
    fn refuse(&mut self, reason: u32) -> !;
}

struct HostCreateHle;

unsafe extern "C" {
    fn dsound_hle_register_rust(address: u32) -> bool;
    fn dsound_stream_create_hle_extension_rust(
        context: *mut c_void,
        caller: u32,
        result_out: *mut u32,
    ) -> bool;
    fn dsound_stream_control_hle_guest_pid_rust() -> i32;
    fn dsound_stream_control_hle_refuse_rust(entry: u32, reason: u32) -> !;
}

/// Run the measured frame-to-lease sequence while keeping kernel and device services
/// behind synchronous C adapters. The C host build keeps its original transaction.
fn create_transaction(descriptor: u32, output: u32) -> u32 {
    let policy_error = unsafe { tsfp_audio_stream_create_policy_error_snapshot_host_c() };
    if !policy_error.is_null() {
        unsafe { tsfp_audio_stream_create_refuse_host_c(policy_error) };
    }

    let host_pid = unsafe { dsound_stream_control_hle_guest_pid_rust() };
    if host_pid <= 0 {
        unsafe {
            tsfp_audio_stream_create_refuse_host_c(c"stream guest process is unavailable".as_ptr())
        };
    }
    let Some(scope) =
        crate::stream_scope_ffi::snapshot_lease_scope(&FrameGuestMemory::new(host_pid), descriptor)
    else {
        unsafe {
            tsfp_audio_stream_create_refuse_host_c(
                c"descriptor/format outside measured startup scope".as_ptr(),
            )
        };
    };
    let mut request = StreamCreateRequestContext {
        finalize: StreamCreateFinalizeContext {
            host_pid,
            stream_address: 0,
            output_address: 0,
            routing_reserved: 0,
            failures: 0,
            announce: 0,
        },
        scope,
        output,
        status: 0,
        node: std::ptr::null_mut(),
        error: std::ptr::null(),
        announce: false,
        routing_reserved: false,
        lease_reserved: false,
    };
    let device = snapshot_device_singleton(&FrameGuestMemory::new(host_pid));
    let Some(device) = device.filter(|device| *device != 0 && *device <= u32::MAX - 8) else {
        unsafe { tsfp_audio_stream_create_refuse_host_c(c"no owned SILENT device".as_ptr()) };
    };
    let mut status = LEASE_OK;
    let mut lease = CDeviceLease {
        device_heap: 0,
        internal_address: 0,
        serial: 0,
    };
    unsafe {
        tsfp_audio_stream_create_acquire_host_c(
            device + 8,
            (&mut request as *mut StreamCreateRequestContext).cast(),
            &mut status,
            &mut lease,
        );
    }
    request.announce = request.finalize.announce != 0;
    unsafe {
        tsfp_audio_stream_create_report_host_c(
            (&request as *const StreamCreateRequestContext).cast(),
            &lease,
            status,
        )
    };
    if status != LEASE_OK {
        unsafe {
            tsfp_audio_stream_create_cleanup_host_c(
                (&mut request as *mut StreamCreateRequestContext).cast(),
            )
        };
    }
    let outcome = tsfp_audio_stream_create_outcome_rust(
        status,
        request.status,
        u32::from(!request.error.is_null()),
    );
    tsfp_audio_stream_create_release_request_rust(request.node.cast());
    if outcome == CREATE_OUTCOME_OOM {
        CREATE_OOM_STATUS
    } else if outcome != CREATE_OUTCOME_READY {
        let reason = if request.error.is_null() {
            c"device lease transaction refused".as_ptr()
        } else {
            request.error
        };
        unsafe { tsfp_audio_stream_create_refuse_host_c(reason) }
    } else {
        0
    }
}

fn snapshot_device_singleton(memory: &impl GuestMemory) -> Option<u32> {
    memory.read_u32(SINGLETON)
}

fn snapshot_extension_caller(memory: &impl GuestMemory, stack_pointer: u32) -> Option<u32> {
    memory.read_u32(stack_pointer)
}

/// Read the measured DirectSound singleton through the Rust guest-memory adapter.
///
/// # Safety
/// `output` must point to writable storage for one `u32`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_stream_device_singleton_rust(
    host_pid: i32,
    output: *mut u32,
) -> bool {
    if host_pid <= 0 || output.is_null() {
        return false;
    }
    let Some(device) = snapshot_device_singleton(&FrameGuestMemory::new(host_pid)) else {
        return false;
    };
    // SAFETY: output validity is the caller's FFI contract.
    unsafe { output.write(device) };
    true
}

/// Probe an output slot with the oracle's same-value write before lease preparation.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_stream_create_probe_output_rust(host_pid: i32, address: u32) -> bool {
    let memory = FrameGuestMemory::new(host_pid);
    let Some(value) = memory.read_u32(address) else {
        return false;
    };
    let mut memory = memory;
    memory.write_u32(address, value)
}

/// Publish a committed stream pointer through the fault-safe guest-memory adapter.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_stream_create_publish_rust(
    host_pid: i32,
    address: u32,
    stream: u32,
) -> bool {
    FrameGuestMemory::new(host_pid).write_u32(address, stream)
}

/// Construct and write the initialized private stream header through Rust guest memory.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_stream_create_write_header_rust(
    host_pid: i32,
    address: u32,
    internal_address: u32,
) -> bool {
    if host_pid <= 0 {
        return false;
    }
    let words = passive_stream_header(internal_address);
    let mut bytes = [0u8; 40];
    for (word, out) in words.iter().zip(bytes.chunks_exact_mut(4)) {
        out.copy_from_slice(&word.to_le_bytes());
    }
    // SAFETY: `bytes` remains live and readable through the synchronous guest write.
    unsafe {
        crate::effects_guest_copy_ffi::tsfp_effects_guest_copy_write_rust(
            host_pid,
            bytes.as_ptr(),
            address,
            bytes.len(),
        )
    }
}

/// Reserve the Rust-owned lease using the same header that was written to guest memory.
///
/// # Safety
/// `scope` must point to a readable `StreamLeaseScope` for this synchronous call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_stream_create_reserve_rust(
    stream_address: u32,
    stream_heap: u32,
    publication_address: u32,
    internal_address: u32,
    scope: *const tsfp_audio::stream_lease::StreamLeaseScope,
) -> bool {
    if scope.is_null() {
        return false;
    }
    let header = passive_stream_header(internal_address);
    // SAFETY: `scope` is forwarded under the documented synchronous caller contract.
    unsafe {
        crate::stream_lease_ffi::tsfp_audio_stream_lease_reserve_rust(
            stream_address,
            stream_heap,
            publication_address,
            header.as_ptr(),
            scope,
        )
    }
}

fn decode_create(memory: &impl GuestMemory, prefix: KernelCallFramePrefix) -> Option<FrameCall> {
    decode_frame(
        memory,
        CREATE,
        CallFrame {
            stack_pointer: prefix.stack_pointer,
            stack_limit: prefix.stack_limit,
        },
        false,
    )
    .ok()
}

impl CreateHleHost for HostCreateHle {
    fn extension(&mut self, context: *mut c_void) -> Option<u32> {
        if context.is_null() {
            return None;
        }
        // SAFETY: the HLE callback receives a live kernel frame whose prefix is stable.
        let prefix = unsafe { *(context.cast::<KernelCallFramePrefix>()) };
        let host_pid = unsafe { dsound_stream_control_hle_guest_pid_rust() };
        if host_pid <= 0 {
            return None;
        }
        let caller =
            snapshot_extension_caller(&FrameGuestMemory::new(host_pid), prefix.stack_pointer)?;
        let mut result = 0;
        // SAFETY: C retains synchronized route lookup and invokes the measured extension.
        unsafe {
            dsound_stream_create_hle_extension_rust(context, caller, &mut result).then_some(result)
        }
    }

    fn decode(&mut self, context: *mut c_void) -> Option<FrameCall> {
        if context.is_null() {
            return None;
        }
        // SAFETY: C passes a live kernel_call_frame with this stable prefix.
        let prefix = unsafe { context.cast::<KernelCallFramePrefix>().read() };
        // SAFETY: this is the process whose identity-mapped guest stack is active.
        let host_pid = unsafe { dsound_stream_control_hle_guest_pid_rust() };
        if host_pid <= 0 {
            return None;
        }
        decode_create(&FrameGuestMemory::new(host_pid), prefix)
    }

    fn create(&mut self, call: &FrameCall) -> u32 {
        create_transaction(call.args[0], call.args[1])
    }

    fn refuse(&mut self, reason: u32) -> ! {
        // SAFETY: C preserves the established fatal HLE refusal path.
        unsafe { dsound_stream_control_hle_refuse_rust(CREATE, reason) }
    }
}

fn dispatch(host: &mut impl CreateHleHost, context: *mut c_void) -> u32 {
    if let Some(result) = host.extension(context) {
        return result;
    }
    let Some(call) = host.decode(context) else {
        host.refuse(REFUSE_DECODE);
    };
    host.create(&call)
}

unsafe extern "C" fn create_handler(context: *mut c_void) -> u32 {
    dispatch(&mut HostCreateHle, context)
}

/// Register Rust's measured stream-create callback on the Rust audio host.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_stream_create_register_rust() -> usize {
    if !crate::movie_device_hle_ffi::tsfp_audio_hle_register_handler_rust(CREATE, create_handler) {
        return 0;
    }
    // SAFETY: C initialized its measured HLE surface before stream registration.
    unsafe { dsound_hle_register_rust(CREATE) as usize }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::{CStr, c_void};
    use tsfp_audio::adapter::GuestMemory;

    #[test]
    fn rust_create_request_owner_releases_transient_node() {
        let request = allocate_rollback_node();
        assert!(!request.is_null());
        tsfp_audio_stream_create_release_request_rust(request.cast());
        tsfp_audio_stream_create_release_request_rust(std::ptr::null_mut());
    }

    #[test]
    fn create_prepare_reports_invalid_adapter_inputs_in_result() {
        let mut result = StreamCreatePrepareResult::default();
        let code = unsafe {
            tsfp_audio_stream_create_prepare_rust(
                0,
                std::ptr::null(),
                std::ptr::null(),
                0,
                &mut result,
            )
        };
        assert_eq!(code, 1);
        assert_eq!(result.code, 1);
        assert_eq!(result.stream_heap, 0);
        assert_eq!(result.stream_address, 0);
    }

    struct Memory([u32; 6]);

    impl GuestMemory for Memory {
        fn read_u32(&self, address: u32) -> Option<u32> {
            let index = address.checked_sub(0x1000)? as usize / 4;
            self.0.get(index).copied()
        }

        fn write_u32(&mut self, _: u32, _: u32) -> bool {
            false
        }

        fn read_bytes(&self, _: u32, _: usize) -> Option<Vec<u8>> {
            None
        }

        fn range_readable(&self, _: u32, _: u32) -> bool {
            false
        }
    }

    struct SingletonMemory {
        value: Option<u32>,
        reads: std::cell::RefCell<Vec<u32>>,
    }

    struct GlobalStateMemory {
        value: Option<u32>,
        reads: std::cell::RefCell<Vec<u32>>,
    }

    impl GuestMemory for GlobalStateMemory {
        fn read_u32(&self, address: u32) -> Option<u32> {
            self.reads.borrow_mut().push(address);
            (address == GLOBAL_AUDIO_STATE)
                .then_some(self.value)
                .flatten()
        }

        fn write_u32(&mut self, _: u32, _: u32) -> bool {
            false
        }
        fn read_bytes(&self, _: u32, _: usize) -> Option<Vec<u8>> {
            None
        }
        fn range_readable(&self, _: u32, _: u32) -> bool {
            false
        }
    }

    #[test]
    fn stream_create_global_state_snapshot_reads_only_the_fixed_word() {
        let memory = GlobalStateMemory {
            value: Some(7),
            reads: Default::default(),
        };
        assert_eq!(read_global_audio_state(&memory), Some(7));
        assert_eq!(&*memory.reads.borrow(), &[GLOBAL_AUDIO_STATE]);
        let unreadable = GlobalStateMemory {
            value: None,
            reads: Default::default(),
        };
        assert_eq!(read_global_audio_state(&unreadable), None);
    }

    #[test]
    fn stream_create_header_matches_the_passive_lifecycle_layout() {
        let mut header = [u32::MAX; STREAM_HEADER_WORDS];
        // SAFETY: `header` has exactly the output capacity required by the FFI contract.
        assert!(unsafe { tsfp_audio_stream_create_header_rust(0x1234_5678, header.as_mut_ptr()) });
        assert_eq!(
            header,
            [
                STREAM_VTABLE,
                STREAM_SECONDARY,
                1,
                0x1234_5678,
                0,
                0,
                0,
                0,
                0,
                0,
            ]
        );
        // SAFETY: a null pointer is rejected before any write.
        assert!(!unsafe { tsfp_audio_stream_create_header_rust(0, std::ptr::null_mut()) });
    }

    impl GuestMemory for SingletonMemory {
        fn read_u32(&self, address: u32) -> Option<u32> {
            self.reads.borrow_mut().push(address);
            if address == SINGLETON {
                self.value
            } else {
                None
            }
        }

        fn write_u32(&mut self, _: u32, _: u32) -> bool {
            false
        }

        fn read_bytes(&self, _: u32, _: usize) -> Option<Vec<u8>> {
            None
        }

        fn range_readable(&self, _: u32, _: u32) -> bool {
            false
        }
    }

    #[test]
    fn singleton_snapshot_reads_the_lifecycle_address_once() {
        let memory = SingletonMemory {
            value: Some(0xfeed_beef),
            reads: std::cell::RefCell::new(Vec::new()),
        };
        assert_eq!(snapshot_device_singleton(&memory), Some(0xfeed_beef));
        assert_eq!(&*memory.reads.borrow(), &[SINGLETON]);
        let unreadable = SingletonMemory {
            value: None,
            reads: std::cell::RefCell::new(Vec::new()),
        };
        assert_eq!(snapshot_device_singleton(&unreadable), None);
    }

    #[test]
    fn create_policy_preserves_disabled_irql_and_guest_state_refusals() {
        assert_eq!(
            create_policy_decision(0, 1, 0, 1, 0),
            CREATE_POLICY_DISABLED
        );
        assert_eq!(create_policy_decision(1, 0, 0, 1, 0), CREATE_POLICY_IRQL);
        assert_eq!(create_policy_decision(1, 1, 1, 1, 0), CREATE_POLICY_IRQL);
        assert_eq!(
            create_policy_decision(1, 1, 0, 0, 0),
            CREATE_POLICY_STATE_UNREADABLE
        );
        assert_eq!(
            create_policy_decision(1, 1, 0, 1, 1),
            CREATE_POLICY_STATE_NONZERO
        );
        assert_eq!(create_policy_decision(1, 1, 0, 1, 0), CREATE_POLICY_ALLOWED);
    }

    #[test]
    fn create_policy_error_strings_remain_c_abi_compatible() {
        assert!(policy_error_message(CREATE_POLICY_ALLOWED).is_null());
        assert_eq!(
            unsafe { CStr::from_ptr(policy_error_message(CREATE_POLICY_DISABLED)) }.to_bytes(),
            b"explicit headless-streams policy is disabled"
        );
        assert_eq!(
            unsafe { CStr::from_ptr(policy_error_message(CREATE_POLICY_IRQL)) }.to_bytes(),
            b"only known IRQL0 is supported"
        );
        for reason in [CREATE_POLICY_STATE_UNREADABLE, CREATE_POLICY_STATE_NONZERO] {
            assert_eq!(
                unsafe { CStr::from_ptr(policy_error_message(reason)) }.to_bytes(),
                b"original global audio state must be zero"
            );
        }
    }

    #[test]
    fn rust_host_enabled_policy_is_stored_and_read_atomically() {
        tsfp_audio_stream_create_set_enabled_rust(false);
        assert!(!tsfp_audio_stream_create_enabled_rust());
        tsfp_audio_stream_create_set_enabled_rust(true);
        assert!(tsfp_audio_stream_create_enabled_rust());
        tsfp_audio_stream_create_set_enabled_rust(false);
    }

    #[test]
    fn create_outcome_keeps_oom_narrow_and_other_lease_failures_fatal() {
        assert_eq!(
            tsfp_audio_stream_create_outcome_rust(0, 0, 0),
            CREATE_OUTCOME_READY
        );
        assert_eq!(
            tsfp_audio_stream_create_outcome_rust(4, 0, 0),
            CREATE_OUTCOME_OOM
        );
        assert_eq!(
            tsfp_audio_stream_create_outcome_rust(5, 0x8007_000e, 0),
            CREATE_OUTCOME_OOM
        );
        assert_eq!(
            tsfp_audio_stream_create_outcome_rust(5, 0x8007_000e, 1),
            CREATE_OUTCOME_REFUSE
        );
        assert_eq!(
            tsfp_audio_stream_create_outcome_rust(5, 0, 0),
            CREATE_OUTCOME_REFUSE
        );
        assert_eq!(
            tsfp_audio_stream_create_outcome_rust(6, 0x8007_000e, 0),
            CREATE_OUTCOME_REFUSE
        );
        assert_eq!(
            tsfp_audio_stream_create_outcome_rust(1, 0x8007_000e, 0),
            CREATE_OUTCOME_REFUSE
        );
    }

    struct Host {
        extension_result: Option<u32>,
        decoded: Option<FrameCall>,
        events: Vec<&'static str>,
        created: Option<(u32, u32)>,
    }

    impl CreateHleHost for Host {
        fn extension(&mut self, _: *mut c_void) -> Option<u32> {
            self.events.push("extension");
            self.extension_result
        }

        fn decode(&mut self, _: *mut c_void) -> Option<FrameCall> {
            self.events.push("decode");
            self.decoded
        }

        fn create(&mut self, call: &FrameCall) -> u32 {
            self.events.push("create");
            self.created = Some((call.args[0], call.args[1]));
            0x1234
        }

        fn refuse(&mut self, reason: u32) -> ! {
            panic!("unexpected create refusal {reason}")
        }
    }

    fn prefix() -> KernelCallFramePrefix {
        KernelCallFramePrefix {
            stack_pointer: 0x1000,
            stack_limit: 0x100c,
        }
    }

    #[test]
    fn create_decodes_measured_return_address_and_two_words() {
        let memory = Memory([0x29943, 0x1111, 0x2222, 0, 0, 0]);
        let call = decode_create(&memory, prefix()).expect("measured CreateStream frame");
        assert_eq!(call.entry, CREATE);
        assert_eq!(call.caller, 0x29943);
        assert_eq!(&call.args[..2], &[0x1111, 0x2222]);

        let unmeasured = Memory([0x29944, 0x1111, 0x2222, 0, 0, 0]);
        assert!(decode_create(&unmeasured, prefix()).is_none());
        let truncated = KernelCallFramePrefix {
            stack_limit: 0x1008,
            ..prefix()
        };
        assert!(decode_create(&memory, truncated).is_none());
    }

    #[test]
    fn movie_extension_short_circuits_create_decode() {
        let mut host = Host {
            extension_result: Some(0xabcd),
            decoded: None,
            events: Vec::new(),
            created: None,
        };
        assert_eq!(dispatch(&mut host, core::ptr::null_mut()), 0xabcd);
        assert_eq!(host.events, ["extension"]);
        assert_eq!(host.created, None);
    }

    #[test]
    fn create_dispatch_forwards_the_decoded_arguments() {
        let memory = Memory([0x29943, 0x3333, 0x4444, 0, 0, 0]);
        let call = decode_create(&memory, prefix()).expect("measured CreateStream frame");
        let mut host = Host {
            extension_result: None,
            decoded: Some(call),
            events: Vec::new(),
            created: None,
        };
        assert_eq!(dispatch(&mut host, core::ptr::null_mut()), 0x1234);
        assert_eq!(host.events, ["extension", "decode", "create"]);
        assert_eq!(host.created, Some((0x3333, 0x4444)));
    }

    #[test]
    fn extension_caller_snapshot_reads_only_the_stack_argument() {
        let memory = Memory([0x0044_570b, 0, 0, 0, 0, 0]);
        assert_eq!(
            snapshot_extension_caller(&memory, 0x1000),
            Some(0x0044_570b)
        );
        assert_eq!(snapshot_extension_caller(&memory, 0x2000), None);
    }

    #[test]
    fn probes_then_publishes_committed_stream_pointer_to_guest_word() {
        let (page, address) = crate::test_guest_memory::map_guest_page();
        // SAFETY: the test owns a writable page and initializes the output word.
        unsafe { page.cast::<u32>().write(0x1111_2222) };
        let pid = std::process::id() as i32;
        assert!(tsfp_audio_stream_create_probe_output_rust(pid, address));
        // SAFETY: the mapped word remains live and readable in this test.
        assert_eq!(unsafe { page.cast::<u32>().read() }, 0x1111_2222);
        assert!(tsfp_audio_stream_create_publish_rust(
            pid,
            address,
            0xAABB_CCDD
        ));
        assert!(!tsfp_audio_stream_create_probe_output_rust(
            pid,
            u32::MAX - 1
        ));
        // SAFETY: read the still-mapped word owned by this test.
        assert_eq!(unsafe { page.cast::<u32>().read() }, 0xAABB_CCDD);
        crate::test_guest_memory::unmap_guest_page(page);
    }

    #[test]
    fn writes_initialized_stream_header_as_one_guest_span() {
        let (page, address) = crate::test_guest_memory::map_guest_page();
        let internal_address = 0x004a_1d00;
        let pid = std::process::id() as i32;
        assert!(tsfp_audio_stream_create_write_header_rust(
            pid,
            address,
            internal_address
        ));
        // SAFETY: the mapped page is writable and the adapter wrote exactly forty bytes.
        let written = unsafe { std::slice::from_raw_parts(page.cast::<u8>(), 40) };
        let header = passive_stream_header(internal_address);
        let expected: Vec<u8> = header.iter().flat_map(|word| word.to_le_bytes()).collect();
        assert_eq!(written, expected);
        // SAFETY: the mapping is owned by this test.
        assert!(!unsafe {
            tsfp_audio_stream_create_write_header_rust(pid, u32::MAX - 8, internal_address)
        });
        assert!(!tsfp_audio_stream_create_write_header_rust(
            -1,
            address,
            internal_address
        ));
        crate::test_guest_memory::unmap_guest_page(page);
    }

    #[test]
    fn stream_table_policy_accepts_only_fifteen_protected_targets() {
        struct TableMemory(Option<Vec<u8>>);
        impl GuestMemory for TableMemory {
            fn read_u32(&self, _: u32) -> Option<u32> {
                None
            }
            fn write_u32(&mut self, _: u32, _: u32) -> bool {
                false
            }
            fn read_bytes(&self, address: u32, len: usize) -> Option<Vec<u8>> {
                (address == tsfp_audio::lifecycle::STREAM_SECONDARY && len == 60)
                    .then(|| self.0.clone())
                    .flatten()
            }
            fn range_readable(&self, _: u32, _: u32) -> bool {
                false
            }
        }
        let encode = |words: &[u32; 15]| words.iter().flat_map(|word| word.to_le_bytes()).collect();
        let table: [u32; 15] = [tsfp_audio::lifecycle::STREAM_STOPPED_TARGETS[0]; 15];
        assert!(stream_tables_stopped(&TableMemory(Some(encode(&table)))));
        // SAFETY: these local arrays have exactly fifteen readable words.
        assert!(unsafe {
            tsfp_audio_stream_create_table_snapshot_stopped_rust(table.as_ptr(), table.len())
        });
        let mut invalid = table;
        invalid[14] = 0x0123_4567;
        assert!(!stream_tables_stopped(&TableMemory(Some(encode(&invalid)))));
        // SAFETY: `invalid` is also a readable fifteen-word snapshot.
        assert!(!unsafe {
            tsfp_audio_stream_create_table_snapshot_stopped_rust(invalid.as_ptr(), invalid.len())
        });
        assert!(!unsafe {
            tsfp_audio_stream_create_table_snapshot_stopped_rust(table.as_ptr(), 14)
        });
        assert!(!unsafe {
            tsfp_audio_stream_create_table_snapshot_stopped_rust(std::ptr::null(), 15)
        });
        assert!(!stream_tables_stopped(&TableMemory(None)));
        assert!(!tsfp_audio_stream_create_tables_stopped_rust(0));
    }
}
