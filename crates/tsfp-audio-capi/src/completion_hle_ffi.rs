// SPDX-License-Identifier: GPL-3.0-or-later
//! T3933: Rust-owned passive completion front-end for the Rust audio host. Replaces `src/audio/dsound_completion.c`
//! (kept as the `TSFP_RUST_AUDIO_HOST=0` fallback) under the same C symbols. State transitions are the
//! `tsfp_audio::completion::Completion` model; this module adds what the model leaves to the adapter: the
//! measured-scope admission against the C snapshots, guest word writes, the audio-runtime verdicts, the command-log and
//! capture observation lines, the census, the announce-once banners and the refuse/fatal path. The order of every side
//! effect follows the C file.

use std::ffi::{CString, c_char, c_void};
use std::fs::File;
use std::io::Write;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{LazyLock, Mutex, MutexGuard};

use tsfp_audio::completion::{Completion, Error, Event};

use crate::completion_layout::*;
use crate::movie_device_hle_ffi::tsfp_audio_hle_register_handler_rust;

type FatalFn = unsafe extern "C" fn(u32, *const c_char);
type ClockFn = unsafe extern "C" fn() -> u64;
type VolumeFn = unsafe extern "C" fn(u32, u64, i32) -> bool;
type DsoundHandler = unsafe extern "C" fn(*mut c_void) -> u32;

unsafe extern "C" {
    fn dsound_hle_log_line(line: *const c_char);
    fn dsound_hle_register_rust(address: u32) -> bool;
    fn kernel_guest_read_u32(address: u32, out: *mut u32) -> bool;
    fn kernel_guest_write_u32(address: u32, value: u32) -> bool;
    fn kernel_frame_arg(frame: *const c_void, index: u32, out: *mut u32) -> bool;
    fn kernel_host_pid() -> i32;
    fn kernel_clock_peek() -> u64;
    fn dsound_stream_get_snapshot(stream: u32, out: *mut c_void) -> bool;
    fn dsound_buffer_get_snapshot(buffer: u32, out: *mut c_void) -> bool;
    fn dsound_buffer_set_completion_volume(volume: Option<VolumeFn>);
    fn dsound_audio_cmdlog_line(line: *const c_char);
    fn dsound_audio_runtime_active() -> bool;
    fn dsound_audio_runtime_clock_frequency() -> u64;
    fn dsound_audio_runtime_reset_buffers();
    fn dsound_audio_runtime_set_completion_route_policy_rust(enabled: bool, clock_frequency: u64);
    fn dsound_audio_runtime_route_stream_admitted_now(
        key: u32,
        serial: u64,
        completion_enabled: bool,
        completion_clock_frequency: u64,
        routing: *const c_void,
    ) -> bool;
    fn dsound_audio_runtime_frequency_stream(
        key: u32,
        serial: u64,
        ticks: u64,
        base_hz: u32,
        old_hz: u32,
        new_hz: u32,
    ) -> bool;
    fn dsound_audio_runtime_format_stream(
        key: u32,
        serial: u64,
        ticks: u64,
        old_hz: u32,
        new_hz: u32,
    ) -> bool;
    fn dsound_audio_runtime_set_stream_running(key: u32, ticks: u64, running: bool) -> bool;
    fn dsound_audio_runtime_submit_guest(
        key: u32,
        ticks: u64,
        rate: u32,
        host_pid: i32,
        guest_data: u32,
        bytes: usize,
        mono: bool,
    ) -> bool;
    fn dsound_audio_runtime_play_buffer_guest(
        key: u32,
        serial: u64,
        ticks: u64,
        frequency: u32,
        host_pid: i32,
        guest_data: u32,
        bytes: usize,
        paused: bool,
        looping: bool,
        loop_start: usize,
        loop_end: usize,
        volume: i32,
    ) -> bool;
    fn dsound_audio_runtime_pause_buffer(key: u32, serial: u64, ticks: u64, paused: bool) -> bool;
    fn dsound_audio_runtime_stop_buffer(key: u32, serial: u64, ticks: u64) -> bool;
    fn dsound_audio_runtime_last_buffer_refusal() -> *const c_char;
    fn dsound_audio_runtime_frequency_buffer(
        key: u32,
        serial: u64,
        ticks: u64,
        frequency: u32,
    ) -> bool;
    fn dsound_audio_runtime_volume_buffer(key: u32, serial: u64, ticks: u64, volume: i32) -> bool;
    fn dsound_audio_runtime_loop_buffer(
        key: u32,
        serial: u64,
        ticks: u64,
        loop_start: usize,
        loop_end: usize,
    ) -> bool;
    fn dsound_audio_runtime_mixbin_headroom(
        identity: u64,
        ticks: u64,
        bin: u32,
        headroom: u32,
    ) -> bool;
    fn dsound_audio_runtime_bind_mixbin_headroom(identity: u64) -> bool;
    fn tsfp_audio_stream_completion_publish_rust(
        host_pid: i32,
        completed: u32,
        size: u32,
        status: u32,
    ) -> bool;
    fn tsfp_audio_stream_packet_decode_rust(
        host_pid: i32,
        packet_address: u32,
        words_out: *mut u32,
    ) -> bool;
    fn tsfp_audio_stream_packet_buffer_probe_rust(
        host_pid: i32,
        buffer_address: u32,
        buffer_bytes: u32,
    ) -> bool;
    fn tsfp_audio_stream_pending_publish_rust(
        host_pid: i32,
        stream: u32,
        completed: u32,
        status: u32,
    ) -> bool;
    fn tsfp_audio_stream_packet_classify_rust(
        format: *const u8,
        format_bytes: u32,
        packet_bytes: u32,
        kind_out: *mut u32,
        rate_out: *mut u32,
    ) -> bool;
}

const STREAM_GET_STATUS: u32 = 0x0040_73D3;
const STREAM_PROCESS: u32 = 0x0040_7424;
const STREAM_GET_INFO: u32 = 0x0040_72D4;
const BUFFER_PLAY: u32 = 0x0040_7A80;
const BUFFER_GET_STATUS: u32 = 0x0040_7AF8;
const BUFFER_STOP: u32 = 0x0040_7AA4;
const PENDING_STATUS: u32 = 0x8000_000A;
const PACKET_LIST_FULL: u32 = 0x8878_0032;
const MAX_QUEUE: u32 = 8;
const BUFFER_BLOCK: u32 = 36;
const BUFFER_BLOCK_SAMPLES: u64 = 64;
const KERNEL_CLOCK_FREQUENCY_HZ: u64 = 733_333_333;
const AUDIO_BUFFER_MAX_BYTES: u32 = 36 * 32768;
const CENSUS_MAX: usize = 12;
const CAPTURE_LIMIT: u64 = 64 * 1024 * 1024;

/// `dsound_completion_stats`.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct CStats {
    stream_packets: u32,
    stream_completed: u32,
    buffer_plays: u32,
    buffer_finished: u32,
    observations: u32,
    buffer_stops: u32,
    stream_aborted: u32,
}

/// `dsound_completion_census_entry`.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct CensusEntry {
    tag: u32,
    channels: u32,
    rate: u32,
    block: u32,
    bits: u32,
    packets: u32,
    mixed: u32,
    bytes: u32,
    first_ms: u64,
    last_ms: u64,
}

/// `dsound_completion_buffer_position`.
#[repr(C)]
pub struct CBufferPosition {
    position_samples: u64,
    total_samples: u64,
    frequency: u32,
    playing: bool,
    looping: bool,
    paused: bool,
}

/// The opt-in ordered completion observer (`dsound_completion_capture.h`).
struct Capture {
    file: Option<File>,
    checked: bool,
    failed: bool,
    closed: bool,
    hash: u64,
    lines: u64,
    bytes: u64,
    process_pending: bool,
    stream: u32,
    serial: u64,
    ticks: u64,
}

impl Capture {
    const fn new() -> Self {
        Self {
            file: None,
            checked: false,
            failed: false,
            closed: false,
            hash: 1469598103934665603,
            lines: 0,
            bytes: 0,
            process_pending: false,
            stream: 0,
            serial: 0,
            ticks: 0,
        }
    }

    fn write(&mut self, line: &str) {
        let Some(file) = self.file.as_mut() else {
            return;
        };
        if self.failed || self.closed {
            return;
        }
        let n = line.len() as u64;
        if n == 0
            || n > 255
            || self.bytes + n + 64 > CAPTURE_LIMIT
            || file.write_all(line.as_bytes()).is_err()
        {
            self.failed = true;
            return;
        }
        for byte in line.bytes() {
            self.hash = (self.hash ^ u64::from(byte)).wrapping_mul(1099511628211);
        }
        self.bytes += n;
        self.lines += 1;
    }

    fn open(&mut self, frequency: u64, dowork: bool) {
        if self.checked {
            return;
        }
        self.checked = true;
        let Some(path) =
            std::env::var_os("TSFP_AUDIO_COMPLETION_CAPTURE").filter(|p| !p.is_empty())
        else {
            return;
        };
        let Ok(file) = File::options().write(true).create_new(true).open(path) else {
            return;
        };
        self.file = Some(file);
        self.write("ACMP1\n");
        self.write(&format!("CFG {frequency} {}\n", u32::from(dowork)));
    }

    fn observe(&mut self, line: &str, frequency: u64, dowork: bool) {
        // Buffer-only records do not mutate stream completion state.
        if line.starts_with("CB")
            || line.starts_with("CCFG ")
            || line.starts_with("CSD ")
            || line.starts_with("CSM ")
        {
            return;
        }
        self.open(frequency, dowork);
        if line.starts_with("CSP ") {
            if line.ends_with(" 1") {
                return; // full was recorded at begin
            }
            let mut fields = line.split_whitespace().skip(1);
            let parsed = (|| {
                Some((
                    fields.next()?.parse::<u32>().ok()?,
                    fields.next()?.parse::<u64>().ok()?,
                    fields.next()?.parse::<u64>().ok()?,
                ))
            })();
            if parsed != Some((self.stream, self.serial, self.ticks)) || !self.process_pending {
                self.write("GAP process-pairing\n");
            }
            self.process_pending = false;
        } else if self.process_pending {
            self.write("GAP interleaved-process\n");
        }
        let unsupported = line.starts_with("CSF ");
        if line.len() + 1 >= 256 {
            self.failed = true;
            return;
        }
        self.write(&format!("{line}\n"));
        if unsupported {
            self.write("GAP unsupported-format-or-frequency\n");
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn format(
        &mut self,
        stream: u32,
        serial: u64,
        ticks: u64,
        old_hz: u32,
        new_hz: u32,
        address: u32,
        accepted: bool,
        frequency: u64,
        dowork: bool,
    ) {
        self.open(frequency, dowork);
        if self.process_pending {
            self.write("GAP interleaved-process\n");
        }
        self.write(&format!(
            "CSM {stream} {serial} {ticks} {old_hz} {new_hz} {address} {}\n",
            u32::from(accepted)
        ));
        self.write("GAP unsupported-format-or-frequency\n");
    }

    #[allow(clippy::too_many_arguments)]
    fn begin(
        &mut self,
        stream: u32,
        serial: u64,
        ticks: u64,
        max_packets: u32,
        full: bool,
        frequency: u64,
        dowork: bool,
    ) {
        self.open(frequency, dowork);
        if self.file.is_none() {
            return;
        }
        if self.process_pending {
            self.write("GAP nested-process\n");
        }
        self.write(&format!(
            "CPB {stream} {serial} {ticks} {max_packets} {}\n",
            u32::from(full)
        ));
        self.process_pending = !full;
        self.stream = stream;
        self.serial = serial;
        self.ticks = ticks;
    }

    fn gap(&mut self) {
        if self.file.is_some() {
            self.write("GAP configuration-change\n");
        }
    }

    fn seal(&mut self, quiescent: bool) {
        if self.file.is_none() || self.closed {
            return;
        }
        if quiescent && !self.failed && !self.process_pending {
            let (lines, hash) = (self.lines, self.hash);
            let file = self.file.as_mut().unwrap();
            if writeln!(file, "END {lines} {hash:016x}").is_err() || file.flush().is_err() {
                self.failed = true;
            }
        }
        self.file = None;
        self.closed = true;
    }
}

struct State {
    model: Completion,
    enabled: bool,
    announced_stream: bool,
    announced_buffer: bool,
    announced_stop: bool,
    announced_audio_skip: bool,
    census: Vec<CensusEntry>,
    capture: Capture,
}

impl State {
    fn new() -> Self {
        Self {
            model: Completion::new(KERNEL_CLOCK_FREQUENCY_HZ),
            enabled: false,
            announced_stream: false,
            announced_buffer: false,
            announced_stop: false,
            announced_audio_skip: false,
            census: Vec::new(),
            capture: Capture::new(),
        }
    }

    fn frequency(&self) -> u64 {
        self.model.clock_hz()
    }

    fn dowork(&self) -> bool {
        self.model.dowork_delivery
    }

    /// The command-log observation line of the oracle's `OBS` macro (no effect without the log/capture variables).
    fn obs(&mut self, line: String) {
        let line = if line.len() > 255 {
            line[..255].to_string()
        } else {
            line
        };
        if let Ok(c) = CString::new(line.as_str()) {
            // SAFETY: NUL-terminated local string for a synchronous call.
            unsafe { dsound_audio_cmdlog_line(c.as_ptr()) };
        }
        let (frequency, dowork) = (self.frequency(), self.dowork());
        self.capture.observe(&line, frequency, dowork);
    }

    /// Ticks of running time for `units` at `per_second` units a second, rounded up (oracle `ticks_for`).
    fn ticks_for(&self, units: u64, per_second: u64) -> u64 {
        if per_second == 0 {
            return u64::MAX;
        }
        units
            .wrapping_mul(self.frequency())
            .wrapping_add(per_second - 1)
            / per_second
    }

    fn census_note(&mut self, format: &[u8; 20], bytes: u32, mixed: bool) {
        let le16 = |o: usize| u32::from(u16::from_le_bytes([format[o], format[o + 1]]));
        let le32 =
            |o: usize| u32::from_le_bytes([format[o], format[o + 1], format[o + 2], format[o + 3]]);
        let per_ms = (self.frequency() / 1000).max(1);
        let now_ms = now_ticks() / per_ms;
        let (tag, channels, rate, block, bits) = (le16(0), le16(2), le32(4), le16(12), le16(14));
        let existing = self.census.iter().position(|e| {
            e.tag == tag
                && e.channels == channels
                && e.rate == rate
                && e.block == block
                && e.bits == bits
                && (e.mixed != 0) == mixed
        });
        let i = match existing {
            Some(i) => i,
            None if self.census.len() == CENSUS_MAX => CENSUS_MAX - 1,
            None => {
                self.census.push(CensusEntry {
                    tag,
                    channels,
                    rate,
                    block,
                    bits,
                    first_ms: now_ms,
                    last_ms: now_ms,
                    ..CensusEntry::default()
                });
                self.census.len() - 1
            }
        };
        let e = &mut self.census[i];
        e.packets += 1;
        e.bytes = e.bytes.wrapping_add(bytes);
        e.last_ms = now_ms;
        if mixed {
            e.mixed = 1;
        }
    }
}

static STATE: LazyLock<Mutex<State>> = LazyLock::new(|| Mutex::new(State::new()));
static FATAL: Mutex<Option<FatalFn>> = Mutex::new(None);
static CLOCK: AtomicUsize = AtomicUsize::new(0);

fn lock() -> MutexGuard<'static, State> {
    STATE.lock().unwrap()
}

fn now_ticks() -> u64 {
    let raw = CLOCK.load(Ordering::Acquire);
    if raw == 0 {
        // SAFETY: plain C accessor of the kernel virtual clock.
        unsafe { kernel_clock_peek() }
    } else {
        // SAFETY: the pointer was stored from a `dsound_completion_clock_fn`.
        unsafe { std::mem::transmute::<usize, ClockFn>(raw)() }
    }
}

fn log(text: &str) {
    if let Ok(line) = CString::new(text) {
        // SAFETY: NUL-terminated local string for a synchronous call.
        unsafe { dsound_hle_log_line(line.as_ptr()) };
    }
}

fn refuse(entry: u32, reason: &str) -> ! {
    let fatal = *FATAL.lock().unwrap();
    log(&format!(
        "dsound passive completion {entry:#x} refused: {reason}\n"
    ));
    if let (Some(fatal), Ok(reason)) = (fatal, CString::new(reason)) {
        // SAFETY: NUL-terminated string for a synchronous call.
        unsafe { fatal(entry, reason.as_ptr()) };
    }
    std::process::abort();
}

fn overflow() -> ! {
    log("dsound: completion source clock overflow\n");
    std::process::abort();
}

fn read_u32(address: u32) -> Option<u32> {
    let mut value = 0u32;
    // SAFETY: checked guest read into a live local.
    unsafe { kernel_guest_read_u32(address, &mut value) }.then_some(value)
}

fn write_u32(address: u32, value: u32) -> bool {
    // SAFETY: checked guest write.
    unsafe { kernel_guest_write_u32(address, value) }
}

fn host_pid() -> i32 {
    // SAFETY: plain C accessor.
    unsafe { kernel_host_pid() }
}

fn runtime_active() -> bool {
    // SAFETY: plain C accessor.
    unsafe { dsound_audio_runtime_active() }
}

fn runtime_clock_matches(st: &State) -> bool {
    // SAFETY: plain C accessor.
    unsafe { dsound_audio_runtime_clock_frequency() == st.frequency() }
}

/// `word_ok`: a guest word the model will later write must be null or unaliased and writable now.
fn word_ok(address: u32, stream_address: u32) -> bool {
    if address == 0 {
        return true;
    }
    let (a, s) = (u64::from(address), u64::from(stream_address));
    a + 4 <= 0x1_0000_0000
        && !(a < s + 40 && s < a + 4)
        && read_u32(address).is_some_and(|old| write_u32(address, old))
}

/// Guest output word: not null, below 4 GiB, outside the stream header, readable and writable.
fn output_ok(output: u32, header: u32, bytes: u64) -> bool {
    let (o, h) = (u64::from(output), u64::from(header));
    output != 0
        && o + bytes <= 0x1_0000_0000
        && !(o < h + 40 && h < o + bytes)
        && read_u32(output).is_some_and(|old| write_u32(output, old))
}

/// The packet words the model finished, delivered to the guest in order (Rust host publication) with their
/// observation lines.
fn deliver(st: &mut State, events: &[Event]) {
    for e in events {
        // SAFETY: plain guest publication helper of the Rust host.
        unsafe {
            tsfp_audio_stream_completion_publish_rust(
                host_pid(),
                e.completed,
                e.size,
                e.status_addr,
            )
        };
        let (frequency, dowork) = (st.frequency(), st.dowork());
        st.capture.open(frequency, dowork);
        st.capture.write("GAP rust-host-publication\n");
        st.obs(format!(
            "CSD {} {} {} {}",
            e.stream, e.completed, e.size, e.status_addr
        ));
    }
}

/// `dsound_completion_set_enabled`.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_completion_set_enabled(value: bool) {
    {
        let mut st = lock();
        st.capture.gap();
        st.enabled = value;
        // SAFETY: plain C configuration call.
        unsafe { dsound_audio_runtime_set_completion_route_policy_rust(value, st.frequency()) };
    }
    // Quiescent configuration: the completion lock is released before the buffer lock is taken.
    // SAFETY: registers a function pointer (or none) with the C buffer module.
    unsafe {
        dsound_buffer_set_completion_volume(value.then_some(
            dsound_completion_buffer_volume as unsafe extern "C" fn(u32, u64, i32) -> bool,
        ))
    };
}

/// `dsound_completion_enabled`.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_completion_enabled() -> bool {
    lock().enabled
}

/// `dsound_completion_set_dowork_delivery`.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_completion_set_dowork_delivery(value: bool) {
    let mut st = lock();
    st.capture.gap();
    st.model.dowork_delivery = value;
    st.obs(format!("CCFG {}", i32::from(value)));
}

/// `dsound_completion_dowork_delivery`.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_completion_dowork_delivery() -> bool {
    lock().dowork()
}

/// `dsound_completion_set_clock`.
///
/// # Safety
/// `clock` is null or `uint64_t (*)(void)`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dsound_completion_set_clock(clock: Option<ClockFn>, frequency: u64) {
    let mut st = lock();
    st.capture.gap();
    CLOCK.store(clock.map_or(0, |f| f as usize), Ordering::Release);
    let hz = if clock.is_some() {
        frequency
    } else {
        KERNEL_CLOCK_FREQUENCY_HZ
    };
    st.model.set_clock_hz(hz);
    // SAFETY: plain C configuration call.
    unsafe { dsound_audio_runtime_set_completion_route_policy_rust(st.enabled, hz) };
}

/// `dsound_completion_set_fatal`.
///
/// # Safety
/// `fatal` is null or `void (*)(uint32_t, const char *)`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dsound_completion_set_fatal(fatal: Option<FatalFn>) {
    *FATAL.lock().unwrap() = fatal;
}

/// `dsound_completion_reset`.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_completion_reset() {
    let mut st = lock();
    st.capture.gap();
    st.model.reset();
    // SAFETY: plain C reset of the runtime buffer voices.
    unsafe { dsound_audio_runtime_reset_buffers() };
    st.census.clear();
    st.announced_stream = false;
    st.announced_buffer = false;
    st.announced_stop = false;
    st.announced_audio_skip = false;
}

/// `dsound_completion_capture_finish`.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_completion_capture_finish(quiescent: bool) {
    lock().capture.seal(quiescent);
}

/// `dsound_completion_stream_census`.
///
/// # Safety
/// `out` is null or has `capacity` writable entries.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dsound_completion_stream_census(
    out: *mut CensusEntry,
    capacity: usize,
) -> usize {
    let st = lock();
    let count = st.census.len().min(capacity);
    if !out.is_null() && count != 0 {
        // SAFETY: `count <= capacity` writable entries by contract.
        unsafe { std::ptr::copy_nonoverlapping(st.census.as_ptr(), out, count) };
    }
    count
}

/// `dsound_completion_get_stats`.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_completion_get_stats() -> CStats {
    let s = lock().model.stats;
    CStats {
        stream_packets: s.stream_packets as u32,
        stream_completed: s.stream_completed as u32,
        buffer_plays: s.buffer_plays as u32,
        buffer_finished: s.buffer_finished as u32,
        observations: s.observations as u32,
        buffer_stops: s.buffer_stops as u32,
        stream_aborted: s.stream_aborted as u32,
    }
}

/// `dsound_completion_stream_routing`.
///
/// # Safety
/// `routing` is the C routing record (opaque here).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dsound_completion_stream_routing(
    stream: u32,
    serial: u64,
    routing: *const c_void,
) -> bool {
    let st = lock();
    // SAFETY: the runtime reads the record synchronously.
    unsafe {
        dsound_audio_runtime_route_stream_admitted_now(
            stream,
            serial,
            st.enabled,
            st.frequency(),
            routing,
        )
    }
}

fn stream_frequency_change(
    stream: u32,
    serial: u64,
    ticks: u64,
    old_hz: u32,
    new_hz: u32,
    pcm: bool,
) -> bool {
    if stream == 0
        || serial == 0
        || !(8000..=96000).contains(&old_hz)
        || !(8000..=96000).contains(&new_hz)
    {
        return false;
    }
    let mut st = lock();
    let mut accepted = false;
    let mut events = Vec::new();
    if st.enabled && ticks == now_ticks() {
        let clock_ok = runtime_clock_matches(&st);
        let result = st.model.stream_frequency_with_base(
            stream,
            serial,
            ticks,
            old_hz,
            new_hz,
            pcm,
            &mut |base| {
                // SAFETY: plain C runtime verdict.
                clock_ok
                    && unsafe {
                        dsound_audio_runtime_frequency_stream(
                            stream, serial, ticks, base, old_hz, new_hz,
                        )
                    }
            },
        );
        match result {
            Ok((ok, delivered)) => {
                accepted = ok;
                events = delivered;
            }
            Err(_) => overflow(),
        }
    }
    deliver(&mut st, &events);
    st.obs(format!(
        "CSF {stream} {serial} {ticks} {old_hz} {new_hz} {} {}",
        i32::from(pcm),
        i32::from(accepted)
    ));
    accepted
}

/// `dsound_completion_stream_frequency`.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_completion_stream_frequency(
    stream: u32,
    serial: u64,
    ticks: u64,
    old_hz: u32,
    new_hz: u32,
) -> bool {
    stream_frequency_change(stream, serial, ticks, old_hz, new_hz, false)
}

/// `dsound_completion_stream_frequency_pcm`.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_completion_stream_frequency_pcm(
    stream: u32,
    serial: u64,
    ticks: u64,
    old_hz: u32,
    new_hz: u32,
) -> bool {
    stream_frequency_change(stream, serial, ticks, old_hz, new_hz, true)
}

/// `dsound_completion_note_frequency`.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_completion_note_frequency(
    stream: u32,
    serial: u64,
    old_hz: u32,
    new_hz: u32,
) -> bool {
    let ticks = now_ticks();
    let pcm = runtime_active();
    stream_frequency_change(stream, serial, ticks, old_hz, new_hz, pcm)
}

/// `dsound_completion_note_format`.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_completion_note_format(
    stream: u32,
    serial: u64,
    old_hz: u32,
    new_hz: u32,
    format_address: u32,
) -> bool {
    if stream == 0
        || serial == 0
        || !(8000..=96000).contains(&old_hz)
        || !(8000..=48000).contains(&new_hz)
    {
        return false;
    }
    let mut st = lock();
    let ticks = now_ticks();
    let mut accepted = false;
    let mut events = Vec::new();
    if st.enabled && runtime_clock_matches(&st) {
        let fmt = u64::from(format_address);
        let result = st.model.stream_format_with(
            stream,
            serial,
            ticks,
            old_hz,
            new_hz,
            &mut |pending| {
                pending.iter().all(|&(completed, status)| {
                    [completed, status].into_iter().all(|address| {
                        let a = u64::from(address);
                        (address == 0 || !(a < fmt + 20 && fmt < a + 4)) && word_ok(address, stream)
                    })
                })
            },
            // SAFETY: plain C runtime verdict.
            &mut || unsafe {
                dsound_audio_runtime_format_stream(stream, serial, ticks, old_hz, new_hz)
            },
        );
        match result {
            Ok((ok, aborts)) => {
                accepted = ok;
                events = aborts;
            }
            Err(_) => overflow(),
        }
    }
    for e in &events {
        if e.completed != 0 {
            let _ = write_u32(e.completed, e.size);
        }
        if e.status_addr != 0 {
            let _ = write_u32(e.status_addr, e.status);
        }
    }
    st.obs(format!(
        "CSM {stream} {serial} {ticks} {old_hz} {new_hz} {}",
        i32::from(accepted)
    ));
    let (frequency, dowork) = (st.frequency(), st.dowork());
    st.capture.format(
        stream,
        serial,
        ticks,
        old_hz,
        new_hz,
        format_address,
        accepted,
        frequency,
        dowork,
    );
    if !accepted {
        let (state_serial, queued, current, last) =
            st.model.stream_info(stream, serial).unwrap_or((0, 0, 0, 0));
        // SAFETY: plain C accessor.
        let pcm_clock = unsafe { dsound_audio_runtime_clock_frequency() };
        log(&format!(
            "dsound: format transaction refused stream={stream:#x} serial={serial} state-serial={state_serial} queued={queued} old={old_hz} current={current} new={new_hz} now={ticks} last={last} clock={frequency} pcm-clock={pcm_clock}\n"
        ));
    }
    accepted
}

/// `dsound_completion_note_pause`.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_completion_note_pause(stream: u32, mode: u32) {
    let tick = now_ticks();
    let mut st = lock();
    if !st.enabled {
        return;
    }
    match st.model.pause_note_checked(stream, mode, tick) {
        Err(_) => overflow(),
        Ok(Some(events)) => {
            deliver(&mut st, &events);
            st.obs(format!("CPN {stream} {mode} {tick}"));
        }
        Ok(None) => {}
    }
    let audio_enabled = runtime_active();
    drop(st);
    if audio_enabled {
        // SAFETY: plain C runtime call.
        unsafe { dsound_audio_runtime_set_stream_running(stream, tick, mode == 0) };
    }
}

fn announce_text(audio: bool, stream: bool) -> &'static str {
    match (stream, audio) {
        (true, false) => {
            "dsound: explicit PASSIVE COMPLETION streams; Process packets complete after size/average bytes of RUNNING virtual clock time (a paused stream consumes none), status words from the measured original, the drain stopping the voice is INFERRED; no samples are produced, no voice or mixer, no event or callback\n"
        }
        (true, true) => {
            "dsound: explicit PASSIVE COMPLETION streams; Process packets complete after size/average bytes of RUNNING virtual clock time; opt-in stereo Xbox ADPCM packets feed the deterministic PCM mixer on the same virtual clock (timing INFERRED); no event or callback\n"
        }
        (false, false) => {
            "dsound: explicit PASSIVE COMPLETION buffers; Play marks the buffer playing for samples/frequency of virtual clock time (looping never ends), status 1 or 5, then 0 (INFERRED, the drain is hardware); no voice, mixer or samples\n"
        }
        (false, true) => {
            "dsound: opt-in real mono Xbox ADPCM buffer voices; original-backed restart/pause/loop/Stop-tail transport feeds PCM on the completion clock; timing/resampling/stereo routing/speaker gain INFERRED, immutable Play data snapshot, no spatial DSP/HRTF emulation\n"
        }
    }
}

fn stream_snapshot(stream: u32) -> Option<Snapshot<STREAM_SNAPSHOT_BYTES>> {
    let mut snap = Snapshot::<STREAM_SNAPSHOT_BYTES>::zeroed();
    // SAFETY: the record is at least `sizeof(dsound_stream_snapshot)` bytes (guarded by completion_layout_c.rs) and 8-aligned.
    unsafe { dsound_stream_get_snapshot(stream, snap.as_mut_ptr()) }.then_some(snap)
}

fn buffer_snapshot(buffer: u32) -> Option<Snapshot<BUFFER_SNAPSHOT_BYTES>> {
    let mut snap = Snapshot::<BUFFER_SNAPSHOT_BYTES>::zeroed();
    // SAFETY: as `stream_snapshot`.
    unsafe { dsound_buffer_get_snapshot(buffer, snap.as_mut_ptr()) }.then_some(snap)
}

/// The stream format the packets are measured against: the SetFormat record when set, else the creation scope's.
fn stream_format_of(snap: &Snapshot<STREAM_SNAPSHOT_BYTES>) -> [u8; 20] {
    if snap.u32_at(STREAM_FORMAT_SETS) != 0 {
        snap.bytes_at::<20>(STREAM_FORMAT)
    } else {
        snap.bytes_at::<20>(STREAM_SCOPE_FORMAT)
    }
}

fn le16(bytes: &[u8], offset: usize) -> u32 {
    u32::from(u16::from_le_bytes([bytes[offset], bytes[offset + 1]]))
}

fn le32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

/// `dsound_completion_stream_status`.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_completion_stream_status(stream: u32, output: u32) -> u32 {
    let snap = match (dsound_completion_enabled(), stream_snapshot(stream)) {
        (true, Some(snap)) => snap,
        _ => refuse(
            STREAM_GET_STATUS,
            "stream is not owned by the passive adapter",
        ),
    };
    if snap.u8_at(STREAM_VOLUME_SEEN) == 0 {
        refuse(
            STREAM_GET_STATUS,
            "only a passive stream with its startup volume recorded has a status",
        );
    }
    if !output_ok(output, stream, 4) {
        refuse(
            STREAM_GET_STATUS,
            "status output is null, aliases the stream header or is not writable",
        );
    }
    let (serial, max_packets) = (
        snap.u64_at(STREAM_LEASE_SERIAL),
        snap.u32_at(STREAM_MAX_PACKETS),
    );
    let mut st = lock();
    let status_tick = now_ticks();
    let (value, events) = match st
        .model
        .stream_status(stream, serial, status_tick, max_packets)
    {
        Ok(result) => result,
        Err(Error::NoFreeState) => {
            drop(st);
            refuse(STREAM_GET_STATUS, "no free passive stream state");
        }
        Err(Error::ClockOverflow) => overflow(),
    };
    deliver(&mut st, &events);
    st.obs(format!(
        "CSS {stream} {serial} {status_tick} {max_packets} {value}"
    ));
    drop(st);
    if !write_u32(output, value) {
        refuse(STREAM_GET_STATUS, "status output is not writable");
    }
    0
}

/// `dsound_completion_stream_info`.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_completion_stream_info(stream: u32, output: u32) -> u32 {
    let snap = match (dsound_completion_enabled(), stream_snapshot(stream)) {
        (true, Some(snap)) => snap,
        _ => refuse(
            STREAM_GET_INFO,
            "stream is not owned by the passive adapter",
        ),
    };
    let format_set = snap.u32_at(STREAM_FORMAT_SETS) != 0;
    let info_format = stream_format_of(&snap);
    let block = if format_set {
        le16(&snap.bytes_at::<20>(STREAM_FORMAT), 12)
    } else {
        u32::from(snap.u16_at(STREAM_BLOCK_ALIGN))
    };
    let channels = le16(&info_format, 2);
    if !((block == 72 && channels == 2) || (block == 36 && channels == 1))
        || le16(&info_format, 0) != 0x69
        || le16(&info_format, 14) != 4
    {
        refuse(
            STREAM_GET_INFO,
            "only the measured 72 byte stereo and 36 byte mono ADPCM block streams answer GetInfo",
        );
    }
    let words = [5u32, block, 0, 2 * block];
    if !output_ok(output, stream, 16) {
        refuse(
            STREAM_GET_INFO,
            "info output is null, aliases the stream header or is not writable",
        );
    }
    for (i, word) in words.iter().enumerate() {
        if !write_u32(output + 4 * i as u32, *word) {
            refuse(STREAM_GET_INFO, "info output is not writable");
        }
    }
    0
}

/// `dsound_completion_stream_process`.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_completion_stream_process(
    stream: u32,
    packet_address: u32,
    output_packet: u32,
) -> u32 {
    let snap = match (dsound_completion_enabled(), stream_snapshot(stream)) {
        (true, Some(snap)) => snap,
        _ => refuse(STREAM_PROCESS, "stream is not owned by the passive adapter"),
    };
    if snap.u8_at(STREAM_VOLUME_SEEN) == 0 || snap.u8_at(STREAM_DISCONTINUITY_SEEN) == 0 {
        refuse(
            STREAM_PROCESS,
            "only a started passive stream takes packets",
        );
    }
    if output_packet != 0 {
        refuse(
            STREAM_PROCESS,
            "Process output packet must be NULL (no codec output is modelled)",
        );
    }
    let serial = snap.u64_at(STREAM_LEASE_SERIAL);
    let max = snap.u32_at(STREAM_MAX_PACKETS).min(MAX_QUEUE);
    let mut st = lock();
    let process_ticks = now_ticks();
    let mut events = Vec::new();
    let running = match st.model.stream_process_begin(
        stream,
        serial,
        process_ticks,
        snap.u32_at(STREAM_MAX_PACKETS),
        &mut events,
    ) {
        Ok(running) => running,
        Err(Error::NoFreeState) => {
            drop(st);
            refuse(STREAM_PROCESS, "no free passive stream state");
        }
        Err(Error::ClockOverflow) => overflow(),
    };
    deliver(&mut st, &events);
    let full = running.is_none();
    let (frequency, dowork) = (st.frequency(), st.dowork());
    st.capture
        .begin(stream, serial, process_ticks, max, full, frequency, dowork);
    if full {
        st.obs(format!(
            "CSP {stream} {serial} {process_ticks} 0 0 0 0 {max} 0 1"
        ));
        return PACKET_LIST_FULL;
    }
    let stream_running = running == Some(true);
    drop(st);
    if runtime_active() {
        // SAFETY: plain C runtime call.
        unsafe { dsound_audio_runtime_set_stream_running(stream, process_ticks, stream_running) };
    }
    let pid = host_pid();
    let mut words = [0u32; 6];
    // SAFETY: `words` has the six writable words the helper fills.
    if !unsafe { tsfp_audio_stream_packet_decode_rust(pid, packet_address, words.as_mut_ptr()) } {
        refuse(STREAM_PROCESS, "Rust stream packet descriptor read refused");
    }
    // words: buffer, size, completed*, status*, completion event, timestamp*.
    let mut payload_readable = true;
    if words[1] != 0 && words[0] != 0 {
        // SAFETY: plain guest probe of the Rust host.
        payload_readable =
            unsafe { tsfp_audio_stream_packet_buffer_probe_rust(pid, words[0], words[1]) };
    }
    if words[1] == 0 || words[0] == 0 || !payload_readable {
        refuse(STREAM_PROCESS, "packet buffer is null, empty or unreadable");
    }
    if words[4] != 0 {
        refuse(STREAM_PROCESS, "a packet completion event is not modelled");
    }
    if words[5] != 0 {
        refuse(STREAM_PROCESS, "a packet timestamp is not modelled");
    }
    let format = stream_format_of(&snap);
    let average = if snap.u32_at(STREAM_FORMAT_SETS) != 0 {
        le32(&snap.bytes_at::<20>(STREAM_FORMAT), 8)
    } else {
        snap.u32_at(STREAM_AVERAGE_BYTES_PER_SECOND)
    };
    // SAFETY: plain guest publication helper of the Rust host.
    if !unsafe { tsfp_audio_stream_pending_publish_rust(pid, stream, words[2], words[3]) } {
        refuse(
            STREAM_PROCESS,
            "Rust stream packet completion publication refused its guest words",
        );
    }
    let audio_enabled = runtime_active();
    let (mut audio_ready, mut audio_rate, mut audio_mono) = (false, 0u32, false);
    if audio_enabled {
        let (mut kind, mut rate) = (0u32, 0u32);
        // SAFETY: `format` is 20 readable bytes; the outputs are live locals.
        if !unsafe {
            tsfp_audio_stream_packet_classify_rust(
                format.as_ptr(),
                20,
                words[1],
                &mut kind,
                &mut rate,
            )
        } {
            refuse(
                STREAM_PROCESS,
                "Rust stream packet format admission refused its snapshot",
            );
        }
        audio_rate = rate;
        audio_mono = kind == 2;
        audio_ready = kind != 0;
        if !audio_ready {
            let mut st = lock();
            let announce = !st.announced_audio_skip;
            st.announced_audio_skip = true;
            drop(st);
            if announce {
                log(
                    "dsound: opt-in PCM mixer skipped an unsupported stream packet; guest packet completion remains modelled without PCM\n",
                );
            }
        }
    }
    let mut st = lock();
    if st
        .model
        .stream_queued(stream, serial)
        .is_some_and(|queued| queued >= max)
    {
        drop(st);
        refuse(STREAM_PROCESS, "stream state changed during Process");
    }
    if audio_ready {
        // SAFETY: plain C runtime submission of a validated guest packet.
        let accepted = unsafe {
            dsound_audio_runtime_submit_guest(
                stream,
                process_ticks,
                audio_rate,
                pid,
                words[0],
                words[1] as usize,
                audio_mono,
            )
        };
        if !accepted {
            drop(st);
            refuse(
                STREAM_PROCESS,
                "opt-in mixer rejected measured stereo or mono ADPCM packet or reached its bounded queue limit",
            );
        }
    }
    if !st.model.stream_process_push(
        stream,
        serial,
        process_ticks,
        words[1],
        words[2],
        words[3],
        average,
        le32(&format, 4),
    ) {
        drop(st);
        refuse(STREAM_PROCESS, "stream state changed during Process");
    }
    st.obs(format!(
        "CSP {stream} {serial} {process_ticks} {} {} {} {average} {max} {} 0",
        words[1],
        words[2],
        words[3],
        le32(&format, 4)
    ));
    st.census_note(&format, words[1], audio_ready);
    let announce = !st.announced_stream;
    st.announced_stream = true;
    drop(st);
    if announce {
        log(announce_text(audio_enabled, true));
    }
    0
}

fn ordinary_audio_volume(volume: i32) -> Option<i32> {
    if volume > 0 || volume < -10000 {
        return None;
    }
    // Original ordinary mono settings subtract their +0x20 default 600 millibels; the floor is silent.
    Some((i64::from(volume) - 600).max(-10000) as i32)
}

fn ordinary_or_spatial(snap: &Snapshot<BUFFER_SNAPSHOT_BYTES>) -> bool {
    let flags = snap.u32_at(BUFFER_FLAGS);
    flags == 0 || (flags == 0x10 && snap.u32_at(BUFFER_CACHE_MASK) == 7)
}

fn whole_start_recorded(snap: &Snapshot<BUFFER_SNAPSHOT_BYTES>) -> bool {
    snap.u32_at(BUFFER_DATA_SETS) != 0
        && snap.u32_at(BUFFER_VOLUME_SETS) != 0
        && snap.u32_at(BUFFER_PAUSE_SETS) != 0
        && snap.u32_at(BUFFER_FREQUENCY_SETS) != 0
        && ordinary_or_spatial(snap)
}

/// `dsound_completion_buffer_pause`.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_completion_buffer_pause(buffer: u32, serial: u64, mode: u32) -> bool {
    let mut st = lock();
    let tick = now_ticks();
    let clock_ok = runtime_clock_matches(&st);
    let frequency_ok = st.model.has_buffer(buffer, serial) && mode <= 2;
    if !frequency_ok {
        return false;
    }
    let active = runtime_active();
    let admitted = st
        .model
        .buffer_pause(buffer, serial, tick, mode, &mut |paused| {
            // SAFETY: plain C runtime verdict.
            !active
                || (clock_ok
                    && unsafe { dsound_audio_runtime_pause_buffer(buffer, serial, tick, paused) })
        });
    if admitted {
        st.obs(format!("CBA {buffer} {serial} {tick} {mode} 1"));
    }
    admitted
}

/// `dsound_completion_buffer_frequency`.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_completion_buffer_frequency(
    buffer: u32,
    serial: u64,
    old_hertz: u32,
    new_hertz: u32,
) -> bool {
    let mut st = lock();
    let tick = now_ticks();
    let clock_ok = runtime_clock_matches(&st);
    let active = runtime_active();
    let mut observed = false;
    let mut pending_obs = None;
    let admitted = st.model.buffer_frequency_with(
        buffer,
        serial,
        tick,
        old_hertz,
        new_hertz,
        &mut || {
            observed = true;
            pending_obs = Some(format!(
                "CBF {buffer} {serial} {tick} {old_hertz} {new_hertz}"
            ));
        },
        // SAFETY: plain C runtime verdict.
        &mut || {
            !active
                || (clock_ok
                    && unsafe {
                        dsound_audio_runtime_frequency_buffer(buffer, serial, tick, new_hertz)
                    })
        },
    );
    if let Some(line) = pending_obs {
        st.obs(line);
    }
    admitted
}

/// `dsound_completion_buffer_loop`.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_completion_buffer_loop(
    buffer: u32,
    serial: u64,
    loop_start: u32,
    loop_length: u32,
    frequency: u32,
) -> bool {
    let mut st = lock();
    if frequency == 0
        || loop_length == 0
        || loop_start % BUFFER_BLOCK != 0
        || loop_length % BUFFER_BLOCK != 0
        || !st.model.has_buffer(buffer, serial)
    {
        return false;
    }
    let start_samples = u64::from(loop_start / BUFFER_BLOCK) * BUFFER_BLOCK_SAMPLES;
    let end_samples = (u64::from(loop_start) + u64::from(loop_length)) / u64::from(BUFFER_BLOCK)
        * BUFFER_BLOCK_SAMPLES;
    let tick = now_ticks();
    let clock_ok = runtime_clock_matches(&st);
    let active = runtime_active();
    let accepted = st.model.buffer_loop(
        buffer,
        serial,
        tick,
        start_samples,
        end_samples,
        frequency,
        // SAFETY: plain C runtime verdict.
        &mut || {
            !active
                || (clock_ok
                    && unsafe {
                        dsound_audio_runtime_loop_buffer(
                            buffer,
                            serial,
                            tick,
                            start_samples as usize,
                            end_samples as usize,
                        )
                    })
        },
    );
    if accepted {
        st.obs(format!(
            "CBL {buffer} {serial} {tick} {start_samples} {end_samples} {frequency}"
        ));
    }
    accepted
}

/// `dsound_completion_buffer_volume`.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_completion_buffer_volume(buffer: u32, serial: u64, volume: i32) -> bool {
    let mut st = lock();
    if !st.enabled {
        return true;
    }
    let tick = now_ticks();
    let Some((playing, _)) = st.model.buffer_observe(buffer, serial, tick) else {
        return true;
    };
    if playing && runtime_active() {
        let clock_ok = runtime_clock_matches(&st);
        // SAFETY: plain C runtime verdict.
        return clock_ok
            && ordinary_audio_volume(volume).is_some_and(|normalized| unsafe {
                dsound_audio_runtime_volume_buffer(buffer, serial, tick, normalized)
            });
    }
    true
}

/// `dsound_completion_buffer_started`.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_completion_buffer_started(buffer: u32, serial: u64) -> bool {
    lock().model.has_buffer(buffer, serial)
}

/// `dsound_completion_buffer_voice_running`.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_completion_buffer_voice_running(buffer: u32, serial: u64) -> bool {
    let mut st = lock();
    let tick = now_ticks();
    st.model
        .buffer_observe(buffer, serial, tick)
        .is_some_and(|(playing, stopped)| playing && !stopped)
}

/// `dsound_completion_buffer_play`.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_completion_buffer_play(buffer: u32, flags: u32) -> u32 {
    let snap = match (dsound_completion_enabled(), buffer_snapshot(buffer)) {
        (true, Some(snap)) => snap,
        _ => refuse(BUFFER_PLAY, "buffer is not owned by the passive adapter"),
    };
    if !whole_start_recorded(&snap) {
        refuse(
            BUFFER_PLAY,
            &format!(
                "only an ordinary buffer with its whole start (data, volume, Pause(0), frequency) recorded can Play (data {} volume {} pause {} frequency {} flags {:#x} block {})",
                snap.u32_at(BUFFER_DATA_SETS),
                snap.u32_at(BUFFER_VOLUME_SETS),
                snap.u32_at(BUFFER_PAUSE_SETS),
                snap.u32_at(BUFFER_FREQUENCY_SETS),
                snap.u32_at(BUFFER_FLAGS),
                snap.u16_at(BUFFER_BLOCK_ALIGN),
            ),
        );
    }
    if flags > 3 {
        refuse(
            BUFFER_PLAY,
            &format!("only Play flags 0 to 3 are measured, got {flags:#x}"),
        );
    }
    let (frequency, data_length, data_address) = (
        snap.u32_at(BUFFER_FREQUENCY),
        snap.u32_at(BUFFER_DATA_LENGTH),
        snap.u32_at(BUFFER_DATA_ADDRESS),
    );
    if u32::from(snap.u16_at(BUFFER_BLOCK_ALIGN)) != BUFFER_BLOCK
        || frequency == 0
        || data_length < BUFFER_BLOCK
    {
        refuse(
            BUFFER_PLAY,
            "only the measured 36 byte ADPCM block buffer with a frequency can Play",
        );
    }
    let audio_enabled = runtime_active();
    let mut normalized_volume = 0i32;
    if audio_enabled {
        match ordinary_audio_volume(snap.i32_at(BUFFER_VOLUME)) {
            Some(v) if data_length <= AUDIO_BUFFER_MAX_BYTES => normalized_volume = v,
            _ => refuse(
                BUFFER_PLAY,
                "real ordinary buffer PCM needs bounded data and volume -10000..0",
            ),
        }
    }
    let samples = u64::from(data_length / BUFFER_BLOCK) * BUFFER_BLOCK_SAMPLES;
    let (loop_start, loop_length, loop_sets) = (
        snap.u32_at(BUFFER_LOOP_START),
        snap.u32_at(BUFFER_LOOP_LENGTH),
        snap.u32_at(BUFFER_LOOP_SETS),
    );
    let looping = flags & 1 != 0;
    if looping
        && loop_sets != 0
        && (loop_length == 0
            || loop_start % BUFFER_BLOCK != 0
            || loop_length % BUFFER_BLOCK != 0
            || u64::from(loop_start) + u64::from(loop_length) > u64::from(data_length))
    {
        refuse(
            BUFFER_PLAY,
            "loop region must be nonempty, block-aligned and within buffer data",
        );
    }
    let loop_start_samples = u64::from(loop_start / BUFFER_BLOCK) * BUFFER_BLOCK_SAMPLES;
    let loop_end_samples =
        u64::from(loop_start.wrapping_add(loop_length) / BUFFER_BLOCK) * BUFFER_BLOCK_SAMPLES;
    let serial = snap.u64_at(BUFFER_LEASE_SERIAL);
    let mut st = lock();
    if looping
        && st.ticks_for(loop_end_samples, u64::from(frequency))
            <= st.ticks_for(loop_start_samples, u64::from(frequency))
    {
        drop(st);
        refuse(
            BUFFER_PLAY,
            "loop interval is below the configured virtual clock resolution",
        );
    }
    if !st.model.buffer_slot_available(buffer) {
        drop(st);
        refuse(BUFFER_PLAY, "no free passive buffer state");
    }
    let tick = now_ticks();
    let paused = st.model.buffer_is_paused(buffer, serial);
    let mut pcm_accepted = false;
    if audio_enabled && st.frequency() != 0 && runtime_clock_matches(&st) {
        // SAFETY: plain C runtime call; the runtime reads the guest data through the Rust host.
        pcm_accepted = unsafe {
            dsound_audio_runtime_play_buffer_guest(
                buffer,
                serial,
                tick,
                frequency,
                host_pid(),
                data_address,
                data_length as usize,
                paused,
                looping,
                loop_start_samples as usize,
                loop_end_samples as usize,
                normalized_volume,
            )
        };
    }
    if audio_enabled && !pcm_accepted {
        drop(st);
        refuse(
            BUFFER_PLAY,
            "real mono buffer decoder/mixer refused clock, data, identity, timestamp or bounded storage",
        );
    }
    if !st.model.buffer_play(
        buffer,
        serial,
        tick,
        looping,
        samples,
        frequency,
        loop_start_samples,
        loop_end_samples,
    ) {
        drop(st);
        refuse(BUFFER_PLAY, "no free passive buffer state");
    }
    let duration = st.ticks_for(samples, u64::from(frequency));
    st.obs(format!(
        "CBP {buffer} {serial} {tick} {} {samples} {duration} {frequency} {loop_start_samples} {loop_end_samples} {frequency}",
        i32::from(looping)
    ));
    let announce = !st.announced_buffer;
    st.announced_buffer = true;
    drop(st);
    if announce {
        log(announce_text(audio_enabled, false));
    }
    0
}

/// `dsound_completion_buffer_stop`.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_completion_buffer_stop(buffer: u32) -> u32 {
    let snap = match (dsound_completion_enabled(), buffer_snapshot(buffer)) {
        (true, Some(snap)) => snap,
        _ => refuse(BUFFER_STOP, "buffer is not owned by the passive adapter"),
    };
    if !whole_start_recorded(&snap) {
        refuse(
            BUFFER_STOP,
            "only an ordinary buffer with its whole start (data, volume, Pause(0), frequency) recorded can Stop",
        );
    }
    let serial = snap.u64_at(BUFFER_LEASE_SERIAL);
    let mut st = lock();
    let tick = now_ticks();
    let found = st.model.has_buffer(buffer, serial);
    let clock_ok = runtime_clock_matches(&st);
    let active = runtime_active();
    let stopped = st.model.buffer_stop(buffer, serial, tick, &mut || {
        // SAFETY: plain C runtime verdict.
        !active || (clock_ok && unsafe { dsound_audio_runtime_stop_buffer(buffer, serial, tick) })
    });
    if !stopped {
        drop(st);
        // SAFETY: the runtime returns a static NUL-terminated string.
        let reason =
            unsafe { std::ffi::CStr::from_ptr(dsound_audio_runtime_last_buffer_refusal()) }
                .to_string_lossy()
                .into_owned();
        refuse(
            BUFFER_STOP,
            &format!("real buffer PCM rejected Stop: {reason}"),
        );
    }
    if found {
        st.obs(format!("CBS {buffer} {serial} {tick}"));
    }
    let announce = !st.announced_stop;
    st.announced_stop = true;
    drop(st);
    if announce {
        log(
            "dsound: explicit PASSIVE COMPLETION buffer Stop; a playing buffer loses its pause and drains from its virtual play position (position and hardware drain INFERRED), one that finished or never played is left alone; active PCM voices follow the same finite tail\n",
        );
    }
    0
}

/// `dsound_completion_buffer_status`.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_completion_buffer_status(buffer: u32, output: u32) -> u32 {
    let snap = match (dsound_completion_enabled(), buffer_snapshot(buffer)) {
        (true, Some(snap)) => snap,
        _ => refuse(
            BUFFER_GET_STATUS,
            "buffer is not owned by the passive adapter",
        ),
    };
    let writable = output != 0
        && u64::from(output) + 4 <= 0x1_0000_0000
        && read_u32(output).is_some_and(|old| write_u32(output, old));
    if !writable {
        refuse(BUFFER_GET_STATUS, "status output is null or not writable");
    }
    let serial = snap.u64_at(BUFFER_LEASE_SERIAL);
    let mut st = lock();
    let found = st.model.has_buffer(buffer, serial);
    let t = now_ticks();
    let value = st.model.buffer_status(buffer, serial, t);
    if found {
        st.obs(format!("CBG {buffer} {serial} {t} {value}"));
    } else {
        let t = now_ticks();
        st.obs(format!("CBG {buffer} {serial} {t} 0"));
    }
    drop(st);
    if !write_u32(output, value) {
        refuse(BUFFER_GET_STATUS, "status output is not writable");
    }
    0
}

/// `dsound_completion_buffer_get_position`.
///
/// # Safety
/// `position` is null or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dsound_completion_buffer_get_position(
    buffer: u32,
    position: *mut CBufferPosition,
) -> bool {
    if position.is_null() {
        return false;
    }
    let Some(snap) = buffer_snapshot(buffer) else {
        return false;
    };
    let (serial, frequency) = (
        snap.u64_at(BUFFER_LEASE_SERIAL),
        snap.u32_at(BUFFER_FREQUENCY),
    );
    let mut st = lock();
    if !st.enabled || st.frequency() == 0 {
        return false;
    }
    let tick = now_ticks();
    let Some((samples, total, playing, looping, paused)) =
        st.model.buffer_position(buffer, serial, tick, frequency)
    else {
        return false;
    };
    // SAFETY: non-null writable output by contract.
    unsafe {
        *position = CBufferPosition {
            position_samples: samples,
            total_samples: total,
            frequency,
            playing,
            looping,
            paused,
        };
    }
    true
}

/// `dsound_completion_work`.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_completion_work() {
    let mut st = lock();
    if !st.enabled {
        return;
    }
    let now = now_ticks();
    match st.model.work(now) {
        Ok(events) => deliver(&mut st, &events),
        Err(_) => overflow(),
    }
    st.obs(format!("CW {now}"));
}

/// `dsound_completion_method_owned`.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_completion_method_owned(stack_pointer: u32) -> bool {
    dsound_completion_enabled()
        && stack_pointer <= u32::MAX - 8
        && read_u32(stack_pointer + 4).is_some_and(|stream| stream_snapshot(stream).is_some())
}

/// `dsound_completion_route_method`.
///
/// # Safety
/// `result` and `pop_bytes` are null or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dsound_completion_route_method(
    entry: u32,
    stack_pointer: u32,
    result: *mut u32,
    pop_bytes: *mut u32,
) -> bool {
    let count = match entry {
        STREAM_GET_STATUS | STREAM_GET_INFO => 2usize,
        STREAM_PROCESS => 3,
        _ => return false,
    };
    if result.is_null()
        || pop_bytes.is_null()
        || !dsound_completion_enabled()
        || stack_pointer > u32::MAX - 16
    {
        return false;
    }
    let mut args = [0u32; 3];
    for (i, slot) in args.iter_mut().enumerate().take(count) {
        match read_u32(stack_pointer + 4 + 4 * i as u32) {
            Some(value) => *slot = value,
            None => refuse(entry, "unreadable argument"),
        }
    }
    let value = match entry {
        STREAM_GET_STATUS => dsound_completion_stream_status(args[0], args[1]),
        STREAM_GET_INFO => dsound_completion_stream_info(args[0], args[1]),
        _ => dsound_completion_stream_process(args[0], args[1], args[2]),
    };
    // SAFETY: non-null writable outputs by contract.
    unsafe {
        *result = value;
        *pop_bytes = 4 * count as u32;
    }
    true
}

fn frame_handler(frame: *mut c_void, entry: u32) -> u32 {
    // T733: no caller gate. The originals never read their return address, so a call site is not a property of the
    // method. The state checks of the model decide, and the handlers exist only with the model on.
    if frame.is_null() {
        refuse(entry, "unreadable call frame");
    }
    let count = match entry {
        BUFFER_PLAY => 4,
        BUFFER_STOP => 1,
        _ => 2,
    };
    let mut args = [0u32; 4];
    for (i, slot) in args.iter_mut().enumerate().take(count) {
        // SAFETY: the registry passes the live call frame; the slot is a live local.
        if !unsafe { kernel_frame_arg(frame, i as u32, slot) } {
            refuse(entry, "unreadable argument");
        }
    }
    match entry {
        BUFFER_PLAY => {
            if args[1] != 0 || args[2] != 0 {
                refuse(
                    entry,
                    &format!(
                        "Play reserved arguments must be zero, got ({:#x}, {:#x}, {:#x}, {:#x})",
                        args[0], args[1], args[2], args[3]
                    ),
                );
            }
            dsound_completion_buffer_play(args[0], args[3])
        }
        BUFFER_STOP => dsound_completion_buffer_stop(args[0]),
        _ => dsound_completion_buffer_status(args[0], args[1]),
    }
}

unsafe extern "C" fn play_handler(frame: *mut c_void) -> u32 {
    frame_handler(frame, BUFFER_PLAY)
}
unsafe extern "C" fn status_handler(frame: *mut c_void) -> u32 {
    frame_handler(frame, BUFFER_GET_STATUS)
}
unsafe extern "C" fn stop_handler(frame: *mut c_void) -> u32 {
    frame_handler(frame, BUFFER_STOP)
}

/// `dsound_completion_register`: Play, Stop and buffer GetStatus, only while the policy is enabled.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_completion_register() -> usize {
    if !dsound_completion_enabled() {
        return 0;
    }
    let entries: [(u32, DsoundHandler); 3] = [
        (BUFFER_PLAY, play_handler),
        (BUFFER_GET_STATUS, status_handler),
        (BUFFER_STOP, stop_handler),
    ];
    entries
        .into_iter()
        .filter(|&(address, handler)| {
            tsfp_audio_hle_register_handler_rust(address, handler)
                // SAFETY: the C compatibility table is initialised before subsystem handlers register.
                && unsafe { dsound_hle_register_rust(address) }
        })
        .count()
}

/// `dsound_completion_mixbin_headroom`.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_completion_mixbin_headroom(
    identity: u64,
    bin: u32,
    headroom: u32,
) -> bool {
    let st = lock();
    st.enabled
        && runtime_clock_matches(&st)
        // SAFETY: plain C runtime call.
        && unsafe { dsound_audio_runtime_mixbin_headroom(identity, now_ticks(), bin, headroom) }
}

/// `dsound_completion_bind_mixbin_headroom`.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_completion_bind_mixbin_headroom(identity: u64) -> bool {
    let st = lock();
    // SAFETY: plain C runtime call.
    st.enabled
        && runtime_clock_matches(&st)
        && unsafe { dsound_audio_runtime_bind_mixbin_headroom(identity) }
}
