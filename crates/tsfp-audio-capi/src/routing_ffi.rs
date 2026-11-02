// SPDX-License-Identifier: GPL-3.0-or-later
//! Rust-owned stream routing state transitions for the Rust DirectSound host.
//!
//! The C guest adapter still validates stream leases and snapshots host-owned protected ranges;
//! this module owns their overlap policy, descriptor read ordering, fault-safe guest snapshots,
//! typed routing mutation, accepted PCM transaction ordering and gain projection. The C-host
//! oracle retains the original inline implementation.

use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::{Mutex, OnceLock};
use tsfp_audio::adapter::GuestMemory;
use tsfp_audio::routing::{ROUTE_BINS, ROUTE_SLOTS, Routing as StreamRouting, frequency_pitch};
use tsfp_audio::stream_lease::{DeviceLeaseIdentity, StreamLeaseScope};

use crate::frame_guest_memory::FrameGuestMemory;
use crate::stream_lease_ffi::{CDeviceLease, tsfp_audio_stream_lease_scope_rust};

#[derive(Default)]
struct FrequencyTransaction {
    accepted_sets: u32,
    current_hz: u32,
    pitch: i32,
}

/// Stable C snapshot of the Rust-owned per-stream frequency transaction.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CStreamFrequencyState {
    pub accepted_sets: u32,
    pub current_hz: u32,
    pub pitch: i32,
}

struct StreamLeaseState {
    serial: u64,
    routing: StreamRouting,
    frequency: FrequencyTransaction,
}

type RoutingLeases = HashMap<u32, StreamLeaseState>;
static ROUTING_LEASES: OnceLock<Mutex<RoutingLeases>> = OnceLock::new();

fn routing_leases() -> &'static Mutex<RoutingLeases> {
    ROUTING_LEASES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn routing_leases_lock() -> std::sync::MutexGuard<'static, RoutingLeases> {
    routing_leases()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

const GUEST_PAIRS_ACCEPTED: u32 = 0;
const GUEST_PAIRS_READ_REFUSED: u32 = 1;
const GUEST_PAIRS_POLICY_REFUSED: u32 = 2;

const ROUTING_UPDATE_HEADROOM: u32 = 0;
const ROUTING_UPDATE_BINS: u32 = 1;
const ROUTING_UPDATE_VOLUMES: u32 = 2;
const ROUTING_UPDATE_APPLY_REFUSED: u32 = 3;
const ROUTING_SCOPE_ACCEPTED: u32 = 0;
const ROUTING_SCOPE_STALE_LEASE: u32 = 1;
const ROUTING_SCOPE_POLICY_REFUSED: u32 = 2;

fn routing_scope_supported(flags: u32, channels: u16, completion_available: bool) -> bool {
    completion_available && flags == 0 && channels == 2
}

/// Validate the measured PCM-routing capability against the Rust-owned creation scope and the
/// exact active lease generation. C supplies only the synchronized completion/route-service fact.
///
/// # Safety
/// `lease` points to a live CDeviceLease record for this synchronous call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_stream_routing_scope_admit_rust(
    stream: u32,
    lease: *const CDeviceLease,
    completion_available: u32,
) -> u32 {
    if stream == 0 || lease.is_null() {
        return ROUTING_SCOPE_STALE_LEASE;
    }
    // SAFETY: caller provides a live fixed-size lease record for this call.
    let identity: DeviceLeaseIdentity = unsafe { (*lease).into() };
    let c_lease = CDeviceLease {
        device_heap: identity.device_heap,
        internal_address: identity.internal_address,
        serial: identity.serial,
    };
    let mut scope = std::mem::MaybeUninit::<StreamLeaseScope>::uninit();
    // SAFETY: both records stay valid through the synchronous exact-generation lookup.
    if !unsafe { tsfp_audio_stream_lease_scope_rust(stream, &c_lease, scope.as_mut_ptr()) } {
        return ROUTING_SCOPE_STALE_LEASE;
    }
    // SAFETY: successful scope lookup initialized the complete Copy record.
    let scope = unsafe { scope.assume_init() };
    if !routing_scope_supported(scope.flags, scope.channels, completion_available != 0) {
        return ROUTING_SCOPE_POLICY_REFUSED;
    }
    let leases = routing_leases_lock();
    if leases
        .get(&stream)
        .is_none_or(|state| state.serial != identity.serial)
    {
        return ROUTING_SCOPE_STALE_LEASE;
    }
    ROUTING_SCOPE_ACCEPTED
}

pub type GuestSpansSnapshot = unsafe extern "C" fn(
    context: u32,
    spans: *mut CGuestSpan,
    capacity: u32,
    count_out: *mut u32,
) -> bool;

pub type StreamRoutingApply = unsafe extern "C" fn(
    context: *mut c_void,
    stream: u32,
    serial: u64,
    candidate: *const CRouting,
) -> bool;

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CRouting {
    pub volume: u32,
    pub headroom: u32,
    pub aggregate: u32,
    pub count: u32,
    pub bins: [u8; 8],
    pub bin_volume: [u32; 32],
}

type StreamRoutingNote =
    unsafe extern "C" fn(stream: u32, serial: u64, routing: *const CRouting) -> bool;

/// Forward the Rust transaction's synchronous apply point to the active host note callback.
/// The C adapter passes the address of its callback variable for the duration of this call.
///
/// # Safety
/// `context` must point to a live nullable `StreamRoutingNote` and `routing` to a live routing
/// snapshot for the duration of the callback.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_stream_routing_apply_note_rust(
    context: *mut c_void,
    stream: u32,
    serial: u64,
    routing: *const CRouting,
) -> bool {
    let Some(note) =
        (unsafe { context.cast::<Option<StreamRoutingNote>>().as_ref() }).and_then(|note| *note)
    else {
        return false;
    };
    if routing.is_null() {
        return false;
    }
    // SAFETY: the caller keeps both callback context and routing snapshot live synchronously.
    unsafe { note(stream, serial, routing) }
}

impl From<CRouting> for StreamRouting {
    fn from(value: CRouting) -> Self {
        Self {
            volume: value.volume,
            headroom: value.headroom,
            aggregate: value.aggregate,
            count: value.count,
            bins: value.bins,
            bin_volume: value.bin_volume,
        }
    }
}

impl From<StreamRouting> for CRouting {
    fn from(value: StreamRouting) -> Self {
        Self {
            volume: value.volume,
            headroom: value.headroom,
            aggregate: value.aggregate,
            count: value.count,
            bins: value.bins,
            bin_volume: value.bin_volume,
        }
    }
}

impl From<CRouting> for tsfp_audio::contract::Routing {
    fn from(value: CRouting) -> Self {
        Self {
            volume: value.volume,
            headroom: value.headroom,
            aggregate: value.aggregate,
            count: value.count,
            bins: value.bins,
            bin_volume: value.bin_volume,
        }
    }
}

/// Reserve Rust-owned routing state while the C adapter still holds its stream lock and before
/// the device lease can publish a guest-visible stream pointer. The entry is bound to the
/// committed lease serial by `bind_rust` in the lease-finalize callback.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_stream_routing_prepare_rust(stream: u32) -> bool {
    if stream == 0 {
        return false;
    }
    let mut leases = routing_leases_lock();
    if leases.contains_key(&stream) || leases.try_reserve(1).is_err() {
        return false;
    }
    leases.insert(
        stream,
        StreamLeaseState {
            serial: 0,
            routing: StreamRouting::default(),
            frequency: FrequencyTransaction::default(),
        },
    );
    true
}

/// Bind a prepared routing record to the committed stream lease. This only updates the
/// preallocated entry and cannot allocate in the lease-finalize callback.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_stream_routing_bind_rust(stream: u32, serial: u64) -> bool {
    if stream == 0 || serial == 0 {
        return false;
    }
    let mut leases = routing_leases_lock();
    let Some(state) = leases.get_mut(&stream) else {
        return false;
    };
    if state.serial != 0 {
        return false;
    }
    state.serial = serial;
    true
}

/// Cancel an unpublished prepared entry after the C device-lease transaction aborts.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_stream_routing_cancel_rust(stream: u32) -> bool {
    let mut leases = routing_leases_lock();
    if leases.get(&stream).is_some_and(|state| state.serial == 0) {
        leases.remove(&stream);
        true
    } else {
        false
    }
}

/// Retire the routing mirror only for its exact lease generation.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_stream_routing_retire_rust(stream: u32, serial: u64) -> bool {
    let mut leases = routing_leases_lock();
    if leases
        .get(&stream)
        .is_some_and(|state| state.serial == serial && serial != 0)
    {
        leases.remove(&stream);
        true
    } else {
        false
    }
}

/// Copy a live lease's Rust-owned routing state into a transient C adapter record.
/// A stale stream address or generation is never treated as a valid sidecar.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_stream_routing_load_rust(
    stream: u32,
    serial: u64,
    output: *mut CRouting,
) -> bool {
    let Some(output) = (unsafe { output.as_mut() }) else {
        return false;
    };
    let leases = routing_leases_lock();
    let Some(state) = leases.get(&stream) else {
        return false;
    };
    if state.serial != serial || serial == 0 {
        return false;
    }
    *output = state.routing.clone().into();
    true
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct CRoutePair {
    pub bin: u32,
    pub volume: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CGuestSpan {
    pub address: u32,
    pub bytes: u32,
}

impl Default for CGuestSpan {
    fn default() -> Self {
        Self {
            address: 0,
            bytes: 0,
        }
    }
}

fn guest_spans_overlap(address: u32, bytes: u32, protected: &[CGuestSpan]) -> bool {
    let start = u64::from(address);
    let end = start + u64::from(bytes);
    protected.iter().any(|span| {
        start < u64::from(span.address) + u64::from(span.bytes) && u64::from(span.address) < end
    })
}

/// Apply the Rust-host stream adapter's half-open guest span alias policy to a host-owned
/// snapshot of protected ranges. C retains address discovery and lifecycle ownership.
/// Invalid host snapshots fail closed as an alias.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_guest_span_aliases_rust(
    address: u32,
    bytes: u32,
    protected_spans: *const CGuestSpan,
    protected_count: u32,
) -> bool {
    const MAX_PROTECTED_SPANS: u32 = 65_536;
    if protected_count > MAX_PROTECTED_SPANS || (protected_count != 0 && protected_spans.is_null())
    {
        return true;
    }
    if protected_count == 0 {
        return false;
    }
    // SAFETY: the synchronous C adapter supplies exactly protected_count initialized span rows.
    let protected =
        unsafe { std::slice::from_raw_parts(protected_spans, protected_count as usize) };
    guest_spans_overlap(address, bytes, protected)
}

fn pairs_from_raw(pairs: *const CRoutePair, count: u32, limit: usize) -> Option<Vec<(u32, u32)>> {
    let count = usize::try_from(count).ok()?;
    if count > limit {
        return None;
    }
    if count == 0 {
        return Some(Vec::new());
    }
    if pairs.is_null() {
        return None;
    }
    // SAFETY: callers provide the synchronous descriptor snapshot; count is bounded above.
    let pairs = unsafe { std::slice::from_raw_parts(pairs, count) };
    Some(pairs.iter().map(|pair| (pair.bin, pair.volume)).collect())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_stream_routing_init_rust(state: *mut CRouting) {
    // SAFETY: the C adapter passes its live candidate record for this synchronous operation.
    let Some(state) = (unsafe { state.as_mut() }) else {
        return;
    };
    *state = StreamRouting::default().into();
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_stream_routing_volume_rust(state: *mut CRouting, volume: u32) {
    // SAFETY: the C adapter passes its live candidate record for this synchronous operation.
    let Some(state) = (unsafe { state.as_mut() }) else {
        return;
    };
    let mut routing = StreamRouting::from(*state);
    routing.set_volume(volume);
    *state = routing.into();
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_stream_routing_headroom_rust(
    state: *mut CRouting,
    headroom: u32,
) {
    // SAFETY: the C adapter passes its live candidate record for this synchronous operation.
    let Some(state) = (unsafe { state.as_mut() }) else {
        return;
    };
    let mut routing = StreamRouting::from(*state);
    routing.set_headroom(headroom);
    *state = routing.into();
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_stream_routing_bins_rust(
    state: *mut CRouting,
    pairs: *const CRoutePair,
    count: u32,
    defaults: bool,
) -> bool {
    // SAFETY: the C adapter passes its live candidate record for this synchronous operation.
    let Some(state) = (unsafe { state.as_mut() }) else {
        return false;
    };
    let values = if defaults {
        Some(Vec::new())
    } else {
        pairs_from_raw(pairs, count, 8)
    };
    let Some(values) = values else {
        return false;
    };
    let mut routing = StreamRouting::from(*state);
    if !routing.set_bins(&values, defaults) {
        return false;
    }
    *state = routing.into();
    true
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_stream_routing_volumes_rust(
    state: *mut CRouting,
    pairs: *const CRoutePair,
    count: u32,
) -> bool {
    // SAFETY: the C adapter passes its live candidate record for this synchronous operation.
    let Some(state) = (unsafe { state.as_mut() }) else {
        return false;
    };
    let Some(values) = pairs_from_raw(pairs, count, 32) else {
        return false;
    };
    let mut routing = StreamRouting::from(*state);
    if !routing.set_bin_volumes(&values) {
        return false;
    }
    *state = routing.into();
    true
}

/// Snapshot a guest mix-bin descriptor and its optional route pairs through the fault-safe Rust
/// guest-memory seam, then apply them to a candidate routing record. The C adapter supplies a
/// synchronous guard backed by a snapshot of host-owned protected ranges; Rust owns the overlap
/// policy and read ordering and never reads the descriptor's pair pointer when count is zero. Rust commits the candidate to
/// the stream sidecar only after the timestamped PCM routing transaction accepts it.
///
/// Status is 0 for accepted, 1 for an invalid/unreadable guest span, and 2 for a rejected route.
/// A zero count is an empty transaction and never reads the descriptor's pointer word.
///
/// # Safety
/// `state` points to one writable candidate record for this synchronous call. `descriptor_address`
/// is a guest address owned by `host_pid`; guest memory is read through `process_vm_readv`, never
/// dereferenced. `span_allowed` is a valid synchronous C callback which does not retain pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_stream_routing_guest_descriptor_rust(
    host_pid: i32,
    state: *mut CRouting,
    descriptor_address: u32,
    maximum: u32,
    volumes: bool,
    context: u32,
    spans_snapshot: GuestSpansSnapshot,
) -> u32 {
    let Some(state) = (unsafe { state.as_mut() }) else {
        return GUEST_PAIRS_READ_REFUSED;
    };
    let memory = FrameGuestMemory::new(host_pid);
    let Some(protected_spans) = (unsafe { protected_guest_spans(context, spans_snapshot) }) else {
        return GUEST_PAIRS_READ_REFUSED;
    };
    // Preserve the adapter's alias-before-read ordering for every guest span.
    if guest_spans_overlap(descriptor_address, 4, &protected_spans) {
        return GUEST_PAIRS_READ_REFUSED;
    }
    let Some(count) = memory.read_u32(descriptor_address) else {
        return GUEST_PAIRS_READ_REFUSED;
    };
    let pair_address = if count == 0 {
        0
    } else {
        let Some(pointer_word) = descriptor_address.checked_add(4) else {
            return GUEST_PAIRS_READ_REFUSED;
        };
        if guest_spans_overlap(pointer_word, 4, &protected_spans) {
            return GUEST_PAIRS_READ_REFUSED;
        }
        let Some(pair_address) = memory.read_u32(pointer_word) else {
            return GUEST_PAIRS_READ_REFUSED;
        };
        pair_address
    };
    let Ok(count_usize) = usize::try_from(count) else {
        return GUEST_PAIRS_READ_REFUSED;
    };
    // The original adapter reads the pointer word for any non-zero count, then checks this bound
    // before validating or copying the pair span.
    if count > maximum {
        return GUEST_PAIRS_READ_REFUSED;
    }
    let Some(byte_count) = count_usize.checked_mul(8) else {
        return GUEST_PAIRS_READ_REFUSED;
    };
    let Ok(byte_count_u32) = u32::try_from(byte_count) else {
        return GUEST_PAIRS_READ_REFUSED;
    };
    if count != 0 {
        let Some(end) = u64::from(pair_address).checked_add(byte_count as u64) else {
            return GUEST_PAIRS_READ_REFUSED;
        };
        if pair_address == 0
            || end > (u64::from(u32::MAX) + 1)
            || guest_spans_overlap(pair_address, byte_count_u32, &protected_spans)
        {
            return GUEST_PAIRS_READ_REFUSED;
        }
    }
    let Some(bytes) = memory.read_bytes(pair_address, byte_count) else {
        return GUEST_PAIRS_READ_REFUSED;
    };
    let values: Vec<(u32, u32)> = bytes
        .chunks_exact(8)
        .map(|pair| {
            (
                u32::from_le_bytes(pair[..4].try_into().expect("fixed pair word")),
                u32::from_le_bytes(pair[4..].try_into().expect("fixed pair word")),
            )
        })
        .collect();
    let mut routing = StreamRouting::from(*state);
    let accepted = if volumes {
        routing.set_bin_volumes(&values)
    } else {
        routing.set_bins(&values, false)
    };
    if !accepted {
        return GUEST_PAIRS_POLICY_REFUSED;
    }
    *state = routing.into();
    GUEST_PAIRS_ACCEPTED
}

unsafe fn protected_guest_spans(
    context: u32,
    snapshot: GuestSpansSnapshot,
) -> Option<Vec<CGuestSpan>> {
    const MAX_PROTECTED_SPANS: u32 = 65_536;
    let mut count = 0;
    if !unsafe { snapshot(context, std::ptr::null_mut(), 0, &mut count) }
        || count > MAX_PROTECTED_SPANS
    {
        return None;
    }
    let count_usize = usize::try_from(count).ok()?;
    let mut spans = Vec::new();
    spans.try_reserve_exact(count_usize).ok()?;
    spans.resize(count_usize, CGuestSpan::default());
    let mut actual = 0;
    if !unsafe { snapshot(context, spans.as_mut_ptr(), count, &mut actual) } || actual != count {
        return None;
    }
    Some(spans)
}

/// Apply a candidate route to timestamped PCM history, then publish the Rust-owned lease sidecar
/// only when the host operation accepts it. The C callback is the narrow clock/completion
/// boundary; it must be synchronous and must not re-enter the stream adapter.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_stream_routing_commit_rust(
    candidate: *const CRouting,
    stream: u32,
    serial: u64,
    context: *mut c_void,
    apply: Option<StreamRoutingApply>,
) -> bool {
    if candidate.is_null() || stream == 0 || serial == 0 {
        return false;
    }
    // SAFETY: C supplies a live candidate for this synchronous call. Copy it so the callback
    // cannot mutate the transaction input through another alias.
    let candidate = unsafe { *candidate };
    {
        let leases = routing_leases_lock();
        if !leases
            .get(&stream)
            .is_some_and(|state| state.serial == serial)
        {
            return false;
        }
    }
    if let Some(apply) = apply {
        // SAFETY: the adapter keeps callback context valid through this synchronous operation.
        if !unsafe { apply(context, stream, serial, &candidate) } {
            return false;
        }
    }
    let mut leases = routing_leases_lock();
    let Some(state) = leases.get_mut(&stream) else {
        return false;
    };
    if state.serial != serial {
        return false;
    }
    state.routing = candidate.into();
    true
}

/// Build a stream-volume route from Rust-owned state, apply the synchronous host callback, and
/// publish the route only when that callback accepts it. The caller serializes this operation
/// with the stream adapter lock; the callback must not re-enter the adapter.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_stream_routing_volume_commit_rust(
    stream: u32,
    serial: u64,
    volume: u32,
    context: *mut c_void,
    apply: Option<StreamRoutingApply>,
) -> bool {
    if stream == 0 || serial == 0 {
        return false;
    }
    let mut candidate = {
        let leases = routing_leases_lock();
        let Some(state) = leases.get(&stream) else {
            return false;
        };
        if state.serial != serial {
            return false;
        }
        state.routing.clone()
    };
    candidate.set_volume(volume);
    let candidate: CRouting = candidate.into();
    // SAFETY: this function has the same synchronous callback contract as the generic commit.
    unsafe { tsfp_audio_stream_routing_commit_rust(&candidate, stream, serial, context, apply) }
}

/// Apply one checked stream-routing setter to the exact Rust-owned lease and publish it only
/// after the synchronous host callback accepts. Descriptor reads retain the guest adapter's
/// alias-before-read ordering. The caller serializes this call with stream HLE handlers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_stream_routing_update_rust(
    stream: u32,
    serial: u64,
    operation: u32,
    argument: u32,
    host_pid: i32,
    descriptor_address: u32,
    internal: u32,
    context: *mut c_void,
    apply: Option<StreamRoutingApply>,
    spans_snapshot: Option<GuestSpansSnapshot>,
) -> u32 {
    if stream == 0 || serial == 0 {
        return GUEST_PAIRS_READ_REFUSED;
    }
    let mut candidate = {
        let leases = routing_leases_lock();
        let Some(state) = leases.get(&stream) else {
            return GUEST_PAIRS_READ_REFUSED;
        };
        if state.serial != serial {
            return GUEST_PAIRS_READ_REFUSED;
        }
        state.routing.clone()
    };
    let admitted = match operation {
        ROUTING_UPDATE_HEADROOM => {
            candidate.set_headroom(argument);
            true
        }
        ROUTING_UPDATE_BINS if argument == 0 => candidate.set_bins(&[], true),
        ROUTING_UPDATE_BINS | ROUTING_UPDATE_VOLUMES => {
            let maximum = if operation == ROUTING_UPDATE_BINS {
                ROUTE_SLOTS as u32
            } else {
                ROUTE_BINS as u32
            };
            let Some(spans_snapshot) = spans_snapshot else {
                return GUEST_PAIRS_READ_REFUSED;
            };
            let mut snapshot = CRouting::from(candidate);
            // SAFETY: the caller keeps the process identity and protected-span callback valid for
            // this synchronous guest-memory snapshot.
            let status = unsafe {
                tsfp_audio_stream_routing_guest_descriptor_rust(
                    host_pid,
                    &mut snapshot,
                    descriptor_address,
                    maximum,
                    operation == ROUTING_UPDATE_VOLUMES,
                    internal,
                    spans_snapshot,
                )
            };
            if status == GUEST_PAIRS_READ_REFUSED {
                return status;
            }
            if status == GUEST_PAIRS_POLICY_REFUSED {
                return status;
            }
            candidate = snapshot.into();
            status == GUEST_PAIRS_ACCEPTED
        }
        _ => return GUEST_PAIRS_POLICY_REFUSED,
    };
    if !admitted {
        return GUEST_PAIRS_POLICY_REFUSED;
    }
    let candidate: CRouting = candidate.into();
    // SAFETY: the adapter keeps callback context valid through this synchronous operation.
    if !unsafe { tsfp_audio_stream_routing_commit_rust(&candidate, stream, serial, context, apply) }
    {
        return ROUTING_UPDATE_APPLY_REFUSED;
    }
    GUEST_PAIRS_ACCEPTED
}

pub type StreamFrequencyApply = unsafe extern "C" fn(
    context: *mut c_void,
    stream: u32,
    serial: u64,
    old_hz: u32,
    new_hz: u32,
) -> bool;

pub type StreamFrequencyFormatApply = unsafe extern "C" fn(
    context: *mut c_void,
    stream: u32,
    serial: u64,
    old_hz: u32,
    new_hz: u32,
    format_address: u32,
) -> bool;

/// Apply the measured SetFrequency conversion and publish Rust-owned per-lease rate, pitch and
/// count only after the synchronous completion/PCM transaction accepts the candidate. The
/// callback is the remaining C guest-completion seam and must not re-enter this registry. C
/// serializes a stream call while holding the object lock; this entry checks that the same
/// lease/count/rate remains before publishing. Refusal leaves Rust state and `pitch_out` unchanged.
///
/// # Safety
/// `pitch_out` is writable for one `i32`; `context` remains valid through `apply`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_stream_frequency_commit_rust(
    stream: u32,
    serial: u64,
    accepted_sets: u32,
    old_hz: u32,
    new_hz: u32,
    control_word: u16,
    context: *mut c_void,
    apply: Option<StreamFrequencyApply>,
    pitch_out: *mut i32,
) -> bool {
    if stream == 0 || serial == 0 || pitch_out.is_null() || accepted_sets == u32::MAX {
        return false;
    }
    let Some(apply) = apply else {
        return false;
    };
    let Some(pitch) = frequency_pitch(new_hz, control_word) else {
        return false;
    };
    if !(8000..=96000).contains(&old_hz) {
        return false;
    }
    {
        let leases = routing_leases_lock();
        if !leases.get(&stream).is_some_and(|state| {
            state.serial == serial
                && state.frequency.accepted_sets == accepted_sets
                && (state.frequency.current_hz == 0 || state.frequency.current_hz == old_hz)
        }) {
            return false;
        }
    }
    // SAFETY: the C stream adapter keeps the callback/context alive and serializes this lease.
    if !unsafe { apply(context, stream, serial, old_hz, new_hz) } {
        return false;
    }
    let mut leases = routing_leases_lock();
    let Some(state) = leases.get_mut(&stream) else {
        return false;
    };
    if state.serial != serial
        || state.frequency.accepted_sets != accepted_sets
        || (state.frequency.current_hz != 0 && state.frequency.current_hz != old_hz)
    {
        return false;
    }
    state.frequency.accepted_sets += 1;
    state.frequency.current_hz = new_hz;
    state.frequency.pitch = pitch;
    // SAFETY: checked non-null; the adapter supplies writable storage for one pitch value.
    unsafe { pitch_out.write(pitch) };
    true
}

/// Read the current lease-bound frequency transaction without keeping a C payload copy.
/// The output remains unchanged when the stream or generation does not match.
///
/// # Safety
/// `state_out` points to writable storage for one `CStreamFrequencyState`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_stream_frequency_state_rust(
    stream: u32,
    serial: u64,
    state_out: *mut CStreamFrequencyState,
) -> bool {
    if stream == 0 || serial == 0 {
        return false;
    }
    // SAFETY: the synchronous C adapter supplies one writable output record.
    let Some(state_out) = (unsafe { state_out.as_mut() }) else {
        return false;
    };
    let leases = routing_leases_lock();
    let Some(state) = leases.get(&stream).filter(|state| state.serial == serial) else {
        return false;
    };
    *state_out = CStreamFrequencyState {
        accepted_sets: state.frequency.accepted_sets,
        current_hz: state.frequency.current_hz,
        pitch: state.frequency.pitch,
    };
    true
}

/// Apply a previously admitted SetFormat rate reset and keep Rust's stream frequency sidecar
/// authoritative. When present, the C callback retains packet completion and guest-word writes;
/// without it, the measured logical rate and pitch are recorded for a headless host with no PCM
/// runtime. Rust publishes its new rate and pitch only after the callback accepts.
///
/// # Safety
/// `pitch_out` is writable for one `i32`; when `apply` is present, `context` remains valid through it.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_stream_frequency_format_commit_rust(
    stream: u32,
    serial: u64,
    accepted_sets: u32,
    old_hz: u32,
    new_hz: u32,
    control_word: u16,
    format_address: u32,
    context: *mut c_void,
    apply: Option<StreamFrequencyFormatApply>,
    pitch_out: *mut i32,
) -> bool {
    if stream == 0 || serial == 0 || pitch_out.is_null() {
        return false;
    }
    let Some(pitch) = frequency_pitch(new_hz, control_word) else {
        return false;
    };
    if !(8000..=96000).contains(&old_hz) {
        return false;
    }
    {
        let leases = routing_leases_lock();
        if !leases.get(&stream).is_some_and(|state| {
            state.serial == serial
                && state.frequency.accepted_sets == accepted_sets
                && (state.frequency.current_hz == 0 || state.frequency.current_hz == old_hz)
        }) {
            return false;
        }
    }
    if let Some(apply) = apply {
        // SAFETY: the adapter retains the callback/context and serializes this stream lease.
        if !unsafe { apply(context, stream, serial, old_hz, new_hz, format_address) } {
            return false;
        }
    }
    let mut leases = routing_leases_lock();
    let Some(state) = leases.get_mut(&stream) else {
        return false;
    };
    if state.serial != serial
        || state.frequency.accepted_sets != accepted_sets
        || (state.frequency.current_hz != 0 && state.frequency.current_hz != old_hz)
    {
        return false;
    }
    state.frequency.current_hz = new_hz;
    state.frequency.pitch = pitch;
    // SAFETY: checked non-null; the adapter supplies writable storage for one pitch value.
    unsafe { pitch_out.write(pitch) };
    true
}

/// Compute the measured SetFrequency pitch value for Rust-host SetFormat handling.
///
/// # Safety
/// `pitch_out` is writable for one `i32`; refusal leaves it unchanged.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_stream_frequency_pitch_rust(
    hertz: u32,
    control_word: u16,
    pitch_out: *mut i32,
) -> bool {
    let Some(pitch_out) = (unsafe { pitch_out.as_mut() }) else {
        return false;
    };
    let Some(pitch) = frequency_pitch(hertz, control_word) else {
        return false;
    };
    *pitch_out = pitch;
    true
}

/// Decide whether a measured SetFrequency stream shape has its required startup state.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_stream_frequency_admit_rust(
    stereo: u32,
    spatial: u32,
    completion: u32,
    frequency_handler: u32,
    volume_seen: u32,
    discontinuity_seen: u32,
) -> bool {
    completion != 0
        && frequency_handler != 0
        && (stereo != 0 || spatial != 0)
        && volume_seen != 0
        && discontinuity_seen != 0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_stream_routing_attenuation_rust(
    state: *const CRouting,
    slot: u32,
) -> u16 {
    // SAFETY: the C adapter passes its live immutable routing record for this synchronous call.
    let Some(state) = (unsafe { state.as_ref() }) else {
        return 4095;
    };
    StreamRouting::from(*state).attenuation(slot)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frequency_admission_requires_measured_started_stream_state() {
        assert!(tsfp_audio_stream_frequency_admit_rust(1, 0, 1, 1, 1, 1));
        assert!(tsfp_audio_stream_frequency_admit_rust(0, 1, 1, 1, 1, 1));
        for args in [
            [0, 0, 1, 1, 1, 1],
            [1, 0, 0, 1, 1, 1],
            [1, 0, 1, 0, 1, 1],
            [1, 0, 1, 1, 0, 1],
            [1, 0, 1, 1, 1, 0],
        ] {
            assert!(!tsfp_audio_stream_frequency_admit_rust(
                args[0], args[1], args[2], args[3], args[4], args[5]
            ));
        }
    }

    unsafe extern "C" fn accept_routing_note(
        stream: u32,
        serial: u64,
        routing: *const CRouting,
    ) -> bool {
        let Some(routing) = (unsafe { routing.as_ref() }) else {
            return false;
        };
        stream == 0x1234 && serial == 9 && routing.volume == 17
    }

    #[test]
    fn rust_apply_bridge_calls_only_the_supplied_live_note() {
        let note = Some(accept_routing_note as StreamRoutingNote);
        let routing = CRouting {
            volume: 17,
            ..StreamRouting::default().into()
        };
        // SAFETY: note and routing remain live throughout the synchronous callback.
        assert!(unsafe {
            tsfp_audio_stream_routing_apply_note_rust(
                (&note as *const Option<StreamRoutingNote>)
                    .cast_mut()
                    .cast(),
                0x1234,
                9,
                &routing,
            )
        });
        assert!(!unsafe {
            tsfp_audio_stream_routing_apply_note_rust(std::ptr::null_mut(), 0x1234, 9, &routing)
        });
    }

    #[test]
    fn routing_scope_admission_requires_completion_and_nonspatial_stereo() {
        assert!(routing_scope_supported(0, 2, true));
        assert!(!routing_scope_supported(0, 2, false));
        assert!(!routing_scope_supported(0x10, 2, true));
        assert!(!routing_scope_supported(0, 1, true));
    }

    #[test]
    fn c_routing_abi_owns_all_mutations_and_preserves_transactional_refusals() {
        assert_eq!(std::mem::size_of::<CRouting>(), 152);
        assert_eq!(std::mem::size_of::<CRoutePair>(), 8);
        assert_eq!(std::mem::size_of::<CGuestSpan>(), 8);

        let mut state = CRouting {
            volume: 0,
            headroom: 0,
            aggregate: 0,
            count: 0,
            bins: [0; 8],
            bin_volume: [0; 32],
        };
        // SAFETY: local records and pair arrays satisfy each FFI function's synchronous contract.
        unsafe { tsfp_audio_stream_routing_init_rust(&mut state) };
        let mut expected = StreamRouting::default();
        assert_eq!(StreamRouting::from(state), expected);

        unsafe {
            tsfp_audio_stream_routing_volume_rust(&mut state, u32::MAX);
            tsfp_audio_stream_routing_headroom_rust(&mut state, 1200);
        }
        expected.set_volume(u32::MAX);
        expected.set_headroom(1200);

        let routes = [
            CRoutePair {
                bin: 0,
                volume: u32::MAX,
            },
            CRoutePair { bin: 1, volume: 4 },
            CRoutePair { bin: 0, volume: 9 },
        ];
        assert!(unsafe {
            tsfp_audio_stream_routing_bins_rust(&mut state, routes.as_ptr(), 3, false)
        });
        assert!(expected.set_bins(&[(0, u32::MAX), (1, 4), (0, 9)], false));

        let before = state;
        let invalid = [CRoutePair { bin: 6, volume: 1 }];
        assert!(!unsafe {
            tsfp_audio_stream_routing_bins_rust(&mut state, invalid.as_ptr(), 1, false)
        });
        assert_eq!(state, before);
        assert_eq!(StreamRouting::from(state), expected);
        assert_eq!(
            unsafe { tsfp_audio_stream_routing_attenuation_rust(&state, 0) },
            expected.attenuation(0)
        );

        let bin_volumes = [CRoutePair {
            bin: 31,
            volume: u32::MAX,
        }];
        assert!(unsafe {
            tsfp_audio_stream_routing_volumes_rust(&mut state, bin_volumes.as_ptr(), 1)
        });
        assert!(expected.set_bin_volumes(&[(31, u32::MAX)]));
        assert_eq!(StreamRouting::from(state), expected);
        let before = state;
        let invalid_volume = [CRoutePair { bin: 32, volume: 0 }];
        assert!(!unsafe {
            tsfp_audio_stream_routing_volumes_rust(&mut state, invalid_volume.as_ptr(), 1)
        });
        assert_eq!(state, before);

        assert!(unsafe {
            tsfp_audio_stream_routing_bins_rust(&mut state, std::ptr::null(), 0, true)
        });
        assert!(expected.set_bins(&[], true));
        assert_eq!(StreamRouting::from(state), expected);
    }

    #[test]
    fn rust_guest_span_policy_uses_half_open_ranges_and_fails_closed_on_bad_snapshot() {
        let protected = [CGuestSpan {
            address: 0x1000,
            bytes: 40,
        }];
        assert!(!guest_spans_overlap(0x0ffc, 4, &protected));
        assert!(guest_spans_overlap(0x0fff, 2, &protected));
        assert!(guest_spans_overlap(0x1000, 1, &protected));
        assert!(!guest_spans_overlap(0x1028, 4, &protected));
        assert!(!guest_spans_overlap(0x1000, 0, &protected));

        // SAFETY: `protected` is a valid immutable span snapshot for this synchronous call.
        assert!(unsafe {
            tsfp_audio_guest_span_aliases_rust(
                0x1027,
                2,
                protected.as_ptr(),
                protected.len() as u32,
            )
        });
        // A missing snapshot cannot authorize a guest read.
        assert!(unsafe { tsfp_audio_guest_span_aliases_rust(0x2000, 4, std::ptr::null(), 1) });
        // SAFETY: the count cap is checked before the supplied one-row pointer is dereferenced.
        assert!(unsafe {
            tsfp_audio_guest_span_aliases_rust(0x2000, 4, protected.as_ptr(), 65_537)
        });
    }

    unsafe extern "C" fn allow_guest_spans(
        _: u32,
        _: *mut CGuestSpan,
        _: u32,
        count_out: *mut u32,
    ) -> bool {
        if count_out.is_null() {
            return false;
        }
        unsafe { count_out.write(0) };
        true
    }

    unsafe extern "C" fn reject_guest_spans(
        _: u32,
        spans: *mut CGuestSpan,
        capacity: u32,
        count_out: *mut u32,
    ) -> bool {
        if count_out.is_null() {
            return false;
        }
        unsafe { count_out.write(1) };
        if capacity == 0 {
            return spans.is_null();
        }
        if capacity < 1 || spans.is_null() {
            return false;
        }
        unsafe {
            spans.write(CGuestSpan {
                address: 0,
                bytes: u32::MAX,
            })
        };
        true
    }

    unsafe extern "C" fn changing_guest_spans(
        _: u32,
        spans: *mut CGuestSpan,
        capacity: u32,
        count_out: *mut u32,
    ) -> bool {
        if count_out.is_null() {
            return false;
        }
        if capacity == 0 {
            unsafe { count_out.write(1) };
            return spans.is_null();
        }
        if capacity != 1 || spans.is_null() {
            return false;
        }
        unsafe {
            spans.write(CGuestSpan {
                address: 0x1000,
                bytes: 4,
            });
            count_out.write(2);
        }
        true
    }

    unsafe extern "C" fn oversized_guest_spans(
        _: u32,
        spans: *mut CGuestSpan,
        capacity: u32,
        count_out: *mut u32,
    ) -> bool {
        if count_out.is_null() || !spans.is_null() || capacity != 0 {
            return false;
        }
        unsafe { count_out.write(65_537) };
        true
    }

    #[test]
    fn protected_guest_span_snapshot_fails_closed_on_unstable_or_oversized_shape() {
        assert!(unsafe { protected_guest_spans(0, changing_guest_spans) }.is_none());
        assert!(unsafe { protected_guest_spans(0, oversized_guest_spans) }.is_none());
    }

    struct RouteApplyProbe {
        before: CRouting,
        calls: u32,
        accepted: bool,
        seen: Option<CRouting>,
    }

    unsafe extern "C" fn inspect_route_apply(
        context: *mut c_void,
        stream: u32,
        serial: u64,
        candidate: *const CRouting,
    ) -> bool {
        // SAFETY: the test passes a live RouteApplyProbe as callback context.
        let probe = unsafe { &mut *context.cast::<RouteApplyProbe>() };
        // SAFETY: the transaction passes a pointer to its local candidate copy.
        let candidate = unsafe { *candidate };
        assert_eq!(stream, 0x1234);
        assert_eq!(serial, 0x1_0000_0002);
        // The old Rust-owned sidecar remains visible until PCM history accepts the candidate.
        let mut visible = CRouting {
            volume: 0,
            headroom: 0,
            aggregate: 0,
            count: 0,
            bins: [0; 8],
            bin_volume: [0; 32],
        };
        assert!(unsafe { tsfp_audio_stream_routing_load_rust(stream, serial, &mut visible) });
        assert_eq!(visible, probe.before);
        probe.calls += 1;
        probe.seen = Some(candidate);
        probe.accepted
    }

    #[test]
    fn routing_commit_applies_before_publication_and_refusal_rolls_back() {
        assert!(tsfp_audio_stream_routing_prepare_rust(0x1234));
        assert!(tsfp_audio_stream_routing_bind_rust(0x1234, 0x1_0000_0002));
        let mut before = CRouting {
            volume: 0,
            headroom: 0,
            aggregate: 0,
            count: 0,
            bins: [0; 8],
            bin_volume: [0; 32],
        };
        assert!(unsafe { tsfp_audio_stream_routing_load_rust(0x1234, 0x1_0000_0002, &mut before) });
        let mut routing = StreamRouting::from(before);
        routing.set_headroom(800);
        let candidate: CRouting = routing.into();
        let mut probe = RouteApplyProbe {
            before,
            calls: 0,
            accepted: false,
            seen: None,
        };

        assert!(!unsafe {
            tsfp_audio_stream_routing_commit_rust(
                &candidate,
                0x1234,
                0x1_0000_0002,
                (&mut probe as *mut RouteApplyProbe).cast(),
                Some(inspect_route_apply),
            )
        });
        let mut state = before;
        assert!(unsafe { tsfp_audio_stream_routing_load_rust(0x1234, 0x1_0000_0002, &mut state) });
        assert_eq!(state, before);
        assert_eq!(probe.calls, 1);
        assert_eq!(probe.seen, Some(candidate));

        probe.accepted = true;
        assert!(unsafe {
            tsfp_audio_stream_routing_commit_rust(
                &candidate,
                0x1234,
                0x1_0000_0002,
                (&mut probe as *mut RouteApplyProbe).cast(),
                Some(inspect_route_apply),
            )
        });
        assert!(unsafe { tsfp_audio_stream_routing_load_rust(0x1234, 0x1_0000_0002, &mut state) });
        assert_eq!(state, candidate);
        assert_eq!(probe.calls, 2);

        probe.before = state;
        probe.seen = None;
        let mut volume_expected = StreamRouting::from(state);
        volume_expected.set_volume(0xFFFF_FB6F);
        let volume_expected: CRouting = volume_expected.into();
        probe.accepted = false;
        assert!(!unsafe {
            tsfp_audio_stream_routing_volume_commit_rust(
                0x1234,
                0x1_0000_0002,
                0xFFFF_FB6F,
                (&mut probe as *mut RouteApplyProbe).cast(),
                Some(inspect_route_apply),
            )
        });
        assert!(unsafe { tsfp_audio_stream_routing_load_rust(0x1234, 0x1_0000_0002, &mut state) });
        assert_eq!(state, probe.before);
        probe.accepted = true;
        assert!(unsafe {
            tsfp_audio_stream_routing_volume_commit_rust(
                0x1234,
                0x1_0000_0002,
                0xFFFF_FB6F,
                (&mut probe as *mut RouteApplyProbe).cast(),
                Some(inspect_route_apply),
            )
        });
        assert_eq!(probe.seen, Some(volume_expected));
        assert!(unsafe { tsfp_audio_stream_routing_load_rust(0x1234, 0x1_0000_0002, &mut state) });
        assert_eq!(state, volume_expected);
        assert_eq!(probe.calls, 4);

        probe.before = state;
        let mut headroom_expected = StreamRouting::from(state);
        headroom_expected.set_headroom(1200);
        let headroom_expected: CRouting = headroom_expected.into();
        probe.accepted = false;
        assert_eq!(
            unsafe {
                tsfp_audio_stream_routing_update_rust(
                    0x1234,
                    0x1_0000_0002,
                    ROUTING_UPDATE_HEADROOM,
                    1200,
                    0,
                    0,
                    0,
                    (&mut probe as *mut RouteApplyProbe).cast(),
                    Some(inspect_route_apply),
                    None,
                )
            },
            ROUTING_UPDATE_APPLY_REFUSED
        );
        assert!(unsafe { tsfp_audio_stream_routing_load_rust(0x1234, 0x1_0000_0002, &mut state) });
        assert_eq!(state, probe.before);
        assert_eq!(probe.seen, Some(headroom_expected));
        probe.accepted = true;
        assert_eq!(
            unsafe {
                tsfp_audio_stream_routing_update_rust(
                    0x1234,
                    0x1_0000_0002,
                    ROUTING_UPDATE_HEADROOM,
                    1200,
                    0,
                    0,
                    0,
                    (&mut probe as *mut RouteApplyProbe).cast(),
                    Some(inspect_route_apply),
                    None,
                )
            },
            GUEST_PAIRS_ACCEPTED
        );
        assert!(unsafe { tsfp_audio_stream_routing_load_rust(0x1234, 0x1_0000_0002, &mut state) });
        assert_eq!(state, headroom_expected);

        // Empty SetMixBins restores the default stereo pair without reading a guest pointer.
        probe.before = state;
        let mut bins_expected = StreamRouting::from(state);
        assert!(bins_expected.set_bins(&[], true));
        let bins_expected: CRouting = bins_expected.into();
        assert_eq!(
            unsafe {
                tsfp_audio_stream_routing_update_rust(
                    0x1234,
                    0x1_0000_0002,
                    ROUTING_UPDATE_BINS,
                    0,
                    -1,
                    0,
                    0,
                    (&mut probe as *mut RouteApplyProbe).cast(),
                    Some(inspect_route_apply),
                    None,
                )
            },
            GUEST_PAIRS_ACCEPTED
        );
        assert_eq!(probe.seen, Some(bins_expected));
        assert!(unsafe { tsfp_audio_stream_routing_load_rust(0x1234, 0x1_0000_0002, &mut state) });
        assert_eq!(state, bins_expected);

        let without_pcm: CRouting = StreamRouting::default().into();
        assert!(unsafe {
            tsfp_audio_stream_routing_commit_rust(
                &without_pcm,
                0x1234,
                0x1_0000_0002,
                std::ptr::null_mut(),
                None,
            )
        });
        assert!(unsafe { tsfp_audio_stream_routing_load_rust(0x1234, 0x1_0000_0002, &mut state) });
        assert_eq!(state, without_pcm);
        assert!(tsfp_audio_stream_routing_retire_rust(0x1234, 0x1_0000_0002));
        assert!(!unsafe { tsfp_audio_stream_routing_load_rust(0x1234, 0x1_0000_0002, &mut state) });
    }

    #[test]
    fn routing_lease_registry_cancels_prepared_and_rejects_stale_generations() {
        let stream = 0x2345;
        let mut state: CRouting = StreamRouting::default().into();
        assert!(tsfp_audio_stream_routing_prepare_rust(stream));
        assert!(!unsafe { tsfp_audio_stream_routing_load_rust(stream, 7, &mut state) });
        assert!(tsfp_audio_stream_routing_cancel_rust(stream));
        assert!(!tsfp_audio_stream_routing_cancel_rust(stream));

        assert!(tsfp_audio_stream_routing_prepare_rust(stream));
        assert!(tsfp_audio_stream_routing_bind_rust(stream, 7));
        assert!(!unsafe { tsfp_audio_stream_routing_load_rust(stream, 8, &mut state) });
        assert!(!tsfp_audio_stream_routing_retire_rust(stream, 8));
        assert!(unsafe { tsfp_audio_stream_routing_load_rust(stream, 7, &mut state) });
        assert_eq!(state, StreamRouting::default().into());
        assert!(tsfp_audio_stream_routing_retire_rust(stream, 7));
    }

    struct FrequencyApplyProbe {
        accepted: bool,
        calls: u32,
        args: (u32, u64, u32, u32),
    }

    struct FrequencyFormatApplyProbe {
        accepted: bool,
        calls: u32,
        args: (u32, u64, u32, u32, u32),
    }

    unsafe extern "C" fn inspect_frequency_apply(
        context: *mut c_void,
        stream: u32,
        serial: u64,
        old_hz: u32,
        new_hz: u32,
    ) -> bool {
        // SAFETY: tests pass a live FrequencyApplyProbe for this synchronous callback.
        let probe = unsafe { &mut *context.cast::<FrequencyApplyProbe>() };
        probe.calls += 1;
        probe.args = (stream, serial, old_hz, new_hz);
        probe.accepted
    }

    unsafe extern "C" fn inspect_frequency_format_apply(
        context: *mut c_void,
        stream: u32,
        serial: u64,
        old_hz: u32,
        new_hz: u32,
        format_address: u32,
    ) -> bool {
        // SAFETY: tests pass a live FrequencyFormatApplyProbe for this synchronous callback.
        let probe = unsafe { &mut *context.cast::<FrequencyFormatApplyProbe>() };
        probe.calls += 1;
        probe.args = (stream, serial, old_hz, new_hz, format_address);
        probe.accepted
    }

    #[test]
    fn frequency_transaction_publishes_only_after_completion_pcm_acceptance() {
        let stream = 0xF204_6A01;
        let serial = 0x2_0000_0042;
        assert!(tsfp_audio_stream_routing_prepare_rust(stream));
        assert!(tsfp_audio_stream_routing_bind_rust(stream, serial));

        let mut owned = CStreamFrequencyState {
            accepted_sets: 77,
            current_hz: 88,
            pitch: 99,
        };
        assert!(unsafe { tsfp_audio_stream_frequency_state_rust(stream, serial, &mut owned) });
        assert_eq!(owned, CStreamFrequencyState::default());

        let mut pitch = 0x1234_5678;
        assert!(!unsafe { tsfp_audio_stream_frequency_pitch_rust(7999, 0x027F, &mut pitch) });
        assert_eq!(pitch, 0x1234_5678);

        let mut probe = FrequencyApplyProbe {
            accepted: false,
            calls: 0,
            args: (0, 0, 0, 0),
        };
        assert!(!unsafe {
            tsfp_audio_stream_frequency_commit_rust(
                stream,
                serial,
                0,
                44_100,
                22_050,
                0x027F,
                (&mut probe as *mut FrequencyApplyProbe).cast(),
                Some(inspect_frequency_apply),
                &mut pitch,
            )
        });
        assert_eq!(probe.calls, 1);
        assert_eq!(probe.args, (stream, serial, 44_100, 22_050));
        assert_eq!(pitch, 0x1234_5678);

        probe.accepted = true;
        assert!(unsafe {
            tsfp_audio_stream_frequency_commit_rust(
                stream,
                serial,
                0,
                44_100,
                22_050,
                0x027F,
                (&mut probe as *mut FrequencyApplyProbe).cast(),
                Some(inspect_frequency_apply),
                &mut pitch,
            )
        });
        assert_eq!(pitch, frequency_pitch(22_050, 0x027F).unwrap());
        assert_eq!(probe.calls, 2);
        assert!(unsafe { tsfp_audio_stream_frequency_state_rust(stream, serial, &mut owned) });
        assert_eq!(
            owned,
            CStreamFrequencyState {
                accepted_sets: 1,
                current_hz: 22_050,
                pitch,
            }
        );
        let mut stale = CStreamFrequencyState {
            accepted_sets: 3,
            current_hz: 4,
            pitch: -5,
        };
        assert!(!unsafe { tsfp_audio_stream_frequency_state_rust(stream, serial + 1, &mut stale) });
        assert_eq!(stale.accepted_sets, 3);
        assert_eq!(stale.current_hz, 4);
        assert_eq!(stale.pitch, -5);

        // Rust and C counts must agree; stale generations and invalid control words fail before
        // the callback, leaving the accepted event count unchanged.
        assert!(!unsafe {
            tsfp_audio_stream_frequency_commit_rust(
                stream,
                serial,
                0,
                22_050,
                48_000,
                0x027F,
                (&mut probe as *mut FrequencyApplyProbe).cast(),
                Some(inspect_frequency_apply),
                &mut pitch,
            )
        });
        assert!(!unsafe {
            tsfp_audio_stream_frequency_commit_rust(
                stream,
                serial + 1,
                1,
                22_050,
                48_000,
                0x027F,
                (&mut probe as *mut FrequencyApplyProbe).cast(),
                Some(inspect_frequency_apply),
                &mut pitch,
            )
        });
        assert!(!unsafe {
            tsfp_audio_stream_frequency_commit_rust(
                stream,
                serial,
                1,
                22_050,
                48_000,
                0x017F,
                (&mut probe as *mut FrequencyApplyProbe).cast(),
                Some(inspect_frequency_apply),
                &mut pitch,
            )
        });
        assert_eq!(probe.calls, 2);
        assert!(tsfp_audio_stream_routing_retire_rust(stream, serial));
    }

    #[test]
    fn frequency_format_transaction_keeps_rust_rate_sidecar_in_sync() {
        let stream = 0xF204_6A02;
        let serial = 0x2_0000_0043;
        assert!(tsfp_audio_stream_routing_prepare_rust(stream));
        assert!(tsfp_audio_stream_routing_bind_rust(stream, serial));
        let mut pitch = 0;
        let mut frequency = FrequencyApplyProbe {
            accepted: true,
            calls: 0,
            args: (0, 0, 0, 0),
        };
        assert!(unsafe {
            tsfp_audio_stream_frequency_commit_rust(
                stream,
                serial,
                0,
                44_100,
                22_050,
                0x027F,
                (&mut frequency as *mut FrequencyApplyProbe).cast(),
                Some(inspect_frequency_apply),
                &mut pitch,
            )
        });
        let prior_pitch = frequency_pitch(22_050, 0x027F).unwrap();
        assert_eq!(pitch, prior_pitch);

        let mut format = FrequencyFormatApplyProbe {
            accepted: false,
            calls: 0,
            args: (0, 0, 0, 0, 0),
        };
        assert!(!unsafe {
            tsfp_audio_stream_frequency_format_commit_rust(
                stream,
                serial,
                1,
                22_050,
                48_000,
                0x027F,
                0xA000,
                (&mut format as *mut FrequencyFormatApplyProbe).cast(),
                Some(inspect_frequency_format_apply),
                &mut pitch,
            )
        });
        assert_eq!(pitch, prior_pitch);
        assert_eq!(format.calls, 1);
        assert_eq!(format.args, (stream, serial, 22_050, 48_000, 0xA000));

        format.accepted = true;
        assert!(unsafe {
            tsfp_audio_stream_frequency_format_commit_rust(
                stream,
                serial,
                1,
                22_050,
                48_000,
                0x027F,
                0xA000,
                (&mut format as *mut FrequencyFormatApplyProbe).cast(),
                Some(inspect_frequency_format_apply),
                &mut pitch,
            )
        });
        let format_pitch = frequency_pitch(48_000, 0x027F).unwrap();
        assert_eq!(pitch, format_pitch);
        assert_eq!(format.calls, 2);
        let mut owned = CStreamFrequencyState::default();
        assert!(unsafe { tsfp_audio_stream_frequency_state_rust(stream, serial, &mut owned) });
        assert_eq!(
            owned,
            CStreamFrequencyState {
                accepted_sets: 1,
                current_hz: 48_000,
                pitch: format_pitch,
            }
        );

        // The accepted format rebase is authoritative for the next frequency event.
        assert!(!unsafe {
            tsfp_audio_stream_frequency_commit_rust(
                stream,
                serial,
                1,
                22_050,
                32_000,
                0x027F,
                (&mut frequency as *mut FrequencyApplyProbe).cast(),
                Some(inspect_frequency_apply),
                &mut pitch,
            )
        });
        assert_eq!(frequency.calls, 1);
        assert!(unsafe {
            tsfp_audio_stream_frequency_commit_rust(
                stream,
                serial,
                1,
                48_000,
                32_000,
                0x027F,
                (&mut frequency as *mut FrequencyApplyProbe).cast(),
                Some(inspect_frequency_apply),
                &mut pitch,
            )
        });
        assert_eq!(pitch, frequency_pitch(32_000, 0x027F).unwrap());
        assert!(unsafe { tsfp_audio_stream_frequency_state_rust(stream, serial, &mut owned) });
        assert_eq!(owned.accepted_sets, 2);
        assert_eq!(owned.current_hz, 32_000);
        assert_eq!(owned.pitch, pitch);
        assert!(unsafe {
            tsfp_audio_stream_frequency_format_commit_rust(
                stream,
                serial,
                2,
                32_000,
                44_100,
                0x027F,
                0xF204_6B00,
                std::ptr::null_mut(),
                None,
                &mut pitch,
            )
        });
        assert_eq!(format.calls, 2);
        assert!(unsafe { tsfp_audio_stream_frequency_state_rust(stream, serial, &mut owned) });
        assert_eq!(owned.accepted_sets, 2);
        assert_eq!(owned.current_hz, 44_100);
        assert_eq!(owned.pitch, frequency_pitch(44_100, 0x027F).unwrap());
        assert!(tsfp_audio_stream_routing_retire_rust(stream, serial));
    }

    #[test]
    fn guest_descriptor_and_pairs_are_snapshotted_before_candidate_mutation() {
        let (page, address) = crate::test_guest_memory::map_guest_page();
        let pair_address = address + 8;
        let words = [2u32, pair_address, 0, 120, 1, 240];
        let mut bytes = [0u8; 24];
        for (index, word) in words.into_iter().enumerate() {
            bytes[index * 4..index * 4 + 4].copy_from_slice(&word.to_le_bytes());
        }
        // SAFETY: this test owns a writable guest page and copies exactly 24 bytes into it.
        unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), page, bytes.len()) };

        let mut state = StreamRouting::default().into();
        assert_eq!(
            unsafe {
                tsfp_audio_stream_routing_guest_descriptor_rust(
                    std::process::id() as i32,
                    &mut state,
                    address,
                    8,
                    false,
                    0,
                    allow_guest_spans,
                )
            },
            GUEST_PAIRS_ACCEPTED
        );
        let mut expected = StreamRouting::default();
        assert!(expected.set_bins(&[(0, 120), (1, 240)], false));
        assert_eq!(StreamRouting::from(state), expected);

        let before = state;
        assert_eq!(
            unsafe {
                tsfp_audio_stream_routing_guest_descriptor_rust(
                    std::process::id() as i32,
                    &mut state,
                    address + 16,
                    8,
                    true,
                    0,
                    reject_guest_spans,
                )
            },
            GUEST_PAIRS_READ_REFUSED
        );
        assert_eq!(state, before);

        // Count is at the final mapped word; the following pointer word is unmapped. Zero count
        // must not ask the guard about, or read, that word.
        let zero_descriptor = address + 4092;
        unsafe { page.add(4092).cast::<u32>().write(0) };
        assert_eq!(
            unsafe {
                tsfp_audio_stream_routing_guest_descriptor_rust(
                    std::process::id() as i32,
                    &mut state,
                    zero_descriptor,
                    8,
                    false,
                    0,
                    allow_guest_spans,
                )
            },
            GUEST_PAIRS_ACCEPTED
        );
        expected.set_bins(&[], false);
        assert_eq!(StreamRouting::from(state), expected);

        let before = state;
        // A readable descriptor with an unsupported route is an atomic policy refusal.
        unsafe {
            page.cast::<u32>().write(1);
            page.add(4).cast::<u32>().write(pair_address);
            page.add(8).cast::<u32>().write(6);
            page.add(12).cast::<u32>().write(100);
        }
        assert_eq!(
            unsafe {
                tsfp_audio_stream_routing_guest_descriptor_rust(
                    std::process::id() as i32,
                    &mut state,
                    address,
                    8,
                    false,
                    0,
                    allow_guest_spans,
                )
            },
            GUEST_PAIRS_POLICY_REFUSED
        );
        assert_eq!(state, before);

        // SetMixBinVolumes uses the full DWORD index and shares the same guest descriptor read.
        unsafe {
            page.cast::<u32>().write(1);
            page.add(4).cast::<u32>().write(pair_address);
            page.add(8).cast::<u32>().write(31);
            page.add(12).cast::<u32>().write(u32::MAX);
        }
        assert_eq!(
            unsafe {
                tsfp_audio_stream_routing_guest_descriptor_rust(
                    std::process::id() as i32,
                    &mut state,
                    address,
                    32,
                    true,
                    0,
                    allow_guest_spans,
                )
            },
            GUEST_PAIRS_ACCEPTED
        );
        assert!(expected.set_bin_volumes(&[(31, u32::MAX)]));
        assert_eq!(StreamRouting::from(state), expected);
        crate::test_guest_memory::unmap_guest_page(page);
    }
}
