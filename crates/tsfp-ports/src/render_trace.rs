// SPDX-License-Identifier: GPL-3.0-or-later
//! T2024 slice 2: opt-in trace of the `RenderDevice` and `RenderHost` calls the render lane makes, written in the text stream
//! format of `crates/tsfp-wgpu/src/stream.rs` (`call <Name> <hex args>`, `host <va> <args>`, `mem <addr> <hex>`, `present`).
//! Off by default: `tsfp_render_trace_enabled` (src/host/rust_lane_call.c) is true only when the host runs with the env var
//! `TSFP_RENDER_TRACE=<file>` (optional `TSFP_RENDER_TRACE_FRAMES=<n>` stops after n presents). When off every hook is a
//! single call that returns false and nothing else changes. The lines carry guest memory (vertex runs), so the output is
//! disc-derived and stays under `tmp/`, never committed.
//!
//! T2136d: with `TSFP_RT_HOST_BINARY=1` (and the in-process rt-host sink, no tee file) the same events are written in the binary
//! form of `trace_wire` (records, no text formatting, comments dropped) into a buffer that is handed to the rt-host once per present
//! or when it grows past [`FLUSH_AT`]. The text form stays the default and the format of every dump.
use alloc::{string::String, vec::Vec};
use core::cell::UnsafeCell;
use core::fmt::Write as _;
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

use crate::{
    map::{VERTEX_RING_BASE, VERTEX_RING_INDEX, VERTEX_RING_STRIDE},
    memory::GuestMemory,
    trace_wire as wire,
};

unsafe extern "C" {
    fn tsfp_render_trace_binary_active() -> i32;
    fn tsfp_render_trace_native_push_method() -> i32;
    fn tsfp_render_trace_native_vertex_bindings() -> i32;
    fn tsfp_render_trace_native_state_handler(index: u32) -> i32;
    fn tsfp_render_trace_fast_hooks() -> i32;
    fn tsfp_render_trace_dropping() -> i32;
    fn tsfp_render_trace_texel_budget() -> u32;
    fn tsfp_render_trace_binary_send(bytes: *const u8, length: usize, presents: u32);
    fn tsfp_render_trace_enabled() -> i32;
    fn tsfp_render_trace_line(text: *const u8, length: usize);
    fn tsfp_render_trace_virtual_of_physical(physical: u32, length: u32) -> u32;
    fn tsfp_render_trace_readable(address: u32, length: u32) -> u32;
    fn tsfp_render_trace_now_ns() -> u64;
    fn tsfp_render_trace_stats_enabled() -> i32;
    fn tsfp_render_trace_stderr(text: *const u8, length: usize);
}

// ---- T2136d: guest thread cost accounting (opt-in with TSFP_RT_HOST_STATS=1, otherwise one relaxed load per hook) ----
/// Hook kinds. Kinds below `NESTED` add to the per-frame total, the nested ones only break a parent kind down.
pub const K_CALL: usize = 0;
pub const K_SCISSOR: usize = 1;
pub const K_HOST: usize = 2;
pub const K_DRAW: usize = 3;
pub const K_TEXTURE: usize = 4;
pub const K_SHADER: usize = 5;
pub const K_PUSH: usize = 6;
pub const K_LOCK: usize = 7;
pub const K_C_LIB: usize = 8;
pub const K_C_CONST: usize = 9;
pub const K_C_SURFACE: usize = 10;
pub const K_PRESENT: usize = 11;
const NESTED: usize = 12;
pub const K_N_SLOT_SYNC: usize = 12;
pub const K_N_FORMAT: usize = 13;
pub const K_N_EMIT: usize = 14;
pub const K_N_HASH: usize = 15;
const KINDS: usize = 16;
const KIND_NAMES: [&str; KINDS] = [
    "call",
    "scissor",
    "host",
    "draw",
    "texture",
    "shader",
    "push_end",
    "lock",
    "C lib",
    "C constants",
    "C surface",
    "C present",
    "(slot_sync)",
    "(snapshot format)",
    "(emit to C)",
    "(dedup hash)",
];
const SAMPLE_CAP: usize = 1 << 16;
static STATS_ON: AtomicU32 = AtomicU32::new(0); // 0 unread, 1 on, 2 off
static KIND_NS: [AtomicU64; KINDS] = [const { AtomicU64::new(0) }; KINDS];
static KIND_COUNT: [AtomicU64; KINDS] = [const { AtomicU64::new(0) }; KINDS];
static KIND_BYTES: [AtomicU64; KINDS] = [const { AtomicU64::new(0) }; KINDS];
static FRAME_NS: AtomicU64 = AtomicU64::new(0);
static FRAME_BYTES: AtomicU64 = AtomicU64::new(0);
static FRAME_COUNT: AtomicU64 = AtomicU64::new(0);
static SAMPLE_NS: [AtomicU32; SAMPLE_CAP] = [const { AtomicU32::new(0) }; SAMPLE_CAP];
static SAMPLE_BYTES: [AtomicU32; SAMPLE_CAP] = [const { AtomicU32::new(0) }; SAMPLE_CAP];
/// Kind whose hook is running, so `emit` can attribute the bytes it writes.
static CURRENT_KIND: AtomicU32 = AtomicU32::new(0);

fn stats_on() -> bool {
    match STATS_ON.load(Ordering::Relaxed) {
        1 => true,
        2 => false,
        _ => {
            let on = unsafe { tsfp_render_trace_stats_enabled() } != 0;
            STATS_ON.store(if on { 1 } else { 2 }, Ordering::Relaxed);
            on
        }
    }
}

/// Times a hook from creation to drop.
struct Span {
    kind: usize,
    started: u64,
}

fn span(kind: usize) -> Span {
    if stats_on() {
        if kind < NESTED {
            CURRENT_KIND.store(kind as u32, Ordering::Relaxed);
        }
        Span {
            kind,
            started: unsafe { tsfp_render_trace_now_ns() },
        }
    } else {
        Span { kind, started: 0 }
    }
}

impl Drop for Span {
    fn drop(&mut self) {
        if self.started != 0 {
            let spent = unsafe { tsfp_render_trace_now_ns() }.saturating_sub(self.started);
            stat_add(self.kind, spent, 0);
        }
    }
}

fn stat_add(kind: usize, nanoseconds: u64, bytes: u64) {
    KIND_NS[kind].fetch_add(nanoseconds, Ordering::Relaxed);
    KIND_COUNT[kind].fetch_add(1, Ordering::Relaxed);
    KIND_BYTES[kind].fetch_add(bytes, Ordering::Relaxed);
    if kind < NESTED {
        FRAME_NS.fetch_add(nanoseconds, Ordering::Relaxed);
    }
}

/// C side hooks (lib methods, constants, surface, present) report their cost here.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_render_trace_stat_add(kind: u32, nanoseconds: u64, bytes: u64) {
    if (kind as usize) < KINDS {
        stat_add(kind as usize, nanoseconds, bytes);
        FRAME_BYTES.fetch_add(bytes, Ordering::Relaxed);
    }
}

/// End of a present: store the frame sample (guest nanoseconds spent in trace hooks, bytes emitted) and start the next frame.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_render_trace_stat_frame() {
    let index = FRAME_COUNT.fetch_add(1, Ordering::Relaxed) as usize;
    let ns = FRAME_NS.swap(0, Ordering::Relaxed);
    let bytes = FRAME_BYTES.swap(0, Ordering::Relaxed);
    if index < SAMPLE_CAP {
        SAMPLE_NS[index].store(ns.min(u64::from(u32::MAX)) as u32, Ordering::Relaxed);
        SAMPLE_BYTES[index].store(bytes.min(u64::from(u32::MAX)) as u32, Ordering::Relaxed);
    }
}

fn percentiles(cells: &[AtomicU32], count: usize) -> (u64, u64, u64) {
    let mut samples: Vec<u32> = cells[..count]
        .iter()
        .map(|cell| cell.load(Ordering::Relaxed))
        .collect();
    samples.sort_unstable();
    match samples.len() {
        0 => (0, 0, 0),
        n => (
            u64::from(samples[n / 2]),
            u64::from(samples[(n * 99 / 100).min(n - 1)]),
            u64::from(samples[n - 1]),
        ),
    }
}

/// Print the guest side section of the stats (called by the C side every few seconds and at close).
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_render_trace_stat_print() {
    if !stats_on() {
        return;
    }
    let frames = (FRAME_COUNT.load(Ordering::Relaxed) as usize).min(SAMPLE_CAP);
    let (n50, n99, nmax) = percentiles(&SAMPLE_NS, frames);
    let (b50, b99, bmax) = percentiles(&SAMPLE_BYTES, frames);
    // cost of one timer read: 256 back to back reads
    let probe = unsafe { tsfp_render_trace_now_ns() };
    for _ in 0..256 {
        let _ = unsafe { tsfp_render_trace_now_ns() };
    }
    let overhead = (unsafe { tsfp_render_trace_now_ns() } - probe) / 257;
    let mut text = alloc::format!(
        "rt-host: guest-trace: {frames} presents, guest hook time per present p50 {} us p99 {} us max {} us, emitted per present p50 {} KiB p99 {} KiB max {} KiB (timer read {overhead} ns, two per hook)
",
        n50 / 1000, n99 / 1000, nmax / 1000, b50 / 1024, b99 / 1024, bmax / 1024
    );
    let _ = writeln!(
        text,
        "rt-host: guest-trace: lib counter-only {} (ignored by live renderer, per-word timing omitted), fast hooks {}",
        LIB_FILTERED.load(Ordering::Relaxed),
        fast_hooks()
    );
    let deferred = TEXELS_DEFERRED.load(Ordering::Relaxed);
    if deferred != 0 {
        let _ = write!(
            text,
            "rt-host: guest-trace: {deferred} texture uploads were deferred to a later frame (per frame budget)\n"
        );
    }
    let omitted = DRAWS_OMITTED.load(Ordering::Relaxed);
    if omitted != 0 {
        let _ = write!(
            text,
            "rt-host: guest-trace: {omitted} draw calls were omitted while the renderer was behind\n"
        );
    }
    let frames_f = frames.max(1) as u64;
    for kind in 0..KINDS {
        let (ns, count, bytes) = (
            KIND_NS[kind].load(Ordering::Relaxed),
            KIND_COUNT[kind].load(Ordering::Relaxed),
            KIND_BYTES[kind].load(Ordering::Relaxed),
        );
        if count != 0 {
            let _ = write!(
                text,
                "rt-host: guest-trace:   {:<18} {:>7} us/present {:>6} hooks/present {:>7} B/present {:>6} ns/hook
",
                KIND_NAMES[kind], ns / frames_f / 1000, count / frames_f, bytes / frames_f, ns / count
            );
            let _ = writeln!(
                text,
                "rt-host: guest-trace-ms: {} total {} ms, {} timed hooks",
                KIND_NAMES[kind],
                ns / 1_000_000,
                count
            );
        }
    }
    unsafe { tsfp_render_trace_stderr(text.as_ptr(), text.len()) }
}

/// Last `BindVertexStream` arguments (base, stride). The lane is single threaded, relaxed ordering is enough.
static BOUND_BASE: AtomicU32 = AtomicU32::new(0);
static BOUND_STRIDE: AtomicU32 = AtomicU32::new(0);
/// `lock_vertex_buffer` results by buffer record: the vertex ring buffers are locked once per batch and their data pointer is what
/// the draws read. `BindVertexStream` carries the ring record field `+0x10` (a small ring offset, 0x200 measured), not the pointer.
static LOCK_BUFFER: [AtomicU32; 4] = [
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
];
static LOCK_DATA: [AtomicU32; 4] = [
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
];
/// Guest addresses outside this window are never read (the mapped guest faults on the null page).
const GUEST_LOW: u32 = 0x1_0000;
const GUEST_HIGH: u32 = 0x8000_0000;
/// Largest memory snapshot of one draw (bytes); a larger read is cut and flagged in a comment line.
const SNAPSHOT_LIMIT: u32 = 0x40_0000;

/// True when the guest span is inside the window and the host maps it (`tsfp_render_trace_readable`: the guest image, or one
/// tracked allocation over the whole span). The mapped guest faults on anything else, and the engine's vertex base can be stale
/// (MEASURED: a snapshot read at the end of a mapped region killed the host).
fn readable(address: u32, length: u32) -> bool {
    address >= GUEST_LOW
        && address.saturating_add(length) <= GUEST_HIGH
        && unsafe { tsfp_render_trace_readable(address, length) != 0 }
}

pub fn enabled() -> bool {
    unsafe { tsfp_render_trace_enabled() != 0 }
}

/// The native wgpu stream already carries `RenderDevice::push_method` as a typed call. When the
/// host opts in, that call is the implementation and the C D3D8 method-packet path is skipped.
/// Cache the host decision after trace activation so the per-method path stays in Rust.
static NATIVE_PUSH_METHOD: AtomicU32 = AtomicU32::new(0);

pub fn native_push_method_enabled() -> bool {
    match NATIVE_PUSH_METHOD.load(Ordering::Relaxed) {
        1 => false,
        2 => true,
        _ => {
            // A text trace may be active before the live binary sink is installed. Do not
            // latch that temporary state as the process-wide switch decision.
            if unsafe { tsfp_render_trace_binary_active() } != 1 {
                return false;
            }
            let enabled = unsafe { tsfp_render_trace_native_push_method() != 0 };
            let selected = if enabled { 2 } else { 1 };
            match NATIVE_PUSH_METHOD.compare_exchange(
                0,
                selected,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => enabled,
                Err(value) => value == 2,
            }
        }
    }
}

/// The native wgpu stream can consume the engine's vertex arrays as typed state. This switch is
/// separate from raw `PushMethod` forwarding so each replacement seam can be gated independently.
static NATIVE_VERTEX_BINDINGS: AtomicU32 = AtomicU32::new(0);

pub fn native_vertex_bindings_enabled() -> bool {
    match NATIVE_VERTEX_BINDINGS.load(Ordering::Relaxed) {
        1 => false,
        2 => true,
        _ => {
            if unsafe { tsfp_render_trace_binary_active() } != 1 {
                return false;
            }
            let enabled = unsafe { tsfp_render_trace_native_vertex_bindings() != 0 };
            let selected = if enabled { 2 } else { 1 };
            match NATIVE_VERTEX_BINDINGS.compare_exchange(
                0,
                selected,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => enabled,
                Err(value) => value == 2,
            }
        }
    }
}

/// Whether a supported state handler emits typed methods in the binary live path.
pub fn native_state_handler_enabled(index: u32) -> bool {
    (unsafe { tsfp_render_trace_binary_active() == 1 })
        && tsfp_port_engine_render_d3d::native_state::selector_enabled(index, |index| unsafe {
            tsfp_render_trace_native_state_handler(index) != 0
        })
}

/// Draws counted as omitted since the start (the frames the guest dropped because the renderer was behind).
static DRAWS_OMITTED: AtomicU64 = AtomicU64::new(0);

/// True while the guest omits draws: the binary form only, and only while the renderer is behind (the rt-host reports it, the C
/// side latches it at each present). The lane skips the draw hooks entirely (`render_lane.rs`): no vertex or index snapshot, no
/// draw call, no slot sync. State, constants, textures and clears are still traced, so the renderer's state stays exact and the
/// next frame that is not omitted draws as it would have.
pub fn dropping() -> bool {
    if !omitting() {
        return false;
    }
    DRAWS_OMITTED.fetch_add(1, Ordering::Relaxed);
    true
}

/// True while the frames are omitted ([`dropping`] without the counter).
fn omitting() -> bool {
    mode() == 1 && unsafe { tsfp_render_trace_dropping() } != 0
}

/// `Frame` record from the C side: the flags for the frames that follow.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_render_trace_bin_frame(flags: u32) {
    bin(|buffer| wire::frame(buffer, flags));
}

// ---- T2136d: binary sink ----
/// 0 unread, 1 binary events only, 2 text lines only, 3 both (`TSFP_RT_HOST_BINARY=dual`: the test mode that writes the text
/// and the binary form of the same run, so `binary_check --against` can compare the two on real guest data).
static BINARY: AtomicU32 = AtomicU32::new(0);

fn mode() -> u32 {
    match BINARY.load(Ordering::Relaxed) {
        0 => {
            let mode = match unsafe { tsfp_render_trace_binary_active() } {
                0 => 2,
                2 => 3,
                _ => 1,
            };
            BINARY.store(mode, Ordering::Relaxed);
            mode
        }
        mode => mode,
    }
}

static FAST_HOOKS: AtomicU32 = AtomicU32::new(0);
pub(crate) fn fast_hooks() -> bool {
    let flag = FAST_HOOKS.load(Ordering::Relaxed);
    if flag != 0 {
        return flag == 1;
    }
    let on = unsafe { tsfp_render_trace_fast_hooks() } != 0;
    FAST_HOOKS.store(if on { 1 } else { 2 }, Ordering::Relaxed);
    on
}
static LIB_FILTERED: AtomicU64 = AtomicU64::new(0);

/// Fast lib entry, called before the C per-word timer. Ignored upload methods
/// become a counter only; state and clear commands retain their original order.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_render_trace_fast_lib(method: u32, value: u32) {
    if !crate::hook_policy::live_library_method(method) {
        LIB_FILTERED.fetch_add(1, Ordering::Relaxed);
        return;
    }
    let _span = span(K_C_LIB);
    bin(|buffer| wire::lib(buffer, method, value));
}

/// True when the events go out as binary records. Read after the trace opened (every hook runs behind [`enabled`]).
fn binary() -> bool {
    matches!(mode(), 1 | 3)
}

/// True when the events go out as text lines (the default).
fn text() -> bool {
    matches!(mode(), 2 | 3)
}

/// Binary records waiting for the hand-over. The lane is single threaded; the flag only keeps a stray second thread out.
struct Out {
    busy: AtomicBool,
    bytes: UnsafeCell<Vec<u8>>,
}

// SAFETY: every access to `bytes` happens while `busy` is held.
unsafe impl Sync for Out {}

static OUT: Out = Out {
    busy: AtomicBool::new(false),
    bytes: UnsafeCell::new(Vec::new()),
};

/// A buffer this large is handed over before the present (a long frame).
const FLUSH_AT: usize = 256 * 1024;

fn hand_over(buffer: &mut Vec<u8>, presents: u32) {
    if !buffer.is_empty() {
        unsafe { tsfp_render_trace_binary_send(buffer.as_ptr(), buffer.len(), presents) };
        buffer.clear();
    }
}

/// Run `write` on the binary buffer, account the bytes it added and hand the buffer over when it is large.
fn bin<R>(write: impl FnOnce(&mut Vec<u8>) -> R) -> R {
    while OUT
        .busy
        .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
        .is_err()
    {
        core::hint::spin_loop();
    }
    let buffer = unsafe { &mut *OUT.bytes.get() };
    if buffer.capacity() == 0 {
        buffer.reserve(if fast_hooks() {
            4 * FLUSH_AT + 64 * 1024
        } else {
            FLUSH_AT + 64 * 1024
        });
    }
    let before = buffer.len();
    let result = write(buffer);
    if stats_on() {
        let added = buffer.len().saturating_sub(before) as u64;
        let kind = CURRENT_KIND.load(Ordering::Relaxed) as usize;
        KIND_BYTES[kind].fetch_add(added, Ordering::Relaxed);
        FRAME_BYTES.fetch_add(added, Ordering::Relaxed);
    }
    if buffer.len() >= if fast_hooks() { 4 * FLUSH_AT } else { FLUSH_AT } {
        hand_over(buffer, 0);
    }
    OUT.busy.store(false, Ordering::Release);
    result
}

/// Insert the Rust-owned view layer at the source drawer boundary. Keep the marker in
/// the same producer buffer as preceding draws: handing over a separate text marker
/// would overtake those records. Present still seals frame layers before consumption.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_render_trace_view_layer_boundary() {
    if !enabled() {
        return;
    }
    if binary() {
        bin(|buffer| wire::call(buffer, "NativeViewLayerBoundary", &[]));
    }
    if text() {
        emit("view-layer-boundary");
    }
}

/// End of a present (C side, binary mode): the `Present` record, then everything queued goes to the render thread.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_render_trace_bin_present(number: u64) {
    TEXEL_BYTES.store(0, Ordering::Relaxed);
    bin(|buffer| {
        wire::present(buffer, number);
        hand_over(buffer, 1);
    });
}

/// `call SetSurface` from the C side (binary mode): eight words at `words`.
///
/// # Safety
/// `words` points at eight readable `u32`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_render_trace_bin_surface(words: *const u32) {
    let words: [u32; 8] = core::array::from_fn(|index| unsafe { *words.add(index) });
    bin(|buffer| wire::surface(buffer, &words));
}

/// `lib <method> <value>` from the C side (binary mode).
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_render_trace_bin_lib(method: u32, value: u32) {
    bin(|buffer| wire::lib(buffer, method, value));
}

/// `call SetVertexConstants <index> <words>` from the C side (binary mode).
///
/// # Safety
/// `words` points at `count` readable `u32`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_render_trace_bin_constants(
    index: u32,
    words: *const u32,
    count: u32,
) {
    let words = unsafe { core::slice::from_raw_parts(words, count as usize) };
    bin(|buffer| wire::call_with_first(buffer, "SetVertexConstants", index, words));
}

/// Comment lines only exist in the text form; the closure is not run for the binary one.
fn note(make: impl FnOnce() -> String) {
    if text() {
        emit(&make());
    }
}

/// Fast content hash of a snapshot (four independent 64 bit lanes, the binary dedup tables only compare it with itself).
fn fast_hash(bytes: &[u8]) -> u32 {
    const K: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut lanes = [K ^ bytes.len() as u64, 1, 2, 3];
    let mut blocks = bytes.chunks_exact(32);
    for block in &mut blocks {
        for (lane, word) in lanes.iter_mut().zip(block.chunks_exact(8)) {
            let value = u64::from_le_bytes([
                word[0], word[1], word[2], word[3], word[4], word[5], word[6], word[7],
            ]);
            *lane = (*lane ^ value).wrapping_mul(K).rotate_left(29);
        }
    }
    for &byte in blocks.remainder() {
        lanes[0] = (lanes[0] ^ u64::from(byte)).wrapping_mul(K).rotate_left(11);
    }
    let folded =
        lanes[0] ^ lanes[1].rotate_left(16) ^ lanes[2].rotate_left(32) ^ lanes[3].rotate_left(48);
    (folded ^ (folded >> 32)) as u32
}

fn emit(text: &str) {
    let started = if stats_on() {
        unsafe { tsfp_render_trace_now_ns() }
    } else {
        0
    };
    unsafe { tsfp_render_trace_line(text.as_ptr(), text.len()) }
    if started != 0 {
        let kind = CURRENT_KIND.load(Ordering::Relaxed) as usize;
        let bytes = text.len() as u64 + 1;
        KIND_BYTES[kind].fetch_add(bytes, Ordering::Relaxed);
        FRAME_BYTES.fetch_add(bytes, Ordering::Relaxed);
        stat_add(
            K_N_EMIT,
            unsafe { tsfp_render_trace_now_ns() }.saturating_sub(started),
            0,
        );
    }
}

fn words_text(name: &str, words: &[u32]) -> String {
    let mut text = String::from(name);
    for word in words {
        let _ = write!(text, " {word:x}");
    }
    text
}

/// `call <Name> <hex words>`.
pub fn call(name: &str, words: &[u32]) {
    let _span = span(K_CALL);
    emit_call(name, words);
}

/// A `call` line or record, no span (for the hooks that already have one).
fn emit_call(name: &str, words: &[u32]) {
    if binary() {
        bin(|buffer| wire::call(buffer, name, words));
    }
    if text() {
        emit(&words_text(&alloc::format!("call {name}"), words));
    }
}

/// `call SetScissor <count> <flags> <signed decimal corners>` (the stream format keeps the corners decimal).
pub fn scissor(count: u32, flags: u32, rects: &[[i32; 4]]) {
    let _span = span(K_SCISSOR);
    if binary() {
        bin(|buffer| wire::scissor(buffer, count, flags, rects));
    }
    if !text() {
        return;
    }
    let mut text = alloc::format!("call SetScissor {count:x} {flags:x}");
    for rect in rects {
        let _ = write!(text, " {} {} {} {}", rect[0], rect[1], rect[2], rect[3]);
    }
    emit(&text);
}

/// Records the data pointer a `lock_vertex_buffer` returned.
pub fn lock_result(buffer: u32, data: u32) {
    let _span = span(K_LOCK);
    let slot = LOCK_BUFFER
        .iter()
        .position(|cell| cell.load(Ordering::Relaxed) == buffer)
        .or_else(|| {
            LOCK_BUFFER
                .iter()
                .position(|cell| cell.load(Ordering::Relaxed) == 0)
        });
    if let Some(slot) = slot {
        LOCK_BUFFER[slot].store(buffer, Ordering::Relaxed);
        LOCK_DATA[slot].store(data, Ordering::Relaxed);
    }
    note(|| alloc::format!("# lock {buffer:x} -> {data:x}"));
}

/// Data pointer of the vertex ring buffer the renderer is currently filling (ring record `[0x54F128 + ring * 20]` holds the buffer).
fn ring_data<M: GuestMemory>(memory: &M) -> Option<u32> {
    let read32 = |address: u32| memory.read32(address);
    let ring = read32(VERTEX_RING_INDEX);
    if ring > 1 {
        return None;
    }
    let buffer = read32(ring * VERTEX_RING_STRIDE + VERTEX_RING_BASE);
    let slot = LOCK_BUFFER
        .iter()
        .position(|cell| cell.load(Ordering::Relaxed) == buffer)?;
    Some(LOCK_DATA[slot].load(Ordering::Relaxed))
}

/// A `RenderHost` call. The vertex stream binding is written with the ring data pointer as its base (the replayer reads vertices at
/// `base + index * stride`) and also updates the bound base and stride used by the draw snapshots.
pub fn host<M: GuestMemory>(memory: &M, function: u32, args: &[u32]) {
    let _span = span(K_HOST);
    if function == 0x19240 && args.len() == 2 {
        let base = ring_data(memory).unwrap_or(args[0]);
        BOUND_BASE.store(base, Ordering::Relaxed);
        BOUND_STRIDE.store(args[1], Ordering::Relaxed);
        ARRAYS_ACTIVE.store(0, Ordering::Relaxed);
        note(|| alloc::format!("# bind ring offset {:x}", args[0]));
        emit_host(function, &[base, args[1]]);
        return;
    }
    emit_host(function, args);
}

fn emit_host(function: u32, words: &[u32]) {
    if binary() {
        bin(|buffer| wire::host(buffer, function, words));
    }
    if text() {
        emit(&words_text(&alloc::format!("host {function:x}"), words));
    }
}

/// `mem <address> <hex bytes>` of guest memory the next draw reads.
pub fn snapshot<M: GuestMemory>(memory: &M, address: u32, length: u32) {
    snapshot_alias(memory, address, address, length);
}

/// Append `length` guest bytes: one `memcpy` from the mapped guest when the memory can hand out a slice, else a copy loop.
fn append_guest_bytes<M: GuestMemory>(buffer: &mut Vec<u8>, memory: &M, address: u32, length: u32) {
    if let Some(source) = memory.as_slice(address, length) {
        buffer.extend_from_slice(source);
    } else {
        let start = buffer.len();
        buffer.resize(start + length as usize, 0);
        memory.read_bytes(address, &mut buffer[start..]);
    }
}

/// `mem <alias> <hex bytes>` of the guest bytes at `address` (the replay image keeps them at `alias`).
fn snapshot_alias<M: GuestMemory>(memory: &M, address: u32, alias: u32, length: u32) {
    snapshot_alias_to(memory, address, alias, length, binary(), text());
}

/// [`snapshot_alias`] to the chosen forms only.
fn snapshot_alias_to<M: GuestMemory>(
    memory: &M,
    address: u32,
    alias: u32,
    length: u32,
    to_binary: bool,
    to_text: bool,
) {
    if length == 0 {
        return;
    }
    if !readable(address, length) {
        note(|| {
            alloc::format!(
                "# snapshot at {address:x} length {length:x} outside the guest window, skipped"
            )
        });
        return;
    }
    let capped = length.min(SNAPSHOT_LIMIT);
    if to_binary {
        let _copy = span(K_N_FORMAT);
        bin(|buffer| {
            let at = wire::memory_begin(buffer, alias, capped);
            append_guest_bytes(buffer, memory, address, capped);
            wire::memory_end(buffer, at);
        });
    }
    if !to_text {
        return;
    }
    let format_span = span(K_N_FORMAT);
    let mut text = alloc::format!("mem {alias:x} ");
    text.reserve(capped as usize * 2);
    for offset in 0..capped {
        let _ = write!(text, "{:02x}", memory.read8(address.wrapping_add(offset)));
    }
    drop(format_span);
    emit(&text);
    if capped != length {
        note(|| {
            alloc::format!("# snapshot at {address:x} cut from {length:x} to {capped:x} bytes")
        });
    }
}

/// Last snapshots written by [`snapshot_dedup`]: address, length and a full content hash, direct mapped by address. A draw that
/// reads bytes identical to the last snapshot at the same address and length writes nothing (the replay image keeps them).
static DEDUP_ADDRESS: [AtomicU32; 2048] = [const { AtomicU32::new(0) }; 2048];
static DEDUP_LENGTH: [AtomicU32; 2048] = [const { AtomicU32::new(0) }; 2048];
static DEDUP_HASH: [AtomicU32; 2048] = [const { AtomicU32::new(0) }; 2048];

/// [`snapshot`] unless the same bytes were the last snapshot at this address (full FNV-1a over the dwords).
fn snapshot_dedup<M: GuestMemory>(memory: &M, address: u32, length: u32) {
    if length == 0 || !readable(address, length) {
        snapshot(memory, address, length);
        return;
    }
    if binary() {
        snapshot_binary_dedup(memory, address, length);
    }
    if !text() {
        return;
    }
    let hash_span = span(K_N_HASH);
    let mut hash = 0x811C_9DC5u32 ^ length;
    let mut offset = 0;
    while offset + 4 <= length {
        hash = (hash ^ read32(memory, address + offset)).wrapping_mul(0x0100_0193);
        offset += 4;
    }
    for tail in offset..length {
        hash = (hash ^ u32::from(memory.read8(address + tail))).wrapping_mul(0x0100_0193);
    }
    drop(hash_span);
    let slot = ((address >> 2) & 2047) as usize;
    if DEDUP_ADDRESS[slot].load(Ordering::Relaxed) == address
        && DEDUP_LENGTH[slot].load(Ordering::Relaxed) == length
        && DEDUP_HASH[slot].load(Ordering::Relaxed) == hash
    {
        return;
    }
    DEDUP_ADDRESS[slot].store(address, Ordering::Relaxed);
    DEDUP_LENGTH[slot].store(length, Ordering::Relaxed);
    DEDUP_HASH[slot].store(hash, Ordering::Relaxed);
    snapshot_alias_to(memory, address, address, length, false, true);
}

/// The binary form's own dedup tables (the text form keeps its FNV tables above, so the dual test mode can run both).
static BDEDUP_ADDRESS: [AtomicU32; 2048] = [const { AtomicU32::new(0) }; 2048];
static BDEDUP_LENGTH: [AtomicU32; 2048] = [const { AtomicU32::new(0) }; 2048];
static BDEDUP_HASH: [AtomicU32; 2048] = [const { AtomicU32::new(0) }; 2048];

/// Binary snapshot with dedup; an unreadable, empty or oversized span goes through the plain snapshot.
fn snapshot_binary_dedup<M: GuestMemory>(memory: &M, address: u32, length: u32) {
    if length == 0 || !readable(address, length) || length > SNAPSHOT_LIMIT {
        snapshot_alias_to(memory, address, address, length, true, false);
    } else {
        snapshot_dedup_binary(memory, address, length);
    }
}

/// Binary form of [`snapshot_dedup`]: the bytes are copied into the record once, hashed there (one pass over cached data) and
/// the record is dropped again when the last snapshot at this address held the same bytes.
fn snapshot_dedup_binary<M: GuestMemory>(memory: &M, address: u32, length: u32) {
    // Flat guest memory: hash the bytes where they are and copy them only when they changed (no copy of an unchanged buffer).
    if let Some(source) = memory.as_slice(address, length) {
        let hash_span = span(K_N_HASH);
        let hash = fast_hash(source);
        drop(hash_span);
        let slot = ((address >> 2) & 2047) as usize;
        if BDEDUP_ADDRESS[slot].load(Ordering::Relaxed) == address
            && BDEDUP_LENGTH[slot].load(Ordering::Relaxed) == length
            && BDEDUP_HASH[slot].load(Ordering::Relaxed) == hash
        {
            return;
        }
        BDEDUP_ADDRESS[slot].store(address, Ordering::Relaxed);
        BDEDUP_LENGTH[slot].store(length, Ordering::Relaxed);
        BDEDUP_HASH[slot].store(hash, Ordering::Relaxed);
        let _copy = span(K_N_FORMAT);
        bin(|buffer| {
            let at = wire::memory_begin(buffer, address, length);
            buffer.extend_from_slice(source);
            wire::memory_end(buffer, at);
        });
        return;
    }
    let copy = span(K_N_FORMAT);
    bin(|buffer| {
        let at = wire::memory_begin(buffer, address, length);
        let start = buffer.len();
        buffer.resize(start + length as usize, 0);
        memory.read_bytes(address, &mut buffer[start..]);
        drop(copy);
        let hash_span = span(K_N_HASH);
        let hash = fast_hash(&buffer[start..]);
        drop(hash_span);
        let slot = ((address >> 2) & 2047) as usize;
        if BDEDUP_ADDRESS[slot].load(Ordering::Relaxed) == address
            && BDEDUP_LENGTH[slot].load(Ordering::Relaxed) == length
            && BDEDUP_HASH[slot].load(Ordering::Relaxed) == hash
        {
            buffer.truncate(at);
            return;
        }
        BDEDUP_ADDRESS[slot].store(address, Ordering::Relaxed);
        BDEDUP_LENGTH[slot].store(length, Ordering::Relaxed);
        BDEDUP_HASH[slot].store(hash, Ordering::Relaxed);
        wire::memory_end(buffer, at);
    });
}

/// NV2A vertex arrays the engine sets up with its own push buffer writes (methods `0x1720 + 4i` offset and `0x1760 + 4i` format,
/// seen by [`push_end`]). Active until the next `BindVertexStream`, which makes the lifted library write the arrays out of sight.
/// MEASURED: the in level meshes use separate arrays per attribute, not the one interleaved stream the bind carries.
static ARRAY_OFFSET: [AtomicU32; 16] = [const { AtomicU32::new(0) }; 16];
static ARRAY_FORMAT: [AtomicU32; 16] = [const { AtomicU32::new(0) }; 16];
static ARRAYS_ACTIVE: AtomicU32 = AtomicU32::new(0);

/// Record the engine's typed vertex-array operation for both the live stream and the draw
/// snapshotter. The ordinary push-buffer path reaches the same state through [`push_end`]; the
/// native typed path bypasses that packet decoder, so it must update these snapshot inputs here.
fn store_vertex_bindings(
    array_formats: &[AtomicU32; 16],
    array_offsets: &[AtomicU32; 16],
    arrays_active: &AtomicU32,
    formats: &[u32; 16],
    offsets: &[u32; 16],
) {
    for index in 0..16 {
        array_formats[index].store(formats[index], Ordering::Relaxed);
        array_offsets[index].store(offsets[index], Ordering::Relaxed);
    }
    arrays_active.store(1, Ordering::Relaxed);
}

pub fn native_vertex_bindings(formats: [u32; 16], offsets: [u32; 16]) {
    store_vertex_bindings(
        &ARRAY_FORMAT,
        &ARRAY_OFFSET,
        &ARRAYS_ACTIVE,
        &formats,
        &offsets,
    );

    let mut words = Vec::with_capacity(32);
    words.extend_from_slice(&formats);
    words.extend_from_slice(&offsets);
    call("SetVertexBindings", &words);
}

#[cfg(test)]
mod tests {
    use super::{AtomicU32, store_vertex_bindings};

    #[test]
    fn native_vertex_bindings_feed_draw_memory_snapshots() {
        let formats = core::array::from_fn(|index| 0x1000 + index as u32);
        let offsets = core::array::from_fn(|index| 0x2000 + index as u32);
        let array_formats = core::array::from_fn(|_| AtomicU32::new(0));
        let array_offsets = core::array::from_fn(|_| AtomicU32::new(0));
        let arrays_active = AtomicU32::new(0);

        store_vertex_bindings(
            &array_formats,
            &array_offsets,
            &arrays_active,
            &formats,
            &offsets,
        );

        for index in 0..16 {
            assert_eq!(
                array_formats[index].load(core::sync::atomic::Ordering::Relaxed),
                formats[index]
            );
            assert_eq!(
                array_offsets[index].load(core::sync::atomic::Ordering::Relaxed),
                offsets[index]
            );
        }
        assert_eq!(arrays_active.load(core::sync::atomic::Ordering::Relaxed), 1);
    }
}

/// Snapshot the vertex array bytes of `count` vertices from vertex `first` of every enabled array.
fn snapshot_arrays<M: GuestMemory>(memory: &M, first: u32, count: u32) {
    for index in 0..16 {
        let format = ARRAY_FORMAT[index].load(Ordering::Relaxed);
        if (format >> 4) & 0xF == 0 {
            continue;
        }
        let stride = format >> 8;
        let offset = ARRAY_OFFSET[index].load(Ordering::Relaxed);
        if stride == 0 {
            snapshot_dedup(memory, offset, 16);
        } else {
            snapshot_dedup(
                memory,
                offset.wrapping_add(first.wrapping_mul(stride)),
                count.wrapping_mul(stride),
            );
        }
    }
}

/// The guest bytes `DrawVertices` reads from the bound stream.
pub fn draw_vertices<M: GuestMemory>(memory: &M, start: u32, count: u32) {
    let _span = span(K_DRAW);
    slot_sync(memory);
    if ARRAYS_ACTIVE.load(Ordering::Relaxed) != 0 {
        snapshot_arrays(memory, start, count);
        return;
    }
    let stride = BOUND_STRIDE.load(Ordering::Relaxed);
    if stride != 0 {
        let base = BOUND_BASE.load(Ordering::Relaxed);
        snapshot_dedup(
            memory,
            base.wrapping_add(start.wrapping_mul(stride)),
            count.wrapping_mul(stride),
        );
    }
}

/// The index array and the vertex range it reaches for `DrawIndexedVertices` (lowest to highest index, not from zero).
pub fn draw_indexed<M: GuestMemory>(memory: &M, count: u32, indices: u32) {
    let _span = span(K_DRAW);
    slot_sync(memory);
    snapshot_dedup(memory, indices, count.wrapping_mul(2));
    let stride = BOUND_STRIDE.load(Ordering::Relaxed);
    let arrays = ARRAYS_ACTIVE.load(Ordering::Relaxed) != 0;
    if (stride != 0 || arrays) && count != 0 && readable(indices, count.min(0x10000) * 2) {
        let (mut lowest, mut highest) = (u32::MAX, 0u32);
        for index in 0..count.min(0x10000) {
            let value = u32::from(memory.read8(indices.wrapping_add(index * 2)))
                | (u32::from(memory.read8(indices.wrapping_add(index * 2 + 1))) << 8);
            lowest = lowest.min(value);
            highest = highest.max(value);
        }
        if arrays {
            snapshot_arrays(memory, lowest, highest - lowest + 1);
            return;
        }
        snapshot_dedup(
            memory,
            BOUND_BASE
                .load(Ordering::Relaxed)
                .wrapping_add(lowest.wrapping_mul(stride)),
            (highest - lowest + 1).wrapping_mul(stride),
        );
    }
}

/// Direct mapped cache of the texture snapshots already written (texel address and a sampled content hash), so a texture bound
/// thousands of times is written once and a texture whose texels change (render to texture, video) is written again.
static TEXTURE_SEEN_ADDRESS: [AtomicU32; 16384] = [const { AtomicU32::new(0) }; 16384];
static TEXTURE_SEEN_HASH: [AtomicU32; 16384] = [const { AtomicU32::new(0) }; 16384];
static TEXTURE_WRITES: [AtomicU32; 16384] = [const { AtomicU32::new(0) }; 16384];

/// Slice 6 A/B control (`TSFP_RENDER_TRACE_LEGACY_TEXTURES=1`, set by the C side): the slice 5 texture cache, 4096 slots keyed by
/// `address >> 4` and a write budget of three snapshots per slot that was never given back, which lost the texels of a texture
/// whose slot another texture had used (the black gun).
static LEGACY_TEXTURES: AtomicU32 = AtomicU32::new(0);

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_render_trace_set_legacy_textures(on: u32) {
    LEGACY_TEXTURES.store(on, Ordering::Relaxed);
}

/// T2024 slice 6: forget every snapshot already written. The C side calls it when a skip stream starts recording
/// (`TSFP_RENDER_TRACE_SKIP`): the dedupe caches were filled while the lines were being dropped, so a texture or buffer bound
/// before the skip point and never changed was missing from the stream (the black gun of slice 5 was a missing texture).
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_render_trace_reset_dedupe() {
    for slot in 0..2048 {
        DEDUP_ADDRESS[slot].store(0, Ordering::Relaxed);
        DEDUP_LENGTH[slot].store(0, Ordering::Relaxed);
        DEDUP_HASH[slot].store(0, Ordering::Relaxed);
        BDEDUP_ADDRESS[slot].store(0, Ordering::Relaxed);
        BDEDUP_LENGTH[slot].store(0, Ordering::Relaxed);
        BDEDUP_HASH[slot].store(0, Ordering::Relaxed);
    }
    for slot in 0..16384 {
        TEXTURE_SEEN_ADDRESS[slot].store(0, Ordering::Relaxed);
        TEXTURE_SEEN_HASH[slot].store(0, Ordering::Relaxed);
        TEXTURE_WRITES[slot].store(0, Ordering::Relaxed);
    }
}

/// Bits per texel of an NV2A colour format code (hardware format word bits 8 to 15), `None` when unknown.
fn texel_bits(format: u32) -> Option<u32> {
    Some(match format {
        0x00 | 0x01 | 0x0B | 0x0E | 0x0F | 0x13 | 0x19 | 0x1B | 0x1F => 8,
        0x0C => 4,
        0x02..=0x05 | 0x10 | 0x11 | 0x1A | 0x1C | 0x1D | 0x20 | 0x24 | 0x25 | 0x27 => 16,
        0x06 | 0x07 | 0x12 | 0x1E => 32,
        _ => return None,
    })
}

/// Linear (non swizzled) NV2A colour formats.
fn linear_format(format: u32) -> bool {
    matches!(format, 0x10..=0x13 | 0x1B..=0x1F | 0x20 | 0x25 | 0x27)
}

/// Base level size in bytes of a texture from its D3D pixel container header words (`format_word` is the hardware format word,
/// `size_word` the pitch and dimension word of linear textures). `None` for formats it does not know or sizes that look wrong.
fn texture_bytes(format_word: u32, size_word: u32) -> Option<u32> {
    let format = (format_word >> 8) & 0xFF;
    let bits = texel_bits(format)?;
    let bytes = if linear_format(format) {
        let height = ((size_word >> 12) & 0xFFF) + 1;
        let pitch = (((size_word >> 24) & 0xFF) + 1) * 64;
        pitch * height
    } else {
        let width = 1u32 << ((format_word >> 20) & 0xF);
        let height = 1u32 << ((format_word >> 24) & 0xF);
        match format {
            0x0C => width.div_ceil(4) * height.div_ceil(4) * 8,
            0x0E | 0x0F => width.div_ceil(4) * height.div_ceil(4) * 16,
            _ => width * height * bits / 8,
        }
    };
    (bytes != 0 && bytes <= 0x80_0000).then_some(bytes)
}

/// Where the replay image keeps texel data whose header Data word is `data`: the Xbox physical alias `0x80000000 | physical`
/// (the replayer applies the same rule, `tsfp_wgpu::texture::data_address`). The host's contiguous memory is at ordinary
/// addresses with a synthetic physical address (docs/d3d8-usage.md 13.8), so the bytes are read from the guest address the C
/// side finds for the physical address and written under the alias.
pub fn texture_data_alias(data: u32) -> u32 {
    (data & 0x0FFF_FFFF) | 0x8000_0000
}

fn read32<M: GuestMemory>(memory: &M, address: u32) -> u32 {
    (0..4).fold(0u32, |word, byte| {
        word | (u32::from(memory.read8(address.wrapping_add(byte))) << (8 * byte))
    })
}

/// Texel bytes written since the last present, and the budget per frame (0 unread; `tsfp_render_trace_texel_budget`, default 8 MiB,
/// `TSFP_RT_HOST_TEXEL_BUDGET_MB` overrides, 0 means unlimited). A level load binds tens of megabytes of new textures in one frame
/// (MEASURED on the owner's machine: 32 to 46 MB per present); copying all of it on the guest thread is a frame long stall, so the
/// uploads past the budget wait for a later frame (the draws using them show the old texels meanwhile).
static TEXEL_BYTES: AtomicU64 = AtomicU64::new(0);
static TEXEL_BUDGET: AtomicU64 = AtomicU64::new(u64::MAX);
static TEXELS_DEFERRED: AtomicU64 = AtomicU64::new(0);

fn texel_budget_exceeded(length: u32) -> bool {
    let mut budget = TEXEL_BUDGET.load(Ordering::Relaxed);
    if budget == u64::MAX {
        budget = match unsafe { tsfp_render_trace_texel_budget() } {
            0 => 0,
            bytes => u64::from(bytes),
        };
        TEXEL_BUDGET.store(budget, Ordering::Relaxed);
    }
    let used = TEXEL_BYTES.load(Ordering::Relaxed);
    // while the frames are omitted nothing is drawn, so no texel is needed: every upload waits until the renderer has caught up
    if !omitting() && (budget == 0 || used == 0 || used + u64::from(length) <= budget) {
        return false;
    }
    TEXELS_DEFERRED.fetch_add(1, Ordering::Relaxed);
    true
}

/// A texture record, raw: the 0x20 bytes at `texture` (D3D pixel container: data, format, size words), followed by the
/// base level texel data when the header decodes (once per distinct content, see [`TEXTURE_SEEN_HASH`]).
pub fn texture_record<M: GuestMemory>(memory: &M, texture: u32) {
    if texture == 0 {
        return;
    }
    let _span = span(K_TEXTURE);
    snapshot_cached(memory, texture, 0x20);
    if !readable(texture, 0x20) {
        return;
    }
    let (data, format, size) = (
        read32(memory, texture + 4),
        read32(memory, texture + 12),
        read32(memory, texture + 16),
    );
    let alias = texture_data_alias(data);
    let Some(length) = texture_bytes(format, size) else {
        note(|| {
            alloc::format!(
                "# texture {texture:x} data {data:x} format {format:x} size {size:x}: size unknown, no texel snapshot"
            )
        });
        return;
    };
    let address = unsafe { tsfp_render_trace_virtual_of_physical(data & 0x0FFF_FFFF, length) };
    if !readable(address, length) {
        note(|| {
            alloc::format!(
                "# texture {texture:x} data {data:x} format {format:x} size {size:x}: no live guest region (virtual {address:x})"
            )
        });
        return;
    }
    // Sampled hash: every 16th dword plus the length, cheap enough for thousands of binds.
    let mut hash = 0x811C_9DC5u32 ^ length;
    let mut offset = 0;
    while offset + 4 <= length {
        hash = (hash ^ read32(memory, address + offset)).wrapping_mul(0x0100_0193);
        offset += 64;
    }
    let legacy = LEGACY_TEXTURES.load(Ordering::Relaxed) != 0;
    let slot = if legacy {
        ((address >> 4) & 4095) as usize
    } else {
        (((address >> 4) ^ (address >> 14) ^ (address >> 21)) & 16383) as usize
    };
    if TEXTURE_SEEN_ADDRESS[slot].load(Ordering::Relaxed) == address
        && TEXTURE_SEEN_HASH[slot].load(Ordering::Relaxed) == hash
    {
        return;
    }
    if binary() && texel_budget_exceeded(length) {
        return; // upload deferred to a later frame: the seen tables are not updated, the next bind of this texture tries again
    }
    if !legacy && TEXTURE_SEEN_ADDRESS[slot].load(Ordering::Relaxed) != address {
        TEXTURE_WRITES[slot].store(0, Ordering::Relaxed); // a different texture took the slot: its write budget starts fresh
    }
    TEXTURE_SEEN_ADDRESS[slot].store(address, Ordering::Relaxed);
    TEXTURE_SEEN_HASH[slot].store(hash, Ordering::Relaxed);
    // A texture whose texels keep changing is a render target or a video frame the replayer cannot reproduce from a snapshot:
    // after 3 snapshots of one slot further changes are only noted (keeps the stream size bounded).
    if TEXTURE_WRITES[slot].fetch_add(1, Ordering::Relaxed) >= 3 {
        return;
    }
    TEXEL_BYTES.fetch_add(u64::from(length), Ordering::Relaxed);
    note(|| {
        alloc::format!(
            "# texture {texture:x} data {data:x} -> guest {address:x} alias {alias:x} format {format:x} size {size:x} bytes {length:x}"
        )
    });
    snapshot_alias(memory, address, alias, length);
}

/// The vertex program the lifted library uploads: u16 instruction count at `program + 2`, then 16 bytes per instruction at
/// `program + 4` (the layout `d3d8_upload_vertex_program` reads). Written as one `mem` line from `program + 2`.
pub fn program<M: GuestMemory>(memory: &M, program: u32) {
    let _span = span(K_SHADER);
    if !readable(program, 8) {
        return;
    }
    let count = u32::from(memory.read8(program + 2)) | (u32::from(memory.read8(program + 3)) << 8);
    if count <= 136 {
        snapshot_cached(memory, program + 2, 2 + count * 16);
    }
}

/// The 16 entry vertex declaration block (16 bytes per entry: +4 offset, +8 `size << 4 | type`).
pub fn declaration<M: GuestMemory>(memory: &M, declaration: u32) {
    let _span = span(K_SHADER);
    snapshot_cached(memory, declaration, 0x100);
}

/// The pixel shader definition the game binds (0x100 byte snapshot, `D3DPIXELSHADERDEF` is smaller) so the combiner words can
/// be decoded later.
pub fn pixel_shader<M: GuestMemory>(memory: &M, program: u32) {
    let _span = span(K_SHADER);
    if program != 0 {
        snapshot_cached(memory, program, 0x100);
    }
}

/// A snapshot of an object the game rebinds over and over (texture record, shader programs, declarations). The binary form drops
/// the record when the last snapshot at this address held the same bytes; the text form keeps writing every one (its output is
/// the reference the binary one is compared against).
fn snapshot_cached<M: GuestMemory>(memory: &M, address: u32, length: u32) {
    if binary() {
        snapshot_binary_dedup(memory, address, length);
    }
    if text() {
        snapshot_alias_to(memory, address, address, length, false, true);
    }
}

/// The IEEE bits of pixel shader constant vectors.
pub fn float_words(vectors: &[[f32; 4]]) -> Vec<u32> {
    vectors
        .iter()
        .flatten()
        .map(|value| value.to_bits())
        .collect()
}

/// Start cursor of the last engine `BeginPush` reservation (not cleared by [`push_end`]): the C side skips the `lib` trace of a
/// range that starts here, because the engine's own writes are traced as `PushMethod` (no duplicate clears or state).
static ENGINE_PUSH_BEGIN: AtomicU32 = AtomicU32::new(0);

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_render_trace_is_engine_push(begin: u32) -> i32 {
    i32::from(begin != 0 && ENGINE_PUSH_BEGIN.load(Ordering::Relaxed) == begin)
}

/// Start cursor of the open `BeginPush` reservation.
static PUSH_BEGIN: AtomicU32 = AtomicU32::new(0);

/// `begin_push` returned `cursor`: the engine now writes pushbuffer dwords at it directly (guest memory), invisible to the device
/// boundary until `end_push`.
pub fn push_begin(cursor: u32) {
    PUSH_BEGIN.store(cursor, Ordering::Relaxed);
    ENGINE_PUSH_BEGIN.store(cursor, Ordering::Relaxed);
}

/// `end_push(cursor)`: decode the dwords the engine wrote since [`push_begin`] as NV2A method packets and write one
/// `call PushMethod <method | 0x40000> <value>` per data dword (the same line shape the device call produces, so the replayer's
/// state decoder applies them: blend, depth and cull enables and the like only reach the pushbuffer this way, MEASURED).
pub fn push_end<M: GuestMemory>(memory: &M, cursor: u32) {
    let _span = span(K_PUSH);
    let begin = PUSH_BEGIN.swap(0, Ordering::Relaxed);
    if cursor <= begin || cursor - begin > 0x4000 || !readable(begin, cursor - begin) {
        return;
    }
    let mut at = begin;
    while at + 4 <= cursor {
        let header = read32(memory, at);
        let count = (header >> 18) & 0x7FF;
        let subchannel = (header >> 13) & 7;
        if header & 0xA000_0003 != 0 || at + 4 + count * 4 > cursor {
            note(|| {
                alloc::format!(
                    "# push {header:x} at {at:x}: not a method packet inside the reservation, decoding stopped"
                )
            });
            return;
        }
        let method = header & 0x1FFC;
        let incrementing = header & 0x4000_0000 == 0;
        for index in 0..count {
            let value = read32(memory, at + 4 + index * 4);
            let target = if incrementing {
                method + index * 4
            } else {
                method
            };
            if subchannel == 0 {
                match target {
                    0x1720..=0x175C => {
                        ARRAY_OFFSET[((target - 0x1720) / 4) as usize]
                            .store(value, Ordering::Relaxed);
                        ARRAYS_ACTIVE.store(1, Ordering::Relaxed);
                    }
                    0x1760..=0x179C => {
                        ARRAY_FORMAT[((target - 0x1760) / 4) as usize]
                            .store(value, Ordering::Relaxed);
                        ARRAYS_ACTIVE.store(1, Ordering::Relaxed);
                    }
                    _ => {}
                }
                emit_call("PushMethod", &[target | 0x4_0000, value]);
            } else {
                note(|| {
                    alloc::format!(
                        "# push subchannel {subchannel} method {target:x} value {value:x}"
                    )
                });
            }
        }
        at += 4 + count * 4;
    }
}

/// Engine render state slot table (`tsfp_engine_render::map::SLOT_TABLE`): 35 records of 0x18 bytes, `+0` the D3D render state
/// index the slot drives, `+4` the value last sent to the device. `METHOD_HEADER_TABLE` maps an index below 0x5C to the NV2A
/// method header the engine pushes for it.
const SLOT_TABLE: u32 = 0x004B_8D28;
const SLOT_COUNT: u32 = 35;
const SLOT_STRIDE: u32 = 0x18;
const METHOD_HEADER_TABLE: u32 = 0x0047_5B08;
static SLOT_SENT: [AtomicU32; 35] = [const { AtomicU32::new(0) }; 35];
static SLOT_SENT_VALID: [AtomicU32; 35] = [const { AtomicU32::new(0) }; 35];

/// Before a draw: write a `PushMethod` line for every immediate render state slot (index below 0x5C) whose committed value is not
/// the one last written to the trace. The engine only pushes a method when the value CHANGES from its committed copy, so the
/// device defaults (for example the blend enable the D3D library sets at device creation) never appear as a call: this writes the
/// complete state the draw runs under (MEASURED need: without it the real frames have no blend enable and every fade quad is opaque).
pub fn slot_sync<M: GuestMemory>(memory: &M) {
    let _span = span(K_N_SLOT_SYNC);
    for slot in 0..SLOT_COUNT {
        let record = SLOT_TABLE + slot * SLOT_STRIDE;
        let index = read32(memory, record);
        let committed = read32(memory, record + 4);
        if index >= 0x5C {
            continue;
        }
        let cell = slot as usize;
        if SLOT_SENT_VALID[cell].load(Ordering::Relaxed) != 0
            && SLOT_SENT[cell].load(Ordering::Relaxed) == committed
        {
            continue;
        }
        SLOT_SENT[cell].store(committed, Ordering::Relaxed);
        SLOT_SENT_VALID[cell].store(1, Ordering::Relaxed);
        let header = read32(memory, METHOD_HEADER_TABLE + index * 4);
        emit_call("PushMethod", &[(header & 0x1FFC) | 0x4_0000, committed]);
    }
}
