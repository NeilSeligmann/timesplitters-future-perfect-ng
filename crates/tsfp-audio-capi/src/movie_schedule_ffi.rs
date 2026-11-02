//! Live queue/deadline and caller-admission seams for the Rust-host XMV movie stream path.
//! Guest object creation and callback execution stay in the host facade.
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard};

#[cfg(feature = "rust-audio-host")]
use crate::device_ffi::HostWorld;
use tsfp_audio::movie::{MovieStreams, PacketAdmission, ScheduledCompletion};

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_movie_caller_ok_rust(address: u32) -> bool {
    tsfp_audio::movie::caller_ok(address)
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_movie_create_caller_ok_rust(address: u32) -> bool {
    tsfp_audio::movie::create_caller_ok(address)
}

#[cfg(test)]
mod caller_policy_tests {
    use super::*;

    #[test]
    fn exported_caller_policy_matches_the_movie_contract() {
        assert!(tsfp_audio_movie_caller_ok_rust(0x0044_4920));
        assert!(tsfp_audio_movie_caller_ok_rust(0x0003_0675));
        assert!(!tsfp_audio_movie_caller_ok_rust(0x0044_CE4A));
        assert!(!tsfp_audio_movie_caller_ok_rust(0x0003_0676));
        assert!(tsfp_audio_movie_create_caller_ok_rust(0x0044_51F4));
        assert!(!tsfp_audio_movie_create_caller_ok_rust(0x0044_51F3));
    }
}

#[cfg(test)]
mod final_release_tests {
    use super::*;

    #[test]
    fn reset_tail_retires_rollback_nodes_in_order_before_finalization() {
        let addresses = [0x1000, 0x2000, 0];
        let index = std::cell::Cell::new(0usize);
        let retired = std::cell::RefCell::new(Vec::new());
        let mut finalized = false;
        assert!(finish_reset_tail(
            || {
                let i = index.get();
                index.set(i + 1);
                addresses[i]
            },
            |address| {
                retired.borrow_mut().push(address);
                true
            },
            || {
                finalized = true;
                true
            },
        ));
        assert_eq!(*retired.borrow(), [0x1000, 0x2000]);
        assert!(finalized);
    }

    #[test]
    fn reset_tail_clears_rust_diagnostics_after_host_finalization() {
        let events = std::cell::RefCell::new(Vec::new());
        assert!(finish_movie_reset_tail(
            || 0,
            |_| true,
            || {
                events.borrow_mut().push("host");
                true
            },
            || events.borrow_mut().push("rust"),
        ));
        assert_eq!(*events.borrow(), ["host", "rust"]);
    }

    #[test]
    fn reset_tail_stops_on_refused_rollback_retirement() {
        let addresses = [0x1000, 0x2000, 0];
        let index = std::cell::Cell::new(0usize);
        let finalized = std::cell::Cell::new(false);
        assert!(!finish_reset_tail(
            || {
                let i = index.get();
                index.set(i + 1);
                addresses[i]
            },
            |_| false,
            || {
                finalized.set(true);
                true
            },
        ));
        assert!(!finalized.get());
    }

    #[test]
    fn abort_drain_cuts_the_selected_stream_before_publishing_completion() {
        let event = ScheduledCompletion {
            event: tsfp_audio::movie::Completion {
                stream: 0x1234,
                callback: 0x5678,
                stream_context: 0x9abc,
                packet_context: 0xdef0,
                size: 16,
                status: 0x8000_4004,
            },
            completed_address: 0x2000,
            status_address: 0x2004,
        };
        let calls = std::cell::RefCell::new(Vec::new());
        let result = cut_before_completion(event, true, 91, |stream, ticks| {
            calls.borrow_mut().push((stream, ticks));
        });
        assert_eq!(*calls.borrow(), [(0x1234, 91)]);
        assert_eq!(result, event);

        let calls = std::cell::RefCell::new(Vec::new());
        let result = cut_before_completion(event, false, 92, |stream, ticks| {
            calls.borrow_mut().push((stream, ticks));
        });
        assert!(calls.borrow().is_empty());
        assert_eq!(result, event);
    }

    #[test]
    fn rust_drain_loop_dispatches_in_order_and_stops_when_host_refuses() {
        let events = [
            CMovieScheduleCompletion {
                stream: 1,
                packet_context: 10,
                ..CMovieScheduleCompletion::default()
            },
            CMovieScheduleCompletion {
                stream: 2,
                packet_context: 20,
                ..CMovieScheduleCompletion::default()
            },
            CMovieScheduleCompletion {
                stream: 3,
                packet_context: 30,
                ..CMovieScheduleCompletion::default()
            },
        ];
        let mut next = events.into_iter();
        let dispatched = std::cell::RefCell::new(Vec::new());
        let count = drain_completion_events(
            || next.next(),
            |event| {
                dispatched.borrow_mut().push(event.packet_context);
                event.stream != 2
            },
        );
        assert_eq!(count, 1);
        assert_eq!(*dispatched.borrow(), [10, 20]);
    }

    #[test]
    fn final_release_orders_packet_drain_gain_reset_and_retirement() {
        let events = std::cell::RefCell::new(Vec::new());
        assert!(finish_final_release(
            || {
                events.borrow_mut().push("drain");
                true
            },
            || {
                events.borrow_mut().push("reset");
                true
            },
            || {
                events.borrow_mut().push("retire");
                true
            },
        ));
        assert_eq!(*events.borrow(), ["drain", "reset", "retire"]);
    }

    #[test]
    fn final_release_stops_after_a_refused_teardown_step() {
        let events = std::cell::RefCell::new(Vec::new());
        assert!(!finish_final_release(
            || {
                events.borrow_mut().push("drain");
                true
            },
            || {
                events.borrow_mut().push("reset");
                false
            },
            || {
                events.borrow_mut().push("retire");
                true
            },
        ));
        assert_eq!(*events.borrow(), ["drain", "reset"]);
    }
}

static SCHEDULE: Mutex<Option<MovieStreams>> = Mutex::new(None);
static PCM_PACKETS: AtomicU64 = AtomicU64::new(0);
static PCM_FRAMES: AtomicU64 = AtomicU64::new(0);
static PCM_REFUSED: AtomicU64 = AtomicU64::new(0);
static PCM_REFUSAL_REPORTED: AtomicBool = AtomicBool::new(false);

#[derive(Default)]
struct MovieDiagnostics {
    operations: AtomicU64,
    announced: AtomicBool,
}

impl MovieDiagnostics {
    fn count_operation(&self) {
        self.operations.fetch_add(1, Ordering::Relaxed);
    }

    fn operation_count(&self) -> u64 {
        self.operations.load(Ordering::Relaxed)
    }

    fn announce_once(&self) -> bool {
        !self.announced.swap(true, Ordering::Relaxed)
    }

    fn reset(&self) {
        self.announced.store(false, Ordering::Relaxed);
        self.operations.store(0, Ordering::Relaxed);
    }
}

static MOVIE_DIAGNOSTICS: MovieDiagnostics = MovieDiagnostics {
    operations: AtomicU64::new(0),
    announced: AtomicBool::new(false),
};

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_movie_diagnostics_count_operation_rust() {
    MOVIE_DIAGNOSTICS.count_operation();
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_movie_diagnostics_operation_count_rust() -> u64 {
    MOVIE_DIAGNOSTICS.operation_count()
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_movie_diagnostics_announce_once_rust() -> bool {
    MOVIE_DIAGNOSTICS.announce_once()
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_movie_diagnostics_reset_rust() {
    MOVIE_DIAGNOSTICS.reset();
}

#[cfg(test)]
mod movie_diagnostics_tests {
    use super::*;

    #[test]
    fn diagnostics_count_announce_and_reset_are_owned_by_one_state() {
        let state = MovieDiagnostics::default();
        assert_eq!(state.operation_count(), 0);
        state.count_operation();
        state.count_operation();
        assert_eq!(state.operation_count(), 2);
        assert!(state.announce_once());
        assert!(!state.announce_once());
        state.reset();
        assert_eq!(state.operation_count(), 0);
        assert!(state.announce_once());
    }
}

const MOVIE_PCM_RATE_HZ: u32 = 44_100;

fn schedule() -> MutexGuard<'static, Option<MovieStreams>> {
    SCHEDULE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(feature = "rust-audio-host")]
fn release_is_final(refs: u32) -> bool {
    refs == 1
}

fn finish_final_release<D, V, X>(mut drain: D, mut reset_volume: V, mut destroy: X) -> bool
where
    D: FnMut() -> bool,
    V: FnMut() -> bool,
    X: FnMut() -> bool,
{
    drain() && reset_volume() && destroy()
}

fn finish_reset_tail<N, D, F>(mut next: N, mut destroy: D, finish: F) -> bool
where
    N: FnMut() -> u32,
    D: FnMut(u32) -> bool,
    F: FnOnce() -> bool,
{
    loop {
        let address = next();
        if address == 0 {
            return finish();
        }
        if !destroy(address) {
            return false;
        }
    }
}

fn finish_movie_reset_tail<N, D, F, R>(next: N, destroy: D, finish: F, reset_rust: R) -> bool
where
    N: FnMut() -> u32,
    D: FnMut(u32) -> bool,
    F: FnOnce() -> bool,
    R: FnOnce(),
{
    if !finish_reset_tail(next, destroy, finish) {
        return false;
    }
    reset_rust();
    true
}

fn cut_before_completion<F>(
    event: ScheduledCompletion,
    everything: bool,
    now: u64,
    mut cut_stream: F,
) -> ScheduledCompletion
where
    F: FnMut(u32, u64),
{
    if everything {
        cut_stream(event.event.stream, now);
    }
    event
}

#[cfg(feature = "rust-audio-host")]
unsafe extern "C" {
    fn tsfp_audio_movie_release_drain_host_c(stream: u32) -> bool;
    fn tsfp_audio_movie_reset_destroy_host_c(stream: u32) -> bool;
}

/// Run final Release teardown in the measured drain, gain-reset, lease/heap order.
#[cfg(feature = "rust-audio-host")]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_movie_release_teardown_rust(stream: u32) -> bool {
    finish_final_release(
        || {
            // SAFETY: the synchronous C adapter drains this stream and dispatches each guest callback.
            unsafe { tsfp_audio_movie_release_drain_host_c(stream) }
        },
        || tsfp_audio_movie_schedule_reset_volume_rust(stream),
        || {
            // SAFETY: the adapter validates and detaches this exact stream lease before heap retirement.
            unsafe { tsfp_audio_movie_reset_destroy_host_c(stream) }
        },
    )
}

/// Reset a movie stream's mixer gain after the final Release or synchronous flush.
#[unsafe(no_mangle)]
#[cfg(feature = "rust-audio-host")]
pub extern "C" fn tsfp_audio_movie_schedule_reset_volume_rust(stream: u32) -> bool {
    if !crate::host_runtime::dsound_audio_runtime_active() {
        return true;
    }
    crate::host_runtime::dsound_audio_runtime_reset_stream_volume(stream)
}

#[cfg(feature = "rust-audio-host")]
unsafe extern "C" {
    fn tsfp_audio_movie_reset_tail_host_c() -> bool;
    fn tsfp_audio_movie_reset_rollback_first_host_c() -> u32;
    fn tsfp_audio_movie_reset_rollback_destroy_host_c(address: u32) -> bool;
}

/// Run the measured one-at-a-time movie-stream reset sweep from Rust-owned schedule order.
/// Each host call performs one synchronous lease detach and private-heap retirement; a refusal
/// stops the sweep so prior successful retirements remain committed.
#[cfg(feature = "rust-audio-host")]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_movie_stream_reset_rust() -> bool {
    loop {
        let stream = {
            let state = schedule();
            let Some(model) = state.as_ref() else {
                return false;
            };
            model.first_stream()
        };
        let Some(stream) = stream else {
            // Rollback nodes are unleased allocations. Rust owns the sweep order; each C call
            // synchronously validates and retires exactly the current head allocation.
            return finish_movie_reset_tail(
                || {
                    // SAFETY: the adapter snapshots the rollback head while holding the movie lock.
                    unsafe { tsfp_audio_movie_reset_rollback_first_host_c() }
                },
                |address| {
                    // SAFETY: retirement rechecks that this address is still the rollback head.
                    unsafe { tsfp_audio_movie_reset_rollback_destroy_host_c(address) }
                },
                || {
                    // SAFETY: schedule and rollback sweep are empty; this only resets host diagnostics.
                    unsafe { tsfp_audio_movie_reset_tail_host_c() }
                },
                || {
                    // Rust-owned diagnostics reset only after the native host confirms an empty tail.
                    crate::movie_completion_ffi::tsfp_audio_movie_completion_count_reset_rust();
                    tsfp_audio_movie_pcm_stats_reset_rust();
                },
            );
        };
        // SAFETY: the host adapter validates the exact address and lease under its locks.
        if !unsafe { tsfp_audio_movie_reset_destroy_host_c(stream) } {
            return false;
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub struct CMovieScheduleCompletion {
    pub stream: u32,
    pub callback: u32,
    pub stream_context: u32,
    pub packet_context: u32,
    pub size: u32,
    pub status: u32,
    pub completed_address: u32,
    pub status_address: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct CMovieScheduleSnapshot {
    pub refs: u32,
    pub queued: u32,
    pub pause_mode: u32,
    pub flush_pending: u32,
    pub head_deadline: u64,
    pub packet_size: [u32; 2],
}

impl From<ScheduledCompletion> for CMovieScheduleCompletion {
    fn from(value: ScheduledCompletion) -> Self {
        Self {
            stream: value.event.stream,
            callback: value.event.callback,
            stream_context: value.event.stream_context,
            packet_context: value.event.packet_context,
            size: value.event.size,
            status: value.event.status,
            completed_address: value.completed_address,
            status_address: value.status_address,
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_movie_schedule_configure_rust(clock_frequency: u64) -> bool {
    if clock_frequency == 0 {
        return false;
    }
    let mut state = schedule();
    match state.as_mut() {
        Some(model) if model.count() == 0 => model.clock_frequency = clock_frequency,
        Some(model) => return model.clock_frequency == clock_frequency,
        None => *state = Some(MovieStreams::new(clock_frequency)),
    }
    true
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_movie_schedule_attach_rust(
    stream: u32,
    internal: u32,
    publication_address: u32,
    callback: u32,
    context: u32,
) -> bool {
    let mut state = schedule();
    state.as_mut().is_some_and(|model| {
        model
            .attach_scheduler(stream, internal, publication_address, callback, context)
            .is_ok()
    })
}

/// Apply AddRef to the Rust-owned movie stream model and guest header.
#[unsafe(no_mangle)]
#[cfg(feature = "rust-audio-host")]
pub unsafe extern "C" fn tsfp_audio_movie_schedule_add_ref_rust(
    stream: u32,
    output: *mut u32,
) -> bool {
    if output.is_null() {
        return false;
    }
    let mut state = schedule();
    let Some(model) = state.as_mut() else {
        return false;
    };
    let Ok(refs) = model.add_ref(&mut HostWorld, stream) else {
        return false;
    };
    // SAFETY: caller provides a writable output word.
    unsafe { output.write(refs) };
    true
}

/// Select the final Release path from the Rust-owned movie reference count.
#[unsafe(no_mangle)]
#[cfg(feature = "rust-audio-host")]
pub unsafe extern "C" fn tsfp_audio_movie_schedule_release_is_final_rust(
    stream: u32,
    output: *mut bool,
) -> bool {
    if output.is_null() {
        return false;
    }
    let state = schedule();
    let Some(snapshot) = state.as_ref().and_then(|model| model.snapshot(stream)) else {
        return false;
    };
    // SAFETY: caller provides a writable output byte.
    unsafe { output.write(release_is_final(snapshot.refs)) };
    true
}

/// Apply only a nonfinal Release; final packet drain and lease retirement remain host-owned.
#[unsafe(no_mangle)]
#[cfg(feature = "rust-audio-host")]
pub unsafe extern "C" fn tsfp_audio_movie_schedule_release_ref_rust(
    stream: u32,
    output: *mut u32,
) -> bool {
    if output.is_null() {
        return false;
    }
    let mut state = schedule();
    let Some(model) = state.as_mut() else {
        return false;
    };
    if !model
        .snapshot(stream)
        .is_some_and(|snapshot| snapshot.refs > 1)
    {
        return false;
    }
    let Ok(tsfp_audio::movie::Released::Count(refs)) = model.release(&mut HostWorld, stream, 0)
    else {
        return false;
    };
    // SAFETY: caller provides a writable output word.
    unsafe { output.write(refs) };
    true
}

/// Returns true only for a registered stream with a completion word outside protected state.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_movie_schedule_completion_word_protected_rust(
    stream: u32,
    internal: u32,
    address: u32,
) -> bool {
    schedule().as_ref().is_some_and(|model| {
        model.owns(stream) && !model.completion_word_aliases_protected_state(address, internal)
    })
}

/// Publishes the live Rust queue status to a guest word after the same protected-range and
/// fault-safe writable checks used for XMV completion words.
///
/// # Safety
/// `output` must be a guest address in the process identified by `host_pid`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_movie_schedule_get_status_rust(
    host_pid: i32,
    stream: u32,
    internal: u32,
    output: u32,
) -> bool {
    if host_pid <= 0 {
        return false;
    }
    let mut state = schedule();
    let Some(model) = state.as_mut() else {
        return false;
    };
    if !model.owns(stream) || model.completion_word_aliases_protected_state(output, internal) {
        return false;
    }
    let Ok(value) = model.status_word(stream) else {
        return false;
    };
    let mut old = [0u8; 4];
    // SAFETY: guest-copy validates both spans and refuses inaccessible pages without faulting.
    if !unsafe {
        crate::effects_guest_copy_ffi::tsfp_effects_guest_copy_read_rust(
            host_pid,
            output,
            old.as_mut_ptr(),
            old.len(),
        )
    } || !unsafe {
        crate::effects_guest_copy_ffi::tsfp_effects_guest_copy_write_rust(
            host_pid,
            old.as_ptr(),
            output,
            old.len(),
        )
    } {
        return false;
    }
    let bytes = value.to_le_bytes();
    // SAFETY: the process_vm boundary validates the destination and refuses inaccessible pages.
    unsafe {
        crate::effects_guest_copy_ffi::tsfp_effects_guest_copy_write_rust(
            host_pid,
            bytes.as_ptr(),
            output,
            bytes.len(),
        )
    }
}

/// Submits an admitted XMV PCM packet through the Rust mixer on the Bevy audio path.
/// Returns true if the runtime is inactive or the packet was accepted.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_movie_schedule_submit_pcm_rust(
    host_pid: i32,
    stream: u32,
    ticks: u64,
    buffer: u32,
    bytes: u32,
) -> bool {
    if !schedule().as_ref().is_some_and(|model| model.owns(stream)) {
        record_pcm_refusal(bytes);
        return false;
    }
    if !crate::host_runtime::dsound_audio_runtime_active() {
        return true;
    }
    if crate::host_runtime::dsound_audio_runtime_submit_guest_pcm16_stereo(
        stream,
        ticks,
        MOVIE_PCM_RATE_HZ,
        host_pid,
        buffer,
        bytes as usize,
    ) {
        PCM_PACKETS.fetch_add(1, Ordering::Relaxed);
        PCM_FRAMES.fetch_add(u64::from(bytes / 4), Ordering::Relaxed);
        return true;
    }
    record_pcm_refusal(bytes);
    false
}

fn record_pcm_refusal(bytes: u32) {
    if !PCM_REFUSAL_REPORTED.swap(true, Ordering::Relaxed) {
        eprintln!(
            "dsound: movie PCM packet (0x{bytes:X} bytes) was refused by the mixer; its audio is dropped, guest timing is unchanged"
        );
    }
    PCM_REFUSED.fetch_add(1, Ordering::Relaxed);
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_movie_pcm_packets_rust() -> u64 {
    PCM_PACKETS.load(Ordering::Relaxed)
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_movie_pcm_frames_rust() -> u64 {
    PCM_FRAMES.load(Ordering::Relaxed)
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_movie_pcm_refused_rust() -> u64 {
    PCM_REFUSED.load(Ordering::Relaxed)
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_movie_pcm_stats_reset_rust() {
    PCM_PACKETS.store(0, Ordering::Relaxed);
    PCM_FRAMES.store(0, Ordering::Relaxed);
    PCM_REFUSED.store(0, Ordering::Relaxed);
    PCM_REFUSAL_REPORTED.store(false, Ordering::Relaxed);
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_movie_schedule_detach_rust(stream: u32) -> bool {
    schedule()
        .as_mut()
        .is_some_and(|model| model.detach_scheduler(stream))
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_movie_schedule_count_rust(stream: u32) -> u32 {
    schedule()
        .as_ref()
        .and_then(|model| model.scheduled_count(stream))
        .map_or(u32::MAX, |count| count as u32)
}

/// Returns true only when the registered Rust queue can accept another packet.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_movie_schedule_has_capacity_rust(stream: u32) -> bool {
    schedule()
        .as_ref()
        .and_then(|model| model.scheduled_has_capacity(stream))
        .unwrap_or(false)
}

/// Checks typed packet policy before the host probes guest spans or publishes pending state.
/// 0=accepted, 1=unknown stream, 2=full, 3=invalid size, 4=completion words overlap,
/// 5=completion words alias an attached packet.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_movie_schedule_admission_rust(
    stream: u32,
    size: u32,
    completed: u32,
    status: u32,
) -> u32 {
    let state = schedule();
    let Some(model) = state.as_ref() else {
        return 1;
    };
    match model.scheduled_packet_admission(stream, size, completed, status) {
        Ok(()) => 0,
        Err(PacketAdmission::StreamUnavailable) => 1,
        Err(PacketAdmission::QueueFull) => 2,
        Err(PacketAdmission::InvalidSize) => 3,
        Err(PacketAdmission::CompletionWordsOverlap) => 4,
        Err(PacketAdmission::CompletionWordsAliasQueuedPacket) => 5,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_movie_schedule_aliases_rust(
    stream: u32,
    completed: u32,
    status: u32,
) -> bool {
    schedule()
        .as_ref()
        .and_then(|model| model.scheduled_words_alias(stream, completed, status))
        .unwrap_or(true)
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_movie_schedule_enqueue_rust(
    stream: u32,
    size: u32,
    completed: u32,
    status: u32,
    context: u32,
    now: u64,
) -> bool {
    schedule().as_mut().is_some_and(|model| {
        model
            .schedule_packet(stream, size, completed, status, context, now)
            .is_ok()
    })
}

/// Returns false for the unresolved/unsupported pause mode; `result` receives the HRESULT otherwise.
///
/// # Safety
/// `result` must point to a writable dword.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_movie_schedule_pause_rust(
    stream: u32,
    mode: u32,
    now: u64,
    result: *mut u32,
) -> bool {
    if result.is_null() {
        return false;
    }
    let mut state = schedule();
    let Some(model) = state.as_mut() else {
        return false;
    };
    let Ok(value) = model.pause(stream, mode, now) else {
        return false;
    };
    // SAFETY: caller supplied a writable output word by contract.
    unsafe { result.write(value) };
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(feature = "rust-audio-host")]
    fn snapshot_exports_the_rust_owned_stream_reference_count() {
        const STREAM: u32 = 0x8A00_0100;
        assert!(tsfp_audio_movie_schedule_configure_rust(176_400));
        assert!(tsfp_audio_movie_schedule_attach_rust(
            STREAM,
            0x9000,
            0xA000,
            0x0044_5071,
            0xDEC0
        ));
        let mut snapshot = CMovieScheduleSnapshot::default();
        // SAFETY: snapshot is writable for the duration of this call.
        assert!(unsafe { tsfp_audio_movie_schedule_snapshot_rust(STREAM, &mut snapshot) });
        assert_eq!(snapshot.refs, 1);
        let mut is_final = false;
        // SAFETY: is_final is writable for the duration of this call.
        assert!(unsafe { tsfp_audio_movie_schedule_release_is_final_rust(STREAM, &mut is_final) });
        assert!(is_final);
    }

    #[test]
    #[cfg(feature = "rust-audio-host")]
    fn finality_selection_distinguishes_final_and_nonfinal_counts() {
        assert!(release_is_final(1));
        assert!(!release_is_final(2));
        assert!(!release_is_final(0));
    }

    #[test]
    fn admission_ffi_reports_typed_queue_policy() {
        const STREAM: u32 = 0x8A00_0000;
        assert!(tsfp_audio_movie_schedule_configure_rust(176_400));
        assert!(tsfp_audio_movie_schedule_attach_rust(
            STREAM,
            0x9000,
            0xA000,
            0x0044_5071,
            0xDEC0
        ));
        assert!(tsfp_audio_movie_schedule_completion_word_protected_rust(
            STREAM, 0x9000, 0xB000
        ));
        assert!(!tsfp_audio_movie_schedule_completion_word_protected_rust(
            STREAM, 0x9000, 0x9001
        ));
        assert!(!tsfp_audio_movie_schedule_completion_word_protected_rust(
            STREAM, 0x9000, 0xA001
        ));
        assert!(!tsfp_audio_movie_schedule_completion_word_protected_rust(
            STREAM,
            0x9000,
            0x0041_2B30
        ));
        assert!(!tsfp_audio_movie_schedule_completion_word_protected_rust(
            STREAM,
            0x9000,
            0x0041_24A8
        ));
        assert!(!tsfp_audio_movie_schedule_completion_word_protected_rust(
            STREAM,
            0x9000,
            0x004A_1D2B
        ));
        assert!(tsfp_audio_movie_schedule_has_capacity_rust(STREAM));
        assert_eq!(
            tsfp_audio_movie_schedule_admission_rust(STREAM, 3, 0x3000, 0x3004),
            3
        );
        assert_eq!(
            tsfp_audio_movie_schedule_admission_rust(STREAM, 4, 0x3000, 0x3002),
            4
        );
        assert_eq!(
            tsfp_audio_movie_schedule_admission_rust(STREAM, 4, 0x3000, 0x3004),
            0
        );
        assert!(tsfp_audio_movie_schedule_enqueue_rust(
            STREAM, 4, 0x3000, 0x3004, 0xA, 100
        ));
        assert_eq!(
            tsfp_audio_movie_schedule_admission_rust(STREAM, 4, 0x3004, 0x3008),
            5
        );
        assert!(tsfp_audio_movie_schedule_enqueue_rust(
            STREAM, 4, 0x3100, 0x3104, 0xB, 100
        ));
        assert!(!tsfp_audio_movie_schedule_has_capacity_rust(STREAM));
        assert_eq!(
            tsfp_audio_movie_schedule_admission_rust(STREAM, 4, 0x3200, 0x3204),
            2
        );
        assert!(tsfp_audio_movie_schedule_detach_rust(STREAM));
    }

    #[test]
    fn get_status_ffi_publishes_rust_status_to_odd_guest_word() {
        const STREAM: u32 = 0x8B00_0000;
        const INTERNAL: u32 = 0x9000;
        assert!(tsfp_audio_movie_schedule_configure_rust(176_400));
        assert!(tsfp_audio_movie_schedule_attach_rust(
            STREAM,
            INTERNAL,
            0xA000,
            0x0044_5071,
            0xDEC0
        ));

        let (page, base) = crate::test_guest_memory::map_guest_page();
        // SAFETY: this test owns a writable identity-mapped page; the odd dword is in bounds.
        unsafe { page.add(1).cast::<u32>().write_unaligned(0xA1B2_C3D4) };
        // SAFETY: the output points into the writable page owned by this test process.
        assert!(unsafe {
            tsfp_audio_movie_schedule_get_status_rust(
                std::process::id() as i32,
                STREAM,
                INTERNAL,
                base + 1,
            )
        });
        // SAFETY: the same odd dword remains inside the mapped page.
        assert_eq!(unsafe { page.add(1).cast::<u32>().read_unaligned() }, 1);

        // The same service refuses a protected device range before touching guest memory.
        // SAFETY: INTERNAL is only inspected as a guest address and is rejected as protected.
        assert!(!unsafe {
            tsfp_audio_movie_schedule_get_status_rust(
                std::process::id() as i32,
                STREAM,
                INTERNAL,
                INTERNAL + 1,
            )
        });
        assert!(tsfp_audio_movie_schedule_detach_rust(STREAM));
        crate::test_guest_memory::unmap_guest_page(page);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_movie_schedule_flush_ex_rust(
    stream: u32,
    time_low: u32,
    time_high: u32,
    flags: u32,
    now: u64,
) -> bool {
    schedule().as_mut().is_some_and(|model| {
        model
            .flush_ex(stream, time_low, time_high, flags, now)
            .is_ok()
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_movie_schedule_synch_rust(now: u64) {
    if let Some(model) = schedule().as_mut() {
        model.synch_playback(now);
    }
}

/// Writes one retired event and returns whether an event was produced.
///
/// # Safety
/// `output` must point to a writable [`CMovieScheduleCompletion`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_movie_schedule_take_rust(
    now: u64,
    everything: bool,
    only_stream: u32,
    output: *mut CMovieScheduleCompletion,
) -> bool {
    if output.is_null() {
        return false;
    }
    let mut state = schedule();
    let Some(event) = state
        .as_mut()
        .and_then(|model| model.take_scheduled(now, everything, only_stream))
    else {
        return false;
    };
    drop(state);
    let event = cut_before_completion(event, everything, now, |stream, ticks| {
        #[cfg(feature = "rust-audio-host")]
        {
            let _ = crate::host_runtime::dsound_audio_runtime_cut_stream(stream, ticks);
        }
        #[cfg(not(feature = "rust-audio-host"))]
        let _ = (stream, ticks);
    });
    // SAFETY: caller supplied a writable output struct by contract.
    unsafe { output.write(event.into()) };
    true
}

fn drain_completion_events<N, D>(mut next_event: N, mut dispatch: D) -> usize
where
    N: FnMut() -> Option<CMovieScheduleCompletion>,
    D: FnMut(&CMovieScheduleCompletion) -> bool,
{
    let mut delivered = 0;
    while let Some(event) = next_event() {
        if !dispatch(&event) {
            break;
        }
        delivered += 1;
    }
    delivered
}

pub type MovieScheduleNowTicks = extern "C" fn() -> u64;
pub type MovieScheduleHostPid = extern "C" fn() -> i32;
pub type MovieScheduleBeforeUnlock = unsafe extern "C" fn(u32, u32, u32);
pub type MovieScheduleUnlock = unsafe extern "C" fn();
pub type MovieScheduleRefuse = unsafe extern "C" fn(u32, *const core::ffi::c_char) -> !;

/// Runs the Rust-owned movie completion selection and delivery loop. Rust samples its selected
/// kernel clock when enabled; the host supplies a synchronous fallback clock, guest-memory
/// identity, bookkeeping, unlock, and fatal-stop services.
///
/// # Safety
/// Host callbacks must remain valid for this synchronous call; completion callbacks must not
/// reenter this drain operation.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_movie_schedule_drain_rust(
    entry: u32,
    everything: bool,
    only_stream: u32,
    now_ticks: MovieScheduleNowTicks,
    host_pid: MovieScheduleHostPid,
    before_unlock: MovieScheduleBeforeUnlock,
    unlock: MovieScheduleUnlock,
    refuse: MovieScheduleRefuse,
) -> usize {
    drain_completion_events(
        || {
            let mut event = CMovieScheduleCompletion::default();
            // SAFETY: `event` is a writable local output for the schedule service.
            let produced = unsafe {
                tsfp_audio_movie_schedule_take_rust(
                    crate::host_runtime::movie_schedule_now_ticks(|| now_ticks()),
                    everything,
                    only_stream,
                    &mut event,
                )
            };
            produced.then_some(event)
        },
        |event| unsafe {
            crate::movie_completion_ffi::movie_completion_count_increment_rust();
            let published = crate::movie_completion_ffi::tsfp_audio_movie_completion_publish_rust(
                host_pid(),
                event.completed_address,
                event.size,
                event.status_address,
                event.status,
            );
            before_unlock(event.size, event.status, event.packet_context);
            unlock();
            if !published {
                refuse(
                    entry,
                    c"packet completion words are no longer writable".as_ptr(),
                );
            }
            if !crate::movie_completion_ffi::tsfp_audio_movie_completion_call_guest_rust(
                event.callback,
                event.stream_context,
                event.packet_context,
                event.status,
            ) {
                refuse(entry, c"guest completion callback could not run".as_ptr());
            }
            true
        },
    )
}

/// Copies the Rust queue view for the C diagnostic snapshot; no queue state is returned to C for
/// mutation.
///
/// # Safety
/// `output` must point to a writable [`CMovieScheduleSnapshot`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_movie_schedule_snapshot_rust(
    stream: u32,
    output: *mut CMovieScheduleSnapshot,
) -> bool {
    if output.is_null() {
        return false;
    }
    let state = schedule();
    let Some(snapshot) = state.as_ref().and_then(|model| model.snapshot(stream)) else {
        return false;
    };
    let value = CMovieScheduleSnapshot {
        refs: snapshot.refs,
        queued: snapshot.queued,
        pause_mode: snapshot.pause_mode,
        flush_pending: u32::from(snapshot.flush_pending),
        head_deadline: snapshot.head_deadline,
        packet_size: snapshot.packet_size,
    };
    // SAFETY: caller supplied a writable output struct by contract.
    unsafe { output.write(value) };
    true
}
