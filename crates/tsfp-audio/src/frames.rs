//! Stream HLE frame contracts (T2046 slice 9b): the pure logic of `frame_handler`, `max_distance_handler`,
//! `set_position_handler`, `rolloff_handler`, `set_frequency_handler` and `routing_handler` in `dsound_stream.c`
//! as Rust over [`GuestWorld`]. The Rust audio host wires measured callbacks incrementally through
//! `tsfp-audio-capi`; stream Create is registered in `stream_create_hle_ffi`, while its exact lease/heap
//! transaction remains the C oracle. A dispatcher decodes the return address at `sp` and argument `i` at
//! `sp + 4 + 4 * i`, matching `kernel_frame_arg` in `kernel_call.h`.
//!
//! Return-address admission is data ([`STREAM_FRAMES`]): a caller outside the table is a [`Refusal`] (a stop),
//! never a silent accept. The movie stream extension route (`public_route`, T392) is not ported, and the
//! discontinuity and startup GetStatus entries have no C frame handler in `dsound_stream.c` (they are reached
//! through other registrations), so they are absent here too.
//!
//! Evidence class: Tier 1 against the C source, no recorded call log exists (see docs/t-audio-replace.md).

use crate::adapter::{GuestMemory, Refusal};
use crate::device_frames::CallFrame;
use crate::lifecycle::{Device, GuestWorld, Objects};
use crate::routing::StreamHooks;
use crate::setters::Setters;

pub const CREATE: u32 = 0x0040_967C;
pub const I3DL2: u32 = 0x0040_8637;
pub const MINIMUM: u32 = 0x0040_85F1;
pub const MAX_DISTANCE: u32 = 0x0040_85D9;
pub const SET_POSITION: u32 = 0x0040_8609;
pub const ROLLOFF: u32 = 0x0040_8632;
pub const VOLUME: u32 = 0x0040_7B14;
pub const PAUSE: u32 = 0x0040_7B23;
pub const SET_FORMAT: u32 = 0x0040_8C2D;
pub const HEADROOM: u32 = 0x0040_7B19;
pub const MIX_VOLUMES: u32 = 0x0040_7B1E;
pub const MIX_BINS: u32 = 0x0040_85D4;
pub const FLUSH_EX: u32 = 0x0040_7B28;
pub const SET_FREQUENCY: u32 = 0x0040_85CF;

/// Pause caller that reaches the started stream (admitted only with the completion model, class 2).
pub const PAUSE_RUNTIME_CALLER: u32 = 0x29ED1;
/// Pause caller right after SetFormat (class 1).
pub const PAUSE_AFTER_FORMAT_CALLER: u32 = 0x29A51;
/// SetVolume caller right after the re-format (needs a recorded SetFormat).
pub const VOLUME_AFTER_FORMAT_CALLER: u32 = 0x29A67;
/// The only SetFrequency caller.
pub const FREQUENCY_CALLER: u32 = 0x29B35;

/// One decoded stream HLE frame. This contains only the admitted caller and stack words;
/// guest object lookup and state application remain separate contracts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameCall {
    pub entry: u32,
    pub caller: u32,
    pub args: [u32; 5],
}

fn frame_word(memory: &impl GuestMemory, frame: CallFrame, offset: u64) -> Option<u32> {
    let address = u64::from(frame.stack_pointer).checked_add(offset)?;
    let end = address.checked_add(4)?;
    if frame.stack_pointer == 0
        || end > (1u64 << 32)
        || (frame.stack_limit != 0 && end > u64::from(frame.stack_limit))
    {
        return None;
    }
    memory.read_u32(address as u32)
}

/// Decode and admit one stream HLE stack frame without changing guest or Rust-owned state.
/// `completion` is the existing adapter policy that widens only the recorded Pause callers.
pub fn decode_frame(
    memory: &impl GuestMemory,
    entry: u32,
    frame: CallFrame,
    completion: bool,
) -> Result<FrameCall, Refusal> {
    let spec = STREAM_FRAMES.iter().find(|spec| spec.entry == entry);
    let routing = matches!(entry, HEADROOM | MIX_BINS | MIX_VOLUMES);
    let frequency = entry == SET_FREQUENCY;
    let (argc, caller) = if let Some(spec) = spec {
        let caller =
            frame_word(memory, frame, 0).ok_or(Refusal("unreadable stream return address"))?;
        if !admit(spec, caller, completion) {
            return Err(Refusal("only measured stream caller is supported"));
        }
        (spec.args, caller)
    } else if frequency {
        let caller =
            frame_word(memory, frame, 0).ok_or(Refusal("unreadable stream return address"))?;
        if caller != FREQUENCY_CALLER {
            return Err(Refusal(
                "only measured stream frequency caller is supported",
            ));
        }
        (2, caller)
    } else if routing {
        (2, 0)
    } else {
        return Err(Refusal("no stream frame handler for this entry"));
    };

    let mut args = [0; 5];
    for (index, slot) in args.iter_mut().enumerate().take(argc) {
        let offset = 4 + 4 * index as u64;
        *slot = frame_word(memory, frame, offset)
            .ok_or(Refusal("unreadable or out-of-bounds stream argument"))?;
    }
    Ok(FrameCall {
        entry,
        caller,
        args,
    })
}

/// One admitted entry: the VA, its measured return addresses (title code offsets, as the C compares them),
/// the callers admitted only under the completion model, and the argument count.
pub struct FrameSpec {
    pub entry: u32,
    pub callers: &'static [u32],
    pub completion_callers: &'static [u32],
    pub args: usize,
}

const fn spec(
    entry: u32,
    callers: &'static [u32],
    completion_callers: &'static [u32],
    args: usize,
) -> FrameSpec {
    FrameSpec {
        entry,
        callers,
        completion_callers,
        args,
    }
}

/// The measured caller set (oracle: the `*_handler` one-liners and the explicit handlers of `dsound_stream.c`).
/// Routing entries and SetFrequency are handled apart (no caller table, single caller).
pub const STREAM_FRAMES: &[FrameSpec] = &[
    spec(CREATE, &[0x29943], &[], 2),
    spec(I3DL2, &[0x2998A], &[], 3),
    spec(MINIMUM, &[0x29998], &[], 3),
    spec(MAX_DISTANCE, &[0x29ADA, 0x29D86], &[], 3),
    spec(SET_POSITION, &[0x29ACA, 0x29E2B, 0x29FAD], &[], 5),
    spec(ROLLOFF, &[0x299A8, 0x29E81, 0x29E90], &[], 4),
    spec(
        PAUSE,
        &[0x29B5A, PAUSE_AFTER_FORMAT_CALLER],
        &[0x29F13, PAUSE_RUNTIME_CALLER],
        2,
    ),
    spec(FLUSH_EX, &[0x29B69], &[], 4),
    spec(SET_FORMAT, &[0x299FA], &[], 2),
    spec(VOLUME, &[0x29F68, VOLUME_AFTER_FORMAT_CALLER], &[], 2),
];

/// Oracle caller admission: the return address must be in `callers`, or in `completion_callers` when the
/// completion model is on.
pub fn admit(spec: &FrameSpec, actual: u32, completion: bool) -> bool {
    spec.callers.contains(&actual) || (completion && spec.completion_callers.contains(&actual))
}

fn arg<W: GuestWorld + ?Sized>(w: &W, sp: u32, i: u32) -> Result<u32, Refusal> {
    w.read_u32(sp.wrapping_add(4 + 4 * i))
        .ok_or(Refusal("unreadable argument"))
}

/// Dispatch one stream HLE call by entry VA. `sp` points at the return address. `pause_note` receives the
/// accepted Pause (stream, mode). Returns the guest EAX; a [`Refusal`] is a host stop.
#[allow(clippy::too_many_arguments)]
pub fn stream_frame<W: GuestWorld, H: StreamHooks>(
    s: &mut Setters,
    objs: &mut Objects,
    device: &mut Device,
    w: &mut W,
    hooks: &mut H,
    entry: u32,
    sp: u32,
    pause_note: &mut dyn FnMut(u32, u32),
) -> Result<u32, Refusal> {
    let completion = s.policy.completion;
    match entry {
        SET_FREQUENCY => {
            if let Some(fail) = s.hooked_preflight(&*w, hooks.frequency_installed())? {
                return Ok(fail);
            }
            let actual = w.read_u32(sp).unwrap_or(0);
            if actual != FREQUENCY_CALLER {
                return Err(Refusal("only measured return address 0x29B35 is supported"));
            }
            let (stream, hertz) = (arg(&*w, sp, 0)?, arg(&*w, sp, 1)?);
            let cw = hooks
                .control_word()
                .ok_or(Refusal("guest x87 control word unavailable"))?;
            s.stream_frequency(objs, device, &*w, stream, hertz, cw, hooks)
        }
        HEADROOM | MIX_BINS | MIX_VOLUMES => {
            // Original routing wrappers have no caller restriction: preflight, then the checked adapter.
            if let Some(fail) = s.hooked_preflight(&*w, hooks.routing_installed())? {
                return Ok(fail);
            }
            let (stream, a) = (arg(&*w, sp, 0)?, arg(&*w, sp, 1)?);
            match entry {
                HEADROOM => s.stream_headroom(objs, device, &*w, stream, a, hooks),
                MIX_BINS => s.stream_mix_bins(objs, device, &*w, stream, a, hooks),
                _ => s.stream_mix_bin_volumes(objs, device, &*w, stream, a, hooks),
            }
        }
        _ => {
            let spec = STREAM_FRAMES
                .iter()
                .find(|f| f.entry == entry)
                .ok_or(Refusal("no stream frame handler for this entry"))?;
            let actual = w.read_u32(sp).unwrap_or(0);
            if !admit(spec, actual, completion) {
                return Err(Refusal("only measured startup caller is supported"));
            }
            let mut a = [0u32; 5];
            for (i, slot) in a.iter_mut().enumerate().take(spec.args) {
                *slot = arg(&*w, sp, i as u32)?;
            }
            match entry {
                CREATE => s.create_stream(objs, device, w, a[0], a[1]),
                I3DL2 => s.stream_i3dl2(objs, device, &*w, a[0], a[1], a[2]),
                MINIMUM => s.stream_min_distance(objs, device, &*w, a[0], a[1], a[2]),
                MAX_DISTANCE => s.stream_max_distance(objs, device, &*w, a[0], a[1], a[2]),
                SET_POSITION => {
                    s.stream_position(objs, device, &*w, a[0], [a[1], a[2], a[3]], a[4])
                }
                ROLLOFF => s.stream_rolloff(objs, device, &*w, a[0], a[1], a[2], a[3]),
                SET_FORMAT => s.stream_set_format_with(objs, device, &*w, a[0], a[1], hooks),
                FLUSH_EX => s.stream_flush_ex(objs, device, &*w, a[0], a[1], a[2], a[3]),
                VOLUME => {
                    let second = actual == VOLUME_AFTER_FORMAT_CALLER;
                    s.stream_volume_with(objs, device, &*w, a[0], a[1], second, hooks)
                }
                _ => {
                    let class = match actual {
                        PAUSE_AFTER_FORMAT_CALLER => 1,
                        PAUSE_RUNTIME_CALLER => 2,
                        _ => 0,
                    };
                    s.stream_pause_class(objs, device, &*w, a[0], a[1], class, pause_note)
                }
            }
        }
    }
}
