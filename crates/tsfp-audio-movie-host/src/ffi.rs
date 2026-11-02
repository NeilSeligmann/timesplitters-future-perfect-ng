// SPDX-License-Identifier: GPL-3.0-or-later
//! C ABI mirrors and imports of the movie stream host glue: layouts of `dsound_device.h` and `dsound_movie_stream.h`,
//! the C host services (kernel guest access, kmem heaps, clock) and the Rust models of `tsfp-audio-capi`.
use std::ffi::{c_char, c_void};

#[repr(C)]
#[derive(Clone, Copy, Default, PartialEq, Eq)]
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

pub type LeaseStatus = u32;
pub const LEASE_OK: LeaseStatus = 0;
pub const LEASE_OUT_OF_MEMORY: LeaseStatus = 4;
pub const LEASE_PREPARE_FAILED: LeaseStatus = 5;

/// `dsound_audio_movie_schedule_snapshot`.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct ScheduleSnapshot {
    pub refs: u32,
    pub queued: u32,
    pub pause_mode: u32,
    pub flush_pending: u32,
    pub head_deadline: u64,
    pub packet_size: [u32; 2],
}

/// `dsound_movie_stream_snapshot`.
#[repr(C)]
pub struct StreamSnapshot {
    pub stream_address: u32,
    pub refs: u32,
    pub callback: u32,
    pub context: u32,
    pub volume: i32,
    pub volume_seen: bool,
    pub discontinuity_seen: bool,
    pub flush_pending: bool,
    pub pause_mode: u32,
    pub queued: u32,
    pub packet_size: [u32; 2],
    pub head_deadline: u64,
}

pub type IrqlProvider = unsafe extern "C" fn(*mut u8) -> bool;
pub type FatalFn = unsafe extern "C" fn(u32, *const c_char);
pub type ClockFn = unsafe extern "C" fn() -> u64;
pub type CallbackFn = unsafe extern "C" fn(u32, u32, u32, u32) -> bool;
pub type OwnedInterfaceFn = unsafe extern "C" fn(u32, *mut c_void, *mut u32);
pub type NowTicks = extern "C" fn() -> u64;
pub type HostPid = extern "C" fn() -> i32;
pub type BeforeUnlock = unsafe extern "C" fn(u32, u32, u32);
pub type Unlock = unsafe extern "C" fn();
pub type Refuse = unsafe extern "C" fn(u32, *const c_char) -> !;
pub type CurrentIrql = unsafe extern "C" fn(*mut u8) -> bool;
pub type ReadGuestU32 = unsafe extern "C" fn(u32, *mut u32) -> bool;

unsafe extern "C" {
    // Host services (C host).
    pub fn kernel_host_pid() -> i32;
    pub fn kernel_clock_peek() -> u64;
    pub fn kernel_guest_read_u32(address: u32, out: *mut u32) -> bool;
    pub fn kernel_guest_write_u32(address: u32, value: u32) -> bool;
    pub fn kernel_guest_read_bytes(address: u32, out: *mut c_void, length: usize) -> bool;
    pub fn kernel_guest_write_bytes(address: u32, source: *const c_void, length: usize) -> bool;
    pub fn kernel_guest_at_ro(address: u32, length: usize) -> *const c_void;
    pub fn kernel_frame_arg(frame: *const c_void, index: u32, out: *mut u32) -> bool;
    pub fn dsound_hle_log_line(line: *const c_char);

    // Guest heaps: Rust kmem when active, the kernel memory ABI otherwise.
    pub fn tsfp_kmem_audio_heap_active() -> bool;
    pub fn tsfp_kmem_audio_heap_create(flags: u32, initial: u32, maximum: u32) -> u32;
    pub fn tsfp_kmem_audio_heap_alloc(heap: u32, bytes: u32) -> u32;
    pub fn tsfp_kmem_audio_heap_valid(heap: u32) -> bool;
    pub fn tsfp_kmem_audio_heap_block_size(heap: u32, address: u32, out: *mut u32) -> bool;
    pub fn tsfp_kmem_audio_heap_destroy(heap: u32) -> bool;
    pub fn kmem_heap_create(flags: u32, initial: u32, maximum: u32) -> u32;
    pub fn kmem_heap_alloc(heap: u32, bytes: u32) -> u32;
    pub fn kmem_heap_valid(heap: u32) -> bool;
    pub fn kmem_heap_block_size(heap: u32, address: u32, out: *mut u32) -> bool;
    pub fn kmem_heap_destroy(heap: u32) -> bool;

    // The device and stream models (tsfp-audio-capi, tsfp-audio-stream-glue).
    pub fn dsound_device_acquire_lease(
        interface: u32,
        ops: *const LeaseOps,
        userdata: *mut c_void,
        output: *mut Lease,
    ) -> LeaseStatus;
    pub fn dsound_device_release_lease(
        lease: *const Lease,
        ops: *const LeaseOps,
        userdata: *mut c_void,
    ) -> LeaseStatus;
    pub fn dsound_device_with_owned_interface(
        interface: u32,
        callback: OwnedInterfaceFn,
        userdata: *mut c_void,
        result: *mut u32,
    ) -> bool;
    pub fn dsound_stream_original_tables_stopped() -> bool;

    // The audio runtime (tsfp-audio-capi host_runtime.rs) and the movie models.
    pub fn dsound_audio_runtime_active() -> bool;
    pub fn dsound_audio_runtime_cut_stream(key: u32, ticks: u64) -> bool;
    pub fn dsound_audio_runtime_set_stream_running(key: u32, ticks: u64, running: bool) -> bool;
    pub fn dsound_audio_runtime_set_stream_volume(key: u32, millibels: i32) -> bool;
    pub fn tsfp_audio_volume_to_gain_q16(millibels: i32, gain: *mut u32) -> bool;

    pub fn tsfp_audio_movie_completion_set_runner_rust(runner: Option<CallbackFn>) -> bool;
    pub fn tsfp_audio_movie_completion_count_rust() -> u64;
    pub fn tsfp_audio_movie_completion_word_probe_rust(host_pid: i32, address: u32) -> bool;
    pub fn tsfp_audio_movie_pending_publish_rust(
        host_pid: i32,
        completed: u32,
        status: u32,
    ) -> bool;
    pub fn tsfp_audio_movie_packet_decode_rust(host_pid: i32, packet: u32, words: *mut u32)
    -> bool;
    pub fn tsfp_audio_stream_packet_buffer_probe_rust(
        host_pid: i32,
        buffer: u32,
        bytes: u32,
    ) -> bool;
    pub fn tsfp_audio_movie_diagnostics_count_operation_rust();
    pub fn tsfp_audio_movie_diagnostics_operation_count_rust() -> u64;
    pub fn tsfp_audio_movie_diagnostics_announce_once_rust() -> bool;
    pub fn tsfp_audio_movie_diagnostics_reset_rust();
    pub fn tsfp_audio_movie_release_teardown_rust(stream: u32) -> bool;
    pub fn tsfp_audio_movie_stream_reset_rust() -> bool;
    pub fn tsfp_audio_movie_policy_rust(
        current_irql: Option<CurrentIrql>,
        read: Option<ReadGuestU32>,
    ) -> u32;
    pub fn tsfp_audio_movie_pcm_packets_rust() -> u64;
    pub fn tsfp_audio_movie_pcm_frames_rust() -> u64;
    pub fn tsfp_audio_movie_pcm_refused_rust() -> u64;

    pub fn tsfp_audio_movie_schedule_reset_volume_rust(stream: u32) -> bool;
    pub fn tsfp_audio_movie_schedule_configure_rust(clock_frequency: u64) -> bool;
    pub fn tsfp_audio_movie_schedule_attach_rust(
        stream: u32,
        internal: u32,
        publication_address: u32,
        callback: u32,
        context: u32,
    ) -> bool;
    pub fn tsfp_audio_movie_schedule_detach_rust(stream: u32) -> bool;
    pub fn tsfp_audio_movie_schedule_add_ref_rust(stream: u32, output: *mut u32) -> bool;
    pub fn tsfp_audio_movie_schedule_release_is_final_rust(stream: u32, output: *mut bool) -> bool;
    pub fn tsfp_audio_movie_schedule_release_ref_rust(stream: u32, output: *mut u32) -> bool;
    pub fn tsfp_audio_movie_schedule_completion_word_protected_rust(
        stream: u32,
        internal: u32,
        address: u32,
    ) -> bool;
    pub fn tsfp_audio_movie_schedule_get_status_rust(
        host_pid: i32,
        stream: u32,
        internal: u32,
        output: u32,
    ) -> bool;
    pub fn tsfp_audio_movie_schedule_submit_pcm_rust(
        host_pid: i32,
        stream: u32,
        ticks: u64,
        buffer: u32,
        bytes: u32,
    ) -> bool;
    pub fn tsfp_audio_movie_schedule_count_rust(stream: u32) -> u32;
    pub fn tsfp_audio_movie_schedule_has_capacity_rust(stream: u32) -> bool;
    pub fn tsfp_audio_movie_schedule_admission_rust(
        stream: u32,
        size: u32,
        completed: u32,
        status: u32,
    ) -> u32;
    pub fn tsfp_audio_movie_schedule_enqueue_rust(
        stream: u32,
        size: u32,
        completed: u32,
        status: u32,
        context: u32,
        now: u64,
    ) -> bool;
    pub fn tsfp_audio_movie_schedule_pause_rust(
        stream: u32,
        mode: u32,
        now: u64,
        result: *mut u32,
    ) -> bool;
    pub fn tsfp_audio_movie_schedule_flush_ex_rust(
        stream: u32,
        low: u32,
        high: u32,
        flags: u32,
        now: u64,
    ) -> bool;
    pub fn tsfp_audio_movie_schedule_drain_rust(
        entry: u32,
        everything: bool,
        only_stream: u32,
        now_ticks: NowTicks,
        host_pid: HostPid,
        before_unlock: BeforeUnlock,
        unlock: Unlock,
        refuse: Refuse,
    ) -> usize;
    pub fn tsfp_audio_movie_schedule_snapshot_rust(
        stream: u32,
        output: *mut ScheduleSnapshot,
    ) -> bool;
}
