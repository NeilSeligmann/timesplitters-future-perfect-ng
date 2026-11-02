// SPDX-License-Identifier: GPL-3.0-or-later
//! Module state of the movie stream host glue: the stream nodes under the module lock, the host configuration and the
//! small helpers every handler shares. The lock is a raw one because the device-lease transactions take it in
//! `prepare` and release it in `abort` or `finalize`, across separate calls.
use crate::ffi::*;
use std::cell::UnsafeCell;
use std::ffi::{CString, c_char};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Condvar, Mutex};

pub const CREATE: u32 = 0x0040_967C;
pub const ADD_REF: u32 = 0x0040_723F;
pub const RELEASE: u32 = 0x0040_7286;
pub const GET_STATUS: u32 = 0x0040_73D3;
pub const PROCESS: u32 = 0x0040_7424;
pub const DISCONTINUITY: u32 = 0x0040_733B;
pub const PAUSE: u32 = 0x0040_7B23;
pub const FLUSH_EX: u32 = 0x0040_7B28;
pub const FLUSH: u32 = 0x0040_7388;
pub const VOLUME: u32 = 0x0040_7B14;
pub const WORK: u32 = 0x0040_7B40;
/// `IDirectSound::SynchPlayback`, called once from the XMV GetNextFrame (call at 0x445716).
pub const SYNCH: u32 = 0x0040_7A4C;
pub const SYNCH_RETURN: u32 = 0x0044_571B;
pub const SINGLETON: u32 = 0x0041_2B30;
pub const GLOBAL_STATE: u32 = 0x0041_24A8;
pub const VTABLE: u32 = 0x004A_1D00;
pub const SECONDARY: u32 = 0x004A_1CF0;
pub const OOM: u32 = 0x8007_000E;
pub const E_FAIL_STATUS: u32 = 0x8000_4005;
pub const STREAM_BYTES: u32 = 40;
pub const MAX_PACKETS: u32 = 2;
pub const MOVIE_CALLBACK: u32 = 0x0044_5071;
pub const TOO_MANY_PACKETS: u32 = 0x8878_0032;
pub const GUEST_HEAP_CHUNK_MIN: u32 = 0x10000;
pub const CLOCK_FREQUENCY_HZ: u64 = 733_333_333;

const FORMAT_AVERAGE: u32 = 176_400;
const FORMAT_RATE: u32 = 44_100;
const FORMAT_BLOCK: u16 = 4;

/// One live movie stream: the private guest allocation, its sidecar header copy and the request-side observations.
pub struct Node {
    pub stream_address: u32,
    pub stream_heap: u32,
    pub publication_address: u32,
    pub header: [u32; 10],
    pub callback: u32,
    pub context: u32,
    pub pause_mode: u32,
    pub queued: u32,
    pub volume: i32,
    pub volume_seen: bool,
    pub discontinuity_seen: bool,
    pub flush_pending: bool,
    pub paused: bool,
    pub flush_tick: u64,
    pub lease: Lease,
}

impl Node {
    pub fn new(publication_address: u32, callback: u32, context: u32) -> Self {
        Self {
            stream_address: 0,
            stream_heap: 0,
            publication_address,
            header: [0; 10],
            callback,
            context,
            pause_mode: 0,
            queued: 0,
            volume: 0,
            volume_seen: false,
            discontinuity_seen: false,
            flush_pending: false,
            paused: false,
            flush_tick: 0,
            lease: Lease::default(),
        }
    }
}

#[derive(Default)]
pub struct State {
    pub nodes: Vec<Node>,
    /// Failed allocations that outlived the scheduler entry: protected until reset.
    pub rollback: Vec<Node>,
    pub volume_announced: bool,
}

struct RawLock {
    held: Mutex<bool>,
    free: Condvar,
}

impl RawLock {
    const fn new() -> Self {
        Self {
            held: Mutex::new(false),
            free: Condvar::new(),
        }
    }

    fn lock(&self) {
        let mut held = self.held.lock().unwrap_or_else(|e| e.into_inner());
        while *held {
            held = self.free.wait(held).unwrap_or_else(|e| e.into_inner());
        }
        *held = true;
    }

    fn unlock(&self) {
        *self.held.lock().unwrap_or_else(|e| e.into_inner()) = false;
        self.free.notify_one();
    }
}

struct Guarded(UnsafeCell<Option<State>>);
// SAFETY: every access goes through `state()`, whose contract is that the module lock is held.
unsafe impl Sync for Guarded {}

static LOCK: RawLock = RawLock::new();
static STATE: Guarded = Guarded(UnsafeCell::new(None));
pub static RESET_LOCK: Mutex<()> = Mutex::new(());

pub fn lock() {
    LOCK.lock();
}

pub fn unlock() {
    LOCK.unlock();
}

/// The module state.
///
/// # Safety
/// The module lock must be held, and the returned reference must not outlive it.
pub unsafe fn state() -> &'static mut State {
    // SAFETY: exclusive by the module lock.
    unsafe { (*STATE.0.get()).get_or_insert_with(State::default) }
}

/// Run `action` with the module lock held.
pub fn locked<T>(action: impl FnOnce(&mut State) -> T) -> T {
    lock();
    // SAFETY: the lock is held for the whole call.
    let result = action(unsafe { state() });
    unlock();
    result
}

static IRQL_PROVIDER: AtomicUsize = AtomicUsize::new(0);
static FATAL_HANDLER: AtomicUsize = AtomicUsize::new(0);
static CLOCK_FN: AtomicUsize = AtomicUsize::new(0);
static CLOCK_FREQUENCY: AtomicU64 = AtomicU64::new(CLOCK_FREQUENCY_HZ);
static TRACE: AtomicBool = AtomicBool::new(false);

fn store_fn<T>(slot: &AtomicUsize, value: Option<T>)
where
    T: Copy,
{
    debug_assert_eq!(size_of::<T>(), size_of::<usize>());
    // SAFETY: `T` is a function pointer, the same size as `usize` (asserted above).
    let bits = value.map_or(0, |f| unsafe { std::mem::transmute_copy::<T, usize>(&f) });
    slot.store(bits, Ordering::Release);
}

fn load_fn<T: Copy>(slot: &AtomicUsize) -> Option<T> {
    let bits = slot.load(Ordering::Acquire);
    // SAFETY: only ever stored from a valid `T` function pointer by `store_fn`.
    (bits != 0).then(|| unsafe { std::mem::transmute_copy::<usize, T>(&bits) })
}

pub fn set_irql_provider(provider: Option<IrqlProvider>) {
    store_fn(&IRQL_PROVIDER, provider);
}

pub fn set_fatal(handler: Option<FatalFn>) {
    store_fn(&FATAL_HANDLER, handler);
}

/// Returns false when the Rust scheduler refused the frequency (the C host aborts then).
pub fn set_clock(clock: Option<ClockFn>, frequency: u64) -> bool {
    store_fn(&CLOCK_FN, clock);
    let frequency = if clock.is_some() {
        frequency
    } else {
        CLOCK_FREQUENCY_HZ
    };
    CLOCK_FREQUENCY.store(frequency, Ordering::Release);
    // SAFETY: scalar configure call of the Rust scheduler.
    unsafe { tsfp_audio_movie_schedule_configure_rust(frequency) }
}

pub fn clock_frequency() -> u64 {
    CLOCK_FREQUENCY.load(Ordering::Acquire)
}

pub fn set_trace(on: bool) {
    TRACE.store(on, Ordering::Release);
}

pub fn now_ticks() -> u64 {
    match load_fn::<ClockFn>(&CLOCK_FN) {
        // SAFETY: the host-installed clock takes no arguments.
        Some(clock) => unsafe { clock() },
        // SAFETY: plain host query.
        None => unsafe { kernel_clock_peek() },
    }
}

pub extern "C" fn now_ticks_c() -> u64 {
    now_ticks()
}

pub extern "C" fn host_pid_c() -> i32 {
    host_pid()
}

pub fn host_pid() -> i32 {
    // SAFETY: plain host query.
    unsafe { kernel_host_pid() }
}

/// `--trace-xmv`: one stderr line per model event, with the virtual tick it happened at.
pub fn trace(line: impl FnOnce() -> String) {
    if TRACE.load(Ordering::Acquire) {
        eprintln!("movie-stream: {}", line());
    }
}

pub fn log_line(line: &str) {
    if let Ok(text) = CString::new(line) {
        // SAFETY: NUL-terminated finished line.
        unsafe { dsound_hle_log_line(text.as_ptr()) };
    }
}

/// `%#x`: zero prints bare, like C.
pub fn hex(value: u32) -> String {
    if value == 0 {
        "0".to_string()
    } else {
        format!("{value:#x}")
    }
}

/// Fatal refusal: the fatal handler if installed, then abort (never returns).
pub fn refuse(entry: u32, reason: &str) -> ! {
    log_line(&format!(
        "dsound movie stream {} refused: {}\n",
        hex(entry),
        reason
    ));
    if let (Some(fatal), Ok(text)) = (load_fn::<FatalFn>(&FATAL_HANDLER), CString::new(reason)) {
        // SAFETY: the host installed the handler for exactly this call.
        unsafe { fatal(entry, text.as_ptr()) };
    }
    std::process::abort()
}

/// The `refuse` callback of the Rust drain (a C string reason).
pub unsafe extern "C" fn refuse_c(entry: u32, reason: *const c_char) -> ! {
    let text = if reason.is_null() {
        String::new()
    } else {
        // SAFETY: the drain passes a static NUL-terminated reason.
        unsafe { std::ffi::CStr::from_ptr(reason) }
            .to_string_lossy()
            .into_owned()
    };
    refuse(entry, &text)
}

pub fn overlap(a: u32, an: u32, b: u32, bn: u32) -> bool {
    u64::from(a) < u64::from(b) + u64::from(bn) && u64::from(b) < u64::from(a) + u64::from(an)
}

unsafe extern "C" fn current_irql(out: *mut u8) -> bool {
    match load_fn::<IrqlProvider>(&IRQL_PROVIDER) {
        // SAFETY: forwarded writable byte.
        Some(provider) => unsafe { provider(out) },
        None => false,
    }
}

unsafe extern "C" fn read_guest_u32(address: u32, out: *mut u32) -> bool {
    // SAFETY: forwarded writable word.
    address == GLOBAL_STATE && unsafe { kernel_guest_read_u32(address, out) }
}

/// The policy refusal text, or `None` when movie audio may run: policy on, known IRQL0, original global audio state zero.
pub fn policy_error() -> Option<&'static str> {
    // SAFETY: both callbacks are valid for this synchronous call.
    match unsafe { tsfp_audio_movie_policy_rust(Some(current_irql), Some(read_guest_u32)) } {
        0 => None,
        1 => Some("explicit headless-movie-audio policy is disabled"),
        2 => Some("only known IRQL0 is supported"),
        3 => Some("original global audio state must be zero"),
        _ => Some("Rust movie policy returned an unknown refusal"),
    }
}

pub fn count_operation() {
    locked(|_| {
        // SAFETY: scalar counter of the Rust diagnostics.
        unsafe { tsfp_audio_movie_diagnostics_count_operation_rust() }
    });
}

pub fn read_u32(address: u32) -> Option<u32> {
    let mut value = 0;
    // SAFETY: writable local.
    unsafe { kernel_guest_read_u32(address, &mut value) }.then_some(value)
}

pub fn read_bytes<const N: usize>(address: u32) -> Option<[u8; N]> {
    let mut bytes = [0u8; N];
    // SAFETY: `bytes` is writable for N bytes.
    unsafe { kernel_guest_read_bytes(address, bytes.as_mut_ptr().cast(), N) }.then_some(bytes)
}

pub fn heap_active() -> bool {
    // SAFETY: scalar selector.
    unsafe { tsfp_kmem_audio_heap_active() }
}

pub fn heap_create() -> u32 {
    // SAFETY: scalar allocation call.
    unsafe {
        if heap_active() {
            tsfp_kmem_audio_heap_create(0, GUEST_HEAP_CHUNK_MIN, 0)
        } else {
            kmem_heap_create(0, GUEST_HEAP_CHUNK_MIN, 0)
        }
    }
}

pub fn heap_alloc(heap: u32, bytes: u32) -> u32 {
    // SAFETY: scalar allocation call.
    unsafe {
        if heap_active() {
            tsfp_kmem_audio_heap_alloc(heap, bytes)
        } else {
            kmem_heap_alloc(heap, bytes)
        }
    }
}

pub fn heap_valid(heap: u32) -> bool {
    // SAFETY: scalar query.
    unsafe {
        if heap_active() {
            tsfp_kmem_audio_heap_valid(heap)
        } else {
            kmem_heap_valid(heap)
        }
    }
}

pub fn heap_block_size(heap: u32, address: u32) -> Option<u32> {
    let mut size = 0;
    // SAFETY: writable local.
    let found = unsafe {
        if heap_active() {
            tsfp_kmem_audio_heap_block_size(heap, address, &mut size)
        } else {
            kmem_heap_block_size(heap, address, &mut size)
        }
    };
    found.then_some(size)
}

pub fn heap_destroy(heap: u32) -> bool {
    // SAFETY: scalar destroy call.
    unsafe {
        if heap_active() {
            tsfp_kmem_audio_heap_destroy(heap)
        } else {
            kmem_heap_destroy(heap)
        }
    }
}

fn word16(bytes: &[u8]) -> u32 {
    u32::from(u16::from_le_bytes([bytes[0], bytes[1]]))
}

fn word32(bytes: &[u8]) -> u32 {
    u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

/// The exact DSSTREAMDESC and WAVEFORMATEX the retained EnableAudioStream builds: flags 0, two packets, callback 0x445071,
/// a nonzero context (the decoder), no mixbins, PCM stereo 44100 Hz 16-bit, block 4, average 176400, cbSize 0.
#[derive(Clone, Copy, Default)]
pub struct Scope {
    pub descriptor: u32,
    pub format: u32,
    pub callback: u32,
    pub context: u32,
}

pub fn scope_snapshot(descriptor: u32) -> Option<Scope> {
    let desc = read_bytes::<24>(descriptor)?;
    let format = word32(&desc[8..]);
    if format == 0 || u64::from(format) + 18 > 1 << 32 || overlap(descriptor, 24, format, 18) {
        return None;
    }
    let wave = read_bytes::<18>(format)?;
    if word32(&desc) != 0
        || word32(&desc[4..]) != MAX_PACKETS
        || word32(&desc[12..]) != MOVIE_CALLBACK
        || word32(&desc[16..]) == 0
        || word32(&desc[20..]) != 0
    {
        return None;
    }
    if word16(&wave) != 1
        || word16(&wave[2..]) != 2
        || word32(&wave[4..]) != FORMAT_RATE
        || word32(&wave[8..]) != FORMAT_AVERAGE
        || word16(&wave[12..]) != u32::from(FORMAT_BLOCK)
        || word16(&wave[14..]) != 16
        || word16(&wave[16..]) != 0
    {
        return None;
    }
    Some(Scope {
        descriptor,
        format,
        callback: word32(&desc[12..]),
        context: word32(&desc[16..]),
    })
}

pub fn token_equal(a: &Lease, b: &Lease) -> bool {
    a.device_heap == b.device_heap
        && a.internal_address == b.internal_address
        && a.serial == b.serial
}

/// The header is guest memory the title changes only through AddRef/Release here: every word must match the sidecar,
/// which mirrors the reference count in word 2.
pub fn valid_node(node: Option<&Node>, identity: &Lease, internal: u32) -> bool {
    let Some(node) = node else { return false };
    if !token_equal(&node.lease, identity)
        || identity.internal_address != internal
        || !heap_valid(identity.device_heap)
    {
        return false;
    }
    let Some(actual) = read_bytes::<40>(node.stream_address) else {
        return false;
    };
    let mut expected = [0u8; 40];
    for (chunk, word) in expected.chunks_exact_mut(4).zip(node.header) {
        chunk.copy_from_slice(&word.to_le_bytes());
    }
    actual == expected
        && heap_valid(node.stream_heap)
        && heap_block_size(node.stream_heap, node.stream_address) == Some(STREAM_BYTES)
}

impl State {
    pub fn find(&self, address: u32) -> Option<&Node> {
        self.nodes.iter().find(|n| n.stream_address == address)
    }

    pub fn find_mut(&mut self, address: u32) -> Option<&mut Node> {
        self.nodes.iter_mut().find(|n| n.stream_address == address)
    }

    pub fn aliases_state(&self, address: u32, bytes: u32, internal: u32) -> bool {
        overlap(address, bytes, internal, 44)
            || overlap(address, bytes, SINGLETON, 4)
            || overlap(address, bytes, GLOBAL_STATE, 4)
            || overlap(address, bytes, SECONDARY, 60)
            || self.nodes.iter().any(|n| {
                overlap(address, bytes, n.stream_address, STREAM_BYTES)
                    || overlap(address, bytes, n.publication_address, 4)
            })
            || self.aliases_rollback_state(address, bytes)
    }

    /// Orphan heap spans of failed allocations stay protected until reset; ordinary active-stream protection is owned by
    /// the Rust schedule.
    pub fn aliases_rollback_state(&self, address: u32, bytes: u32) -> bool {
        self.rollback.iter().any(|n| {
            n.stream_address != 0 && overlap(address, bytes, n.stream_address, STREAM_BYTES)
        })
    }
}

pub fn queued_count(node: &Node) -> u32 {
    // SAFETY: scalar query of the Rust schedule.
    let count = unsafe { tsfp_audio_movie_schedule_count_rust(node.stream_address) };
    if count != u32::MAX {
        count
    } else {
        node.queued
    }
}
