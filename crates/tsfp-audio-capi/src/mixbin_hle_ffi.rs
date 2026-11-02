// SPDX-License-Identifier: GPL-3.0-or-later
//! T3933: Rust-owned IDirectSound_SetMixBinHeadroom front-end for the Rust audio host. Replaces
//! `src/audio/dsound_mixbin_headroom.c` (kept as the `TSFP_RUST_AUDIO_HOST=0` fallback) under the same
//! C symbols; the state machine is `tsfp_audio::mixbin`. The C file's lock is the state mutex here.

use std::ffi::{CString, c_char, c_void};
use std::sync::Mutex;

use tsfp_audio::adapter::Refusal;
use tsfp_audio::mixbin::{self, BINS, MixbinEnv, MixbinHeadroom};

use crate::device_ffi::tsfp_audio_device_with_owned_identity;
use crate::movie_device_hle_ffi::tsfp_audio_hle_register_handler_rust;

type NoteFn = unsafe extern "C" fn(u64, u32, u32) -> bool;
type BindFn = unsafe extern "C" fn(u64) -> bool;
type IrqlFn = unsafe extern "C" fn(*mut u8) -> bool;
type FatalFn = unsafe extern "C" fn(u32, *const c_char);
type CreateNoteFn = unsafe extern "C" fn(u32, u32) -> bool;
type Erased = Option<unsafe extern "C" fn()>;

unsafe extern "C" {
    fn dsound_device_set_create_note(note: Option<CreateNoteFn>);
    fn dsound_hle_degrade(kind: *const c_char, entry: u32, reason: *const c_char) -> bool;
    fn dsound_hle_register_rust(address: u32) -> bool;
    fn dsound_hle_log_line(line: *const c_char);
    fn kernel_guest_read_u32(address: u32, out: *mut u32) -> bool;
    fn kernel_frame_arg(frame: *const c_void, index: u32, out: *mut u32) -> bool;
}

const GLOBAL_STATE: u32 = 0x0041_24A8;

#[derive(Default)]
struct State {
    model: MixbinHeadroom,
    note: Option<NoteFn>,
    bind: Option<BindFn>,
    irql: Option<IrqlFn>,
    fatal: Option<FatalFn>,
}

static STATE: Mutex<State> = Mutex::new(State {
    model: MixbinHeadroom::new(),
    note: None,
    bind: None,
    irql: None,
    fatal: None,
});

fn refuse(reason: &'static str) -> ! {
    let fatal = STATE.lock().unwrap().fatal;
    let line = CString::new(format!(
        "dsound global mix-bin headroom refused: {reason}\n"
    ))
    .unwrap_or_default();
    // SAFETY: NUL-terminated local strings for synchronous calls; the fatal hook never returns for the host.
    unsafe {
        dsound_hle_log_line(line.as_ptr());
        if let Some(fatal) = fatal {
            let reason = CString::new(reason).unwrap_or_default();
            fatal(mixbin::ENTRY, reason.as_ptr());
        }
    }
    std::process::abort();
}

unsafe extern "C" fn bind_device(heap: u32, internal: u32) -> bool {
    let mut state = STATE.lock().unwrap();
    let identity = mixbin::pack_identity(heap, internal);
    // SAFETY: the bind hook is the completion model's `bool (*)(uint64_t)`.
    let ok = state.bind.is_some_and(|bind| unsafe { bind(identity) });
    state.model.bind(identity, ok)
}

/// `dsound_mixbin_headroom_configure`. The erased pointers are the C hooks of the wiring.
///
/// # Safety
/// Each non-null pointer has the signature named by its parameter's C contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dsound_mixbin_headroom_configure(
    note: Erased,
    bind: Erased,
    irql: Erased,
    fatal: Erased,
) {
    {
        let mut state = STATE.lock().unwrap();
        // SAFETY: erased function pointers are transmuted back to their declared C signatures.
        unsafe {
            state.note = note.map(|f| std::mem::transmute::<_, NoteFn>(f));
            state.bind = bind.map(|f| std::mem::transmute::<_, BindFn>(f));
            state.irql = irql.map(|f| std::mem::transmute::<_, IrqlFn>(f));
            state.fatal = fatal.map(|f| std::mem::transmute::<_, FatalFn>(f));
        }
        state.model.configure();
    }
    // SAFETY: registering a plain function pointer (or none) with the C device module.
    unsafe { dsound_device_set_create_note(note.map(|_| bind_device as CreateNoteFn)) };
}

struct Owned<'a> {
    state: &'a mut State,
    env: MixbinEnv,
    bin: u32,
    headroom: u32,
    snapshot: Option<&'a mut [u8; BINS]>,
    result: Result<u32, Refusal>,
}

unsafe extern "C" fn owned(device: *const u32, userdata: *mut c_void, result: *mut u32) {
    // SAFETY: userdata is the live `Owned` below; device points at (heap, address) under the device mutex.
    let (owned, identity) = unsafe {
        (
            &mut *userdata.cast::<Owned>(),
            mixbin::pack_identity(*device, *device.add(1)),
        )
    };
    if let Some(out) = owned.snapshot.as_deref_mut() {
        *out = owned
            .state
            .model
            .snapshot(Some(identity))
            .unwrap_or_default();
        // SAFETY: the caller passes a writable result slot.
        unsafe { *result = 0 };
        owned.result = Ok(0);
        return;
    }
    let note = owned.state.note;
    let (bin, headroom, env) = (owned.bin, owned.headroom, owned.env);
    owned.result = owned.state.model.set(
        env,
        || Some(identity),
        bin,
        headroom,
        |id, b, h| {
            // SAFETY: the note hook is the completion model's `bool (*)(uint64_t, uint32_t, uint32_t)`.
            note.is_some_and(|note| unsafe { note(id, b, h) })
        },
    );
}

/// `dsound_mixbin_headroom_set`.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_mixbin_headroom_set(interface: u32, bin: u32, headroom: u32) -> u32 {
    let (policy, irql) = {
        let state = STATE.lock().unwrap();
        let mut level = 0u8;
        // SAFETY: the hook writes one byte through the live local.
        let known = state.irql.is_some_and(|f| unsafe { f(&mut level) });
        (state.note.is_some(), known.then_some(level))
    };
    if !policy || irql != Some(0) {
        refuse("requires explicit PCM policy and known IRQL0");
    }
    let mut word = 0u32;
    // SAFETY: checked guest read into a live local.
    let global = unsafe { kernel_guest_read_u32(GLOBAL_STATE, &mut word) }.then_some(word);
    // The model orders its checks like the C: policy, global word, device ownership, bin range, renderer.
    // Global-word failures need no device, so they are answered before the ownership walk.
    match global {
        None => refuse("global audio state inaccessible"),
        Some(0) => {}
        Some(_) => return mixbin::E_FAIL,
    }
    let mut state = STATE.lock().unwrap();
    let mut owned_call = Owned {
        state: &mut state,
        env: MixbinEnv {
            pcm_policy: policy,
            irql,
            global_state: global,
        },
        bin,
        headroom,
        snapshot: None,
        result: Err(Refusal("device ownership/header invalid")),
    };
    let mut result = 0u32;
    // SAFETY: `owned_call` and `result` outlive the synchronous call.
    let ran = unsafe {
        tsfp_audio_device_with_owned_identity(
            interface,
            Some(owned),
            (&mut owned_call as *mut Owned).cast(),
            &mut result,
        )
    };
    let outcome = owned_call.result;
    drop(state);
    if !ran {
        refuse("device ownership/header invalid");
    }
    match outcome {
        Ok(value) => value,
        Err(reason) => {
            let (kind, text) = (c"mixbin", CString::new(reason.0).unwrap_or_default());
            // SAFETY: NUL-terminated strings for a synchronous call.
            if unsafe { dsound_hle_degrade(kind.as_ptr(), mixbin::ENTRY, text.as_ptr()) } {
                0
            } else {
                refuse(reason.0)
            }
        }
    }
}

/// `dsound_mixbin_headroom_snapshot`.
///
/// # Safety
/// `amounts` is null or points to 32 writable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dsound_mixbin_headroom_snapshot(interface: u32, amounts: *mut u8) -> bool {
    if amounts.is_null() {
        return false;
    }
    let mut table = [0u8; BINS];
    let mut state = STATE.lock().unwrap();
    let mut call = Owned {
        state: &mut state,
        env: MixbinEnv::default(),
        bin: 0,
        headroom: 0,
        snapshot: Some(&mut table),
        result: Ok(0),
    };
    let mut result = 0u32;
    // SAFETY: same contract as `dsound_mixbin_headroom_set`.
    let ran = unsafe {
        tsfp_audio_device_with_owned_identity(
            interface,
            Some(owned),
            (&mut call as *mut Owned).cast(),
            &mut result,
        )
    };
    drop(state);
    if ran {
        // SAFETY: 32 writable bytes by contract.
        unsafe { std::ptr::copy_nonoverlapping(table.as_ptr(), amounts, BINS) };
    }
    ran
}

unsafe extern "C" fn handler(frame: *mut c_void) -> u32 {
    let mut args = [0u32; 3];
    for (i, slot) in args.iter_mut().enumerate() {
        // SAFETY: the registry passes the live call frame; the slot is a live local.
        if !unsafe { kernel_frame_arg(frame, i as u32, slot) } {
            refuse("unreadable original stack argument");
        }
    }
    dsound_mixbin_headroom_set(args[0], args[1], args[2])
}

/// `dsound_mixbin_headroom_register`: 1 when PCM policy is active and the entry registered.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_mixbin_headroom_register() -> usize {
    if STATE.lock().unwrap().note.is_none() {
        return 0;
    }
    if !tsfp_audio_hle_register_handler_rust(mixbin::ENTRY, handler) {
        return 0;
    }
    // SAFETY: the C compatibility table is initialised before subsystem handlers register.
    usize::from(unsafe { dsound_hle_register_rust(mixbin::ENTRY) })
}
