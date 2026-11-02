// SPDX-License-Identifier: GPL-3.0-or-later
//! C ABI mirrors and imports. The layouts are the `_Static_assert`ed ones of `src/audio/dsound_stream.c`.
use std::ffi::{c_char, c_void};

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Lease {
    pub device_heap: u32,
    pub internal_address: u32,
    pub serial: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct LeaseChild {
    pub heap: u32,
    pub address: u32,
    pub bytes: u32,
}

#[repr(C)]
pub struct LeaseOps {
    pub prepare: unsafe extern "C" fn(*const Lease, *mut c_void, *mut LeaseChild) -> bool,
    pub abort: unsafe extern "C" fn(*mut c_void),
    pub finalize: unsafe extern "C" fn(*const Lease, *mut c_void),
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Scope {
    pub descriptor_address: u32,
    pub format_address: u32,
    pub flags: u32,
    pub max_packets: u32,
    pub sample_rate: u32,
    pub average_bytes_per_second: u32,
    pub channels: u16,
    pub block_align: u16,
    pub descriptor: [u8; 24],
    pub format: [u8; 20],
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FormatState {
    pub bytes: [u8; 20],
    pub sets: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VolumeState {
    pub seen: u32,
    pub value: i32,
}

#[repr(C)]
pub struct VolumeRequest {
    pub argument: u32,
    pub count: u32,
    pub completion_available: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct ControlState {
    pub pause_seen: u32,
    pub pause_mode: u32,
    pub flush_seen: u32,
    pub flush_time_low: u32,
    pub flush_time_high: u32,
    pub flush_flags: u32,
    pub discontinuity_seen: u32,
    pub revision: u32,
}

pub const CONTROL_DISCONTINUITY: u32 = 1;
pub const CONTROL_FLUSH_EX: u32 = 2;
pub const CONTROL_PAUSE: u32 = 3;

#[repr(C)]
pub struct ControlRequest {
    pub action: u32,
    pub argument: u32,
    pub count: u32,
    pub apply: u32,
    pub completion_available: u32,
    pub pause_callback_available: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct SpatialState {
    pub startup_cache_mask: u32,
    pub i3dl2_address: u32,
    pub i3dl2_words: [u32; 9],
    pub min_distance_bits: u32,
    pub position_bits: [u32; 3],
    pub position_sets: u32,
    pub max_distance_seen: u32,
    pub max_distance_bits: u32,
    pub curve_address: u32,
    pub curve_count: u32,
    pub revision: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FrequencyState {
    pub accepted_sets: u32,
    pub current_hz: u32,
    pub pitch: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct GuestSpan {
    pub address: u32,
    pub bytes: u32,
}

/// `dsound_stream_snapshot`: 440 bytes, filled by Rust and copied out opaquely.
#[repr(C, align(8))]
pub struct Snapshot(pub [u8; 440]);

#[repr(C)]
pub struct KernelFrame {
    pub stack_ptr: u32,
    pub stack_limit: u32,
}

pub type PauseNote = unsafe extern "C" fn(u32, u32);
pub type RoutingNote = unsafe extern "C" fn(u32, u64, *const c_void) -> bool;
pub type FrequencyNote = unsafe extern "C" fn(u32, u64, u32, u32) -> bool;
pub type FormatNote = unsafe extern "C" fn(u32, u64, u32, u32, u32) -> bool;
pub type ControlWordFn = unsafe extern "C" fn(*mut u16) -> bool;
pub type IrqlProvider = unsafe extern "C" fn(*mut u8) -> bool;
pub type FatalFn = unsafe extern "C" fn(u32, *const c_char);
pub type PublicRoute = unsafe extern "C" fn(u32, *const KernelFrame, u32, *mut u32) -> bool;
pub type ResetRoute = unsafe extern "C" fn() -> bool;
pub type ProbeFn = unsafe extern "C" fn(*mut c_void, u32, u32, u32, *mut u32) -> bool;
pub type RoutingApply = unsafe extern "C" fn(*mut c_void, u32, u64, *const c_void) -> bool;
pub type SpansSnapshot = unsafe extern "C" fn(u32, *mut GuestSpan, u32, *mut u32) -> bool;
pub type FrequencyApply = unsafe extern "C" fn(*mut c_void, u32, u64, u32, u32) -> bool;
pub type FormatApply = unsafe extern "C" fn(*mut c_void, u32, u64, u32, u32, u32) -> bool;
pub type OwnedInterfaceFn = unsafe extern "C" fn(u32, *mut c_void, *mut u32);

pub type DeviceStatus = u32;
pub const LEASE_OK: DeviceStatus = 0;
pub const LEASE_INVALID: DeviceStatus = 1;

unsafe extern "C" {
    // Host services (C host).
    pub fn kernel_host_pid() -> i32;
    pub fn kernel_guest_read_u32(address: u32, out: *mut u32) -> bool;
    pub fn kernel_guest_write_u32(address: u32, value: u32) -> bool;
    pub fn kernel_guest_read_bytes(address: u32, out: *mut c_void, length: usize) -> bool;
    pub fn kmem_heap_valid(handle: u32) -> bool;
    pub fn kmem_heap_block_size(handle: u32, ptr: u32, out: *mut u32) -> bool;
    pub fn dsound_hle_log_line(line: *const c_char);
    pub fn dsound_hle_degrade(kind: *const c_char, entry: u32, reason: *const c_char) -> bool;
    pub fn dsound_device_acquire_lease(
        interface: u32,
        ops: *const LeaseOps,
        userdata: *mut c_void,
        output: *mut Lease,
    ) -> DeviceStatus;
    pub fn dsound_device_release_lease(
        lease: *const Lease,
        ops: *const LeaseOps,
        userdata: *mut c_void,
    ) -> DeviceStatus;
    pub fn dsound_device_with_owned_interface(
        interface: u32,
        callback: OwnedInterfaceFn,
        userdata: *mut c_void,
        result: *mut u32,
    ) -> bool;

    // Stream mutexes and the Rust stream models (tsfp-audio-capi).
    pub fn tsfp_audio_stream_mutex_lock_rust();
    pub fn tsfp_audio_stream_mutex_unlock_rust();
    pub fn tsfp_audio_stream_reset_mutex_lock_rust();
    pub fn tsfp_audio_stream_reset_mutex_unlock_rust();
    pub fn tsfp_audio_stream_create_abort_unlock_rust(userdata: *mut c_void);

    pub fn tsfp_audio_stream_lease_identity_matches_rust(a: *const Lease, b: *const Lease) -> bool;
    pub fn tsfp_audio_stream_lease_identity_rust(address: u32, out: *mut Lease) -> bool;
    pub fn tsfp_audio_stream_lease_validate_record_rust(
        address: u32,
        identity: *const Lease,
        internal: u32,
        context: *mut c_void,
        probe: ProbeFn,
    ) -> bool;
    pub fn tsfp_audio_stream_lease_scope_rust(a: u32, l: *const Lease, out: *mut Scope) -> bool;
    pub fn tsfp_audio_stream_lease_format_state_rust(
        a: u32,
        l: *const Lease,
        out: *mut FormatState,
    ) -> bool;
    pub fn tsfp_audio_stream_lease_format_commit_rust(
        a: u32,
        l: *const Lease,
        expected: *const FormatState,
        bytes: *const u8,
    ) -> bool;
    pub fn tsfp_audio_stream_lease_volume_state_rust(
        a: u32,
        l: *const Lease,
        out: *mut VolumeState,
    ) -> bool;
    pub fn tsfp_audio_stream_lease_volume_commit_rust(
        a: u32,
        l: *const Lease,
        expected: *const VolumeState,
        next: *const VolumeState,
    ) -> bool;
    pub fn tsfp_audio_stream_lease_volume_apply_rust(
        a: u32,
        l: *const Lease,
        request: *const VolumeRequest,
        next: *mut VolumeState,
    ) -> u32;
    pub fn tsfp_audio_stream_lease_control_state_rust(
        a: u32,
        l: *const Lease,
        out: *mut ControlState,
    ) -> bool;
    pub fn tsfp_audio_stream_lease_control_apply_rust(
        a: u32,
        l: *const Lease,
        request: *const ControlRequest,
    ) -> u32;
    pub fn tsfp_audio_stream_lease_spatial_state_rust(
        a: u32,
        l: *const Lease,
        out: *mut SpatialState,
    ) -> bool;
    pub fn tsfp_audio_stream_lease_spatial_commit_rust(
        a: u32,
        l: *const Lease,
        expected: *const SpatialState,
        next: *const SpatialState,
    ) -> bool;
    pub fn tsfp_audio_stream_lease_max_distance_rust(
        a: u32,
        l: *const Lease,
        apply: u32,
        bits: u32,
    ) -> bool;
    pub fn tsfp_audio_stream_lease_startup_spatial_rust(
        a: u32,
        l: *const Lease,
        action: u32,
        argument: u32,
        count: u32,
        words: *const u32,
    ) -> u32;
    pub fn tsfp_audio_stream_lease_detach_rust(a: u32, l: *const Lease) -> bool;
    pub fn tsfp_audio_stream_lease_heap_rust(a: u32, heap: *mut u32) -> bool;
    pub fn tsfp_audio_stream_lease_overlaps_guest_span_rust(address: u32, bytes: u32) -> bool;
    pub fn tsfp_audio_stream_lease_guest_spans_rust(
        spans: *mut GuestSpan,
        capacity: u32,
        count: *mut u32,
    ) -> bool;
    pub fn tsfp_audio_stream_heap_destroy_rust(heap: u32) -> bool;

    pub fn tsfp_audio_stream_routing_apply_note_rust(
        context: *mut c_void,
        stream: u32,
        serial: u64,
        candidate: *const c_void,
    ) -> bool;
    pub fn tsfp_audio_stream_routing_volume_commit_rust(
        stream: u32,
        serial: u64,
        volume: u32,
        context: *mut c_void,
        apply: Option<RoutingApply>,
    ) -> bool;
    pub fn tsfp_audio_stream_routing_update_rust(
        stream: u32,
        serial: u64,
        operation: u32,
        argument: u32,
        host_pid: i32,
        descriptor: u32,
        internal: u32,
        context: *mut c_void,
        apply: RoutingApply,
        spans: SpansSnapshot,
    ) -> u32;
    pub fn tsfp_audio_stream_routing_scope_admit_rust(
        stream: u32,
        lease: *const Lease,
        completion_available: u32,
    ) -> u32;
    pub fn tsfp_audio_stream_routing_retire_rust(stream: u32, serial: u64) -> bool;

    pub fn tsfp_audio_stream_observe_rust(
        stream: u32,
        out: *mut Snapshot,
        error: *mut *const c_char,
    ) -> bool;
    pub fn tsfp_audio_stream_set_frequency_rust(
        stream: u32,
        serial: u64,
        scope_flags: u32,
        scope_channels: u32,
        scope_rate: u32,
        format_sets: u32,
        format_bytes: *const u8,
        cache_mask: u32,
        flags: u32,
        hertz: u32,
        control_word: u32,
        context: *mut c_void,
        apply: FrequencyApply,
        error: *mut *const c_char,
    ) -> bool;
    pub fn tsfp_audio_stream_frequency_state_rust(
        stream: u32,
        serial: u64,
        out: *mut FrequencyState,
    ) -> bool;
    pub fn tsfp_audio_stream_frequency_format_commit_rust(
        stream: u32,
        serial: u64,
        accepted_sets: u32,
        old_hz: u32,
        new_hz: u32,
        control_word: u16,
        format_address: u32,
        context: *mut c_void,
        apply: Option<FormatApply>,
        pitch: *mut i32,
    ) -> bool;
    pub fn tsfp_audio_stream_status_admit_rust(
        flags: u32,
        mask: u32,
        rate: u32,
        channels: u32,
        block: u32,
        average: u32,
        packets: u32,
        volume_seen: u32,
        volume: i32,
        pause: u32,
        flush: u32,
        discontinuity: u32,
    ) -> bool;
    pub fn tsfp_audio_stream_format_admit_rust(
        scope_flags: u32,
        cache_mask: u32,
        volume_seen: u32,
        discontinuity_seen: u32,
    ) -> u32;
    pub fn tsfp_audio_stream_format_decode_rust(
        host_pid: i32,
        address: u32,
        channels: u32,
        out_format: *mut u8,
        out_rate: *mut u32,
    ) -> bool;

    pub fn tsfp_audio_stream_scope_validate_rust(
        descriptor: *const u8,
        descriptor_bytes: usize,
        format: *const u8,
        format_bytes: usize,
        output: *mut Scope,
    ) -> bool;
    pub fn tsfp_audio_stream_scope_snapshot_rust(
        host_pid: i32,
        descriptor: u32,
        output: *mut Scope,
    ) -> bool;

    pub fn tsfp_audio_stream_create_set_enabled_rust(enabled: bool);
    pub fn tsfp_audio_stream_create_enabled_rust() -> bool;
    pub fn tsfp_audio_stream_create_policy_error_rust(
        enabled: u32,
        irql_known: u32,
        irql: u32,
        host_pid: i32,
    ) -> *const c_char;
    pub fn tsfp_audio_stream_create_outcome_rust(
        lease_status: u32,
        create_status: u32,
        has_error: u32,
    ) -> u32;
    pub fn tsfp_audio_stream_create_release_request_rust(request: *mut c_void);
    pub fn tsfp_audio_stream_create_cleanup_aborted_rust(userdata: *mut c_void);
    pub fn tsfp_audio_stream_create_prepare_callback_rust(
        candidate: *const Lease,
        userdata: *mut c_void,
        child: *mut LeaseChild,
    ) -> bool;
    pub fn tsfp_audio_stream_create_finalize_callback_rust(
        lease: *const Lease,
        userdata: *mut c_void,
    );
    pub fn tsfp_audio_stream_create_fixed_overlap_rust(
        address: u32,
        bytes: u32,
        internal: u32,
        include_curve: bool,
    ) -> bool;
    pub fn tsfp_audio_stream_create_untracked_overlaps_rust(address: u32, bytes: u32) -> bool;
    pub fn tsfp_audio_stream_create_tables_stopped_rust(host_pid: i32) -> bool;
    pub fn tsfp_audio_stream_device_singleton_rust(host_pid: i32, output: *mut u32) -> bool;
    pub fn tsfp_audio_stream_reset_rust() -> bool;

    pub fn tsfp_audio_stream_create_register_rust() -> usize;
    pub fn tsfp_audio_stream_i3dl2_register_rust() -> usize;
    pub fn tsfp_audio_stream_minimum_register_rust() -> usize;
    pub fn tsfp_audio_stream_rolloff_register_rust() -> usize;
    pub fn tsfp_audio_stream_control_register_rust() -> usize;
    pub fn tsfp_audio_stream_format_register_rust() -> usize;
    pub fn tsfp_audio_stream_spatial_register_rust() -> usize;
    pub fn tsfp_audio_stream_frequency_register_rust() -> usize;
    pub fn tsfp_audio_stream_routing_register_rust() -> usize;
    pub fn tsfp_audio_stream_volume_register_rust() -> usize;
}
