// SPDX-License-Identifier: GPL-3.0-or-later
//! T3933: Rust-owned passive DirectSound listener front-end for the Rust audio host. Replaces
//! `src/audio/dsound_listener.c` (kept as the `TSFP_RUST_AUDIO_HOST=0` fallback) under the same C symbols; the state
//! machine is `tsfp_audio::listener`. Frame/caller admission, the work route and the degrade/refuse paths mirror the C.

use std::ffi::{CString, c_char, c_void};
use std::sync::Mutex;

use tsfp_audio::adapter::Refusal;
use tsfp_audio::listener::{
    self, DeviceIdentity, ListenerCache, ListenerCall, ListenerEnv, ListenerSnapshot,
};

use crate::device_ffi::tsfp_audio_device_with_owned_identity;
use crate::movie_device_hle_ffi::tsfp_audio_hle_register_handler_rust;

type IrqlFn = unsafe extern "C" fn(*mut u8) -> bool;
type FatalFn = unsafe extern "C" fn(u32, *const c_char);
type RouteFn = unsafe extern "C" fn(u32) -> bool;
type Erased = Option<unsafe extern "C" fn()>;
type DsoundHandler = unsafe extern "C" fn(*mut c_void) -> u32;

unsafe extern "C" {
    fn dsound_hle_degrade(kind: *const c_char, entry: u32, reason: *const c_char) -> bool;
    fn dsound_hle_register_rust(address: u32) -> bool;
    fn dsound_hle_log_line(line: *const c_char);
    fn kernel_guest_read_u32(address: u32, out: *mut u32) -> bool;
    fn kernel_frame_arg(frame: *const c_void, index: u32, out: *mut u32) -> bool;
    fn tsfp_audio_movie_release_frame_snapshot(
        context: *mut c_void,
        host_pid_out: *mut i32,
        stack_address_out: *mut u32,
        stack_limit_out: *mut u32,
    ) -> bool;
}

#[derive(Default)]
struct State {
    cache: ListenerCache,
    enabled: bool,
    irql: Option<IrqlFn>,
    fatal: Option<FatalFn>,
    work_route: Option<RouteFn>,
}

static STATE: Mutex<State> = Mutex::new(State {
    cache: ListenerCache::new(),
    enabled: false,
    irql: None,
    fatal: None,
    work_route: None,
});

fn log(text: String) {
    if let Ok(line) = CString::new(text) {
        // SAFETY: NUL-terminated local string for a synchronous call.
        unsafe { dsound_hle_log_line(line.as_ptr()) };
    }
}

fn refuse(entry: u32, reason: &str) -> ! {
    let fatal = STATE.lock().unwrap().fatal;
    log(format!(
        "dsound passive listener {entry:#x} refused: {reason}\n"
    ));
    if let (Some(fatal), Ok(reason)) = (fatal, CString::new(reason)) {
        // SAFETY: NUL-terminated string for a synchronous call.
        unsafe { fatal(entry, reason.as_ptr()) };
    }
    std::process::abort();
}

/// `dsound_listener_set_enabled`.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_listener_set_enabled(enabled: bool) {
    STATE.lock().unwrap().enabled = enabled;
}

/// `dsound_listener_set_irql_provider`.
///
/// # Safety
/// `provider` is null or `bool (*)(uint8_t *)`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dsound_listener_set_irql_provider(provider: Erased) {
    // SAFETY: erased pointer restored to its declared C signature.
    STATE.lock().unwrap().irql = provider.map(|f| unsafe { std::mem::transmute::<_, IrqlFn>(f) });
}

/// `dsound_listener_set_fatal`.
///
/// # Safety
/// `fatal` is null or `void (*)(uint32_t, const char *)`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dsound_listener_set_fatal(fatal: Erased) {
    // SAFETY: erased pointer restored to its declared C signature.
    STATE.lock().unwrap().fatal = fatal.map(|f| unsafe { std::mem::transmute::<_, FatalFn>(f) });
}

/// `dsound_listener_set_work_route`.
///
/// # Safety
/// `route` is null or `bool (*)(uint32_t)`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dsound_listener_set_work_route(route: Erased) {
    // SAFETY: erased pointer restored to its declared C signature.
    STATE.lock().unwrap().work_route =
        route.map(|f| unsafe { std::mem::transmute::<_, RouteFn>(f) });
}

/// `dsound_listener_reset`.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_listener_reset() {
    STATE.lock().unwrap().cache.reset();
}

fn env(state: &State) -> ListenerEnv {
    let irql = state.irql.and_then(|f| {
        let mut level = 0u8;
        // SAFETY: the hook writes one byte through the live local.
        unsafe { f(&mut level) }.then_some(level)
    });
    let mut word = 0u32;
    // SAFETY: checked guest read into a live local.
    let global = unsafe { kernel_guest_read_u32(0x0041_24A8, &mut word) }.then_some(word);
    ListenerEnv {
        enabled: state.enabled,
        irql,
        global_state: global,
    }
}

/// Mirror of `dsound_listener_snapshot`.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct CSnapshot {
    device_heap: u32,
    internal_address: u32,
    cache_mask: u32,
    doppler_bits: u32,
    position: [u32; 3],
    orientation: [u32; 6],
    commit_seen: bool,
    work_seen: bool,
}

impl From<ListenerSnapshot> for CSnapshot {
    fn from(s: ListenerSnapshot) -> Self {
        Self {
            device_heap: s.identity.device_heap,
            internal_address: s.identity.internal_address,
            cache_mask: s.cache_mask,
            doppler_bits: s.doppler_bits,
            position: s.position,
            orientation: s.orientation,
            commit_seen: s.commit_seen,
            work_seen: s.work_seen,
        }
    }
}

struct Request<'a> {
    state: &'a mut State,
    call: Option<ListenerCall>,
    env: ListenerEnv,
    snapshot: Option<CSnapshot>,
    outcome: Option<Result<listener::Accepted, Refusal>>,
}

unsafe extern "C" fn owned(device: *const u32, userdata: *mut c_void, result: *mut u32) {
    // SAFETY: userdata is the live `Request` below; device points at (heap, address) under the device mutex.
    let (request, identity) = unsafe {
        (
            &mut *userdata.cast::<Request>(),
            DeviceIdentity {
                device_heap: *device,
                internal_address: *device.add(1),
            },
        )
    };
    match request.call {
        None => {
            request.snapshot = request
                .state
                .cache
                .snapshot(Some(identity))
                .map(CSnapshot::from);
        }
        Some(call) => {
            let outcome = request.state.cache.call(Some(identity), call, request.env);
            if outcome.is_ok() {
                // SAFETY: the caller passes a writable result slot.
                unsafe { *result = 0 };
            }
            request.outcome = Some(outcome);
        }
    }
}

fn invoke(interface: u32, call: ListenerCall) -> u32 {
    let entry = call.entry();
    let mut state = STATE.lock().unwrap();
    let env = env(&state);
    let mut request = Request {
        state: &mut state,
        call: Some(call),
        env,
        snapshot: None,
        outcome: None,
    };
    let mut result = 0u32;
    // SAFETY: `request` and `result` outlive the synchronous call.
    let ran = unsafe {
        tsfp_audio_device_with_owned_identity(
            interface,
            Some(owned),
            (&mut request as *mut Request).cast(),
            &mut result,
        )
    };
    let outcome = request.outcome.take();
    drop(state);
    let Some(outcome) = outcome.filter(|_| ran) else {
        refuse(entry, "unowned or changed device");
    };
    match outcome {
        Ok(accepted) => {
            if accepted.announce {
                log("dsound explicit passive listener CPU cache; omitted guest settings/derived3D/voice-list/APU/DSP/FP/critical-section effects; no audio\n".into());
            }
            accepted.result
        }
        Err(reason) => {
            let text = CString::new(reason.0).unwrap_or_default();
            // SAFETY: NUL-terminated strings for a synchronous call.
            if unsafe { dsound_hle_degrade(c"listener".as_ptr(), entry, text.as_ptr()) } {
                0
            } else {
                refuse(entry, reason.0)
            }
        }
    }
}

/// `dsound_listener_cache_doppler`.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_listener_cache_doppler(interface: u32, factor: u32, apply: u32) -> u32 {
    invoke(interface, ListenerCall::Doppler { factor, apply })
}

/// `dsound_listener_cache_position`.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_listener_cache_position(
    interface: u32,
    x: u32,
    y: u32,
    z: u32,
    apply: u32,
) -> u32 {
    invoke(
        interface,
        ListenerCall::Position {
            xyz: [x, y, z],
            apply,
        },
    )
}

/// `dsound_listener_cache_orientation`.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_listener_cache_orientation(
    interface: u32,
    fx: u32,
    fy: u32,
    fz: u32,
    tx: u32,
    ty: u32,
    tz: u32,
    apply: u32,
) -> u32 {
    invoke(
        interface,
        ListenerCall::Orientation {
            front_top: [fx, fy, fz, tx, ty, tz],
            apply,
        },
    )
}

/// `dsound_listener_cache_commit`.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_listener_cache_commit(interface: u32) -> u32 {
    invoke(interface, ListenerCall::Commit)
}

/// `dsound_listener_cache_work`.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_listener_cache_work() -> u32 {
    let mut internal = 0u32;
    // SAFETY: checked guest read into a live local.
    let singleton = unsafe { kernel_guest_read_u32(listener::DEVICE_SINGLETON, &mut internal) }
        .then_some(internal);
    match ListenerCache::work_interface(singleton) {
        Ok(interface) => invoke(interface, ListenerCall::Work),
        Err(reason) => refuse(listener::WORK, reason.0),
    }
}

/// `dsound_listener_get_snapshot`.
///
/// # Safety
/// `output` is null or a writable `dsound_listener_snapshot`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dsound_listener_get_snapshot(
    interface: u32,
    output: *mut CSnapshot,
) -> bool {
    if output.is_null() {
        return false;
    }
    let mut state = STATE.lock().unwrap();
    let mut request = Request {
        state: &mut state,
        call: None,
        env: ListenerEnv::default(),
        snapshot: None,
        outcome: None,
    };
    let mut result = 0u32;
    // SAFETY: same contract as `invoke`.
    let ran = unsafe {
        tsfp_audio_device_with_owned_identity(
            interface,
            Some(owned),
            (&mut request as *mut Request).cast(),
            &mut result,
        )
    };
    let snapshot = request.snapshot.take();
    drop(state);
    match snapshot.filter(|_| ran) {
        Some(value) => {
            // SAFETY: non-null writable output by contract.
            unsafe { *output = value };
            true
        }
        None => false,
    }
}

/// The measured-caller admission and argument read shared by every entry (`handler` of the C).
fn call_handler(frame: *mut c_void, entry: u32, caller: u32, count: u32) -> u32 {
    let (mut pid, mut stack, mut limit) = (0i32, 0u32, 0u32);
    // SAFETY: the registry passes the live call frame; the outputs are live locals.
    let has_frame =
        unsafe { tsfp_audio_movie_release_frame_snapshot(frame, &mut pid, &mut stack, &mut limit) };
    if !has_frame {
        refuse(entry, "unsupported caller");
    }
    if entry == listener::WORK
        && (u64::from(stack) + 4 > 0x1_0000_0000
            || (limit != 0 && u64::from(stack) + 4 > u64::from(limit)))
    {
        refuse(entry, "unsupported caller");
    }
    let mut actual = 0u32;
    // SAFETY: checked guest read into a live local.
    if !unsafe { kernel_guest_read_u32(stack, &mut actual) } || actual != caller {
        refuse(entry, "unsupported caller");
    }
    let mut args = [0u32; 8];
    for (i, slot) in args.iter_mut().enumerate().take(count as usize) {
        // SAFETY: the live frame and a live slot.
        if !unsafe { kernel_frame_arg(frame, i as u32, slot) } {
            refuse(entry, "unreadable argument");
        }
    }
    match entry {
        listener::WORK => dsound_listener_cache_work(),
        listener::COMMIT => dsound_listener_cache_commit(args[0]),
        listener::DOPPLER => dsound_listener_cache_doppler(args[0], args[1], args[2]),
        listener::POSITION => {
            dsound_listener_cache_position(args[0], args[1], args[2], args[3], args[4])
        }
        _ => dsound_listener_cache_orientation(
            args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7],
        ),
    }
}

unsafe extern "C" fn doppler_handler(frame: *mut c_void) -> u32 {
    call_handler(frame, listener::DOPPLER, 0x27B78, 3)
}
unsafe extern "C" fn position_handler(frame: *mut c_void) -> u32 {
    call_handler(frame, listener::POSITION, 0x27B88, 5)
}
unsafe extern "C" fn orientation_handler(frame: *mut c_void) -> u32 {
    call_handler(frame, listener::ORIENTATION, 0x27BA2, 8)
}
unsafe extern "C" fn commit_handler(frame: *mut c_void) -> u32 {
    call_handler(frame, listener::COMMIT, 0x2887F, 1)
}

unsafe extern "C" fn work_handler(frame: *mut c_void) -> u32 {
    let (mut pid, mut stack, mut limit) = (0i32, 0u32, 0u32);
    let mut caller = 0u32;
    // SAFETY: the registry passes the live call frame; the outputs are live locals.
    let have = unsafe {
        tsfp_audio_movie_release_frame_snapshot(frame, &mut pid, &mut stack, &mut limit)
            && kernel_guest_read_u32(stack, &mut caller)
    };
    let route = STATE.lock().unwrap().work_route;
    // T392: an installed route (the movie stream) may take DoWork from its own callers first.
    // SAFETY: the route is the host's `bool (*)(uint32_t)`.
    if let Some(route) = route
        && have
        && unsafe { route(caller) }
    {
        return 0;
    }
    let expected = if have && matches!(caller, 0x1CE573 | 0x1CE4CA | 0x1CE50A | 0x1CE5C8) {
        caller
    } else if have && caller == 0x1CE45F {
        0x1CE45F
    } else {
        0x1CE492
    };
    call_handler(frame, listener::WORK, expected, 0)
}

/// `dsound_listener_register`: the number of entries registered.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_listener_register() -> usize {
    let entries: [(u32, DsoundHandler); 5] = [
        (listener::DOPPLER, doppler_handler),
        (listener::POSITION, position_handler),
        (listener::ORIENTATION, orientation_handler),
        (listener::COMMIT, commit_handler),
        (listener::WORK, work_handler),
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
