//! Safe SoundHost wrapper around the production C mixer. Used only offline.
use std::{ffi::c_void, ptr::NonNull, sync::Mutex};
use tsfp_audio::contract::{Command, SoundHost};

// dsound_mixer.c lazily builds one process-global resample_table without a lock.
// Separate mixer instances therefore still need serialized entry to the C renderer.
static RENDER_LOCK: Mutex<()> = Mutex::new(());

fn with_render_lock<T>(render: impl FnOnce() -> T) -> T {
    let _guard = RENDER_LOCK.lock().expect("C oracle render lock poisoned");
    render()
}

#[repr(C)]
struct Route {
    volume: u32,
    headroom: u32,
    aggregate: u32,
    count: u32,
    bins: [u8; 8],
    bin_volume: [u32; 32],
}

unsafe extern "C" {
    fn dsound_adpcm_xbox_decode_mono(
        encoded: *const u8,
        bytes: usize,
        pcm: *mut i16,
        capacity: usize,
        written: *mut usize,
    ) -> bool;
    fn dsound_mixer_create(rate: u32, tps: u64) -> *mut c_void;
    fn dsound_mixer_destroy(m: *mut c_void);
    fn dsound_mixer_reset_buffers(m: *mut c_void);
    fn dsound_mixer_submit_xbox_adpcm(
        m: *mut c_void,
        k: u32,
        t: u64,
        hz: u32,
        p: *const u8,
        n: usize,
    ) -> bool;
    fn dsound_mixer_submit_xbox_adpcm_mono(
        m: *mut c_void,
        k: u32,
        t: u64,
        hz: u32,
        p: *const u8,
        n: usize,
    ) -> bool;
    fn dsound_mixer_submit_pcm16_stereo(
        m: *mut c_void,
        k: u32,
        t: u64,
        hz: u32,
        p: *const i16,
        n: usize,
    ) -> bool;
    fn dsound_mixer_cut_stream(m: *mut c_void, k: u32, t: u64) -> bool;
    fn dsound_mixer_set_stream_running(m: *mut c_void, k: u32, t: u64, running: bool) -> bool;
    fn dsound_mixer_set_stream_frequency(m: *mut c_void, k: u32, t: u64, hz: u32) -> bool;
    fn dsound_mixer_set_stream_volume(m: *mut c_void, k: u32, mb: i32) -> bool;
    fn dsound_mixer_reset_stream_volume(m: *mut c_void, k: u32) -> bool;
    fn dsound_mixer_play_mono_buffer(
        m: *mut c_void,
        k: u32,
        s: u64,
        t: u64,
        hz: u32,
        p: *const i16,
        n: usize,
        paused: bool,
        looping: bool,
        start: usize,
        end: usize,
        mb: i32,
    ) -> bool;
    fn dsound_mixer_pause_buffer(m: *mut c_void, k: u32, s: u64, t: u64, paused: bool) -> bool;
    fn dsound_mixer_stop_buffer(m: *mut c_void, k: u32, s: u64, t: u64) -> bool;
    fn dsound_mixer_frequency_buffer(m: *mut c_void, k: u32, s: u64, t: u64, hz: u32) -> bool;
    fn dsound_mixer_volume_buffer(m: *mut c_void, k: u32, s: u64, t: u64, mb: i32) -> bool;
    fn dsound_mixer_loop_buffer(
        m: *mut c_void,
        k: u32,
        s: u64,
        t: u64,
        start: usize,
        end: usize,
    ) -> bool;
    fn dsound_mixer_route_stream(m: *mut c_void, k: u32, s: u64, t: u64, r: *const Route) -> bool;
    fn dsound_mixer_frequency_stream(
        m: *mut c_void,
        k: u32,
        s: u64,
        t: u64,
        base: u32,
        old: u32,
        new: u32,
    ) -> bool;
    fn dsound_mixer_format_stream(
        m: *mut c_void,
        k: u32,
        s: u64,
        t: u64,
        old: u32,
        new: u32,
    ) -> bool;
    fn dsound_mixer_enable_mixbin_headroom(m: *mut c_void, on: bool) -> bool;
    fn dsound_mixer_bind_mixbin_headroom(m: *mut c_void, id: u64) -> bool;
    fn dsound_mixer_set_mixbin_headroom(m: *mut c_void, id: u64, t: u64, bin: u32, h: u32) -> bool;
    fn dsound_mixer_render_until(
        m: *mut c_void,
        t: u64,
        p: *mut i16,
        cap: usize,
        n: *mut usize,
    ) -> bool;
}

pub struct COracle(NonNull<c_void>);

/// Independent production-C decode for the PB command-log payload boundary.
pub fn decode_mono(data: &[u8]) -> Option<Vec<i16>> {
    if data.is_empty() || data.len() > 36 * 32768 || data.len() % 36 != 0 {
        return None;
    }
    let frames = data.len() / 36 * 64;
    let mut pcm = vec![0; frames];
    let mut written = 0;
    // SAFETY: slices and output count remain live for this synchronous bounded decode.
    let ok = unsafe {
        dsound_adpcm_xbox_decode_mono(
            data.as_ptr(),
            data.len(),
            pcm.as_mut_ptr(),
            frames,
            &mut written,
        )
    };
    (ok && written == frames).then_some(pcm)
}

impl COracle {
    pub fn new(rate: u32, tps: u64) -> Self {
        // SAFETY: constructor owns its allocation; Drop releases it exactly once.
        Self(NonNull::new(unsafe { dsound_mixer_create(rate, tps) }).expect("C mixer allocation"))
    }
}

impl Drop for COracle {
    fn drop(&mut self) {
        // SAFETY: this is the unique owning handle and all calls are synchronous.
        unsafe { dsound_mixer_destroy(self.0.as_ptr()) };
    }
}

impl SoundHost for COracle {
    fn apply(&mut self, command: &Command) -> bool {
        use Command::*;
        let m = self.0.as_ptr();
        // SAFETY: m remains live, borrowed payloads outlive these synchronous copying calls,
        // and C lengths are expressed in the declared API units (frames or bytes).
        unsafe {
            match command {
                SubmitAdpcm {
                    key,
                    ticks,
                    source_hz,
                    mono,
                    data,
                } => {
                    let submit = if *mono {
                        dsound_mixer_submit_xbox_adpcm_mono
                    } else {
                        dsound_mixer_submit_xbox_adpcm
                    };
                    submit(m, *key, *ticks, *source_hz, data.as_ptr(), data.len())
                }
                SubmitPcm16Stereo {
                    key,
                    ticks,
                    source_hz,
                    pcm,
                } => {
                    if pcm.len() % 2 != 0 {
                        return false;
                    }
                    dsound_mixer_submit_pcm16_stereo(
                        m,
                        *key,
                        *ticks,
                        *source_hz,
                        pcm.as_ptr(),
                        pcm.len() / 2,
                    )
                }
                CutStream { key, ticks } => dsound_mixer_cut_stream(m, *key, *ticks),
                SetStreamRunning {
                    key,
                    ticks,
                    running,
                } => dsound_mixer_set_stream_running(m, *key, *ticks, *running),
                SetStreamFrequency {
                    key,
                    ticks,
                    source_hz,
                } => dsound_mixer_set_stream_frequency(m, *key, *ticks, *source_hz),
                SetStreamVolume { key, millibels } => {
                    dsound_mixer_set_stream_volume(m, *key, *millibels)
                }
                ResetStreamVolume { key } => dsound_mixer_reset_stream_volume(m, *key),
                PlayBuffer {
                    key,
                    serial,
                    ticks,
                    frequency,
                    pcm,
                    paused,
                    looping,
                    loop_start,
                    loop_end,
                    volume,
                } => dsound_mixer_play_mono_buffer(
                    m,
                    *key,
                    *serial,
                    *ticks,
                    *frequency,
                    pcm.as_ptr(),
                    pcm.len(),
                    *paused,
                    *looping,
                    *loop_start,
                    *loop_end,
                    *volume,
                ),
                PauseBuffer {
                    key,
                    serial,
                    ticks,
                    paused,
                } => dsound_mixer_pause_buffer(m, *key, *serial, *ticks, *paused),
                StopBuffer { key, serial, ticks } => {
                    dsound_mixer_stop_buffer(m, *key, *serial, *ticks)
                }
                FrequencyBuffer {
                    key,
                    serial,
                    ticks,
                    frequency,
                } => dsound_mixer_frequency_buffer(m, *key, *serial, *ticks, *frequency),
                VolumeBuffer {
                    key,
                    serial,
                    ticks,
                    volume,
                } => dsound_mixer_volume_buffer(m, *key, *serial, *ticks, *volume),
                LoopBuffer {
                    key,
                    serial,
                    ticks,
                    loop_start,
                    loop_end,
                } => dsound_mixer_loop_buffer(m, *key, *serial, *ticks, *loop_start, *loop_end),
                ResetBuffers => {
                    dsound_mixer_reset_buffers(m);
                    true
                }
                RouteStream {
                    key,
                    serial,
                    ticks,
                    routing: r,
                } => {
                    let route = Route {
                        volume: r.volume,
                        headroom: r.headroom,
                        aggregate: r.aggregate,
                        count: r.count,
                        bins: r.bins,
                        bin_volume: r.bin_volume,
                    };
                    dsound_mixer_route_stream(m, *key, *serial, *ticks, &route)
                }
                FrequencyStream {
                    key,
                    serial,
                    ticks,
                    base_hz,
                    old_hz,
                    new_hz,
                } => dsound_mixer_frequency_stream(
                    m, *key, *serial, *ticks, *base_hz, *old_hz, *new_hz,
                ),
                FormatStream {
                    key,
                    serial,
                    ticks,
                    old_hz,
                    new_hz,
                } => dsound_mixer_format_stream(m, *key, *serial, *ticks, *old_hz, *new_hz),
                EnableHeadroom { enabled } => dsound_mixer_enable_mixbin_headroom(m, *enabled),
                BindHeadroom { identity } => dsound_mixer_bind_mixbin_headroom(m, *identity),
                SetHeadroom {
                    identity,
                    ticks,
                    bin,
                    headroom,
                } => dsound_mixer_set_mixbin_headroom(m, *identity, *ticks, *bin, *headroom),
            }
        }
    }

    fn render_until(&mut self, now: u64, capacity: usize) -> Vec<i16> {
        // Keep each comparison block bounded; callers can drain the same tick repeatedly.
        assert!(capacity <= 1_048_576, "offline render block exceeds bound");
        let mut pcm = vec![0; capacity * 2];
        let mut frames = 0;
        // SAFETY: writable storage contains capacity stereo frames; count is a live usize.
        assert!(
            with_render_lock(|| unsafe {
                dsound_mixer_render_until(
                    self.0.as_ptr(),
                    now,
                    pcm.as_mut_ptr(),
                    capacity,
                    &mut frames,
                )
            }),
            "C render refused"
        );
        assert!(frames <= capacity);
        pcm.truncate(frames * 2);
        pcm
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::TryLockError;

    #[test]
    fn c_renderer_entry_holds_lock() {
        let observes_lock = || matches!(RENDER_LOCK.try_lock(), Err(TryLockError::WouldBlock));
        // Removing/bypassing synchronization is the negative control. No timing or sleeps.
        assert!(!observes_lock());
        assert!(with_render_lock(observes_lock));
        assert!(!observes_lock());
    }
}
