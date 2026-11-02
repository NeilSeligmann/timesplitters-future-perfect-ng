// SPDX-License-Identifier: GPL-3.0-or-later
//! T3933 slice 6: Rust-owned passive DirectSound buffer front-end for the Rust audio host. Replaces
//! `src/audio/dsound_buffer.c` (kept as the `TSFP_RUST_AUDIO_HOST=0` fallback and as the C oracle) under the same C symbols.
//! A line-for-line transcription of the C module's Rust-host branch: the same lock order (device lock, then the buffer
//! lock inside the lease callbacks), the same lease transaction, the same refusal texts and the same host records.
//! The setter bodies are in [`State::access_owned`].

use std::ffi::{CString, c_char, c_void};
use std::sync::{Mutex, MutexGuard};

use tsfp_audio::lifecycle::GuestWorld;

use crate::buffer_frame_ffi::{CBufferCall, tsfp_audio_buffer_frame_decode};
use crate::buffer_publish_ffi::{
    tsfp_audio_buffer_header_write_rust, tsfp_audio_buffer_output_probe_rust,
    tsfp_audio_buffer_pointer_publish_rust,
};
use crate::device_ffi::{
    CChild, CLease, CLeaseOps, HostWorld, tsfp_audio_device_acquire_lease,
    tsfp_audio_device_release_lease, tsfp_audio_device_with_owned_interface,
};
use crate::movie_device_hle_ffi::tsfp_audio_hle_register_handler_rust;

const CREATE: u32 = 0x0040_93C8;
const I3DL2: u32 = 0x0040_85AF;
const MINIMUM: u32 = 0x0040_8532;
const ROLLOFF: u32 = 0x0040_858B;
const SET_DATA: u32 = 0x0040_8C0D;
const SET_LOOP_REGION: u32 = 0x0040_7AD8;
const SET_VOLUME: u32 = 0x0040_7A64;
const VOLUME_STARTUP: u32 = 0xFFFF_D8F0;
const VOLUME_UPDATE: u32 = 0xFFFF_F37C;
const PAUSE: u32 = 0x0040_7ABC;
const SET_FREQUENCY: u32 = 0x0040_84F2;
const FREQUENCY_STARTUP: u32 = 22042;
const MAX_DISTANCE: u32 = 0x0040_850E;
const SET_POSITION: u32 = 0x0040_8556;
const FREQUENCY_MIN: u32 = 0xBC;
const FREQUENCY_MAX: u32 = 0x2EDEF;
const DATA_MAX_BYTES: u32 = 0x0400_0000;
const SINGLETON: u32 = 0x0041_2B30;
const GLOBAL_STATE: u32 = 0x0041_24A8;
const VTABLE: u32 = 0x004A_1CE0;
const CURVE: u32 = 0x0064_BC98;
const BUFFER_BYTES: u32 = 36;
const OOM: u32 = 0x8007_000E;
const E_FAIL: u32 = 0x8000_4005;
const PACKET_LIST_FULL: u32 = 0x8878_0032;
const STOPPED: [u32; 4] = [0x0040_6879, 0x0040_6FA9, 0x0040_6FF0, 0x0040_8040];

const LEASE_OK: i32 = 0;
const LEASE_OUT_OF_MEMORY: i32 = 4;
const LEASE_PREPARE_FAILED: i32 = 5;

type IrqlFn = unsafe extern "C" fn(*mut u8) -> bool;
type FatalFn = unsafe extern "C" fn(u32, *const c_char);
type StartedFn = unsafe extern "C" fn(u32, u64) -> bool;
type PauseFn = unsafe extern "C" fn(u32, u64, u32) -> bool;
type FrequencyFn = unsafe extern "C" fn(u32, u64, u32, u32) -> bool;
type VolumeFn = unsafe extern "C" fn(u32, u64, i32) -> bool;
type RunningFn = unsafe extern "C" fn(u32, u64) -> bool;
type LoopFn = unsafe extern "C" fn(u32, u64, u32, u32, u32) -> bool;
type Erased = Option<unsafe extern "C" fn()>;
type DsoundHandler = unsafe extern "C" fn(*mut c_void) -> u32;

unsafe extern "C" {
    fn dsound_hle_degrade(kind: *const c_char, entry: u32, reason: *const c_char) -> bool;
    fn dsound_hle_register_rust(address: u32) -> bool;
    fn dsound_hle_log_line(line: *const c_char);
    fn kernel_guest_read_u32(address: u32, out: *mut u32) -> bool;
    fn kernel_guest_read_bytes(address: u32, out: *mut c_void, length: usize) -> bool;
    fn kernel_guest_at_ro(address: u32, length: usize) -> *const c_void;
    fn kernel_host_pid() -> i32;
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
    fn tsfp_audio_movie_release_frame_snapshot(
        context: *mut c_void,
        host_pid_out: *mut i32,
        stack_address_out: *mut u32,
        stack_limit_out: *mut u32,
    ) -> bool;
}

/// `dsound_buffer_scope` (C layout, 72 bytes).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct CScope {
    descriptor_address: u32,
    format_address: u32,
    flags: u32,
    buffer_bytes: u32,
    sample_rate: u32,
    average_bytes_per_second: u32,
    channels: u16,
    block_align: u16,
    descriptor: [u8; 24],
    format: [u8; 20],
}

/// `dsound_buffer_snapshot` (C layout, 272 bytes); also the node the buffer list owns.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct CSnapshot {
    scope: CScope,
    lease: CLease,
    buffer_address: u32,
    header_address: u32,
    buffer_heap: u32,
    header: [u32; 9],
    publication_address: u32,
    i3dl2_address: u32,
    cache_mask: u32,
    i3dl2: [u32; 9],
    min_distance_bits: u32,
    curve_address: u32,
    curve_count: u32,
    data_address: u32,
    data_length: u32,
    data_sets: u32,
    loop_start: u32,
    loop_length: u32,
    loop_sets: u32,
    volume: i32,
    volume_sets: u32,
    pause_sets: u32,
    frequency: u32,
    frequency_sets: u32,
    max_distance_bits: u32,
    max_distance_sets: u32,
    min_distance_sets: u32,
    rolloff_sets: u32,
    position_bits: [u32; 3],
    position_sets: u32,
}

const _: () = assert!(std::mem::size_of::<CScope>() == 72);
const _: () = assert!(std::mem::size_of::<CSnapshot>() == 272);

impl CSnapshot {
    fn zeroed() -> Self {
        // SAFETY: plain integers and byte arrays; all-zero is a valid value.
        unsafe { std::mem::zeroed() }
    }
}

fn overlap(a: u32, an: u32, b: u32, bn: u32) -> bool {
    u64::from(a) < u64::from(b) + u64::from(bn) && u64::from(b) < u64::from(a) + u64::from(an)
}

fn read_u32(address: u32) -> Option<u32> {
    let mut word = 0u32;
    // SAFETY: checked guest read into a live local.
    unsafe { kernel_guest_read_u32(address, &mut word) }.then_some(word)
}

fn read_words<const N: usize>(address: u32) -> Option<[u32; N]> {
    let mut words = [0u32; N];
    // SAFETY: checked guest read into a live array of N words.
    unsafe { kernel_guest_read_bytes(address, words.as_mut_ptr().cast(), N * 4) }.then_some(words)
}

fn readable(address: u32, length: u32) -> bool {
    // SAFETY: probe only; the pointer is not dereferenced.
    !unsafe { kernel_guest_at_ro(address, length as usize) }.is_null()
}

fn log(text: String) {
    if let Ok(line) = CString::new(text) {
        // SAFETY: NUL-terminated local string for a synchronous call.
        unsafe { dsound_hle_log_line(line.as_ptr()) };
    }
}

#[derive(Default)]
struct Announced {
    create: bool,
    data: bool,
    volume: bool,
    pause: bool,
    frequency: bool,
    loop_region: bool,
    max_distance: bool,
    position: bool,
    apply: bool,
}

struct State {
    enabled: bool,
    completion: bool,
    started: Option<StartedFn>,
    pause: Option<PauseFn>,
    volume: Option<VolumeFn>,
    frequency: Option<FrequencyFn>,
    running: Option<RunningFn>,
    loop_hook: Option<LoopFn>,
    irql: Option<IrqlFn>,
    fatal: Option<FatalFn>,
    buffers: Vec<CSnapshot>,
    detached: Vec<CSnapshot>,
    rollback: Vec<CSnapshot>,
    announced: Announced,
    degraded: u64,
}

impl State {
    const fn new() -> Self {
        Self {
            enabled: false,
            completion: false,
            started: None,
            pause: None,
            volume: None,
            frequency: None,
            running: None,
            loop_hook: None,
            irql: None,
            fatal: None,
            buffers: Vec::new(),
            detached: Vec::new(),
            rollback: Vec::new(),
            announced: Announced {
                create: false,
                data: false,
                volume: false,
                pause: false,
                frequency: false,
                loop_region: false,
                max_distance: false,
                position: false,
                apply: false,
            },
            degraded: 0,
        }
    }
}

static STATE: Mutex<State> = Mutex::new(State::new());
static RESET_LOCK: Mutex<()> = Mutex::new(());

fn state() -> MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(|p| p.into_inner())
}

fn refuse(entry: u32, reason: &str) -> ! {
    let fatal = state().fatal;
    log(format!(
        "dsound passive buffer {entry:#x} refused: {reason}\n"
    ));
    if let (Some(fatal), Ok(reason)) = (fatal, CString::new(reason)) {
        // SAFETY: NUL-terminated string for a synchronous call.
        unsafe { fatal(entry, reason.as_ptr()) };
    }
    std::process::abort();
}

/// `degrade`: a counted, logged S_OK no-op unless `TSFP_DSOUND_STRICT` keeps the stop.
fn degrade(entry: u32, reason: &str) -> bool {
    let text = CString::new(reason).unwrap_or_default();
    // SAFETY: NUL-terminated strings for a synchronous call.
    let degraded = unsafe { dsound_hle_degrade(c"buffer".as_ptr(), entry, text.as_ptr()) };
    if degraded {
        state().degraded += 1;
    }
    degraded
}

/// `dsound_buffer_degraded_count`.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_buffer_degraded_count() -> std::ffi::c_ulong {
    state().degraded as std::ffi::c_ulong
}

/// `dsound_buffer_set_enabled`.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_buffer_set_enabled(enabled: bool) {
    state().enabled = enabled;
}

/// `dsound_buffer_set_completion`.
///
/// # Safety
/// `started` is null or `bool (*)(uint32_t, uint64_t)`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dsound_buffer_set_completion(enabled: bool, started: Erased) {
    let mut s = state();
    s.completion = enabled;
    // SAFETY: erased pointer restored to its declared C signature.
    s.started = started
        .filter(|_| enabled)
        .map(|f| unsafe { std::mem::transmute::<_, StartedFn>(f) });
    if !enabled {
        s.pause = None;
        s.frequency = None;
        s.volume = None;
        s.running = None;
        s.loop_hook = None;
    }
}

/// `dsound_buffer_set_completion_voice_running`.
///
/// # Safety
/// `running` is null or `bool (*)(uint32_t, uint64_t)`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dsound_buffer_set_completion_voice_running(running: Erased) {
    let mut s = state();
    // SAFETY: erased pointer restored to its declared C signature.
    s.running = running
        .filter(|_| s.completion)
        .map(|f| unsafe { std::mem::transmute::<_, RunningFn>(f) });
}

/// `dsound_buffer_set_completion_loop`.
///
/// # Safety
/// `hook` is null or the `dsound_buffer_loop_fn` signature.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dsound_buffer_set_completion_loop(hook: Erased) {
    let mut s = state();
    // SAFETY: erased pointer restored to its declared C signature.
    s.loop_hook = hook
        .filter(|_| s.completion)
        .map(|f| unsafe { std::mem::transmute::<_, LoopFn>(f) });
}

/// `dsound_buffer_set_completion_frequency`.
///
/// # Safety
/// `hook` is null or the `dsound_buffer_frequency_fn` signature.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dsound_buffer_set_completion_frequency(hook: Erased) {
    let mut s = state();
    // SAFETY: erased pointer restored to its declared C signature.
    s.frequency = hook
        .filter(|_| s.completion)
        .map(|f| unsafe { std::mem::transmute::<_, FrequencyFn>(f) });
}

/// `dsound_buffer_set_completion_pause`.
///
/// # Safety
/// `hook` is null or the `dsound_buffer_pause_fn` signature.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dsound_buffer_set_completion_pause(hook: Erased) {
    let mut s = state();
    // SAFETY: erased pointer restored to its declared C signature.
    s.pause = hook
        .filter(|_| s.completion)
        .map(|f| unsafe { std::mem::transmute::<_, PauseFn>(f) });
}

/// `dsound_buffer_set_completion_volume` (installed whatever the completion flag says, like the C).
///
/// # Safety
/// `hook` is null or the `dsound_buffer_volume_fn` signature.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dsound_buffer_set_completion_volume(hook: Erased) {
    // SAFETY: erased pointer restored to its declared C signature.
    state().volume = hook.map(|f| unsafe { std::mem::transmute::<_, VolumeFn>(f) });
}

/// `dsound_buffer_set_irql_provider`.
///
/// # Safety
/// `provider` is null or `bool (*)(uint8_t *)`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dsound_buffer_set_irql_provider(provider: Erased) {
    // SAFETY: erased pointer restored to its declared C signature.
    state().irql = provider.map(|f| unsafe { std::mem::transmute::<_, IrqlFn>(f) });
}

/// `dsound_buffer_set_fatal`.
///
/// # Safety
/// `fatal` is null or `void (*)(uint32_t, const char *)`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dsound_buffer_set_fatal(fatal: Erased) {
    // SAFETY: erased pointer restored to its declared C signature.
    state().fatal = fatal.map(|f| unsafe { std::mem::transmute::<_, FatalFn>(f) });
}

/// `dsound_buffer_stops_ready`: every compiled stop boundary the passive buffer relies on exists.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_buffer_stops_ready() -> bool {
    // SAFETY: scalar probe of the host's weak symbol.
    STOPPED
        .iter()
        // SAFETY: scalar probe of the host's weak symbol.
        .all(|&address| stop_boundary(address) == 1)
}

/// 1 when the compiled stop boundary exists, 0 or 2 for the other recorded states, -1 without the host symbol
/// (Rust has no stable weak linkage, so the host executable's `recomp_has_stop_boundary` is looked up once).
fn stop_boundary(address: u32) -> i32 {
    static PROBE: std::sync::OnceLock<Option<unsafe extern "C" fn(u32) -> i32>> =
        std::sync::OnceLock::new();
    let probe = PROBE.get_or_init(|| {
        // SAFETY: RTLD_DEFAULT (null) lookup of a NUL-terminated name; the symbol has this C signature when present.
        let found = unsafe { dlsym(std::ptr::null_mut(), c"recomp_has_stop_boundary".as_ptr()) };
        // SAFETY: a non-null result is the host's `int recomp_has_stop_boundary(uint32_t)`.
        (!found.is_null()).then(|| unsafe {
            std::mem::transmute::<*mut c_void, unsafe extern "C" fn(u32) -> i32>(found)
        })
    });
    // SAFETY: scalar probe of the host symbol.
    probe.map_or(-1, |probe| unsafe { probe(address) })
}

/// `dsound_buffer_scope_snapshot` (model `tsfp_audio::scope::buffer_scope_snapshot` over host guest memory).
fn scope_snapshot(descriptor: u32) -> Option<CScope> {
    let world = HostWorld;
    let desc = tsfp_audio::adapter::GuestMemory::read_bytes(&world, descriptor, 24)?;
    let scope = tsfp_audio::scope::buffer_scope_snapshot(&world, descriptor)?;
    Some(CScope {
        descriptor_address: scope.descriptor_address,
        format_address: scope.format_address,
        flags: scope.flags,
        buffer_bytes: scope.buffer_bytes,
        sample_rate: scope.sample_rate,
        average_bytes_per_second: scope.average_bytes_per_second,
        channels: scope.channels,
        block_align: scope.block_align,
        descriptor: desc.try_into().ok()?,
        format: scope.format,
    })
}

fn safe_tables(table: &[u32; 4]) -> bool {
    table.iter().all(|word| STOPPED.contains(word))
}

fn output_scope(flags: u32, output: u32) -> bool {
    let base = if flags == 16 {
        0x0058_35F0
    } else {
        0x0058_18E8
    };
    output >= base && output - base < 160 && (output - base) % 4 == 0
}

fn token_equal(a: &CLease, b: &CLease) -> bool {
    a.device_heap == b.device_heap
        && a.internal_address == b.internal_address
        && a.serial == b.serial
}

impl State {
    /// Called with the buffer lock held (`policy_error`).
    fn policy_error(&self) -> Option<&'static str> {
        if !self.enabled {
            return Some("explicit headless-buffers policy is disabled");
        }
        let irql = self.irql.and_then(|f| {
            let mut level = 0u8;
            // SAFETY: the hook writes one byte through the live local.
            unsafe { f(&mut level) }.then_some(level)
        });
        if irql != Some(0) {
            return Some("only known IRQL0 is supported");
        }
        if read_u32(GLOBAL_STATE) != Some(0) {
            return Some("original global audio state must be zero");
        }
        None
    }

    fn find(&mut self, address: u32) -> Option<&mut CSnapshot> {
        self.buffers
            .iter_mut()
            .find(|n| n.buffer_address == address)
    }

    fn tracked_headers_overlap(&self, address: u32, bytes: u32, with_publication: bool) -> bool {
        self.buffers.iter().any(|n| {
            overlap(address, bytes, n.header_address, BUFFER_BYTES)
                || (with_publication && overlap(address, bytes, n.publication_address, 4))
        }) || self
            .detached
            .iter()
            .any(|n| overlap(address, bytes, n.header_address, BUFFER_BYTES))
            || self.rollback.iter().any(|n| {
                n.header_address != 0 && overlap(address, bytes, n.header_address, BUFFER_BYTES)
            })
    }

    fn aliases_state(&self, address: u32, bytes: u32, internal: u32) -> bool {
        overlap(address, bytes, internal, 44)
            || overlap(address, bytes, SINGLETON, 4)
            || overlap(address, bytes, GLOBAL_STATE, 4)
            || overlap(address, bytes, VTABLE, 16)
            || overlap(address, bytes, CURVE, 4)
            || self.tracked_headers_overlap(address, bytes, true)
    }

    /// Input buffers are historical stack snapshots, not guest-owned state.
    fn aliases_headers(&self, address: u32, bytes: u32, internal: u32) -> bool {
        overlap(address, bytes, internal, 44)
            || overlap(address, bytes, SINGLETON, 4)
            || overlap(address, bytes, GLOBAL_STATE, 4)
            || overlap(address, bytes, VTABLE, 16)
            || self.tracked_headers_overlap(address, bytes, true)
    }
}

fn valid_node(node: &CSnapshot, identity: &CLease, internal: u32) -> bool {
    let world = HostWorld;
    token_equal(&node.lease, identity)
        && identity.internal_address == internal
        && world.heap_valid(identity.device_heap)
        && read_words::<9>(node.header_address).is_some_and(|actual| actual == node.header)
        && world.heap_valid(node.buffer_heap)
        && world.heap_block_size(node.buffer_heap, node.header_address) == Some(BUFFER_BYTES)
}

struct CreateRequest {
    scope: CScope,
    output: u32,
    status: u32,
    node: Option<Box<CSnapshot>>,
    error: Option<&'static str>,
    guard: Option<MutexGuard<'static, State>>,
    announce: bool,
}

unsafe extern "C" fn create_prepare(
    candidate: *const CLease,
    userdata: *mut c_void,
    child: *mut CChild,
) -> bool {
    // SAFETY: the lease transaction passes the live request, token and child slot.
    let (r, candidate) = unsafe { (&mut *userdata.cast::<CreateRequest>(), &*candidate) };
    let s = r.guard.insert(state());
    let internal = candidate.internal_address;
    r.error = s.policy_error();
    if r.error.is_some() {
        return false;
    }
    if !dsound_buffer_stops_ready() {
        r.error = Some("compiled buffer stop boundaries missing");
        return false;
    }
    if s.aliases_headers(r.scope.descriptor_address, 24, internal)
        || s.aliases_headers(r.scope.format_address, 20, internal)
    {
        r.error = Some("input aliases owned state");
        return false;
    }
    // SAFETY: scalar pid query.
    let pid = unsafe { kernel_host_pid() };
    // SAFETY: the Rust bridge copies through the fault-safe process_vm boundary.
    let Some(table) = read_words::<4>(VTABLE)
        .filter(|_| unsafe { tsfp_audio_buffer_output_probe_rust(pid, r.output) })
    else {
        r.error = Some("output/original vtables unreadable");
        return false;
    };
    if !safe_tables(&table) {
        r.error = Some("original vtable contains an unprotected target");
        return false;
    }
    if s.aliases_state(r.output, 4, internal)
        || overlap(r.output, 4, r.scope.descriptor_address, 24)
        || overlap(r.output, 4, r.scope.format_address, 20)
        || overlap(r.scope.descriptor_address, 24, internal, 44)
        || overlap(r.scope.format_address, 20, internal, 44)
    {
        r.error = Some("unproven output/input alias with owned state");
        return false;
    }
    let mut node = Box::new(CSnapshot::zeroed());
    node.scope = r.scope;
    node.publication_address = r.output;
    let mut world = HostWorld;
    let heap = world.heap_create();
    node.buffer_heap = heap;
    r.node = Some(node);
    let node = r.node.as_mut().unwrap();
    if heap == 0 {
        r.status = OOM;
        return false;
    }
    let address = world.heap_alloc(heap, BUFFER_BYTES);
    node.header_address = address;
    node.buffer_address = address.wrapping_add(28);
    if address == 0 {
        r.status = OOM;
        return false;
    }
    node.header[0] = VTABLE;
    node.header[1] = 1;
    node.header[2] = internal;
    if s.aliases_state(address, BUFFER_BYTES, internal)
        || overlap(address, BUFFER_BYTES, r.output, 4)
        || overlap(address, BUFFER_BYTES, r.scope.descriptor_address, 24)
        || overlap(address, BUFFER_BYTES, r.scope.format_address, 20)
    {
        r.error = Some("new private buffer allocation/alias is invalid");
        return false;
    }
    // SAFETY: the Rust bridge writes the 36-byte header into the fresh private block.
    if !unsafe {
        tsfp_audio_buffer_header_write_rust(
            pid,
            address,
            node.header[0],
            node.header[1],
            node.header[2],
        )
    } {
        r.error = Some("Rust guest buffer-header write refused");
        return false;
    }
    // SAFETY: caller-provided child slot.
    unsafe {
        child.write(CChild {
            heap,
            address,
            bytes: BUFFER_BYTES,
        })
    };
    true
}

unsafe extern "C" fn create_abort(userdata: *mut c_void) {
    // SAFETY: the lease transaction passes the live request.
    let r = unsafe { &mut *userdata.cast::<CreateRequest>() };
    // Cleanup is fallible and happens after the device transaction unlocks.
    r.guard = None;
}

unsafe extern "C" fn create_finalize(committed: *const CLease, userdata: *mut c_void) {
    // SAFETY: the lease transaction passes the live request and committed token.
    let (r, committed) = unsafe { (&mut *userdata.cast::<CreateRequest>(), &*committed) };
    let mut s = r.guard.take().expect("prepare ran");
    // The committed token is stored before the sidecar or guest pointer becomes visible.
    let mut node = *r.node.take().expect("prepare ran");
    node.lease = *committed;
    r.announce = !s.announced.create;
    s.announced.create = true;
    // SAFETY: scalar pid query.
    let pid = unsafe { kernel_host_pid() };
    // SAFETY: the Rust bridge writes the published pointer into the guest output word.
    if !unsafe { tsfp_audio_buffer_pointer_publish_rust(pid, r.output, node.header_address) } {
        log("dsound: Rust guest buffer pointer publication failed after lease commit\n".into());
        std::process::abort();
    }
    s.buffers.insert(0, node);
}

fn create_from_scope(interface: u32, scope: &CScope, output: u32, outer: u32) -> u32 {
    let error = state().policy_error();
    if let Some(error) = error {
        refuse(CREATE, error);
    }
    if outer != 0 {
        refuse(CREATE, "aggregation is unsupported");
    }
    let device = read_u32(SINGLETON)
        .filter(|&d| d != 0 && d <= u32::MAX - 8)
        .unwrap_or_else(|| refuse(CREATE, "no owned SILENT device"));
    if interface != device + 8 || !output_scope(scope.flags, output) {
        refuse(CREATE, "device/output outside measured startup scope");
    }
    let mut request = CreateRequest {
        scope: *scope,
        output,
        status: 0,
        node: None,
        error: None,
        guard: None,
        announce: false,
    };
    let ops = CLeaseOps {
        prepare: Some(create_prepare),
        abort: Some(create_abort),
        finalize: Some(create_finalize),
    };
    let mut lease = CLease::default();
    // SAFETY: the request, ops and output outlive the synchronous call.
    let status = unsafe {
        tsfp_audio_device_acquire_lease(
            device + 8,
            &ops,
            (&mut request as *mut CreateRequest).cast(),
            &mut lease,
        )
    };
    if status != LEASE_OK
        && let Some(node) = request.node.take()
    {
        let mut s = state();
        let heap = node.buffer_heap;
        if heap != 0 && !HostWorld.heap_destroy(heap) {
            s.rollback.insert(0, *node);
            request.error = Some("private allocation rollback needs retry");
        }
    }
    if status == LEASE_OUT_OF_MEMORY
        || (status == LEASE_PREPARE_FAILED && request.status == OOM && request.error.is_none())
    {
        return OOM;
    }
    if status != LEASE_OK {
        refuse(
            CREATE,
            request.error.unwrap_or("device lease transaction refused"),
        );
    }
    if request.announce {
        log("dsound: explicit HEADLESS buffers; passive36-byte public headers and host startup setter caches; omitted settings/hardware/list/packet objects, APU/DSP/FP/critical-section operations; no audio output or packet completion\n".into());
    }
    0
}

/// `dsound_buffer_create`.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_buffer_create(
    interface: u32,
    descriptor: u32,
    output: u32,
    outer: u32,
) -> u32 {
    let Some(scope) = scope_snapshot(descriptor) else {
        refuse(CREATE, "descriptor/format outside measured startup scope");
    };
    create_from_scope(interface, &scope, output, outer)
}

struct AccessRequest {
    address: u32,
    entry: u32,
    argument: u32,
    apply: u32,
    count: u32,
    position_z: u32,
    identity: CLease,
    snapshot: Option<CSnapshot>,
    error: Option<String>,
    observer: bool,
    announce: bool,
}

impl State {
    /// `access_owned`: runs under the device lock with the owned internal address.
    fn access_owned(&mut self, internal: u32, r: &mut AccessRequest) -> u32 {
        let mut result = 0u32;
        let completion = self.completion;
        let (started, pause_hook, volume_hook, frequency_hook, running_hook, loop_hook) = (
            self.started,
            self.pause,
            self.volume,
            self.frequency,
            self.running,
            self.loop_hook,
        );
        let policy_error = if r.observer {
            None
        } else {
            self.policy_error()
        };
        let aliases_data = r.entry == SET_DATA && self.aliases_state(r.argument, r.count, internal);
        let aliases_i3dl2 = r.entry == I3DL2 && self.aliases_headers(r.argument, 36, internal);
        let announced = &mut self.announced;
        let Some(n) = self
            .buffers
            .iter_mut()
            .find(|n| n.buffer_address == r.address)
        else {
            r.error = Some("buffer ownership/header/generation changed".into());
            return result;
        };
        if !valid_node(n, &r.identity, internal) {
            r.error = Some("buffer ownership/header/generation changed".into());
            return result;
        }
        if r.observer {
            r.snapshot = Some(*n);
            return 0;
        }
        if let Some(error) = policy_error {
            r.error = Some(error.into());
            return result;
        }
        let serial = n.lease.serial;
        macro_rules! fail {
            ($text:expr) => {{
                r.error = Some($text.into());
                return result;
            }};
        }
        if r.entry == SET_LOOP_REGION {
            let shape_ok = n.scope.flags == 0 || (n.scope.flags == 0x10 && n.cache_mask == 7);
            if !completion || !shape_ok || n.data_sets == 0 {
                fail!(if !completion {
                    "loop regions need passive completion and ordinary buffer data (passive completion is off)"
                } else if !shape_ok {
                    "loop regions need passive completion and ordinary buffer data (a flags 0x10 buffer without its three 3D caches, or another flag)"
                } else {
                    "loop regions need passive completion and ordinary buffer data (SetBufferData has not been recorded)"
                });
            }
            // SAFETY: completion hooks are the host's C functions; they must not reenter buffer APIs.
            let running = match started {
                None => true,
                Some(started) => {
                    let live = unsafe { started(r.address, serial) };
                    live && running_hook.is_none_or(|hook| unsafe { hook(r.address, serial) })
                }
            };
            if running && loop_hook.is_none() {
                fail!(
                    "loop region changes while the voice is running (after Play, before Stop or the end of its data) are not supported"
                );
            }
            let end = u64::from(r.argument) + u64::from(r.count);
            if end > u64::from(u32::MAX) || r.argument > n.data_length {
                fail!("wrapped or out-of-range loop start is not supported");
            }
            if r.count != 0 && end > u64::from(n.data_length) {
                return PACKET_LIST_FULL;
            }
            let length = if r.count != 0 {
                r.count
            } else {
                n.data_length - r.argument
            };
            if length == 0 || r.argument % 36 != 0 || length % 36 != 0 {
                fail!("only nonempty 36-byte ADPCM block loop regions are supported");
            }
            if running
                // SAFETY: completion hook, see above.
                && !unsafe { loop_hook.unwrap()(r.address, serial, r.argument, length, n.frequency) }
            {
                fail!(
                    "the completion model refused the live loop region of the running voice (empty at the clock resolution, or the mixer refused it)"
                );
            }
            n.loop_start = r.argument;
            n.loop_length = length;
            n.loop_sets += 1;
            r.announce = !announced.loop_region;
            announced.loop_region = true;
            return 0;
        }
        if r.entry == SET_VOLUME {
            if n.data_sets == 0 {
                fail!("SetVolume before SetBufferData is not the measured sound start order");
            }
            if !completion {
                if r.argument != VOLUME_STARTUP && r.argument != VOLUME_UPDATE {
                    fail!(
                        "only the measured volumes -10000 (start) and -3204 (update) are supported"
                    );
                }
                if r.argument != VOLUME_STARTUP && (n.frequency_sets == 0 || n.scope.flags != 0) {
                    fail!(
                        "the update volume -3204 needs a started ordinary buffer (SetBufferData, SetVolume, Pause and SetFrequency recorded)"
                    );
                }
            }
            if completion
                && let Some(hook) = volume_hook
                // SAFETY: completion hook, see above.
                && !unsafe { hook(r.address, serial, r.argument as i32) }
            {
                fail!("real buffer PCM rejected volume or its timestamp/history");
            }
            n.volume = r.argument as i32;
            n.volume_sets += 1;
            r.announce = !announced.volume;
            announced.volume = true;
            return 0;
        }
        if r.entry == PAUSE {
            if n.volume_sets == 0 {
                fail!(
                    "Pause before SetBufferData and SetVolume is not the measured sound start order"
                );
            }
            if let Some(started) = started
                // SAFETY: completion hooks, see above.
                && unsafe { started(r.address, serial) }
                && let Some(hook) = pause_hook
                && unsafe { hook(r.address, serial, r.argument) }
            {
                n.pause_sets += 1;
                r.announce = !announced.pause;
                announced.pause = true;
                return 0;
            }
            if r.argument > 2 {
                fail!("only Pause modes 0, 1 and 2 are modelled");
            }
            n.pause_sets += 1;
            r.announce = !announced.pause;
            announced.pause = true;
            return r.argument;
        }
        if r.entry == SET_FREQUENCY {
            if n.pause_sets == 0 {
                fail!(
                    "SetFrequency before SetBufferData, SetVolume and Pause is not the measured sound start order"
                );
            }
            if r.argument != FREQUENCY_STARTUP
                && !(completion && (FREQUENCY_MIN..=FREQUENCY_MAX).contains(&r.argument))
            {
                fail!(
                    "only the measured startup frequency 22042 is supported (the completion model takes the title's clamp 0xBC..0x2EDEF)"
                );
            }
            if let Some(started) = started
                // SAFETY: completion hooks, see above.
                && unsafe { started(r.address, serial) }
                && !frequency_hook
                    .is_some_and(|hook| unsafe { hook(r.address, serial, n.frequency, r.argument) })
            {
                fail!(
                    "SetFrequency on a played buffer the completion model does not know is refused"
                );
            }
            n.frequency = r.argument;
            n.frequency_sets += 1;
            r.announce = !announced.frequency;
            announced.frequency = true;
            return 0;
        }
        if r.entry == SET_DATA {
            let spatial = n.scope.flags == 0x10 && n.cache_mask == 7;
            let ordinary = n.scope.flags == 0 && n.cache_mask == 0;
            if !spatial && !ordinary {
                fail!(
                    "only fully configured spatial or fresh ordinary startup buffers accept SetBufferData"
                );
            }
            if r.argument == 0 || r.count == 0 {
                fail!(
                    "only non-null data with non-zero length is supported (the original allocates or clears otherwise)"
                );
            }
            if r.count > DATA_MAX_BYTES || !readable(r.argument, r.count) {
                fail!("buffer data range is longer than 64 MiB or not readable guest memory");
            }
            if aliases_data {
                fail!("buffer data aliases owned state");
            }
            if n.data_address != r.argument || n.data_length != r.count {
                n.loop_start = 0;
                n.loop_length = r.count;
            }
            n.data_address = r.argument;
            n.data_length = r.count;
            n.data_sets += 1;
            r.announce = !announced.data;
            announced.data = true;
            return 0;
        }
        if r.entry == MAX_DISTANCE {
            if n.scope.flags != 0x10 || n.cache_mask != 7 || n.data_sets == 0 || r.apply != 1 {
                fail!(
                    "only the measured apply=1 update on a configured spatial buffer with recorded data is supported"
                );
            }
            n.max_distance_bits = r.argument;
            n.max_distance_sets += 1;
            r.announce = !announced.max_distance;
            announced.max_distance = true;
            return 0;
        }
        if r.entry == SET_POSITION {
            if n.scope.flags != 0x10 || n.cache_mask != 7 || n.data_sets == 0 {
                fail!("only a configured spatial buffer with recorded data is supported");
            }
            if r.apply != 1 {
                fail!("SetPosition apply=0 enters the unmodeled settings/APU commit");
            }
            n.position_bits = [r.argument, r.count, r.position_z];
            n.position_sets += 1;
            r.announce = !announced.position;
            announced.position = true;
            return 0;
        }
        if (r.entry == MINIMUM || r.entry == ROLLOFF) && r.apply == 1 {
            if n.scope.flags != 0x10 || n.cache_mask != 7 || n.data_sets == 0 {
                fail!(
                    "only the measured apply=1 update on a configured spatial buffer with recorded data is supported"
                );
            }
            if r.entry == MINIMUM {
                n.min_distance_bits = r.argument;
                n.min_distance_sets += 1;
            } else {
                n.curve_address = r.argument;
                n.curve_count = r.count;
                n.rolloff_sets += 1;
            }
            r.announce = !announced.apply;
            announced.apply = true;
            return 0;
        }
        if n.scope.flags != 0x10 || r.apply != 0 {
            fail!("only startup spatial setters/apply0 are supported");
        }
        if r.entry == I3DL2 {
            let expected = [0u32, 0, 0xFFFF_F448, 0, 0, 0, 0, 0, 0];
            let words = read_words::<9>(r.argument);
            if n.cache_mask != 0 || aliases_i3dl2 || words != Some(expected) {
                fail!("only exact first startup I3DL2 cache is supported");
            }
            n.i3dl2 = expected;
            n.i3dl2_address = r.argument;
            n.cache_mask = 1;
        } else if r.entry == MINIMUM {
            if n.cache_mask != 1 || r.argument != 0x3F80_0000 {
                fail!("only ordered startup minDistance1.0 is supported");
            }
            n.min_distance_bits = r.argument;
            n.cache_mask = 3;
        } else {
            if n.cache_mask != 3 || r.argument != CURVE || r.count != 1 {
                fail!("only ordered startup curve VA/count1 is supported");
            }
            n.curve_address = r.argument;
            n.curve_count = r.count;
            n.cache_mask = 7;
        }
        0
    }
}

unsafe extern "C" fn access_callback(
    internal: *const u32,
    userdata: *mut c_void,
    result: *mut u32,
) {
    // SAFETY: the device passes the owned internal address under its mutex; userdata is the live request.
    let (r, internal) = unsafe { (&mut *userdata.cast::<AccessRequest>(), *internal) };
    let answer = state().access_owned(internal, r);
    // SAFETY: writable result slot by contract.
    unsafe { *result = answer };
}

fn identify(address: u32) -> Option<CLease> {
    state().find(address).map(|n| n.lease)
}

/// Runs `access_owned` through the device's owned-interface lock; false when the device refused.
fn run_access(r: &mut AccessRequest) -> (bool, u32) {
    let mut result = 0u32;
    // SAFETY: the request and result outlive the synchronous call.
    let ran = unsafe {
        tsfp_audio_device_with_owned_interface(
            r.identity.internal_address + 8,
            Some(access_callback),
            (r as *mut AccessRequest).cast(),
            &mut result,
        )
    };
    (ran, result)
}

/// Original global error precedes buffer/settings access for these setters.
fn setter_preflight(entry: u32) -> u32 {
    let (admitted, irql) = {
        let s = state();
        let admitted = s.enabled && (entry != SET_LOOP_REGION || s.completion);
        let irql = s.irql.and_then(|f| {
            let mut level = 0u8;
            // SAFETY: the hook writes one byte through the live local.
            unsafe { f(&mut level) }.then_some(level)
        });
        (admitted, irql)
    };
    if !admitted || irql != Some(0) {
        refuse(entry, "buffer setter needs explicit policy at known IRQL0");
    }
    let Some(global) = read_u32(GLOBAL_STATE) else {
        refuse(entry, "global audio state inaccessible");
    };
    if global != 0 { E_FAIL } else { 0 }
}

fn access_extended(
    entry: u32,
    address: u32,
    argument: u32,
    count: u32,
    apply: u32,
    position_z: u32,
) -> u32 {
    if entry == SET_POSITION || entry == SET_LOOP_REGION || entry == MAX_DISTANCE {
        let failure = setter_preflight(entry);
        if failure != 0 {
            return failure;
        }
    }
    let Some(identity) = identify(address) else {
        refuse(entry, "buffer is not owned by passive adapter");
    };
    let mut r = AccessRequest {
        address,
        entry,
        argument,
        apply,
        count,
        position_z,
        identity,
        snapshot: None,
        error: None,
        observer: false,
        announce: false,
    };
    let (ran, result) = run_access(&mut r);
    if !ran {
        refuse(entry, "parent SILENT device ownership/generation changed");
    }
    if let Some(error) = r.error.take() {
        let text =
            format!("{error} (buffer {address:#x}, argument {argument:#x}, count {count:#x})");
        if std::env::var_os("TSFP_DSOUND_STRICT").is_none() && degrade(entry, &text) {
            return 0;
        }
        refuse(entry, &text);
    }
    if r.announce {
        if let Some(line) = announce_text(entry, apply) {
            log(line.into());
        }
    }
    result
}

fn announce_text(entry: u32, apply: u32) -> Option<&'static str> {
    Some(match entry {
        SET_LOOP_REGION => {
            "dsound: explicit passive SetLoopRegion; original byte region recorded on host; virtual loop cursor and Stop drain INFERRED, no guest settings/APU write or buffer audio output\n"
        }
        SET_DATA => {
            "dsound: explicit HEADLESS SetBufferData; the passive buffer records the data pointer and length on the host only (no guest word, settings object, page lock, APU write or critical section), S_OK, no playback\n"
        }
        SET_FREQUENCY => {
            "dsound: explicit HEADLESS SetFrequency; the passive buffer records the startup frequency 22042 on the host only (no guest word, settings pitch, APU pitch register or critical section), S_OK, no playback\n"
        }
        PAUSE => {
            "dsound: explicit HEADLESS buffer Pause(0); a buffer nothing started has no state to change, the passive buffer counts the call on the host only (no guest word, APU write or critical section), S_OK, no playback\n"
        }
        SET_VOLUME => {
            "dsound: explicit HEADLESS SetVolume; the passive buffer records the startup volume -10000 and the sound update volume -3204 on the host only (no guest word, settings object, APU voice write or critical section), S_OK, no playback\n"
        }
        MINIMUM | ROLLOFF if apply == 1 => {
            "dsound: explicit HEADLESS apply=1 SetMinDistance/SetRolloffCurve; the sound start's raw value (or curve pointer and count) is recorded on the host only (no guest settings words, commit helper or APU write), S_OK, no playback\n"
        }
        MAX_DISTANCE => {
            "dsound: explicit HEADLESS SetMaxDistance; the measured apply=1 caller's raw float bits are recorded on the host only (no guest settings word, commit, APU write or spatial playback)\n"
        }
        SET_POSITION => {
            "dsound: explicit HEADLESS SetPosition; measured apply=1 title callers' raw xyz bits are recorded on the host only (no guest settings words, commit, APU write or spatial playback)\n"
        }
        _ => return None,
    })
}

fn access(entry: u32, address: u32, argument: u32, count: u32, apply: u32) -> u32 {
    access_extended(entry, address, argument, count, apply, 0)
}

/// `dsound_buffer_cache_i3dl2`.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_buffer_cache_i3dl2(buffer: u32, parameters: u32, apply: u32) -> u32 {
    access(I3DL2, buffer, parameters, 0, apply)
}

/// `dsound_buffer_cache_min_distance`.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_buffer_cache_min_distance(buffer: u32, bits: u32, apply: u32) -> u32 {
    access(MINIMUM, buffer, bits, 0, apply)
}

/// `dsound_buffer_cache_rolloff`.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_buffer_cache_rolloff(
    buffer: u32,
    curve: u32,
    count: u32,
    apply: u32,
) -> u32 {
    access(ROLLOFF, buffer, curve, count, apply)
}

/// `dsound_buffer_cache_max_distance`.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_buffer_cache_max_distance(buffer: u32, bits: u32, apply: u32) -> u32 {
    access(MAX_DISTANCE, buffer, bits, 0, apply)
}

/// `dsound_buffer_set_position`.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_buffer_set_position(
    buffer: u32,
    x_bits: u32,
    y_bits: u32,
    z_bits: u32,
    apply: u32,
) -> u32 {
    access_extended(SET_POSITION, buffer, x_bits, y_bits, apply, z_bits)
}

/// `dsound_buffer_set_data`.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_buffer_set_data(buffer: u32, data: u32, length: u32) -> u32 {
    access(SET_DATA, buffer, data, length, 0)
}

/// `dsound_buffer_set_loop_region`.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_buffer_set_loop_region(buffer: u32, start: u32, length: u32) -> u32 {
    access(SET_LOOP_REGION, buffer, start, length, 0)
}

/// `dsound_buffer_set_volume`.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_buffer_set_volume(buffer: u32, volume: i32) -> u32 {
    access(SET_VOLUME, buffer, volume as u32, 0, 0)
}

/// `dsound_buffer_pause`.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_buffer_pause(buffer: u32, mode: u32) -> u32 {
    access(PAUSE, buffer, mode, 0, 0)
}

/// `dsound_buffer_set_frequency`.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_buffer_set_frequency(buffer: u32, hertz: u32) -> u32 {
    access(SET_FREQUENCY, buffer, hertz, 0, 0)
}

/// `dsound_buffer_get_snapshot`.
///
/// # Safety
/// `output` is null or a writable `dsound_buffer_snapshot`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dsound_buffer_get_snapshot(buffer: u32, output: *mut CSnapshot) -> bool {
    if output.is_null() {
        return false;
    }
    let Some(identity) = identify(buffer) else {
        return false;
    };
    let mut r = AccessRequest {
        address: buffer,
        entry: 0,
        argument: 0,
        apply: 0,
        count: 0,
        position_z: 0,
        identity,
        snapshot: None,
        error: None,
        observer: true,
        announce: false,
    };
    let (ran, _) = run_access(&mut r);
    match r.snapshot.filter(|_| ran && r.error.is_none()) {
        Some(value) => {
            // SAFETY: non-null writable output by contract.
            unsafe { output.write(value) };
            true
        }
        None => false,
    }
}

struct ResetRequest {
    address: u32,
    identity: CLease,
    node: Option<CSnapshot>,
    guard: Option<MutexGuard<'static, State>>,
}

unsafe extern "C" fn reset_prepare(
    candidate: *const CLease,
    userdata: *mut c_void,
    child: *mut CChild,
) -> bool {
    // SAFETY: the lease transaction passes the live request, token and child slot.
    let (r, candidate) = unsafe { (&mut *userdata.cast::<ResetRequest>(), &*candidate) };
    let s = r.guard.insert(state());
    r.node = s.find(r.address).map(|n| *n);
    let Some(node) = r.node.as_ref() else {
        return false;
    };
    if !token_equal(candidate, &r.identity)
        || !valid_node(node, &r.identity, candidate.internal_address)
    {
        return false;
    }
    // SAFETY: caller-provided child slot.
    unsafe {
        child.write(CChild {
            heap: node.buffer_heap,
            address: node.header_address,
            bytes: BUFFER_BYTES,
        })
    };
    true
}

unsafe extern "C" fn reset_abort(userdata: *mut c_void) {
    // SAFETY: the lease transaction passes the live request.
    unsafe { &mut *userdata.cast::<ResetRequest>() }.guard = None;
}

unsafe extern "C" fn reset_finalize(_committed: *const CLease, userdata: *mut c_void) {
    // SAFETY: the lease transaction passes the live request.
    let r = unsafe { &mut *userdata.cast::<ResetRequest>() };
    let mut s = r.guard.take().expect("prepare ran");
    if let Some(index) = s.buffers.iter().position(|n| n.buffer_address == r.address) {
        let node = s.buffers.remove(index);
        s.detached.insert(0, node);
    }
}

fn reset_buffers() -> bool {
    let world = HostWorld;
    let mut destroy = HostWorld;
    loop {
        let (address, identity) = {
            let s = state();
            match s.buffers.first() {
                Some(n) => (n.buffer_address, n.lease),
                None => break,
            }
        };
        let mut request = ResetRequest {
            address,
            identity,
            node: None,
            guard: None,
        };
        let ops = CLeaseOps {
            prepare: Some(reset_prepare),
            abort: Some(reset_abort),
            finalize: Some(reset_finalize),
        };
        // SAFETY: the request, lease and ops outlive the synchronous call.
        let status = unsafe {
            tsfp_audio_device_release_lease(
                &identity,
                &ops,
                (&mut request as *mut ResetRequest).cast(),
            )
        };
        if status != LEASE_OK {
            return false;
        }
        // Detached nodes remain tracked if checked cleanup refuses; no fallible cleanup runs under the device lock.
        let mut s = state();
        let Some(index) = s.detached.iter().position(|n| n.buffer_address == address) else {
            return false;
        };
        let node = s.detached[index];
        if !valid_node(&node, &identity, identity.internal_address)
            || !destroy.heap_destroy(node.buffer_heap)
        {
            return false;
        }
        s.detached.remove(index);
    }
    let mut s = state();
    while let Some(node) = s.detached.first().copied() {
        if !valid_node(&node, &node.lease, node.lease.internal_address)
            || !destroy.heap_destroy(node.buffer_heap)
        {
            return false;
        }
        s.detached.remove(0);
    }
    while let Some(node) = s.rollback.first().copied() {
        if !world.heap_valid(node.buffer_heap) || !destroy.heap_destroy(node.buffer_heap) {
            return false;
        }
        s.rollback.remove(0);
    }
    s.announced = Announced::default();
    true
}

/// `dsound_buffer_reset_checked`.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_buffer_reset_checked() -> bool {
    let _serial = RESET_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    reset_buffers()
}

/// `dsound_buffer_reset`.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_buffer_reset() {
    let _ = dsound_buffer_reset_checked();
}

fn call_handler(frame: *mut c_void, entry: u32) -> u32 {
    let (mut pid, mut stack, mut limit) = (0i32, 0u32, 0u32);
    // SAFETY: the registry passes the live call frame; the outputs are live locals.
    let have =
        unsafe { tsfp_audio_movie_release_frame_snapshot(frame, &mut pid, &mut stack, &mut limit) };
    let completion = state().completion;
    let mut decoded = CBufferCall {
        entry: 0,
        caller: 0,
        args: [0; 5],
    };
    // SAFETY: scalar pid query and a live out slot.
    let ok = have
        && unsafe {
            tsfp_audio_buffer_frame_decode(
                entry,
                stack,
                limit,
                completion,
                kernel_host_pid(),
                &mut decoded,
            )
        };
    if !ok {
        refuse(entry, "Rust buffer-call decoder refused frame");
    }
    let actual = decoded.caller;
    let args = decoded.args;
    match entry {
        CREATE => {
            let Some(scope) = scope_snapshot(args[1]) else {
                refuse(entry, "caller/descriptor class mismatch");
            };
            if scope.flags != if actual == 0x27AA9 { 16 } else { 0 } {
                refuse(entry, "caller/descriptor class mismatch");
            }
            create_from_scope(args[0], &scope, args[2], args[3])
        }
        I3DL2 => dsound_buffer_cache_i3dl2(args[0], args[1], args[2]),
        MINIMUM => dsound_buffer_cache_min_distance(args[0], args[1], args[2]),
        SET_LOOP_REGION => dsound_buffer_set_loop_region(args[0], args[1], args[2]),
        SET_DATA => dsound_buffer_set_data(args[0], args[1], args[2]),
        SET_VOLUME => access(SET_VOLUME, args[0], args[1], 0, 0),
        PAUSE => dsound_buffer_pause(args[0], args[1]),
        SET_FREQUENCY => dsound_buffer_set_frequency(args[0], args[1]),
        MAX_DISTANCE => dsound_buffer_cache_max_distance(args[0], args[1], args[2]),
        SET_POSITION => dsound_buffer_set_position(args[0], args[1], args[2], args[3], args[4]),
        _ => dsound_buffer_cache_rolloff(args[0], args[1], args[2], args[3]),
    }
}

macro_rules! handler {
    ($name:ident, $entry:expr) => {
        unsafe extern "C" fn $name(frame: *mut c_void) -> u32 {
            call_handler(frame, $entry)
        }
    };
}
handler!(create_handler, CREATE);
handler!(i3dl2_handler, I3DL2);
handler!(minimum_handler, MINIMUM);
handler!(rolloff_handler, ROLLOFF);
handler!(set_data_handler, SET_DATA);
handler!(set_volume_handler, SET_VOLUME);
handler!(pause_handler, PAUSE);
handler!(frequency_handler, SET_FREQUENCY);
handler!(max_distance_handler, MAX_DISTANCE);
handler!(position_handler, SET_POSITION);
handler!(loop_region_handler, SET_LOOP_REGION);

/// `dsound_buffer_register`: the number of entries registered (the policy decides which).
#[unsafe(no_mangle)]
pub extern "C" fn dsound_buffer_register() -> usize {
    let (policy, loops) = {
        let s = state();
        (s.enabled, s.enabled && s.completion)
    };
    let mut entries: Vec<(u32, DsoundHandler)> = vec![(CREATE, create_handler)];
    // Only with the policy on, so a flags-off boot keeps its exact registry (T597, T601).
    if loops {
        entries.push((SET_LOOP_REGION, loop_region_handler));
    }
    if policy {
        entries.extend([
            (SET_DATA, set_data_handler as DsoundHandler),
            (SET_VOLUME, set_volume_handler),
            (PAUSE, pause_handler),
            (SET_FREQUENCY, frequency_handler),
            (MAX_DISTANCE, max_distance_handler),
            (SET_POSITION, position_handler),
        ]);
    }
    entries.extend([
        (I3DL2, i3dl2_handler as DsoundHandler),
        (MINIMUM, minimum_handler),
        (ROLLOFF, rolloff_handler),
    ]);
    entries
        .into_iter()
        .filter(|&(address, handler)| {
            tsfp_audio_hle_register_handler_rust(address, handler)
                // SAFETY: the C compatibility table is initialised before subsystem handlers register.
                && unsafe { dsound_hle_register_rust(address) }
        })
        .count()
}
